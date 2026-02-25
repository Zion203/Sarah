use std::ffi::c_void;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use rfd::FileDialog;
use windows_capture::capture::{Context, GraphicsCaptureApiHandler};
use windows_capture::encoder::{
    AudioSettingsBuilder, ContainerSettingsBuilder, VideoEncoder, VideoSettingsBuilder,
    VideoSettingsSubType,
};
use windows_capture::frame::{Frame, ImageFormat};
use windows_capture::graphics_capture_api::InternalCaptureControl;
use windows_capture::monitor::Monitor;
use windows_capture::settings::{
    ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
    MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
};
use windows_capture::window::Window;

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CaptureSurface {
    Screen,
    Window,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveWindowSource {
    pub id: String,
    pub process_name: String,
    pub title: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeRecordingResult {
    pub duration_ms: u64,
    pub ended_at_ms: u64,
    pub mime_type: String,
    pub started_at_ms: u64,
    pub video_path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeScreenshotResult {
    pub captured_at_ms: u64,
    pub screenshot_path: String,
}

#[derive(Debug)]
struct RecordingArtifacts {
    duration_ms: u64,
    ended_at_ms: u64,
    video_path: PathBuf,
}

#[derive(Debug)]
struct NativeCaptureSession {
    join_handle: JoinHandle<Result<RecordingArtifacts, String>>,
    started_at_ms: u64,
    stop_flag: Arc<AtomicBool>,
}

#[derive(Default)]
struct NativeCaptureState {
    active: Option<NativeCaptureSession>,
}

struct EncoderCapture {
    encoder: Option<VideoEncoder>,
    stop_flag: Arc<AtomicBool>,
}

struct ScreenshotCapture {
    saved: bool,
    screenshot_path: PathBuf,
}

impl GraphicsCaptureApiHandler for EncoderCapture {
    type Error = Box<dyn std::error::Error + Send + Sync>;
    type Flags = (Arc<AtomicBool>, PathBuf, u32, u32);

    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        let (stop_flag, video_path, width, height) = ctx.flags;
        let video_settings =
            VideoSettingsBuilder::new(width, height).sub_type(VideoSettingsSubType::H264);
        let encoder = VideoEncoder::new(
            video_settings,
            AudioSettingsBuilder::default().disabled(true),
            ContainerSettingsBuilder::default(),
            &video_path,
        )?;

        Ok(Self {
            encoder: Some(encoder),
            stop_flag,
        })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        capture_control: InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        if let Some(encoder) = self.encoder.as_mut() {
            encoder.send_frame(frame)?;
        }

        if self.stop_flag.load(Ordering::SeqCst) {
            if let Some(encoder) = self.encoder.take() {
                encoder.finish()?;
            }
            capture_control.stop();
        }

        Ok(())
    }

    fn on_closed(&mut self) -> Result<(), Self::Error> {
        self.stop_flag.store(true, Ordering::SeqCst);
        Ok(())
    }
}

impl GraphicsCaptureApiHandler for ScreenshotCapture {
    type Error = Box<dyn std::error::Error + Send + Sync>;
    type Flags = PathBuf;

    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        Ok(Self {
            saved: false,
            screenshot_path: ctx.flags,
        })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        capture_control: InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        if !self.saved {
            frame.save_as_image(&self.screenshot_path, ImageFormat::Png)?;
            self.saved = true;
        }
        capture_control.stop();
        Ok(())
    }
}

fn state() -> &'static Mutex<NativeCaptureState> {
    static STATE: OnceLock<Mutex<NativeCaptureState>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(NativeCaptureState::default()))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn default_capture_directory() -> Result<PathBuf, String> {
    let base = std::env::temp_dir().join("sarah-screen-recordings");
    fs::create_dir_all(&base)
        .map_err(|error| format!("Failed to create recording directory: {error}"))?;
    Ok(base)
}

fn resolve_capture_directory(output_directory: Option<String>) -> Result<PathBuf, String> {
    let base = output_directory
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or(default_capture_directory()?);

    fs::create_dir_all(&base)
        .map_err(|error| format!("Failed to create recording directory: {error}"))?;
    Ok(base)
}

fn recording_output_path(output_directory: Option<String>) -> Result<PathBuf, String> {
    let base = resolve_capture_directory(output_directory)?;

    let stamp = now_ms();
    let video = base.join(format!("sarah-screen-recording-{stamp}.mp4"));
    Ok(video)
}

fn screenshot_output_path(output_directory: Option<String>) -> Result<PathBuf, String> {
    let base = resolve_capture_directory(output_directory)?;
    let stamp = now_ms();
    Ok(base.join(format!("sarah-screenshot-{stamp}.png")))
}

#[tauri::command]
pub fn get_default_capture_directory() -> Result<String, String> {
    let path = default_capture_directory()?;
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub fn pick_capture_output_directory(initial_directory: Option<String>) -> Result<Option<String>, String> {
    let mut dialog = FileDialog::new();

    if let Some(path) = initial_directory
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        dialog = dialog.set_directory(path);
    } else if let Ok(default_path) = default_capture_directory() {
        dialog = dialog.set_directory(default_path);
    }

    Ok(dialog
        .pick_folder()
        .map(|path| path.to_string_lossy().to_string()))
}

