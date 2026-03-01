use sqlx::SqlitePool;
use uuid::Uuid;

use crate::db::models::SetupState;
use crate::error::AppError;

#[derive(Clone)]
pub struct SetupOrchestratorService {
    read_pool: SqlitePool,
    write_pool: SqlitePool,
}

impl SetupOrchestratorService {
    pub fn new(read_pool: SqlitePool, write_pool: SqlitePool) -> Self {
        Self {
            read_pool,
            write_pool,
        }
    }

    pub async fn get_state(&self, user_id: Option<&str>) -> Result<Option<SetupState>, AppError> {
        let row = if let Some(uid) = user_id {
            sqlx::query_as::<_, SetupState>("SELECT * FROM setup_state WHERE user_id = ?1 LIMIT 1")
                .bind(uid)
                .fetch_optional(&self.read_pool)
                .await?
        } else {
            sqlx::query_as::<_, SetupState>(
                "SELECT * FROM setup_state WHERE user_id IS NULL LIMIT 1",
            )
            .fetch_optional(&self.read_pool)
            .await?
        };
        Ok(row)
    }

    pub async fn start_or_resume(
        &self,
        user_id: Option<&str>,
        selected_bundle: Option<&str>,
        hardware_profile_id: Option<&str>,
    ) -> Result<SetupState, AppError> {
        self.upsert_state(
            user_id,
            "running",
            "stage_a_preflight",
            10.0,
            selected_bundle,
            hardware_profile_id,
            None,
            Some(r#"{"autoUpgrade":"pending"}"#),
        )
        .await
    }

    pub async fn update_stage(
        &self,
        user_id: Option<&str>,
        stage: &str,
        progress_pct: f64,
    ) -> Result<SetupState, AppError> {
        self.upsert_state(
            user_id,
            "running",
            stage,
            progress_pct,
            None,
            None,
            None,
            None,
        )
        .await
    }

    pub async fn mark_completed(&self, user_id: Option<&str>) -> Result<SetupState, AppError> {
        self.upsert_state(
            user_id,
            "completed",
            "stage_d_background_upgrade_queued",
            100.0,
            None,
            None,
            None,
            Some(r#"{"autoUpgrade":"queued"}"#),
        )
        .await
    }

    pub async fn mark_failed(
        &self,
        user_id: Option<&str>,
        stage: &str,
        error: &str,
    ) -> Result<SetupState, AppError> {
        self.upsert_state(user_id, "failed", stage, 0.0, None, None, Some(error), None)
            .await
    }

    pub async fn skip_quality_upgrade(
        &self,
        user_id: Option<&str>,
    ) -> Result<SetupState, AppError> {
        self.upsert_state(
            user_id,
            "completed",
            "stage_d_skipped",
            100.0,
            None,
            None,
            None,
            Some(r#"{"autoUpgrade":"skipped"}"#),
        )
        .await
    }

    pub async fn retry_stage(
        &self,
        user_id: Option<&str>,
        stage: &str,
    ) -> Result<SetupState, AppError> {
        self.upsert_state(user_id, "running", stage, 15.0, None, None, None, None)
            .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn upsert_state(
        &self,
        user_id: Option<&str>,
        status: &str,
        stage: &str,
        progress_pct: f64,
        selected_bundle: Option<&str>,
        hardware_profile_id: Option<&str>,
        last_error: Option<&str>,
        metadata: Option<&str>,
    ) -> Result<SetupState, AppError> {
        let existing = self.get_state(user_id).await?;

        if let Some(row) = existing {
            sqlx::query(
                r#"
                UPDATE setup_state
                SET status = ?1,
                    current_stage = ?2,
                    progress_pct = ?3,
                    selected_bundle = COALESCE(?4, selected_bundle),
                    hardware_profile_id = COALESCE(?5, hardware_profile_id),
                    last_error = ?6,
                    metadata = COALESCE(?7, metadata)
                WHERE id = ?8
                "#,
            )
            .bind(status)
            .bind(stage)
            .bind(progress_pct)
            .bind(selected_bundle)
            .bind(hardware_profile_id)
            .bind(last_error)
            .bind(metadata)
            .bind(&row.id)
            .execute(&self.write_pool)
            .await?;

            return self
                .get_state(user_id)
                .await?
                .ok_or_else(|| AppError::NotFound {
                    entity: "setup_state".to_string(),
                    id: row.id,
                });
        }

        let id = Uuid::new_v4().to_string();
        sqlx::query(
            r#"
            INSERT INTO setup_state (
              id, user_id, status, current_stage, progress_pct, selected_bundle,
              hardware_profile_id, last_error, metadata
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
        )
        .bind(&id)
        .bind(user_id)
        .bind(status)
        .bind(stage)
        .bind(progress_pct)
        .bind(selected_bundle)
        .bind(hardware_profile_id)
        .bind(last_error)
        .bind(metadata.unwrap_or("{}"))
        .execute(&self.write_pool)
        .await?;

        self.get_state(user_id)
            .await?
            .ok_or_else(|| AppError::NotFound {
                entity: "setup_state".to_string(),
                id,
            })
    }
}
