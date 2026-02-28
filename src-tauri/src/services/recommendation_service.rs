use uuid::Uuid;

use crate::db::models::{ModelRecommendation, SystemProfile};
use crate::error::AppError;
use crate::repositories::analytics_repo::AnalyticsRepo;
use crate::repositories::model_repo::ModelRepo;
use crate::services::hardware_service::PerformanceMode;

#[derive(Clone)]
pub struct RecommendationService {
    model_repo: ModelRepo,
    analytics_repo: AnalyticsRepo,
}

impl RecommendationService {
    pub fn new(model_repo: ModelRepo, analytics_repo: AnalyticsRepo) -> Self {
        Self {
            model_repo,
            analytics_repo,
        }
    }

    pub async fn recompute(
        &self,
        profile: &SystemProfile,
        mode: PerformanceMode,
    ) -> Result<Vec<ModelRecommendation>, AppError> {
        let candidates = self
            .model_repo
            .list_compatible_models(profile.total_ram_mb, profile.gpu_vram_mb.unwrap_or(0))
            .await?;

        let mut recs = Vec::new();
        for model in &candidates {
            let ram_fit =
                (profile.total_ram_mb as f64 / model.recommended_ram_mb.max(1) as f64).min(1.0);
            let vram_fit = if model.min_vram_mb <= 0 {
                1.0
            } else {
                (profile.gpu_vram_mb.unwrap_or(0) as f64 / model.min_vram_mb as f64).min(1.0)
            };

            let perf_fit = model
                .avg_tokens_per_sec
                .map(|tps| (tps / 45.0).clamp(0.25, 1.0))
                .unwrap_or(0.55);

            let mut score = (ram_fit * 0.40) + (vram_fit * 0.35) + (perf_fit * 0.25);
            
            if mode == PerformanceMode::Multitasking && model.recommended_ram_mb > 3500 {
                // Heavily penalize large models in Eco Multitasking mode. We want tiny 1.5B/3B models.
                score *= 0.3;
            } else if mode == PerformanceMode::Multitasking && model.recommended_ram_mb <= 3500 {
                // Give a bump to tiny models so they bubble to the top.
                score *= 1.25;
            }

            let tier = if score >= 0.88 {
                "optimal"
            } else if score >= 0.65 {
                "compatible"
            } else if score >= 0.45 {
                "stretch"
            } else {
                "incompatible"
            };

            recs.push(ModelRecommendation {
                id: Uuid::new_v4().to_string(),
                system_profile_id: profile.id.clone(),
                model_id: model.id.clone(),
                recommendation_tier: tier.to_string(),
                score,
                reasoning: format!(
                    "RAM fit {:.2}, VRAM fit {:.2}, perf fit {:.2}, tier {}",
                    ram_fit, vram_fit, perf_fit, tier
                ),
                performance_estimate: Some(
                    serde_json::json!({
                        "tokens_per_sec": ((score * 40.0) + (perf_fit * 15.0)).round() as i64,
                        "load_time_ms": ((1.0 - score).max(0.05) * 4500.0).round() as i64,
                    })
                    .to_string(),
                ),
                energy_rating: Some(
                    if score > 0.85 {
                        "A"
                    } else if score > 0.7 {
                        "B"
                    } else if score > 0.55 {
                        "C"
                    } else {
                        "D"
                    }
                    .to_string(),
                ),
                is_primary_recommendation: 0,
                computed_at: chrono::Utc::now().to_rfc3339(),
                created_at: chrono::Utc::now().to_rfc3339(),
            });
        }

        recs.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for (idx, rec) in recs.iter_mut().enumerate() {
            rec.is_primary_recommendation = if idx == 0 { 1 } else { 0 };
        }

        self.analytics_repo
            .replace_recommendations(&profile.id, &recs)
            .await?;

        Ok(recs)
    }

    pub async fn get_cached(&self, profile_id: &str) -> Result<Vec<ModelRecommendation>, AppError> {
        self.analytics_repo.get_recommendations(profile_id).await
    }
}
