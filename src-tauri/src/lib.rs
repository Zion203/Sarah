use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, PhysicalPosition, Runtime, WebviewUrl, WebviewWindowBuilder,
};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use serde::Serialize;
use std::time::Duration;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

mod native_capture;

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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OllamaModelSummary {
    name: String,
    size_bytes: u64,
    size_label: String,
    modified_at: Option<String>,
    family: String,
    parameter_size: String,
    quantization_level: String,
    digest_short: String,
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

async fn fetch_ollama_tags(app: &AppHandle) -> Result<OllamaTagsResponse, String> {
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

struct SpotifyMcpState(Mutex<Option<Child>>);

impl Default for SpotifyMcpState {
    fn default() -> Self {
        Self(Mutex::new(None))
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SpotifyConfigSnapshot {
    client_id: String,
    client_secret: String,
    redirect_uri: String,
    access_token: Option<String>,
    has_access_token: bool,
    has_refresh_token: bool,
    expires_at: Option<u64>,
    has_config_file: bool,
}

fn parse_expires_at(value: Option<&serde_json::Value>) -> Option<u64> {
    let raw = value?;
    if let Some(number) = raw.as_u64() {
        return Some(number);
    }
    if let Some(number) = raw.as_i64() {
        if number >= 0 {
            return Some(number as u64);
        }
        return None;
    }
    if let Some(text) = raw.as_str() {
        return text.trim().parse::<u64>().ok();
    }
    None
}

const AUDIO_WINDOW_WIDTH: f64 = 380.0;
const AUDIO_WINDOW_HEIGHT: f64 = 140.0;

#[derive(serde::Serialize, Clone)]
struct AudioCommandPayload {
    action: String,
}

fn close_aux_window_group(app: &AppHandle, keep: &str) {
    for window_label in ["settings", "models", "history", "mcp"] {
        if window_label == keep {
            continue;
        }
        if let Some(window) = app.get_webview_window(window_label) {
            let _ = window.close();
        }
    }
}

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn generate_ollama_response(
    prompt: String,
    model: Option<String>,
    app: AppHandle,
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
async fn list_ollama_models(app: AppHandle) -> Result<Vec<String>, String> {
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
async fn list_ollama_models_detailed(app: AppHandle) -> Result<Vec<OllamaModelSummary>, String> {
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
async fn pull_ollama_model(model: String, app: AppHandle) -> Result<String, String> {
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
async fn open_settings_window(app: AppHandle) -> Result<(), String> {
    close_aux_window_group(&app, "settings");

    if let Some(main_window) = app.get_webview_window("main") {
        let _ = main_window.hide();
    }

    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.set_resizable(false);
        let _ = window.set_maximizable(false);
        if window.is_maximized().unwrap_or(false) {
            let _ = window.unmaximize();
        }
        let _ = window.center();
        let _ = window.show();
        let _ = window.set_focus();
        return Ok(());
    }

    WebviewWindowBuilder::new(
        &app,
        "settings",
        WebviewUrl::App("index.html?window=settings".into()),
    )
    .title("Sarah AI Settings")
    .inner_size(920.0, 640.0)
    .resizable(false)
    .maximizable(false)
    .minimizable(true)
    .decorations(false)
    .always_on_top(false)
    .center()
    .build()
    .map_err(|error| error.to_string())?;

    Ok(())
}

#[tauri::command]
async fn open_models_window(app: AppHandle) -> Result<(), String> {
    close_aux_window_group(&app, "models");

    if let Some(main_window) = app.get_webview_window("main") {
        let _ = main_window.hide();
    }

    if let Some(window) = app.get_webview_window("models") {
        let _ = window.set_resizable(false);
        let _ = window.set_maximizable(false);
        if window.is_maximized().unwrap_or(false) {
            let _ = window.unmaximize();
        }
        let _ = window.center();
        let _ = window.show();
        let _ = window.set_focus();
        return Ok(());
    }

    WebviewWindowBuilder::new(
        &app,
        "models",
        WebviewUrl::App("index.html?window=models".into()),
    )
    .title("Sarah AI Models")
    .inner_size(980.0, 660.0)
    .resizable(false)
    .maximizable(false)
    .minimizable(true)
    .decorations(false)
    .always_on_top(false)
    .center()
    .build()
    .map_err(|error| error.to_string())?;

    Ok(())
}

#[tauri::command]
async fn open_history_window(app: AppHandle) -> Result<(), String> {
    close_aux_window_group(&app, "history");

    if let Some(window) = app.get_webview_window("history") {
        let _ = window.center();
        let _ = window.show();
        let _ = window.set_focus();
        return Ok(());
    }

    WebviewWindowBuilder::new(
        &app,
        "history",
        WebviewUrl::App("index.html?window=history".into()),
    )
    .title("Sarah AI Chat History")
    .inner_size(560.0, 520.0)
    .resizable(false)
    .maximizable(false)
    .minimizable(false)
    .always_on_top(true)
    .center()
    .build()
    .map_err(|error| error.to_string())?;

    Ok(())
}

#[tauri::command]
async fn open_mcp_window(app: AppHandle) -> Result<(), String> {
    close_aux_window_group(&app, "mcp");

    if let Some(main_window) = app.get_webview_window("main") {
        let _ = main_window.hide();
    }

    if let Some(window) = app.get_webview_window("mcp") {
        let _ = window.center();
        let _ = window.show();
        let _ = window.set_focus();
        return Ok(());
    }

    WebviewWindowBuilder::new(
        &app,
        "mcp",
        WebviewUrl::App("index.html?window=mcp".into()),
    )
    .title("Sarah AI MCP Marketplace")
    .inner_size(980.0, 680.0)
    .min_inner_size(840.0, 560.0)
    .resizable(true)
    .maximizable(true)
    .minimizable(true)
    .decorations(false)
    .always_on_top(false)
    .center()
    .build()
    .map_err(|error| error.to_string())?;

    Ok(())
}

fn position_audio_window(window: &tauri::WebviewWindow) {
    if let Ok(Some(monitor)) = window.primary_monitor() {
        let size = monitor.size();
        let position = monitor.position();
        let margin = 64;
        let x = position.x + margin;
        let y = position.y + size.height as i32 - AUDIO_WINDOW_HEIGHT as i32 - margin;
        let _ = window.set_position(PhysicalPosition::new(x, y));
    }
}

#[tauri::command]
async fn open_audio_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("audio") {
        let _ = window.set_resizable(false);
        let _ = window.set_maximizable(false);
        let _ = window.set_minimizable(false);
        let _ = window.set_always_on_top(true);
        let _ = window.set_shadow(false);
        let _ = window.set_skip_taskbar(true);
        position_audio_window(&window);
        let _ = window.show();
        let _ = window.set_focus();
        let _ = app.emit("sarah://audio-window-state", true);
        return Ok(());
    }

    let window = WebviewWindowBuilder::new(
        &app,
        "audio",
        WebviewUrl::App("index.html?window=audio".into()),
    )
    .title("Spotify Player")
    .inner_size(AUDIO_WINDOW_WIDTH, AUDIO_WINDOW_HEIGHT)
    .resizable(false)
    .maximizable(false)
    .minimizable(false)
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .build()
    .map_err(|error| error.to_string())?;

    position_audio_window(&window);
    let _ = window.show();
    let _ = window.set_focus();
    let _ = app.emit("sarah://audio-window-state", true);
    Ok(())
}

#[tauri::command]
async fn close_audio_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("audio") {
        let _ = window.hide();
    }
    let _ = app.emit("sarah://audio-window-state", false);
    Ok(())
}

#[tauri::command]
async fn emit_audio_command(app: AppHandle, action: String) -> Result<(), String> {
    let payload = AudioCommandPayload {
        action: action.trim().to_string(),
    };
    let _ = app.emit("sarah://audio-control", payload);
    Ok(())
}

#[tauri::command]
fn run_spotify_tool(
    server_root: String,
    tool: String,
    args: serde_json::Value,
) -> Result<String, String> {
    let root = Path::new(&server_root);
    if !root.exists() {
        return Err("Spotify MCP server root not found.".to_string());
    }

    let tool = tool.trim().to_string();
    if tool.is_empty() {
        return Err("Missing Spotify MCP tool name.".to_string());
    }

    let runner = root.join("tool-runner.js");
    if !runner.exists() {
        return Err("tool-runner.js not found. Rebuild the MCP server.".to_string());
    }

    let output = Command::new("node")
        .arg(runner)
        .arg(&tool)
        .arg(args.to_string())
        .current_dir(root)
        .output()
        .map_err(|error| format!("Failed to run Spotify MCP tool: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if !stderr.is_empty() {
            return Err(stderr);
        }
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        return Err("Spotify MCP tool returned empty output.".to_string());
    }

    Ok(stdout)
}

#[tauri::command]
fn start_spotify_mcp(
    entry_path: String,
    state: tauri::State<SpotifyMcpState>,
) -> Result<(), String> {
    let entry_path = entry_path.trim().to_string();
    if entry_path.is_empty() {
        return Err("Missing Spotify MCP entry path.".to_string());
    }

    let entry = Path::new(&entry_path);
    if !entry.exists() {
        return Err(format!("Spotify MCP entry not found at {entry_path}."));
    }

    let mut guard = state.0.lock().map_err(|_| "Spotify MCP state locked.")?;
    if guard.is_some() {
        return Err("Spotify MCP is already running.".to_string());
    }

    let working_dir = entry
        .parent()
        .map(|path| path.to_path_buf())
        .unwrap_or_else(|| Path::new(".").to_path_buf());

    let child = Command::new("node")
        .arg(entry_path)
        .current_dir(working_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("Failed to start Spotify MCP: {error}"))?;

    *guard = Some(child);
    Ok(())
}

#[tauri::command]
fn stop_spotify_mcp(state: tauri::State<SpotifyMcpState>) -> Result<(), String> {
    let mut guard = state.0.lock().map_err(|_| "Spotify MCP state locked.")?;
    if let Some(mut child) = guard.take() {
        child
            .kill()
            .map_err(|error| format!("Failed to stop Spotify MCP: {error}"))?;
        let _ = child.wait();
    }
    Ok(())
}

#[tauri::command]
fn spotify_mcp_status(state: tauri::State<SpotifyMcpState>) -> Result<bool, String> {
    let mut guard = state.0.lock().map_err(|_| "Spotify MCP state locked.")?;
    if let Some(child) = guard.as_mut() {
        match child.try_wait() {
            Ok(Some(_)) => {
                *guard = None;
                Ok(false)
            }
            Ok(None) => Ok(true),
            Err(_) => Ok(false),
        }
    } else {
        Ok(false)
    }
}

fn npm_command() -> Command {
    if cfg!(windows) {
        let mut cmd = Command::new("cmd");
        cmd.arg("/c").arg("npm");
        cmd
    } else {
        Command::new("npm")
    }
}

#[tauri::command]
fn write_spotify_config(
    server_root: String,
    client_id: String,
    client_secret: String,
    redirect_uri: String,
) -> Result<(), String> {
    let server_root = server_root.trim().to_string();
    if server_root.is_empty() {
        return Err("Missing Spotify MCP server root.".to_string());
    }

    let config_path = Path::new(&server_root).join("spotify-config.json");
    let mut existing: serde_json::Value = if config_path.exists() {
        let raw = std::fs::read_to_string(&config_path)
            .map_err(|error| format!("Failed to read existing config: {error}"))?;
        serde_json::from_str(&raw).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    existing["clientId"] = serde_json::Value::String(client_id.trim().to_string());
    existing["clientSecret"] = serde_json::Value::String(client_secret.trim().to_string());
    existing["redirectUri"] = serde_json::Value::String(redirect_uri.trim().to_string());

    let payload = serde_json::to_string_pretty(&existing)
        .map_err(|error| format!("Failed to serialize config: {error}"))?;
    std::fs::write(&config_path, payload)
        .map_err(|error| format!("Failed to write config: {error}"))?;

    Ok(())
}

#[tauri::command]
fn read_spotify_config(server_root: String) -> Result<SpotifyConfigSnapshot, String> {
    let server_root = server_root.trim().to_string();
    if server_root.is_empty() {
        return Err("Missing Spotify MCP server root.".to_string());
    }

    let config_path = Path::new(&server_root).join("spotify-config.json");
    if !config_path.exists() {
        return Ok(SpotifyConfigSnapshot {
            client_id: String::new(),
            client_secret: String::new(),
            redirect_uri: String::new(),
            access_token: None,
            has_access_token: false,
            has_refresh_token: false,
            expires_at: None,
            has_config_file: false,
        });
    }

    let raw = std::fs::read_to_string(&config_path)
        .map_err(|error| format!("Failed to read Spotify config: {error}"))?;
    let parsed: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|error| format!("Invalid spotify-config.json: {error}"))?;

    let client_id = parsed
        .get("clientId")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let client_secret = parsed
        .get("clientSecret")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let redirect_uri = parsed
        .get("redirectUri")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let has_access_token = parsed
        .get("accessToken")
        .and_then(|value| value.as_str())
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false);
    let access_token = parsed
        .get("accessToken")
        .and_then(|value| value.as_str())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let has_refresh_token = parsed
        .get("refreshToken")
        .and_then(|value| value.as_str())
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false);
    let expires_at = parse_expires_at(parsed.get("expiresAt"));

    Ok(SpotifyConfigSnapshot {
        client_id,
        client_secret,
        redirect_uri,
        access_token,
        has_access_token,
        has_refresh_token,
        expires_at,
        has_config_file: true,
    })
}

#[tauri::command]
fn build_spotify_mcp(server_root: String) -> Result<(), String> {
    let server_root = server_root.trim().to_string();
    if server_root.is_empty() {
        return Err("Missing Spotify MCP server root.".to_string());
    }

    let package_path = Path::new(&server_root).join("package.json");
    if !package_path.exists() {
        return Err("Spotify MCP package.json not found. Check the server path.".to_string());
    }

    let install_status = npm_command()
        .arg("install")
        .current_dir(&server_root)
        .status()
        .map_err(|error| format!("Failed to run npm install: {error}"))?;

    if !install_status.success() {
        return Err(format!("npm install failed with status {install_status}."));
    }

    let build_status = npm_command()
        .arg("run")
        .arg("build")
        .current_dir(&server_root)
        .status()
        .map_err(|error| format!("Failed to run npm build: {error}"))?;

    if !build_status.success() {
        return Err(format!("npm run build failed with status {build_status}."));
    }

    Ok(())
}

#[tauri::command]
fn run_spotify_oauth(server_root: String) -> Result<(), String> {
    let server_root = server_root.trim().to_string();
    if server_root.is_empty() {
        return Err("Missing Spotify MCP server root.".to_string());
    }

    let auth_path = Path::new(&server_root).join("build").join("auth.js");
    if !auth_path.exists() {
        return Err("Spotify MCP auth script not found. Run `npm run build` in the server folder first.".to_string());
    }

    let status = Command::new("node")
        .arg(auth_path)
        .current_dir(server_root)
        .status()
        .map_err(|error| format!("Failed to run Spotify OAuth: {error}"))?;

    if !status.success() {
        return Err(format!("Spotify OAuth exited with status {status}."));
    }

    Ok(())
}

fn toggle_main_window<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            window.hide()?;
        } else {
            let _ = window.center();
            window.show()?;
            window.set_focus()?;
            let _ = window.emit("sarah://show-overlay", ());
        }
    }

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(360))
        .build()
        .expect("Failed to create reqwest client");

    tauri::Builder::default()
        .manage(SpotifyMcpState::default())
        .manage(client)
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.center();
                let _ = window.set_always_on_top(true);
                let _ = window.hide();
            }

            let ctrl_space = Shortcut::new(Some(Modifiers::CONTROL), Code::Space);
            app.global_shortcut()
                .on_shortcut(ctrl_space, |app, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        let _ = toggle_main_window(app);
                    }
                })
                .map_err(|error| error.to_string())?;

            let toggle_item = MenuItemBuilder::with_id("toggle", "Show / Hide Sarah AI").build(app)?;
            let quit_item = MenuItemBuilder::with_id("quit", "Quit Sarah AI").build(app)?;
            let tray_menu = MenuBuilder::new(app)
                .item(&toggle_item)
                .separator()
                .item(&quit_item)
                .build()?;

            let mut tray_builder = TrayIconBuilder::with_id("sarah-tray")
                .menu(&tray_menu)
                .tooltip("Sarah AI")
                .show_menu_on_left_click(false);

            if let Some(icon) = app.default_window_icon().cloned() {
                tray_builder = tray_builder.icon(icon);
            }

            tray_builder
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "toggle" => {
                        let _ = toggle_main_window(app);
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let _ = toggle_main_window(tray.app_handle());
                    }
                })
                .build(app)?;

            Ok(())
        })
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            generate_ollama_response,
            list_ollama_models,
            list_ollama_models_detailed,
            pull_ollama_model,
            open_settings_window,
            open_models_window,
            native_capture::list_active_windows,
            native_capture::get_default_capture_directory,
            native_capture::pick_capture_output_directory,
            native_capture::start_native_screen_recording,
            native_capture::stop_native_screen_recording,
            native_capture::take_native_screenshot,
            open_history_window,
            open_mcp_window,
            open_audio_window,
            close_audio_window,
            emit_audio_command,
            run_spotify_tool,
            start_spotify_mcp,
            stop_spotify_mcp,
            spotify_mcp_status,
            write_spotify_config,
            read_spotify_config,
            build_spotify_mcp,
            run_spotify_oauth
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
