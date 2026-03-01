use sqlx::SqlitePool;
use uuid::Uuid;

use crate::db::models::{Model, ModelWithScore, NewModel};
use crate::error::AppError;

#[derive(Clone)]
pub struct ModelRepo {
    read_pool: SqlitePool,
    write_pool: SqlitePool,
}

impl ModelRepo {
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

    pub async fn insert_model(&self, model: NewModel) -> Result<Model, AppError> {
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            r#"
            INSERT INTO models (
              id, name, display_name, family, version, parameter_count, quantization,
              file_format, file_path, file_size_mb, context_length, embedding_size, category,
              capabilities, min_ram_mb, recommended_ram_mb, min_vram_mb, performance_tier,
              energy_tier, download_url, sha256_checksum, tags, metadata
            ) VALUES (
              ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17,
              ?18, ?19, ?20, ?21, ?22, ?23
            )
            "#,
        )
        .bind(&id)
        .bind(&model.name)
        .bind(&model.display_name)
        .bind(&model.family)
        .bind(&model.version)
        .bind(&model.parameter_count)
        .bind(&model.quantization)
        .bind(&model.file_format)
        .bind(&model.file_path)
        .bind(model.file_size_mb)
        .bind(model.context_length)
        .bind(model.embedding_size)
        .bind(&model.category)
        .bind(&model.capabilities)
        .bind(model.min_ram_mb)
        .bind(model.recommended_ram_mb)
        .bind(model.min_vram_mb)
        .bind(&model.performance_tier)
        .bind(&model.energy_tier)
        .bind(&model.download_url)
        .bind(&model.sha256_checksum)
        .bind(&model.tags)
        .bind(&model.metadata)
        .execute(&self.write_pool)
        .await?;

