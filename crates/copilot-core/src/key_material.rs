//! Audio-key lifecycle (Chunk 8, pure part).
//!
//! Crypto-shredding is the mechanism that makes "the recording is gone" true.
//! Unlinking a file does not erase blocks on SSD/APFS; zeroizing the per-session
//! key renders the ciphertext unrecoverable. This type owns that key and
//! guarantees it is zeroed on destruction.

/// A per-session encryption key held only in memory.
pub struct SessionKey {
    bytes: Vec<u8>,
    destroyed: bool,
}

impl SessionKey {
    /// Wrap raw key bytes generated from the OS CSPRNG.
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self {
            bytes,
            destroyed: false,
        }
    }

    /// Expose the key bytes for a cipher operation. Returns `None` once destroyed.
    pub fn expose(&self) -> Option<&[u8]> {
        if self.destroyed {
            None
        } else {
            Some(&self.bytes)
        }
    }

    pub fn is_destroyed(&self) -> bool {
        self.destroyed
    }

    /// Zeroize and discard the key. Idempotent. After this the ciphertext is
    /// unrecoverable.
    pub fn destroy(&mut self) {
        for b in self.bytes.iter_mut() {
            *b = 0;
        }
        self.bytes.clear();
        self.destroyed = true;
    }

    #[cfg(test)]
    fn raw(&self) -> &[u8] {
        &self.bytes
    }
}

impl Drop for SessionKey {
    fn drop(&mut self) {
        self.destroy();
    }
}

/// The audit record written on every destruction path.
#[derive(Debug, Clone, PartialEq)]
pub struct DisposalAudit {
    pub meeting_id: String,
    pub destroyed_at: String,
    pub duration_ms: i64,
    pub reason: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expose_returns_bytes_before_destroy() {
        let key = SessionKey::from_bytes(vec![1, 2, 3, 4]);
        assert!(!key.is_destroyed());
        assert_eq!(key.expose(), Some(&[1u8, 2, 3, 4][..]));
    }

    #[test]
    fn destroy_zeroizes_and_blocks_exposure() {
        let mut key = SessionKey::from_bytes(vec![9, 9, 9]);
        key.destroy();
        assert!(key.is_destroyed());
        assert!(key.expose().is_none());
        assert!(key.raw().is_empty());
    }

    #[test]
    fn destroy_is_idempotent() {
        let mut key = SessionKey::from_bytes(vec![5, 6]);
        key.destroy();
        key.destroy();
        assert!(key.is_destroyed());
    }
}
