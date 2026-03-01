use std::sync::Arc;

use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;

use crate::db::models::{
    GenerationOptions, Message, MessageStreamChunk, Model, NewMessage, NewToolCall,
    RoutingDecision, SystemProfile, ToolResult,
};
use crate::error::AppError;
use crate::repositories::conversation_repo::ConversationRepo;
use crate::repositories::model_repo::ModelRepo;
use crate::repositories::system_repo::SystemRepo;
use crate::services::analytics_service::AnalyticsService;
use crate::services::context_service::ContextService;
use crate::services::inference_service::InferenceService;
use crate::services::mcp_service::McpService;
use crate::services::memory_service::MemoryService;
use crate::services::rag_service::RagService;
use crate::services::runtime_governor_service::RuntimeGovernorService;
use crate::services::runtime_orchestrator_service::RuntimeOrchestratorService;
use crate::services::task_router_service::TaskRouterService;
use crate::services::hardware_service::HardwareService;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallRequest {
    pub mcp_id: String,
    pub tool_name: String,
    pub args: serde_json::Value,
}

#[derive(Clone)]
pub struct ConversationService {
    conversation_repo: ConversationRepo,
    context_service: ContextService,
    inference_service: InferenceService,
    memory_service: MemoryService,
    rag_service: Option<Arc<RagService>>,
    mcp_service: McpService,
    analytics_service: AnalyticsService,
    model_repo: ModelRepo,
    runtime_governor: RuntimeGovernorService,
    task_router: TaskRouterService,
    runtime_orchestrator: Arc<RuntimeOrchestratorService>,
    system_repo: SystemRepo,
    hardware_service: Arc<HardwareService>,
}

impl ConversationService {
    pub fn new(
        conversation_repo: ConversationRepo,
        context_service: ContextService,
        inference_service: InferenceService,
        memory_service: MemoryService,
        rag_service: Option<Arc<RagService>>,
        mcp_service: McpService,
        analytics_service: AnalyticsService,
        model_repo: ModelRepo,
        runtime_governor: RuntimeGovernorService,
        task_router: TaskRouterService,
        runtime_orchestrator: Arc<RuntimeOrchestratorService>,
        system_repo: SystemRepo,
        hardware_service: Arc<HardwareService>,
    ) -> Self {
        Self {
            conversation_repo,
            context_service,
            inference_service,
            memory_service,
            rag_service,
            mcp_service,
            analytics_service,
            model_repo,
            runtime_governor,
            task_router,
            runtime_orchestrator,
            system_repo,
            hardware_service,
        }
    }

    fn is_manual_selection_mode(mode: Option<&str>) -> bool {
        match mode.map(str::trim) {
            Some(value) => value.eq_ignore_ascii_case("manual"),
            None => false,
        }
    }

    async fn resolve_selected_model(&self, selected_model: &str) -> Result<Option<Model>, AppError> {
        let normalized = selected_model.trim();
        if normalized.is_empty() {
            return Ok(None);
        }

        if let Some(by_id) = self.model_repo.get_by_id(normalized).await? {
            return Ok(Some(by_id));
        }

        self.model_repo.get_by_name(normalized).await
    }

    async fn resolve_target_model_for_routing(
        &self,
        routing: &RoutingDecision,
    ) -> Result<Option<Model>, AppError> {
        if let Some(model_id) = routing.selected_model_id.as_deref() {
            if let Some(model) = self.model_repo.get_by_id(model_id).await? {
                return Ok(Some(model));
            }
        }

        let installed = self.model_repo.list_installed().await?;
        Ok(installed
            .iter()
            .find(|m| m.is_default == 1)
            .cloned()
            .or_else(|| installed.first().cloned()))
    }

