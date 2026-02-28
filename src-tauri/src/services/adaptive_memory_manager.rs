use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;
use tokio::time::interval;

use crate::services::hardware_service::HardwareService;
use crate::services::inference_service::InferenceService;

#[derive(Clone)]
pub struct AdaptiveMemoryManager {
    hardware: Arc<HardwareService>,
    inference: Arc<InferenceService>,
    enabled: Arc<AtomicBool>,
    memory_threshold_high: f64,
    memory_threshold_low: f64,
    cpu_threshold_high: f64,
    cpu_threshold_low: f64,
    last_unload_time: Arc<Mutex<Option<Instant>>>,
    unload_cooldown_secs: u64,
    total_unloads: Arc<AtomicU64>,
    total_loads: Arc<AtomicU64>,
}

impl AdaptiveMemoryManager {
    pub fn new(hardware: Arc<HardwareService>, inference: Arc<InferenceService>) -> Self {
        Self {
            hardware,
            inference,
            enabled: Arc::new(AtomicBool::new(true)),
            memory_threshold_high: 0.85, // Drop to 85% instead of 88%
            memory_threshold_low: 0.65,
            cpu_threshold_high: 85.0,
            cpu_threshold_low: 50.0,
            last_unload_time: Arc::new(Mutex::new(None)),
            unload_cooldown_secs: 30, // Drop from 120s down to 30s for aggressive pruning
            total_unloads: Arc::new(AtomicU64::new(0)),
            total_loads: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn with_thresholds(
        hardware: Arc<HardwareService>,
        inference: Arc<InferenceService>,
        memory_high: f64,
        memory_low: f64,
        cpu_high: f64,
        cpu_low: f64,
    ) -> Self {
        Self {
            hardware,
            inference,
            enabled: Arc::new(AtomicBool::new(true)),
            memory_threshold_high: memory_high.clamp(0.50, 0.98),
            memory_threshold_low: memory_low.clamp(0.20, 0.95),
            cpu_threshold_high: cpu_high.clamp(40.0, 99.0),
            cpu_threshold_low: cpu_low.clamp(10.0, 90.0),
            last_unload_time: Arc::new(Mutex::new(None)),
            unload_cooldown_secs: 90,
            total_unloads: Arc::new(AtomicU64::new(0)),
            total_loads: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub async fn check_and_manage(&self) -> MemoryAction {
        if !self.is_enabled() {
            return MemoryAction::None;
        }

        let stats = self.hardware.live_stats();
        let memory_pct = if stats.memory_total_mb > 0 {
            stats.memory_used_mb as f64 / stats.memory_total_mb as f64
        } else {
            0.0
        };
        let cpu_pct = stats.cpu_usage_pct as f64;

        let is_high_pressure =
            memory_pct >= self.memory_threshold_high || cpu_pct >= self.cpu_threshold_high;
        let is_low_pressure =
            memory_pct <= self.memory_threshold_low && cpu_pct <= self.cpu_threshold_low;

        if is_high_pressure {
            return self.maybe_unload_model().await;
        }
        if is_low_pressure {
            return self.maybe_load_model().await;
        }

        MemoryAction::None
    }

    async fn maybe_unload_model(&self) -> MemoryAction {
        let mut last_unload = self.last_unload_time.lock().await;
        if let Some(previous) = *last_unload {
            if previous.elapsed() < Duration::from_secs(self.unload_cooldown_secs) {
                return MemoryAction::None;
            }
        }

        let active_model = self.inference.get_active_model_info().await;
        if let Some(model) = active_model {
            tracing::info!(
                "Adaptive memory manager unloading model under pressure: {}",
                model.path
            );

            if self.inference.unload_model().await.is_ok() {
                *last_unload = Some(Instant::now());
                self.total_unloads.fetch_add(1, Ordering::Relaxed);
                return MemoryAction::Unloaded(model.path);
            }
        }

        MemoryAction::None
    }

    async fn maybe_load_model(&self) -> MemoryAction {
        let last_unload = self.last_unload_time.lock().await;
        if let Some(previous) = *last_unload {
            if previous.elapsed() < Duration::from_secs(30) {
                return MemoryAction::None;
            }
        }

        // Loading is coordinated by model manager/runtime orchestrator because it needs
        // explicit model selection and hardware-aware routing.
        MemoryAction::None
    }

    pub async fn start_memory_monitor(&self) {
        if !self.is_enabled() {
            return;
        }

        let manager = self.clone();
        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(15));
            loop {
                ticker.tick().await;
                match manager.check_and_manage().await {
                    MemoryAction::Unloaded(name) => {
                        tracing::info!("Adaptive memory monitor unloaded model: {}", name);
                    }
                    MemoryAction::Loaded(name) => {
                        tracing::info!("Adaptive memory monitor loaded model: {}", name);
                    }
                    MemoryAction::None => {}
                }
            }
        });
    }

    pub fn get_stats(&self) -> MemoryManagerStats {
        MemoryManagerStats {
            enabled: self.is_enabled(),
            total_unloads: self.total_unloads.load(Ordering::Relaxed),
            total_loads: self.total_loads.load(Ordering::Relaxed),
            memory_threshold_high: self.memory_threshold_high,
            memory_threshold_low: self.memory_threshold_low,
            cpu_threshold_high: self.cpu_threshold_high,
            cpu_threshold_low: self.cpu_threshold_low,
        }
    }
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MemoryAction {
    None,
    Unloaded(String),
    Loaded(String),
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryManagerStats {
    pub enabled: bool,
    pub total_unloads: u64,
    pub total_loads: u64,
    pub memory_threshold_high: f64,
    pub memory_threshold_low: f64,
    pub cpu_threshold_high: f64,
    pub cpu_threshold_low: f64,
}
