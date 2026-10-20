//! Intake Co-Pilot app side: Tauri commands, the post-meeting analysis driven by the
//! app's summary model, the live status pass, and the OS secret store. The analysis
//! primitives live in the `copilot-core` crate.

pub mod analysis;
pub mod app_llm;
pub mod commands;
pub mod embedding;
pub mod secret_store;

pub use secret_store::{KeyringSecretStore, MemorySecretStore, SecretStore};