    async fn active_or_default_profile(&self) -> Result<SystemProfile, AppError> {
        Ok(self
            .system_repo
            .get_current_profile()
            .await?
            .unwrap_or_else(|| crate::db::models::SystemProfile {
                id: "temp".to_string(),
                cpu_brand: "unknown".to_string(),
                cpu_cores: 4,
                cpu_threads: 8,
                cpu_frequency_mhz: Some(2800),
                total_ram_mb: 8192,
                available_ram_mb: Some(4096),
                gpu_name: None,
                gpu_vendor: Some("none".to_string()),
                gpu_vram_mb: Some(0),
                gpu_backend: Some("cpu".to_string()),
                storage_total_gb: Some(0),
                storage_available_gb: Some(0),
                storage_type: None,
                os_name: std::env::consts::OS.to_string(),
                os_version: "unknown".to_string(),
                os_arch: std::env::consts::ARCH.to_string(),
                platform: std::env::consts::OS.to_string(),
                benchmark_tokens_per_sec: None,
                benchmark_embed_ms: None,
                capability_score: None,
                supports_cuda: 0,
                supports_metal: 0,
                supports_vulkan: 0,
                supports_avx2: 0,
                supports_avx512: 0,
                last_scan_at: chrono::Utc::now().to_rfc3339(),
                created_at: chrono::Utc::now().to_rfc3339(),
                updated_at: chrono::Utc::now().to_rfc3339(),
            }))
    }

    async fn ensure_model_loaded(
        &self,
        model: &Model,
        profile: &SystemProfile,
    ) -> Result<(), AppError> {
        let model_path = model.file_path.clone().ok_or_else(|| {
            AppError::Inference(format!(
                "Selected model '{}' is missing local file path.",
                model.display_name
            ))
        })?;

        let active_model = self.inference_service.get_active_model_info().await;
        let should_load = active_model
            .as_ref()
            .map(|loaded| loaded.path != model_path)
            .unwrap_or(true);

        if !should_load {
            return Ok(());
        }

        self.runtime_orchestrator
            .maybe_preload_model(&model_path, profile)
            .await;
        let mode = self.hardware_service.get_performance_mode(None).await;
        self.inference_service
            .load_model(&model_path, profile, mode)
            .await
    }

