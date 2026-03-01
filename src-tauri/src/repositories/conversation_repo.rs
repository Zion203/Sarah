use sqlx::SqlitePool;
use uuid::Uuid;

use crate::db::models::{Message, MessageSearchResult, NewMessage, NewToolCall, Session, ToolCall};
use crate::error::AppError;

#[derive(Clone)]
pub struct ConversationRepo {
    read_pool: SqlitePool,
    write_pool: SqlitePool,
}

impl ConversationRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            read_pool: pool.clone(),
            write_pool: pool,
        }
    }

    pub fn with_pools(read_pool: SqlitePool, write_pool: SqlitePool) -> Self {
        Self {
            read_pool,
            write_pool,
        }
    }

    pub async fn create_session(
        &self,
        user_id: &str,
        model_id: Option<&str>,
    ) -> Result<Session, AppError> {
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO sessions (id, user_id, model_id, status) VALUES (?1, ?2, ?3, 'active')",
        )
        .bind(&id)
        .bind(user_id)
        .bind(model_id)
        .execute(&self.write_pool)
        .await?;

        self.get_session(&id)
            .await?
            .ok_or_else(|| AppError::NotFound {
                entity: "session".to_string(),
                id,
            })
    }

    pub async fn get_session(&self, id: &str) -> Result<Option<Session>, AppError> {
        let row = sqlx::query_as::<_, Session>("SELECT * FROM sessions WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.read_pool)
            .await?;
        Ok(row)
    }

    pub async fn list_sessions(
        &self,
        user_id: &str,
        limit: i64,
        cursor: Option<&str>,
    ) -> Result<Vec<Session>, AppError> {
        let rows = if let Some(cursor_id) = cursor {
            sqlx::query_as::<_, Session>(
                r#"
                SELECT * FROM sessions
                WHERE user_id = ?1 AND status != 'deleted'
                  AND datetime(created_at) < (
                    SELECT datetime(created_at) FROM sessions WHERE id = ?2
                  )
                ORDER BY datetime(last_message_at) DESC, datetime(created_at) DESC
                LIMIT ?3
                "#,
            )
            .bind(user_id)
            .bind(cursor_id)
            .bind(limit)
            .fetch_all(&self.read_pool)
            .await?
        } else {
            sqlx::query_as::<_, Session>(
                r#"
                SELECT * FROM sessions
                WHERE user_id = ?1 AND status != 'deleted'
                ORDER BY datetime(last_message_at) DESC, datetime(created_at) DESC
                LIMIT ?2
                "#,
            )
            .bind(user_id)
            .bind(limit)
            .fetch_all(&self.read_pool)
            .await?
        };

        Ok(rows)
    }

    pub async fn update_session_title(&self, id: &str, title: &str) -> Result<(), AppError> {
        sqlx::query("UPDATE sessions SET title = ?1 WHERE id = ?2")
            .bind(title)
            .bind(id)
            .execute(&self.write_pool)
            .await?;
        Ok(())
    }

    pub async fn archive_session(&self, id: &str) -> Result<(), AppError> {
        sqlx::query("UPDATE sessions SET status = 'archived' WHERE id = ?1")
            .bind(id)
            .execute(&self.write_pool)
            .await?;
        Ok(())
    }

    pub async fn insert_message(&self, msg: NewMessage) -> Result<Message, AppError> {
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            r#"
            INSERT INTO messages (
              id, session_id, role, content, content_type, token_count, model_id, metadata, position
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
        )
        .bind(&id)
        .bind(&msg.session_id)
        .bind(&msg.role)
        .bind(&msg.content)
        .bind(&msg.content_type)
        .bind(msg.token_count)
        .bind(&msg.model_id)
        .bind(&msg.metadata)
        .bind(msg.position)
        .execute(&self.write_pool)
        .await?;

        sqlx::query(
            r#"
            UPDATE sessions
            SET message_count = message_count + 1,
                token_count = token_count + COALESCE(?1, 0),
                last_message_at = datetime('now','utc')
            WHERE id = ?2
            "#,
        )
        .bind(msg.token_count)
        .bind(&msg.session_id)
        .execute(&self.write_pool)
        .await?;

        self.get_message_by_id(&id)
            .await?
            .ok_or_else(|| AppError::NotFound {
                entity: "message".to_string(),
                id,
            })
    }

    pub async fn get_message_by_id(&self, id: &str) -> Result<Option<Message>, AppError> {
        let row = sqlx::query_as::<_, Message>("SELECT * FROM messages WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.read_pool)
            .await?;
        Ok(row)
    }

    pub async fn get_messages(
        &self,
        session_id: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Message>, AppError> {
        let rows = sqlx::query_as::<_, Message>(
            "SELECT * FROM messages WHERE session_id = ?1 ORDER BY position ASC LIMIT ?2 OFFSET ?3",
        )
        .bind(session_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.read_pool)
        .await?;

        Ok(rows)
    }

    pub async fn get_context_window(
        &self,
        session_id: &str,
        max_tokens: i64,
    ) -> Result<Vec<Message>, AppError> {
        let mut rows = sqlx::query_as::<_, Message>(
            "SELECT * FROM messages WHERE session_id = ?1 ORDER BY position DESC",
        )
        .bind(session_id)
        .fetch_all(&self.read_pool)
        .await?;

        let mut running_tokens: i64 = 0;
        let mut selected = Vec::new();

        for message in rows.drain(..) {
            let tokens = message
                .token_count
                .unwrap_or((message.content.len() / 4) as i64 + 1);
            if running_tokens + tokens > max_tokens {
                break;
            }
            running_tokens += tokens;
            selected.push(message);
        }

        selected.reverse();
        Ok(selected)
    }

    pub async fn update_session_summary(
        &self,
        session_id: &str,
        summary: &str,
    ) -> Result<(), AppError> {
        sqlx::query("UPDATE sessions SET summary = ?1 WHERE id = ?2")
            .bind(summary)
            .bind(session_id)
            .execute(&self.write_pool)
            .await?;
        Ok(())
    }

    pub async fn insert_tool_call(&self, call: NewToolCall) -> Result<ToolCall, AppError> {
        let id = Uuid::new_v4().to_string();

        sqlx::query(
            r#"
            INSERT INTO tool_calls (id, message_id, session_id, mcp_id, tool_name, tool_input, status)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'pending')
            "#,
        )
        .bind(&id)
        .bind(&call.message_id)
        .bind(&call.session_id)
        .bind(&call.mcp_id)
        .bind(&call.tool_name)
        .bind(&call.tool_input)
        .execute(&self.write_pool)
        .await?;

        self.get_tool_call(&id)
            .await?
            .ok_or_else(|| AppError::NotFound {
                entity: "tool_call".to_string(),
                id,
            })
    }

    pub async fn get_tool_call(&self, id: &str) -> Result<Option<ToolCall>, AppError> {
        let row = sqlx::query_as::<_, ToolCall>("SELECT * FROM tool_calls WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.read_pool)
            .await?;
        Ok(row)
    }

    pub async fn update_tool_call_result(
        &self,
        id: &str,
        output: Option<&str>,
        status: &str,
        latency_ms: i64,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"
            UPDATE tool_calls
            SET tool_output = ?1,
                status = ?2,
                latency_ms = ?3,
                completed_at = datetime('now','utc')
            WHERE id = ?4
            "#,
        )
        .bind(output)
        .bind(status)
        .bind(latency_ms)
        .bind(id)
        .execute(&self.write_pool)
        .await?;

        Ok(())
    }

    pub async fn search_messages(
        &self,
        user_id: &str,
        query: &str,
    ) -> Result<Vec<MessageSearchResult>, AppError> {
        let rows = sqlx::query_as::<_, MessageSearchResult>(
            r#"
            SELECT m.id, m.session_id, m.role, m.content, m.position, m.created_at
            FROM messages_fts f
            JOIN messages m ON m.id = f.message_id
            JOIN sessions s ON s.id = m.session_id
            WHERE s.user_id = ?1
              AND messages_fts MATCH ?2
            ORDER BY rank
            LIMIT 50
            "#,
        )
        .bind(user_id)
        .bind(query)
        .fetch_all(&self.read_pool)
        .await?;

        Ok(rows)
    }
}
