use std::num::NonZeroU32;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use encoding_rs::UTF_8;
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{AddBos, LlamaModel};
use llama_cpp_2::sampling::LlamaSampler;
use tauri::Emitter;
use tokio::sync::{mpsc, Semaphore};
use tokio_stream::wrappers::ReceiverStream;

use crate::db::models::{
    GenerationOptions, GenerationResult, Message, MessageStreamChunk, SystemProfile,
};
use crate::error::AppError;
use crate::services::hardware_service::PerformanceMode;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub path: String,
    pub context_length: usize,
    pub n_gpu_layers: i32,
    pub n_threads: usize,
}

struct LoadedModel {
    backend: LlamaBackend,
    model: LlamaModel,
    info: ModelInfo,
    seed: u32,
    last_used_secs: Arc<AtomicU64>,
}

#[derive(Clone)]
pub struct InferenceService {
    loaded: Arc<Mutex<Option<LoadedModel>>>,
    limiter: Arc<Semaphore>,
}

impl InferenceService {
    pub fn new() -> Self {
        Self {
            loaded: Arc::new(Mutex::new(None)),
            limiter: Arc::new(Semaphore::new(1)),
        }
    }

    pub async fn is_loaded(&self) -> bool {
        self.loaded.lock().map(|g| g.is_some()).unwrap_or(false)
    }

    pub async fn load_model(
        &self,
        model_path: &str,
        hardware_profile: &SystemProfile,
        mode: PerformanceMode,
    ) -> Result<(), AppError> {
        if !Path::new(model_path).exists() {
            return Err(AppError::Inference(format!(
                "Model file does not exist: {model_path}"
            )));
        }

        let mut n_threads = hardware_profile.cpu_threads.max(1) as usize;
        if mode == PerformanceMode::Multitasking {
            // Brutally strict: max 25% of threads, minimum 1, max 4
            n_threads = (n_threads / 4).clamp(1, 4);
            crate::log_info!("sarah.inference", "Multitasking mode active. Restricted inference to {} threads.", n_threads);
        }

        // Aggressive GPU offloading: Llama 1B takes ~1GB VRAM. 
        // If the user has at least 1024MB of VRAM, offload ALL layers to the GPU.
        let n_gpu_layers: i32 = if hardware_profile.gpu_vram_mb.unwrap_or(0) >= 1024 {
            -1 // -1 tells llama.cpp to offload all layers
        } else {
            0
        };

        let model_path_owned = model_path.to_string();

        let loaded = tokio::task::spawn_blocking(move || -> Result<LoadedModel, AppError> {
            let backend = LlamaBackend::init()
                .map_err(|e| AppError::Inference(format!("Failed to init llama backend: {e}")))?;

            let mut model_params = LlamaModelParams::default();
            if n_gpu_layers > 0 {
                model_params = model_params.with_n_gpu_layers(n_gpu_layers as u32);
            } else if n_gpu_layers < 0 {
                model_params = model_params.with_n_gpu_layers(1000);
            }

            let model = LlamaModel::load_from_file(&backend, &model_path_owned, &model_params)
                .map_err(|e| AppError::Inference(format!("Failed to load GGUF model: {e}")))?;

            let context_length = model.n_ctx_train() as usize;
            Ok(LoadedModel {
                backend,
                model,
                info: ModelInfo {
                    path: model_path_owned,
                    context_length,
                    n_gpu_layers,
                    n_threads,
                },
                seed: 1234,
                last_used_secs: Arc::new(AtomicU64::new(SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs())),
            })
        })
        .await
        .map_err(|e| AppError::Inference(e.to_string()))??;

        let mut guard = self
            .loaded
            .lock()
            .map_err(|_| AppError::Inference("Model lock poisoned".to_string()))?;
        *guard = Some(loaded);

        if mode == PerformanceMode::Multitasking {
            self.start_auto_unloader();
        }

        Ok(())
    }