    pub async fn send_message(
        &self,
        user_id: &str,
        session_id: &str,
        content: &str,
        attachments: &[String],
        model_selection_mode: Option<&str>,
        selected_model: Option<&str>,
        task_type: Option<&str>,
        qos: Option<&str>,
        allow_background_defer: bool,
        app_handle: Option<tauri::AppHandle>,
    ) -> Result<ReceiverStream<MessageStreamChunk>, AppError> {
        let existing = self
            .conversation_repo
            .get_messages(session_id, 1_000, 0)
            .await
            .unwrap_or_default();
        let position = existing.last().map(|m| m.position + 1).unwrap_or(0);

        let user_message = self
            .conversation_repo
            .insert_message(NewMessage {
                session_id: session_id.to_string(),
                role: "user".to_string(),
                content: content.to_string(),
                content_type: "text".to_string(),
                token_count: Some((content.len() / 4) as i64 + 1),
                model_id: None,
                metadata: "{}".to_string(),
                position,
            })
            .await?;

        for path in attachments {
            if let Some(rag) = self.rag_service.as_ref() {
                let _ = rag.ingest_document(user_id, path).await;
            }
        }

        let context = self
            .context_service
            .build_context(user_id, session_id, content)
            .await?;

        let orchestrated = self
            .runtime_orchestrator
            .plan_request(user_id, content, task_type, qos, allow_background_defer)
            .await?;

        let auto_routing = self
            .task_router
            .route(
                user_id,
                Some(session_id),
                content,
                Some(orchestrated.task_type.as_str()),
                Some(orchestrated.qos.as_str()),
                orchestrated.defer_background,
            )
            .await?;

        let manual_mode = Self::is_manual_selection_mode(model_selection_mode);
        let mut routing = auto_routing.clone();
        let mut fallback_notice: Option<String> = None;

        if manual_mode {
            let requested = selected_model.map(str::trim).unwrap_or_default();
            if requested.is_empty() {
                fallback_notice = Some(
                    "Manual model mode was selected without picking a model. Using automatic routing."
                        .to_string(),
                );
            } else {
                match self.resolve_selected_model(requested).await? {
                    Some(model) if model.is_downloaded == 1 => {
                        routing.selected_model_id = Some(model.id.clone());
                        routing.selected_model_name = Some(model.display_name.clone());
                        routing.reason = format!(
                            "{}; selection_mode=manual; requested_model={}",
                            auto_routing.reason, model.name
                        );
                    }
                    Some(model) => {
                        fallback_notice = Some(format!(
                            "Selected model '{}' is not installed. Using automatic routing.",
                            model.display_name
                        ));
                    }
                    None => {
                        fallback_notice = Some(format!(
                            "Selected model '{}' was not found. Using automatic routing.",
                            requested
                        ));
                    }
                }
            }
        }

        let profile = self.active_or_default_profile().await?;
        let mut target_model = self.resolve_target_model_for_routing(&routing).await?;

        if let Some(model) = target_model.clone() {
            if let Err(load_error) = self.ensure_model_loaded(&model, &profile).await {
                if manual_mode {
                    let auto_target = self.resolve_target_model_for_routing(&auto_routing).await?;
                    if let Some(fallback_model) = auto_target {
                        if fallback_model.id != model.id {
                            self.ensure_model_loaded(&fallback_model, &profile)
                                .await
                                .map_err(|fallback_error| {
                                    AppError::Inference(format!(
                                        "Selected model '{}' failed to load ({load_error}). Auto fallback '{}' also failed to load ({fallback_error}).",
                                        model.display_name, fallback_model.display_name
                                    ))
                                })?;

                            fallback_notice = Some(format!(
                                "Selected model '{}' could not be loaded. Switched to '{}' automatically.",
                                model.display_name, fallback_model.display_name
                            ));
                            routing.selected_model_id = Some(fallback_model.id.clone());
                            routing.selected_model_name = Some(fallback_model.display_name.clone());
                            routing.reason =
                                format!("{}; fallback=auto_after_manual_load_failure", routing.reason);
                            target_model = Some(fallback_model);
                        } else {
                            return Err(load_error.context(format!(
                                "Failed to load selected model '{}'",
                                model.display_name
                            )));
                        }
                    } else {
                        return Err(load_error.context(format!(
                            "Failed to load selected model '{}'",
                            model.display_name
                        )));
                    }
                }
            }
        }

        if let Some(model) = target_model.as_ref() {
            self.runtime_orchestrator
                .record_model_usage(&model.name)
                .await;
        }

        let policy = self.runtime_governor.get_policy(Some(user_id)).await?;
        let pressure = self
            .runtime_governor
            .classify_pressure(&self.runtime_governor.current_stats(), &policy);
        let mut tuned_options = GenerationOptions::default();
        tuned_options.max_tokens = routing.max_tokens.min(orchestrated.max_tokens_hint);
        tuned_options = self.runtime_governor.tune_generation(
            tuned_options,
            &policy,
            &orchestrated.qos,
            &pressure,
            orchestrated.defer_background,
        );

        let mut inference_stream = match self
            .inference_service
            .generate_stream(
                session_id,
                context.messages.clone(),
                tuned_options.clone(),
                app_handle.clone(),
            )
            .await
        {
            Ok(stream) => stream,
            Err(error) if manual_mode => {
                let auto_target = self.resolve_target_model_for_routing(&auto_routing).await?;
                if let Some(fallback_model) = auto_target {
                    let already_using_fallback =
                        routing.selected_model_id.as_deref() == Some(fallback_model.id.as_str());
                    if already_using_fallback {
                        return Err(error);
                    }

                    self.ensure_model_loaded(&fallback_model, &profile)
                        .await
                        .map_err(|fallback_error| {
                            AppError::Inference(format!(
                                "Selected model response failed ({error}). Auto fallback '{}' also failed ({fallback_error}).",
                                fallback_model.display_name
                            ))
                        })?;

                    fallback_notice = Some(format!(
                        "Selected model response failed. Switched to '{}' automatically.",
                        fallback_model.display_name
                    ));
                    routing.selected_model_id = Some(fallback_model.id.clone());
                    routing.selected_model_name = Some(fallback_model.display_name.clone());
                    routing.reason =
                        format!("{}; fallback=auto_after_manual_generation_failure", routing.reason);

                    self.inference_service
                        .generate_stream(
                            session_id,
                            context.messages.clone(),
                            tuned_options,
                            app_handle,
                        )
                        .await?
                } else {
                    return Err(error);
                }
            }
            Err(error) => return Err(error),
        };

        let (tx, rx) = tokio::sync::mpsc::channel::<MessageStreamChunk>(256);

        let conversation_repo = self.conversation_repo.clone();
        let memory_service = self.memory_service.clone();
        let analytics_service = self.analytics_service.clone();
        let session_id_owned = session_id.to_string();
        let user_id_owned = user_id.to_string();
        let content_len_estimate = (content.len() / 4) as i64 + 1;
        let selected_model_id = routing.selected_model_id.clone();
        let fallback_notice_for_stream = fallback_notice.clone();

        tokio::spawn(async move {
            let started = std::time::Instant::now();
            let mut full_text = String::new();

            if let Some(notice) = fallback_notice_for_stream {
                let notice_token = format!("{notice}\n\n");
                full_text.push_str(&notice_token);
                if tx
                    .send(MessageStreamChunk {
                        session_id: session_id_owned.clone(),
                        token: notice_token,
                        done: false,
                    })
                    .await
                    .is_err()
                {
                    return;
                }
            }

            while let Some(chunk) = inference_stream.next().await {
                if !chunk.done {
                    full_text.push_str(&chunk.token);
                }
                if tx.send(chunk.clone()).await.is_err() {
                    break;
                }
            }

            if !full_text.trim().is_empty() {
                let existing_messages = conversation_repo
                    .get_messages(&session_id_owned, 1_000, 0)
                    .await
                    .unwrap_or_default();
                let next_position = existing_messages
                    .last()
                    .map(|m| m.position + 1)
                    .unwrap_or(1);

                let assistant = conversation_repo
                    .insert_message(NewMessage {
                        session_id: session_id_owned.clone(),
                        role: "assistant".to_string(),
                        content: full_text.clone(),
                        content_type: "markdown".to_string(),
                        token_count: Some((full_text.len() / 4) as i64 + 1),
                        model_id: selected_model_id.clone(),
                        metadata: "{}".to_string(),
                        position: next_position,
                    })
                    .await;

                if let Ok(assistant_message) = assistant {
                    let paired = vec![user_message.clone(), assistant_message.clone()];
                    if let Ok(extracted) =
                        memory_service.extract_batch(&paired, &user_id_owned).await
                    {
                        let _ = memory_service.persist_extracted(extracted).await;
                    }
                }

                let latency_ms = started.elapsed().as_millis() as i64;
                let _ = analytics_service
                    .log_inference(
                        Some(session_id_owned.clone()),
                        selected_model_id.clone(),
                        latency_ms,
                        Some(content_len_estimate),
                        Some((full_text.len() / 4) as i64 + 1),
                        Some(
                            (full_text.split_whitespace().count() as f64)
                                / (started.elapsed().as_secs_f64().max(0.001)),
                        ),
                        true,
                        None,
                    )
                    .await;
            }
        });

        Ok(ReceiverStream::new(rx))
    }

