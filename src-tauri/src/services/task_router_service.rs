use sqlx::SqlitePool;
use uuid::Uuid;

use crate::db::models::{GenerationOptions, RoutingDecision};
use crate::error::AppError;
use crate::repositories::model_repo::ModelRepo;
use crate::services::runtime_governor_service::RuntimeGovernorService;

#[derive(Clone)]
pub struct TaskRouterService {
    model_repo: ModelRepo,
    runtime_governor: RuntimeGovernorService,
    write_pool: SqlitePool,
}

impl TaskRouterService {
    pub fn new(
        model_repo: ModelRepo,
        runtime_governor: RuntimeGovernorService,
        write_pool: SqlitePool,
    ) -> Self {
        Self {
            model_repo,
            runtime_governor,
            write_pool,
        }
    }

    pub async fn route(
        &self,
        user_id: &str,
        session_id: Option<&str>,
        content: &str,
        task_type: Option<&str>,
        qos: Option<&str>,
        is_background: bool,
    ) -> Result<RoutingDecision, AppError> {
        let task = task_type
            .map(normalize_task_type)
            .unwrap_or_else(|| infer_task_type(content));
        let requested_qos = qos.map(normalize_qos).unwrap_or_else(|| default_qos(&task));

        let policy = self.runtime_governor.get_policy(Some(user_id)).await?;
        let stats = self.runtime_governor.current_stats();
        let pressure = self.runtime_governor.classify_pressure(&stats, &policy);

        let installed = self.model_repo.list_installed().await?;
        let selected = select_model(&installed, &task, &requested_qos);

        let mut base_opts = GenerationOptions::default();
        base_opts.max_tokens = base_max_tokens(&task);
        let tuned = self.runtime_governor.tune_generation(
            base_opts,
            &policy,
            &requested_qos,
            &pressure,
            is_background,
        );

        let fallback_chain = fallback_chain(&installed, selected.as_ref().map(|m| m.id.as_str()));
        let reason = format!(
            "task={}, qos={}, pressure={}, fallback={}",
            task,
            requested_qos,
            pressure,
            fallback_chain.len()
        );

        let decision = RoutingDecision {
            task_type: task,
            qos: requested_qos,
            selected_model_id: selected.as_ref().map(|m| m.id.clone()),
            selected_model_name: selected.as_ref().map(|m| m.display_name.clone()),
            max_tokens: tuned.max_tokens,
            pressure_level: pressure,
            reason,
            fallback_chain,
        };

        let _ = self
            .record_route_event(user_id, session_id, &decision)
            .await;
        Ok(decision)
    }

    pub async fn preview(
        &self,
        user_id: &str,
        content: &str,
        task_type: Option<&str>,
        qos: Option<&str>,
    ) -> Result<RoutingDecision, AppError> {
        let task = task_type
            .map(normalize_task_type)
            .unwrap_or_else(|| infer_task_type(content));
        let requested_qos = qos.map(normalize_qos).unwrap_or_else(|| default_qos(&task));

        let policy = self.runtime_governor.get_policy(Some(user_id)).await?;
        let stats = self.runtime_governor.current_stats();
        let pressure = self.runtime_governor.classify_pressure(&stats, &policy);
        let installed = self.model_repo.list_installed().await?;
        let selected = select_model(&installed, &task, &requested_qos);

        let mut base_opts = GenerationOptions::default();
        base_opts.max_tokens = base_max_tokens(&task);
        let tuned = self.runtime_governor.tune_generation(
            base_opts,
            &policy,
            &requested_qos,
            &pressure,
            false,
        );

        Ok(RoutingDecision {
            task_type: task,
            qos: requested_qos,
            selected_model_id: selected.as_ref().map(|m| m.id.clone()),
            selected_model_name: selected.as_ref().map(|m| m.display_name.clone()),
            max_tokens: tuned.max_tokens,
            pressure_level: pressure,
            reason: "preview".to_string(),
            fallback_chain: fallback_chain(&installed, selected.as_ref().map(|m| m.id.as_str())),
        })
    }

    async fn record_route_event(
        &self,
        user_id: &str,
        session_id: Option<&str>,
        decision: &RoutingDecision,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
            INSERT INTO routing_events (
              id, user_id, session_id, requested_task_type, requested_qos,
              selected_model_id, fallback_chain, pressure_level, max_tokens, reason
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(user_id)
        .bind(session_id)
        .bind(&decision.task_type)
        .bind(&decision.qos)
        .bind(&decision.selected_model_id)
        .bind(serde_json::to_string(&decision.fallback_chain).unwrap_or_else(|_| "[]".to_string()))
        .bind(&decision.pressure_level)
        .bind(decision.max_tokens as i64)
        .bind(&decision.reason)
        .execute(&self.write_pool)
        .await?;
        Ok(())
    }
}

fn normalize_task_type(value: &str) -> String {
    match value.trim().to_lowercase().as_str() {
        "chat" | "code" | "reasoning" | "retrieval_heavy" | "tool_heavy" => {
            value.trim().to_lowercase()
        }
        _ => "chat".to_string(),
    }
}

fn normalize_qos(value: &str) -> String {
    match value.trim().to_lowercase().as_str() {
        "fast" | "balanced" | "max_quality" => value.trim().to_lowercase(),
        _ => "balanced".to_string(),
    }
}

fn infer_task_type(content: &str) -> String {
    let q = content.to_lowercase();
    if q.contains("debug")
        || q.contains("error")
        || q.contains("stack trace")
        || q.contains("refactor")
    {
        return "code".to_string();
    }
    if q.contains("analy")
        || q.contains("reason")
        || q.contains("compare")
        || q.contains("tradeoff")
    {
        return "reasoning".to_string();
    }
    if q.contains("document") || q.contains("source") || q.contains("citation") {
        return "retrieval_heavy".to_string();
    }
    if q.contains("tool") || q.contains("calendar") || q.contains("spotify") || q.contains("run") {
        return "tool_heavy".to_string();
    }
    "chat".to_string()
}

fn default_qos(task_type: &str) -> String {
    match task_type {
        "reasoning" => "max_quality".to_string(),
        "tool_heavy" => "fast".to_string(),
        _ => "balanced".to_string(),
    }
}

fn base_max_tokens(task_type: &str) -> usize {
    match task_type {
        "reasoning" => 1024,
        "code" => 768,
        "retrieval_heavy" => 640,
        "tool_heavy" => 384,
        _ => 512,
    }
}

fn select_model(
    installed: &[crate::db::models::Model],
    task_type: &str,
    qos: &str,
) -> Option<crate::db::models::Model> {
    if installed.is_empty() {
        return None;
    }

    if let Some(default_model) = installed.iter().find(|row| row.is_default == 1) {
        if qos == "balanced" && task_type == "chat" {
            return Some(default_model.clone());
        }
    }

    if qos == "fast" {
        if let Some(fast) = installed.iter().find(|row| row.performance_tier == "fast") {
            return Some(fast.clone());
        }
    }

    if qos == "max_quality" || task_type == "reasoning" {
        if let Some(best) = installed
            .iter()
            .filter(|row| row.performance_tier != "fast")
            .max_by_key(|row| row.context_length)
        {
            return Some(best.clone());
        }
    }

    installed
        .iter()
        .find(|row| row.is_default == 1)
        .cloned()
        .or_else(|| installed.first().cloned())
}

fn fallback_chain(installed: &[crate::db::models::Model], selected: Option<&str>) -> Vec<String> {
    installed
        .iter()
        .filter(|row| selected.map(|id| id != row.id).unwrap_or(true))
        .map(|row| row.id.clone())
        .take(3)
        .collect()
}
