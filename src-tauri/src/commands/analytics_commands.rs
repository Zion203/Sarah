use std::sync::Arc;

use tauri::State;

use crate::db::models::PerfLog;
use crate::error::AppError;
use crate::state::AppState;

#[tauri::command]
pub async fn get_recent_perf_logs(
    state: State<'_, Arc<AppState>>,
    limit: Option<i64>,
) -> Result<Vec<PerfLog>, AppError> {
    crate::log_info!("sarah.command", "get_recent_perf_logs invoked");
    state
        .analytics_repo
        .get_recent_perf_logs(limit.unwrap_or(200))
        .await
}

#[tauri::command]
pub async fn run_analytics_aggregation(state: State<'_, Arc<AppState>>) -> Result<(), AppError> {
    crate::log_info!("sarah.command", "run_analytics_aggregation invoked");
    state.analytics.aggregate_daily().await
}
