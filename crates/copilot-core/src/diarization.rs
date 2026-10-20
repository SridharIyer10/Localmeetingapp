//! Offline speaker assignment (Chunk 4, pure part).
//!
//! Channel is a strong prior: channel 0 is always the user. Within the loopback
//! stream (channel 1) we cluster embeddings so multiple remote participants are
//! distinguished instead of collapsing into one "Participant". This is a greedy
//! cosine-threshold clusterer; no vector database.

use crate::retrieval::cosine;

pub const USER_LABEL: &str = "User";

/// Assign a speaker label per segment.
pub fn assign_speakers(channels: &[i32], embeddings: &[Vec<f32>], threshold: f32) -> Vec<String> {
    let n = channels.len().min(embeddings.len());
    let mut labels = Vec::with_capacity(n);

    let mut centroids: Vec<Vec<f32>> = Vec::new();
    let mut counts: Vec<f32> = Vec::new();

    for i in 0..n {
        if channels[i] == 0 {
            labels.push(USER_LABEL.to_string());
            continue;
        }
        let emb = &embeddings[i];
        let mut best: Option<(usize, f32)> = None;
        for (c, centroid) in centroids.iter().enumerate() {
            let sim = cosine(emb, centroid);
            if best.map_or(true, |(_, s)| sim > s) {
                best = Some((c, sim));
            }
        }
        match best {
            Some((c, sim)) if sim >= threshold => {
                // Running average update of the centroid.
                let count = counts[c];
                let centroid = &mut centroids[c];
                for (k, v) in emb.iter().enumerate() {
                    centroid[k] = (centroid[k] * count + v) / (count + 1.0);
                }
                counts[c] += 1.0;
                labels.push(format!("Speaker {}", c + 1));
            }
            _ => {
                centroids.push(emb.clone());
                counts.push(1.0);
                labels.push(format!("Speaker {}", centroids.len()));
            }
        }
    }
    labels
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_zero_is_always_the_user() {
        let channels = vec![0, 0, 1];
        let embeddings = vec![vec![9.0, 9.0], vec![8.0, 1.0], vec![1.0, 0.0]];
        let labels = assign_speakers(&channels, &embeddings, 0.8);
        assert_eq!(labels[0], USER_LABEL);
        assert_eq!(labels[1], USER_LABEL);
    }

    #[test]
    fn identical_loopback_embeddings_share_a_speaker() {
        let channels = vec![1, 1];
        let embeddings = vec![vec![1.0, 0.0], vec![1.0, 0.0]];
        let labels = assign_speakers(&channels, &embeddings, 0.8);
        assert_eq!(labels[0], labels[1]);
        assert_eq!(labels[0], "Speaker 1");
    }

    #[test]
    fn distinct_loopback_embeddings_get_distinct_speakers() {
        let channels = vec![1, 1, 1];
        let embeddings = vec![vec![1.0, 0.0], vec![1.0, 0.0], vec![0.0, 1.0]];
        let labels = assign_speakers(&channels, &embeddings, 0.8);
        assert_eq!(labels[0], "Speaker 1");
        assert_eq!(labels[1], "Speaker 1");
        assert_eq!(labels[2], "Speaker 2");
    }

    #[test]
    fn mixed_channels() {
        let channels = vec![0, 1, 0, 1];
        let embeddings = vec![
            vec![1.0, 1.0],
            vec![1.0, 0.0],
            vec![2.0, 2.0],
            vec![0.0, 1.0],
        ];
        let labels = assign_speakers(&channels, &embeddings, 0.8);
        assert_eq!(labels, vec!["User", "Speaker 1", "User", "Speaker 2"]);
    }
}
