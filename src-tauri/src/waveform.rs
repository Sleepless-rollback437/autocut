//! Compute a downsampled amplitude envelope (waveform) from a video's audio
//! track. Used to render the audio waveform in the timeline UI.
//!
//! Strategy: extract 16kHz mono PCM (same path as VAD), then bin into
//! `target_bins` equal-width slices, taking the peak |sample| per bin.
//! Result is a Vec<f32> in [0, 1] with `target_bins` entries.

use std::path::Path;

use anyhow::Result;

use crate::audio::extract_pcm;

pub fn extract_waveform(
    ffmpeg: &Path,
    video: &Path,
    target_bins: usize,
) -> Result<Vec<f32>> {
    let samples = extract_pcm(ffmpeg, video, None)?;
    if samples.is_empty() || target_bins == 0 {
        return Ok(Vec::new());
    }
    let bins = target_bins.min(samples.len());
    let bin_size = (samples.len() as f64 / bins as f64).max(1.0);
    let mut out = Vec::with_capacity(bins);
    for i in 0..bins {
        let start = (i as f64 * bin_size) as usize;
        let end = (((i + 1) as f64 * bin_size) as usize).min(samples.len());
        if end <= start {
            out.push(0.0);
            continue;
        }
        let peak = samples[start..end]
            .iter()
            .map(|s| s.abs())
            .fold(0.0_f32, f32::max);
        out.push(peak);
    }
    Ok(out)
}