    fn start_auto_unloader(&self) {
        let loaded_ref = self.loaded.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
                let mut guard = if let Ok(g) = loaded_ref.lock() { g } else { return; };
                
                if let Some(loaded) = guard.as_ref() {
                    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
                    let last_used = loaded.last_used_secs.load(Ordering::Relaxed);
                    // 5 minutes (300 seconds) idle timeout
                    if now.saturating_sub(last_used) > 300 {
                        crate::log_info!("sarah.inference", "Model idle for 5+ minutes in Multitasking mode. Auto-unloading from memory.");
                        *guard = None; // Drops LlamaModel, freeing RAM/VRAM
                        break;
                    }
                } else {
                    break;
                }
            }
        });
    }

    pub async fn generate_stream(
        &self,
        session_id: &str,
        messages: Vec<Message>,
        opts: GenerationOptions,
        app_handle: Option<tauri::AppHandle>,
    ) -> Result<ReceiverStream<MessageStreamChunk>, AppError> {
        {
            let guard = self
                .loaded
                .lock()
                .map_err(|_| AppError::Inference("Model lock poisoned".to_string()))?;
            if let Some(loaded) = guard.as_ref() {
                loaded.last_used_secs.store(SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(), Ordering::Relaxed);
            } else {
                return Err(AppError::Inference(
                    "No active model loaded. Register a local GGUF model first.".to_string(),
                ));
            }
        }

        let permit = self
            .limiter
            .clone()
            .acquire_owned()
            .await
            .map_err(|e| AppError::Inference(e.to_string()))?;

        let prompt = Self::build_prompt(&messages);
        let session_id_owned = session_id.to_string();
        let loaded = self.loaded.clone();

        let (tx, rx) = mpsc::channel::<MessageStreamChunk>(256);

        tokio::task::spawn_blocking(move || {
            let _permit_guard = permit;

            let generation = (|| -> Result<GenerationResult, AppError> {
                let mut guard = loaded
                    .lock()
                    .map_err(|_| AppError::Inference("Model lock poisoned".to_string()))?;
                let loaded = guard.as_mut().ok_or_else(|| {
                    AppError::Inference("No model loaded for generation".to_string())
                })?;

                Self::generate_with_llama(loaded, &prompt, &opts, |piece| {
                    if let Some(app) = app_handle.as_ref() {
                        let _ = app.emit(
                            "inference:token",
                            MessageStreamChunk {
                                session_id: session_id_owned.clone(),
                                token: piece.to_string(),
                                done: false,
                            },
                        );
                    }

                    tx.blocking_send(MessageStreamChunk {
                        session_id: session_id_owned.clone(),
                        token: piece.to_string(),
                        done: false,
                    })
                    .map_err(|e| AppError::Inference(e.to_string()))?;

                    Ok(())
                })
            })();

            if let Err(error) = generation {
                let _ = tx.blocking_send(MessageStreamChunk {
                    session_id: session_id_owned.clone(),
                    token: format!("[inference error] {error}"),
                    done: false,
                });
            }

            let _ = tx.blocking_send(MessageStreamChunk {
                session_id: session_id_owned,
                token: String::new(),
                done: true,
            });
        });

        Ok(ReceiverStream::new(rx))
    }

    pub async fn generate_with_tools(
        &self,
        messages: Vec<Message>,
        tool_schemas: &[String],
    ) -> Result<GenerationResult, AppError> {
        let _permit = self
            .limiter
            .clone()
            .acquire_owned()
            .await
            .map_err(|e| AppError::Inference(e.to_string()))?;

        let prompt = if tool_schemas.is_empty() {
            Self::build_prompt(&messages)
        } else {
            format!(
                "{}\n\nAvailable tools:\n{}",
                Self::build_prompt(&messages),
                tool_schemas.join("\n")
            )
        };

        let loaded = self.loaded.clone();
        let opts = GenerationOptions::default();

        tokio::task::spawn_blocking(move || {
            let mut guard = loaded
                .lock()
                .map_err(|_| AppError::Inference("Model lock poisoned".to_string()))?;
            let loaded = guard
                .as_mut()
                .ok_or_else(|| AppError::Inference("No active model loaded".to_string()))?;

            Self::generate_with_llama(loaded, &prompt, &opts, |_| Ok(()))
        })
        .await
        .map_err(|e| AppError::Inference(e.to_string()))?
    }

    pub async fn generate_embedding(&self, text: &str) -> Result<Vec<f32>, AppError> {
        let mut vec = vec![0.0f32; 384];
        for (idx, byte) in text.as_bytes().iter().enumerate() {
            vec[idx % 384] += (*byte as f32 / 255.0) * (((idx % 17) + 1) as f32 / 17.0);
        }
        let norm = vec.iter().map(|v| v * v).sum::<f32>().sqrt().max(1e-6);
        for v in &mut vec {
            *v /= norm;
        }
        Ok(vec)
    }

    pub async fn get_active_model_info(&self) -> Option<ModelInfo> {
        self.loaded
            .lock()
            .ok()
            .and_then(|guard| guard.as_ref().map(|loaded| loaded.info.clone()))
    }

    pub async fn unload_model(&self) -> Result<(), AppError> {
        let mut guard = self
            .loaded
            .lock()
            .map_err(|_| AppError::Inference("Model lock poisoned".to_string()))?;
        *guard = None;
        Ok(())
    }

    /// Graceful shutdown: unload any loaded model and release resources.
    pub async fn shutdown(&self) {
        tracing::info!("InferenceService shutting down, unloading model...");
        match self.unload_model().await {
            Ok(()) => tracing::info!("Model unloaded successfully"),
            Err(e) => tracing::warn!("Failed to unload model during shutdown: {}", e),
        }
    }

    fn build_prompt(messages: &[Message]) -> String {
        let mut prompt = String::new();
        prompt.push_str("<|begin_of_text|>");

        for message in messages {
            let role = match message.role.as_str() {
                "user" => "user",
                "assistant" => "assistant",
                "system" => "system",
                _ => "user",
            };

            prompt.push_str(&format!(
                "<|start_header_id|>{role}<|end_header_id|>\n\n{}<|eot_id|>",
                message.content.trim()
            ));
        }

        // Final header to trigger assistant response
        prompt.push_str("<|start_header_id|>assistant<|end_header_id|>\n\n");
        prompt
    }

    fn generate_with_llama(
        loaded: &mut LoadedModel,
        prompt: &str,
        opts: &GenerationOptions,
        mut on_token: impl FnMut(&str) -> Result<(), AppError>,
    ) -> Result<GenerationResult, AppError> {
        let prompt_tokens = loaded
            .model
            .str_to_token(prompt, AddBos::Always)
            .map_err(|e| AppError::Inference(format!("Tokenization failed: {e}")))?;

        if prompt_tokens.is_empty() {
            return Err(AppError::Inference(
                "Prompt tokenization was empty".to_string(),
            ));
        }

        // Calculate exact required context width instead of mindlessly allocating the model's max train context
        // Llama 3.2 defaults to 131,072 which would instantly consume 4.1GB of RAM for the blank KV Cache!
        let required_ctx = prompt_tokens.len() + opts.max_tokens;
        // Clamp dynamically to at least 1024, at most 8192 to heavily protect system RAM from overflowing 
        let safe_ctx_len = (required_ctx as u32).max(1024).min(8192).min(loaded.info.context_length as u32);

        let n_ctx = NonZeroU32::new(safe_ctx_len)
            .ok_or_else(|| AppError::Inference("Invalid context window size computed".to_string()))?;

        // Enforce the hardware-profile driven CPU thread limits (e.g., 20-30% in multitasking)
        // Without this, llama.cpp ignores the model struct and defaults to spawning threads for all cores!
        let safe_threads = loaded.info.n_threads.max(1) as i32;

        let ctx_params = LlamaContextParams::default()
            .with_n_ctx(Some(n_ctx))
            .with_n_threads(safe_threads)
            .with_n_threads_batch(safe_threads);
        
        let mut ctx = loaded
            .model
            .new_context(&loaded.backend, ctx_params)
            .map_err(|e| AppError::Inference(format!("Failed to create llama context: {e}")))?;

        let n_len = prompt_tokens.len() + opts.max_tokens;
        let n_ctx_i32 = ctx.n_ctx() as usize;
        if n_len > n_ctx_i32 {
            return Err(AppError::Inference(format!(
                "Context overflow: prompt + max_tokens is {} but we clamped context to {} to prevent RAM exhaustion. Please send a shorter message.",
                n_len, n_ctx_i32
            )));
        }

        // Increase batch size to avoid "Insufficient Space" errors on long prompts
        let batch_size = (prompt_tokens.len() + 128).max(1024).min(4096);
        let mut batch = LlamaBatch::new(batch_size, 1);
        let last_index = (prompt_tokens.len() - 1) as i32;
        for (idx, token) in (0_i32..).zip(prompt_tokens.into_iter()) {
            let is_last = idx == last_index;
            batch
                .add(token, idx, &[0], is_last)
                .map_err(|e| AppError::Inference(format!("Batch add failed: {e}")))?;
        }

        ctx.decode(&mut batch)
            .map_err(|e| AppError::Inference(format!("Initial decode failed: {e}")))?;

        let mut sampler = if opts.temperature <= 0.0 {
            LlamaSampler::chain_simple([LlamaSampler::greedy()])
        } else {
            LlamaSampler::chain_simple([
                LlamaSampler::temp(opts.temperature),
                LlamaSampler::dist(loaded.seed),
                LlamaSampler::greedy(),
            ])
        };

        let mut generated = String::new();
        let mut decoder = UTF_8.new_decoder();
        let mut n_cur = batch.n_tokens();
        let mut n_decode = 0usize;

        while n_decode < opts.max_tokens {
            let token = sampler.sample(&ctx, batch.n_tokens() - 1);
            sampler.accept(token);

            if loaded.model.is_eog_token(token) {
                break;
            }

            let piece = loaded
                .model
                .token_to_piece(token, &mut decoder, true, None)
                .map_err(|e| AppError::Inference(format!("Token decode failed: {e}")))?;

            on_token(&piece)?;
            generated.push_str(&piece);

            batch.clear();
            batch
                .add(token, n_cur, &[0], true)
                .map_err(|e| AppError::Inference(format!("Batch add failed: {e}")))?;

            n_cur += 1;
            n_decode += 1;

            ctx.decode(&mut batch)
                .map_err(|e| AppError::Inference(format!("Decode failed: {e}")))?;
        }

        Ok(GenerationResult {
            text: generated,
            tokens_generated: n_decode,
            finish_reason: if n_decode >= opts.max_tokens {
                "length".to_string()
            } else {
                "stop".to_string()
            },
        })
    }
}
