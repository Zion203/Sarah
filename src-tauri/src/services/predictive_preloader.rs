use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;
use tokio::time::interval;

use crate::db::models::SystemProfile;
use crate::services::embedding_service::EmbeddingService;
use crate::services::hardware_service::{HardwareService, LoadDecision};
use crate::services::inference_service::InferenceService;

#[derive(Clone)]
pub struct PredictivePreloader {
    enabled: Arc<AtomicBool>,
    inference: Arc<InferenceService>,
    embedding: Option<Arc<EmbeddingService>>,
    hardware: Arc<HardwareService>,
    recent_queries: Arc<Mutex<VecDeque<QueryContext>>>,
    preload_scheduled: Arc<AtomicBool>,
    cooldown_secs: u64,
}

#[derive(Clone, Debug)]
struct QueryContext {
    observed_at: Instant,
    estimated_complexity: f32,
}

impl PredictivePreloader {
    pub fn new(
        inference: Arc<InferenceService>,
        embedding: Option<Arc<EmbeddingService>>,
        hardware: Arc<HardwareService>,
    ) -> Self {
        Self {
            enabled: Arc::new(AtomicBool::new(true)),
            inference,
            embedding,
            hardware,
            recent_queries: Arc::new(Mutex::new(VecDeque::with_capacity(128))),
            preload_scheduled: Arc::new(AtomicBool::new(false)),
            cooldown_secs: 30,
        }
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub fn record_query(&self, query: &str) {
        if !self.is_enabled() {
            return;
        }

        let context = QueryContext {
            observed_at: Instant::now(),
            estimated_complexity: estimate_complexity(query),
        };
        let recent = Arc::clone(&self.recent_queries);

        tokio::spawn(async move {
            let mut queue = recent.lock().await;
            queue.push_back(context);
            while queue.len() > 128 {
                let _ = queue.pop_front();
            }
        });
    }

    pub async fn sample_count(&self) -> usize {
        let queue = self.recent_queries.lock().await;
        queue.len()
    }

    pub async fn maybe_preload(&self, model_path: &str, profile: &SystemProfile) {
        if !self.is_enabled() || self.preload_scheduled.load(Ordering::Relaxed) {
            return;
        }

        let avg_complexity = self.recent_complexity().await;
        if avg_complexity < 0.62 {
            return;
        }

        let has_embedding = self.embedding.is_some();
        let load_decision = self.hardware.should_load_model(4096);

        if !has_embedding && avg_complexity < 0.75 {
            return;
        }

        match load_decision {
            LoadDecision::LoadNow | LoadDecision::BackgroundOnly => {
                self.trigger_preload(model_path, profile.clone()).await;
            }
            LoadDecision::Defer | LoadDecision::Skip => {
                tracing::debug!(
                    "Predictive preload skipped due to hardware load decision: {:?}",
                    load_decision
                );
            }
        }
    }

    async fn recent_complexity(&self) -> f32 {
        let queue = self.recent_queries.lock().await;
        let mut count = 0usize;
        let mut sum = 0.0f32;

        for sample in queue.iter().rev() {
            if sample.observed_at.elapsed() > Duration::from_secs(300) {
                break;
            }
            count += 1;
            sum += sample.estimated_complexity;
        }

        if count == 0 {
            0.0
        } else {
            sum / count as f32
        }
    }

    async fn trigger_preload(&self, model_path: &str, profile: SystemProfile) {
        if self.preload_scheduled.swap(true, Ordering::Relaxed) {
            return;
        }

        let inference = Arc::clone(&self.inference);
        let hardware = Arc::clone(&self.hardware);
        let path = model_path.to_string();
        let cooldown = self.cooldown_secs;
        let scheduled = Arc::clone(&self.preload_scheduled);

        tokio::spawn(async move {
            tracing::info!("Predictive preloader warming model: {}", path);
            let mode = hardware.get_performance_mode(None).await;
            if let Err(error) = inference.load_model(&path, &profile, mode).await {
                tracing::warn!("Predictive preload failed: {}", error);
            }

            tokio::time::sleep(Duration::from_secs(cooldown)).await;
            scheduled.store(false, Ordering::Relaxed);
        });
    }

    pub async fn start_background_predictor(&self) {
        if !self.is_enabled() {
            return;
        }

        let preloader = self.clone();
        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(60));
            loop {
                ticker.tick().await;

                if !preloader.is_enabled() {
                    continue;
                }

                let stats = preloader.hardware.live_stats();
                let memory_pressure = if stats.memory_total_mb > 0 {
                    stats.memory_used_mb as f64 / stats.memory_total_mb as f64
                } else {
                    0.0
                };

                if stats.cpu_usage_pct > 85.0 || memory_pressure > 0.88 {
                    tracing::debug!("Predictive preloader paused under high pressure");
                    continue;
                }

                let mut queue = preloader.recent_queries.lock().await;
                while let Some(front) = queue.front() {
                    if front.observed_at.elapsed() > Duration::from_secs(1200) {
                        let _ = queue.pop_front();
                    } else {
                        break;
                    }
                }
            }
        });
    }
}

fn estimate_complexity(query: &str) -> f32 {
    let word_count = query.split_whitespace().count() as f32;
    let length_score = (word_count / 48.0).clamp(0.0, 1.0);

    let lower = query.to_lowercase();
    let code_indicators = [
        "```", "function", "class", "def ", "import ", "const ", "let ", "error", "stack",
    ]
    .iter()
    .filter(|needle| lower.contains(**needle))
    .count() as f32;
    let code_score = (code_indicators / 5.0).clamp(0.0, 1.0);

    let reasoning_indicators = [
        "analy",
        "compare",
        "tradeoff",
        "reason",
        "optimiz",
        "performance",
        "architecture",
    ]
    .iter()
    .filter(|needle| lower.contains(**needle))
    .count() as f32;
    let reasoning_score = (reasoning_indicators / 4.0).clamp(0.0, 1.0);

    (length_score * 0.35 + code_score * 0.4 + reasoning_score * 0.25).clamp(0.0, 1.0)
}
