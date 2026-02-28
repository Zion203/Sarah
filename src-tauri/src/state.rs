use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use moka::future::Cache;
use tokio::sync::RwLock;

use tauri::Manager;

use crate::db::models::{Memory, Model, Session, SystemProfile};
use crate::db::Database;
use crate::error::AppError;
use crate::log_info;
use crate::repositories::analytics_repo::AnalyticsRepo;
use crate::repositories::conversation_repo::ConversationRepo;
use crate::repositories::document_repo::DocumentRepo;
use crate::repositories::embedding_repo::EmbeddingRepo;
use crate::repositories::mcp_repo::McpRepo;
use crate::repositories::memory_repo::MemoryRepo;
use crate::repositories::model_repo::ModelRepo;
use crate::repositories::settings_repo::{Setting, SettingsRepo};
use crate::repositories::system_repo::SystemRepo;
use crate::repositories::user_repo::UserRepo;
use crate::services::adaptive_memory_manager::AdaptiveMemoryManager;
use crate::services::analytics_service::AnalyticsService;
use crate::services::background_service::BackgroundService;
use crate::services::context_service::ContextService;
use crate::services::conversation_service::ConversationService;
use crate::services::crypto_service::CryptoService;
use crate::services::embedding_service::EmbeddingService;
use crate::services::hardware_service::{DeviceTier, HardwareService, TierConfig};
use crate::services::inference_service::InferenceService;
use crate::services::intent_service::IntentService;
use crate::services::mcp_service::McpService;
use crate::services::memory_service::MemoryService;
use crate::services::model_manager_service::ModelManagerService;
use crate::services::predictive_preloader::PredictivePreloader;
use crate::services::rag_service::RagService;
use crate::services::recommendation_service::RecommendationService;
use crate::services::reranker_service::RerankerService;
use crate::services::runtime_governor_service::RuntimeGovernorService;
use crate::services::runtime_orchestrator_service::{FeatureGate, RuntimeOrchestratorService};
use crate::services::setup_orchestrator_service::SetupOrchestratorService;
use crate::services::smart_query_classifier::SmartQueryClassifier;
use crate::services::task_router_service::TaskRouterService;
use crate::services::usage_learner::UsageLearner;

#[derive(Clone)]
pub struct AppCache {
    pub hardware_profile: Cache<String, SystemProfile>,
    pub model_list: Cache<String, Vec<Model>>,
    pub user_settings: Cache<String, Vec<Setting>>,
    pub session_metadata: Cache<String, Session>,
    pub recent_memories: Cache<String, Vec<Memory>>,
    pub text_embeddings: Cache<u64, Vec<f32>>,
    pub mcp_tool_schemas: Cache<String, Vec<String>>,
}

