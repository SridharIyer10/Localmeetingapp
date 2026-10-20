//! Transcript windowing for retrieval.
//!
//! ~3-4 turn windows with 1-turn overlap. Windows keep a stable id derived from
//! the sequence range so evidence citations can map back to segments.

use crate::model::TranscriptTurn;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Window {
    /// Stable identifier for this window.
    pub id: String,
    /// Underlying transcript segment sequence numbers, in order.
    pub segment_seqs: Vec<i64>,
    /// Concatenated, speaker-labelled text.
    pub text: String,
}

/// Build overlapping windows over transcript turns.
///
/// `turns_per_window` is clamped to at least 1. `overlap` is clamped to
/// `0..turns_per_window`. Turns are assumed to already be ordered by `seq`.
pub fn window_transcript(
    turns: &[TranscriptTurn],
    turns_per_window: usize,
    overlap: usize,
) -> Vec<Window> {
    if turns.is_empty() {
        return Vec::new();
    }
    let turns_per_window = turns_per_window.max(1);
    let overlap = overlap.min(turns_per_window.saturating_sub(1));
    let step = turns_per_window - overlap;

    let mut windows = Vec::new();
    let mut start = 0usize;
    while start < turns.len() {
        let end = (start + turns_per_window).min(turns.len());
        let slice = &turns[start..end];
        let segment_seqs: Vec<i64> = slice.iter().map(|t| t.seq).collect();
        let text = slice
            .iter()
            .map(|t| {
                let speaker = t.speaker_label.clone().unwrap_or_else(|| {
                    if t.channel == 0 {
                        "User".to_string()
                    } else {
                        "Participant".to_string()
                    }
                });
                format!("{}: {}", speaker, t.text.trim())
            })
            .collect::<Vec<_>>()
            .join("\n");

        windows.push(Window {
            id: format!(
                "w{}-{}",
                segment_seqs.first().copied().unwrap_or(0),
                segment_seqs.last().copied().unwrap_or(0)
            ),
            segment_seqs,
            text,
        });

        if end == turns.len() {
            break;
        }
        start += step;
    }
    windows
}

#[cfg(test)]
mod tests {
    use super::*;

    fn turn(seq: i64, text: &str) -> TranscriptTurn {
        TranscriptTurn {
            seq,
            start_ms: seq * 1000,
            end_ms: seq * 1000 + 900,
            channel: 0,
            speaker_label: None,
            text: text.to_string(),
        }
    }

    #[test]
    fn empty_transcript_yields_no_windows() {
        assert!(window_transcript(&[], 3, 1).is_empty());
    }

    #[test]
    fn windows_overlap_by_one_turn() {
        let turns: Vec<TranscriptTurn> = (0..7).map(|i| turn(i, &format!("t{i}"))).collect();
        let windows = window_transcript(&turns, 3, 1);
        assert_eq!(windows.len(), 3);
        assert_eq!(windows[0].segment_seqs, vec![0, 1, 2]);
        assert_eq!(windows[1].segment_seqs, vec![2, 3, 4]);
        assert_eq!(windows[2].segment_seqs, vec![4, 5, 6]);
    }

    #[test]
    fn no_lost_turn_for_short_transcript() {
        let turns = vec![turn(0, "only")];
        let windows = window_transcript(&turns, 3, 1);
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].segment_seqs, vec![0]);
    }

    #[test]
    fn overlap_is_clamped_below_window_size() {
        let turns: Vec<TranscriptTurn> = (0..5).map(|i| turn(i, "x")).collect();
        let windows = window_transcript(&turns, 2, 9);
        // overlap clamped to 1, step 1
        assert_eq!(windows[0].segment_seqs, vec![0, 1]);
        assert_eq!(windows[1].segment_seqs, vec![1, 2]);
    }
}
