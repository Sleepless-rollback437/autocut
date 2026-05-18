//! Cut MP4 export: feed the kept intervals as ffmpeg `filter_complex select`
//! expressions. Output is re-encoded (libx264 + aac), mirroring the POC.
//! Lossless smart-cut is deferred.
//!
//! Progress is parsed from ffmpeg stderr `out_time_us=` lines (emitted via
//! `-progress pipe:2`). Total target duration is the sum of kept intervals
//! since the output stream's wall-clock is shorter than the source.

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use anyhow::{anyhow, Context, Result};

use crate::cutlist::CutList;

pub struct ExportProgress {
    pub pct: f32,
    pub message: String,
}

pub fn export(
    ffmpeg: &Path,
    source: &Path,
    output: &Path,
    cutlist: &CutList,
    cancel: Arc<AtomicBool>,
    on_progress: impl FnMut(ExportProgress) + Send + 'static,
) -> Result<()> {
    let kept_total: f64 = cutlist.total_kept_duration();
    if kept_total <= 0.0 {
        return Err(anyhow!("nothing to keep; cutlist has zero kept duration"));
    }

    let select = build_select_expr(cutlist);
    let filter_complex = format!(
        "[0:v]select='{sel}',setpts=N/FRAME_RATE/TB[v];[0:a]aselect='{sel}',asetpts=N/SR/TB[a]",
        sel = select
    );

    let mut cmd = Command::new(ffmpeg);
    cmd.args(["-y", "-hide_banner", "-nostats", "-progress", "pipe:2", "-loglevel", "error"])
        .arg("-i")
        .arg(source)
        .args(["-filter_complex", &filter_complex])
        .args(["-map", "[v]", "-map", "[a]"])
        .args(["-c:v", "libx264", "-preset", "veryfast", "-crf", "20"])
        .args(["-c:a", "aac", "-b:a", "192k"])
        .arg(output);
    cmd.stdout(Stdio::null()).stderr(Stdio::piped());

    let mut child = cmd.spawn().context("spawning ffmpeg for export")?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow!("ffmpeg stderr unavailable"))?;

    let on_progress = std::sync::Mutex::new(on_progress);
    let cancel_thread = cancel.clone();
    let reader = thread::spawn(move || -> Result<()> {
        let buf = BufReader::new(stderr);
        for line in buf.lines() {
            let line = line.context("reading ffmpeg stderr")?;
            if cancel_thread.load(Ordering::SeqCst) {
                break;
            }
            if let Some(us) = line.strip_prefix("out_time_us=") {
                if let Ok(us) = us.trim().parse::<i64>() {
                    let seconds = us.max(0) as f64 / 1_000_000.0;
                    let pct = ((seconds / kept_total) * 100.0).clamp(0.0, 99.0);
                    let msg = format!("encoded {:.2}s of {:.2}s kept", seconds, kept_total);
                    let mut cb = on_progress.lock().unwrap();
                    cb(ExportProgress { pct: pct as f32, message: msg });
                }
            }
        }
        Ok(())
    });

    // Cancellation: poll the flag and kill the child if set.
    loop {
        if cancel.load(Ordering::SeqCst) {
            let _ = child.kill();
            let _ = child.wait();
            let _ = reader.join();
            return Err(anyhow!("export cancelled"));
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                let _ = reader.join();
                if !status.success() {
                    return Err(anyhow!("ffmpeg exited with {status}"));
                }
                break;
            }
            Ok(None) => thread::sleep(std::time::Duration::from_millis(100)),
            Err(e) => return Err(anyhow!("waiting on ffmpeg: {e}")),
        }
    }
    Ok(())
}

fn build_select_expr(cutlist: &CutList) -> String {
    let parts: Vec<String> = cutlist
        .kept_intervals()
        .map(|c| format!("between(t,{:.6},{:.6})", c.start, c.end))
        .collect();
    if parts.is_empty() {
        "0".to_string()
    } else {
        parts.join("+")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cutlist::{Cut, CutKind};

    #[test]
    fn select_expr_joins_kept_intervals() {
        let cl = CutList {
            source_duration: 10.0,
            intervals: vec![
                Cut { start: 0.0, end: 1.0, kind: CutKind::Keep },
                Cut { start: 1.0, end: 2.0, kind: CutKind::Remove },
                Cut { start: 2.0, end: 3.5, kind: CutKind::Keep },
            ],
        };
        let s = build_select_expr(&cl);
        assert!(s.contains("between(t,0.000000,1.000000)"));
        assert!(s.contains("between(t,2.000000,3.500000)"));
        assert!(s.contains("+"));
    }

    #[test]
    fn select_expr_empty_when_no_keeps() {
        let cl = CutList { source_duration: 5.0, intervals: vec![] };
        assert_eq!(build_select_expr(&cl), "0");
    }
}
