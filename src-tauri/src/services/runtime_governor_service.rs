use sqlx::SqlitePool;
use uuid::Uuid;

use crate::db::models::{GenerationOptions, LiveSystemStats, RuntimePolicy, RuntimePolicyPatch};
use crate::error::AppError;
use crate::services::hardware_service::HardwareService;

#[derive(Clone)]
pub struct RuntimeGovernorService {
    read_pool: SqlitePool,
    write_pool: SqlitePool,
    hardware_service: HardwareService,
}

impl RuntimeGovernorService {
    pub fn new(
        read_pool: SqlitePool,
        write_pool: SqlitePool,
        hardware_service: HardwareService,
    ) -> Self {
        Self {
            read_pool,
            write_pool,
            hardware_service,
        }
    }

    pub async fn get_policy(&self, user_id: Option<&str>) -> Result<RuntimePolicy, AppError> {
        let policy = if let Some(uid) = user_id {
            let scoped = sqlx::query_scalar::<_, String>(
                "SELECT policy_json FROM runtime_policy_overrides WHERE user_id = ?1 LIMIT 1",
            )
            .bind(uid)
            .fetch_optional(&self.read_pool)
            .await?;

            if scoped.is_some() {
                scoped
            } else {
                sqlx::query_scalar::<_, String>(
                    "SELECT policy_json FROM runtime_policy_overrides WHERE user_id IS NULL LIMIT 1",
                )
                .fetch_optional(&self.read_pool)
                .await?
            }
        } else {
            sqlx::query_scalar::<_, String>(
                "SELECT policy_json FROM runtime_policy_overrides WHERE user_id IS NULL LIMIT 1",
            )
            .fetch_optional(&self.read_pool)
            .await?
        };

        let parsed = policy
            .as_deref()
            .and_then(|raw| serde_json::from_str::<RuntimePolicy>(raw).ok())
            .unwrap_or_default();
        Ok(parsed)
    }

    pub async fn set_policy(
        &self,
        user_id: Option<&str>,
        patch: RuntimePolicyPatch,
    ) -> Result<RuntimePolicy, AppError> {
        let mut next = self.get_policy(user_id).await?;
        apply_patch(&mut next, patch);

        let encoded = serde_json::to_string(&next)
            .map_err(|e| AppError::Config(format!("Invalid runtime policy JSON: {e}")))?;

        let updated = if let Some(uid) = user_id {
            sqlx::query(
                "UPDATE runtime_policy_overrides SET policy_json = ?1, version = version + 1 WHERE user_id = ?2",
            )
            .bind(&encoded)
            .bind(uid)
            .execute(&self.write_pool)
            .await?
            .rows_affected()
        } else {
            sqlx::query(
                "UPDATE runtime_policy_overrides SET policy_json = ?1, version = version + 1 WHERE user_id IS NULL",
            )
            .bind(&encoded)
            .execute(&self.write_pool)
            .await?
            .rows_affected()
        };

        if updated == 0 {
            sqlx::query(
                r#"
                INSERT INTO runtime_policy_overrides (id, user_id, policy_json, source)
                VALUES (?1, ?2, ?3, 'user')
                "#,
            )
            .bind(Uuid::new_v4().to_string())
            .bind(user_id)
            .bind(&encoded)
            .execute(&self.write_pool)
            .await?;
        }

        Ok(next)
    }

    pub fn current_stats(&self) -> LiveSystemStats {
        self.hardware_service.live_stats()
    }

    pub fn classify_pressure(&self, stats: &LiveSystemStats, policy: &RuntimePolicy) -> String {
        let mem_pct = if stats.memory_total_mb == 0 {
            0.0
        } else {
            (stats.memory_used_mb as f64 / stats.memory_total_mb as f64) * 100.0
        };
        let cpu = stats.cpu_usage_pct as f64;

        if cpu >= 95.0 || mem_pct >= 93.0 {
            return "critical".to_string();
        }
        if cpu >= policy.pressure_cpu_pct || mem_pct >= policy.pressure_memory_pct {
            return "high".to_string();
        }
        if cpu >= 70.0 || mem_pct >= 75.0 {
            return "warm".to_string();
        }
        "normal".to_string()
    }

    pub fn tune_generation(
        &self,
        base: GenerationOptions,
        policy: &RuntimePolicy,
        qos: &str,
        pressure: &str,
        is_background: bool,
    ) -> GenerationOptions {
        let lane_cap = if is_background {
            policy.background_max_tokens
        } else {
            policy.interactive_max_tokens
        };

        let qos_factor = match qos {
            "fast" => 0.72,
            "max_quality" => 1.20,
            _ => 1.0,
        };

        let pressure_factor = match pressure {
            "critical" => 0.40,
            "high" => 0.60,
            "warm" => 0.85,
            _ => 1.0,
        };

        let budget = ((lane_cap as f64) * qos_factor * pressure_factor).round() as usize;
        let mut tuned = base;
        tuned.max_tokens = tuned.max_tokens.min(lane_cap).min(budget.max(96));
        tuned.temperature = if qos == "fast" {
            0.1
        } else {
            tuned.temperature
        };
        tuned
    }
}

fn apply_patch(policy: &mut RuntimePolicy, patch: RuntimePolicyPatch) {
    if let Some(value) = patch.pressure_cpu_pct {
        policy.pressure_cpu_pct = value.clamp(50.0, 99.0);
    }
    if let Some(value) = patch.pressure_memory_pct {
        policy.pressure_memory_pct = value.clamp(50.0, 99.0);
    }
    if let Some(value) = patch.interactive_max_tokens {
        policy.interactive_max_tokens = value.clamp(96, 4096);
    }
    if let Some(value) = patch.background_max_tokens {
        policy.background_max_tokens = value.clamp(64, 2048);
    }
    if let Some(value) = patch.interactive_max_concurrency {
        policy.interactive_max_concurrency = value.clamp(1, 4);
    }
    if let Some(value) = patch.background_max_concurrency {
        policy.background_max_concurrency = value.clamp(1, 4);
    }
    if let Some(value) = patch.retrieval_candidate_limit {
        policy.retrieval_candidate_limit = value.clamp(8, 128);
    }
    if let Some(value) = patch.defer_background_under_pressure {
        policy.defer_background_under_pressure = value;
    }
}
