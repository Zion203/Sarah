use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::OnceLock;
use std::time::Duration;

use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tokio::process::{Child, Command};
use tokio::sync::{oneshot, Mutex};

const APP_ENTRY: &str = "index.html";

struct SpotifyMcpProcess {
    child: Child,
}

fn spotify_state() -> &'static Mutex<Option<SpotifyMcpProcess>> {
    static STATE: OnceLock<Mutex<Option<SpotifyMcpProcess>>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(None))
}

fn npm_executable() -> &'static str {
    if cfg!(target_os = "windows") {
        "npm.cmd"
    } else {
        "npm"
    }
}

fn node_executable() -> &'static str {
    if cfg!(target_os = "windows") {
        "node.exe"
    } else {
        "node"
    }
}

fn open_or_focus_window(
    app: &AppHandle,
    label: &str,
    title: &str,
    width: f64,
    height: f64,
    min_width: f64,
    min_height: f64,
) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(label) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        #[cfg(debug_assertions)]
        {
            window.open_devtools();
        }
        return Ok(());
    }

    let window = WebviewWindowBuilder::new(app, label, WebviewUrl::App(APP_ENTRY.into()))
        .initialization_script(build_window_type_init_script(label))
        .title(title)
        .inner_size(width, height)
        .min_inner_size(min_width, min_height)
        .decorations(false)
        .resizable(true)
        .build()
        .map_err(|error| format!("Failed to open {label} window: {error}"))?;

    window
        .show()
        .map_err(|error| format!("Failed to show {label} window: {error}"))?;
    window
        .set_focus()
        .map_err(|error| format!("Failed to focus {label} window: {error}"))?;
    #[cfg(debug_assertions)]
    {
        window.open_devtools();
    }

    Ok(())
}

async fn open_or_focus_window_async(
    app: AppHandle,
    label: &str,
    title: &str,
    width: f64,
    height: f64,
    min_width: f64,
    min_height: f64,
) -> Result<(), String> {
    let label_owned = label.to_string();
    let title_owned = title.to_string();
    let app_for_ui = app.clone();
    let (tx, rx) = oneshot::channel::<Result<(), String>>();

    app.run_on_main_thread(move || {
        let result = open_or_focus_window(
            &app_for_ui,
            &label_owned,
            &title_owned,
            width,
            height,
            min_width,
            min_height,
        );
        let _ = tx.send(result);
    })
    .map_err(|error| format!("Failed to schedule {label} window creation: {error}"))?;

    rx.await
        .map_err(|_| format!("Window task for {label} was cancelled"))?
}

fn build_window_type_init_script(label: &str) -> String {
    let serialized_label =
        serde_json::to_string(label).unwrap_or_else(|_| "\"main\"".to_string());

    format!(
        r#"(function () {{
  const windowType = {serialized_label};
  try {{
    window.__SARAH_WINDOW_TYPE__ = windowType;
    const url = new URL(window.location.href);
    if (!url.searchParams.get("window")) {{
      url.searchParams.set("window", windowType);
      history.replaceState(history.state, "", url.toString());
    }}
  }} catch (_error) {{
    window.__SARAH_WINDOW_TYPE__ = windowType;
  }}
}})();"#,
    )
}

fn resolve_directory(path: &str, field_name: &str) -> Result<PathBuf, String> {
    let normalized = path.trim();
    if normalized.is_empty() {
        return Err(format!("{field_name} is required"));
    }

    let directory = PathBuf::from(normalized);
    if !directory.exists() {
        return Err(format!(
            "{field_name} does not exist: {}",
            directory.display()
        ));
    }
    if !directory.is_dir() {
        return Err(format!(
            "{field_name} must be a directory: {}",
            directory.display()
        ));
    }

    Ok(directory)
}

