//! Embedding abstraction (Chunk 6, pure part).
//!
//! The real embedder is ONNX `bge-small-en-v1.5` (in the app layer). This module
//! defines the trait the engine depends on and a deterministic hashing embedder
//! used by tests and as a no-model fallback, so the pipeline can run end to end
//! without a downloaded model.

use crate::retrieval::{normalize, tokenize};

pub trait Embedder: Send + Sync {
    fn embed(&self, text: &str) -> Vec<f32>;
    fn dim(&self) -> usize;
}

/// Deterministic signed-hashing bag-of-words embedder.
///
/// Not a semantic model: it exists so retrieval and the full pipeline can run
/// deterministically in tests and on machines with no embedding model present.
pub struct HashingEmbedder {
    dim: usize,
}

impl HashingEmbedder {
    pub fn new(dim: usize) -> Self {
        Self { dim: dim.max(1) }
    }
}

fn fnv1a(s: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for b in s.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

impl Default for HashingEmbedder {
    fn default() -> Self {
        Self::new(256)
    }
}

impl Embedder for HashingEmbedder {
    fn embed(&self, text: &str) -> Vec<f32> {
        let mut v = vec![0.0f32; self.dim];
        for tok in tokenize(text) {
            let h = fnv1a(&tok);
            let idx = (h % self.dim as u64) as usize;
            let sign = if (h >> 33) & 1 == 0 { 1.0 } else { -1.0 };
            v[idx] += sign;
        }
        normalize(&mut v);
        v
    }

    fn dim(&self) -> usize {
        self.dim
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retrieval::cosine;

    #[test]
    fn deterministic_and_normalized() {
        let e = HashingEmbedder::new(64);
        let a = e.embed("the salary is 50k");
        let b = e.embed("the salary is 50k");
        assert_eq!(a, b);
        assert!((cosine(&a, &a) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn similar_text_is_closer_than_unrelated() {
        let e = HashingEmbedder::new(256);
        let q = e.embed("what is the salary and start date");
        let near = e.embed("the base salary is 50k and the start date is Q3");
        let far = e.embed("coffee preferences and lunch plans");
        assert!(cosine(&q, &near) > cosine(&q, &far));
    }

    #[test]
    fn dimension_is_respected() {
        assert_eq!(HashingEmbedder::new(32).embed("x").len(), 32);
    }
}
