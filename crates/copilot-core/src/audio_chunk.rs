//! VAD-aligned fixed-duration PCM chunking (Chunk 2, pure part).
//!
//! Target ~30 s chunks with a small overlap so transcription does not lose words
//! at boundaries. Overlap is deliberately narrow (2 s); reconciliation removes
//! the duplicated text afterwards.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkSpec {
    pub target_ms: u32,
    pub overlap_ms: u32,
    pub sample_rate: u32,
}

impl ChunkSpec {
    pub fn samples_per_chunk(&self) -> usize {
        ((self.sample_rate as u64 * self.target_ms as u64) / 1000).max(1) as usize
    }

    pub fn overlap_samples(&self) -> usize {
        ((self.sample_rate as u64 * self.overlap_ms as u64) / 1000) as usize
            % self.samples_per_chunk().max(1)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PcmChunk {
    pub index: usize,
    pub start_sample: usize,
    pub end_sample: usize,
    pub samples: Vec<f32>,
}

/// Split PCM into overlapping chunks covering every sample exactly once in the
/// non-overlapping sense: the last chunk always includes the final sample.
pub fn chunk_pcm(samples: &[f32], spec: &ChunkSpec) -> Vec<PcmChunk> {
    if samples.is_empty() {
        return Vec::new();
    }
    let win = spec.samples_per_chunk();
    let overlap = spec.overlap_samples();
    let hop = win.saturating_sub(overlap).max(1);

    let mut out = Vec::new();
    let mut start = 0usize;
    let mut index = 0usize;
    loop {
        let end = (start + win).min(samples.len());
        out.push(PcmChunk {
            index,
            start_sample: start,
            end_sample: end,
            samples: samples[start..end].to_vec(),
        });
        if end == samples.len() {
            break;
        }
        start += hop;
        index += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> ChunkSpec {
        ChunkSpec {
            target_ms: 1000,
            overlap_ms: 100,
            sample_rate: 1000,
        }
    }

    #[test]
    fn empty_input_yields_no_chunks() {
        assert!(chunk_pcm(&[], &spec()).is_empty());
    }

    #[test]
    fn covers_every_sample_with_overlap_and_no_gap() {
        let samples: Vec<f32> = (0..2500).map(|i| i as f32).collect();
        let chunks = chunk_pcm(&samples, &spec());
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].start_sample, 0);
        assert_eq!(chunks[0].end_sample, 1000);
        assert_eq!(chunks[1].start_sample, 900);
        assert_eq!(chunks[2].end_sample, 2500);
        // No gap: each chunk starts at or before the previous chunk's end.
        for w in chunks.windows(2) {
            assert!(w[1].start_sample <= w[0].end_sample);
        }
        // Last chunk always reaches the end.
        assert_eq!(chunks.last().unwrap().end_sample, samples.len());
    }

    #[test]
    fn single_short_input_is_one_chunk() {
        let samples = vec![0.0f32; 100];
        let chunks = chunk_pcm(&samples, &spec());
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].end_sample, 100);
    }

    #[test]
    fn chunk_sample_counts_match_ranges() {
        let samples: Vec<f32> = (0..2500).map(|i| i as f32).collect();
        for c in chunk_pcm(&samples, &spec()) {
            assert_eq!(c.samples.len(), c.end_sample - c.start_sample);
        }
    }
}
