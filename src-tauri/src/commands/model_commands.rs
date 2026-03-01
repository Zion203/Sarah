use std::path::{Path, PathBuf};
use std::sync::Arc;

use dashmap::DashMap;
use futures::StreamExt;
use once_cell::sync::Lazy;
use tauri::{Manager, State};
use tokio::sync::OnceCell;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::db::models::{Model, ModelRecommendation, NewModel};
use crate::error::AppError;
use crate::state::AppState;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompatibilityInfo {
    pub model_id: String,
    pub compatibility_score: f64,
    pub reason: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadHandle {
    pub model_id: String,
    pub status: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub model_id: String,
    pub status: String,
    pub progress_pct: f64,
    pub bytes_downloaded: i64,
    pub bytes_total: Option<i64>,
    pub error_message: Option<String>,
    pub file_path: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NlpSetupResult {
    pub target_model_id: String,
    pub target_model_name: String,
    pub download_status: String,
    pub embedding_ready: bool,
    pub reranker_ready: bool,
}

#[derive(Debug, Clone, Copy)]
struct SeedModel {
    name: &'static str,
    display_name: &'static str,
    family: &'static str,
    parameter_count: &'static str,
    quantization: &'static str,
    context_length: i64,
    min_ram_mb: i64,
    recommended_ram_mb: i64,
    min_vram_mb: i64,
    performance_tier: &'static str,
    energy_tier: &'static str,
    download_url: &'static str,
}

const MODEL_CATALOG: &[SeedModel] = &[
    SeedModel {
        name: "tinyllama-1.1b-chat-q4_k_m",
        display_name: "TinyLlama 1.1B Chat (Q4_K_M)",
        family: "llama",
        parameter_count: "1.1B",
        quantization: "Q4_K_M",
        context_length: 2048,
        min_ram_mb: 3000,
        recommended_ram_mb: 5000,
        min_vram_mb: 0,
        performance_tier: "fast",
        energy_tier: "low",
        download_url: "https://huggingface.co/TinyLlama/TinyLlama-1.1B-Chat-v1.0-GGUF/resolve/main/tinyllama-1.1b-chat-v1.0.Q4_K_M.gguf?download=true",
    },
    SeedModel {
        name: "qwen2.5-0.5b-instruct-q4_k_m",
        display_name: "Qwen2.5 0.5B Instruct (Q4_K_M)",
        family: "qwen",
        parameter_count: "0.5B",
        quantization: "Q4_K_M",
        context_length: 32768,
        min_ram_mb: 3000,
        recommended_ram_mb: 6000,
        min_vram_mb: 0,
        performance_tier: "fast",
        energy_tier: "low",
        download_url: "https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct-GGUF/resolve/main/qwen2.5-0.5b-instruct-q4_k_m.gguf?download=true",
    },
    SeedModel {
        name: "llama-3.2-1b-instruct-q4_k_m",
        display_name: "Llama 3.2 1B Instruct (Q4_K_M)",
        family: "llama",
        parameter_count: "1B",
        quantization: "Q4_K_M",
        context_length: 131072,
        min_ram_mb: 4500,
        recommended_ram_mb: 7000,
        min_vram_mb: 0,
        performance_tier: "balanced",
        energy_tier: "medium",
        download_url: "https://huggingface.co/unsloth/Llama-3.2-1B-Instruct-GGUF/resolve/main/Llama-3.2-1B-Instruct-Q4_K_M.gguf?download=true",
    },
    SeedModel {
        name: "qwen2.5-1.5b-instruct-q4_k_m",
        display_name: "Qwen2.5 1.5B Instruct (Q4_K_M)",
        family: "qwen",
        parameter_count: "1.5B",
        quantization: "Q4_K_M",
        context_length: 32768,
        min_ram_mb: 6000,
        recommended_ram_mb: 10000,
        min_vram_mb: 0,
        performance_tier: "quality",
        energy_tier: "medium",
        download_url: "https://huggingface.co/Qwen/Qwen2.5-1.5B-Instruct-GGUF/resolve/main/qwen2.5-1.5b-instruct-q4_k_m.gguf?download=true",
    },
    SeedModel {
        name: "gemma-2-2b-it-q4_k_m",
        display_name: "Gemma 2 2B Instruct (Q4_K_M)",
        family: "gemma",
        parameter_count: "2B",
        quantization: "Q4_K_M",
        context_length: 8192,
        min_ram_mb: 7000,
        recommended_ram_mb: 11000,
        min_vram_mb: 0,
        performance_tier: "balanced",
        energy_tier: "medium",
        download_url: "https://huggingface.co/bartowski/gemma-2-2b-it-GGUF/resolve/main/gemma-2-2b-it-Q4_K_M.gguf?download=true",
    },
    SeedModel {
        name: "qwen2.5-3b-instruct-q4_k_m",
        display_name: "Qwen2.5 3B Instruct (Q4_K_M)",
        family: "qwen",
        parameter_count: "3B",
        quantization: "Q4_K_M",
        context_length: 32768,
        min_ram_mb: 8000,
        recommended_ram_mb: 13000,
        min_vram_mb: 0,
        performance_tier: "balanced",
        energy_tier: "medium",
        download_url: "https://huggingface.co/Qwen/Qwen2.5-3B-Instruct-GGUF/resolve/main/qwen2.5-3b-instruct-q4_k_m.gguf?download=true",
    },
    SeedModel {
        name: "phi-3.5-mini-instruct-q4_k_m",
        display_name: "Phi 3.5 Mini Instruct (Q4_K_M)",
        family: "phi",
        parameter_count: "3.8B",
        quantization: "Q4_K_M",
        context_length: 128000,
        min_ram_mb: 9000,
        recommended_ram_mb: 14000,
        min_vram_mb: 0,
        performance_tier: "balanced",
        energy_tier: "medium",
        download_url: "https://huggingface.co/bartowski/Phi-3.5-mini-instruct-GGUF/resolve/main/Phi-3.5-mini-instruct-Q4_K_M.gguf?download=true",
    },
    SeedModel {
        name: "llama-3.2-3b-instruct-q4_k_m",
        display_name: "Llama 3.2 3B Instruct (Q4_K_M)",
        family: "llama",
        parameter_count: "3B",
        quantization: "Q4_K_M",
        context_length: 131072,
        min_ram_mb: 9000,
        recommended_ram_mb: 14000,
        min_vram_mb: 0,
        performance_tier: "balanced",
        energy_tier: "medium",
        download_url: "https://huggingface.co/unsloth/Llama-3.2-3B-Instruct-GGUF/resolve/main/Llama-3.2-3B-Instruct-Q4_K_M.gguf?download=true",
    },
    SeedModel {
        name: "mistral-7b-instruct-v0.3-q4_k_m",
        display_name: "Mistral 7B Instruct v0.3 (Q4_K_M)",
        family: "mistral",
        parameter_count: "7B",
        quantization: "Q4_K_M",
        context_length: 32768,
        min_ram_mb: 14000,
        recommended_ram_mb: 22000,
        min_vram_mb: 0,
        performance_tier: "quality",
        energy_tier: "high",
        download_url: "https://huggingface.co/bartowski/Mistral-7B-Instruct-v0.3-GGUF/resolve/main/Mistral-7B-Instruct-v0.3-Q4_K_M.gguf?download=true",
    },
    SeedModel {
        name: "qwen2.5-7b-instruct-q4_k_m",
        display_name: "Qwen2.5 7B Instruct (Q4_K_M)",
        family: "qwen",
        parameter_count: "7B",
        quantization: "Q4_K_M",
        context_length: 32768,
        min_ram_mb: 14000,
        recommended_ram_mb: 22000,
        min_vram_mb: 0,
        performance_tier: "quality",
        energy_tier: "high",
        download_url: "https://huggingface.co/bartowski/Qwen2.5-7B-Instruct-GGUF/resolve/main/Qwen2.5-7B-Instruct-Q4_K_M.gguf?download=true",
    },
    SeedModel {
        name: "qwen2.5-coder-7b-instruct-q4_k_m",
        display_name: "Qwen2.5 Coder 7B Instruct (Q4_K_M)",
        family: "qwen",
        parameter_count: "7B",
        quantization: "Q4_K_M",
        context_length: 32768,
        min_ram_mb: 15000,
        recommended_ram_mb: 24000,
        min_vram_mb: 0,
        performance_tier: "quality",
        energy_tier: "high",
        download_url: "https://huggingface.co/bartowski/Qwen2.5-Coder-7B-Instruct-GGUF/resolve/main/Qwen2.5-Coder-7B-Instruct-Q4_K_M.gguf?download=true",
    },
    SeedModel {
        name: "qwen2.5-math-7b-instruct-q4_k_m",
        display_name: "Qwen2.5 Math 7B Instruct (Q4_K_M)",
        family: "qwen",
        parameter_count: "7B",
        quantization: "Q4_K_M",
        context_length: 32768,
        min_ram_mb: 15000,
        recommended_ram_mb: 24000,
        min_vram_mb: 0,
        performance_tier: "quality",
        energy_tier: "high",
        download_url: "https://huggingface.co/bartowski/Qwen2.5-Math-7B-Instruct-GGUF/resolve/main/Qwen2.5-Math-7B-Instruct-Q4_K_M.gguf?download=true",
    },
];

static DOWNLOAD_TRACKER: Lazy<DashMap<String, DownloadProgress>> = Lazy::new(DashMap::new);
static CATALOG_SEEDED: OnceCell<()> = OnceCell::const_new();

fn normalize_filename(url: &str, fallback_name: &str) -> String {
    let raw = url
        .split('?')
        .next()
        .and_then(|value| value.rsplit('/').next())
        .unwrap_or(fallback_name)
        .trim();

    let mut out = raw
        .chars()
        .map(|ch| match ch {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            _ => ch,
        })
        .collect::<String>();

    if !out.to_ascii_lowercase().ends_with(".gguf") {
        out.push_str(".gguf");
    }
    out
}

pub(crate) async fn ensure_catalog_seeded(state: &Arc<AppState>) -> Result<(), AppError> {
    CATALOG_SEEDED
        .get_or_try_init(|| async {
            for item in MODEL_CATALOG {
                if state.model_repo.get_by_name(item.name).await?.is_some() {
                    continue;
                }

                let new_model = NewModel {
                    name: item.name.to_string(),
                    display_name: item.display_name.to_string(),
                    family: item.family.to_string(),
                    version: None,
                    parameter_count: Some(item.parameter_count.to_string()),
                    quantization: Some(item.quantization.to_string()),
                    file_format: "gguf".to_string(),
                    file_path: None,
                    file_size_mb: None,
                    context_length: item.context_length,
                    embedding_size: None,
                    category: "chat".to_string(),
                    capabilities: r#"["chat","local"]"#.to_string(),
                    min_ram_mb: item.min_ram_mb,
                    recommended_ram_mb: item.recommended_ram_mb,
                    min_vram_mb: item.min_vram_mb,
                    performance_tier: item.performance_tier.to_string(),
                    energy_tier: item.energy_tier.to_string(),
                    download_url: Some(item.download_url.to_string()),
                    sha256_checksum: None,
                    tags: r#"["gguf","local"]"#.to_string(),
                    metadata: "{}".to_string(),
                };

                let _ = state.model_repo.insert_model(new_model).await?;
            }

            Ok::<(), AppError>(())
        })
        .await?;

    Ok(())
}

pub(crate) async fn resolve_model(
    state: &Arc<AppState>,
    model_id_or_name: &str,
) -> Result<Model, AppError> {
    if let Some(model) = state.model_repo.get_by_id(model_id_or_name).await? {
        return Ok(model);
    }

    if let Some(model) = state.model_repo.get_by_name(model_id_or_name).await? {
        return Ok(model);
    }

    Err(AppError::NotFound {
        entity: "model".to_string(),
        id: model_id_or_name.to_string(),
    })
}

async fn upsert_download_row(
    state: &Arc<AppState>,
    row_id: &str,
    model_id: &str,
    progress: &DownloadProgress,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
        INSERT INTO model_downloads (
            id, model_id, status, bytes_downloaded, bytes_total, progress_pct, started_at, error_message
        ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, datetime('now','utc'), ?7
        )
        ON CONFLICT(id) DO UPDATE SET
            status = excluded.status,
            bytes_downloaded = excluded.bytes_downloaded,
            bytes_total = excluded.bytes_total,
            progress_pct = excluded.progress_pct,
            error_message = excluded.error_message,
            completed_at = CASE WHEN excluded.status IN ('completed','failed') THEN datetime('now','utc') ELSE completed_at END
        "#,
    )
    .bind(row_id)
    .bind(model_id)
    .bind(&progress.status)
    .bind(progress.bytes_downloaded)
    .bind(progress.bytes_total)
    .bind(progress.progress_pct)
    .bind(&progress.error_message)
    .execute(state.db.write_pool())
    .await?;

    Ok(())
}

pub(crate) async fn refresh_installed_cache(state: &Arc<AppState>) -> Result<(), AppError> {
    let installed = state.model_repo.list_installed().await?;
    state
        .cache
        .model_list
        .insert("installed".to_string(), installed)
        .await;
    Ok(())
}

fn tracker_entry(model_id: &str, status: &str) -> DownloadProgress {
    DownloadProgress {
        model_id: model_id.to_string(),
        status: status.to_string(),
        progress_pct: 0.0,
        bytes_downloaded: 0,
        bytes_total: None,
        error_message: None,
        file_path: None,
    }
}

#[tauri::command]
pub async fn get_installed_models(state: State<'_, Arc<AppState>>) -> Result<Vec<Model>, AppError> {
    crate::log_info!("sarah.command", "get_installed_models invoked");
    ensure_catalog_seeded(&state).await?;

    if let Some(cached) = state.cache.model_list.get(&"installed".to_string()).await {
        return Ok(cached);
    }

    let models = state.model_repo.list_installed().await?;
    state
        .cache
        .model_list
        .insert("installed".to_string(), models.clone())
        .await;
    Ok(models)
}

#[tauri::command]
pub async fn get_model_catalog(state: State<'_, Arc<AppState>>) -> Result<Vec<Model>, AppError> {
    crate::log_info!("sarah.command", "get_model_catalog invoked");
    ensure_catalog_seeded(&state).await?;
    state.model_repo.list_all().await
}

#[tauri::command]
pub async fn run_nlp_setup(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    target_model_id: Option<String>,
) -> Result<NlpSetupResult, AppError> {
    crate::log_info!("sarah.command", "run_nlp_setup invoked");
    run_nlp_setup_inner(app, Arc::clone(&state), target_model_id, None).await
}

pub(crate) async fn run_nlp_setup_inner(
    app: tauri::AppHandle,
    state: Arc<AppState>,
    target_model_id: Option<String>,
    user_id: Option<String>,
) -> Result<NlpSetupResult, AppError> {
    ensure_catalog_seeded(&state).await?;

    // Step 1: Initialize Core Vectors (Embedding) (10%)
    let _ = state.setup_orchestrator.update_stage(user_id.as_deref(), "stage_b_core_vectors", 10.0).await;
    if let Some(emb) = &state.embedding {
        let _ = emb.ensure_initialized().await;
    }

    // Step 2: Initialize Neural Routing (Reranker) (20%)
    let _ = state.setup_orchestrator.update_stage(user_id.as_deref(), "stage_b_neural_routing", 20.0).await;
    if let Some(reranker) = &state.reranker {
        let _ = reranker.ensure_initialized().await;
    }

    // Step 3: LLM Download (starting at 30%)
    let _ = state.setup_orchestrator.update_stage(user_id.as_deref(), "stage_b_model_download", 30.0).await;


    let target = if let Some(requested) = target_model_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        resolve_model(&state, requested).await?
    } else {
        let installed = state.model_repo.list_installed().await?;
        let mut target = installed
            .iter()
            .find(|row| row.is_default == 1)
            .cloned()
            .or_else(|| installed.first().cloned());

        if target.is_none() {
            target = state
                .model_repo
                .get_by_name("qwen2.5-0.5b-instruct-q4_k_m")
                .await?;
        }

        if target.is_none() {
            target = state.model_repo.list_all().await?.into_iter().next();
        }

        target.ok_or_else(|| AppError::NotFound {
            entity: "model".to_string(),
            id: "catalog-empty".to_string(),
        })?
    };
    let target_id = target.id.clone();
    let target_name = target.display_name.clone();

    let handle = start_model_download_inner(app, Arc::clone(&state), target_id.clone()).await?;

    Ok(NlpSetupResult {
        target_model_id: target_id,
        target_model_name: target_name,
        download_status: handle.status,
        embedding_ready: true,
        reranker_ready: true,
    })
}

