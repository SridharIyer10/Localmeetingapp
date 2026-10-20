//! Model manifest and digest verification (Chunk 3, pure part).
//!
//! No model may load without a manifest entry whose SHA-256 matches the bytes on
//! disk. There is no user-supplied URL path for embedded models.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelKind {
    Stt,
    Embedding,
    Llm,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelEntry {
    pub name: String,
    pub url: String,
    /// Lowercase hex SHA-256 of the model file.
    pub sha256: String,
    pub bytes: u64,
    pub license: String,
    pub min_ram_mb: u64,
    pub kind: ModelKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelManifest {
    pub version: u32,
    pub models: Vec<ModelEntry>,
}

impl ModelManifest {
    pub fn from_json(json: &str) -> Result<Self, ManifestError> {
        serde_json::from_str(json).map_err(|e| ManifestError::Invalid(e.to_string()))
    }

    pub fn get(&self, name: &str) -> Option<&ModelEntry> {
        self.models.iter().find(|m| m.name == name)
    }
}

#[derive(Debug, Error, PartialEq)]
pub enum ManifestError {
    #[error("invalid manifest: {0}")]
    Invalid(String),
    #[error("model not present in the manifest: {0}")]
    UnknownModel(String),
    #[error("digest mismatch for {name}: expected {expected}, got {actual}")]
    DigestMismatch {
        name: String,
        expected: String,
        actual: String,
    },
    #[error("size mismatch for {name}: expected {expected} bytes, got {actual}")]
    SizeMismatch {
        name: String,
        expected: u64,
        actual: u64,
    },
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Verify model bytes against the manifest. Fails closed: an unknown model, a
/// digest mismatch, or a size mismatch all reject the load.
pub fn verify_model<'a>(
    manifest: &'a ModelManifest,
    name: &str,
    bytes: &[u8],
) -> Result<&'a ModelEntry, ManifestError> {
    let entry = manifest
        .get(name)
        .ok_or_else(|| ManifestError::UnknownModel(name.to_string()))?;

    let actual_size = bytes.len() as u64;
    if actual_size != entry.bytes {
        return Err(ManifestError::SizeMismatch {
            name: name.to_string(),
            expected: entry.bytes,
            actual: actual_size,
        });
    }

    let actual = sha256_hex(bytes);
    if !actual.eq_ignore_ascii_case(&entry.sha256) {
        return Err(ManifestError::DigestMismatch {
            name: name.to_string(),
            expected: entry.sha256.clone(),
            actual,
        });
    }
    Ok(entry)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest_for(bytes: &[u8]) -> ModelManifest {
        ModelManifest {
            version: 1,
            models: vec![ModelEntry {
                name: "gemm".into(),
                url: "https://example.invalid/m.bin".into(),
                sha256: sha256_hex(bytes),
                bytes: bytes.len() as u64,
                license: "MIT".into(),
                min_ram_mb: 512,
                kind: ModelKind::Stt,
            }],
        }
    }

    #[test]
    fn valid_model_passes() {
        let bytes = b"model-bytes";
        let manifest = manifest_for(bytes);
        assert!(verify_model(&manifest, "gemm", bytes).is_ok());
    }

    #[test]
    fn tampered_byte_is_rejected() {
        let bytes = b"model-bytes";
        let manifest = manifest_for(bytes);
        let mut tampered = bytes.to_vec();
        tampered[0] ^= 0x01;
        assert!(matches!(
            verify_model(&manifest, "gemm", &tampered),
            Err(ManifestError::DigestMismatch { .. })
        ));
    }

    #[test]
    fn unknown_model_is_refused() {
        let bytes = b"model-bytes";
        let manifest = manifest_for(bytes);
        assert_eq!(
            verify_model(&manifest, "not-listed", bytes).unwrap_err(),
            ManifestError::UnknownModel("not-listed".into())
        );
    }

    #[test]
    fn size_mismatch_is_rejected() {
        let bytes = b"model-bytes";
        let manifest = manifest_for(bytes);
        assert!(matches!(
            verify_model(&manifest, "gemm", b"short"),
            Err(ManifestError::SizeMismatch { .. })
        ));
    }

    #[test]
    fn parses_manifest_json() {
        let json = r#"{"version":1,"models":[{"name":"m","url":"u","sha256":"00","bytes":1,"license":"MIT","min_ram_mb":1,"kind":"llm"}]}"#;
        let m = ModelManifest::from_json(json).unwrap();
        assert_eq!(m.get("m").unwrap().kind, ModelKind::Llm);
    }
}
