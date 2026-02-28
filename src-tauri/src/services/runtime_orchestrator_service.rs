use std::cmp::min;
use std::sync::Arc;

use crate::db::models::{RuntimePolicy, SystemProfile};
use crate::error::AppError;
use crate::services::adaptive_memory_manager::{AdaptiveMemoryManager, MemoryManagerStats};
use crate::services::hardware_service::{DeviceTier, HardwareService};
use crate::services::predictive_preloader::PredictivePreloader;
use crate::services::runtime_governor_service::RuntimeGovernorService;
use crate::services::smart_query_classifier::{QueryCategory, SmartQueryClassifier};
use crate::services::usage_learner::{LearningStats, UsageLearner};

#[derive(Clone)]
pub struct RuntimeOrchestratorService {
    runtime_governor: RuntimeGovernorService,
    hardware_service: Arc<HardwareService>,
    query_classifier: Arc<SmartQueryClassifier>,
    usage_learner: Arc<UsageLearner>,
    adaptive_memory: Arc<AdaptiveMemoryManager>,
    predictive_preloader: Arc<PredictivePreloader>,
    detected_tier: DeviceTier,
    active_tier: DeviceTier,
    feature_gates: FeatureGate,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureGate {
    pub rag_enabled: bool,
    pub mcp_enabled: bool,
    pub spotify_enabled: bool,
    pub background_tasks_enabled: bool,
    pub predictive_preload_enabled: bool,
    pub adaptive_memory_enabled: bool,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceBudget {
    pub interactive_max_tokens: usize,
    pub background_max_tokens: usize,
    pub retrieval_candidate_limit: usize,
    pub interactive_max_concurrency: usize,
    pub background_max_concurrency: usize,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrchestratedRequest {
    pub task_type: String,
    pub qos: String,
    pub query_category: String,
    pub max_tokens_hint: usize,
    pub context_window_hint: usize,
    pub pressure_level: String,
    pub defer_background: bool,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProfileSnapshot {
    pub detected_tier: String,
    pub active_tier: String,
    pub low_safe_startup: bool,
    pub pressure_level: String,
    pub feature_gates: FeatureGate,
    pub service_budget: ServiceBudget,
    pub policy: RuntimePolicy,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceHealthSnapshot {
    pub query_classifier_ready: bool,
    pub usage_learner_ready: bool,
    pub adaptive_memory_enabled: bool,
    pub predictive_preload_enabled: bool,
    pub recent_query_samples: usize,
    pub memory_manager: MemoryManagerStats,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OptimizationStatsSnapshot {
    pub learning: LearningStats,
    pub memory_manager: MemoryManagerStats,
    pub suggested_optimizations: Vec<String>,
    pub recommended_context_window: usize,
    pub preferred_model: Option<String>,
    pub fallback_model: Option<String>,
    pub classification_distribution: std::collections::HashMap<String, u64>,
}

impl RuntimeOrchestratorService {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        runtime_governor: RuntimeGovernorService,
        hardware_service: Arc<HardwareService>,
        query_classifier: Arc<SmartQueryClassifier>,
        usage_learner: Arc<UsageLearner>,
        adaptive_memory: Arc<AdaptiveMemoryManager>,
        predictive_preloader: Arc<PredictivePreloader>,
        detected_tier: DeviceTier,
        active_tier: DeviceTier,
        feature_gates: FeatureGate,
    ) -> Self {
        Self {
            runtime_governor,
            hardware_service,
            query_classifier,
            usage_learner,
            adaptive_memory,
            predictive_preloader,
            detected_tier,
            active_tier,
            feature_gates,
        }
    }

    pub async fn start_background_loops(&self) {
        self.adaptive_memory
            .set_enabled(self.feature_gates.adaptive_memory_enabled);
        self.predictive_preloader
            .set_enabled(self.feature_gates.predictive_preload_enabled);

        self.adaptive_memory.start_memory_monitor().await;
        self.predictive_preloader.start_background_predictor().await;
    }

    pub async fn plan_request(
        &self,
        user_id: &str,
        content: &str,
        explicit_task_type: Option<&str>,
        explicit_qos: Option<&str>,
        allow_background_defer: bool,
    ) -> Result<OrchestratedRequest, AppError> {
        let category = self.query_classifier.classify(content).await;
        let category_str = category.as_str().to_string();

        self.usage_learner
            .record_query(content, &category_str)
            .await;
        self.usage_learner.record_time_usage().await;
        self.usage_learner.record_session_start().await;
        self.predictive_preloader.record_query(content);

        let policy = self.runtime_governor.get_policy(Some(user_id)).await?;
        let stats = self.runtime_governor.current_stats();
        let pressure = self.runtime_governor.classify_pressure(&stats, &policy);

        let task_type = explicit_task_type
            .map(sanitize_task_type)
            .unwrap_or_else(|| infer_task_type_from_category(&category));
        let qos = explicit_qos
            .map(sanitize_qos)
            .unwrap_or_else(|| infer_qos_from_category(&category));

        let mut max_tokens = self.query_classifier.suggest_max_tokens(&category).await;
        let budget = self.compute_budget(&policy);
        max_tokens = min(max_tokens, budget.interactive_max_tokens);

        let pressure_factor = match pressure.as_str() {
            "critical" => 0.45,
            "high" => 0.65,
            "warm" => 0.85,
            _ => 1.0,
        };
        max_tokens = ((max_tokens as f64) * pressure_factor).round() as usize;
        max_tokens = max_tokens.clamp(96, budget.interactive_max_tokens);

        let context_window_hint = self.query_classifier.context_window_hint().await;
        let defer_background = allow_background_defer
            && policy.defer_background_under_pressure
            && matches!(pressure.as_str(), "high" | "critical");

        Ok(OrchestratedRequest {
            task_type,
            qos,
            query_category: category_str,
            max_tokens_hint: max_tokens,
            context_window_hint,
            pressure_level: pressure,
            defer_background,
        })
    }

    pub async fn maybe_preload_model(&self, model_path: &str, profile: &SystemProfile) {
        if !self.feature_gates.predictive_preload_enabled {
            return;
        }
        self.predictive_preloader
            .maybe_preload(model_path, profile)
            .await;
    }

    pub async fn record_model_usage(&self, model_name: &str) {
        self.usage_learner.record_model_usage(model_name).await;
    }

    pub async fn get_runtime_profile(
        &self,
        user_id: Option<&str>,
    ) -> Result<RuntimeProfileSnapshot, AppError> {
        let policy = self.runtime_governor.get_policy(user_id).await?;
        let budget = self.compute_budget(&policy);
        let pressure = self
            .runtime_governor
            .classify_pressure(&self.runtime_governor.current_stats(), &policy);

        Ok(RuntimeProfileSnapshot {
            detected_tier: self.detected_tier.to_string(),
            active_tier: self.active_tier.to_string(),
            low_safe_startup: matches!(self.active_tier, DeviceTier::Low | DeviceTier::Minimal),
            pressure_level: pressure,
            feature_gates: self.feature_gates.clone(),
            service_budget: budget,
            policy,
        })
    }

    pub async fn get_service_health(&self) -> ServiceHealthSnapshot {
        ServiceHealthSnapshot {
            query_classifier_ready: true,
            usage_learner_ready: true,
            adaptive_memory_enabled: self.adaptive_memory.is_enabled(),
            predictive_preload_enabled: self.predictive_preloader.is_enabled(),
            recent_query_samples: self.predictive_preloader.sample_count().await,
            memory_manager: self.adaptive_memory.get_stats(),
        }
    }

    pub async fn get_optimization_stats(&self) -> OptimizationStatsSnapshot {
        let learning = self.usage_learner.get_learning_stats().await;
        let memory_manager = self.adaptive_memory.get_stats();
        let suggested_optimizations = self.usage_learner.suggest_optimizations().await;
        let recommended_context_window = self.usage_learner.get_optimal_context_window().await;
        let (preferred_model, fallback_model) = self.usage_learner.get_model_recommendation().await;
        let classification_distribution = self.query_classifier.distribution_snapshot().await;

        OptimizationStatsSnapshot {
            learning,
            memory_manager,
            suggested_optimizations,
            recommended_context_window,
            preferred_model,
            fallback_model,
            classification_distribution,
        }
    }

    fn compute_budget(&self, policy: &RuntimePolicy) -> ServiceBudget {
        let stats = self.hardware_service.live_stats();
        let free_ram_mb = stats.memory_total_mb.saturating_sub(stats.memory_used_mb);

        // 1. Adaptive Context Windows scaling. (Base: 512 tokens. +100 tokens per 1GB of free RAM)
        let dynamic_max_tokens = 512 + ((free_ram_mb / 1024) * 100) as usize;

        let tier_budget = match self.active_tier {
            DeviceTier::Potato => ServiceBudget {
                interactive_max_tokens: min(dynamic_max_tokens, 128),
                background_max_tokens: 64,
                retrieval_candidate_limit: 4,
                interactive_max_concurrency: 1,
                background_max_concurrency: 1,
            },
            DeviceTier::Minimal => ServiceBudget {
                interactive_max_tokens: min(dynamic_max_tokens, 256),
                background_max_tokens: 128,
                retrieval_candidate_limit: 12,
                interactive_max_concurrency: 1,
                background_max_concurrency: 1,
            },
            DeviceTier::Low => ServiceBudget {
                interactive_max_tokens: min(dynamic_max_tokens, 512),
                background_max_tokens: 192,
                retrieval_candidate_limit: 24,
                interactive_max_concurrency: 1,
                background_max_concurrency: 1,
            },
            DeviceTier::Medium => ServiceBudget {
                interactive_max_tokens: min(dynamic_max_tokens, 1024),
                background_max_tokens: 320,
                retrieval_candidate_limit: 48,
                interactive_max_concurrency: 2,
                background_max_concurrency: 1,
            },
            DeviceTier::High => ServiceBudget {
                interactive_max_tokens: min(dynamic_max_tokens, 4096),
                background_max_tokens: 1024,
                retrieval_candidate_limit: 96,
                interactive_max_concurrency: 3,
                background_max_concurrency: 2,
            },
            DeviceTier::Ultra => ServiceBudget {
                interactive_max_tokens: min(dynamic_max_tokens, 16384),
                background_max_tokens: 4096,
                retrieval_candidate_limit: 256,
                interactive_max_concurrency: 5,
                background_max_concurrency: 4,
            },
        };

        let pressure = self
            .runtime_governor
            .classify_pressure(&stats, policy);
        let factor = match pressure.as_str() {
            "critical" => 0.5,
            "high" => 0.7,
            "warm" => 0.9,
            _ => 1.0,
        };

        // Sparse RAG Retrieval: when pressure is critical, violently prune candidate count.
        let dynamic_retrieval_limit = if pressure == "critical" {
            min(tier_budget.retrieval_candidate_limit, 3) 
        } else {
            tier_budget.retrieval_candidate_limit
        };

        ServiceBudget {
            interactive_max_tokens: min(
                ((tier_budget.interactive_max_tokens as f64) * factor).round() as usize,
                policy.interactive_max_tokens,
            )
            .max(96),
            background_max_tokens: min(
                ((tier_budget.background_max_tokens as f64) * factor).round() as usize,
                policy.background_max_tokens,
            )
            .max(64),
            retrieval_candidate_limit: min(
                dynamic_retrieval_limit,
                policy.retrieval_candidate_limit,
            ),
            interactive_max_concurrency: min(
                tier_budget.interactive_max_concurrency,
                policy.interactive_max_concurrency,
            )
            .max(1),
            background_max_concurrency: min(
                tier_budget.background_max_concurrency,
                policy.background_max_concurrency,
            )
            .max(1),
        }
    }
}

fn sanitize_task_type(task_type: &str) -> String {
    match task_type.trim().to_lowercase().as_str() {
        "chat" | "code" | "reasoning" | "retrieval_heavy" | "tool_heavy" => {
            task_type.trim().to_lowercase()
        }
        _ => "chat".to_string(),
    }
}

fn sanitize_qos(qos: &str) -> String {
    match qos.trim().to_lowercase().as_str() {
        "fast" | "balanced" | "max_quality" => qos.trim().to_lowercase(),
        _ => "balanced".to_string(),
    }
}

fn infer_task_type_from_category(category: &QueryCategory) -> String {
    match category {
        QueryCategory::Code => "code".to_string(),
        QueryCategory::Math | QueryCategory::Analytical | QueryCategory::Complex => {
            "reasoning".to_string()
        }
        QueryCategory::Creative => "chat".to_string(),
        QueryCategory::Simple | QueryCategory::Medium => "chat".to_string(),
        QueryCategory::Summarization | QueryCategory::Translation => "reasoning".to_string(),
    }
}

fn infer_qos_from_category(category: &QueryCategory) -> String {
    match category {
        QueryCategory::Simple => "fast".to_string(),
        QueryCategory::Medium | QueryCategory::Creative | QueryCategory::Summarization | QueryCategory::Translation => "balanced".to_string(),
        QueryCategory::Complex
        | QueryCategory::Code
        | QueryCategory::Math
        | QueryCategory::Analytical => "max_quality".to_string(),
    }
}
