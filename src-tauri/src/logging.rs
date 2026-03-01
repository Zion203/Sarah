use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

use chrono::Local;

static LOG_FILE: Mutex<Option<PathBuf>> = Mutex::new(None);

pub fn init_logging(app_data_dir: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let logs_dir = app_data_dir.join("logs");
    fs::create_dir_all(&logs_dir)?;

    let log_file = logs_dir.join(format!("sarah_{}.log", Local::now().format("%Y-%m-%d")));

    let mut guard = LOG_FILE.lock().unwrap();
    *guard = Some(log_file.clone());
    drop(guard);

    tracing::info!("Logging initialized. Log file: {:?}", log_file);

    Ok(())
}

pub fn log_to_file(level: &str, target: &str, message: &str) {
    if let Ok(guard) = LOG_FILE.lock() {
        if let Some(ref path) = *guard {
            if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
                let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
                let _ = writeln!(file, "{} [{}] {}: {}", timestamp, level, target, message);
            }
        }
    }
}

#[macro_export]
macro_rules! log_info {
    ($target:expr, $($arg:tt)*) => ({
        let msg = format!($($arg)*);
        $crate::logging::log_to_file("INFO", $target, &msg);
        tracing::info!(target: $target, "{}", msg);
    });
}

#[macro_export]
macro_rules! log_warn {
    ($target:expr, $($arg:tt)*) => ({
        let msg = format!($($arg)*);
        $crate::logging::log_to_file("WARN", $target, &msg);
        tracing::warn!(target: $target, "{}", msg);
    });
}

#[macro_export]
macro_rules! log_error {
    ($target:expr, $($arg:tt)*) => ({
        let msg = format!($($arg)*);
        $crate::logging::log_to_file("ERROR", $target, &msg);
        tracing::error!(target: $target, "{}", msg);
    });
}

pub fn init_tracing() {
    use tracing_subscriber::EnvFilter;

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("warn,sarah_lib=info,llama=warn"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_thread_ids(false)
        .init();

    log_to_file("INFO", "sarah", "Application started");
}
