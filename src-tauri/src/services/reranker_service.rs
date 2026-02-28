use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use fastembed::TextRerank;

use crate::db::models::{RankCandidate, RankedResult};
use crate::error::AppError;
use crate::services::hardware_service::{HardwareService, PerformanceMode};

pub struct RerankerService {
    model_name: String,
    hardware: Arc<HardwareService>,
    engine: Arc<Mutex<Option<TextRerank>>>,
    initialized: AtomicBool,
    last_used_secs: Arc<AtomicU64>,
    _cache_dir: PathBuf,
}

impl Clone for RerankerService {
    fn clone(&self) -> Self {
        Self {
            model_name: self.model_name.clone(),
            hardware: Arc::clone(&self.hardware),
            engine: Arc::clone(&self.engine),
            initialized: AtomicBool::new(self.initialized.load(Ordering::Relaxed)),
            last_used_secs: Arc::clone(&self.last_used_secs),
            _cache_dir: self._cache_dir.clone(),
        }
    }
}

impl RerankerService {
    pub fn new(
        model_name: &str,
        cache_dir: PathBuf,
        hardware: Arc<HardwareService>,
    ) -> Result<Self, AppError> {
        Ok(Self {
            model_name: model_name.to_string(),
            hardware,
            engine: Arc::new(Mutex::new(None)),
            initialized: AtomicBool::new(false),
            last_used_secs: Arc::new(AtomicU64::new(SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs())),
            _cache_dir: cache_dir,
        })
    }

    pub async fn ensure_initialized(&self) -> Result<(), AppError> {
        if self.initialized.load(Ordering::Relaxed) {
            return Ok(());
        }

        let is_none = {
            let guard = self
                .engine
                .lock()
                .map_err(|_| AppError::Embedding("Reranker engine lock poisoned".to_string()))?;
            guard.is_none()
        };

        if is_none {
            let mode = self.hardware.get_performance_mode(None).await;
            let stats = self.hardware.detect_hardware().await?;

            // Aggressively govern CPU thread limits for NLP processing batches
            if mode == PerformanceMode::Multitasking {
                let threads = (stats.cpu_threads.max(1) as usize / 4).clamp(1, 4);
                std::env::set_var("RAYON_NUM_THREADS", threads.to_string());
                crate::log_info!("sarah.reranker", "Restricting NLP threads to {}", threads);
            }

            let mut providers = vec![];
            
            // On Windows, use DirectML ONLY. DirectML provides GPU acceleration via DirectX 
            // and is native to Windows, avoiding the "missing cublasLt64_12.dll" errors 
            // common with the CUDA provider on systems without the full CUDA Toolkit.
            if stats.gpu_vram_mb.unwrap_or(0) >= 1024 {
                if cfg!(target_os = "windows") {
                    providers.push(ort::execution_providers::DirectMLExecutionProvider::default().build());
                } else {
                    providers.push(ort::execution_providers::CUDAExecutionProvider::default().build());
                }
                crate::log_info!("sarah.reranker", "Enabled ONNX GPU Execution Providers for Reranker");
            }

            let options = fastembed::RerankInitOptions::new(fastembed::RerankerModel::BGERerankerBase)
                .with_show_download_progress(true)
                .with_execution_providers(providers);

            let engine = TextRerank::try_new(options)
                .map_err(|e| AppError::Embedding(format!("Failed to initialize reranker: {e}")))?;
            
            {
                let mut guard = self
                    .engine
                    .lock()
                    .map_err(|_| AppError::Embedding("Reranker engine lock poisoned".to_string()))?;
                *guard = Some(engine);
            }
            
            self.initialized.store(true, Ordering::Relaxed);
            
            if mode == PerformanceMode::Multitasking {
                self.start_auto_unloader();
            }
        }

        Ok(())
    }

    fn start_auto_unloader(&self) {
        let engine_ref = self.engine.clone();
        let last_used_ref = self.last_used_secs.clone();
        
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
                let mut guard = if let Ok(g) = engine_ref.lock() { g } else { return; };
                
                if guard.is_some() {
                    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
                    let last_used = last_used_ref.load(Ordering::Relaxed);
                    // 5 minutes
                    if now.saturating_sub(last_used) > 300 {
                        crate::log_info!("sarah.reranker", "Reranker model idle for 5+ minutes in Multitasking mode. Auto-unloading from memory.");
                        *guard = None; // Drop reranker model
                        break;
                    }
                } else {
                    break;
                }
            }
        });
    }

    pub fn is_initialized(&self) -> bool {
        self.initialized.load(Ordering::Relaxed)
    }

    pub async fn rerank(
        &self,
        query: &str,
        candidates: Vec<RankCandidate>,
    ) -> Result<Vec<RankedResult>, AppError> {
        if candidates.is_empty() {
            return Ok(Vec::new());
        }

        self.ensure_initialized().await?;
        self.last_used_secs.store(SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(), Ordering::Relaxed);

        let docs: Vec<String> = candidates
            .iter()
            .map(|candidate| candidate.text.clone())
            .collect();
        let candidates_clone = candidates.clone();
        let query_owned = query.to_string();
        let engine = self.engine.clone();

        let rows = tokio::task::spawn_blocking(move || {
            let mut guard = engine
                .lock()
                .map_err(|_| AppError::Embedding("Reranker lock poisoned".to_string()))?;
            let engine = guard.as_mut().ok_or_else(|| {
                AppError::Embedding("Reranker engine not initialized".to_string())
            })?;
            let results = engine
                .rerank(query_owned, docs, false, None)
                .map_err(|e| AppError::Embedding(format!("Rerank failed: {e}")))?;

            let mut ranked = Vec::with_capacity(results.len());
            for result in results {
                if let Some(candidate) = candidates_clone.get(result.index) {
                    ranked.push(RankedResult {
                        id: candidate.id.clone(),
                        score: result.score,
                        metadata: candidate.metadata.clone(),
                    });
                }
            }
            Ok::<Vec<RankedResult>, AppError>(ranked)
        })
        .await
        .map_err(|e| AppError::Embedding(e.to_string()))??;

        Ok(rows)
    }

    pub fn model_name(&self) -> &str {
        &self.model_name
    }
}
