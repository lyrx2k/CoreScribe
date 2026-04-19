pub mod audio;
pub mod inference;

// Re-export commonly used types
pub use inference::{WhisperConfig, WhisperModel, ModelSize};
