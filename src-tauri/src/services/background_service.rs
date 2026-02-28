use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tauri::Emitter;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::error::AppError;
use crate::repositories::conversation_repo::ConversationRepo;
use crate::repositories::system_repo::SystemRepo;
use crate::services::analytics_service::AnalyticsService;
use crate::services::conversation_service::ConversationService;
use crate::services::hardware_service::HardwareService;
use crate::services::mcp_service::McpService;
use crate::services::memory_service::MemoryService;
use crate::services::rag_service::RagService;
use crate::services::recommendation_service::RecommendationService;

#[derive(Debug, Clone)]
pub enum BackgroundTask {
    EmbedDocument(String),
    SummarizeSession(String),
    RefreshRecommendations,
}

#[derive(Clone)]
pub struct BackgroundService {
    app_handle: tauri::AppHandle,
    mcp_service: McpService,
    memory_service: MemoryService,
    rag_service: Option<Arc<RagService>>,
    recommendation_service: RecommendationService,
    analytics_service: AnalyticsService,
    conversation_service: ConversationService,
    hardware_service: HardwareService,
    conversation_repo: ConversationRepo,
    system_repo: SystemRepo,
    tasks: Arc<Mutex<HashMap<String, JoinHandle<()>>>>,
    queue_tx: flume::Sender<BackgroundTask>,
    queue_rx: flume::Receiver<BackgroundTask>,
    enabled: bool,
    cancel_token: CancellationToken,
}

impl BackgroundService {
    pub fn new(
        app_handle: tauri::AppHandle,
        mcp_service: McpService,
        memory_service: MemoryService,
        rag_service: Option<Arc<RagService>>,
        recommendation_service: RecommendationService,
        analytics_service: AnalyticsService,
        conversation_service: ConversationService,
        hardware_service: HardwareService,
        conversation_repo: ConversationRepo,
        system_repo: SystemRepo,
        enabled: bool,
    ) -> Self {
        let (queue_tx, queue_rx) = flume::bounded(256);

        Self {
            app_handle,
            mcp_service,
            memory_service,
            rag_service,
            recommendation_service,
            analytics_service,
            conversation_service,
            hardware_service,
            conversation_repo,
            system_repo,
            tasks: Arc::new(Mutex::new(HashMap::new())),
            queue_tx,
            queue_rx,
            enabled,
            cancel_token: CancellationToken::new(),
        }
    }

    pub fn sender(&self) -> flume::Sender<BackgroundTask> {
        self.queue_tx.clone()
    }

    pub async fn start_critical_tasks(&self) -> Result<(), AppError> {
        self.start_mcp_health_check_job().await;

        if self.enabled {
            self.start_worker().await;

            tokio::spawn({
                let service = self.clone();
                async move {
                    tokio::time::sleep(Duration::from_secs(10)).await;
                    service.start_secondary_tasks().await;
                }
            });

            tokio::spawn({
                let service = self.clone();
                async move {
                    tokio::time::sleep(Duration::from_secs(60)).await;
                    service.start_background_tasks().await;
                }
            });
        }

        Ok(())
    }

    async fn start_secondary_tasks(&self) {
        self.start_session_summary_job().await;
        self.start_memory_decay_job().await;
    }

    async fn start_background_tasks(&self) {
        self.start_model_refresh_job().await;
        self.start_analytics_aggregation_job().await;
    }

