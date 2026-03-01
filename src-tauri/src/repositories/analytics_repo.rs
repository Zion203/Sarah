use sqlx::SqlitePool;
use uuid::Uuid;

use crate::db::models::{ModelRecommendation, PerfLog};
use crate::error::AppError;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewPerfLog {
    pub event_type: String,
    pub session_id: Option<String>,
    pub model_id: Option<String>,
    pub mcp_id: Option<String>,
    pub latency_ms: i64,
    pub tokens_in: Option<i64>,
    pub tokens_out: Option<i64>,
    pub tokens_per_sec: Option<f64>,
    pub cpu_usage_pct: Option<f64>,
    pub ram_usage_mb: Option<i64>,
    pub gpu_usage_pct: Option<f64>,
    pub success: bool,
    pub error_code: Option<String>,
    pub metadata: Option<String>,
}

#[derive(Clone)]
pub struct AnalyticsRepo {
    read_pool: SqlitePool,
    write_pool: SqlitePool,
}

impl AnalyticsRepo {
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

    pub async fn insert_perf_log(&self, entry: NewPerfLog) -> Result<(), AppError> {
        sqlx::query(
            r#"
            INSERT INTO perf_logs (
              id, event_type, session_id, model_id, mcp_id, latency_ms,
              tokens_in, tokens_out, tokens_per_sec, cpu_usage_pct, ram_usage_mb,
              gpu_usage_pct, success, error_code, metadata
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&entry.event_type)
        .bind(&entry.session_id)
        .bind(&entry.model_id)
        .bind(&entry.mcp_id)
        .bind(entry.latency_ms)
        .bind(entry.tokens_in)
        .bind(entry.tokens_out)
        .bind(entry.tokens_per_sec)
        .bind(entry.cpu_usage_pct)
        .bind(entry.ram_usage_mb)
        .bind(entry.gpu_usage_pct)
        .bind(if entry.success { 1 } else { 0 })
        .bind(&entry.error_code)
        .bind(&entry.metadata)
        .execute(&self.write_pool)
        .await?;

        Ok(())
    }

    pub async fn insert_error_report(
        &self,
        error_code: &str,
        message: &str,
        component: &str,
        severity: &str,
        metadata: Option<&str>,
    ) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO error_reports (id, error_code, error_message, component, severity, metadata) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(error_code)
        .bind(message)
        .bind(component)
        .bind(severity)
        .bind(metadata)
        .execute(&self.write_pool)
        .await?;

        Ok(())
    }

    pub async fn get_recent_perf_logs(&self, limit: i64) -> Result<Vec<PerfLog>, AppError> {
        let rows = sqlx::query_as::<_, PerfLog>(
            "SELECT * FROM perf_logs ORDER BY datetime(created_at) DESC LIMIT ?1",
        )
        .bind(limit)
        .fetch_all(&self.read_pool)
        .await?;
        Ok(rows)
    }

    pub async fn replace_recommendations(
        &self,
        profile_id: &str,
        recommendations: &[ModelRecommendation],
    ) -> Result<(), AppError> {
        let mut tx = self.write_pool.begin().await?;

        sqlx::query("DELETE FROM model_recommendations WHERE system_profile_id = ?1")
            .bind(profile_id)
            .execute(&mut *tx)
            .await?;

        for rec in recommendations {
            sqlx::query(
                r#"
                INSERT INTO model_recommendations (
                  id, system_profile_id, model_id, recommendation_tier, score, reasoning,
                  performance_estimate, energy_rating, is_primary_recommendation, computed_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                "#,
            )
            .bind(&rec.id)
            .bind(&rec.system_profile_id)
            .bind(&rec.model_id)
            .bind(&rec.recommendation_tier)
            .bind(rec.score)
            .bind(&rec.reasoning)
            .bind(&rec.performance_estimate)
            .bind(&rec.energy_rating)
            .bind(rec.is_primary_recommendation)
            .bind(&rec.computed_at)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    pub async fn get_recommendations(
        &self,
        profile_id: &str,
    ) -> Result<Vec<ModelRecommendation>, AppError> {
        let rows = sqlx::query_as::<_, ModelRecommendation>(
            "SELECT * FROM model_recommendations WHERE system_profile_id = ?1 ORDER BY score DESC",
        )
        .bind(profile_id)
        .fetch_all(&self.read_pool)
        .await?;

        Ok(rows)
    }

    pub async fn prune_old_perf_logs(&self, days: i64) -> Result<u64, AppError> {
        let result = sqlx::query(
            "DELETE FROM perf_logs WHERE datetime(created_at) < datetime('now', '-' || ?1 || ' day')",
        )
        .bind(days)
        .execute(&self.write_pool)
        .await?;

        Ok(result.rows_affected())
    }
}
