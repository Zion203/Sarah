use std::collections::HashMap;
use std::sync::Arc;

use crate::db::models::{Memory, Message, NewMemory};
use crate::error::AppError;
use crate::repositories::embedding_repo::EmbeddingRepo;
use crate::repositories::memory_repo::MemoryRepo;
use crate::services::embedding_service::EmbeddingService;
use crate::services::inference_service::InferenceService;

#[derive(Clone)]
pub struct MemoryService {
    memory_repo: MemoryRepo,
    embedding_repo: EmbeddingRepo,
    embedding_service: Option<Arc<EmbeddingService>>,
    inference_service: InferenceService,
}

impl MemoryService {
    pub fn new(
        memory_repo: MemoryRepo,
        embedding_repo: EmbeddingRepo,
        embedding_service: Option<Arc<EmbeddingService>>,
        inference_service: InferenceService,
    ) -> Self {
        Self {
            memory_repo,
            embedding_repo,
            embedding_service,
            inference_service,
        }
    }

    fn is_embedding_available(&self) -> bool {
        self.embedding_service
            .as_ref()
            .map(|e| e.is_initialized())
            .unwrap_or(false)
    }

    pub async fn extract_from_message(
        &self,
        message: &Message,
        user_id: &str,
    ) -> Result<Vec<NewMemory>, AppError> {
        let mut extracted = Vec::new();

        for sentence in message.content.split(['.', '!', '?']) {
            let trimmed = sentence.trim();
            if trimmed.len() < 16 {
                continue;
            }

            let confidence = if trimmed.contains("always") || trimmed.contains("prefer") {
                0.84
            } else {
                0.72
            };

            if confidence < 0.7 {
                continue;
            }

            extracted.push(NewMemory {
                user_id: user_id.to_string(),
                memory_type: "semantic".to_string(),
                category: Some("fact".to_string()),
                subject: Some("user".to_string()),
                predicate: Some("said".to_string()),
                object: None,
                content: trimmed.to_string(),
                summary: Some(trimmed.chars().take(96).collect()),
                source: "conversation".to_string(),
                source_id: Some(message.id.clone()),
                session_id: Some(message.session_id.clone()),
                confidence,
                importance: 0.55,
                decay_rate: 0.001,
                privacy_level: "private".to_string(),
                tags: "[]".to_string(),
                metadata: "{}".to_string(),
            });
        }

        Ok(extracted)
    }

    pub async fn extract_batch(
        &self,
        messages: &[Message],
        user_id: &str,
    ) -> Result<Vec<NewMemory>, AppError> {
        let mut all = Vec::new();
        for message in messages {
            all.extend(self.extract_from_message(message, user_id).await?);
        }
        Ok(all)
    }

