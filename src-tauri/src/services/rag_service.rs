use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use calamine::Reader;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::db::models::{NewChunk, NewDocument, RankCandidate, RetrievedChunk};
use crate::error::AppError;
use crate::repositories::document_repo::DocumentRepo;
use crate::repositories::embedding_repo::EmbeddingRepo;
use crate::services::embedding_service::EmbeddingService;
use crate::services::reranker_service::RerankerService;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextChunk {
    pub chunk_index: i64,
    pub content: String,
    pub token_count: i64,
    pub start_char: i64,
    pub end_char: i64,
}

#[derive(Clone)]
pub struct RagService {
    document_repo: DocumentRepo,
    embedding_repo: EmbeddingRepo,
    embedding_service: Arc<EmbeddingService>,
    reranker_service: Arc<RerankerService>,
    write_pool: SqlitePool,
}

impl RagService {
    pub fn new(
        document_repo: DocumentRepo,
        embedding_repo: EmbeddingRepo,
        embedding_service: Arc<EmbeddingService>,
        reranker_service: Arc<RerankerService>,
        write_pool: SqlitePool,
    ) -> Self {
        Self {
            document_repo,
            embedding_repo,
            embedding_service,
            reranker_service,
            write_pool,
        }
    }

    pub async fn ingest_document(
        &self,
        user_id: &str,
        file_path: &str,
    ) -> Result<String, AppError> {
        let path = Path::new(file_path);
        if !path.exists() {
            return Err(AppError::Validation {
                field: "file_path".to_string(),
                message: format!("Path does not exist: {file_path}"),
            });
        }

        let mime = mime_guess::from_path(path)
            .first_raw()
            .unwrap_or("application/octet-stream")
            .to_string();

        let metadata = tokio::fs::metadata(path).await?;
        let content = self.extract_text(path, &mime).await?;
        let chunks = self.chunker(&content, 512, 64);

        let title = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("document")
            .to_string();

        let document = self
            .document_repo
            .insert_document(NewDocument {
                user_id: user_id.to_string(),
                title,
                file_path: Some(path.to_string_lossy().to_string()),
                source_url: None,
                source_type: "file".to_string(),
                mime_type: Some(mime),
                file_size_bytes: Some(metadata.len() as i64),
                namespace: "personal".to_string(),
                checksum: None,
                metadata: "{}".to_string(),
            })
            .await?;

        for chunk in chunks {
            self.document_repo
                .insert_chunk(NewChunk {
                    document_id: document.id.clone(),
                    user_id: user_id.to_string(),
                    chunk_index: chunk.chunk_index,
                    content: chunk.content,
                    token_count: chunk.token_count,
                    start_char: Some(chunk.start_char),
                    end_char: Some(chunk.end_char),
                    page_number: None,
                    section_title: None,
                    heading_path: None,
                    metadata: "{}".to_string(),
                })
                .await?;
        }

        self.document_repo
            .update_index_status(
                &document.id,
                "indexing",
                self.document_repo
                    .get_chunks_by_document(&document.id)
                    .await?
                    .len() as i64,
            )
            .await?;

        Ok(document.id)
    }

    pub async fn embed_document_chunks(&self, document_id: &str) -> Result<(), AppError> {
        let chunks = self
            .document_repo
            .get_chunks_by_document(document_id)
            .await?;
        if chunks.is_empty() {
            self.document_repo
                .update_index_status(document_id, "failed", 0)
                .await?;
            return Ok(());
        }

        let texts: Vec<String> = chunks.iter().map(|chunk| chunk.content.clone()).collect();
        let vectors = self.embedding_service.embed_batch(texts).await?;

        for (chunk, vector) in chunks.iter().zip(vectors.into_iter()) {
            let embedding_id = self
                .embedding_repo
                .upsert_embedding(
                    "chunk",
                    &chunk.id,
                    &chunk.user_id,
                    "default",
                    vector,
                    "fastembed",
                )
                .await?;

            sqlx::query("UPDATE document_chunks SET embedding_id = ?1 WHERE id = ?2")
                .bind(embedding_id)
                .bind(&chunk.id)
                .execute(&self.write_pool)
                .await?;
        }

        self.document_repo
            .update_index_status(document_id, "indexed", chunks.len() as i64)
            .await?;

        Ok(())
    }

    pub fn chunker(&self, text: &str, chunk_size: usize, overlap: usize) -> Vec<TextChunk> {
        let words: Vec<&str> = text.split_whitespace().collect();
        if words.is_empty() {
            return Vec::new();
        }

        let mut out = Vec::new();
        let mut idx = 0usize;
        let mut chunk_index = 0i64;

        while idx < words.len() {
            let end = (idx + chunk_size).min(words.len());
            let slice = &words[idx..end];
            let content = slice.join(" ");

            out.push(TextChunk {
                chunk_index,
                token_count: slice.len() as i64,
                start_char: idx as i64,
                end_char: end as i64,
                content,
            });

            if end == words.len() {
                break;
            }

            idx = end.saturating_sub(overlap);
            chunk_index += 1;
        }

        out
    }

