//! SMPTE timecode parsing and FCPXML rational rendering.
//!
//! Ported from the Python video-edit project (services/timecode.py). DaVinci
//! Resolve and Premiere bind FCPXML clips to the source media via the embedded
//! source timecode AND the file path; if the asset says it starts at 0s but the
//! MP4 carries `15:33:27;24`, the relink dialog fires. We need accurate parsing.
//!
//! Drop-frame is honored for NTSC 30 (29.97) and 60 (59.94): drop 2 or 4 frames
//! at every minute boundary except every tenth minute.
//!
//! `seconds_to_rational` keeps the canonical denominator (e.g. 60000 for 59.94,
//! 24000 for 23.976) un-reduced; NLEs key on these well-known values.

/// One frame, expressed as (numerator, denominator) seconds.
#[derive(Debug, Clone, Copy)]
pub struct TimecodeRate {
    pub nominal_fps: u32,
    pub is_drop_frame: bool,
    pub frame_duration_num: u64,
    pub frame_duration_den: u64,
}

impl TimecodeRate {
    pub fn frame_seconds(&self) -> f64 {
        self.frame_duration_num as f64 / self.frame_duration_den as f64
    }
}

const NTSC_RATES: &[(u32, u64, u64)] = &[
    (24, 1001, 24000),  // 23.976
    (30, 1001, 30000),  // 29.97
    (60, 1001, 60000),  // 59.94
];

pub fn detect_rate(fps: f64, drop_frame: bool) -> TimecodeRate {
    let nominal = fps.round() as i64;
    if nominal > 0 {
        if let Some(&(n, num, den)) = NTSC_RATES.iter().find(|(n, _, _)| *n as i64 == nominal) {
            let ntsc = n as f64 * 1000.0 / 1001.0;
            if (fps - ntsc).abs() < 0.01 {
                return TimecodeRate {
                    nominal_fps: n,
                    is_drop_frame: drop_frame,
                    frame_duration_num: num,
                    frame_duration_den: den,
                };
            }
        }
    }
    if nominal > 0 && (fps - nominal as f64).abs() < 0.001 {
        return TimecodeRate {
            nominal_fps: nominal as u32,
            is_drop_frame: false,
            frame_duration_num: 1,
            frame_duration_den: nominal as u64,
        };
    }
    // Fallback: approximate with a 120000-denominator rational. Rare path.
    let den = 120_000u64;
    let num = (den as f64 / fps).round() as u64;
    TimecodeRate {
        nominal_fps: nominal.max(1) as u32,
        is_drop_frame: drop_frame,
        frame_duration_num: num,
        frame_duration_den: den,
    }
}

/// Parse `HH:MM:SS:FF` (non-drop) or `HH:MM:SS;FF` (drop) into source-relative
/// seconds. Returns 0.0 for None/empty/unparseable input. Drop-frame only
/// honored for NTSC 30/60.
pub fn parse_smpte(tc: Option<&str>, fps: f64) -> f64 {
    let Some(tc) = tc.map(str::trim).filter(|s| !s.is_empty()) else {
        return 0.0;
    };

    // Manual parse: HH:MM:SS<sep>FF where sep is ':' or ';'.
    let bytes = tc.as_bytes();
    let mut parts: [u32; 4] = [0; 4];
    let mut sep_drop = false;
    let mut field = 0usize;
    let mut acc: u32 = 0;
    let mut have_digit = false;
    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'0'..=b'9' => {
                acc = acc.saturating_mul(10) + (b - b'0') as u32;
                have_digit = true;
            }
            b':' | b';' => {
                if !have_digit || field >= 3 {
                    return 0.0;
                }
                parts[field] = acc;
                if field == 2 && b == b';' {
                    sep_drop = true;
                }
                field += 1;
                acc = 0;
                have_digit = false;
            }
            _ => return 0.0,
        }
        if i + 1 == bytes.len() {
            if !have_digit || field != 3 {
                return 0.0;
            }
            parts[field] = acc;
        }
    }

    let h = parts[0];
    let m = parts[1];
    let s = parts[2];
    let f = parts[3];

    let rate = detect_rate(fps, sep_drop);
    let nominal = rate.nominal_fps;
    if nominal == 0 {
        return 0.0;
    }
    let display_frames =
        (h as u64) * 3600 * nominal as u64 + (m as u64) * 60 * nominal as u64 + (s as u64) * nominal as u64 + f as u64;
    let continuous = if rate.is_drop_frame && (nominal == 30 || nominal == 60) {
        let drops_per_min: u64 = if nominal == 30 { 2 } else { 4 };
        let total_minutes = (h as u64) * 60 + m as u64;
        let dropped = drops_per_min * (total_minutes - total_minutes / 10);
        display_frames.saturating_sub(dropped)
    } else {
        display_frames
    };

    (continuous as f64) * rate.frame_seconds()
}

/// Render `seconds` snapped to the nearest frame as a FCPXML rational string.
/// Preserves the canonical denominator unreduced.
pub fn seconds_to_rational(seconds: f64, rate: &TimecodeRate) -> String {
    if seconds <= 0.0 {
        return "0s".to_string();
    }
    let frames = (seconds / rate.frame_seconds()).round() as u64;
    let numerator = frames * rate.frame_duration_num;
    format!("{}/{}s", numerator, rate.frame_duration_den)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_rate_ntsc_2997() {
        let r = detect_rate(29.97, true);
        assert_eq!(r.nominal_fps, 30);
        assert!(r.is_drop_frame);
        assert_eq!(r.frame_duration_num, 1001);
        assert_eq!(r.frame_duration_den, 30000);
    }

    #[test]
    fn detect_rate_integer_25() {
        let r = detect_rate(25.0, false);
        assert_eq!(r.nominal_fps, 25);
        assert!(!r.is_drop_frame);
        assert_eq!(r.frame_duration_den, 25);
    }

    #[test]
    fn parse_non_drop_25() {
        // 00:00:01:00 at 25 fps -> exactly 1.0s
        let s = parse_smpte(Some("00:00:01:00"), 25.0);
        assert!((s - 1.0).abs() < 1e-9, "got {s}");
    }

    #[test]
    fn parse_drop_frame_2997_one_hour() {
        // 01:00:00;00 at 29.97 drop -> 3600.0s (the drop-frame design intent)
        let s = parse_smpte(Some("01:00:00;00"), 30.0 * 1000.0 / 1001.0);
        assert!((s - 3600.0).abs() < 0.01, "got {s}");
    }

    #[test]
    fn parse_empty_returns_zero() {
        assert_eq!(parse_smpte(None, 30.0), 0.0);
        assert_eq!(parse_smpte(Some(""), 30.0), 0.0);
        assert_eq!(parse_smpte(Some("garbage"), 30.0), 0.0);
    }

    #[test]
    fn seconds_to_rational_2997() {
        let r = detect_rate(29.97, true);
        // 1 second at 29.97 NTSC is 30 frames * 1001/30000 = 30030/30000 s.
        let s = seconds_to_rational(1.0, &r);
        assert_eq!(s, "30030/30000s");
    }

    #[test]
    fn seconds_to_rational_zero() {
        let r = detect_rate(30.0, false);
        assert_eq!(seconds_to_rational(0.0, &r), "0s");
    }
}
