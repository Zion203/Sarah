use std::sync::Arc;

use tauri::State;

use crate::db::models::{BenchmarkResult, LiveSystemStats, SystemProfile};
use crate::error::AppError;
use crate::state::AppState;

#[tauri::command]
pub async fn get_hardware_profile(
    state: State<'_, Arc<AppState>>,
) -> Result<SystemProfile, AppError> {
    crate::log_info!("sarah.command", "get_hardware_profile invoked");
    if let Some(profile) = state.hardware.read().await.clone() {
        return Ok(profile);
    }

    let profile = state.hardware_service.detect_hardware().await?;
    *state.hardware.write().await = Some(profile.clone());
    Ok(profile)
}

#[tauri::command]
pub async fn run_hardware_benchmark(
    state: State<'_, Arc<AppState>>,
) -> Result<BenchmarkResult, AppError> {
    crate::log_info!("sarah.command", "run_hardware_benchmark invoked");
    let result = state.hardware_service.run_benchmark().await?;
    Ok(result)
}

#[tauri::command]
pub async fn get_system_stats(
    state: State<'_, Arc<AppState>>,
) -> Result<LiveSystemStats, AppError> {
    crate::log_info!("sarah.command", "get_system_stats invoked");
    Ok(state.hardware_service.live_stats())
}
