use std::sync::Arc;

use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};

use crate::db::models::SystemProfile;
use crate::log_error;
use crate::log_info;
use crate::log_warn;
use crate::repositories::model_repo::ModelRepo;
use crate::services::embedding_service::EmbeddingService;
use crate::services::inference_service::InferenceService;
use crate::services::hardware_service::{HardwareService, PerformanceMode};
use crate::services::reranker_service::RerankerService;

#[derive(Clone)]
pub enum ModelTier {
    Light,    // Smallest model for fast responses
    Balanced, // Default for daily use
    High,     // Best quality, slower
}

pub struct ModelManagerService {
    inference: Arc<InferenceService>,
    embedding: Arc<EmbeddingService>,
    reranker: Arc<RerankerService>,
    model_repo: Arc<ModelRepo>,
    hardware_service: Arc<HardwareService>,
    current_llm_tier: Arc<RwLock<ModelTier>>,
    is_loading: Arc<RwLock<bool>>,
}

impl ModelManagerService {
    pub fn new(
        inference: Arc<InferenceService>,
        embedding: Arc<EmbeddingService>,
        reranker: Arc<RerankerService>,
        model_repo: Arc<ModelRepo>,
        hardware_service: Arc<HardwareService>,
    ) -> Self {
        log_info!("sarah.model_manager", "Initializing ModelManagerService");

        Self {
            inference,
            embedding,
            reranker,
            model_repo,
            hardware_service,
            current_llm_tier: Arc::new(RwLock::new(ModelTier::Light)),
            is_loading: Arc::new(RwLock::new(false)),
        }
    }

    pub async fn initialize(&self, profile: &SystemProfile) {
        log_info!(
            "sarah.model_manager",
            "Starting background model initialization"
        );

        let mode = self.hardware_service.get_performance_mode(None).await;

        tokio::spawn({
            let inference = self.inference.clone();
            let embedding = self.embedding.clone();
            let reranker = self.reranker.clone();
            let model_repo = self.model_repo.clone();
            let profile = profile.clone();
            let mode = mode;

            async move {
                sleep(Duration::from_secs(5)).await;

                if let Err(e) = Self::download_and_load_light_models(
                    inference,
                    embedding,
                    reranker,
                    model_repo,
                    &profile,
                    mode,
                )
                .await
                {
                    log_error!(
                        "sarah.model_manager",
                        "Failed to initialize light models: {}",
                        e
                    );
                }
            }
        });
    }

    async fn download_and_load_light_models(
        inference: Arc<InferenceService>,
        embedding: Arc<EmbeddingService>,
        reranker: Arc<RerankerService>,
        model_repo: Arc<ModelRepo>,
        profile: &SystemProfile,
        mode: PerformanceMode,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        log_info!(
            "sarah.model_manager",
            "Checking for installed light models..."
        );

        let installed = model_repo.list_installed().await?;

        let llm_model = installed
            .iter()
            .find(|m| {
                m.is_downloaded == 1
                    && m.family.to_lowercase().contains("llama")
                    && m.parameter_count
                        .as_ref()
                        .map(|p| p.contains("1B"))
                        .unwrap_or(false)
            })
            .or_else(|| {
                installed
                    .iter()
                    .find(|m| m.is_downloaded == 1 && m.family.to_lowercase().contains("llama"))
            });

        if let Some(model) = llm_model {
            if let Some(path) = &model.file_path {
                log_info!("sarah.model_manager", "Found installed LLM: {}", model.name);
                if let Err(e) = inference.load_model(path, profile, mode.clone()).await {
                    log_warn!("sarah.model_manager", "Failed to load LLM: {}", e);
                } else {
                    log_info!("sarah.model_manager", "LLM loaded successfully");
                }
            }
        } else {
            log_warn!(
                "sarah.model_manager",
                "No LLM installed. User needs to download."
            );
        }

        if mode != PerformanceMode::Multitasking {
            if let Err(e) = embedding.ensure_initialized().await {
                log_warn!(
                    "sarah.model_manager",
                    "Failed to warm embedding service: {}",
                    e
                );
            }

            if let Err(e) = reranker.ensure_initialized().await {
                log_warn!(
                    "sarah.model_manager",
                    "Failed to warm reranker service: {}",
                    e
                );
            }
        } else {
            log_info!("sarah.model_manager", "Multitasking mode: skipping embedding/reranker warmup to preserve RAM.");
        }

        log_info!(
            "sarah.model_manager",
            "Model init complete - embeddings on-demand"
        );
        Ok(())
    }

    pub async fn upgrade_to_balanced(
        &self,
        profile: &SystemProfile,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut is_loading = self.is_loading.write().await;
        if *is_loading {
            log_warn!("sarah.model_manager", "Model upgrade already in progress");
            return Ok(());
        }
        *is_loading = true;
        drop(is_loading);

        log_info!("sarah.model_manager", "Upgrading to balanced tier models");

        let installed = self.model_repo.list_installed().await?;

        if let Some(model) = installed.iter().find(|m| {
            m.is_downloaded == 1
                && m.parameter_count
                    .as_ref()
                    .map(|p| p.contains("3B"))
                    .unwrap_or(false)
        }) {
            if let Some(path) = &model.file_path {
                log_info!(
                    "sarah.model_manager",
                    "Loading balanced LLM: {}",
                    model.name
                );
                
                let mode = self.hardware_service.get_performance_mode(None).await;
                self.inference.load_model(path, profile, mode).await?;
                
                *self.current_llm_tier.write().await = ModelTier::Balanced;
                log_info!("sarah.model_manager", "Upgraded to balanced tier");
            }
        }

        *self.is_loading.write().await = false;
        Ok(())
    }

    pub async fn downgrade_to_light(&self) {
        log_info!("sarah.model_manager", "Downgrading to light tier");
        *self.current_llm_tier.write().await = ModelTier::Light;
    }

    pub async fn get_current_tier(&self) -> ModelTier {
        self.current_llm_tier.read().await.clone()
    }

    pub async fn is_model_loaded(&self) -> bool {
        self.inference.is_loaded().await
    }
}
