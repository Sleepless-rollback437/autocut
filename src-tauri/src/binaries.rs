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
pub enum Tool {
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

pub fn binary_path(tool: Tool, resource_dir: Option<&Path>) -> Result<PathBuf> {
    let leaf = tool.name();
    let suffix = ext();
    let triple = TARGET_TRIPLE;

    // 1. Dev: manifest_dir/binaries/<tool>-<triple><ext>
    if let Some(dev) = dev_candidate(leaf, triple, suffix) {
        if dev.exists() {
            return Ok(dev);
        }
    }
    // 2. Production: <resource_dir>/<tool><ext> (Tauri strips the suffix at bundle time)
    if let Some(rd) = resource_dir {
        let stripped = rd.join(format!("{leaf}{suffix}"));
        if stripped.exists() {
            return Ok(stripped);
        }
        // 3. Production fallback: tauri sometimes keeps the suffix
        let suffixed = rd.join(format!("{leaf}-{triple}{suffix}"));
        if suffixed.exists() {
            return Ok(suffixed);
        }
    }

    Err(anyhow!(
        "could not locate {leaf} binary (looked in dev manifest_dir/binaries and resource_dir)"
    ))
}

fn dev_candidate(leaf: &str, triple: &str, suffix: &str) -> Option<PathBuf> {
    // CARGO_MANIFEST_DIR is set at compile time. env! makes the binary tied to
    // the build location, which is exactly what we want for dev resolution.
    let manifest = env!("CARGO_MANIFEST_DIR");
    let candidate = PathBuf::from(manifest)
        .join("binaries")
        .join(format!("{leaf}-{triple}{suffix}"));
    Some(candidate)
}

pub fn ffmpeg_path(resource_dir: Option<&Path>) -> Result<PathBuf> {
    binary_path(Tool::Ffmpeg, resource_dir).context("resolving ffmpeg")
}

pub fn ffprobe_path(resource_dir: Option<&Path>) -> Result<PathBuf> {
    binary_path(Tool::Ffprobe, resource_dir).context("resolving ffprobe")
}