#[tauri::command]
pub async fn get_recommended_models(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<ModelRecommendation>, AppError> {
    crate::log_info!("sarah.command", "get_recommended_models invoked");
    ensure_catalog_seeded(&state).await?;

    let profile = state
        .hardware
        .read()
        .await
        .clone()
        .ok_or_else(|| AppError::NotFound {
            entity: "system_profile".to_string(),
            id: "current".to_string(),
        })?;

    let recs = state.recommendation.get_cached(&profile.id).await?;
    if !recs.is_empty() {
        return Ok(recs);
    }

    let mode = state.hardware_service.get_performance_mode(None).await;
    state.recommendation.recompute(&profile, mode).await
}

#[tauri::command]
pub async fn set_default_model(
    state: State<'_, Arc<AppState>>,
    model_id: String,
) -> Result<(), AppError> {
    crate::log_info!("sarah.command", "set_default_model invoked");
    state.model_repo.set_default_model(&model_id).await?;
    refresh_installed_cache(&state).await?;
    Ok(())
}

#[tauri::command]
pub async fn get_model_compatibility_score(
    state: State<'_, Arc<AppState>>,
    model_id: String,
) -> Result<CompatibilityInfo, AppError> {
    crate::log_info!("sarah.command", "get_model_compatibility_score invoked");
    ensure_catalog_seeded(&state).await?;

    let model = resolve_model(&state, &model_id).await?;
    let profile = state
        .hardware
        .read()
        .await
        .clone()
        .ok_or_else(|| AppError::NotFound {
            entity: "system_profile".to_string(),
            id: "current".to_string(),
        })?;

    let ram_score = (profile.total_ram_mb as f64 / model.recommended_ram_mb.max(1) as f64).min(1.0);
    let vram_score = if model.min_vram_mb <= 0 {
        1.0
    } else {
        (profile.gpu_vram_mb.unwrap_or(0) as f64 / model.min_vram_mb as f64).min(1.0)
    };

    let score = ram_score * 0.55 + vram_score * 0.45;
    Ok(CompatibilityInfo {
        model_id: model.id,
        compatibility_score: score,
        reason: format!("RAM {:.2}, VRAM {:.2}", ram_score, vram_score),
    })
}

#[tauri::command]
pub async fn start_model_download(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    model_id: String,
) -> Result<DownloadHandle, AppError> {
    crate::log_info!("sarah.command", "start_model_download invoked");
    start_model_download_inner(app, Arc::clone(&state), model_id).await
}

pub(crate) async fn start_model_download_inner(
    app: tauri::AppHandle,
    state: Arc<AppState>,
    model_id: String,
) -> Result<DownloadHandle, AppError> {
    ensure_catalog_seeded(&state).await?;

    let model = resolve_model(&state, &model_id).await?;
    let canonical_id = model.id.clone();

    if let Some(progress) = DOWNLOAD_TRACKER.get(&canonical_id) {
        if progress.status == "queued" || progress.status == "downloading" {
            return Ok(DownloadHandle {
                model_id: canonical_id,
                status: progress.status.clone(),
            });
        }
    }

    let model_url = model
        .download_url
        .clone()
        .ok_or_else(|| AppError::Validation {
            field: "download_url".to_string(),
            message: format!("Model {} does not have a download URL", model.display_name),
        })?;

    let models_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Config(format!("Failed to resolve app data dir: {e}")))?
        .join("models");
    tokio::fs::create_dir_all(&models_dir).await?;

    let fallback_name = format!("{}.gguf", model.name);
    let filename = normalize_filename(&model_url, &fallback_name);
    let final_path = models_dir.join(filename);
    let temp_path = PathBuf::from(format!("{}.part", final_path.to_string_lossy()));

    if Path::new(&final_path).exists() {
        let metadata = tokio::fs::metadata(&final_path).await?;
        let file_size_mb = ((metadata.len() as f64) / (1024.0 * 1024.0)).round() as i64;

        sqlx::query(
            "UPDATE models SET file_path = ?1, file_size_mb = ?2, is_downloaded = 1 WHERE id = ?3",
        )
        .bind(final_path.to_string_lossy().to_string())
        .bind(file_size_mb)
        .bind(&canonical_id)
        .execute(state.db.write_pool())
        .await?;

        let completed = DownloadProgress {
            model_id: canonical_id.clone(),
            status: "completed".to_string(),
            progress_pct: 100.0,
            bytes_downloaded: metadata.len() as i64,
            bytes_total: Some(metadata.len() as i64),
            error_message: None,
            file_path: Some(final_path.to_string_lossy().to_string()),
        };
        DOWNLOAD_TRACKER.insert(canonical_id.clone(), completed);
        refresh_installed_cache(&state).await?;

        return Ok(DownloadHandle {
            model_id: canonical_id,
            status: "already_downloaded".to_string(),
        });
    }

    let queued = tracker_entry(&canonical_id, "queued");
    DOWNLOAD_TRACKER.insert(canonical_id.clone(), queued.clone());

    let download_row_id = Uuid::new_v4().to_string();
    upsert_download_row(&state, &download_row_id, &canonical_id, &queued).await?;

    let state_cloned = Arc::clone(&state);
    let canonical_id_cloned = canonical_id.clone();
    let model_url_cloned = model_url.clone();
    let final_path_cloned = final_path.clone();
    let temp_path_cloned = temp_path.clone();

    tokio::spawn(async move {
        let run = async {
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60 * 60 * 4))
                .build()
                .map_err(|error| {
                    AppError::Inference(format!("Download client init failed: {error}"))
                })?;

            let response = client
                .get(&model_url_cloned)
                .send()
                .await
                .map_err(|error| {
                    AppError::Inference(format!("Failed to start model download request: {error}"))
                })?;

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                return Err(AppError::Inference(format!(
                    "Model download failed with status {status}. {body}"
                )));
            }

            let total_bytes = response.content_length().map(|v| v as i64);
            let mut downloading = tracker_entry(&canonical_id_cloned, "downloading");
            downloading.bytes_total = total_bytes;
            DOWNLOAD_TRACKER.insert(canonical_id_cloned.clone(), downloading.clone());
            upsert_download_row(
                &state_cloned,
                &download_row_id,
                &canonical_id_cloned,
                &downloading,
            )
            .await?;

            let mut stream = response.bytes_stream();
            let mut file = tokio::fs::File::create(&temp_path_cloned).await?;
            let mut downloaded: i64 = 0;

            while let Some(chunk_result) = stream.next().await {
                let chunk = chunk_result.map_err(|error| {
                    AppError::Inference(format!("Download stream error: {error}"))
                })?;

                file.write_all(&chunk).await?;
                downloaded += chunk.len() as i64;

                let progress_pct = total_bytes
                    .map(|total| {
                        if total <= 0 {
                            0.0
                        } else {
                            ((downloaded as f64 / total as f64) * 100.0).min(100.0)
                        }
                    })
                    .unwrap_or(0.0);

                let progress = DownloadProgress {
                    model_id: canonical_id_cloned.clone(),
                    status: "downloading".to_string(),
                    progress_pct,
                    bytes_downloaded: downloaded,
                    bytes_total: total_bytes,
                    error_message: None,
                    file_path: None,
                };
                DOWNLOAD_TRACKER.insert(canonical_id_cloned.clone(), progress.clone());
                upsert_download_row(
                    &state_cloned,
                    &download_row_id,
                    &canonical_id_cloned,
                    &progress,
                )
                .await?;
            }

            file.flush().await?;
            drop(file);

            tokio::fs::rename(&temp_path_cloned, &final_path_cloned).await?;
            let metadata = tokio::fs::metadata(&final_path_cloned).await?;
            let file_size_mb = ((metadata.len() as f64) / (1024.0 * 1024.0)).round() as i64;

            sqlx::query(
                "UPDATE models SET file_path = ?1, file_size_mb = ?2, is_downloaded = 1 WHERE id = ?3",
            )
            .bind(final_path_cloned.to_string_lossy().to_string())
            .bind(file_size_mb)
            .bind(&canonical_id_cloned)
            .execute(state_cloned.db.write_pool())
            .await?;

            let has_default: (i64,) = sqlx::query_as(
                "SELECT COUNT(*) as count FROM models WHERE is_default = 1 AND is_downloaded = 1",
            )
            .fetch_one(state_cloned.db.read_pool())
            .await?;

            if has_default.0 == 0 {
                sqlx::query("UPDATE models SET is_default = 1, is_active = 1 WHERE id = ?1")
                    .bind(&canonical_id_cloned)
                    .execute(state_cloned.db.write_pool())
                    .await?;
            }

            let completed = DownloadProgress {
                model_id: canonical_id_cloned.clone(),
                status: "completed".to_string(),
                progress_pct: 100.0,
                bytes_downloaded: metadata.len() as i64,
                bytes_total: Some(metadata.len() as i64),
                error_message: None,
                file_path: Some(final_path_cloned.to_string_lossy().to_string()),
            };
            DOWNLOAD_TRACKER.insert(canonical_id_cloned.clone(), completed.clone());
            upsert_download_row(
                &state_cloned,
                &download_row_id,
                &canonical_id_cloned,
                &completed,
            )
            .await?;

            refresh_installed_cache(&state_cloned).await?;
            Ok::<(), AppError>(())
        };

        if let Err(error) = run.await {
            let _ = tokio::fs::remove_file(&temp_path_cloned).await;
            let failed = DownloadProgress {
                model_id: canonical_id_cloned.clone(),
                status: "failed".to_string(),
                progress_pct: 0.0,
                bytes_downloaded: 0,
                bytes_total: None,
                error_message: Some(error.to_string()),
                file_path: None,
            };
            DOWNLOAD_TRACKER.insert(canonical_id_cloned.clone(), failed.clone());
            let _ = upsert_download_row(
                &state_cloned,
                &download_row_id,
                &canonical_id_cloned,
                &failed,
            )
            .await;
        }
    });

    Ok(DownloadHandle {
        model_id: canonical_id,
        status: "queued".to_string(),
    })
}

