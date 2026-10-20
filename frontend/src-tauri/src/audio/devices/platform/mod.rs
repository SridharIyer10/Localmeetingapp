// Platform-specific audio device implementations

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "macos")]
pub mod macos;

// Re-export platform-specific functions
#[cfg(target_os = "windows")]
pub use windows::{configure_windows_audio, get_windows_device};

#[cfg(target_os = "macos")]
pub use macos::configure_macos_audio;

