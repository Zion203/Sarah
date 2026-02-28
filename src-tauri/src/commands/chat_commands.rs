use std::sync::Arc;

use tauri::State;
use tokio_stream::StreamExt;

use crate::db::models::{Message, MessageSearchResult, Session};
use crate::error::AppError;
use crate::state::AppState;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageRequest {
    pub user_id: String,
    pub session_id: String,
    pub content: String,
    pub attachments: Vec<String>,
    pub task_type: Option<String>,
    pub qos: Option<String>,
    pub allow_background_defer: Option<bool>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageResponse {
    pub accepted: bool,
    pub session_id: String,
}

#[tauri::command]
pub async fn send_message(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    request: SendMessageRequest,
) -> Result<SendMessageResponse, AppError> {
    crate::log_info!("sarah.command", "send_message invoked");
    let mut stream = state
        .conversation
        .send_message(
            &request.user_id,
            &request.session_id,
            &request.content,
            &request.attachments,
            request.task_type.as_deref(),
            request.qos.as_deref(),
            request.allow_background_defer.unwrap_or(false),
            Some(app.clone()),
        )
        .await?;

    let session_id_clone = request.session_id.clone();
    tokio::spawn(async move {
        use tauri::Emitter;
        while let Some(chunk) = stream.next().await {
            let _ = app.emit("ai:token", serde_json::json!({
                "sessionId": chunk.session_id,
                "token": chunk.token,
                "done": chunk.done,
            }));
        }
        let _ = app.emit("ai:done", serde_json::json!({
            "sessionId": session_id_clone,
        }));
    });

    Ok(SendMessageResponse {
        accepted: true,
        session_id: request.session_id,
    })
}

#[tauri::command]
pub async fn create_session(
    state: State<'_, Arc<AppState>>,
    user_id: String,
    model_id: Option<String>,
) -> Result<Session, AppError> {
    crate::log_info!("sarah.command", "create_session invoked");
    state
        .conversation_repo
        .create_session(&user_id, model_id.as_deref())
        .await
}

#[tauri::command]
pub async fn list_sessions(
    state: State<'_, Arc<AppState>>,
    user_id: String,
    cursor: Option<String>,
    limit: Option<i64>,
) -> Result<Vec<Session>, AppError> {
    crate::log_info!("sarah.command", "list_sessions invoked");
    state
        .conversation_repo
        .list_sessions(&user_id, limit.unwrap_or(50).min(100), cursor.as_deref())
        .await
}

#[tauri::command]
pub async fn get_session_messages(
    state: State<'_, Arc<AppState>>,
    session_id: String,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<Message>, AppError> {
    crate::log_info!("sarah.command", "get_session_messages invoked");
    state
        .conversation_repo
        .get_messages(&session_id, limit.unwrap_or(200), offset.unwrap_or(0))
        .await
}

#[tauri::command]
pub async fn archive_session(
    state: State<'_, Arc<AppState>>,
    session_id: String,
) -> Result<(), AppError> {
    crate::log_info!("sarah.command", "archive_session invoked");
    state.conversation_repo.archive_session(&session_id).await
}

#[tauri::command]
pub async fn search_conversations(
    state: State<'_, Arc<AppState>>,
    user_id: String,
    query: String,
) -> Result<Vec<MessageSearchResult>, AppError> {
    crate::log_info!("sarah.command", "search_conversations invoked");
    state
        .conversation_repo
        .search_messages(&user_id, &query)
        .await
}
