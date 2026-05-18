//! Tauri command surface. Frontend talks to these via `invoke()`.
//!
//! Long-running operations (`detect_silence`, `export_mp4`) run on a worker
//! thread and emit progress events via the Tauri Emitter API. Frontend
//! subscribes via `listen('export-progress', ...)` etc.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::audio::extract_pcm;
use crate::binaries::{ffmpeg_path, ffprobe_path};
use crate::cutlist::CutList;
use crate::export_fcpxml::{render as render_fcpxml, FcpxmlParams};
use crate::export_mp4;
use crate::probe::{probe, VideoInfo};
use crate::vad::{detect as detect_vad, VadParams};
use crate::waveform::extract_waveform;

/// Render an anyhow::Error including the full chain of `with_context`
/// messages. Plain `.to_string()` would only show the outermost message,
/// hiding the actual root cause (e.g. "spawning ffprobe at ...: No such
/// file or directory" instead of just "spawning ffprobe at ...").
fn fmt_err(e: impl std::fmt::Display) -> String {
    format!("{e:#}")
}

#[derive(Default)]
pub struct AppState {
    /// Single-job cancellation flag. Set by `cancel_export`, polled by the
    /// export worker. We only support one concurrent export.
    pub export_cancel: Mutex<Option<Arc<AtomicBool>>>,
}

#[derive(Debug, Deserialize)]
pub struct DetectParams {
    pub threshold: f32,
    pub min_silence_ms: u32,
    pub min_speech_ms: u32,
    pub pad: f64,
    pub preview_range: Option<(f64, f64)>,
}