impl AppCache {
    pub fn new(config: &TierConfig) -> Self {
        Self {
            hardware_profile: Cache::builder()
                .time_to_live(std::time::Duration::from_secs(300))
                .max_capacity(8)
                .build(),
            model_list: Cache::builder()
                .time_to_live(std::time::Duration::from_secs(60))
                .max_capacity(32)
                .build(),
            user_settings: Cache::builder()
                .time_to_live(std::time::Duration::from_secs(600))
                .max_capacity(config.settings_cache_capacity)
                .build(),
            session_metadata: Cache::builder()
                .time_to_live(std::time::Duration::from_secs(1800))
                .max_capacity(config.session_cache_capacity)
                .build(),
            recent_memories: Cache::builder()
                .time_to_live(std::time::Duration::from_secs(300))
                .max_capacity(256)
                .build(),
            text_embeddings: Cache::builder()
                .time_to_live(std::time::Duration::from_secs(60 * 60 * 24))
                .max_capacity(config.embed_cache_capacity)
                .build(),
            mcp_tool_schemas: Cache::builder()
                .time_to_live(std::time::Duration::from_secs(300))
                .max_capacity(256)
                .build(),
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Database>,
    pub cache: Arc<AppCache>,
    pub hardware: Arc<RwLock<Option<SystemProfile>>>,
    pub detected_tier: DeviceTier,
    pub tier: DeviceTier,
    pub tier_config: TierConfig,
    pub startup_started_at_utc: String,
    pub startup_completed_at_utc: String,
    pub startup_init_ms: i64,

    pub system_repo: Arc<SystemRepo>,
    pub model_repo: Arc<ModelRepo>,
    pub user_repo: Arc<UserRepo>,
    pub mcp_repo: Arc<McpRepo>,
    pub conversation_repo: Arc<ConversationRepo>,
    pub memory_repo: Arc<MemoryRepo>,
    pub document_repo: Arc<DocumentRepo>,
    pub embedding_repo: Arc<EmbeddingRepo>,
    pub settings_repo: Arc<SettingsRepo>,
    pub analytics_repo: Arc<AnalyticsRepo>,

    pub hardware_service: Arc<HardwareService>,
    pub inference: Arc<InferenceService>,
    pub embedding: Option<Arc<EmbeddingService>>,
    pub reranker: Option<Arc<RerankerService>>,
    pub model_manager: Option<Arc<ModelManagerService>>,
    pub intent: Arc<IntentService>,
    pub memory: Arc<MemoryService>,
    pub rag: Option<Arc<RagService>>,
    pub mcp: Arc<McpService>,
    pub context: Arc<ContextService>,
    pub conversation: Arc<ConversationService>,
    pub crypto: Arc<CryptoService>,
    pub analytics: Arc<AnalyticsService>,
    pub recommendation: Arc<RecommendationService>,
    pub runtime_governor: Arc<RuntimeGovernorService>,
    pub task_router: Arc<TaskRouterService>,
    pub runtime_orchestrator: Arc<RuntimeOrchestratorService>,
    pub setup_orchestrator: Arc<SetupOrchestratorService>,
    pub background: Arc<BackgroundService>,
}

impl AppState {
    pub async fn initialize(app_handle: &tauri::AppHandle) -> Result<Self, AppError> {
        let startup_clock = Instant::now();
        let startup_started_at_utc = chrono::Utc::now().to_rfc3339();
        let database = Arc::new(Database::new(app_handle, 4).await?);

        let read_pool = database.read_pool().clone();
        let write_pool = database.write_pool().clone();

        let system_repo = Arc::new(SystemRepo::with_pools(
            read_pool.clone(),
            write_pool.clone(),
        ));
        let model_repo = Arc::new(ModelRepo::with_pools(read_pool.clone(), write_pool.clone()));
        let user_repo = Arc::new(UserRepo::with_pools(read_pool.clone(), write_pool.clone()));
        let mcp_repo = Arc::new(McpRepo::with_pools(read_pool.clone(), write_pool.clone()));
        let conversation_repo = Arc::new(ConversationRepo::with_pools(
            read_pool.clone(),
            write_pool.clone(),
        ));
        let memory_repo = Arc::new(MemoryRepo::with_pools(
            read_pool.clone(),
            write_pool.clone(),
        ));
        let document_repo = Arc::new(DocumentRepo::with_pools(
            read_pool.clone(),
            write_pool.clone(),
        ));
        let embedding_repo = Arc::new(EmbeddingRepo::with_pools(
            read_pool.clone(),
            write_pool.clone(),
        ));
        let settings_repo = Arc::new(SettingsRepo::with_pools(
            read_pool.clone(),
            write_pool.clone(),
        ));
        let analytics_repo = Arc::new(AnalyticsRepo::with_pools(
            read_pool.clone(),
            write_pool.clone(),
        ));

        let hardware_service = Arc::new(HardwareService::new((*system_repo).clone(), (*settings_repo).clone()));
        let detected_profile = hardware_service.detect_hardware().await?;

        let detected_tier = detected_profile.classify();
        let startup_tier = match detected_tier {
            DeviceTier::Ultra | DeviceTier::High | DeviceTier::Medium | DeviceTier::Low => DeviceTier::Low,
            DeviceTier::Minimal | DeviceTier::Potato => DeviceTier::Minimal,
        };

        let mut tier_config = hardware_service.get_tier_config(startup_tier, None).await;
        tier_config.background_tasks_enabled = false;

        tracing::info!(
            "Hardware detected: {} cores, {}MB RAM, GPU: {:?}, detected tier {:?}, startup tier {:?}",
            detected_profile.cpu_threads,
            detected_profile.total_ram_mb,
            detected_profile.gpu_name,
            detected_tier,
            startup_tier
        );

        let cache = Arc::new(AppCache::new(&tier_config));

        cache
            .hardware_profile
            .insert("current".to_string(), detected_profile.clone())
            .await;

        let hardware = Arc::new(RwLock::new(Some(detected_profile.clone())));

        let bundle_id = app_handle.config().identifier.clone();
        let crypto = Arc::new(CryptoService::new(&bundle_id)?);

        let cache_dir = app_handle
            .path()
            .app_cache_dir()
            .map_err(|e| AppError::Config(format!("Failed to resolve cache dir: {e}")))?;
        tokio::fs::create_dir_all(&cache_dir).await?;

        let embedding: Option<Arc<EmbeddingService>> = if let Some(ref model_name) =
            tier_config.embedding_model
        {
            match EmbeddingService::new(
                model_name,
                cache_dir.join("embeddings"),
                (*embedding_repo).clone(),
                hardware_service.clone(),
            ) {
                Ok(service) => {
                    tracing::info!(
                        "Embedding service created (lazy init: {})",
                        service.is_initialized()
                    );
                    Some(Arc::new(service))
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to create embedding service: {e}. Continuing without embeddings."
                    );
                    None
                }
            }
        } else {
            tracing::info!("Embedding service disabled for minimal tier");
            None
        };

        let reranker: Option<Arc<RerankerService>> =
            if let Some(ref model_name) = tier_config.reranker_model {
                match RerankerService::new(model_name, cache_dir.join("reranker"), hardware_service.clone()) {
                    Ok(service) => {
                        tracing::info!(
                            "Reranker service created (lazy init: {})",
                            service.is_initialized()
                        );
                        Some(Arc::new(service))
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Failed to create reranker service: {e}. Continuing without reranking."
                        );
                        None
                    }
                }
            } else {
                tracing::info!("Reranker disabled for low/minimal tier");
                None
            };

        let intent = Arc::new(IntentService::new());
        let inference = Arc::new(InferenceService::new());

        let embedding_for_memory = embedding.clone();
        let memory = Arc::new(MemoryService::new(
            (*memory_repo).clone(),
            (*embedding_repo).clone(),
            embedding_for_memory,
            (*inference).clone(),
        ));

        let rag: Option<Arc<RagService>> =
            if let (Some(ref emb), Some(ref rer)) = (embedding.as_ref(), reranker.as_ref()) {
                Some(Arc::new(RagService::new(
                    (*document_repo).clone(),
                    (*embedding_repo).clone(),
                    Arc::clone(emb),
                    Arc::clone(rer),
                    write_pool.clone(),
                )))
            } else {
                tracing::info!("RAG service disabled (requires embedding + reranker)");
                None
            };

        let mcp = Arc::new(McpService::new(
            (*mcp_repo).clone(),
            (*crypto).clone(),
            (*intent).clone(),
        ));

        let context = Arc::new(ContextService::new(
            (*memory).clone(),
            rag.clone(),
            (*intent).clone(),
            (*mcp).clone(),
            (*conversation_repo).clone(),
            (*model_repo).clone(),
        ));

        let analytics = Arc::new(AnalyticsService::new((*analytics_repo).clone()));
        let recommendation = Arc::new(RecommendationService::new(
            (*model_repo).clone(),
            (*analytics_repo).clone(),
        ));
        let runtime_governor = Arc::new(RuntimeGovernorService::new(
            read_pool.clone(),
            write_pool.clone(),
            (*hardware_service).clone(),
        ));
        let task_router = Arc::new(TaskRouterService::new(
            (*model_repo).clone(),
            (*runtime_governor).clone(),
            write_pool.clone(),
        ));
        let setup_orchestrator = Arc::new(SetupOrchestratorService::new(
            read_pool.clone(),
            write_pool.clone(),
        ));

        let query_classifier = Arc::new(SmartQueryClassifier::new());
        let usage_learner = Arc::new(UsageLearner::new());
        let adaptive_memory = Arc::new(AdaptiveMemoryManager::new(
            Arc::clone(&hardware_service),
            Arc::clone(&inference),
        ));
        let predictive_preloader = Arc::new(PredictivePreloader::new(
            Arc::clone(&inference),
            embedding.clone(),
            Arc::clone(&hardware_service),
        ));
        let runtime_orchestrator = Arc::new(RuntimeOrchestratorService::new(
            (*runtime_governor).clone(),
            Arc::clone(&hardware_service),
            query_classifier,
            usage_learner,
            adaptive_memory,
            predictive_preloader,
            detected_tier,
            startup_tier,
            FeatureGate {
                rag_enabled: rag.is_some(),
                mcp_enabled: true,
                spotify_enabled: true,
                background_tasks_enabled: tier_config.background_tasks_enabled,
                predictive_preload_enabled: !matches!(startup_tier, DeviceTier::Minimal),
                adaptive_memory_enabled: true,
            },
        ));
        runtime_orchestrator.start_background_loops().await;

        let conversation = Arc::new(ConversationService::new(
            (*conversation_repo).clone(),
            (*context).clone(),
            (*inference).clone(),
            (*memory).clone(),
            rag.clone(),
            (*mcp).clone(),
            (*analytics).clone(),
            (*model_repo).clone(),
            (*runtime_governor).clone(),
            (*task_router).clone(),
            Arc::clone(&runtime_orchestrator),
            (*system_repo).clone(),
            Arc::clone(&hardware_service),
        ));

        let background = Arc::new(BackgroundService::new(
            app_handle.clone(),
            (*mcp).clone(),
            (*memory).clone(),
            rag.clone(),
            (*recommendation).clone(),
            (*analytics).clone(),
            (*conversation).clone(),
            (*hardware_service).clone(),
            (*conversation_repo).clone(),
            (*system_repo).clone(),
            tier_config.background_tasks_enabled,
        ));

        background.start_critical_tasks().await?;

        let model_manager =
            if tier_config.auto_load_model && embedding.is_some() && reranker.is_some() {
                log_info!(
                    "sarah.state",
                    "Creating ModelManagerService for automatic model management"
                );
                let mm = Arc::new(ModelManagerService::new(
                    inference.clone(),
                    embedding.clone().unwrap(),
                    reranker.clone().unwrap(),
                    model_repo.clone(),
                    hardware_service.clone(),
                ));

                mm.initialize(&detected_profile).await;
                Some(mm)
            } else {
                log_info!(
                    "sarah.state",
                    "Model auto-loading disabled for {} tier",
                    startup_tier
                );
                None
            };

        let _ = user_repo.get_or_create_default_user().await?;

        let startup_completed_at_utc = chrono::Utc::now().to_rfc3339();
        let startup_init_ms = startup_clock.elapsed().as_millis() as i64;

        Ok(Self {
            db: database,
            cache,
            hardware,
            detected_tier,
            tier: startup_tier,
            tier_config,
            startup_started_at_utc,
            startup_completed_at_utc,
            startup_init_ms,
            system_repo,
            model_repo,
            user_repo,
            mcp_repo,
            conversation_repo,
            memory_repo,
            document_repo,
            embedding_repo,
            settings_repo,
            analytics_repo,
            hardware_service,
            inference,
            embedding,
            reranker,
            model_manager,
            intent,
            memory,
            rag,
            mcp,
            context,
            conversation,
            crypto,
            analytics,
            recommendation,
            runtime_governor,
            task_router,
            runtime_orchestrator,
            setup_orchestrator,
            background,
        })
    }

    pub fn cache_dir(&self) -> Result<PathBuf, AppError> {
        Ok(self
            .db
            .db_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .to_path_buf())
    }
}
