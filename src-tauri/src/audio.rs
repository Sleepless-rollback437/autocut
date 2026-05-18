//! Extract 16kHz mono f32 PCM samples from a video via ffmpeg subprocess.
//!
//! Pipes raw audio through stdout into memory; no intermediate file. silero-vad
//! consumes f32 in [-1.0, 1.0] at 16kHz, so we ask ffmpeg for s16le and convert.

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{anyhow, Context, Result};

pub const VAD_SAMPLE_RATE: u32 = 16_000;

pub fn extract_pcm(
    ffmpeg: &Path,
    video: &Path,
    range: Option<(f64, f64)>,
) -> Result<Vec<f32>> {
    let mut cmd = Command::new(ffmpeg);
    if let Some((start, end)) = range {
        // -ss before -i for fast seek. The window may not be frame-accurate
        // but VAD doesn't need it; we only care about audio bulk.
        cmd.args(["-ss", &format!("{:.3}", start.max(0.0))]);
        cmd.args(["-to", &format!("{:.3}", end.max(start))]);
    }
    cmd.arg("-i").arg(video);
    cmd.args([
        "-vn",
        "-ac",
        "1",
        "-ar",
        &VAD_SAMPLE_RATE.to_string(),
        "-f",
        "s16le",
        "-loglevel",
        "error",
        "-",
    ]);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .with_context(|| format!("spawning ffmpeg for {}", video.display()))?;
    let mut stdout = child.stdout.take().ok_or_else(|| anyhow!("no stdout"))?;
    let mut bytes = Vec::new();
    stdout.read_to_end(&mut bytes).context("reading ffmpeg stdout")?;

    let status = child.wait().context("waiting for ffmpeg")?;
    if !status.success() {
        let mut err = String::new();
        if let Some(mut stderr) = child.stderr.take() {
            let _ = stderr.read_to_string(&mut err);
        }
        return Err(anyhow!("ffmpeg failed ({status}): {err}"));
    }

    let samples = s16le_to_f32(&bytes);
    Ok(samples)
}

fn s16le_to_f32(bytes: &[u8]) -> Vec<f32> {
    let scale = 1.0_f32 / 32768.0;
    bytes
        .chunks_exact(2)
        .map(|chunk| {
            let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
            (sample as f32) * scale
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s16_conversion_bounds() {
        let bytes = [0x00, 0x80, 0xff, 0x7f, 0x00, 0x00];
        let samples = s16le_to_f32(&bytes);
        assert_eq!(samples.len(), 3);
        assert!((samples[0] - (-1.0)).abs() < 1e-4);
        assert!((samples[1] - (32767.0 / 32768.0)).abs() < 1e-4);
        assert_eq!(samples[2], 0.0);
    }
}