fn parse_window_handle(raw: Option<String>) -> Result<Option<u64>, String> {
    raw.as_deref()
        .map(|value| {
            value
                .trim()
                .parse::<u64>()
                .map_err(|_| "Invalid window handle was provided.".to_string())
        })
        .transpose()
}

fn capture_single_screenshot(
    surface: CaptureSurface,
    window_hwnd: Option<u64>,
    screenshot_path: PathBuf,
) -> Result<(), String> {
    match surface {
        CaptureSurface::Screen => {
            let monitor = Monitor::primary()
                .map_err(|error| format!("Failed to access primary monitor: {error}"))?;
            let mut last_error = None;
            for color_format in [ColorFormat::Rgba8, ColorFormat::Bgra8] {
                let settings = Settings::new(
                    monitor,
                    CursorCaptureSettings::Default,
                    DrawBorderSettings::WithoutBorder,
                    SecondaryWindowSettings::Default,
                    MinimumUpdateIntervalSettings::Default,
                    DirtyRegionSettings::Default,
                    color_format,
                    screenshot_path.clone(),
                );
                match ScreenshotCapture::start_free_threaded(settings) {
                    Ok(control) => match control.wait() {
                        Ok(()) => {
                            last_error = None;
                            break;
                        }
                        Err(error) => {
                            last_error = Some(format!("Native screenshot failed: {error}"));
                        }
                    },
                    Err(error) => {
                        last_error = Some(format!("Native screenshot failed: {error}"));
                    }
                }
            }
            if let Some(error) = last_error {
                return Err(error);
            }
        }
        CaptureSurface::Window => {
            let window = window_hwnd
                .map(|value| Window::from_raw_hwnd(value as usize as *mut c_void))
                .ok_or_else(|| "Window mode requires a selected window.".to_string())?;

            if !window.is_valid() {
                return Err("Selected window is no longer valid for capture.".to_string());
            }

            let mut last_error = None;
            for color_format in [ColorFormat::Rgba8, ColorFormat::Bgra8] {
                let settings = Settings::new(
                    window,
                    CursorCaptureSettings::Default,
                    DrawBorderSettings::WithoutBorder,
                    SecondaryWindowSettings::Default,
                    MinimumUpdateIntervalSettings::Default,
                    DirtyRegionSettings::Default,
                    color_format,
                    screenshot_path.clone(),
                );
                match ScreenshotCapture::start_free_threaded(settings) {
                    Ok(control) => match control.wait() {
                        Ok(()) => {
                            last_error = None;
                            break;
                        }
                        Err(error) => {
                            last_error = Some(format!("Native screenshot failed: {error}"));
                        }
                    },
                    Err(error) => {
                        last_error = Some(format!("Native screenshot failed: {error}"));
                    }
                }
            }
            if let Some(error) = last_error {
                return Err(error);
            }
        }
    }

    Ok(())
}

#[tauri::command]
pub fn take_native_screenshot(
    surface: CaptureSurface,
    window_hwnd: Option<String>,
    output_directory: Option<String>,
) -> Result<NativeScreenshotResult, String> {
    let parsed_window_handle = parse_window_handle(window_hwnd)?;
    if matches!(surface, CaptureSurface::Window) && parsed_window_handle.is_none() {
        return Err("Window mode requires a selected window.".to_string());
    }

    let screenshot_path = screenshot_output_path(output_directory)?;
    capture_single_screenshot(surface, parsed_window_handle, screenshot_path.clone())?;

    if !Path::new(&screenshot_path).exists() {
        return Err("Screenshot could not be saved.".to_string());
    }

    Ok(NativeScreenshotResult {
        captured_at_ms: now_ms(),
        screenshot_path: screenshot_path.to_string_lossy().to_string(),
    })
}

fn compute_dimensions_for_window(window: Window) -> Result<(u32, u32), String> {
    let rect = window
        .rect()
        .map_err(|error| format!("Failed to get selected window bounds: {error}"))?;
    let width = (rect.right - rect.left).max(2) as u32;
    let height = (rect.bottom - rect.top).max(2) as u32;
    Ok((width, height))
}

