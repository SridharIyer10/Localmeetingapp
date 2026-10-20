//! Overlap reconciliation across chunk boundaries (Chunk 4, pure part).
//!
//! Chunks carry ~2 s of overlap, so the same words are transcribed twice. Naive
//! concatenation duplicates text at every boundary; instead we merge on the
//! longest suffix/prefix word overlap.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Segment {
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
}

/// Merge `a` and `b`, dropping the duplicated overlap.
pub fn merge_texts(a: &str, b: &str) -> String {
    let aw: Vec<&str> = a.split_whitespace().collect();
    let bw: Vec<&str> = b.split_whitespace().collect();
    if aw.is_empty() {
        return bw.join(" ");
    }
    if bw.is_empty() {
        return aw.join(" ");
    }
    let max_k = aw.len().min(bw.len());
    for k in (1..=max_k).rev() {
        if aw[aw.len() - k..] == bw[..k] {
            let mut out = aw.join(" ");
            if bw.len() > k {
                out.push(' ');
                out.push_str(&bw[k..].join(" "));
            }
            return out;
        }
    }
    format!("{a} {b}")
}

/// Sort by start time and merge overlapping segments, removing duplicated words.
pub fn reconcile(segments: &[Segment]) -> Vec<Segment> {
    let mut sorted = segments.to_vec();
    sorted.sort_by_key(|s| (s.start_ms, s.end_ms));

    let mut out: Vec<Segment> = Vec::new();
    for seg in sorted {
        match out.last_mut() {
            Some(cur) if seg.start_ms <= cur.end_ms => {
                cur.text = merge_texts(&cur.text, &seg.text);
                cur.end_ms = cur.end_ms.max(seg.end_ms);
            }
            _ => out.push(seg),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(start: i64, end: i64, text: &str) -> Segment {
        Segment {
            start_ms: start,
            end_ms: end,
            text: text.to_string(),
        }
    }

    #[test]
    fn merges_word_overlap_once() {
        let merged = merge_texts("the base is 50k", "50k and starts in Q3");
        assert_eq!(merged, "the base is 50k and starts in Q3");
    }

    #[test]
    fn merges_longest_overlap() {
        let merged = merge_texts("we will start you in Q3", "you in Q3 pending approval");
        assert_eq!(merged, "we will start you in Q3 pending approval");
    }

    #[test]
    fn unrelated_texts_are_space_joined() {
        assert_eq!(
            merge_texts("alpha beta", "gamma delta"),
            "alpha beta gamma delta"
        );
    }

    #[test]
    fn reconcile_dedupes_boundary_text() {
        let segments = vec![
            seg(0, 1000, "the base is 50k"),
            seg(900, 1900, "50k and starts in Q3"),
            seg(1900, 2900, "in Q3 pending approval"),
        ];
        let out = reconcile(&segments);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].start_ms, 0);
        assert_eq!(out[0].end_ms, 2900);
        assert_eq!(
            out[0].text,
            "the base is 50k and starts in Q3 pending approval"
        );
        assert_eq!(out[0].text.matches("50k").count(), 1usize);
    }

    #[test]
    fn reconcile_keeps_non_overlapping_segments_separate() {
        let segments = vec![seg(0, 1000, "hello"), seg(2000, 3000, "world")];
        assert_eq!(reconcile(&segments).len(), 2);
    }

    #[test]
    fn reconcile_sorts_unordered_input() {
        let segments = vec![seg(2000, 3000, "world"), seg(0, 1000, "hello")];
        let out = reconcile(&segments);
        assert_eq!(out[0].text, "hello");
        assert_eq!(out[1].text, "world");
    }
}
