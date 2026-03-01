use sqlx::SqlitePool;
use uuid::Uuid;

use crate::db::models::{Mcp, McpHealthStatus, McpUsageStat};
use crate::error::AppError;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct McpSecret {
    pub id: String,
    pub mcp_id: String,
    pub user_id: String,
    pub key_name: String,
    pub encrypted_value: String,
    pub nonce: String,
    pub key_hint: Option<String>,
    pub expires_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone)]
pub struct McpRepo {
    read_pool: SqlitePool,
    write_pool: SqlitePool,
}

impl McpRepo {
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

    pub async fn list_mcps(&self, installed_only: bool) -> Result<Vec<Mcp>, AppError> {
        let sql = if installed_only {
            "SELECT * FROM mcps WHERE is_installed = 1 ORDER BY is_active DESC, display_name ASC"
        } else {
            "SELECT * FROM mcps ORDER BY is_active DESC, display_name ASC"
        };

        let rows = sqlx::query_as::<_, Mcp>(sql)
            .fetch_all(&self.read_pool)
            .await?;

        Ok(rows)
    }

    pub async fn get_mcp(&self, mcp_id: &str) -> Result<Option<Mcp>, AppError> {
        let row = sqlx::query_as::<_, Mcp>("SELECT * FROM mcps WHERE id = ?1")
            .bind(mcp_id)
            .fetch_optional(&self.read_pool)
            .await?;
        Ok(row)
    }

    pub async fn install_mcp(&self, mcp_id: &str) -> Result<(), AppError> {
        sqlx::query("UPDATE mcps SET is_installed = 1 WHERE id = ?1")
            .bind(mcp_id)
            .execute(&self.write_pool)
            .await?;
        Ok(())
    }

    pub async fn set_active(&self, mcp_id: &str, active: bool) -> Result<(), AppError> {
        sqlx::query("UPDATE mcps SET is_active = ?1 WHERE id = ?2")
            .bind(if active { 1 } else { 0 })
            .bind(mcp_id)
            .execute(&self.write_pool)
            .await?;
        Ok(())
    }

