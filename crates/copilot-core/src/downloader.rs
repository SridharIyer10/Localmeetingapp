//! Chunked download state machine (Chunk 3, pure part).
//!
//! Tracks progress, cancellation and resume for the model downloader. Digest
//! verification is a separate step (`model_manifest::verify_model`); a download
//! only reaches `Completed` after verification succeeds.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum DownloadState {
    Idle,
    Downloading { received: u64, total: u64 },
    Paused { received: u64, total: u64 },
    Verifying { received: u64, total: u64 },
    Completed,
    Failed { message: String },
}

pub struct Downloader {
    total: u64,
    received: u64,
    state: DownloadState,
}

impl Downloader {
    pub fn new(total: u64) -> Self {
        Self {
            total,
            received: 0,
            state: DownloadState::Idle,
        }
    }

    /// Begin (or resume) downloading. `already_downloaded` is the number of bytes
    /// already present on disk from a previous attempt.
    pub fn start(&mut self, already_downloaded: u64) {
        self.received = already_downloaded.min(self.total);
        self.state = if self.received >= self.total && self.total > 0 {
            DownloadState::Verifying {
                received: self.received,
                total: self.total,
            }
        } else {
            DownloadState::Downloading {
                received: self.received,
                total: self.total,
            }
        };
    }

    pub fn on_chunk(&mut self, len: u64) {
        if !matches!(self.state, DownloadState::Downloading { .. }) {
            return;
        }
        self.received = (self.received + len).min(self.total);
        self.state = if self.received >= self.total && self.total > 0 {
            DownloadState::Verifying {
                received: self.received,
                total: self.total,
            }
        } else {
            DownloadState::Downloading {
                received: self.received,
                total: self.total,
            }
        };
    }

    /// Pause; the received byte count is retained for resume.
    pub fn pause(&mut self) {
        if matches!(self.state, DownloadState::Downloading { .. }) {
            self.state = DownloadState::Paused {
                received: self.received,
                total: self.total,
            };
        }
    }

    pub fn resume(&mut self) {
        if matches!(self.state, DownloadState::Paused { .. }) {
            self.state = DownloadState::Downloading {
                received: self.received,
                total: self.total,
            };
        }
    }

    pub fn fail(&mut self, message: impl Into<String>) {
        self.state = DownloadState::Failed {
            message: message.into(),
        };
    }

    /// Called after digest verification. `ok = false` fails the download.
    pub fn finish_verification(&mut self, ok: bool) {
        if matches!(
            self.state,
            DownloadState::Verifying { .. } | DownloadState::Downloading { .. }
        ) {
            if ok {
                self.state = DownloadState::Completed;
            } else {
                self.state = DownloadState::Failed {
                    message: "digest mismatch".to_string(),
                };
            }
        }
    }

    pub fn state(&self) -> &DownloadState {
        &self.state
    }

    pub fn received(&self) -> u64 {
        self.received
    }

    pub fn total(&self) -> u64 {
        self.total
    }

    pub fn is_complete(&self) -> bool {
        matches!(self.state, DownloadState::Completed)
    }

    /// Byte offset a resume should use.
    pub fn resume_from(&self) -> u64 {
        self.received
    }

    pub fn progress(&self) -> f32 {
        if self.total == 0 {
            1.0
        } else {
            self.received as f32 / self.total as f32
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downloads_to_verifying_then_completed() {
        let mut d = Downloader::new(1000);
        d.start(0);
        d.on_chunk(400);
        d.on_chunk(600);
        assert!(matches!(
            d.state(),
            DownloadState::Verifying { received: 1000, .. }
        ));
        assert_eq!(d.progress(), 1.0);
        d.finish_verification(true);
        assert!(d.is_complete());
    }

    #[test]
    fn pause_retains_progress_and_ignores_chunks() {
        let mut d = Downloader::new(1000);
        d.start(0);
        d.on_chunk(250);
        d.pause();
        d.on_chunk(250); // ignored while paused
        assert_eq!(d.received(), 250);
        d.resume();
        d.on_chunk(250);
        assert_eq!(d.received(), 500);
    }

    #[test]
    fn resume_from_existing_bytes() {
        let mut d = Downloader::new(1000);
        d.start(700);
        assert_eq!(d.received(), 700);
        assert_eq!(d.resume_from(), 700);
        d.on_chunk(300);
        assert!(matches!(d.state(), DownloadState::Verifying { .. }));
    }

    #[test]
    fn overlong_chunk_is_capped_at_total() {
        let mut d = Downloader::new(1000);
        d.start(0);
        d.on_chunk(5000);
        assert_eq!(d.received(), 1000);
    }

    #[test]
    fn digest_failure_marks_failed() {
        let mut d = Downloader::new(100);
        d.start(0);
        d.on_chunk(100);
        d.finish_verification(false);
        assert!(matches!(d.state(), DownloadState::Failed { .. }));
        assert!(!d.is_complete());
    }

    #[test]
    fn failure_is_terminal_for_this_attempt() {
        let mut d = Downloader::new(100);
        d.start(0);
        d.fail("network reset");
        d.on_chunk(50);
        assert_eq!(d.received(), 0);
        assert!(matches!(d.state(), DownloadState::Failed { .. }));
    }
}
