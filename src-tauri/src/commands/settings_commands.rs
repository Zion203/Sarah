use std::sync::Arc;

use tauri::State;

use crate::error::AppError;
use crate::repositories::settings_repo::Setting;
use crate::state::AppState;

#[tauri::command]
pub async fn get_setting(
    state: State<'_, Arc<AppState>>,
    user_id: Option<String>,
    namespace: String,
    key: String,
) -> Result<Option<Setting>, AppError> {
    crate::log_info!("sarah.command", "get_setting invoked");
    state
        .settings_repo
        .get_setting(user_id.as_deref(), &namespace, &key)
        .await
}

#[tauri::command]
pub async fn set_setting(
    state: State<'_, Arc<AppState>>,
    user_id: Option<String>,
    namespace: String,
    key: String,
    value: String,
    value_type: String,
    is_encrypted: bool,
) -> Result<Setting, AppError> {
    crate::log_info!("sarah.command", "set_setting invoked");
    let setting = state
        .settings_repo
        .upsert_setting(
            user_id.as_deref(),
            &namespace,
            &key,
            &value,
            &value_type,
            is_encrypted,
        )
        .await?;

    Ok(setting)
}

#[tauri::command]
pub async fn list_settings_namespace(
    state: State<'_, Arc<AppState>>,
    user_id: Option<String>,
    namespace: String,
) -> Result<Vec<Setting>, AppError> {
    crate::log_info!("sarah.command", "list_settings_namespace invoked");
    state
        .settings_repo
        .list_namespace(user_id.as_deref(), &namespace)
        .await
}
