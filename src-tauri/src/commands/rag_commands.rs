use std::sync::Arc;

use tauri::State;

use crate::db::models::RetrievedChunk;
use crate::error::AppError;
use crate::services::background_service::BackgroundTask;
use crate::services::rag_service::RagService;
use crate::state::AppState;

fn get_rag(state: &Arc<AppState>) -> Result<&Arc<RagService>, AppError> {
    state.rag.as_ref().ok_or_else(|| AppError::Validation {
        field: "rag".to_string(),
        message: "RAG service is not available".to_string(),
    })
}

#[tauri::command]
pub async fn ingest_document(
    state: State<'_, Arc<AppState>>,
    user_id: String,
    file_path: String,
) -> Result<String, AppError> {
    crate::log_info!("sarah.command", "ingest_document invoked");
    let rag = get_rag(&state)?;

    let document_id = rag.ingest_document(&user_id, &file_path).await?;
    let _ = state
        .background
        .sender()
        .send(BackgroundTask::EmbedDocument(document_id.clone()));
    Ok(document_id)
}

#[tauri::command]
pub async fn embed_document(
    state: State<'_, Arc<AppState>>,
    document_id: String,
) -> Result<(), AppError> {
    crate::log_info!("sarah.command", "embed_document invoked");
    let rag = get_rag(&state)?;
    rag.embed_document_chunks(&document_id).await
}

#[tauri::command]
pub async fn retrieve_knowledge(
    state: State<'_, Arc<AppState>>,
    user_id: String,
    query: String,
    namespace: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<RetrievedChunk>, AppError> {
    crate::log_info!("sarah.command", "retrieve_knowledge invoked");
    let rag = get_rag(&state)?;

    rag.retrieve(
        &user_id,
        &query,
        namespace.as_deref().unwrap_or("personal"),
        limit.unwrap_or(6),
    )
    .await
}