    pub async fn retrieve_relevant(
        &self,
        user_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<Memory>, AppError> {
        let text_hits = self
            .memory_repo
            .search_memories_text(user_id, query, (limit as i64) * 4)
            .await
            .unwrap_or_default();
        let candidate_memories = if text_hits.is_empty() {
            self.memory_repo
                .get_memories(user_id, None, (limit as i64) * 6)
                .await
                .unwrap_or_default()
        } else {
            text_hits
        };

        if !self.is_embedding_available() {
            return Ok(candidate_memories.into_iter().take(limit).collect());
        }

        let embedding = self
            .embedding_service
            .as_ref()
            .ok_or_else(|| AppError::Embedding("Embedding service not available".to_string()))?;

        let query_vec = embedding.embed_text(query).await?;
        let mut candidate_ids: Vec<String> = candidate_memories
            .iter()
            .map(|memory| memory.id.clone())
            .collect();
        candidate_ids.sort();
        candidate_ids.dedup();

        let all_embeddings = self
            .embedding_repo
            .get_embeddings_for_entities("memory", user_id, "memory", &candidate_ids)
            .await
            .unwrap_or_default();

        let mut vector_scores: HashMap<String, f32> = HashMap::new();
        for row in all_embeddings {
            let vec = crate::repositories::blob_to_vector(&row.vector);
            if vec.len() != query_vec.len() {
                continue;
            }

            let score = cosine_similarity(&query_vec, &vec);
            vector_scores.insert(row.entity_id, score);
        }

        let mut scored = Vec::new();
        for memory in candidate_memories {
            let vec_score = vector_scores.get(&memory.id).copied().unwrap_or(0.0) as f64;
            let recency_boost = recency_factor(&memory.created_at);
            let rank = memory.importance * recency_boost * (1.0 + vec_score);
            scored.push((rank, memory));
        }

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        let mut top = Vec::new();
        for (_, memory) in scored.into_iter().take(limit) {
            let _ = self.memory_repo.update_access_count(&memory.id).await;
            top.push(memory);
        }

        Ok(top)
    }

    pub async fn apply_decay_job(&self, user_id: &str) -> Result<u64, AppError> {
        self.memory_repo.apply_time_decay(user_id).await
    }

    pub async fn consolidate_memories(&self, user_id: &str) -> Result<(), AppError> {
        let Some(embedding) = &self.embedding_service else {
            tracing::warn!("Cannot consolidate memories - embedding service not available");
            return Ok(());
        };

        if !embedding.is_initialized() {
            tracing::warn!("Cannot consolidate memories - embedding model not loaded");
            return Ok(());
        }

        let memories = self.memory_repo.get_memories(user_id, None, 500).await?;

        let mut merged_ids = std::collections::HashSet::new();
        for i in 0..memories.len() {
            if merged_ids.contains(&memories[i].id) {
                continue;
            }

            for j in (i + 1)..memories.len() {
                if merged_ids.contains(&memories[j].id) {
                    continue;
                }

                let left_vec = embedding.embed_text(&memories[i].content).await?;
                let right_vec = embedding.embed_text(&memories[j].content).await?;
                let similarity = cosine_similarity(&left_vec, &right_vec);
                if similarity > 0.92 {
                    merged_ids.insert(memories[j].id.clone());
                }
            }
        }

        for id in merged_ids {
            let _ = self.memory_repo.delete_memory(&id).await;
        }

        Ok(())
    }

    pub async fn build_memory_context(
        &self,
        user_id: &str,
        query: &str,
        max_tokens: usize,
    ) -> Result<String, AppError> {
        let memories = self.retrieve_relevant(user_id, query, 12).await?;
        let mut output = String::new();
        let mut used_tokens = 0usize;

        for memory in memories {
            let line = format!("- [Memory:{}] {}\n", memory.id, memory.content);
            let estimate = line.len() / 4;
            if used_tokens + estimate > max_tokens {
                break;
            }
            used_tokens += estimate;
            output.push_str(&line);
        }

        Ok(output)
    }

    pub async fn persist_extracted(
        &self,
        extracted: Vec<NewMemory>,
    ) -> Result<Vec<Memory>, AppError> {
        let mut saved = Vec::new();
        let embedding_opt = self.embedding_service.as_ref();

        for memory in extracted {
            let row = self.memory_repo.upsert_memory(memory).await?;
            if let Some(embedding) = embedding_opt {
                if embedding.is_initialized() {
                    let _ = embedding
                        .embed_and_store("memory", &row.id, &row.user_id, "memory", &row.content)
                        .await;
                }
            }
            saved.push(row);
        }
        Ok(saved)
    }

    pub async fn get_memory_graph(
        &self,
        user_id: &str,
        memory_id: &str,
        depth: i64,
    ) -> Result<crate::db::models::MemoryGraph, AppError> {
        self.memory_repo
            .get_memory_graph(user_id, memory_id, depth)
            .await
    }

    pub async fn classify_query_intent_for_memory(&self, query: &str) -> Result<String, AppError> {
        let pseudo_message = Message {
            id: String::new(),
            session_id: String::new(),
            role: "user".to_string(),
            content: query.to_string(),
            content_type: "text".to_string(),
            thinking: None,
            token_count: None,
            model_id: None,
            latency_ms: None,
            tokens_per_sec: None,
            finish_reason: None,
            is_error: 0,
            error_message: None,
            parent_message_id: None,
            edited_at: None,
            original_content: None,
            metadata: "{}".to_string(),
            position: 0,
            created_at: String::new(),
            updated_at: String::new(),
        };

        let _ = pseudo_message;
        let _ = &self.inference_service;
        Ok("memory".to_string())
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

fn recency_factor(created_at: &str) -> f64 {
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(created_at) {
        let age_days = (chrono::Utc::now() - dt.with_timezone(&chrono::Utc))
            .num_days()
            .max(0) as f64;
        return (1.0 / (1.0 + age_days / 30.0)).max(0.2);
    }
    1.0
}
