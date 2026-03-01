use std::sync::atomic::{AtomicI64, Ordering};

use crate::error::AppError;
use crate::repositories::analytics_repo::{AnalyticsRepo, NewPerfLog};

static FIRST_INFERENCE_LATENCY_MS: AtomicI64 = AtomicI64::new(-1);

#[derive(Clone)]
pub struct AnalyticsService {
    repo: AnalyticsRepo,
}

impl AnalyticsService {
    pub fn new(repo: AnalyticsRepo) -> Self {
        Self { repo }
    }

    pub async fn log_inference(
        &self,
        session_id: Option<String>,
        model_id: Option<String>,
        latency_ms: i64,
        tokens_in: Option<i64>,
        tokens_out: Option<i64>,
        tokens_per_sec: Option<f64>,
        success: bool,
        error_code: Option<String>,
    ) -> Result<(), AppError> {
        if success && latency_ms >= 0 {
            let _ = FIRST_INFERENCE_LATENCY_MS.compare_exchange(
                -1,
                latency_ms,
                Ordering::SeqCst,
                Ordering::SeqCst,
            );
        }

        self.repo
            .insert_perf_log(NewPerfLog {
                event_type: "inference".to_string(),
                session_id,
                model_id,
                mcp_id: None,
                latency_ms,
                tokens_in,
                tokens_out,
                tokens_per_sec,
                cpu_usage_pct: None,
                ram_usage_mb: None,
                gpu_usage_pct: None,
                success,
                error_code,
                metadata: None,
            })
            .await
    }

    pub fn first_inference_latency_ms(&self) -> Option<i64> {
        let value = FIRST_INFERENCE_LATENCY_MS.load(Ordering::SeqCst);
        if value < 0 {
            None
        } else {
            Some(value)
        }
    }

    pub async fn log_event(
        &self,
        event_type: &str,
        latency_ms: i64,
        success: bool,
        metadata: Option<String>,
    ) -> Result<(), AppError> {
        self.repo
            .insert_perf_log(NewPerfLog {
                event_type: event_type.to_string(),
                session_id: None,
                model_id: None,
                mcp_id: None,
                latency_ms,
                tokens_in: None,
                tokens_out: None,
                tokens_per_sec: None,
                cpu_usage_pct: None,
                ram_usage_mb: None,
                gpu_usage_pct: None,
                success,
                error_code: None,
                metadata,
            })
            .await
    }

    pub async fn report_error(
        &self,
        component: &str,
        code: &str,
        message: &str,
    ) -> Result<(), AppError> {
        self.repo
            .insert_error_report(code, message, component, "error", None)
            .await
    }

    pub async fn aggregate_daily(&self) -> Result<(), AppError> {
        let _ = self.repo.prune_old_perf_logs(30).await?;
        Ok(())
    }
}