impl From<&DetectParams> for VadParams {
    fn from(p: &DetectParams) -> Self {
        VadParams {
            threshold: p.threshold,
            min_silence_ms: p.min_silence_ms,
            min_speech_ms: p.min_speech_ms,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct DetectResult {
    pub cutlist: CutList,
}

fn resource_dir(app: &AppHandle) -> Option<PathBuf> {
    app.path().resource_dir().ok()
}

#[tauri::command]
pub async fn compute_waveform(
    app: AppHandle,
    path: String,
    target_bins: usize,
) -> Result<Vec<f32>, String> {
    let ffmpeg = ffmpeg_path(resource_dir(&app).as_deref()).map_err(fmt_err)?;
    let video = PathBuf::from(&path);
    tauri::async_runtime::spawn_blocking(move || extract_waveform(&ffmpeg, &video, target_bins))
        .await
        .map_err(fmt_err)?
        .map_err(fmt_err)
}

#[derive(Debug, Serialize)]
pub struct DiagnosticInfo {
    pub app_version: &'static str,
    pub target_os: &'static str,
    pub target_arch: &'static str,
    pub ffmpeg_path: Option<String>,
    pub ffmpeg_exists: bool,
    pub ffprobe_path: Option<String>,
    pub ffprobe_exists: bool,
}

/// Snapshot of build/runtime info we ask the user to paste into bug reports
/// when something fails. No PII; just versions and paths.
#[tauri::command]
pub fn diagnostic_info(app: AppHandle) -> DiagnosticInfo {
    let rd = resource_dir(&app);
    let ffmpeg = ffmpeg_path(rd.as_deref()).ok();
    let ffprobe = ffprobe_path(rd.as_deref()).ok();
    DiagnosticInfo {
        app_version: env!("CARGO_PKG_VERSION"),
        target_os: std::env::consts::OS,
        target_arch: std::env::consts::ARCH,
        ffmpeg_exists: ffmpeg.as_ref().map(|p| p.exists()).unwrap_or(false),
        ffmpeg_path: ffmpeg.map(|p| p.display().to_string()),
        ffprobe_exists: ffprobe.as_ref().map(|p| p.exists()).unwrap_or(false),
        ffprobe_path: ffprobe.map(|p| p.display().to_string()),
    }
}

#[tauri::command]
pub async fn open_video(app: AppHandle, path: String) -> Result<VideoInfo, String> {
    let ffprobe = ffprobe_path(resource_dir(&app).as_deref()).map_err(fmt_err)?;
    let video = PathBuf::from(&path);
    tauri::async_runtime::spawn_blocking(move || probe(&ffprobe, &video))
        .await
        .map_err(fmt_err)?
        .map_err(fmt_err)
}

#[tauri::command]
pub async fn detect_silence(
    app: AppHandle,
    path: String,
    duration: f64,
    params: DetectParams,
) -> Result<DetectResult, String> {
    let ffmpeg = ffmpeg_path(resource_dir(&app).as_deref()).map_err(fmt_err)?;
    let video = PathBuf::from(&path);
    let range = params.preview_range;
    let pad = params.pad;
    let vad_params: VadParams = (&params).into();
    let offset = range.map(|(s, _)| s).unwrap_or(0.0);

    tauri::async_runtime::spawn_blocking(move || -> Result<DetectResult, String> {
        let samples = extract_pcm(&ffmpeg, &video, range).map_err(fmt_err)?;
        let segments = detect_vad(&samples, vad_params, offset).map_err(fmt_err)?;
        let cutlist = CutList::from_speech_segments(&segments, duration, pad);
        Ok(DetectResult { cutlist })
    })
    .await
    .map_err(fmt_err)?
}

#[derive(Debug, Deserialize)]
pub struct ExportMp4Args {
    pub source: String,
    pub output: String,
    pub cutlist: CutList,
}

#[derive(Debug, Clone, Serialize)]
struct ExportProgressEvent {
    pct: f32,
    message: String,
}

#[tauri::command]
pub async fn export_mp4(
    app: AppHandle,
    state: State<'_, AppState>,
    args: ExportMp4Args,
) -> Result<(), String> {
    let ffmpeg = ffmpeg_path(resource_dir(&app).as_deref()).map_err(fmt_err)?;
    let source = PathBuf::from(&args.source);
    let output = PathBuf::from(&args.output);

    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut guard = state.export_cancel.lock().unwrap();
        *guard = Some(cancel.clone());
    }

    let app_for_progress = app.clone();
    let (tx, rx) = std::sync::mpsc::channel::<Result<(), String>>();
    let cancel_for_worker = cancel.clone();
    thread::spawn(move || {
        let on_progress = move |p: export_mp4::ExportProgress| {
            let _ = app_for_progress.emit(
                "export-progress",
                ExportProgressEvent { pct: p.pct, message: p.message },
            );
        };
        let res = export_mp4::export(
            &ffmpeg,
            &source,
            &output,
            &args.cutlist,
            cancel_for_worker,
            on_progress,
        );
        let _ = tx.send(res.map_err(fmt_err));
    });

    let result = tauri::async_runtime::spawn_blocking(move || rx.recv().unwrap_or_else(|e| Err(e.to_string())))
        .await
        .map_err(fmt_err)?;

    {
        let mut guard = state.export_cancel.lock().unwrap();
        *guard = None;
    }
    result
}

#[tauri::command]
pub fn cancel_export(state: State<'_, AppState>) -> Result<(), String> {
    if let Some(flag) = state.export_cancel.lock().unwrap().clone() {
        flag.store(true, Ordering::SeqCst);
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct ExportFcpxmlArgs {
    pub source: String,
    pub output: String,
    pub cutlist: CutList,
    pub fps: f64,
    pub start_timecode: Option<String>,
    pub title: String,
}

/// Reveal a file in the OS file manager. macOS opens Finder with the file
/// selected; Linux opens the parent folder; Windows opens Explorer with
/// the file selected.
#[tauri::command]
pub fn reveal_in_finder(path: String) -> Result<(), String> {
    let p = std::path::PathBuf::from(&path);
    if !p.exists() {
        return Err(format!("file does not exist: {path}"));
    }
    #[cfg(target_os = "macos")]
    let mut cmd = {
        let mut c = std::process::Command::new("open");
        c.args(["-R", &path]);
        c
    };
    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = std::process::Command::new("explorer");
        c.arg(format!("/select,{path}"));
        c
    };
    #[cfg(target_os = "linux")]
    let mut cmd = {
        let parent = p
            .parent()
            .map(|x| x.to_path_buf())
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        let mut c = std::process::Command::new("xdg-open");
        c.arg(parent);
        c
    };
    cmd.spawn().map_err(fmt_err)?;
    Ok(())
}

#[tauri::command]
pub fn export_fcpxml(args: ExportFcpxmlArgs) -> Result<(), String> {
    let asset_path = Path::new(&args.source);
    let xml = render_fcpxml(
        &args.cutlist,
        FcpxmlParams {
            fps: args.fps,
            asset_path: Some(asset_path),
            start_timecode: args.start_timecode.as_deref(),
            title: &args.title,
        },
    );
    std::fs::write(&args.output, xml).map_err(fmt_err)?;
    Ok(())
}
