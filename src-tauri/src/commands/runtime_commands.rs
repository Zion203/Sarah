use std::sync::Arc;
use std::time::Instant;

use tauri::State;
use tokio::time::Duration;
use uuid::Uuid;

use crate::commands::model_commands::{
    ensure_catalog_seeded, run_nlp_setup_inner, start_model_download_inner,
};
use crate::db::models::{
    Message, ModelBenchmark, PerformanceSummary, RoutingDecision, RoutingPreviewRequest,
    RuntimePolicy, RuntimePolicyPatch, SetupState, SystemProfile,
};
use crate::error::AppError;
use crate::services::runtime_orchestrator_service::{
    OptimizationStatsSnapshot, RuntimeProfileSnapshot, ServiceHealthSnapshot,
};
use crate::state::AppState;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupTelemetrySnapshot {
    pub startup_started_at_utc: String,
    pub startup_completed_at_utc: String,
    pub startup_init_ms: i64,
    pub first_response_latency_ms: Option<i64>,
    pub latest_inference_latency_ms: Option<i64>,
    pub last_setup_duration_ms: Option<i64>,
    pub active_hardware_profile_id: Option<String>,
    pub active_hardware_tier: String,
}

#[tauri::command]
pub async fn get_runtime_policy(
    state: State<'_, Arc<AppState>>,
    user_id: Option<String>,
) -> Result<RuntimePolicy, AppError> {
    crate::log_info!("sarah.command", "get_runtime_policy invoked");
    state.runtime_governor.get_policy(user_id.as_deref()).await
}

#[tauri::command]
pub async fn set_runtime_policy(
    state: State<'_, Arc<AppState>>,
    user_id: Option<String>,
    patch: RuntimePolicyPatch,
) -> Result<RuntimePolicy, AppError> {
    crate::log_info!("sarah.command", "set_runtime_policy invoked");
    state
        .runtime_governor
        .set_policy(user_id.as_deref(), patch)
        .await
}

#[tauri::command]
pub async fn get_runtime_profile(
    state: State<'_, Arc<AppState>>,
    user_id: Option<String>,
) -> Result<RuntimeProfileSnapshot, AppError> {
    crate::log_info!("sarah.command", "get_runtime_profile invoked");
    state
        .runtime_orchestrator
        .get_runtime_profile(user_id.as_deref())
        .await
}

#[tauri::command]
pub async fn get_service_health(
    state: State<'_, Arc<AppState>>,
) -> Result<ServiceHealthSnapshot, AppError> {
    crate::log_info!("sarah.command", "get_service_health invoked");
    Ok(state.runtime_orchestrator.get_service_health().await)
}

#[tauri::command]
pub async fn get_optimization_stats(
    state: State<'_, Arc<AppState>>,
) -> Result<OptimizationStatsSnapshot, AppError> {
    crate::log_info!("sarah.command", "get_optimization_stats invoked");
    Ok(state.runtime_orchestrator.get_optimization_stats().await)
}

#[tauri::command]
pub async fn get_startup_telemetry(
    state: State<'_, Arc<AppState>>,
) -> Result<StartupTelemetrySnapshot, AppError> {
    crate::log_info!("sarah.command", "get_startup_telemetry invoked");
    let latest_inference_latency_ms = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT latency_ms
        FROM perf_logs
        WHERE event_type = 'inference'
        ORDER BY datetime(created_at) DESC
        LIMIT 1
        "#,
    )
    .fetch_optional(state.db.read_pool())
    .await?;

    let setup_state_duration_ms = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT CAST((julianday(updated_at) - julianday(created_at)) * 86400000 AS INTEGER)
        FROM setup_state
        WHERE status = 'completed'
        ORDER BY datetime(updated_at) DESC
        LIMIT 1
        "#,
    )
    .fetch_optional(state.db.read_pool())
    .await?;

    let model_download_duration_ms = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT CAST((julianday(completed_at) - julianday(started_at)) * 86400000 AS INTEGER)
        FROM model_downloads
        WHERE status = 'completed' AND completed_at IS NOT NULL
        ORDER BY datetime(completed_at) DESC
        LIMIT 1
        "#,
    )
    .fetch_optional(state.db.read_pool())
    .await?;

    let last_setup_duration_ms = setup_state_duration_ms.or(model_download_duration_ms);
    let active_hardware_profile_id = state.hardware.read().await.as_ref().map(|p| p.id.clone());

    Ok(StartupTelemetrySnapshot {
        startup_started_at_utc: state.startup_started_at_utc.clone(),
        startup_completed_at_utc: state.startup_completed_at_utc.clone(),
        startup_init_ms: state.startup_init_ms,
        first_response_latency_ms: state.analytics.first_inference_latency_ms(),
        latest_inference_latency_ms,
        last_setup_duration_ms,
        active_hardware_profile_id,
        active_hardware_tier: state.tier.to_string(),
    })
}

