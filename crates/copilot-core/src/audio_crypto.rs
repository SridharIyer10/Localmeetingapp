//! Per-session audio encryption (Chunk 2, pure part).
//!
//! XChaCha20-Poly1305 with a 24-byte random nonce prepended to the ciphertext.
//! The key is a per-session key generated from the OS CSPRNG and held only in
//! memory (see `key_material::SessionKey`). Additional authenticated data (AAD)
//! binds each chunk to its meeting id and sequence number.

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use thiserror::Error;

pub const KEY_LEN: usize = 32;
pub const NONCE_LEN: usize = 24;
pub const TAG_LEN: usize = 16;

#[derive(Debug, Error, PartialEq)]
pub enum CryptoError {
    #[error("invalid key length")]
    InvalidKey,
    #[error("ciphertext is too short to contain a nonce")]
    TooShort,
    #[error("authentication failed (wrong key, wrong aad, or tampered data)")]
    DecryptFailed,
}

pub type Key = [u8; KEY_LEN];

/// Generate a 32-byte key from the OS CSPRNG.
pub fn generate_key() -> Key {
    let mut key = [0u8; KEY_LEN];
    key[..16].copy_from_slice(uuid::Uuid::new_v4().as_bytes());
    key[16..].copy_from_slice(uuid::Uuid::new_v4().as_bytes());
    key
}

fn random_nonce() -> [u8; NONCE_LEN] {
    let mut nonce = [0u8; NONCE_LEN];
    nonce[..16].copy_from_slice(uuid::Uuid::new_v4().as_bytes());
    nonce[16..].copy_from_slice(&uuid::Uuid::new_v4().as_bytes()[..8]);
    nonce
}

pub struct AudioCrypto {
    cipher: XChaCha20Poly1305,
}

impl AudioCrypto {
    pub fn new(key: &Key) -> Result<Self, CryptoError> {
        let cipher = XChaCha20Poly1305::new_from_slice(key).map_err(|_| CryptoError::InvalidKey)?;
        Ok(Self { cipher })
    }

    /// Encrypt `plaintext`, returning `nonce || ciphertext+tag`.
    pub fn encrypt(&self, plaintext: &[u8], aad: &[u8]) -> Result<Vec<u8>, CryptoError> {
        let nonce = random_nonce();
        let ct = self
            .cipher
            .encrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: plaintext,
                    aad,
                },
            )
            .map_err(|_| CryptoError::DecryptFailed)?;
        let mut out = Vec::with_capacity(NONCE_LEN + ct.len());
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&ct);
        Ok(out)
    }

    /// Decrypt `nonce || ciphertext+tag`.
    pub fn decrypt(&self, blob: &[u8], aad: &[u8]) -> Result<Vec<u8>, CryptoError> {
        if blob.len() < NONCE_LEN + TAG_LEN {
            return Err(CryptoError::TooShort);
        }
        let (nonce, ct) = blob.split_at(NONCE_LEN);
        self.cipher
            .decrypt(XNonce::from_slice(nonce), Payload { msg: ct, aad })
            .map_err(|_| CryptoError::DecryptFailed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pcm() -> Vec<u8> {
        // A short buffer of "PCM"-like bytes.
        (0..2048u32).map(|i| (i % 251) as u8).collect()
    }

    #[test]
    fn key_is_32_bytes_and_unique() {
        let a = generate_key();
        let b = generate_key();
        assert_eq!(a.len(), KEY_LEN);
        assert_ne!(a, b);
    }

    #[test]
    fn round_trips() {
        let key = generate_key();
        let crypto = AudioCrypto::new(&key).unwrap();
        let plaintext = pcm();
        let blob = crypto.encrypt(&plaintext, b"meeting-1:chunk-0").unwrap();
        let out = crypto.decrypt(&blob, b"meeting-1:chunk-0").unwrap();
        assert_eq!(out, plaintext);
    }

    #[test]
    fn ciphertext_does_not_contain_plaintext_pcm() {
        let key = generate_key();
        let crypto = AudioCrypto::new(&key).unwrap();
        let plaintext = pcm();
        let blob = crypto.encrypt(&plaintext, b"aad").unwrap();
        // The raw PCM run must not appear verbatim in the ciphertext.
        let needle = &plaintext[100..160];
        assert!(
            !blob.windows(needle.len()).any(|w| w == needle),
            "plaintext leaked into ciphertext"
        );
    }

    #[test]
    fn wrong_aad_fails_authentication() {
        let key = generate_key();
        let crypto = AudioCrypto::new(&key).unwrap();
        let blob = crypto.encrypt(&pcm(), b"meeting-1:chunk-0").unwrap();
        assert_eq!(
            crypto.decrypt(&blob, b"meeting-1:chunk-1"),
            Err(CryptoError::DecryptFailed)
        );
    }

    #[test]
    fn tampering_is_detected() {
        let key = generate_key();
        let crypto = AudioCrypto::new(&key).unwrap();
        let mut blob = crypto.encrypt(&pcm(), b"aad").unwrap();
        let last = blob.len() - 1;
        blob[last] ^= 0x01;
        assert_eq!(
            crypto.decrypt(&blob, b"aad"),
            Err(CryptoError::DecryptFailed)
        );
    }

    #[test]
    fn wrong_key_fails() {
        let c1 = AudioCrypto::new(&generate_key()).unwrap();
        let c2 = AudioCrypto::new(&generate_key()).unwrap();
        let blob = c1.encrypt(&pcm(), b"aad").unwrap();
        assert_eq!(c2.decrypt(&blob, b"aad"), Err(CryptoError::DecryptFailed));
    }

    #[test]
    fn nonces_are_unique_so_ciphertexts_differ() {
        let crypto = AudioCrypto::new(&generate_key()).unwrap();
        let a = crypto.encrypt(&pcm(), b"aad").unwrap();
        let b = crypto.encrypt(&pcm(), b"aad").unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn short_blob_is_rejected() {
        let crypto = AudioCrypto::new(&generate_key()).unwrap();
        assert_eq!(
            crypto.decrypt(&[0u8; 10], b"aad"),
            Err(CryptoError::TooShort)
        );
    }
}
