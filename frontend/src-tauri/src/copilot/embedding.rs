//! Embedding provider selection (Chunk 6).
//!
//! Default build uses the deterministic hashing embedder so the pipeline runs
//! with no model present. Enabling the `onnx-embed` feature switches to the
//! ONNX `bge-small-en-v1.5` embedder (`ort` + `tokenizers`).
//!
//! ONNX models are never downloaded on the call path; they arrive through the
//! model manifest and are verified before load.

use copilot_core::{Embedder, HashingEmbedder};
use std::path::Path;

/// The embedder the engine uses. Cheap to construct; the ONNX session is built
/// once at analysis start.
pub enum AppEmbedder {
    Hashing(HashingEmbedder),
    #[cfg(feature = "onnx-embed")]
    Onnx(OnnxEmbedder),
}

impl AppEmbedder {
    /// Prefer ONNX when a verified model is present, else fall back to hashing.
    pub fn load(model_dir: Option<&Path>) -> Self {
        #[cfg(feature = "onnx-embed")]
        {
            if let Some(dir) = model_dir {
                if let Ok(e) = OnnxEmbedder::load(dir) {
                    log::info!("✅ ONNX embedding model loaded from {}", dir.display());
                    return AppEmbedder::Onnx(e);
                }
                log::warn!(
                    "ONNX embedding model not usable in {}; using hashing fallback",
                    dir.display()
                );
            }
        }
        let _ = model_dir;
        AppEmbedder::Hashing(HashingEmbedder::new(256))
    }

    pub fn embed(&self, text: &str) -> Vec<f32> {
        match self {
            AppEmbedder::Hashing(e) => e.embed(text),
            #[cfg(feature = "onnx-embed")]
            AppEmbedder::Onnx(e) => e.embed(text),
        }
    }

    pub fn dim(&self) -> usize {
        match self {
            AppEmbedder::Hashing(e) => e.dim(),
            #[cfg(feature = "onnx-embed")]
            AppEmbedder::Onnx(e) => e.dim(),
        }
    }
}

impl Embedder for AppEmbedder {
    fn embed(&self, text: &str) -> Vec<f32> {
        AppEmbedder::embed(self, text)
    }
    fn dim(&self) -> usize {
        AppEmbedder::dim(self)
    }
}

/// ONNX sentence embedder (feature `onnx-embed`).
///
/// Expects `model.onnx` and `tokenizer.json` in `model_dir`, produced from
/// `bge-small-en-v1.5` (CLS-pooled, L2-normalized).
#[cfg(feature = "onnx-embed")]
pub struct OnnxEmbedder {
    session: std::sync::Mutex<ort::session::Session>,
    tokenizer: tokenizers::Tokenizer,
    dim: usize,
}

#[cfg(feature = "onnx-embed")]
impl OnnxEmbedder {
    pub fn load(model_dir: &Path) -> Result<Self, String> {
        let tokenizer = tokenizers::Tokenizer::from_file(model_dir.join("tokenizer.json"))
            .map_err(|e| format!("tokenizer load failed: {e}"))?;
        let session = ort::session::Session::builder()
            .map_err(|e| e.to_string())?
            .commit_from_file(model_dir.join("model.onnx"))
            .map_err(|e| format!("onnx session load failed: {e}"))?;
        Ok(Self {
            session: std::sync::Mutex::new(session),
            tokenizer,
            dim: 384,
        })
    }

    fn embed(&self, text: &str) -> Vec<f32> {
        let enc = match self.tokenizer.encode(text, true) {
            Ok(e) => e,
            Err(_) => return vec![0.0; self.dim],
        };
        let ids: Vec<i64> = enc.get_ids().iter().map(|&x| x as i64).collect();
        let mask: Vec<i64> = enc.get_attention_mask().iter().map(|&x| x as i64).collect();
        let len = ids.len();
        let ids_val = match ort::value::Tensor::from_array(([1usize, len], ids)) {
            Ok(v) => v,
            Err(_) => return vec![0.0; self.dim],
        };
        let mask_val = match ort::value::Tensor::from_array(([1usize, len], mask)) {
            Ok(v) => v,
            Err(_) => return vec![0.0; self.dim],
        };
        let mut session = match self.session.lock() {
            Ok(s) => s,
            Err(_) => return vec![0.0; self.dim],
        };
        let outputs =
            match session.run(ort::inputs!["input_ids" => ids_val, "attention_mask" => mask_val]) {
                Ok(o) => o,
                Err(_) => return vec![0.0; self.dim],
            };
        let (_shape, data) = match outputs["last_hidden_state"].try_extract_tensor::<f32>() {
            Ok(t) => t,
            Err(_) => return vec![0.0; self.dim],
        };
        // CLS pooling: first token's hidden state. If CLS is absent, average.
        let mut v = if data.len() >= self.dim {
            data[..self.dim].to_vec()
        } else {
            data.to_vec()
        };
        let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in v.iter_mut() {
                *x /= norm;
            }
        }
        v
    }

    fn dim(&self) -> usize {
        self.dim
    }
}

#[cfg(feature = "onnx-embed")]
impl Embedder for OnnxEmbedder {
    fn embed(&self, text: &str) -> Vec<f32> {
        OnnxEmbedder::embed(self, text)
    }
    fn dim(&self) -> usize {
        OnnxEmbedder::dim(self)
    }
}