    pub async fn retrieve(
        &self,
        user_id: &str,
        query: &str,
        namespace: &str,
        limit: usize,
    ) -> Result<Vec<RetrievedChunk>, AppError> {
        let started = Instant::now();
        let query_embedding = self.embedding_service.embed_text(query).await?;

        let bm25 = self
            .document_repo
            .search_chunks_bm25(user_id, query, namespace, 20)
            .await
            .unwrap_or_default();

        let mut candidate_ids: Vec<String> = bm25.iter().map(|row| row.id.clone()).collect();
        if candidate_ids.is_empty() {
            candidate_ids = self
                .document_repo
                .get_recent_chunk_ids(user_id, namespace, 48)
                .await
                .unwrap_or_default();
        }
        candidate_ids.sort();
        candidate_ids.dedup();

        let candidate_embeddings = self
            .embedding_repo
            .get_embeddings_for_entities("default", user_id, "chunk", &candidate_ids)
            .await
            .unwrap_or_default();

        let mut vector_ranked: Vec<(String, f32)> = candidate_embeddings
            .into_iter()
            .filter_map(|row| {
                let vec = crate::repositories::blob_to_vector(&row.vector);
                if vec.len() != query_embedding.len() {
                    return None;
                }
                Some((row.entity_id, cosine_similarity(&query_embedding, &vec)))
            })
            .collect();

        vector_ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));
        vector_ranked.truncate(20);

        let mut ranks: HashMap<String, f32> = HashMap::new();
        let k = 60.0f32;

        for (idx, item) in bm25.iter().enumerate() {
            *ranks.entry(item.id.clone()).or_insert(0.0) += 1.0 / (k + idx as f32 + 1.0);
        }

        for (idx, (chunk_id, _score)) in vector_ranked.iter().enumerate() {
            *ranks.entry(chunk_id.clone()).or_insert(0.0) += 1.0 / (k + idx as f32 + 1.0);
        }

        let mut fused: Vec<(String, f32)> = ranks.into_iter().collect();
        fused.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));

        let mut candidates = Vec::new();
        for (chunk_id, _) in fused.iter().take(15) {
            if let Some(chunk) = self.document_repo.get_chunk(chunk_id).await? {
                candidates.push(chunk);
            }
        }

        let rerank_input: Vec<RankCandidate> = candidates
            .iter()
            .map(|chunk| RankCandidate {
                id: chunk.id.clone(),
                text: chunk.content.clone(),
                metadata: None,
            })
            .collect();

        let reranked = self.reranker_service.rerank(query, rerank_input).await?;
        let mut selected_ids: Vec<String> =
            reranked.into_iter().take(limit).map(|row| row.id).collect();
        if selected_ids.is_empty() {
            selected_ids = candidates.into_iter().take(limit).map(|c| c.id).collect();
        }

        let mut with_neighbors = Vec::new();
        let mut seen = HashSet::new();

        for chunk_id in &selected_ids {
            let neighbors = self
                .document_repo
                .get_chunk_with_neighbors(chunk_id, 1)
                .await
                .unwrap_or_default();

            for chunk in neighbors {
                if seen.insert(chunk.id.clone()) {
                    let vector_score = vector_ranked
                        .iter()
                        .find(|(id, _)| id == &chunk.id)
                        .map(|(_, score)| *score);
                    let bm25_score = bm25
                        .iter()
                        .find(|entry| entry.id == chunk.id)
                        .map(|entry| entry.score as f32);

                    with_neighbors.push(RetrievedChunk {
                        chunk,
                        vector_score,
                        bm25_score,
                        rerank_score: None,
                    });
                }
            }
        }

        let latency_ms = started.elapsed().as_millis() as i64;

        sqlx::query(
            r#"
            INSERT INTO rag_retrievals (id, query_text, retrieved_chunk_ids, reranked_chunk_ids, strategy, latency_ms)
            VALUES (?1, ?2, ?3, ?4, 'hybrid', ?5)
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(query)
        .bind(serde_json::to_string(&selected_ids).unwrap_or_else(|_| "[]".to_string()))
        .bind(serde_json::to_string(&selected_ids).unwrap_or_else(|_| "[]".to_string()))
        .bind(latency_ms)
        .execute(&self.write_pool)
        .await?;

        Ok(with_neighbors)
    }

    async fn extract_text(&self, path: &Path, mime: &str) -> Result<String, AppError> {
        if mime.contains("pdf") {
            return pdf_extract::extract_text(path).map_err(|e| AppError::Io(e.to_string()));
        }

        if mime.contains("markdown") || path.extension().and_then(|e| e.to_str()) == Some("md") {
            return tokio::fs::read_to_string(path)
                .await
                .map_err(AppError::from);
        }

        if mime.contains("sheet")
            || matches!(
                path.extension().and_then(|e| e.to_str()),
                Some("xls") | Some("xlsx")
            )
        {
            let mut workbook =
                calamine::open_workbook_auto(path).map_err(|e| AppError::Io(e.to_string()))?;
            let mut text = String::new();
            for sheet in workbook.sheet_names().to_owned() {
                if let Ok(range) = workbook.worksheet_range(&sheet) {
                    for row in range.rows() {
                        for cell in row {
                            text.push_str(&cell.to_string());
                            text.push(' ');
                        }
                        text.push('\n');
                    }
                }
            }
            return Ok(text);
        }

        tokio::fs::read_to_string(path)
            .await
            .map_err(AppError::from)
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;

    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }

    dot / ((norm_a.sqrt() * norm_b.sqrt()).max(1e-6))
}
