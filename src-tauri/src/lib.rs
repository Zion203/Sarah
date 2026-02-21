use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, Runtime, WebviewUrl, WebviewWindowBuilder,
};
use std::time::Duration;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

#[derive(serde::Deserialize)]
struct OllamaGenerateResponse {
    response: String,
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
async fn open_settings_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("settings") {
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
    .resizable(true)
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
            open_settings_window,
            open_history_window
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