    pub async fn process_tool_calls(
        &self,
        tool_calls: Vec<ToolCallRequest>,
        session_id: &str,
        message_id: &str,
        user_id: &str,
    ) -> Result<Vec<ToolResult>, AppError> {
        let mut results = Vec::new();

        for call in tool_calls {
            let row = self
                .conversation_repo
                .insert_tool_call(NewToolCall {
                    message_id: message_id.to_string(),
                    session_id: session_id.to_string(),
                    mcp_id: Some(call.mcp_id.clone()),
                    tool_name: call.tool_name.clone(),
                    tool_input: call.args.to_string(),
                })
                .await?;

            match self
                .mcp_service
                .call_tool(&call.mcp_id, &call.tool_name, call.args.clone(), user_id)
                .await
            {
                Ok(result) => {
                    self.conversation_repo
                        .update_tool_call_result(
                            &row.id,
                            Some(&result.output),
                            "success",
                            result.latency_ms,
                        )
                        .await?;
                    results.push(result);
                }
                Err(err) => {
                    let _ = self
                        .conversation_repo
                        .update_tool_call_result(&row.id, None, "error", 0)
                        .await;
                    return Err(err);
                }
            }
        }

        Ok(results)
    }

    pub async fn generate_session_title(&self, messages: &[Message]) -> Result<String, AppError> {
        let content = messages
            .iter()
            .take(2)
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>()
            .join(" ");

        let title = content
            .split_whitespace()
            .take(5)
            .collect::<Vec<_>>()
            .join(" ");

        Ok(if title.is_empty() {
            "New Conversation".to_string()
        } else {
            title
        })
    }

    pub async fn summarize_session(&self, session_id: &str) -> Result<(), AppError> {
        let messages = self
            .conversation_repo
            .get_messages(session_id, 500, 0)
            .await?;
        if messages.is_empty() {
            return Ok(());
        }

        let summary = messages
            .iter()
            .rev()
            .take(8)
            .map(|m| format!("{}: {}", m.role, m.content))
            .collect::<Vec<_>>()
            .join("\n");

        self.conversation_repo
            .update_session_summary(session_id, &summary)
            .await
    }
}
