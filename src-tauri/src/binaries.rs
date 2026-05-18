//! Resolve ffmpeg/ffprobe sidecar paths at runtime.
//!
//! Tauri renames sidecars during bundling so the production layout is just
//! `<resource_dir>/ffmpeg[.exe]` (no triple suffix). In dev mode the cargo
//! manifest dir wins (binaries/ffmpeg-<triple>).
//!
//! `binary_path` tries dev paths first (cheap stat), then resource-dir paths.
//! If neither exists the user almost certainly hit a packaging bug; we return
//! Err so the calling command surfaces a clear error.
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};

#[derive(Clone, Copy)]
enum Tool {
    Ffmpeg,
    Ffprobe,
}

impl Tool {
    fn name(self) -> &'static str {
        match self {
            Tool::Ffmpeg => "ffmpeg",
            Tool::Ffprobe => "ffprobe",
        }
    }
}

const TARGET_TRIPLE: &str = current_target_triple();

const fn current_target_triple() -> &'static str {
    // Match tauri's externalBin suffixing. Keep this in sync with build.rs.
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "aarch64-apple-darwin"
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        "x86_64-apple-darwin"
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        "x86_64-unknown-linux-gnu"
    } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
        "aarch64-unknown-linux-gnu"
    } else if cfg!(all(target_os = "windows", target_arch = "x86_64", target_env = "msvc")) {
        "x86_64-pc-windows-msvc"
    } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        "x86_64-pc-windows-gnu"
    } else {
        ""
    }
}

fn ext() -> &'static str {
    if cfg!(target_os = "windows") { ".exe" } else { "" }
}

fn binary_path(tool: Tool, resource_dir: Option<&Path>) -> Result<PathBuf> {
    let leaf = tool.name();
    let suffix = ext();
    let triple = TARGET_TRIPLE;

    // Candidates, tried in priority order. The most important one in
    // production is the directory holding the running executable: on macOS
    // Tauri places sidecar binaries next to the main app binary in
    // Contents/MacOS, NOT in Contents/Resources, so app.path().resource_dir()
    // alone misses them. We also fall back to the build-time dev path
    // (manifest_dir/binaries) so cargo-run / cargo-test work without bundling.
    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            candidates.push(parent.join(format!("{leaf}{suffix}")));
            candidates.push(parent.join(format!("{leaf}-{triple}{suffix}")));
        }
    }
    if let Some(rd) = resource_dir {
        candidates.push(rd.join(format!("{leaf}{suffix}")));
        candidates.push(rd.join(format!("{leaf}-{triple}{suffix}")));
    }
    candidates.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("binaries")
            .join(format!("{leaf}-{triple}{suffix}")),
    );

    for c in &candidates {
        if c.exists() {
            return Ok(c.clone());
        }
    }

    let attempted = candidates
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    Err(anyhow!(
        "could not locate {leaf} binary. tried: {attempted}"
    ))
}

pub fn ffmpeg_path(resource_dir: Option<&Path>) -> Result<PathBuf> {
    binary_path(Tool::Ffmpeg, resource_dir).context("resolving ffmpeg")
}

pub fn ffprobe_path(resource_dir: Option<&Path>) -> Result<PathBuf> {
    binary_path(Tool::Ffprobe, resource_dir).context("resolving ffprobe")
}