#[tauri::command]
pub async fn get_model_routing_decision(
    state: State<'_, Arc<AppState>>,
    request: RoutingPreviewRequest,
) -> Result<RoutingDecision, AppError> {
    crate::log_info!("sarah.command", "get_model_routing_decision invoked");
    state
        .task_router
        .preview(
            &request.user_id,
            &request.content,
            request.task_type.as_deref(),
            request.qos.as_deref(),
        )
        .await
}

#[tauri::command]
pub async fn get_performance_dashboard(
    state: State<'_, Arc<AppState>>,
    window_hours: Option<i64>,
) -> Result<PerformanceSummary, AppError> {
    crate::log_info!("sarah.command", "get_performance_dashboard invoked");
    let window = window_hours.unwrap_or(24).clamp(1, 24 * 30);

    let total_events = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM perf_logs WHERE datetime(created_at) >= datetime('now', '-' || ?1 || ' hour')",
    )
    .bind(window)
    .fetch_one(state.db.read_pool())
    .await?;

    let success_rate = sqlx::query_scalar::<_, Option<f64>>(
        "SELECT AVG(CASE WHEN success = 1 THEN 1.0 ELSE 0.0 END) FROM perf_logs WHERE datetime(created_at) >= datetime('now', '-' || ?1 || ' hour')",
    )
    .bind(window)
    .fetch_one(state.db.read_pool())
    .await?
    .unwrap_or(1.0);

    let avg_tokens_per_sec = sqlx::query_scalar::<_, Option<f64>>(
        "SELECT AVG(tokens_per_sec) FROM perf_logs WHERE datetime(created_at) >= datetime('now', '-' || ?1 || ' hour')",
    )
    .bind(window)
    .fetch_one(state.db.read_pool())
    .await?;

    let mut latencies = sqlx::query_scalar::<_, i64>(
        "SELECT latency_ms FROM perf_logs WHERE datetime(created_at) >= datetime('now', '-' || ?1 || ' hour') ORDER BY latency_ms ASC",
    )
    .bind(window)
    .fetch_all(state.db.read_pool())
    .await?;

    let (p50_latency_ms, p95_latency_ms) = if latencies.is_empty() {
        (None, None)
    } else {
        latencies.sort_unstable();
        let p50_idx = ((latencies.len() as f64) * 0.50).floor() as usize;
        let p95_idx = ((latencies.len() as f64) * 0.95).floor() as usize;
        let p50 = latencies[p50_idx.min(latencies.len() - 1)] as f64;
        let p95 = latencies[p95_idx.min(latencies.len() - 1)] as f64;
        (Some(p50), Some(p95))
    };

    Ok(PerformanceSummary {
        window_hours: window,
        total_events,
        success_rate,
        p50_latency_ms,
        p95_latency_ms,
        avg_tokens_per_sec,
    })
}

#[tauri::command]
pub async fn run_model_microbenchmark(
    state: State<'_, Arc<AppState>>,
    model_id: Option<String>,
) -> Result<ModelBenchmark, AppError> {
    crate::log_info!("sarah.command", "run_model_microbenchmark invoked");
    run_model_microbenchmark_inner(Arc::clone(&state), model_id.as_deref()).await
}