        self.get_by_id(&id)
            .await?
            .ok_or_else(|| AppError::NotFound {
                entity: "model".to_string(),
                id,
            })
    }

    pub async fn upsert_model(&self, model: NewModel) -> Result<Model, AppError> {
        sqlx::query(
            r#"
            INSERT INTO models (
              id, name, display_name, family, version, parameter_count, quantization,
              file_format, file_path, file_size_mb, context_length, embedding_size, category,
              capabilities, min_ram_mb, recommended_ram_mb, min_vram_mb, performance_tier,
              energy_tier, download_url, sha256_checksum, tags, metadata, is_downloaded
            ) VALUES (
              ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17,
              ?18, ?19, ?20, ?21, ?22, ?23, CASE WHEN ?9 IS NULL THEN 0 ELSE 1 END
            )
            ON CONFLICT(name) DO UPDATE SET
              display_name = excluded.display_name,
              family = excluded.family,
              version = excluded.version,
              parameter_count = excluded.parameter_count,
              quantization = excluded.quantization,
              file_format = excluded.file_format,
              file_path = excluded.file_path,
              file_size_mb = excluded.file_size_mb,
              context_length = excluded.context_length,
              embedding_size = excluded.embedding_size,
              category = excluded.category,
              capabilities = excluded.capabilities,
              min_ram_mb = excluded.min_ram_mb,
              recommended_ram_mb = excluded.recommended_ram_mb,
              min_vram_mb = excluded.min_vram_mb,
              performance_tier = excluded.performance_tier,
              energy_tier = excluded.energy_tier,
              download_url = excluded.download_url,
              sha256_checksum = excluded.sha256_checksum,
              tags = excluded.tags,
              metadata = excluded.metadata,
              is_downloaded = CASE WHEN excluded.file_path IS NULL THEN models.is_downloaded ELSE 1 END
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&model.name)
        .bind(&model.display_name)
        .bind(&model.family)
        .bind(&model.version)
        .bind(&model.parameter_count)
        .bind(&model.quantization)
        .bind(&model.file_format)
        .bind(&model.file_path)
        .bind(model.file_size_mb)
        .bind(model.context_length)
        .bind(model.embedding_size)
        .bind(&model.category)
        .bind(&model.capabilities)
        .bind(model.min_ram_mb)
        .bind(model.recommended_ram_mb)
        .bind(model.min_vram_mb)
        .bind(&model.performance_tier)
        .bind(&model.energy_tier)
        .bind(&model.download_url)
        .bind(&model.sha256_checksum)
        .bind(&model.tags)
        .bind(&model.metadata)
        .execute(&self.write_pool)
        .await?;

        self.get_by_name(&model.name)
            .await?
            .ok_or_else(|| AppError::NotFound {
                entity: "model".to_string(),
                id: model.name,
            })
    }

    pub async fn get_by_id(&self, id: &str) -> Result<Option<Model>, AppError> {
        let row = sqlx::query_as::<_, Model>("SELECT * FROM models WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.read_pool)
            .await?;
        Ok(row)
    }

    pub async fn get_by_name(&self, name: &str) -> Result<Option<Model>, AppError> {
        let row = sqlx::query_as::<_, Model>("SELECT * FROM models WHERE name = ?1")
            .bind(name)
            .fetch_optional(&self.read_pool)
            .await?;
        Ok(row)
    }

    pub async fn list_by_category(
        &self,
        category: &str,
        downloaded_only: bool,
    ) -> Result<Vec<Model>, AppError> {
        let sql = if downloaded_only {
            "SELECT * FROM models WHERE category = ?1 AND is_downloaded = 1 ORDER BY is_default DESC, compatibility_score DESC"
        } else {
            "SELECT * FROM models WHERE category = ?1 ORDER BY is_default DESC, compatibility_score DESC"
        };

        let rows = sqlx::query_as::<_, Model>(sql)
            .bind(category)
            .fetch_all(&self.read_pool)
            .await?;

        Ok(rows)
    }

    pub async fn list_compatible_models(
        &self,
        min_ram_mb: i64,
        max_vram_mb: i64,
    ) -> Result<Vec<Model>, AppError> {
        let rows = sqlx::query_as::<_, Model>(
            r#"
            SELECT * FROM models
            WHERE min_ram_mb <= ?1 AND min_vram_mb <= ?2
            ORDER BY compatibility_score DESC, is_recommended DESC
            "#,
        )
        .bind(min_ram_mb)
        .bind(max_vram_mb)
        .fetch_all(&self.read_pool)
        .await?;

        Ok(rows)
    }

    pub async fn set_default_model(&self, id: &str) -> Result<(), AppError> {
        let mut tx = self.write_pool.begin().await?;

        sqlx::query("UPDATE models SET is_default = 0")
            .execute(&mut *tx)
            .await?;

        sqlx::query("UPDATE models SET is_default = 1, is_active = 1, last_used_at = datetime('now','utc') WHERE id = ?1")
            .bind(id)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(())
    }

    pub async fn update_performance_metrics(
        &self,
        id: &str,
        tokens_per_sec: f64,
    ) -> Result<(), AppError> {
        sqlx::query(
            "UPDATE models SET avg_tokens_per_sec = ?1, last_used_at = datetime('now','utc') WHERE id = ?2",
        )
        .bind(tokens_per_sec)
        .bind(id)
        .execute(&self.write_pool)
        .await?;
        Ok(())
    }

    pub async fn get_recommended_models(
        &self,
        profile_id: &str,
    ) -> Result<Vec<ModelWithScore>, AppError> {
        let rows = sqlx::query_as::<_, ModelWithScore>(
            r#"
            SELECT m.id, m.name, m.display_name, r.recommendation_tier, r.score, r.reasoning
            FROM model_recommendations r
            JOIN models m ON m.id = r.model_id
            WHERE r.system_profile_id = ?1
            ORDER BY r.score DESC
            "#,
        )
        .bind(profile_id)
        .fetch_all(&self.read_pool)
        .await?;

        Ok(rows)
    }

    pub async fn list_installed(&self) -> Result<Vec<Model>, AppError> {
        let rows = sqlx::query_as::<_, Model>(
            "SELECT * FROM models WHERE is_downloaded = 1 ORDER BY is_default DESC, display_name ASC",
        )
        .fetch_all(&self.read_pool)
        .await?;
        Ok(rows)
    }

    pub async fn list_all(&self) -> Result<Vec<Model>, AppError> {
        let rows = sqlx::query_as::<_, Model>(
            "SELECT * FROM models ORDER BY is_downloaded DESC, is_default DESC, display_name ASC",
        )
        .fetch_all(&self.read_pool)
        .await?;
        Ok(rows)
    }
}
