use std::sync::Arc;

use serde_json::Value;
use tauri::{State, Manager, Runtime};

use crate::commands::model_commands::start_model_download;
use crate::db::models::{Message, Model, NewMessage};
use crate::state::AppState;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalModelSummary {
    pub name: String,
    pub size_bytes: u64,
    pub size_label: String,
    pub modified_at: Option<String>,
    pub family: String,
    pub parameter_size: String,
    pub quantization_level: String,
    pub digest_short: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalChatHistoryItem {
    pub id: String,
    pub prompt: String,
    pub response: String,
    pub timestamp: String,
    pub session_id: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultUserProfile {
    pub id: String,
    pub username: String,
    pub display_name: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredSpotifyConfig {
    server_root: Option<String>,
}

const SPOTIFY_CONFIG_NAMESPACE: &str = "spotify_mcp";
const SPOTIFY_CONFIG_KEY: &str = "config";
const DEFAULT_SPOTIFY_SERVER_ROOT: &str =
    "C:\\Users\\jesud\\OneDrive\\Desktop\\personal\\Sarah\\mcp\\spotify-mcp-server";

#[derive(Debug, Clone)]
enum AudioIntent {
    Play {
        query: Option<String>,
        media_type: &'static str,
    },
    Queue {
        query: String,
    },
    VolumeSet {
        value: i64,
    },
    VolumeAdjust {
        adjustment: i64,
    },
    Pause,
    Stop,
    Next,
    Prev,
}

fn format_size_bytes(size_bytes: u64) -> String {
    if size_bytes == 0 {
        return "Unknown size".to_string();
    }

    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    const TB: f64 = GB * 1024.0;
    let value = size_bytes as f64;

    if value >= TB {
        return format!("{:.2} TB", value / TB);
    }
    if value >= GB {
        return format!("{:.2} GB", value / GB);
    }
    if value >= MB {
        return format!("{:.2} MB", value / MB);
    }
    if value >= KB {
        return format!("{:.2} KB", value / KB);
    }

    format!("{size_bytes} B")
}

fn normalize_spaces(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn extract_first_number(input: &str) -> Option<i64> {
    let digits = input
        .chars()
        .skip_while(|ch| !ch.is_ascii_digit())
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();

    if digits.is_empty() {
        return None;
    }

    digits.parse::<i64>().ok()
}

fn parse_audio_intent(input: &str) -> Option<AudioIntent> {
    let normalized = normalize_spaces(input).to_lowercase();
    let trimmed = normalized.trim();
    if trimmed.is_empty() {
        return None;
    }
    let has_audio_keyword = trimmed.contains("spotify")
        || trimmed.contains("music")
        || trimmed.contains("song")
        || trimmed.contains("track")
        || trimmed.contains("playlist")
        || trimmed.contains("album")
        || trimmed.contains("artist")
        || trimmed.contains("queue")
        || trimmed.contains("volume");

    if trimmed.contains("volume up") || trimmed == "increase volume" {
        return Some(AudioIntent::VolumeAdjust { adjustment: 10 });
    }
    if trimmed.contains("volume down") || trimmed == "decrease volume" {
        return Some(AudioIntent::VolumeAdjust { adjustment: -10 });
    }

    if trimmed.contains("volume") && trimmed.contains("set") {
        if let Some(value) = extract_first_number(trimmed) {
            return Some(AudioIntent::VolumeSet {
                value: value.clamp(0, 100),
            });
        }
    }
    if let Some(value) = trimmed
        .strip_prefix("volume ")
        .and_then(|rest| extract_first_number(rest))
    {
        return Some(AudioIntent::VolumeSet {
            value: value.clamp(0, 100),
        });
    }

    if trimmed.starts_with("queue ") || (trimmed.starts_with("add ") && trimmed.contains(" queue"))
    {
        let query = trimmed
            .strip_prefix("queue ")
            .or_else(|| trimmed.strip_prefix("add "))
            .unwrap_or_default()
            .replace(" to queue", "")
            .trim()
            .to_string();
        if !query.is_empty() {
            return Some(AudioIntent::Queue { query });
        }
    }

    if trimmed == "pause" || trimmed == "pause music" || trimmed == "pause spotify" {
        return Some(AudioIntent::Pause);
    }
    if trimmed == "stop" || trimmed == "stop music" || trimmed == "stop spotify" {
        return Some(AudioIntent::Stop);
    }
    if trimmed == "next" || trimmed == "next song" || trimmed == "skip" || trimmed == "skip song" {
        return Some(AudioIntent::Next);
    }
    if trimmed == "previous"
        || trimmed == "previous song"
        || trimmed == "prev"
        || trimmed == "back song"
    {
        return Some(AudioIntent::Prev);
    }

    if trimmed.starts_with("play ")
        || (trimmed.starts_with("resume") && has_audio_keyword)
        || (trimmed.starts_with("start ") && has_audio_keyword)
    {
        if trimmed == "play" || trimmed == "resume" || trimmed == "start music" {
            return Some(AudioIntent::Play {
                query: None,
                media_type: "track",
            });
        }

        let rest = trimmed
            .strip_prefix("play ")
            .or_else(|| trimmed.strip_prefix("start "))
            .or_else(|| trimmed.strip_prefix("resume "))
            .unwrap_or_default()
            .trim();

        if rest.is_empty() {
            return Some(AudioIntent::Play {
                query: None,
                media_type: "track",
            });
        }

        if let Some(value) = rest.strip_prefix("playlist ") {
            let query = value.trim();
            if !query.is_empty() {
                return Some(AudioIntent::Play {
                    query: Some(query.to_string()),
                    media_type: "playlist",
                });
            }
        }
        if let Some(value) = rest.strip_prefix("album ") {
            let query = value.trim();
            if !query.is_empty() {
                return Some(AudioIntent::Play {
                    query: Some(query.to_string()),
                    media_type: "album",
                });
            }
        }
        if let Some(value) = rest.strip_prefix("artist ") {
            let query = value.trim();
            if !query.is_empty() {
                return Some(AudioIntent::Play {
                    query: Some(query.to_string()),
                    media_type: "artist",
                });
            }
        }

        return Some(AudioIntent::Play {
            query: Some(rest.to_string()),
            media_type: "track",
        });
    }

    None
}

fn parse_search_result(raw: &str) -> (Option<String>, Option<String>, Option<String>) {
    let tool_payload = serde_json::from_str::<Value>(raw).ok();
    let text = tool_payload
        .as_ref()
        .and_then(|value| value.get("content"))
        .and_then(Value::as_array)
        .and_then(|rows| rows.first())
        .and_then(|row| row.get("text"))
        .and_then(Value::as_str)
        .unwrap_or(raw);

    let id = text
        .split("ID:")
        .nth(1)
        .map(|rest| {
            rest.trim_start()
                .chars()
                .take_while(|ch| ch.is_ascii_alphanumeric())
                .collect::<String>()
        })
        .filter(|value| !value.is_empty());

    let mut title = None;
    let mut artist = None;

    if let Some(start) = text.find("1. \"") {
        let tail = &text[start + 4..];
        if let Some(end_quote) = tail.find('"') {
            let parsed_title = tail[..end_quote].trim();
            if !parsed_title.is_empty() {
                title = Some(parsed_title.to_string());
            }

            let after_title = &tail[end_quote + 1..];
            if let Some(by_index) = after_title.find(" by ") {
                let after_by = &after_title[by_index + 4..];
                let artist_end = after_by.find(" (").unwrap_or(after_by.len());
                let parsed_artist = after_by[..artist_end].trim();
                if !parsed_artist.is_empty() {
                    artist = Some(parsed_artist.to_string());
                }
            }
        }
    }

    (id, title, artist)
}

async fn resolve_installed_model(
    state: &Arc<AppState>,
    requested: Option<&str>,
) -> Result<Model, String> {
    let mut installed = state
        .model_repo
        .list_installed()
        .await
        .map_err(|error| error.to_string())?;

    if installed.is_empty() {
        return Err(
            "No local model is installed. Download a model first in the Models window.".to_string(),
        );
    }

    if let Some(value) = requested {
        let normalized = value.trim();
        if normalized.is_empty() {
            return Err("Model name is empty.".to_string());
        }

        if let Some(found) = installed.iter().find(|row| row.id == normalized) {
            return Ok(found.clone());
        }

        if let Some(found) = installed
            .iter()
            .find(|row| row.name.eq_ignore_ascii_case(normalized))
        {
            return Ok(found.clone());
        }

        if let Some(found) = installed
            .iter()
            .find(|row| row.display_name.eq_ignore_ascii_case(normalized))
        {
            return Ok(found.clone());
        }

        return Err(format!(
            "Model '{normalized}' is not installed locally. Download it in the Models window first."
        ));
    }

    if let Some(default_model) = installed.iter().find(|row| row.is_default == 1) {
        return Ok(default_model.clone());
    }

    installed.sort_by(|left, right| left.display_name.cmp(&right.display_name));
    Ok(installed.remove(0))
}

async fn ensure_model_loaded(state: &Arc<AppState>, model: &Model) -> Result<(), String> {
    let model_path = model
        .file_path
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            format!(
                "Model '{}' is marked installed but has no local GGUF path.",
                model.display_name
            )
        })?
        .to_string();

    let active_model = state.inference.get_active_model_info().await;
    let needs_load = active_model
        .as_ref()
        .map(|info| info.path != model_path)
        .unwrap_or(true);

    if !needs_load {
        return Ok(());
    }

    let hardware_profile = state
        .hardware
        .read()
        .await
        .clone()
        .ok_or_else(|| "Hardware profile is unavailable.".to_string())?;

    let mode = state.hardware_service.get_performance_mode(None).await;

    state
        .inference
        .load_model(&model_path, &hardware_profile, mode)
        .await
        .map_err(|error| error.to_string())
}

async fn resolve_spotify_server_root(state: &Arc<AppState>) -> Result<String, String> {
    let config_setting = state
        .settings_repo
        .get_setting(None, SPOTIFY_CONFIG_NAMESPACE, SPOTIFY_CONFIG_KEY)
        .await
        .map_err(|error| error.to_string())?;

    if let Some(setting) = config_setting {
        let parsed = serde_json::from_str::<StoredSpotifyConfig>(&setting.value).ok();
        if let Some(server_root) = parsed
            .and_then(|row| row.server_root)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        {
            return Ok(server_root);
        }
    }

    Ok(DEFAULT_SPOTIFY_SERVER_ROOT.to_string())
}

async fn ensure_spotify_mcp_running(server_root: &str) -> Result<(), String> {
    let running = crate::commands::integration_commands::spotify_mcp_status().await?;
    if running {
        return Ok(());
    }

    let entry = std::path::PathBuf::from(server_root)
        .join("build")
        .join("index.js")
        .to_string_lossy()
        .to_string();

    crate::commands::integration_commands::start_spotify_mcp(entry).await?;
    Ok(())
}

async fn execute_audio_intent(
    app: &tauri::AppHandle,
    state: &Arc<AppState>,
    intent: AudioIntent,
) -> Result<String, String> {
    let server_root = resolve_spotify_server_root(state).await?;
    ensure_spotify_mcp_running(&server_root).await?;

    match intent {
        AudioIntent::Pause => {
            crate::commands::integration_commands::run_spotify_tool(
                app.clone(),
                server_root,
                "pausePlayback".to_string(),
                serde_json::json!({}),
            )
            .await?;
            Ok("Pausing Spotify playback.".to_string())
        }
        AudioIntent::Stop => {
            crate::commands::integration_commands::run_spotify_tool(
                app.clone(),
                server_root,
                "pausePlayback".to_string(),
                serde_json::json!({}),
            )
            .await?;
            Ok("Stopping Spotify playback.".to_string())
        }
        AudioIntent::Next => {
            crate::commands::integration_commands::run_spotify_tool(
                app.clone(),
                server_root,
                "skipToNext".to_string(),
                serde_json::json!({}),
            )
            .await?;
            Ok("Skipping to the next Spotify track.".to_string())
        }
        AudioIntent::Prev => {
            crate::commands::integration_commands::run_spotify_tool(
                app.clone(),
                server_root,
                "skipToPrevious".to_string(),
                serde_json::json!({}),
            )
            .await?;
            Ok("Going back to the previous Spotify track.".to_string())
        }
        AudioIntent::VolumeSet { value } => {
            crate::commands::integration_commands::run_spotify_tool(
                app.clone(),
                server_root,
                "setVolume".to_string(),
                serde_json::json!({
                    "volumePercent": value,
                }),
            )
            .await?;
            Ok(format!("Volume set to {value}%."))
        }
        AudioIntent::VolumeAdjust { adjustment } => {
            crate::commands::integration_commands::run_spotify_tool(
                app.clone(),
                server_root,
                "adjustVolume".to_string(),
                serde_json::json!({
                    "adjustment": adjustment,
                }),
            )
            .await?;
            Ok(if adjustment >= 0 {
                "Volume increased.".to_string()
            } else {
                "Volume decreased.".to_string()
            })
        }
        AudioIntent::Play { query, media_type } => {
            if let Some(query_text) = query {
                let search_raw = crate::commands::integration_commands::run_spotify_tool(
                    app.clone(),
                    server_root.clone(),
                    "searchSpotify".to_string(),
                    serde_json::json!({
                        "query": query_text,
                        "type": media_type,
                        "limit": 5,
                    }),
                )
                .await?;

                let (id, title, artist) = parse_search_result(&search_raw);
                let Some(track_id) = id else {
                    return Ok("No matching Spotify results were found.".to_string());
                };

                crate::commands::integration_commands::run_spotify_tool(
                    app.clone(),
                    server_root,
                    "playMusic".to_string(),
                    serde_json::json!({
                        "type": media_type,
                        "id": track_id,
                    }),
                )
                .await?;

                if let Some(title) = title {
                    return Ok(if let Some(artist) = artist {
                        format!("Playing \"{title}\" by {artist}.")
                    } else {
                        format!("Playing \"{title}\".")
                    });
                }

                Ok("Playing selected Spotify result.".to_string())
            } else {
                crate::commands::integration_commands::run_spotify_tool(
                    app.clone(),
                    server_root,
                    "resumePlayback".to_string(),
                    serde_json::json!({}),
                )
                .await?;
                Ok("Resuming Spotify playback.".to_string())
            }
        }
        AudioIntent::Queue { query } => {
            let search_raw = crate::commands::integration_commands::run_spotify_tool(
                app.clone(),
                server_root.clone(),
                "searchSpotify".to_string(),
                serde_json::json!({
                    "query": query,
                    "type": "track",
                    "limit": 5,
                }),
            )
            .await?;

            let (id, title, artist) = parse_search_result(&search_raw);
            let Some(track_id) = id else {
                return Ok("No matching Spotify track was found for queue.".to_string());
            };

            crate::commands::integration_commands::run_spotify_tool(
                app.clone(),
                server_root,
                "addToQueue".to_string(),
                serde_json::json!({
                    "type": "track",
                    "id": track_id,
                }),
            )
            .await?;

            if let Some(title) = title {
                return Ok(if let Some(artist) = artist {
                    format!("Queued \"{title}\" by {artist}.")
                } else {
                    format!("Queued \"{title}\".")
                });
            }
            Ok("Track added to Spotify queue.".to_string())
        }
    }
}

async fn persist_prompt_response(
    state: &Arc<AppState>,
    prompt: &str,
    response: &str,
    model_id: Option<&str>,
) -> Result<(), String> {
    if prompt.trim().is_empty() || response.trim().is_empty() {
        return Ok(());
    }

    let user = state
        .user_repo
        .get_or_create_default_user()
        .await
        .map_err(|error| error.to_string())?;

    let session = state
        .conversation_repo
        .create_session(&user.id, model_id)
        .await
        .map_err(|error| error.to_string())?;

    state
        .conversation_repo
        .insert_message(NewMessage {
            session_id: session.id.clone(),
            role: "user".to_string(),
            content: prompt.trim().to_string(),
            content_type: "text".to_string(),
            token_count: Some((prompt.len() / 4) as i64 + 1),
            model_id: model_id.map(ToString::to_string),
            metadata: "{}".to_string(),
            position: 0,
        })
        .await
        .map_err(|error| error.to_string())?;

    state
        .conversation_repo
        .insert_message(NewMessage {
            session_id: session.id,
            role: "assistant".to_string(),
            content: response.trim().to_string(),
            content_type: "markdown".to_string(),
            token_count: Some((response.len() / 4) as i64 + 1),
            model_id: model_id.map(ToString::to_string),
            metadata: "{}".to_string(),
            position: 1,
        })
        .await
        .map_err(|error| error.to_string())?;

    Ok(())
}

#[derive(serde::Deserialize)]
struct OllamaGenerateResponse {
    response: String,
}

#[derive(serde::Deserialize)]
struct OllamaTagItem {
    name: String,
    #[serde(default)]
    modified_at: Option<String>,
    #[serde(default)]
    size: Option<u64>,
    #[serde(default)]
    digest: Option<String>,
    #[serde(default)]
    details: Option<OllamaTagDetails>,
}

#[derive(serde::Deserialize)]
struct OllamaTagDetails {
    #[serde(default)]
    family: Option<String>,
    #[serde(default)]
    parameter_size: Option<String>,
    #[serde(default)]
    quantization_level: Option<String>,
}

#[derive(serde::Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaTagItem>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OllamaModelSummary {
    name: String,
    size_bytes: u64,
    size_label: String,
    modified_at: Option<String>,
    family: String,
    parameter_size: String,
    quantization_level: String,
    digest_short: String,
}

async fn fetch_ollama_tags<R: Runtime>(app: &tauri::AppHandle<R>) -> Result<OllamaTagsResponse, String> {
    let client = app.state::<reqwest::Client>();

    let response = client
        .get("http://127.0.0.1:11434/api/tags")
        .send()
        .await
        .map_err(|error| {
            format!("Failed to connect to Ollama at http://127.0.0.1:11434. Start Ollama first. {error}")
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Ollama tags request failed with status {status}. {body}"));
    }

    response
        .json::<OllamaTagsResponse>()
        .await
        .map_err(|error| format!("Invalid Ollama tags response: {error}"))
}

#[tauri::command]
pub async fn generate_ollama_response<R: Runtime>(
    prompt: String,
    model: Option<String>,
    app: tauri::AppHandle<R>,
) -> Result<String, String> {
    let prompt = prompt.trim().to_string();
    if prompt.is_empty() {
        return Err("Prompt is empty.".to_string());
    }

    let model = model
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "qwen2.5-coder:7b".to_string());

    let client = app.state::<reqwest::Client>();

    let response = client
        .post("http://127.0.0.1:11434/api/generate")
        .json(&serde_json::json!({
            "model": model,
            "prompt": prompt,
            "stream": false
        }))
        .send()
        .await
        .map_err(|error| {
            format!("Failed to connect to Ollama at http://127.0.0.1:11434. Start Ollama and verify the model is installed. {error}")
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Ollama request failed with status {status}. {body}"));
    }

    let payload = response
        .json::<OllamaGenerateResponse>()
        .await
        .map_err(|error| format!("Invalid Ollama response: {error}"))?;

    let text = payload.response.trim().to_string();
    if text.is_empty() {
        return Err("Ollama returned an empty response.".to_string());
    }

    Ok(text)
}

#[tauri::command]
pub async fn list_ollama_models<R: Runtime>(app: tauri::AppHandle<R>) -> Result<Vec<String>, String> {
    let payload = fetch_ollama_tags(&app).await?;

    let mut models: Vec<String> = payload
        .models
        .into_iter()
        .map(|item| item.name.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect();

    models.sort_unstable();
    models.dedup();

    Ok(models)
}

#[tauri::command]
pub async fn list_ollama_models_detailed<R: Runtime>(app: tauri::AppHandle<R>) -> Result<Vec<OllamaModelSummary>, String> {
    let payload = fetch_ollama_tags(&app).await?;
    let mut rows: Vec<OllamaModelSummary> = payload
        .models
        .into_iter()
        .map(|item| {
            let details = item.details;
            let size_bytes = item.size.unwrap_or(0);
            let digest_short = item
                .digest
                .unwrap_or_default()
                .chars()
                .take(12)
                .collect::<String>();

            OllamaModelSummary {
                name: item.name.trim().to_string(),
                size_bytes,
                size_label: format_size_bytes(size_bytes),
                modified_at: item.modified_at,
                family: details
                    .as_ref()
                    .and_then(|entry| entry.family.clone())
                    .unwrap_or_else(|| "Unknown".to_string()),
                parameter_size: details
                    .as_ref()
                    .and_then(|entry| entry.parameter_size.clone())
                    .unwrap_or_else(|| "Unknown".to_string()),
                quantization_level: details
                    .as_ref()
                    .and_then(|entry| entry.quantization_level.clone())
                    .unwrap_or_else(|| "Unknown".to_string()),
                digest_short,
            }
        })
        .filter(|row| !row.name.is_empty())
        .collect();

    rows.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
    rows.dedup_by(|left, right| left.name == right.name);
    Ok(rows)
}

#[tauri::command]
pub async fn pull_ollama_model<R: Runtime>(model: String, app: tauri::AppHandle<R>) -> Result<String, String> {
    let normalized = model.trim().to_string();
    if normalized.is_empty() {
        return Err("Model name is empty.".to_string());
    }

    let client = app.state::<reqwest::Client>();

    let response = client
        .post("http://127.0.0.1:11434/api/pull")
        .json(&serde_json::json!({
            "name": normalized,
            "stream": false
        }))
        .send()
        .await
        .map_err(|error| {
            format!("Failed to connect to Ollama at http://127.0.0.1:11434. Start Ollama first. {error}")
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Ollama pull request failed with status {status}. {body}"));
    }

    let payload = response
        .json::<serde_json::Value>()
        .await
        .map_err(|error| format!("Invalid Ollama pull response: {error}"))?;

    if let Some(error) = payload.get("error").and_then(|value| value.as_str()) {
        if !error.trim().is_empty() {
            return Err(error.trim().to_string());
        }
    }

    let status = payload
        .get("status")
        .and_then(|value| value.as_str())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "Model download complete.".to_string());

    Ok(status)
}

#[tauri::command]
pub fn greet(name: &str) -> String {
    crate::log_info!("sarah.command", "greet invoked");
    format!("Hello, {name}! Sarah backend is running locally.")
}

#[tauri::command]
pub async fn get_default_user(state: State<'_, Arc<AppState>>) -> Result<DefaultUserProfile, String> {
    crate::log_info!("sarah.command", "get_default_user invoked");
    let user = state
        .user_repo
        .get_or_create_default_user()
        .await
        .map_err(|error| error.to_string())?;

    Ok(DefaultUserProfile {
        id: user.id,
        username: user.username,
        display_name: user.display_name,
    })
}

#[tauri::command]
pub async fn generate_local_response(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    prompt: String,
    model: Option<String>,
) -> Result<String, String> {
    crate::log_info!("sarah.command", "generate_local_response invoked");
    let prompt = prompt.trim().to_string();
    if prompt.is_empty() {
        return Err("Prompt is empty.".to_string());
    }

    if let Some(intent) = parse_audio_intent(&prompt) {
        let response = execute_audio_intent(&app, &state, intent).await?;
        let _ = persist_prompt_response(&state, &prompt, &response, None).await;
        return Ok(response);
    }

    let selected_model = resolve_installed_model(&state, model.as_deref()).await?;
    ensure_model_loaded(&state, &selected_model).await?;

    let user_message = Message {
        id: "adhoc-user".to_string(),
        session_id: "adhoc-session".to_string(),
        role: "user".to_string(),
        content: prompt.clone(),
        content_type: "text".to_string(),
        thinking: None,
        token_count: None,
        model_id: Some(selected_model.id.clone()),
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

    let result = state
        .inference
        .generate_with_tools(vec![user_message], &[])
        .await
        .map_err(|error| error.to_string())?;

    let text = result.text.trim().to_string();
    if text.is_empty() {
        return Err("Local model returned an empty response.".to_string());
    }

    let _ = persist_prompt_response(&state, &prompt, &text, Some(&selected_model.id)).await;
    Ok(text)
}

#[tauri::command]
pub async fn list_local_models(state: State<'_, Arc<AppState>>) -> Result<Vec<String>, String> {
    crate::log_info!("sarah.command", "list_local_models invoked");
    let mut models = state
        .model_repo
        .list_installed()
        .await
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(|row| row.name)
        .collect::<Vec<_>>();

    models.sort_unstable();
    models.dedup();
    Ok(models)
}

#[tauri::command]
pub async fn list_local_models_detailed(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<LocalModelSummary>, String> {
    crate::log_info!("sarah.command", "list_local_models_detailed invoked");
    let mut rows = state
        .model_repo
        .list_installed()
        .await
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(|row| {
            let size_bytes = row.file_size_mb.unwrap_or(0).max(0) as u64 * 1024 * 1024;
            let digest_short = row
                .sha256_checksum
                .unwrap_or_default()
                .chars()
                .take(12)
                .collect::<String>();

            LocalModelSummary {
                name: row.name,
                size_bytes,
                size_label: format_size_bytes(size_bytes),
                modified_at: Some(row.updated_at),
                family: row.family,
                parameter_size: row.parameter_count.unwrap_or_else(|| "Unknown".to_string()),
                quantization_level: row.quantization.unwrap_or_else(|| "Unknown".to_string()),
                digest_short,
            }
        })
        .collect::<Vec<_>>();

    rows.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
    rows.dedup_by(|left, right| left.name == right.name);
    Ok(rows)
}

#[tauri::command]
pub async fn get_local_chat_history(
    state: State<'_, Arc<AppState>>,
    limit: Option<i64>,
) -> Result<Vec<LocalChatHistoryItem>, String> {
    crate::log_info!("sarah.command", "get_local_chat_history invoked");
    let user = state
        .user_repo
        .get_or_create_default_user()
        .await
        .map_err(|error| error.to_string())?;

    let rows = sqlx::query_as::<_, (String, String, Option<String>, String, String)>(
        r#"
        SELECT
            m.id,
            m.content,
            (
                SELECT a.content
                FROM messages a
                WHERE a.session_id = m.session_id
                  AND a.role = 'assistant'
                  AND a.position > m.position
                ORDER BY a.position ASC
                LIMIT 1
            ) AS response,
            m.created_at,
            m.session_id
        FROM messages m
        JOIN sessions s ON s.id = m.session_id
        WHERE s.user_id = ?1
          AND m.role = 'user'
        ORDER BY datetime(m.created_at) DESC
        LIMIT ?2
        "#,
    )
    .bind(user.id)
    .bind(limit.unwrap_or(120).clamp(1, 500))
    .fetch_all(state.db.read_pool())
    .await
    .map_err(|error| error.to_string())?;

    Ok(rows
        .into_iter()
        .map(
            |(id, prompt, response, timestamp, session_id)| LocalChatHistoryItem {
                id,
                prompt,
                response: response.unwrap_or_default(),
                timestamp,
                session_id,
            },
        )
        .collect())
}

#[tauri::command]
pub async fn clear_local_chat_history(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    crate::log_info!("sarah.command", "clear_local_chat_history invoked");
    let user = state
        .user_repo
        .get_or_create_default_user()
        .await
        .map_err(|error| error.to_string())?;

    sqlx::query("DELETE FROM sessions WHERE user_id = ?1")
        .bind(user.id)
        .execute(state.db.write_pool())
        .await
        .map_err(|error| error.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn download_local_model(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    model: String,
) -> Result<String, String> {
    crate::log_info!("sarah.command", "download_local_model invoked");
    let target = model.trim();
    if target.is_empty() {
        return Err("Model name is empty.".to_string());
    }

    let handle = start_model_download(app, state, target.to_string())
        .await
        .map_err(|error| error.to_string())?;

    Ok(match handle.status.as_str() {
        "already_downloaded" => "Model already downloaded.".to_string(),
        "queued" => "Model download queued.".to_string(),
        "downloading" => "Model is already downloading.".to_string(),
        other => format!("Model download status: {other}"),
    })
}