#[tauri::command]
pub async fn start_first_run_setup(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    user_id: Option<String>,
) -> Result<SetupState, AppError> {
    crate::log_info!("sarah.command", "start_first_run_setup invoked");
    ensure_catalog_seeded(&state).await?;

    let profile = ensure_hardware_profile(&state).await?;
    let starter_name = choose_starter_bundle(&profile);
    let starter_model = state
        .model_repo
        .get_by_name(starter_name)
        .await?
        .ok_or_else(|| AppError::NotFound {
            entity: "model".to_string(),
            id: starter_name.to_string(),
        })?;

    let uid = user_id.as_deref();
    state
        .setup_orchestrator
        .start_or_resume(uid, Some(&starter_model.id), Some(&profile.id))
        .await?;
    state
        .setup_orchestrator
        .update_stage(uid, "stage_a_preflight", 25.0)
        .await?;

    let setup_result = match run_nlp_setup_inner(
        app.clone(),
        Arc::clone(&state),
        Some(starter_model.id.clone()),
        user_id.clone(),
    )
    .await
    {
        Ok(result) => result,
        Err(error) => {
            let _ = state
                .setup_orchestrator
                .mark_failed(uid, "stage_b_starter_model_install", &error.to_string())
                .await;
            return Err(error);
        }
    };

    state
        .setup_orchestrator
        .update_stage(uid, "stage_b_starter_model_install", 60.0)
        .await?;

    if setup_result.download_status == "already_downloaded"
        || setup_result.download_status == "completed"
    {
        let _ = run_model_microbenchmark_inner(
            Arc::clone(&state),
            Some(setup_result.target_model_id.as_str()),
        )
        .await;
    }

    state
        .setup_orchestrator
        .update_stage(uid, "stage_c_runtime_profile", 85.0)
        .await?;

    maybe_queue_quality_upgrade(
        app,
        Arc::clone(&state),
        &profile,
        &setup_result.target_model_id,
        user_id.clone(),
    )
    .await;

    state.setup_orchestrator.mark_completed(uid).await
}

#[tauri::command]
pub async fn get_setup_status(
    state: State<'_, Arc<AppState>>,
    user_id: Option<String>,
) -> Result<Option<SetupState>, AppError> {
    crate::log_info!("sarah.command", "get_setup_status invoked");
    state.setup_orchestrator.get_state(user_id.as_deref()).await
}

#[tauri::command]
pub async fn retry_setup_stage(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    user_id: Option<String>,
    stage: String,
) -> Result<SetupState, AppError> {
    crate::log_info!("sarah.command", "retry_setup_stage invoked");
    state
        .setup_orchestrator
        .retry_stage(user_id.as_deref(), &stage)
        .await?;
    start_first_run_setup(app, state, user_id).await
}

#[tauri::command]
pub async fn skip_quality_upgrade_for_now(
    state: State<'_, Arc<AppState>>,
    user_id: Option<String>,
) -> Result<SetupState, AppError> {
    crate::log_info!("sarah.command", "skip_quality_upgrade_for_now invoked");
    state
        .setup_orchestrator
        .skip_quality_upgrade(user_id.as_deref())
        .await
}

