use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{Local, Timelike};
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct UsageLearner {
    session_patterns: Arc<RwLock<SessionPattern>>,
    query_patterns: Arc<RwLock<QueryPattern>>,
    time_buckets: Arc<RwLock<TimeUsage>>,
    model_preferences: Arc<RwLock<ModelPreferences>>,
}

#[derive(Clone, Default)]
struct SessionPattern {
    total_sessions: u64,
    peak_hours: Vec<u8>,
}

#[derive(Clone, Default)]
struct QueryPattern {
    total_queries: u64,
    avg_query_length: f32,
    category_distribution: HashMap<String, u32>,
}

#[derive(Clone)]
struct TimeUsage {
    hourly_counts: [u32; 24],
    last_reset: Instant,
}

impl Default for TimeUsage {
    fn default() -> Self {
        Self {
            hourly_counts: [0; 24],
            last_reset: Instant::now(),
        }
    }
}

#[derive(Clone, Default)]
struct ModelPreferences {
    model_usage_counts: HashMap<String, u64>,
    preferred_model: Option<String>,
    fallback_model: Option<String>,
}

impl UsageLearner {
    pub fn new() -> Self {
        Self {
            session_patterns: Arc::new(RwLock::new(SessionPattern::default())),
            query_patterns: Arc::new(RwLock::new(QueryPattern::default())),
            time_buckets: Arc::new(RwLock::new(TimeUsage::default())),
            model_preferences: Arc::new(RwLock::new(ModelPreferences::default())),
        }
    }

    pub async fn record_query(&self, query: &str, category: &str) {
        let mut patterns = self.query_patterns.write().await;
        patterns.total_queries += 1;

        let query_len = query.len() as f32;
        patterns.avg_query_length =
            ((patterns.avg_query_length * ((patterns.total_queries - 1) as f32)) + query_len)
                / patterns.total_queries as f32;

        *patterns
            .category_distribution
            .entry(category.to_string())
            .or_insert(0) += 1;
    }

    pub async fn record_session_start(&self) {
        let mut sessions = self.session_patterns.write().await;
        sessions.total_sessions += 1;

        let hour = Local::now().hour() as u8;
        sessions.peak_hours.push(hour);
        if sessions.peak_hours.len() > 200 {
            sessions.peak_hours.drain(0..100);
        }
    }

    pub async fn record_time_usage(&self) {
        let hour = Local::now().hour() as usize;
        let mut buckets = self.time_buckets.write().await;

        if buckets.last_reset.elapsed() > Duration::from_secs(86_400) {
            buckets.hourly_counts = [0; 24];
            buckets.last_reset = Instant::now();
        }

        buckets.hourly_counts[hour] = buckets.hourly_counts[hour].saturating_add(1);
    }

    pub async fn record_model_usage(&self, model_name: &str) {
        let mut prefs = self.model_preferences.write().await;
        let entry = prefs
            .model_usage_counts
            .entry(model_name.to_string())
            .or_insert(0);
        *entry = entry.saturating_add(1);

        let mut by_count: Vec<(String, u64)> = prefs
            .model_usage_counts
            .iter()
            .map(|(name, count)| (name.clone(), *count))
            .collect();
        by_count.sort_by(|left, right| right.1.cmp(&left.1));

        prefs.preferred_model = by_count.first().map(|(name, _)| name.clone());
        prefs.fallback_model = by_count.get(1).map(|(name, _)| name.clone());
    }

    pub async fn get_optimal_context_window(&self) -> usize {
        let patterns = self.query_patterns.read().await;
        let avg_len = patterns.avg_query_length;

        if avg_len < 50.0 {
            16
        } else if avg_len < 150.0 {
            28
        } else if avg_len < 300.0 {
            44
        } else {
            64
        }
    }

    pub async fn get_model_recommendation(&self) -> (Option<String>, Option<String>) {
        let prefs = self.model_preferences.read().await;
        (prefs.preferred_model.clone(), prefs.fallback_model.clone())
    }

    pub async fn is_peak_hours(&self) -> bool {
        let sessions = self.session_patterns.read().await;
        let current_hour = Local::now().hour() as u8;
        let count_for_hour = sessions
            .peak_hours
            .iter()
            .filter(|&&hour| hour == current_hour)
            .count();
        let total = sessions.peak_hours.len().max(1);
        (count_for_hour as f32 / total as f32) > 0.15
    }

    pub async fn get_learning_stats(&self) -> LearningStats {
        let patterns = self.query_patterns.read().await;
        let sessions = self.session_patterns.read().await;
        let time = self.time_buckets.read().await;

        let peak_hour = time
            .hourly_counts
            .iter()
            .enumerate()
            .max_by_key(|(_, count)| *count)
            .map(|(hour, _)| hour)
            .unwrap_or(12);

        let top_category = patterns
            .category_distribution
            .iter()
            .max_by_key(|(_, count)| *count)
            .map(|(category, _)| category.clone())
            .unwrap_or_else(|| "unknown".to_string());

        LearningStats {
            total_queries: patterns.total_queries,
            avg_query_length: patterns.avg_query_length,
            total_sessions: sessions.total_sessions,
            peak_hour,
            top_category,
        }
    }

    pub async fn suggest_optimizations(&self) -> Vec<String> {
        let mut suggestions = Vec::new();
        let stats = self.get_learning_stats().await;

        if stats.avg_query_length > 200.0 {
            suggestions.push("increase_context_window".to_string());
        }

        if stats.total_queries > 100 && stats.top_category == "simple" {
            suggestions.push("prefer_lightweight_model".to_string());
        }

        if self.is_peak_hours().await {
            suggestions.push("defer_background_jobs".to_string());
        }

        suggestions
    }
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LearningStats {
    pub total_queries: u64,
    pub avg_query_length: f32,
    pub total_sessions: u64,
    pub peak_hour: usize,
    pub top_category: String,
}

impl Default for UsageLearner {
    fn default() -> Self {
        Self::new()
    }
}