fn resolve_file(path: &str, field_name: &str) -> Result<PathBuf, String> {
    let normalized = path.trim();
    if normalized.is_empty() {
        return Err(format!("{field_name} is required"));
    }

    let file_path = PathBuf::from(normalized);
    if !file_path.exists() {
        return Err(format!(
            "{field_name} does not exist: {}",
            file_path.display()
        ));
    }
    if !file_path.is_file() {
        return Err(format!(
            "{field_name} must be a file: {}",
            file_path.display()
        ));
    }

    Ok(file_path)
}

fn read_npm_scripts(server_root: &Path) -> Result<Vec<String>, String> {
    let package_path = server_root.join("package.json");
    if !package_path.exists() {
        return Err(format!(
            "package.json not found in serverRoot: {}",
            server_root.display()
        ));
    }

    let raw = std::fs::read_to_string(&package_path)
        .map_err(|error| format!("Failed to read {}: {error}", package_path.display()))?;
    let parsed: Value = serde_json::from_str(&raw).map_err(|error| {
        format!(
            "Invalid package.json format in {}: {error}",
            package_path.display()
        )
    })?;
    let scripts = parsed
        .get("scripts")
        .and_then(Value::as_object)
        .ok_or_else(|| "package.json does not include a scripts object".to_string())?;

    let mut names = scripts.keys().cloned().collect::<Vec<_>>();
    names.sort();
    Ok(names)
}

fn find_script(scripts: &[String], candidates: &[&str]) -> Option<String> {
    for candidate in candidates {
        if scripts.iter().any(|script| script == candidate) {
            return Some((*candidate).to_string());
        }
    }
    None
}

async fn run_command_output(mut command: Command, timeout_seconds: u64) -> Result<String, String> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let output = tokio::time::timeout(Duration::from_secs(timeout_seconds), command.output())
        .await
        .map_err(|_| format!("Command timed out after {timeout_seconds} seconds"))?
        .map_err(|error| format!("Failed to execute command: {error}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    if !output.status.success() {
        let detail = if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            format!("Exit status: {}", output.status)
        };
        return Err(detail);
    }

    Ok(stdout)
}

async fn run_npm_script(
    server_root: &Path,
    script: &str,
    extra_args: &[String],
    timeout_seconds: u64,
) -> Result<String, String> {
    let mut command = Command::new(npm_executable());
    command.current_dir(server_root).arg("run").arg(script);

    if !extra_args.is_empty() {
        command.arg("--");
        for arg in extra_args {
            command.arg(arg);
        }
    }

    run_command_output(command, timeout_seconds).await
}