async fn run_model_microbenchmark_inner(
    state: Arc<AppState>,
    model_id: Option<&str>,
) -> Result<ModelBenchmark, AppError> {
    ensure_catalog_seeded(&state).await?;

    let selected = if let Some(value) = model_id {
        if let Some(model) = state.model_repo.get_by_id(value).await? {
            Some(model)
        } else {
            state.model_repo.get_by_name(value).await?
        }
    } else {
        let installed = state.model_repo.list_installed().await?;
        installed
            .iter()
            .find(|row| row.is_default == 1)
            .cloned()
            .or_else(|| installed.first().cloned())
    }
    .ok_or_else(|| AppError::NotFound {
        entity: "model".to_string(),
        id: model_id.unwrap_or("default").to_string(),
    })?;

    let model_path = selected
        .file_path
        .clone()
        .ok_or_else(|| AppError::Validation {
            field: "model_id".to_string(),
            message: "Model has no local file path".to_string(),
        })?;

    let profile = ensure_hardware_profile(&state).await?;

    let load_started = std::time::Instant::now();
    let mode = state.hardware_service.get_performance_mode(None).await;
    state.inference.load_model(&model_path, &profile, mode).await?;
    let load_time_ms = load_started.elapsed().as_millis() as i64;

    let prompt = "Write one sentence confirming benchmark execution.";
    let request = Message {
        id: "bench-user".to_string(),
        session_id: "bench-session".to_string(),
        role: "user".to_string(),
        content: prompt.to_string(),
        content_type: "text".to_string(),
        thinking: None,
        token_count: Some((prompt.len() / 4) as i64 + 1),
        model_id: Some(selected.id.clone()),
        latency_ms: None,
        tokens_per_sec: None,
        finish_reason: None,
        is_error: 0,
        error_message: None,
        parent_message_id: None,
        edited_at: None,
        original_content: None,
        metadata: "{}".to_string(),
        position: 0,
        created_at: String::new(),
        updated_at: String::new(),
    };

    let started = std::time::Instant::now();
    let generated = state
        .inference
        .generate_with_tools(vec![request], &[])
        .await?;
    let total_latency_ms = started.elapsed().as_millis() as i64;
    let tokens_per_sec =
        generated.tokens_generated as f64 / started.elapsed().as_secs_f64().max(0.001);

    let stats = state.runtime_governor.current_stats();
    let benchmark_id = Uuid::new_v4().to_string();

    sqlx::query(
        r#"
        INSERT INTO model_benchmarks (
          id, model_id, system_profile_id, context_tokens, prompt_tokens, output_tokens,
          load_time_ms, first_token_ms, total_latency_ms, tokens_per_sec, memory_used_mb,
          cpu_usage_pct, success, metadata
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, ?8, ?9, ?10, ?11, 1, '{}')
        "#,
    )
    .bind(&benchmark_id)
    .bind(&selected.id)
    .bind(&profile.id)
    .bind(0i64)
    .bind((prompt.len() / 4) as i64 + 1)
    .bind(generated.tokens_generated as i64)
    .bind(load_time_ms)
    .bind(total_latency_ms)
    .bind(tokens_per_sec)
    .bind(stats.memory_used_mb as i64)
    .bind(stats.cpu_usage_pct as f64)
    .execute(state.db.write_pool())
    .await?;

    let _ = state
        .model_repo
        .update_performance_metrics(&selected.id, tokens_per_sec)
        .await;

    let row = sqlx::query_as::<_, ModelBenchmark>("SELECT * FROM model_benchmarks WHERE id = ?1")
        .bind(&benchmark_id)
        .fetch_one(state.db.read_pool())
        .await?;
    Ok(row)
}

async fn ensure_hardware_profile(state: &Arc<AppState>) -> Result<SystemProfile, AppError> {
    if let Some(profile) = state.hardware.read().await.clone() {
        return Ok(profile);
    }
    let detected = state.hardware_service.detect_hardware().await?;
    *state.hardware.write().await = Some(detected.clone());
    Ok(detected)
}

fn choose_starter_bundle(profile: &SystemProfile) -> &'static str {
    if profile.total_ram_mb >= 12_000 {
        "llama-3.2-1b-instruct-q4_k_m"
    } else {
        "qwen2.5-0.5b-instruct-q4_k_m"
    }
}

