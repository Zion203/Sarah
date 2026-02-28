use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::RwLock;

#[derive(Clone, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueryCategory {
    Simple,
    Medium,
    Complex,
    Code,
    Math,
    Creative,
    Analytical,
    Summarization,
    Translation,
}

impl QueryCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            QueryCategory::Simple => "simple",
            QueryCategory::Medium => "medium",
            QueryCategory::Complex => "complex",
            QueryCategory::Code => "code",
            QueryCategory::Math => "math",
            QueryCategory::Creative => "creative",
            QueryCategory::Analytical => "analytical",
            QueryCategory::Summarization => "summarization",
            QueryCategory::Translation => "translation",
        }
    }

    pub fn priority(&self) -> u8 {
        match self {
            QueryCategory::Simple => 1,
            QueryCategory::Medium => 2,
            QueryCategory::Complex => 3,
            QueryCategory::Code => 4,
            QueryCategory::Math => 4,
            QueryCategory::Creative => 2,
            QueryCategory::Analytical => 3,
            QueryCategory::Summarization => 2,
            QueryCategory::Translation => 2,
        }
    }

    pub fn max_tokens_hint(&self) -> usize {
        match self {
            QueryCategory::Simple => 256,
            QueryCategory::Medium => 512,
            QueryCategory::Complex => 1024,
            QueryCategory::Code => 1280,
            QueryCategory::Math => 768,
            QueryCategory::Creative => 768,
            QueryCategory::Analytical => 1024,
            QueryCategory::Summarization => 300, // Summarization output should be short! (but input can be massive)
            QueryCategory::Translation => 1024,
        }
    }
}

#[derive(Clone)]
pub struct SmartQueryClassifier {
    recent_classifications: Arc<RwLock<Vec<ClassificationRecord>>>,
}

#[derive(Clone, Debug)]
struct ClassificationRecord {
    category: QueryCategory,
    observed_at: Instant,
}

impl SmartQueryClassifier {
    pub fn new() -> Self {
        Self {
            recent_classifications: Arc::new(RwLock::new(Vec::with_capacity(256))),
        }
    }

    pub async fn classify(&self, query: &str) -> QueryCategory {
        let q = query.to_lowercase();
        let word_count = query.split_whitespace().count();

        let mut scores: HashMap<QueryCategory, f32> = HashMap::new();
        score_keywords(
            &q,
            &[
                "debug",
                "error",
                "stack",
                "trace",
                "function",
                "class",
                "compile",
                "refactor",
                "rust",
                "typescript",
                "python",
                "java",
            ],
            QueryCategory::Code,
            2.0,
            &mut scores,
        );
        score_keywords(
            &q,
            &[
                "calculate",
                "equation",
                "integral",
                "derivative",
                "matrix",
                "probability",
                "statistics",
                "solve",
            ],
            QueryCategory::Math,
            1.8,
            &mut scores,
        );
        score_keywords(
            &q,
            &[
                "analyze",
                "evaluate",
                "compare",
                "tradeoff",
                "reason",
                "architecture",
                "strategy",
                "optimize",
            ],
            QueryCategory::Analytical,
            1.4,
            &mut scores,
        );
        score_keywords(
            &q,
            &[
                "story",
                "poem",
                "creative",
                "imagine",
                "compose",
                "draft",
                "brainstorm",
            ],
            QueryCategory::Creative,
            1.2,
            &mut scores,
        );
        score_keywords(
            &q,
            &[
                "summarize",
                "summary",
                "tldr",
                "tl;dr",
                "in short",
                "brief",
                "outline",
            ],
            QueryCategory::Summarization,
            3.0,
            &mut scores,
        );
        score_keywords(
            &q,
            &[
                "translate",
                "in spanish",
                "in french",
                "in german",
                "in english",
                "meaning of",
            ],
            QueryCategory::Translation,
            2.5,
            &mut scores,
        );

        if q.contains("```")
            || q.contains("=>")
            || q.contains("::")
            || q.contains(" fn ")
            || q.contains(" let ")
        {
            *scores.entry(QueryCategory::Code).or_insert(0.0) += 3.0;
        }

        if query
            .chars()
            .filter(|c| ['+', '-', '*', '/', '='].contains(c))
            .count()
            >= 3
        {
            *scores.entry(QueryCategory::Math).or_insert(0.0) += 2.0;
        }

        if word_count < 10 {
            *scores.entry(QueryCategory::Simple).or_insert(0.0) += 1.0;
        } else if word_count > 80 {
            *scores.entry(QueryCategory::Complex).or_insert(0.0) += 2.0;
        } else if word_count > 35 {
            *scores.entry(QueryCategory::Medium).or_insert(0.0) += 1.0;
        }

        let category = scores
            .into_iter()
            .max_by(|left, right| {
                left.1
                    .partial_cmp(&right.1)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|row| row.0)
            .unwrap_or_else(|| {
                if word_count < 8 {
                    QueryCategory::Simple
                } else {
                    QueryCategory::Medium
                }
            });

        self.store(category.clone()).await;
        category
    }

    pub async fn suggest_max_tokens(&self, category: &QueryCategory) -> usize {
        category.max_tokens_hint()
    }

    pub async fn context_window_hint(&self) -> usize {
        let recent = self.recent_classifications.read().await;
        let active: Vec<_> = recent
            .iter()
            .filter(|record| record.observed_at.elapsed() < Duration::from_secs(300))
            .collect();

        if active.is_empty() {
            return 20;
        }

        let avg_priority = active
            .iter()
            .map(|record| record.category.priority() as f32)
            .sum::<f32>()
            / active.len() as f32;

        if avg_priority >= 3.4 {
            48
        } else if avg_priority >= 2.2 {
            32
        } else {
            18
        }
    }

    pub async fn distribution_snapshot(&self) -> HashMap<String, u64> {
        let recent = self.recent_classifications.read().await;
        let mut map: HashMap<String, u64> = HashMap::new();

        for record in recent
            .iter()
            .filter(|record| record.observed_at.elapsed() < Duration::from_secs(900))
        {
            *map.entry(record.category.as_str().to_string()).or_insert(0) += 1;
        }

        map
    }

    async fn store(&self, category: QueryCategory) {
        let mut recent = self.recent_classifications.write().await;
        recent.push(ClassificationRecord {
            category,
            observed_at: Instant::now(),
        });
        if recent.len() > 256 {
            let drain_to = recent.len().saturating_sub(192);
            recent.drain(0..drain_to);
        }
    }
}

impl Default for SmartQueryClassifier {
    fn default() -> Self {
        Self::new()
    }
}

fn score_keywords(
    query: &str,
    keywords: &[&str],
    category: QueryCategory,
    weight: f32,
    out: &mut HashMap<QueryCategory, f32>,
) {
    let matches = keywords
        .iter()
        .filter(|keyword| query.contains(**keyword))
        .count() as f32;
    if matches > 0.0 {
        *out.entry(category).or_insert(0.0) += matches * weight;
    }
}
