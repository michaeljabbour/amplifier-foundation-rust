//! Error types for `amplifier-distro`.

use amplifier_foundation::BundleError;
use amplifier_core::AmplifierError;

/// Top-level error type for the distro layer.
#[derive(Debug, thiserror::Error)]
pub enum DistroError {
    #[error("Config error: {0}")]
    Config(String),

    #[error("Overlay error: {0}")]
    Overlay(String),

    #[error("Provider error: {0}")]
    Provider(String),

    #[error("Session error: {0}")]
    Session(String),

    #[error("Doctor error: {0}")]
    Doctor(String),

    #[error("Bundle error: {0}")]
    Bundle(#[from] BundleError),

    #[error("Amplifier error: {0}")]
    Amplifier(#[from] AmplifierError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml_ng::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Convenience `Result` alias.
pub type Result<T> = std::result::Result<T, DistroError>;