async fn maybe_queue_quality_upgrade(
    app: tauri::AppHandle,
    state: Arc<AppState>,
    profile: &SystemProfile,
    starter_model_id: &str,
    user_id: Option<String>,
) {
    let target_name = choose_quality_upgrade_target(profile);
    let Some(target_name) = target_name else {
        return;
    };

    let target = match state
        .model_repo
        .get_by_name(target_name)
        .await
        .ok()
        .flatten()
    {
        Some(model) if model.id != starter_model_id => model,
        _ => return,
    };

    if target.is_downloaded == 1 {
        return;
    }

    let target_id = target.id.clone();
    let target_display_name = target.display_name.clone();

    let job_id = Uuid::new_v4().to_string();
    let metadata = serde_json::json!({
        "targetModelId": target_id.clone(),
        "targetModelName": target_display_name,
        "starterModelId": starter_model_id,
        "strategy": "auto-upgrade-idle-gated"
    })
    .to_string();

    let _ = sqlx::query(
        r#"
        INSERT INTO background_job_runs (id, job_type, status, deferred_reason, metadata)
        VALUES (?1, 'auto_model_upgrade', 'queued', NULL, ?2)
        "#,
    )
    .bind(&job_id)
    .bind(&metadata)
    .execute(state.db.write_pool())
    .await;

    let state_cloned = Arc::clone(&state);
    let app_cloned = app.clone();
    let user_id_cloned = user_id.clone();

    tokio::spawn(async move {
        let started = Instant::now();
        let mut deferred_count = 0usize;
        let max_attempts = 24usize;

        for _ in 0..max_attempts {
            let policy = match state_cloned
                .runtime_governor
                .get_policy(user_id_cloned.as_deref())
                .await
            {
                Ok(policy) => policy,
                Err(_) => RuntimePolicy::default(),
            };
            let stats = state_cloned.runtime_governor.current_stats();
            let pressure = state_cloned
                .runtime_governor
                .classify_pressure(&stats, &policy);

            let can_upgrade = matches!(pressure.as_str(), "normal" | "warm");
            if can_upgrade {
                match start_model_download_inner(
                    app_cloned.clone(),
                    Arc::clone(&state_cloned),
                    target_id.clone(),
                )
                .await
                {
                    Ok(handle) => {
                        let _ = sqlx::query(
                            r#"
                            UPDATE background_job_runs
                            SET status = 'submitted',
                                completed_at = datetime('now','utc'),
                                latency_ms = ?1,
                                metadata = json_set(COALESCE(metadata, '{}'), '$.downloadStatus', ?2)
                            WHERE id = ?3
                            "#,
                        )
                        .bind(started.elapsed().as_millis() as i64)
                        .bind(handle.status)
                        .bind(&job_id)
                        .execute(state_cloned.db.write_pool())
                        .await;
                        return;
                    }
                    Err(error) => {
                        let _ = sqlx::query(
                            r#"
                            UPDATE background_job_runs
                            SET status = 'failed',
                                deferred_reason = ?1,
                                completed_at = datetime('now','utc'),
                                latency_ms = ?2
                            WHERE id = ?3
                            "#,
                        )
                        .bind(error.to_string())
                        .bind(started.elapsed().as_millis() as i64)
                        .bind(&job_id)
                        .execute(state_cloned.db.write_pool())
                        .await;
                        return;
                    }
                }
            }

            deferred_count += 1;
            tokio::time::sleep(Duration::from_secs(30)).await;
        }

        let _ = sqlx::query(
            r#"
            UPDATE background_job_runs
            SET status = 'deferred',
                deferred_reason = ?1,
                completed_at = datetime('now','utc'),
                latency_ms = ?2,
                metadata = json_set(COALESCE(metadata, '{}'), '$.deferCount', ?3)
            WHERE id = ?4
            "#,
        )
        .bind("system remained under pressure during upgrade window")
        .bind(started.elapsed().as_millis() as i64)
        .bind(deferred_count as i64)
        .bind(&job_id)
        .execute(state_cloned.db.write_pool())
        .await;
    });
}

fn choose_quality_upgrade_target(profile: &SystemProfile) -> Option<&'static str> {
    if profile.total_ram_mb >= 12_000 {
        return Some("qwen2.5-1.5b-instruct-q4_k_m");
    }

    None
}