fn spawn_capture_thread(
    surface: CaptureSurface,
    window_hwnd: Option<u64>,
    stop_flag: Arc<AtomicBool>,
    video_path: PathBuf,
) -> JoinHandle<Result<RecordingArtifacts, String>> {
    thread::spawn(move || {
        let started = Instant::now();
        match surface {
            CaptureSurface::Screen => {
                let monitor = Monitor::primary()
                    .map_err(|error| format!("Failed to access primary monitor: {error}"))?;
                let width = monitor
                    .width()
                    .map_err(|error| format!("Failed to read monitor width: {error}"))?;
                let height = monitor
                    .height()
                    .map_err(|error| format!("Failed to read monitor height: {error}"))?;

                let settings = Settings::new(
                    monitor,
                    CursorCaptureSettings::Default,
                    DrawBorderSettings::WithoutBorder,
                    SecondaryWindowSettings::Default,
                    MinimumUpdateIntervalSettings::Default,
                    DirtyRegionSettings::Default,
                    ColorFormat::Bgra8,
                    (
                        stop_flag.clone(),
                        video_path.clone(),
                        width,
                        height,
                    ),
                );
                EncoderCapture::start(settings)
                    .map_err(|error| format!("Native capture failed: {error}"))?;
            }
            CaptureSurface::Window => {
                let window = window_hwnd
                    .map(|value| Window::from_raw_hwnd(value as usize as *mut c_void))
                    .ok_or_else(|| "Window handle was not provided.".to_string())?;

                if !window.is_valid() {
                    return Err("Selected window is no longer valid for capture.".to_string());
                }

                let (width, height) = compute_dimensions_for_window(window)?;
                let settings = Settings::new(
                    window,
                    CursorCaptureSettings::Default,
                    DrawBorderSettings::WithoutBorder,
                    SecondaryWindowSettings::Default,
                    MinimumUpdateIntervalSettings::Default,
                    DirtyRegionSettings::Default,
                    ColorFormat::Bgra8,
                    (
                        stop_flag.clone(),
                        video_path.clone(),
                        width,
                        height,
                    ),
                );
                EncoderCapture::start(settings)
                    .map_err(|error| format!("Native capture failed: {error}"))?;
            }
        }

        let duration_ms = started.elapsed().as_millis() as u64;
        let ended_at_ms = now_ms();

        Ok(RecordingArtifacts {
            duration_ms,
            ended_at_ms,
            video_path,
        })
    })
}

fn cleanup_finished_session_if_any(state: &mut NativeCaptureState) {
    let should_cleanup = state
        .active
        .as_ref()
        .map(|session| session.join_handle.is_finished())
        .unwrap_or(false);

    if should_cleanup {
        if let Some(session) = state.active.take() {
            let _ = session.join_handle.join();
        }
    }
}

#[tauri::command]
pub fn list_active_windows() -> Result<Vec<ActiveWindowSource>, String> {
    let windows = Window::enumerate().map_err(|error| format!("Failed to enumerate windows: {error}"))?;
    let mut items = Vec::new();

    for window in windows {
        if !window.is_valid() {
            continue;
        }

        let title = window
            .title()
            .map_err(|error| format!("Failed to read window title: {error}"))?;
        if title.trim().is_empty() {
            continue;
        }

        let process_name = window
            .process_name()
            .unwrap_or_else(|_| "Unknown app".to_string());

        items.push(ActiveWindowSource {
            id: format!("{}", window.as_raw_hwnd() as usize as u64),
            process_name,
            title,
        });
    }

    items.sort_by(|left, right| left.title.to_lowercase().cmp(&right.title.to_lowercase()));
    Ok(items)
}

#[tauri::command]
pub fn start_native_screen_recording(
    surface: CaptureSurface,
    window_hwnd: Option<String>,
    output_directory: Option<String>,
) -> Result<(), String> {
    let mut guard = state()
        .lock()
        .map_err(|_| "Capture state lock was poisoned.".to_string())?;

    cleanup_finished_session_if_any(&mut guard);
    if guard.active.is_some() {
        return Err("Screen recording is already running.".to_string());
    }

    if matches!(surface, CaptureSurface::Window) && window_hwnd.is_none() {
        return Err("Window mode requires a selected window.".to_string());
    }

    let raw_window_handle = parse_window_handle(window_hwnd)?;

    let video_path = recording_output_path(output_directory)?;
    let started_at_ms = now_ms();
    let stop_flag = Arc::new(AtomicBool::new(false));
    let join_handle =
        spawn_capture_thread(surface, raw_window_handle, stop_flag.clone(), video_path);

    guard.active = Some(NativeCaptureSession {
        join_handle,
        started_at_ms,
        stop_flag,
    });

    Ok(())
}

#[tauri::command]
pub fn stop_native_screen_recording() -> Result<NativeRecordingResult, String> {
    let session = {
        let mut guard = state()
            .lock()
            .map_err(|_| "Capture state lock was poisoned.".to_string())?;
        cleanup_finished_session_if_any(&mut guard);
        guard
            .active
            .take()
            .ok_or_else(|| "No active screen recording to stop.".to_string())?
    };

    session.stop_flag.store(true, Ordering::SeqCst);
    let started_at_ms = session.started_at_ms;

    let result = session
        .join_handle
        .join()
        .map_err(|_| "Native capture thread panicked.".to_string())??;

    Ok(NativeRecordingResult {
        duration_ms: result.duration_ms,
        ended_at_ms: result.ended_at_ms,
        mime_type: "video/mp4".to_string(),
        started_at_ms,
        video_path: result.video_path.to_string_lossy().to_string(),
    })
}
