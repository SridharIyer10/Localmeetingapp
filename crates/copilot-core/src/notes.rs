//! Deduplication for map-reduce notes extraction.

use std::collections::HashSet;

/// Normalize a note for comparison: lowercase, alphanumeric+space, trimmed,
/// collapsed whitespace.
pub fn normalize_note(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Jaccard similarity over normalized word sets.
pub fn jaccard(a: &str, b: &str) -> f32 {
    let na = normalize_note(a);
    let nb = normalize_note(b);
    let sa: HashSet<&str> = na.split_whitespace().collect();
    let sb: HashSet<&str> = nb.split_whitespace().collect();
    if sa.is_empty() && sb.is_empty() {
        return 1.0;
    }
    let inter = sa.intersection(&sb).count() as f32;
    let union = sa.union(&sb).count() as f32;
    if union == 0.0 {
        0.0
    } else {
        inter / union
    }
}

/// Deduplicate notes, preserving first-seen order. A note is dropped when it is
/// near-identical (Jaccard >= `threshold`) to something already kept.
pub fn dedupe(notes: &[String], threshold: f32) -> Vec<String> {
    let mut kept: Vec<String> = Vec::new();
    for note in notes {
        if note.trim().is_empty() {
            continue;
        }
        let duplicate = kept.iter().any(|k| jaccard(k, note) >= threshold);
        if !duplicate {
            kept.push(note.trim().to_string());
        }
    }
    kept
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_duplicates_collapse() {
        let notes = vec![
            "Send offer letter".to_string(),
            "send offer letter".to_string(),
        ];
        assert_eq!(dedupe(&notes, 0.8), vec!["Send offer letter"]);
    }

    #[test]
    fn near_duplicates_collapse_but_distinct_survive() {
        let notes = vec![
            "Schedule the panel interview for next week".to_string(),
            "Schedule panel interview next week".to_string(),
            "Prepare compensation benchmark".to_string(),
        ];
        let out = dedupe(&notes, 0.6);
        assert_eq!(out.len(), 2);
        assert!(out
            .iter()
            .any(|n| n.contains("compensation") || n.contains("Compensation")));
    }

    #[test]
    fn empty_notes_are_dropped() {
        assert!(dedupe(&["".into(), "  ".into()], 0.8).is_empty());
    }

    #[test]
    fn jaccard_bounds() {
        assert!((jaccard("a b", "a b") - 1.0).abs() < 1e-6);
        assert_eq!(jaccard("a", "b"), 0.0);
    }
}