fn maybe_emit_audio_event(app: &AppHandle, tool: &str) {
    let action = match tool {
        "resumePlayback" | "playMusic" => Some("play"),
        "pausePlayback" => Some("pause"),
        "skipToNext" => Some("next"),
        "skipToPrevious" => Some("prev"),
        _ => None,
    };

    if let Some(action) = action {
        let _ = app.emit(
            "sarah://audio-control",
            serde_json::json!({
                "action": action,
            }),
        );
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpotifyConfigSnapshot {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub access_token: Option<String>,
    pub has_access_token: bool,
    pub has_refresh_token: bool,
    pub expires_at: Option<u64>,
    pub has_config_file: bool,
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

#[derive(serde::Serialize, Clone)]
struct AudioCommandPayload {
    action: String,
}

#[tauri::command]
pub async fn emit_audio_command(app: AppHandle, action: String) -> Result<(), String> {
    let payload = AudioCommandPayload {
        action: action.trim().to_string(),
    };
    let _ = app.emit("sarah://audio-control", payload);
    Ok(())
}

#[tauri::command]
pub fn read_spotify_config(server_root: String) -> Result<SpotifyConfigSnapshot, String> {
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
pub async fn open_history_window(app: AppHandle) -> Result<(), String> {
    crate::log_info!("sarah.command", "open_history_window invoked");
    open_or_focus_window_async(
        app,
        "history",
        "Sarah AI History",
        1080.0,
        760.0,
        780.0,
        560.0,
    )
    .await
}

#[tauri::command]
pub async fn open_settings_window(app: AppHandle) -> Result<(), String> {
    crate::log_info!("sarah.command", "open_settings_window invoked");
    open_or_focus_window_async(
        app,
        "settings",
        "Sarah AI Settings",
        1120.0,
        780.0,
        860.0,
        600.0,
    )
    .await
}

#[tauri::command]
pub async fn open_models_window(app: AppHandle) -> Result<(), String> {
    crate::log_info!("sarah.command", "open_models_window invoked");
    open_or_focus_window_async(
        app,
        "models",
        "Sarah AI Models",
        1120.0,
        760.0,
        860.0,
        560.0,
    )
    .await
}

#[tauri::command]
pub async fn open_mcp_window(app: AppHandle) -> Result<(), String> {
    crate::log_info!("sarah.command", "open_mcp_window invoked");
    open_or_focus_window_async(
        app,
        "mcp",
        "Sarah AI MCP Marketplace",
        1140.0,
        780.0,
        900.0,
        600.0,
    )
    .await
}

#[tauri::command]
pub async fn open_audio_window(app: AppHandle) -> Result<(), String> {
    crate::log_info!("sarah.command", "open_audio_window invoked");
    open_or_focus_window_async(app, "audio", "Sarah AI Audio", 520.0, 260.0, 420.0, 220.0).await
}

#[tauri::command]
pub fn close_audio_window(app: AppHandle) -> Result<(), String> {
    crate::log_info!("sarah.command", "close_audio_window invoked");
    if let Some(window) = app.get_webview_window("audio") {
        window
            .close()
            .map_err(|error| format!("Failed to close audio window: {error}"))?;
    }
    Ok(())
}

#[tauri::command]
pub async fn spotify_mcp_status() -> Result<bool, String> {
    crate::log_info!("sarah.command", "spotify_mcp_status invoked");
    let mut guard = spotify_state().lock().await;

    match guard.as_mut() {
        Some(process) => match process.child.try_wait() {
            Ok(Some(_)) => {
                *guard = None;
                Ok(false)
            }
            Ok(None) => Ok(true),
            Err(error) => Err(format!("Failed checking Spotify MCP process: {error}")),
        },
        None => Ok(false),
    }
}

#[tauri::command]
pub async fn start_spotify_mcp(entry_path: String) -> Result<(), String> {
    crate::log_info!("sarah.command", "start_spotify_mcp invoked");
    let entry_path = resolve_file(&entry_path, "entryPath")?;
    let working_dir = entry_path
        .parent()
        .ok_or_else(|| "entryPath must include a parent directory".to_string())?
        .to_path_buf();

    let mut guard = spotify_state().lock().await;
    if let Some(process) = guard.as_mut() {
        match process.child.try_wait() {
            Ok(None) => return Ok(()),
            Ok(Some(_)) => {
                *guard = None;
            }
            Err(error) => {
                return Err(format!(
                    "Failed checking existing Spotify MCP process: {error}"
                ))
            }
        }
    }

    let mut command = Command::new(node_executable());
    command
        .current_dir(working_dir)
        .arg(&entry_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let child = command
        .spawn()
        .map_err(|error| format!("Failed to start Spotify MCP process: {error}"))?;

    *guard = Some(SpotifyMcpProcess { child });
    Ok(())
}

#[tauri::command]
pub async fn stop_spotify_mcp() -> Result<(), String> {
    crate::log_info!("sarah.command", "stop_spotify_mcp invoked");
    let mut guard = spotify_state().lock().await;
    let Some(mut process) = guard.take() else {
        return Ok(());
    };

    match process.child.try_wait() {
        Ok(Some(_)) => Ok(()),
        Ok(None) => {
            process
                .child
                .kill()
                .await
                .map_err(|error| format!("Failed to stop Spotify MCP process: {error}"))?;
            let _ = process.child.wait().await;
            Ok(())
        }
        Err(error) => Err(format!("Failed checking Spotify MCP process: {error}")),
    }
}

#[tauri::command]
pub async fn build_spotify_mcp(server_root: String) -> Result<(), String> {
    crate::log_info!("sarah.command", "build_spotify_mcp invoked");
    let server_root = resolve_directory(&server_root, "serverRoot")?;
    let scripts = read_npm_scripts(&server_root)?;
    let script = find_script(&scripts, &["build", "spotify:build"]).ok_or_else(|| {
        format!(
            "No build script found. Expected one of [build, spotify:build]. Available scripts: {}",
            scripts.join(", ")
        )
    })?;

    let _ = run_npm_script(&server_root, &script, &[], 180).await?;
    Ok(())
}

#[tauri::command]
pub async fn run_spotify_oauth(server_root: String) -> Result<(), String> {
    crate::log_info!("sarah.command", "run_spotify_oauth invoked");
    let server_root = resolve_directory(&server_root, "serverRoot")?;
    let scripts = read_npm_scripts(&server_root)?;
    let script = find_script(
        &scripts,
        &[
            "oauth",
            "auth",
            "spotify:oauth",
            "spotify:auth",
            "login",
        ],
    )
    .ok_or_else(|| {
        format!(
            "No OAuth script found. Expected one of [oauth, auth, spotify:oauth, spotify:auth, login]. Available scripts: {}",
            scripts.join(", ")
        )
    })?;

    let _ = run_npm_script(&server_root, &script, &[], 300).await?;
    Ok(())
}

#[tauri::command]
pub fn write_spotify_config(
    server_root: String,
    client_id: String,
    client_secret: String,
    redirect_uri: String,
) -> Result<(), String> {
    crate::log_info!("sarah.command", "write_spotify_config invoked");
    let server_root = resolve_directory(&server_root, "serverRoot")?;
    let config_path = server_root.join("spotify-config.json");

    let payload = serde_json::json!({
        "clientId": client_id,
        "clientSecret": client_secret,
        "redirectUri": redirect_uri,
    });

    let content = serde_json::to_string_pretty(&payload)
        .map_err(|error| format!("Failed to serialize Spotify config: {error}"))?;
    std::fs::write(&config_path, content)
        .map_err(|error| format!("Failed to write {}: {error}", config_path.display()))?;

    Ok(())
}

#[tauri::command]
pub async fn run_spotify_tool(
    app: AppHandle,
    server_root: String,
    tool: String,
    args: Value,
) -> Result<String, String> {
    crate::log_info!("sarah.command", "run_spotify_tool invoked");
    let server_root = resolve_directory(&server_root, "serverRoot")?;
    let tool_name = tool.trim();
    if tool_name.is_empty() {
        return Err("tool is required".to_string());
    }

    maybe_emit_audio_event(&app, tool_name);

    let args_json = serde_json::to_string(&args)
        .map_err(|error| format!("Failed to serialize tool arguments: {error}"))?;
    let scripts = read_npm_scripts(&server_root).ok();

    if let Some(script_names) = scripts.as_ref() {
        if let Some(script) = find_script(
            script_names,
            &["tool", "call-tool", "spotify:tool", "mcp:tool", "run-tool"],
        ) {
            let output = run_npm_script(
                &server_root,
                &script,
                &[tool_name.to_string(), args_json.clone()],
                180,
            )
            .await?;
            return Ok(output);
        }
    }

    let fallback_entry = server_root.join("build").join("index.js");
    if !fallback_entry.exists() {
        return Err(format!(
            "No tool runner script found and fallback entry is missing: {}. Run build_spotify_mcp first.",
            fallback_entry.display()
        ));
    }

    let mut command = Command::new(node_executable());
    command
        .current_dir(&server_root)
        .arg(fallback_entry)
        .arg(tool_name)
        .arg(args_json);

    run_command_output(command, 180).await
}
