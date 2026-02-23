use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, Runtime, WebviewUrl, WebviewWindowBuilder,
};
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

async fn fetch_ollama_tags() -> Result<OllamaTagsResponse, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|error| format!("Failed to initialize HTTP client: {error}"))?;

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

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn generate_ollama_response(prompt: String, model: Option<String>) -> Result<String, String> {
    let prompt = prompt.trim().to_string();
    if prompt.is_empty() {
        return Err("Prompt is empty.".to_string());
    }

    let model = model
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "llama3.1:8b".to_string());

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(180))
        .build()
        .map_err(|error| format!("Failed to initialize HTTP client: {error}"))?;

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
async fn list_ollama_models() -> Result<Vec<String>, String> {
    let payload = fetch_ollama_tags().await?;

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
async fn list_ollama_models_detailed() -> Result<Vec<OllamaModelSummary>, String> {
    let payload = fetch_ollama_tags().await?;
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
async fn pull_ollama_model(model: String) -> Result<String, String> {
    let normalized = model.trim().to_string();
    if normalized.is_empty() {
        return Err("Model name is empty.".to_string());
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(360))
        .build()
        .map_err(|error| format!("Failed to initialize HTTP client: {error}"))?;

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
    tauri::Builder::default()
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
            open_history_window,
            native_capture::list_active_windows,
            native_capture::get_default_capture_directory,
            native_capture::pick_capture_output_directory,
            native_capture::start_native_screen_recording,
            native_capture::stop_native_screen_recording,
            native_capture::take_native_screenshot
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
