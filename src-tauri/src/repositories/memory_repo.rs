use sqlx::SqlitePool;
use uuid::Uuid;

use crate::db::models::{Memory, MemoryGraph, MemoryRelation, NewMemory};
use crate::error::AppError;

#[derive(Clone)]
pub struct MemoryRepo {
    read_pool: SqlitePool,
    write_pool: SqlitePool,
}

impl MemoryRepo {
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

    pub async fn upsert_memory(&self, memory: NewMemory) -> Result<Memory, AppError> {
        if let Some(existing) = sqlx::query_as::<_, Memory>(
            "SELECT * FROM memories WHERE user_id = ?1 AND content = ?2 LIMIT 1",
        )
        .bind(&memory.user_id)
        .bind(&memory.content)
        .fetch_optional(&self.read_pool)
        .await?
        {
            return Ok(existing);
        }

        let id = Uuid::new_v4().to_string();
        sqlx::query(
            r#"
            INSERT INTO memories (
              id, user_id, memory_type, category, subject, predicate, object, content,
              summary, source, source_id, session_id, confidence, importance, decay_rate,
              privacy_level, tags, metadata
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)
            "#,
        )
        .bind(&id)
        .bind(&memory.user_id)
        .bind(&memory.memory_type)
        .bind(&memory.category)
        .bind(&memory.subject)
        .bind(&memory.predicate)
        .bind(&memory.object)
        .bind(&memory.content)
        .bind(&memory.summary)
        .bind(&memory.source)
        .bind(&memory.source_id)
        .bind(&memory.session_id)
        .bind(memory.confidence)
        .bind(memory.importance)
        .bind(memory.decay_rate)
        .bind(&memory.privacy_level)
        .bind(&memory.tags)
        .bind(&memory.metadata)
        .execute(&self.write_pool)
        .await?;

        self.get_memory(&id)
            .await?
            .ok_or_else(|| AppError::NotFound {
                entity: "memory".to_string(),
                id,
            })
    }

    pub async fn get_memory(&self, id: &str) -> Result<Option<Memory>, AppError> {
        let row = sqlx::query_as::<_, Memory>("SELECT * FROM memories WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.read_pool)
            .await?;
        Ok(row)
    }

    pub async fn get_memories(
        &self,
        user_id: &str,
        memory_type: Option<&str>,
        limit: i64,
    ) -> Result<Vec<Memory>, AppError> {
        let rows = if let Some(kind) = memory_type {
            sqlx::query_as::<_, Memory>(
                "SELECT * FROM memories WHERE user_id = ?1 AND memory_type = ?2 AND is_archived = 0 ORDER BY importance DESC LIMIT ?3",
            )
            .bind(user_id)
            .bind(kind)
            .bind(limit)
            .fetch_all(&self.read_pool)
            .await?
        } else {
            sqlx::query_as::<_, Memory>(
                "SELECT * FROM memories WHERE user_id = ?1 AND is_archived = 0 ORDER BY importance DESC LIMIT ?2",
            )
            .bind(user_id)
            .bind(limit)
            .fetch_all(&self.read_pool)
            .await?
        };

        Ok(rows)
    }

    pub async fn search_memories_text(
        &self,
        user_id: &str,
        query: &str,
        limit: i64,
    ) -> Result<Vec<Memory>, AppError> {
        let rows = sqlx::query_as::<_, Memory>(
            r#"
            SELECT m.*
            FROM memories_fts f
            JOIN memories m ON m.id = f.memory_id
            WHERE m.user_id = ?1 AND memories_fts MATCH ?2
            ORDER BY rank
            LIMIT ?3
            "#,
        )
        .bind(user_id)
        .bind(query)
        .bind(limit)
        .fetch_all(&self.read_pool)
        .await?;

        Ok(rows)
    }

    pub async fn get_memories_by_importance(
        &self,
        user_id: &str,
        min_importance: f64,
    ) -> Result<Vec<Memory>, AppError> {
        let rows = sqlx::query_as::<_, Memory>(
            "SELECT * FROM memories WHERE user_id = ?1 AND importance >= ?2 ORDER BY importance DESC",
        )
        .bind(user_id)
        .bind(min_importance)
        .fetch_all(&self.read_pool)
        .await?;
        Ok(rows)
    }

    pub async fn update_access_count(&self, id: &str) -> Result<(), AppError> {
        sqlx::query(
            "UPDATE memories SET access_count = access_count + 1, last_accessed_at = datetime('now','utc') WHERE id = ?1",
        )
        .bind(id)
        .execute(&self.write_pool)
        .await?;
        Ok(())
    }

    pub async fn apply_time_decay(&self, user_id: &str) -> Result<u64, AppError> {
        let result = sqlx::query(
            r#"
            UPDATE memories
            SET importance = MAX(
                0.0,
                importance * (
                    1 - (decay_rate * COALESCE((julianday('now') - julianday(last_accessed_at)), (julianday('now') - julianday(created_at))))
                )
            )
            WHERE user_id = ?1
            "#,
        )
        .bind(user_id)
        .execute(&self.write_pool)
        .await?;

        Ok(result.rows_affected())
    }

    pub async fn delete_memory(&self, id: &str) -> Result<(), AppError> {
        sqlx::query("DELETE FROM memories WHERE id = ?1")
            .bind(id)
            .execute(&self.write_pool)
            .await?;
        Ok(())
    }

    pub async fn get_memory_graph(
        &self,
        user_id: &str,
        memory_id: &str,
        depth: i64,
    ) -> Result<MemoryGraph, AppError> {
        let nodes = sqlx::query_as::<_, Memory>(
            r#"
            WITH RECURSIVE graph(memory_id, level) AS (
                SELECT ?2 AS memory_id, 0 AS level
                UNION ALL
                SELECT r.target_memory_id, g.level + 1
                FROM memory_relations r
                JOIN graph g ON r.source_memory_id = g.memory_id
                WHERE g.level < ?3 AND r.user_id = ?1
            )
            SELECT m.*
            FROM memories m
            JOIN graph g ON g.memory_id = m.id
            WHERE m.user_id = ?1
            "#,
        )
        .bind(user_id)
        .bind(memory_id)
        .bind(depth)
        .fetch_all(&self.read_pool)
        .await?;

        let edges = sqlx::query_as::<_, MemoryRelation>(
            r#"
            SELECT * FROM memory_relations
            WHERE user_id = ?1
              AND (source_memory_id IN (SELECT id FROM memories WHERE user_id = ?1)
                   OR target_memory_id IN (SELECT id FROM memories WHERE user_id = ?1))
              AND (source_memory_id = ?2 OR target_memory_id = ?2)
            "#,
        )
        .bind(user_id)
        .bind(memory_id)
        .fetch_all(&self.read_pool)
        .await?;

        Ok(MemoryGraph {
            root_memory_id: memory_id.to_string(),
            nodes,
            edges,
        })
    }
}
