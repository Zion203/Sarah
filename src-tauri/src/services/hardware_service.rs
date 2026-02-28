use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use sysinfo::{Disks, System};
use uuid::Uuid;

use crate::db::models::{BenchmarkResult, SystemProfile};
use crate::error::AppError;
use crate::repositories::settings_repo::SettingsRepo;
use crate::repositories::system_repo::SystemRepo;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PerformanceMode {
    Max,
    Balanced,
    Multitasking,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum DeviceTier {
    Ultra,
    High,
    Medium,
    Low,
    Minimal,
    Potato,
}

impl std::fmt::Display for DeviceTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeviceTier::Ultra => write!(f, "ultra"),
            DeviceTier::High => write!(f, "high"),
            DeviceTier::Medium => write!(f, "medium"),
            DeviceTier::Low => write!(f, "low"),
            DeviceTier::Minimal => write!(f, "minimal"),
            DeviceTier::Potato => write!(f, "potato"),
        }
    }
}

impl SystemProfile {
    pub fn classify(&self) -> DeviceTier {
        // Expand RAM baseline to 64GB
        let ram_score = (self.total_ram_mb as f32 / 64000.0).min(1.0);
        
        // Bonus for modern vector instructions
        let avx_bonus = if self.supports_avx512 > 0 { 1.2 } else if self.supports_avx2 > 0 { 1.05 } else { 1.0 };
        let cpu_score = ((self.cpu_threads as f32 / 16.0).min(1.0) * avx_bonus).min(1.0);
        
        // Unified Memory / Dynamic VRAM mapping
        let mut vram = self.gpu_vram_mb.unwrap_or(0);
        if let Some(backend) = &self.gpu_backend {
            if backend == "metal" || (backend == "cuda" && vram == 0) {
                // Approximate Unified Memory as half of total system RAM
                vram = (self.total_ram_mb / 2) as i64;
            }
        }
        
        // VRAM baseline pushed to 16GB
        let gpu_score = (vram as f32 / 16384.0).min(1.0);

        let total = (ram_score * 0.35 + cpu_score * 0.25 + gpu_score * 0.40) * 100.0;
        let abs_ram_gb = self.total_ram_mb / 1024;

        if total >= 80.0 && abs_ram_gb >= 60 && vram >= 15000 {
            DeviceTier::Ultra
        } else if total >= 55.0 && abs_ram_gb >= 30 && vram >= 7000 {
            DeviceTier::High
        } else if total >= 30.0 && abs_ram_gb >= 14 && vram >= 3000 {
            DeviceTier::Medium
        } else if abs_ram_gb >= 7 {
            DeviceTier::Low
        } else if abs_ram_gb >= 3 {
            DeviceTier::Minimal
        } else {
            DeviceTier::Potato
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadDecision {
    LoadNow,
    BackgroundOnly,
    Defer,
    Skip,
}

pub struct HardwareService {
    system_repo: SystemRepo,
    settings_repo: SettingsRepo,
    last_stats: std::sync::Arc<std::sync::Mutex<crate::db::models::LiveSystemStats>>,
    last_check: AtomicU64,
}

impl Clone for HardwareService {
    fn clone(&self) -> Self {
        Self {
            system_repo: self.system_repo.clone(),
            settings_repo: self.settings_repo.clone(),
            last_stats: std::sync::Arc::clone(&self.last_stats),
            last_check: AtomicU64::new(self.last_check.load(Ordering::Relaxed)),
        }
    }
}

impl HardwareService {
    pub fn new(system_repo: SystemRepo, settings_repo: SettingsRepo) -> Self {
        Self {
            system_repo,
            settings_repo,
            last_stats: std::sync::Arc::new(std::sync::Mutex::new(
                crate::db::models::LiveSystemStats {
                    cpu_usage_pct: 0.0,
                    memory_used_mb: 0,
                    memory_total_mb: 0,
                    process_count: 0,
                    gpu_name: None,
                    gpu_usage_pct: None,
                    ..Default::default()
                },
            )),
            last_check: AtomicU64::new(0),
        }
    }

    pub async fn detect_hardware(&self) -> Result<SystemProfile, AppError> {
        let mut system = System::new_all();
        system.refresh_all();

        let cpu_brand = system
            .cpus()
            .first()
            .map(|cpu| cpu.brand().to_string())
            .unwrap_or_else(|| "Unknown CPU".to_string());

        let cpu_cores = System::physical_core_count().unwrap_or(system.cpus().len()) as i64;
        let cpu_threads = system.cpus().len() as i64;
        let cpu_frequency_mhz = system.cpus().first().map(|cpu| cpu.frequency() as i64);

        let total_ram_mb = (system.total_memory() / 1024 / 1024) as i64;
        let available_ram_mb = Some((system.available_memory() / 1024 / 1024) as i64);

        let (
            gpu_name,
            gpu_vendor,
            gpu_vram_mb,
            gpu_backend,
            supports_cuda,
            supports_metal,
            supports_vulkan,
        ) = self.detect_gpu().unwrap_or_else(|_| {
            (
                None,
                Some("none".to_string()),
                None,
                Some("cpu".to_string()),
                0,
                0,
                0,
            )
        });

        let disks = Disks::new_with_refreshed_list();
        let storage_total: u64 = disks.list().iter().map(|d| d.total_space()).sum();
        let storage_available: u64 = disks.list().iter().map(|d| d.available_space()).sum();

        let os_name = System::name().unwrap_or_else(|| "Unknown OS".to_string());
        let os_version = System::os_version().unwrap_or_else(|| "Unknown".to_string());

        let supports_avx2 = if cfg!(any(target_arch = "x86", target_arch = "x86_64")) {
            if std::is_x86_feature_detected!("avx2") {
                1
            } else {
                0
            }
        } else {
            0
        };

        let supports_avx512 = if cfg!(any(target_arch = "x86", target_arch = "x86_64")) {
            if std::is_x86_feature_detected!("avx512f") {
                1
            } else {
                0
            }
        } else {
            0
        };

        let mut profile = SystemProfile {
            id: Uuid::new_v4().to_string(),
            cpu_brand,
            cpu_cores,
            cpu_threads,
            cpu_frequency_mhz,
            total_ram_mb,
            available_ram_mb,
            gpu_name,
            gpu_vendor,
            gpu_vram_mb,
            gpu_backend,
            storage_total_gb: Some((storage_total / 1024 / 1024 / 1024) as i64),
            storage_available_gb: Some((storage_available / 1024 / 1024 / 1024) as i64),
            storage_type: None,
            os_name,
            os_version,
            os_arch: std::env::consts::ARCH.to_string(),
            platform: std::env::consts::OS.to_string(),
            benchmark_tokens_per_sec: None,
            benchmark_embed_ms: None,
            capability_score: None,
            supports_cuda,
            supports_metal,
            supports_vulkan,
            supports_avx2,
            supports_avx512,
            last_scan_at: chrono::Utc::now().to_rfc3339(),
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        };

        let score = self.compute_capability_score(&profile);
        profile.capability_score = Some(score as f64);

        self.system_repo.upsert_profile(profile).await
    }

    pub async fn run_benchmark(&self) -> Result<BenchmarkResult, AppError> {
        let mut profile = self
            .system_repo
            .get_current_profile()
            .await?
            .ok_or_else(|| AppError::NotFound {
                entity: "system_profile".to_string(),
                id: "current".to_string(),
            })?;

        let token_start = Instant::now();
        let mut acc = 0u64;
        for i in 0..1_500_000 {
            acc = acc.wrapping_add(i);
        }
        let token_duration = token_start.elapsed();
        let tokens_per_sec = 50.0 / token_duration.as_secs_f64().max(0.001);

        let embed_start = Instant::now();
        for _ in 0..10 {
            let mut tmp = vec![0f32; 1024];
            for (idx, slot) in tmp.iter_mut().enumerate() {
                *slot = ((idx as f32 * 0.001) + (acc as f32 * 0.000001)).sin();
            }
        }
        let embed_duration = embed_start.elapsed();
        let embed_ms = embed_duration.as_millis() as f64 / 10.0;

        profile.benchmark_tokens_per_sec = Some(tokens_per_sec);
        profile.benchmark_embed_ms = Some(embed_ms);
        profile.capability_score = Some(self.compute_capability_score(&profile) as f64);

        let updated = self.system_repo.upsert_profile(profile).await?;

        Ok(BenchmarkResult {
            profile_id: updated.id,
            tokens_per_sec,
            embed_ms,
        })
    }

    pub fn compute_capability_score(&self, profile: &SystemProfile) -> f32 {
        let cpu_score = ((profile.cpu_threads as f32 / 16.0).min(1.0) * 0.6)
            + ((profile.cpu_frequency_mhz.unwrap_or(2200) as f32 / 4500.0).min(1.0) * 0.4);

        let ram_score = (profile.total_ram_mb as f32 / 32768.0).min(1.0);

        let gpu_raw = profile.gpu_vram_mb.unwrap_or(0) as f32;
        let gpu_score = if gpu_raw <= 0.0 {
            0.2
        } else {
            (gpu_raw / 12288.0).min(1.0)
        };

        (cpu_score * 0.3) + (ram_score * 0.25) + (gpu_score * 0.45)
    }

    pub fn suggest_n_gpu_layers(&self, profile: &SystemProfile, model_size_gb: f32) -> i32 {
        let vram_gb = profile.gpu_vram_mb.unwrap_or(0) as f32 / 1024.0;
        if vram_gb <= 0.0 {
            return 0;
        }

        if vram_gb > model_size_gb * 1.1 {
            return -1;
        }

        let per_layer_size_gb = (model_size_gb / 32.0).max(0.05);
        (vram_gb / per_layer_size_gb).floor() as i32
    }

    pub fn can_load_model(&self, required_ram_mb: i64) -> bool {
        let stats = self.live_stats();
        if stats.memory_total_mb == 0 {
            return true;
        }
        let available = (stats.memory_total_mb - stats.memory_used_mb) as f64;
        let headroom = (stats.memory_total_mb as f64 * 0.25).max(2048.0);
        available > (required_ram_mb as f64 + headroom)
    }

    pub fn should_load_model(&self, model_size_mb: i64) -> LoadDecision {
        let stats = self.live_stats();
        let memory_pct = if stats.memory_total_mb > 0 {
            stats.memory_used_mb as f64 / stats.memory_total_mb as f64
        } else {
            0.0
        };

        if stats.cpu_usage_pct >= 90.0 || memory_pct >= 0.90 {
            return LoadDecision::Defer;
        }
        if stats.cpu_usage_pct >= 70.0 || memory_pct >= 0.75 {
            return LoadDecision::BackgroundOnly;
        }
        if self.can_load_model(model_size_mb) {
            LoadDecision::LoadNow
        } else {
            LoadDecision::Skip
        }
    }

    pub async fn get_performance_mode(&self, user_id: Option<&str>) -> PerformanceMode {
        match self.settings_repo.get_setting(user_id, "app_performance", "mode").await {
            Ok(Some(setting)) => match setting.value.as_str() {
                "max" => PerformanceMode::Max,
                "multitasking" => PerformanceMode::Multitasking,
                _ => PerformanceMode::Balanced,
            },
            _ => PerformanceMode::Balanced,
        }
    }

    pub async fn get_tier_config(&self, tier: DeviceTier, user_id: Option<&str>) -> TierConfig {
        let mode = self.get_performance_mode(user_id).await;
        
        // In multitasking eco mode, we brutally downgrade the capabilities to preserve RAM and CPU
        let effective_tier = if mode == PerformanceMode::Multitasking {
            match tier {
                DeviceTier::Ultra | DeviceTier::High => DeviceTier::Low,
                DeviceTier::Medium | DeviceTier::Low => DeviceTier::Minimal,
                DeviceTier::Minimal | DeviceTier::Potato => DeviceTier::Potato,
            }
        } else {
            tier
        };

        let mut config = match effective_tier {
            DeviceTier::Ultra => TierConfig {
                embedding_model: Some("bge-small-en-v1.5".to_string()),
                reranker_model: Some("bge-reranker-base".to_string()),
                max_context: 16384,
                embed_cache_capacity: 100_000,
                session_cache_capacity: 8192,
                settings_cache_capacity: 1024,
                background_tasks_enabled: true,
                auto_load_model: true,
            },
            DeviceTier::High => TierConfig {
                embedding_model: Some("bge-small-en-v1.5".to_string()),
                reranker_model: Some("bge-reranker-base".to_string()),
                max_context: 8192,
                embed_cache_capacity: 50_000,
                session_cache_capacity: 4096,
                settings_cache_capacity: 512,
                background_tasks_enabled: true,
                auto_load_model: true,
            },
            DeviceTier::Medium => TierConfig {
                embedding_model: Some("bge-small-en-v1.5".to_string()),
                reranker_model: Some("bge-reranker-base".to_string()),
                max_context: 4096,
                embed_cache_capacity: 25_000,
                session_cache_capacity: 2048,
                settings_cache_capacity: 256,
                background_tasks_enabled: true,
                auto_load_model: true,
            },
            DeviceTier::Low => TierConfig {
                embedding_model: Some("bge-small-en-v1.5".to_string()),
                reranker_model: Some("bge-reranker-base".to_string()),
                max_context: 2048,
                embed_cache_capacity: 10_000,
                session_cache_capacity: 512,
                settings_cache_capacity: 128,
                background_tasks_enabled: true,
                auto_load_model: false,
            },
            DeviceTier::Minimal => TierConfig {
                embedding_model: None,
                reranker_model: None,
                max_context: 1024,
                embed_cache_capacity: 1_000,
                session_cache_capacity: 128,
                settings_cache_capacity: 32,
                background_tasks_enabled: false,
                auto_load_model: false,
            },
            DeviceTier::Potato => TierConfig {
                embedding_model: None,
                reranker_model: None,
                max_context: 512,
                embed_cache_capacity: 100,
                session_cache_capacity: 32,
                settings_cache_capacity: 16,
                background_tasks_enabled: false,
                auto_load_model: false,
            },
        };

        if mode == PerformanceMode::Multitasking {
            config.auto_load_model = false;
            config.background_tasks_enabled = false;
            // Brutally hack down cache limits to save RAM by 70%+
            config.max_context = std::cmp::min(config.max_context, 1024);
            config.embed_cache_capacity /= 10;
            config.session_cache_capacity /= 4;
        }

        config
    }

    pub fn live_stats(&self) -> crate::db::models::LiveSystemStats {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let last = self.last_check.load(Ordering::Relaxed);
        if now - last < 5 {
            if let Ok(stats) = self.last_stats.lock() {
                return stats.clone();
            }
        }

        let mut system = System::new_all();
        system.refresh_all();

        let cpu_usage_pct = system.global_cpu_usage();
        let memory_total_mb = system.total_memory() / 1024 / 1024;
        let memory_used_mb =
            memory_total_mb.saturating_sub(system.available_memory() / 1024 / 1024);

        let stats = crate::db::models::LiveSystemStats {
            cpu_usage_pct,
            memory_used_mb,
            memory_total_mb,
            process_count: system.processes().len(),
            gpu_name: None,
            gpu_usage_pct: None,
        };

        if let Ok(mut guard) = self.last_stats.lock() {
            *guard = stats.clone();
        }
        self.last_check.store(now, Ordering::Relaxed);

        stats
    }

    fn detect_gpu(
        &self,
    ) -> Result<
        (
            Option<String>,
            Option<String>,
            Option<i64>,
            Option<String>,
            i64,
            i64,
            i64,
        ),
        AppError,
    > {
        #[allow(unused_mut)]
        let mut nvidia_name: Option<String> = None;
        #[allow(unused_mut)]
        let mut nvidia_vram: Option<i64> = None;

        #[cfg(feature = "nvidia")]
        {
            if let Ok(nvml) = nvml_wrapper::Nvml::init() {
                if let Ok(device) = nvml.device_by_index(0) {
                    if let Ok(name) = device.name() {
                        nvidia_name = Some(name);
                    }
                    if let Ok(mem_info) = device.memory_info() {
                        nvidia_vram = Some((mem_info.total / 1024 / 1024) as i64);
                    }
                }
            }
        }

        if nvidia_name.is_some() {
            return Ok((
                nvidia_name,
                Some("nvidia".to_string()),
                nvidia_vram,
                Some("cuda".to_string()),
                1,
                0,
                1,
            ));
        }

        Ok((
            None,
            Some("none".to_string()),
            None,
            Some("cpu".to_string()),
            0,
            0,
            0,
        ))
    }
}

pub struct TierConfig {
    pub embedding_model: Option<String>,
    pub reranker_model: Option<String>,
    pub max_context: usize,
    pub embed_cache_capacity: u64,
    pub session_cache_capacity: u64,
    pub settings_cache_capacity: u64,
    pub background_tasks_enabled: bool,
    pub auto_load_model: bool,
}

impl Clone for TierConfig {
    fn clone(&self) -> Self {
        Self {
            embedding_model: self.embedding_model.clone(),
            reranker_model: self.reranker_model.clone(),
            max_context: self.max_context,
            embed_cache_capacity: self.embed_cache_capacity,
            session_cache_capacity: self.session_cache_capacity,
            settings_cache_capacity: self.settings_cache_capacity,
            background_tasks_enabled: self.background_tasks_enabled,
            auto_load_model: self.auto_load_model,
        }
    }
}
