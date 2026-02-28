use sqlx::SqlitePool;
use uuid::Uuid;

use crate::db::models::{Chunk, ChunkResult, Document, NewChunk, NewDocument};
use crate::error::AppError;

#[derive(Clone)]
pub struct DocumentRepo {
    read_pool: SqlitePool,
    write_pool: SqlitePool,
}

impl DocumentRepo {
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

    pub async fn insert_document(&self, doc: NewDocument) -> Result<Document, AppError> {
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            r#"
            INSERT INTO documents (
              id, user_id, title, file_path, source_url, source_type, mime_type,
              file_size_bytes, namespace, checksum, metadata
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            "#,
        )
        .bind(&id)
        .bind(&doc.user_id)
        .bind(&doc.title)
        .bind(&doc.file_path)
        .bind(&doc.source_url)
        .bind(&doc.source_type)
        .bind(&doc.mime_type)
        .bind(doc.file_size_bytes)
        .bind(&doc.namespace)
        .bind(&doc.checksum)
        .bind(&doc.metadata)
        .execute(&self.write_pool)
        .await?;

        self.get_document(&id)
            .await?
            .ok_or_else(|| AppError::NotFound {
                entity: "document".to_string(),
                id,
            })
    }

    pub async fn get_document(&self, id: &str) -> Result<Option<Document>, AppError> {
        let row = sqlx::query_as::<_, Document>("SELECT * FROM documents WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.read_pool)
            .await?;
        Ok(row)
    }

    pub async fn update_index_status(
        &self,
        id: &str,
        status: &str,
        chunk_count: i64,
    ) -> Result<(), AppError> {
        sqlx::query(
            "UPDATE documents SET index_status = ?1, chunk_count = ?2, last_indexed_at = datetime('now','utc') WHERE id = ?3",
        )
        .bind(status)
        .bind(chunk_count)
        .bind(id)
        .execute(&self.write_pool)
        .await?;
        Ok(())
    }

    pub async fn insert_chunk(&self, chunk: NewChunk) -> Result<Chunk, AppError> {
        let id = Uuid::new_v4().to_string();

        sqlx::query(
            r#"
            INSERT INTO document_chunks (
              id, document_id, user_id, chunk_index, content, token_count,
              start_char, end_char, page_number, section_title, heading_path, metadata
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
            "#,
        )
        .bind(&id)
        .bind(&chunk.document_id)
        .bind(&chunk.user_id)
        .bind(chunk.chunk_index)
        .bind(&chunk.content)
        .bind(chunk.token_count)
        .bind(chunk.start_char)
        .bind(chunk.end_char)
        .bind(chunk.page_number)
        .bind(&chunk.section_title)
        .bind(&chunk.heading_path)
        .bind(&chunk.metadata)
        .execute(&self.write_pool)
        .await?;

        self.get_chunk(&id)
            .await?
            .ok_or_else(|| AppError::NotFound {
                entity: "chunk".to_string(),
                id,
            })
    }

    pub async fn get_chunk(&self, chunk_id: &str) -> Result<Option<Chunk>, AppError> {
        let row = sqlx::query_as::<_, Chunk>("SELECT * FROM document_chunks WHERE id = ?1")
            .bind(chunk_id)
            .fetch_optional(&self.read_pool)
            .await?;
        Ok(row)
    }

    pub async fn get_chunks_by_document(&self, document_id: &str) -> Result<Vec<Chunk>, AppError> {
        let rows = sqlx::query_as::<_, Chunk>(
            "SELECT * FROM document_chunks WHERE document_id = ?1 ORDER BY chunk_index",
        )
        .bind(document_id)
        .fetch_all(&self.read_pool)
        .await?;

        Ok(rows)
    }

    pub async fn search_chunks_bm25(
        &self,
        user_id: &str,
        query: &str,
        namespace: &str,
        limit: i64,
    ) -> Result<Vec<ChunkResult>, AppError> {
        let rows = sqlx::query_as::<_, ChunkResult>(
            r#"
            SELECT c.id, c.document_id, c.chunk_index, c.content, c.section_title,
                   bm25(chunks_fts) AS score
            FROM chunks_fts
            JOIN document_chunks c ON c.id = chunks_fts.chunk_id
            JOIN documents d ON d.id = c.document_id
            WHERE c.user_id = ?1
              AND d.namespace = ?2
              AND chunks_fts MATCH ?3
            ORDER BY score
            LIMIT ?4
            "#,
        )
        .bind(user_id)
        .bind(namespace)
        .bind(query)
        .bind(limit)
        .fetch_all(&self.read_pool)
        .await?;

        Ok(rows)
    }

    pub async fn get_chunk_with_neighbors(
        &self,
        chunk_id: &str,
        window: i64,
    ) -> Result<Vec<Chunk>, AppError> {
        let Some(center) = self.get_chunk(chunk_id).await? else {
            return Ok(Vec::new());
        };

        let min_index = (center.chunk_index - window).max(0);
        let max_index = center.chunk_index + window;

        let rows = sqlx::query_as::<_, Chunk>(
            r#"
            SELECT * FROM document_chunks
            WHERE document_id = ?1
              AND chunk_index BETWEEN ?2 AND ?3
            ORDER BY chunk_index
            "#,
        )
        .bind(&center.document_id)
        .bind(min_index)
        .bind(max_index)
        .fetch_all(&self.read_pool)
        .await?;

        Ok(rows)
    }

    pub async fn get_recent_chunk_ids(
        &self,
        user_id: &str,
        namespace: &str,
        limit: i64,
    ) -> Result<Vec<String>, AppError> {
        let rows = sqlx::query_scalar::<_, String>(
            r#"
            SELECT c.id
            FROM document_chunks c
            JOIN documents d ON d.id = c.document_id
            WHERE c.user_id = ?1
              AND d.namespace = ?2
              AND d.is_deleted = 0
            ORDER BY datetime(c.created_at) DESC
            LIMIT ?3
            "#,
        )
        .bind(user_id)
        .bind(namespace)
        .bind(limit)
        .fetch_all(&self.read_pool)
        .await?;

        Ok(rows)
    }
}
