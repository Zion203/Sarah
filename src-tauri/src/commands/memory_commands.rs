use std::sync::Arc;

use tauri::State;

use crate::db::models::{Memory, MemoryGraph};
use crate::error::AppError;
use crate::state::AppState;

#[tauri::command]
pub async fn get_memories(
    state: State<'_, Arc<AppState>>,
    user_id: String,
    memory_type: Option<String>,
    limit: Option<i64>,
) -> Result<Vec<Memory>, AppError> {
    crate::log_info!("sarah.command", "get_memories invoked");
    state
        .memory_repo
        .get_memories(&user_id, memory_type.as_deref(), limit.unwrap_or(100))
        .await
}

#[tauri::command]
pub async fn search_memories(
    state: State<'_, Arc<AppState>>,
    user_id: String,
    query: String,
) -> Result<Vec<Memory>, AppError> {
    crate::log_info!("sarah.command", "search_memories invoked");
    state
        .memory_repo
        .search_memories_text(&user_id, &query, 100)
        .await
}

#[tauri::command]
pub async fn delete_memory(
    state: State<'_, Arc<AppState>>,
    memory_id: String,
) -> Result<(), AppError> {
    crate::log_info!("sarah.command", "delete_memory invoked");
    state.memory_repo.delete_memory(&memory_id).await
}

#[tauri::command]
pub async fn pin_memory(
    state: State<'_, Arc<AppState>>,
    memory_id: String,
    pinned: bool,
) -> Result<(), AppError> {
    crate::log_info!("sarah.command", "pin_memory invoked");
    sqlx::query("UPDATE memories SET is_pinned = ?1 WHERE id = ?2")
        .bind(if pinned { 1 } else { 0 })
        .bind(&memory_id)
        .execute(state.db.write_pool())
        .await?;

    Ok(())
}

#[tauri::command]
pub async fn update_memory(
    state: State<'_, Arc<AppState>>,
    memory_id: String,
    content: String,
) -> Result<Memory, AppError> {
    crate::log_info!("sarah.command", "update_memory invoked");
    sqlx::query("UPDATE memories SET content = ?1 WHERE id = ?2")
        .bind(&content)
        .bind(&memory_id)
        .execute(state.db.write_pool())
        .await?;

    state
        .memory_repo
        .get_memory(&memory_id)
        .await?
        .ok_or_else(|| AppError::NotFound {
            entity: "memory".to_string(),
            id: memory_id,
        })
}

#[tauri::command]
pub async fn get_memory_graph(
    state: State<'_, Arc<AppState>>,
    user_id: String,
    memory_id: String,
    depth: Option<i64>,
) -> Result<MemoryGraph, AppError> {
    crate::log_info!("sarah.command", "get_memory_graph invoked");
    state
        .memory
        .get_memory_graph(&user_id, &memory_id, depth.unwrap_or(2))
        .await
}