    pub async fn save_secret(
        &self,
        mcp_id: &str,
        user_id: &str,
        key_name: &str,
        encrypted_value: &str,
        nonce: &str,
        key_hint: Option<&str>,
    ) -> Result<(), AppError> {
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            r#"
            INSERT INTO mcp_secrets (id, mcp_id, user_id, key_name, encrypted_value, nonce, key_hint)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(mcp_id, user_id, key_name)
            DO UPDATE SET encrypted_value = excluded.encrypted_value, nonce = excluded.nonce, key_hint = excluded.key_hint
            "#,
        )
        .bind(&id)
        .bind(mcp_id)
        .bind(user_id)
        .bind(key_name)
        .bind(encrypted_value)
        .bind(nonce)
        .bind(key_hint)
        .execute(&self.write_pool)
        .await?;
        Ok(())
    }

    pub async fn get_secret(
        &self,
        mcp_id: &str,
        user_id: &str,
        key_name: &str,
    ) -> Result<Option<McpSecret>, AppError> {
        let row = sqlx::query_as::<_, McpSecret>(
            "SELECT * FROM mcp_secrets WHERE mcp_id = ?1 AND user_id = ?2 AND key_name = ?3",
        )
        .bind(mcp_id)
        .bind(user_id)
        .bind(key_name)
        .fetch_optional(&self.read_pool)
        .await?;

        Ok(row)
    }

    pub async fn update_health(
        &self,
        mcp_id: &str,
        status: &str,
        last_error: Option<&str>,
    ) -> Result<(), AppError> {
        sqlx::query(
            "UPDATE mcps SET health_status = ?1, last_error = ?2, last_health_check_at = datetime('now','utc') WHERE id = ?3",
        )
        .bind(status)
        .bind(last_error)
        .bind(mcp_id)
        .execute(&self.write_pool)
        .await?;
        Ok(())
    }

    pub async fn list_health_statuses(&self) -> Result<Vec<McpHealthStatus>, AppError> {
        let rows = sqlx::query_as::<_, McpHealthStatus>(
            "SELECT id AS mcp_id, health_status, last_error FROM mcps ORDER BY display_name ASC",
        )
        .fetch_all(&self.read_pool)
        .await?;
        Ok(rows)
    }

    pub async fn upsert_connection_state(
        &self,
        mcp_id: &str,
        user_id: &str,
        status: &str,
        circuit_breaker_state: &str,
        latency_ms: Option<f64>,
        success: bool,
    ) -> Result<(), AppError> {
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            r#"
            INSERT INTO mcp_connection_states (
                id, mcp_id, user_id, status, connected_at, last_used_at,
                error_count, success_count, avg_latency_ms, circuit_breaker_state
            ) VALUES (
                ?1, ?2, ?3, ?4,
                CASE WHEN ?4 = 'connected' THEN datetime('now','utc') ELSE NULL END,
                datetime('now','utc'),
                CASE WHEN ?6 = 1 THEN 0 ELSE 1 END,
                CASE WHEN ?6 = 1 THEN 1 ELSE 0 END,
                ?5,
                ?7
            )
            ON CONFLICT(mcp_id, user_id)
            DO UPDATE SET
                status = excluded.status,
                last_used_at = datetime('now','utc'),
                error_count = mcp_connection_states.error_count + CASE WHEN ?6 = 1 THEN 0 ELSE 1 END,
                success_count = mcp_connection_states.success_count + CASE WHEN ?6 = 1 THEN 1 ELSE 0 END,
                avg_latency_ms = CASE
                    WHEN excluded.avg_latency_ms IS NULL THEN mcp_connection_states.avg_latency_ms
                    WHEN mcp_connection_states.avg_latency_ms IS NULL THEN excluded.avg_latency_ms
                    ELSE (mcp_connection_states.avg_latency_ms + excluded.avg_latency_ms) / 2.0
                END,
                circuit_breaker_state = excluded.circuit_breaker_state
            "#,
        )
        .bind(&id)
        .bind(mcp_id)
        .bind(user_id)
        .bind(status)
        .bind(latency_ms)
        .bind(if success { 1 } else { 0 })
        .bind(circuit_breaker_state)
        .execute(&self.write_pool)
        .await?;

        Ok(())
    }

    pub async fn record_usage(
        &self,
        mcp_id: &str,
        user_id: &str,
        tool_name: &str,
        latency_ms: i64,
        success: bool,
    ) -> Result<(), AppError> {
        let id = Uuid::new_v4().to_string();

        sqlx::query(
            r#"
            INSERT INTO mcp_usage_stats (
                id, mcp_id, user_id, tool_name, call_count, success_count, error_count,
                total_latency_ms, last_called_at
            ) VALUES (
                ?1, ?2, ?3, ?4, 1,
                CASE WHEN ?6 = 1 THEN 1 ELSE 0 END,
                CASE WHEN ?6 = 1 THEN 0 ELSE 1 END,
                ?5,
                datetime('now','utc')
            )
            ON CONFLICT(mcp_id, user_id, tool_name)
            DO UPDATE SET
                call_count = mcp_usage_stats.call_count + 1,
                success_count = mcp_usage_stats.success_count + CASE WHEN ?6 = 1 THEN 1 ELSE 0 END,
                error_count = mcp_usage_stats.error_count + CASE WHEN ?6 = 1 THEN 0 ELSE 1 END,
                total_latency_ms = mcp_usage_stats.total_latency_ms + ?5,
                last_called_at = datetime('now','utc')
            "#,
        )
        .bind(&id)
        .bind(mcp_id)
        .bind(user_id)
        .bind(tool_name)
        .bind(latency_ms)
        .bind(if success { 1 } else { 0 })
        .execute(&self.write_pool)
        .await?;

        Ok(())
    }

    pub async fn get_usage_stats(&self, mcp_id: &str) -> Result<Vec<McpUsageStat>, AppError> {
        let rows = sqlx::query_as::<_, McpUsageStat>(
            "SELECT * FROM mcp_usage_stats WHERE mcp_id = ?1 ORDER BY call_count DESC",
        )
        .bind(mcp_id)
        .fetch_all(&self.read_pool)
        .await?;

        Ok(rows)
    }
}
