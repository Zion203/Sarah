use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use fastembed::{InitOptions, TextEmbedding};
use moka::future::Cache;

use crate::error::AppError;
use crate::repositories::embedding_repo::EmbeddingRepo;
use crate::services::hardware_service::{HardwareService, PerformanceMode};

pub struct EmbeddingService {
    model_name: String,
    hardware: Arc<HardwareService>,
    engine: Arc<Mutex<Option<TextEmbedding>>>,
    initialized: AtomicBool,
    last_used_secs: Arc<AtomicU64>,
    cache: Cache<u64, Vec<f32>>,
    embedding_repo: EmbeddingRepo,
    _cache_dir: PathBuf,
}

impl Clone for EmbeddingService {
    fn clone(&self) -> Self {
        Self {
            model_name: self.model_name.clone(),
            hardware: Arc::clone(&self.hardware),
            engine: Arc::clone(&self.engine),
            initialized: AtomicBool::new(self.initialized.load(Ordering::Relaxed)),
            last_used_secs: Arc::clone(&self.last_used_secs),
            cache: self.cache.clone(),
            embedding_repo: self.embedding_repo.clone(),
            _cache_dir: self._cache_dir.clone(),
        }
    }
}

impl EmbeddingService {
    pub fn new(
        model_name: &str,
        cache_dir: PathBuf,
        embedding_repo: EmbeddingRepo,
        hardware: Arc<HardwareService>,
    ) -> Result<Self, AppError> {
        Ok(Self {
            model_name: model_name.to_string(),
            hardware,
            engine: Arc::new(Mutex::new(None)),
            initialized: AtomicBool::new(false),
            last_used_secs: Arc::new(AtomicU64::new(SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs())),
            cache: Cache::builder()
                .time_to_live(std::time::Duration::from_secs(60 * 60 * 24))
                .max_capacity(25_000)
                .build(),
            embedding_repo,
            _cache_dir: cache_dir,
        })
    }

    pub async fn ensure_initialized(&self) -> Result<(), AppError> {
        if self.initialized.load(Ordering::Relaxed) {
            return Ok(());
        }

        let is_initialized = {
            let guard = self
                .engine
                .lock()
                .map_err(|_| AppError::Embedding("Embedding engine lock poisoned".to_string()))?;
            guard.is_some()
        };

        if !is_initialized {
            let mode = self.hardware.get_performance_mode(None).await;
            let stats = self.hardware.detect_hardware().await?;
            let _tier = stats.classify();
            // Aggressively govern CPU thread limits for NLP processing batches
            if mode == PerformanceMode::Multitasking {
                let threads = (stats.cpu_threads.max(1) as usize / 4).clamp(1, 4);
                std::env::set_var("RAYON_NUM_THREADS", threads.to_string());
                crate::log_info!("sarah.embedding", "Restricting NLP threads to {}", threads);
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
                crate::log_info!("sarah.embedding", "Enabled ONNX GPU Execution Providers for Embeddings");
            }

            let options = InitOptions::new(fastembed::EmbeddingModel::BGESmallENV15)
                .with_show_download_progress(true)
                .with_execution_providers(providers);

            let engine = TextEmbedding::try_new(options)
                .map_err(|e| AppError::Embedding(format!("Failed to initialize fastembed: {e}")))?;

            {
                let mut guard = self
                    .engine
                    .lock()
                    .map_err(|_| AppError::Embedding("Embedding engine lock poisoned".to_string()))?;
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
                        crate::log_info!("sarah.embedding", "Embedding model idle for 5+ minutes in Multitasking mode. Auto-unloading from memory.");
                        *guard = None; // Drop embedding model
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

    pub async fn embed_text(&self, text: &str) -> Result<Vec<f32>, AppError> {
        let key = self.hash_text(text);
        if let Some(value) = self.cache.get(&key).await {
            return Ok(value);
        }

        self.ensure_initialized().await?;
        self.last_used_secs.store(SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(), Ordering::Relaxed);

        let text_owned = text.to_string();
        let engine_arc = self.engine.clone();
        
        let vector = tokio::task::spawn_blocking(move || {
            let mut guard = engine_arc.lock().map_err(|_| {
                AppError::Embedding("Embedding engine lock poisoned".to_string())
            })?;
            let engine = guard.as_mut().ok_or_else(|| {
                AppError::Embedding("Embedding engine not initialized".to_string())
            })?;
            
            let embeddings = engine
                .embed(vec![text_owned], None)
                .map_err(|e| AppError::Embedding(format!("fastembed embed failed: {e}")))?;

            embeddings
                .into_iter()
                .next()
                .ok_or_else(|| AppError::Embedding("No embedding was generated".to_string()))
        })
        .await
        .map_err(|e| AppError::Embedding(format!("Task spawn failed: {}", e)))??;

        self.cache.insert(key, vector.clone()).await;
        Ok(vector)
    }

    pub async fn embed_batch(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, AppError> {
        let missing: Vec<String> = texts
            .iter()
            .filter(|text| !self.cache.contains_key(&self.hash_text(text)))
            .cloned()
            .collect();

        if !missing.is_empty() {
            self.ensure_initialized().await?;
            self.last_used_secs.store(SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(), Ordering::Relaxed);

            let to_compute = missing.clone();

            let mut computed = Vec::new();
            
            // Parallel batch chunking to keep thread queues sane
            for chunk in to_compute.chunks(32) {
                let chunk_owned = chunk.to_vec();
                let engine_ref = self.engine.clone();
                let chunk_result = tokio::task::spawn_blocking(move || {
                    let mut guard = engine_ref.lock().map_err(|_| {
                        AppError::Embedding("Embedding engine lock poisoned".to_string())
                    })?;
                    let engine = guard.as_mut().ok_or_else(|| {
                        AppError::Embedding("Embedding engine not initialized".to_string())
                    })?;
                    engine
                        .embed(chunk_owned, None)
                        .map_err(|e| AppError::Embedding(format!("fastembed batch embed failed: {e}")))
                })
                .await
                .map_err(|e| AppError::Embedding(format!("Task spawn failed: {}", e)))??;
                computed.extend(chunk_result);
            }

            for (text, vector) in missing.into_iter().zip(computed.into_iter()) {
                let key = self.hash_text(&text);
                self.cache.insert(key, vector).await;
            }
        }

        let mut out = Vec::with_capacity(texts.len());
        for text in texts {
            let key = self.hash_text(&text);
            if let Some(vector) = self.cache.get(&key).await {
                out.push(vector);
            } else {
                out.push(self.embed_text(&text).await?);
            }
        }
        Ok(out)
    }

    pub async fn embed_and_store(
        &self,
        entity_type: &str,
        entity_id: &str,
        user_id: &str,
        namespace: &str,
        text: &str,
    ) -> Result<String, AppError> {
        let vector = self.embed_text(text).await?;
        self.embedding_repo
            .upsert_embedding(
                entity_type,
                entity_id,
                user_id,
                namespace,
                vector,
                &self.model_name,
            )
            .await
    }

    fn hash_text(&self, text: &str) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        text.hash(&mut hasher);
        hasher.finish()
    }
}
