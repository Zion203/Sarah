#![allow(dead_code, unused_imports)]

use std::sync::Arc;

use tauri::Manager;
use tauri::Emitter;

use std::process::Child;
use std::time::Duration;
use std::sync::Mutex;

mod commands;
mod db;
mod error;
mod logging;
mod native_capture;
mod repositories;
mod services;
mod state;

pub struct SpotifyMcpState(Mutex<Option<Child>>);

impl Default for SpotifyMcpState {
    fn default() -> Self {
        Self(Mutex::new(None))
    }
}
use crate::commands::analytics_commands::{get_recent_perf_logs, run_analytics_aggregation};
use crate::commands::chat_commands::{
    archive_session, create_session, get_session_messages, list_sessions, search_conversations,
    send_message,
};
use crate::commands::integration_commands::{
    build_spotify_mcp, close_audio_window, emit_audio_command, open_audio_window,
    open_history_window, open_mcp_window, open_models_window, open_settings_window,
    read_spotify_config, run_spotify_oauth, run_spotify_tool, spotify_mcp_status,
    start_spotify_mcp, stop_spotify_mcp, write_spotify_config,
};
use crate::commands::local_commands::{
    clear_local_chat_history, download_local_model, generate_local_response,
    generate_ollama_response, get_default_user, get_local_chat_history, greet,
    list_local_models, list_local_models_detailed, list_ollama_models,
    list_ollama_models_detailed, pull_ollama_model,
};
use crate::commands::mcp_commands::{
    activate_mcp, deactivate_mcp, get_mcp_stats, install_mcp, list_mcps, save_mcp_secret,
    test_mcp_connection,
};
use crate::commands::memory_commands::{
    delete_memory, get_memories, get_memory_graph, pin_memory, search_memories, update_memory,
};
use crate::commands::model_commands::{
    get_download_progress, get_installed_models, get_model_catalog, get_model_compatibility_score,
    get_recommended_models, run_nlp_setup, set_default_model, start_model_download,
};
use crate::commands::rag_commands::{embed_document, ingest_document, retrieve_knowledge};
use crate::commands::runtime_commands::{
    get_model_routing_decision, get_optimization_stats, get_performance_dashboard,
    get_runtime_policy, get_runtime_profile, get_service_health, get_setup_status,
    get_startup_telemetry, retry_setup_stage, run_model_microbenchmark, set_runtime_policy,
    skip_quality_upgrade_for_now, start_first_run_setup,
};
use crate::commands::settings_commands::{get_setting, list_settings_namespace, set_setting};
use crate::commands::system_commands::{
    get_hardware_profile, get_system_stats, run_hardware_benchmark,
};
use crate::state::AppState;

fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn,sarah_lib=info"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_thread_ids(false)
        .with_line_number(false)
        .init();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_tracing();

    log_info!("sarah", "Starting Sarah AI application");

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(360))
        .build()
        .expect("Failed to create reqwest client");

    tauri::Builder::default()
        .manage(SpotifyMcpState::default())
        .manage(client)
        .setup(|app| {
            let app_handle = app.handle().clone();

            if let Some(app_dir) = app_handle.path().app_data_dir().ok() {
                if let Err(e) = logging::init_logging(&app_dir) {
                    eprintln!("Failed to init file logging: {}", e);
                }
            }

            let state = tauri::async_runtime::block_on(AppState::initialize(&app_handle)).map_err(
                |error| std::io::Error::new(std::io::ErrorKind::Other, error.to_string()),
            )?;

            app.manage(Arc::new(state));

            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }

            // Signal frontend that the backend is ready
            let _ = app.emit("backend-ready", true);

            log_info!("sarah", "Application setup complete");

            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            greet,
            get_default_user,
            generate_local_response,
            list_local_models,
            list_local_models_detailed,
            download_local_model,
            get_local_chat_history,
            clear_local_chat_history,
            send_message,
            create_session,
            list_sessions,
            get_session_messages,
            archive_session,
            search_conversations,
            get_installed_models,
            get_model_catalog,
            get_recommended_models,
            set_default_model,
            get_model_compatibility_score,
            run_nlp_setup,
            start_model_download,
            get_download_progress,
            get_memories,
            search_memories,
            delete_memory,
            pin_memory,
            update_memory,
            get_memory_graph,
            get_hardware_profile,
            run_hardware_benchmark,
            get_system_stats,
            list_mcps,
            install_mcp,
            activate_mcp,
            deactivate_mcp,
            save_mcp_secret,
            test_mcp_connection,
            get_mcp_stats,
            ingest_document,
            embed_document,
            retrieve_knowledge,
            get_runtime_policy,
            set_runtime_policy,
            get_runtime_profile,
            get_service_health,
            get_optimization_stats,
            get_startup_telemetry,
            run_model_microbenchmark,
            get_model_routing_decision,
            get_performance_dashboard,
            start_first_run_setup,
            get_setup_status,
            retry_setup_stage,
            skip_quality_upgrade_for_now,
            get_setting,
            set_setting,
            list_settings_namespace,
            get_recent_perf_logs,
            run_analytics_aggregation,
            open_history_window,
            open_settings_window,
            open_models_window,
            open_mcp_window,
            open_audio_window,
            close_audio_window,
            spotify_mcp_status,
            start_spotify_mcp,
            stop_spotify_mcp,
            run_spotify_oauth,
            build_spotify_mcp,
            write_spotify_config,
            run_spotify_tool,
            native_capture::list_active_windows,
            native_capture::get_default_capture_directory,
            native_capture::pick_capture_output_directory,
            native_capture::start_native_screen_recording,
            native_capture::stop_native_screen_recording,
            native_capture::take_native_screenshot,
            native_capture::validate_capture_path,
            generate_ollama_response,
            list_ollama_models,
            list_ollama_models_detailed,
            pull_ollama_model,
            emit_audio_command,
            read_spotify_config

        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::ExitRequested { .. } = &event {
                let state = app_handle.state::<Arc<AppState>>();
                let background = state.background.clone();
                let inference = state.inference.clone();
                let db = state.db.clone();

                tauri::async_runtime::block_on(async {
                    tracing::info!("App exit requested — starting graceful shutdown");
                    background.stop_all().await;
                    inference.shutdown().await;
                    db.optimize().await;
                    tracing::info!("Graceful shutdown complete");
                });
            }
        });
}
