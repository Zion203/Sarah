use std::sync::Arc;

use tauri::State;

use crate::db::models::{Mcp, McpHealthStatus, McpUsageStat};
use crate::error::AppError;
use crate::state::AppState;

#[tauri::command]
pub async fn list_mcps(
    state: State<'_, Arc<AppState>>,
    installed_only: bool,
) -> Result<Vec<Mcp>, AppError> {
    crate::log_info!("sarah.command", "list_mcps invoked");
    state.mcp_repo.list_mcps(installed_only).await
}

#[tauri::command]
pub async fn install_mcp(state: State<'_, Arc<AppState>>, mcp_id: String) -> Result<(), AppError> {
    crate::log_info!("sarah.command", "install_mcp invoked");
    state.mcp_repo.install_mcp(&mcp_id).await
}

#[tauri::command]
pub async fn activate_mcp(state: State<'_, Arc<AppState>>, mcp_id: String) -> Result<(), AppError> {
    crate::log_info!("sarah.command", "activate_mcp invoked");
    state.mcp_repo.set_active(&mcp_id, true).await
}

#[tauri::command]
pub async fn deactivate_mcp(
    state: State<'_, Arc<AppState>>,
    mcp_id: String,
) -> Result<(), AppError> {
    crate::log_info!("sarah.command", "deactivate_mcp invoked");
    state.mcp_repo.set_active(&mcp_id, false).await
}

#[tauri::command]
pub async fn save_mcp_secret(
    state: State<'_, Arc<AppState>>,
    mcp_id: String,
    user_id: String,
    key: String,
    value: String,
) -> Result<(), AppError> {
    crate::log_info!("sarah.command", "save_mcp_secret invoked");
    state
        .mcp
        .save_mcp_secret(&mcp_id, &user_id, &key, &value)
        .await
}

#[tauri::command]
pub async fn test_mcp_connection(
    state: State<'_, Arc<AppState>>,
    mcp_id: String,
) -> Result<McpHealthStatus, AppError> {
    crate::log_info!("sarah.command", "test_mcp_connection invoked");
    let _ = state.mcp.ensure_connected(&mcp_id).await?;
    let statuses = state.mcp.health_check_all().await?;

    statuses
        .into_iter()
        .find(|status| status.mcp_id == mcp_id)
        .ok_or_else(|| AppError::NotFound {
            entity: "mcp_health".to_string(),
            id: mcp_id,
        })
}

#[tauri::command]
pub async fn get_mcp_stats(
    state: State<'_, Arc<AppState>>,
    mcp_id: String,
) -> Result<Vec<McpUsageStat>, AppError> {
    crate::log_info!("sarah.command", "get_mcp_stats invoked");
    state.mcp.get_stats(&mcp_id).await
}
