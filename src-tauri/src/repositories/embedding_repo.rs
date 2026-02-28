use sqlx::{QueryBuilder, SqlitePool};
use uuid::Uuid;

use crate::db::models::EmbeddingRow;
use crate::error::AppError;
use crate::repositories::{blob_to_vector, vector_to_blob};

#[derive(Clone)]
pub struct EmbeddingRepo {
    read_pool: SqlitePool,
    write_pool: SqlitePool,
}

impl EmbeddingRepo {
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

    pub async fn upsert_embedding(
        &self,
        entity_type: &str,
        entity_id: &str,
        user_id: &str,
        namespace: &str,
        vector: Vec<f32>,
        model_name: &str,
    ) -> Result<String, AppError> {
        let id = Uuid::new_v4().to_string();
        let blob = vector_to_blob(&vector);
        let norm = Self::l2_norm(&vector);

        sqlx::query(
            r#"
            INSERT INTO embeddings (
                id, entity_type, entity_id, user_id, namespace, model_name, vector, dimensions, norm
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(entity_type, entity_id)
            DO UPDATE SET
                user_id = excluded.user_id,
                namespace = excluded.namespace,
                model_name = excluded.model_name,
                vector = excluded.vector,
                dimensions = excluded.dimensions,
                norm = excluded.norm
            "#,
        )
        .bind(&id)
        .bind(entity_type)
        .bind(entity_id)
        .bind(user_id)
        .bind(namespace)
        .bind(model_name)
        .bind(blob)
        .bind(vector.len() as i64)
        .bind(norm)
        .execute(&self.write_pool)
        .await?;

        let row = sqlx::query_scalar::<_, String>(
            "SELECT id FROM embeddings WHERE entity_type = ?1 AND entity_id = ?2",
        )
        .bind(entity_type)
        .bind(entity_id)
        .fetch_one(&self.read_pool)
        .await?;

        Ok(row)
    }

    pub async fn get_embedding(&self, entity_id: &str) -> Result<Option<Vec<f32>>, AppError> {
        let row =
            sqlx::query_scalar::<_, Vec<u8>>("SELECT vector FROM embeddings WHERE entity_id = ?1")
                .bind(entity_id)
                .fetch_optional(&self.read_pool)
                .await?;

        Ok(row.map(|blob| blob_to_vector(&blob)))
    }

    pub async fn get_embeddings_by_namespace(
        &self,
        namespace: &str,
        user_id: &str,
    ) -> Result<Vec<EmbeddingRow>, AppError> {
        let rows = sqlx::query_as::<_, EmbeddingRow>(
            "SELECT * FROM embeddings WHERE namespace = ?1 AND user_id = ?2",
        )
        .bind(namespace)
        .bind(user_id)
        .fetch_all(&self.read_pool)
        .await?;

        Ok(rows)
    }

    pub async fn get_embeddings_for_entities(
        &self,
        namespace: &str,
        user_id: &str,
        entity_type: &str,
        entity_ids: &[String],
    ) -> Result<Vec<EmbeddingRow>, AppError> {
        if entity_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut builder = QueryBuilder::new("SELECT * FROM embeddings WHERE namespace = ");
        builder
            .push_bind(namespace)
            .push(" AND user_id = ")
            .push_bind(user_id)
            .push(" AND entity_type = ")
            .push_bind(entity_type)
            .push(" AND entity_id IN (");

        let mut separated = builder.separated(", ");
        for entity_id in entity_ids {
            separated.push_bind(entity_id);
        }
        builder.push(")");

        let rows = builder
            .build_query_as::<EmbeddingRow>()
            .fetch_all(&self.read_pool)
            .await?;

        Ok(rows)
    }

    pub async fn delete_embedding_for_entity(
        &self,
        entity_type: &str,
        entity_id: &str,
    ) -> Result<(), AppError> {
        sqlx::query("DELETE FROM embeddings WHERE entity_type = ?1 AND entity_id = ?2")
            .bind(entity_type)
            .bind(entity_id)
            .execute(&self.write_pool)
            .await?;

        Ok(())
    }

    pub async fn count_embeddings_by_namespace(&self, namespace: &str) -> Result<i64, AppError> {
        let count =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM embeddings WHERE namespace = ?1")
                .bind(namespace)
                .fetch_one(&self.read_pool)
                .await?;

        Ok(count)
    }

    fn l2_norm(vector: &[f32]) -> f32 {
        vector.iter().map(|v| v * v).sum::<f32>().sqrt()
    }
}