#[tauri::command]
pub async fn get_download_progress(
    state: State<'_, Arc<AppState>>,
    model_id: String,
) -> Result<DownloadProgress, AppError> {
    crate::log_info!("sarah.command", "get_download_progress invoked");
    ensure_catalog_seeded(&state).await?;
    let model = resolve_model(&state, &model_id).await?;

    if let Some(progress) = DOWNLOAD_TRACKER.get(&model.id) {
        return Ok(progress.clone());
    }

    let row = sqlx::query_as::<_, (String, f64, i64, Option<i64>, Option<String>)>(
        r#"
        SELECT status, progress_pct, bytes_downloaded, bytes_total, error_message
        FROM model_downloads
        WHERE model_id = ?1
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(&model.id)
    .fetch_optional(state.db.read_pool())
    .await?;

    if let Some((status, progress_pct, bytes_downloaded, bytes_total, error_message)) = row {
        return Ok(DownloadProgress {
            model_id: model.id,
            status,
            progress_pct,
            bytes_downloaded,
            bytes_total,
            error_message,
            file_path: model.file_path,
        });
    }

    if model.is_downloaded == 1 {
        return Ok(DownloadProgress {
            model_id: model.id,
            status: "completed".to_string(),
            progress_pct: 100.0,
            bytes_downloaded: model
                .file_size_mb
                .map(|mb| mb * 1024 * 1024)
                .unwrap_or_default(),
            bytes_total: model.file_size_mb.map(|mb| mb * 1024 * 1024),
            error_message: None,
            file_path: model.file_path,
        });
    }

    Ok(DownloadProgress {
        model_id: model.id,
        status: "not_started".to_string(),
        progress_pct: 0.0,
        bytes_downloaded: 0,
        bytes_total: None,
        error_message: None,
        file_path: None,
    })
}