    async fn start_worker(&self) {
        let rx = self.queue_rx.clone();
        let rag = self.rag_service.clone();
        let conv = self.conversation_service.clone();
        let rec = self.recommendation_service.clone();
        let system_repo = self.system_repo.clone();
        let hardware = self.hardware_service.clone();
        let token = self.cancel_token.clone();

        let handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = token.cancelled() => {
                        tracing::info!("Background worker shutting down gracefully");
                        break;
                    }
                    result = rx.recv_async() => {
                        match result {
                            Ok(task) => match task {
                                BackgroundTask::EmbedDocument(doc_id) => {
                                    if let Some(rag_svc) = rag.as_ref() {
                                        let _ = rag_svc.embed_document_chunks(&doc_id).await;
                                    }
                                }
                                BackgroundTask::SummarizeSession(session_id) => {
                                    if is_pressure_high(&hardware) {
                                        continue;
                                    }
                                    let _ = conv.summarize_session(&session_id).await;
                                }
                                BackgroundTask::RefreshRecommendations => {
                                    if is_pressure_high(&hardware) {
                                        continue;
                                    }
                                    if let Ok(Some(profile)) = system_repo.get_current_profile().await {
                                        let mode = hardware.get_performance_mode(None).await;
                                        let _ = rec.recompute(&profile, mode).await;
                                    }
                                }
                            },
                            Err(_) => break, // Channel closed
                        }
                    }
                }
            }
        });

        self.tasks.lock().await.insert("worker".to_string(), handle);
    }

    async fn start_memory_decay_job(&self) {
        let memory_service = self.memory_service.clone();
        let token = self.cancel_token.clone();

        let handle = tokio::spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_secs(60 * 60 * 24));
            loop {
                tokio::select! {
                    _ = token.cancelled() => {
                        tracing::info!("Memory decay job shutting down");
                        break;
                    }
                    _ = ticker.tick() => {
                        let _ = memory_service.apply_decay_job("default").await;
                    }
                }
            }
        });

        self.tasks
            .lock()
            .await
            .insert("memory_decay".to_string(), handle);
    }

    async fn start_mcp_health_check_job(&self) {
        let mcp_service = self.mcp_service.clone();
        let app_handle = self.app_handle.clone();
        let token = self.cancel_token.clone();

        let handle = tokio::spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_secs(120));
            loop {
                tokio::select! {
                    _ = token.cancelled() => {
                        tracing::info!("MCP health check job shutting down");
                        break;
                    }
                    _ = ticker.tick() => {
                        if let Ok(statuses) = mcp_service.health_check_all().await {
                            let _ = app_handle.emit("mcp:health_changed", statuses);
                        }
                        let _ = mcp_service
                            .cleanup_idle_connections(Duration::from_secs(300))
                            .await;
                    }
                }
            }
        });

        self.tasks
            .lock()
            .await
            .insert("mcp_health".to_string(), handle);
    }

    async fn start_model_refresh_job(&self) {
        let tx = self.queue_tx.clone();
        let token = self.cancel_token.clone();

        let handle = tokio::spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_secs(60 * 60 * 24 * 7));
            loop {
                tokio::select! {
                    _ = token.cancelled() => {
                        tracing::info!("Model refresh job shutting down");
                        break;
                    }
                    _ = ticker.tick() => {
                        let _ = tx.send(BackgroundTask::RefreshRecommendations);
                    }
                }
            }
        });

        self.tasks
            .lock()
            .await
            .insert("model_refresh".to_string(), handle);
    }

    async fn start_session_summary_job(&self) {
        let repo = self.conversation_repo.clone();
        let tx = self.queue_tx.clone();
        let token = self.cancel_token.clone();

        let handle = tokio::spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_secs(60 * 15));
            loop {
                tokio::select! {
                    _ = token.cancelled() => {
                        tracing::info!("Session summary job shutting down");
                        break;
                    }
                    _ = ticker.tick() => {
                        if let Ok(sessions) = repo.list_sessions("default", 200, None).await {
                            for session in sessions {
                                if session.message_count >= 20 {
                                    let _ = tx.send(BackgroundTask::SummarizeSession(session.id));
                                }
                            }
                        }
                    }
                }
            }
        });

        self.tasks
            .lock()
            .await
            .insert("session_summary".to_string(), handle);
    }

    async fn start_analytics_aggregation_job(&self) {
        let analytics = self.analytics_service.clone();
        let token = self.cancel_token.clone();

        let handle = tokio::spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_secs(60 * 60 * 24));
            loop {
                tokio::select! {
                    _ = token.cancelled() => {
                        tracing::info!("Analytics aggregation job shutting down");
                        break;
                    }
                    _ = ticker.tick() => {
                        let _ = analytics.aggregate_daily().await;
                    }
                }
            }
        });

        self.tasks
            .lock()
            .await
            .insert("analytics_agg".to_string(), handle);
    }

    /// Gracefully shut down all background tasks.
    /// Cancels the shared token and waits up to 5 seconds for tasks to finish.
    pub async fn stop_all(&self) {
        tracing::info!("Stopping all background tasks...");
        self.cancel_token.cancel();

        let mut tasks = self.tasks.lock().await;
        for (name, handle) in tasks.drain() {
            match tokio::time::timeout(Duration::from_secs(5), handle).await {
                Ok(Ok(())) => tracing::info!("Task '{}' stopped cleanly", name),
                Ok(Err(e)) => tracing::warn!("Task '{}' panicked: {}", name, e),
                Err(_) => {
                    tracing::warn!("Task '{}' did not stop within 5s, aborting", name);
                }
            }
        }
        tracing::info!("All background tasks stopped");
    }
}

fn is_pressure_high(hardware: &HardwareService) -> bool {
    let stats = hardware.live_stats();
    if stats.memory_total_mb == 0 {
        return stats.cpu_usage_pct >= 88.0;
    }
    let memory_pct = (stats.memory_used_mb as f64 / stats.memory_total_mb as f64) * 100.0;
    stats.cpu_usage_pct >= 88.0 || memory_pct >= 88.0
}
