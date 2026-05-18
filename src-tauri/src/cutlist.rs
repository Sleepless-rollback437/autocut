use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CutKind {
    Keep,
    Remove,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cut {
    pub start: f64,
    pub end: f64,
    pub kind: CutKind,
}

impl Cut {
    pub fn duration(&self) -> f64 {
        (self.end - self.start).max(0.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CutList {
    pub source_duration: f64,
    pub intervals: Vec<Cut>,
}

#[derive(Debug, Clone, Copy)]
pub struct SpeechSegment {
    pub start: f64,
    pub end: f64,
}

impl CutList {
    /// Build a CutList from silero-style speech segments. Each speech segment is
    /// padded by `pad` seconds on both sides, clamped to [0, source_duration],
    /// merged when overlapping, and gaps become `Remove` intervals.
    pub fn from_speech_segments(
        segments: &[SpeechSegment],
        source_duration: f64,
        pad: f64,
    ) -> Self {
        let mut keeps: Vec<(f64, f64)> = segments
            .iter()
            .map(|s| {
                (
                    (s.start - pad).max(0.0),
                    (s.end + pad).min(source_duration),
                )
            })
            .filter(|(s, e)| e > s)
            .collect();
        keeps.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

        let mut merged: Vec<(f64, f64)> = Vec::with_capacity(keeps.len());
        for (s, e) in keeps {
            if let Some(last) = merged.last_mut() {
                if s <= last.1 {
                    last.1 = last.1.max(e);
                    continue;
                }
            }
            merged.push((s, e));
        }

        let mut intervals = Vec::with_capacity(merged.len() * 2 + 1);
        let mut cursor = 0.0_f64;
        for (s, e) in &merged {
            if *s > cursor {
                intervals.push(Cut {
                    start: cursor,
                    end: *s,
                    kind: CutKind::Remove,
                });
            }
            intervals.push(Cut {
                start: *s,
                end: *e,
                kind: CutKind::Keep,
            });
            cursor = *e;
        }
        if cursor < source_duration {
            intervals.push(Cut {
                start: cursor,
                end: source_duration,
                kind: CutKind::Remove,
            });
        }

        CutList {
            source_duration,
            intervals,
        }
    }

    pub fn kept_intervals(&self) -> impl Iterator<Item = &Cut> {
        self.intervals.iter().filter(|c| c.kind == CutKind::Keep)
    }

    pub fn total_kept_duration(&self) -> f64 {
        self.kept_intervals().map(Cut::duration).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_speech_segment_padded_and_clamped() {
        let segs = [SpeechSegment { start: 1.0, end: 2.0 }];
        let cl = CutList::from_speech_segments(&segs, 5.0, 0.3);
        let kept: Vec<_> = cl.kept_intervals().collect();
        assert_eq!(kept.len(), 1);
        assert!((kept[0].start - 0.7).abs() < 1e-9);
        assert!((kept[0].end - 2.3).abs() < 1e-9);
        assert!((cl.total_kept_duration() - 1.6).abs() < 1e-9);
    }

    #[test]
    fn overlapping_pads_merge() {
        let segs = [
            SpeechSegment { start: 1.0, end: 2.0 },
            SpeechSegment { start: 2.4, end: 3.0 },
        ];
        let cl = CutList::from_speech_segments(&segs, 5.0, 0.3);
        let kept: Vec<_> = cl.kept_intervals().collect();
        assert_eq!(kept.len(), 1, "overlapping pads should merge");
        assert!((kept[0].start - 0.7).abs() < 1e-9);
        assert!((kept[0].end - 3.3).abs() < 1e-9);
    }

    #[test]
    fn empty_segments_produce_single_remove() {
        let cl = CutList::from_speech_segments(&[], 4.0, 0.3);
        assert_eq!(cl.intervals.len(), 1);
        assert_eq!(cl.intervals[0].kind, CutKind::Remove);
        assert_eq!(cl.total_kept_duration(), 0.0);
    }

    #[test]
    fn pad_clamps_at_zero_and_duration() {
        let segs = [SpeechSegment { start: 0.1, end: 4.9 }];
        let cl = CutList::from_speech_segments(&segs, 5.0, 0.3);
        let kept: Vec<_> = cl.kept_intervals().collect();
        assert_eq!(kept[0].start, 0.0);
        assert_eq!(kept[0].end, 5.0);
    }
}
