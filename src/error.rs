use std::fmt;

/// Validation result carrying errors and warnings.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl fmt::Display for ValidationResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} errors, {} warnings",
            self.errors.len(),
            self.warnings.len()
        )
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BundleError {
    #[error("bundle not found: {uri}")]
    NotFound { uri: String },

    #[error("failed to load bundle: {reason}")]
    LoadError {
        reason: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("validation failed: {0}")]
    ValidationError(ValidationResult),

    #[error("dependency error: {0}")]
    DependencyError(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Yaml(#[from] serde_yaml_ng::Error),

    #[error("http error: {0}")]
    Http(String),

    #[error("git error: {0}")]
    Git(String),
}

pub type Result<T> = std::result::Result<T, BundleError>;
