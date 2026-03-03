use crate::paths::uri::{ParsedURI, ResolvedSource};
use async_trait::async_trait;
use std::path::Path;

pub mod file;
pub mod git;
pub mod http;
pub mod resolver;
pub mod zip;

/// Status of a bundle source (for update checking).
///
/// Matches Python's `SourceStatus` dataclass in `sources/protocol.py`.
/// Fields are a superset of the Python version: the original Rust fields
/// (`current_version`, `latest_version`) are preserved for backward compatibility
/// as Rust-only additions, and all Python fields are included.
///
/// **Field naming:** Python uses `source_uri`; Rust uses `uri` for consistency
/// with `BundleState.uri` and other Rust types. Add `#[serde(rename = "source_uri")]`
/// if cross-language serialization is needed.
///
/// `has_update` is `Some(true)` if an update is available, `Some(false)` if
/// up to date, or `None` if the status could not be determined.
///
/// # Defaults
///
/// All new fields have sensible defaults via `Default`:
/// - `is_cached`: `false`
/// - `summary`: `""`
/// - All `Option<T>` fields: `None`
///
/// Existing construction sites can use `..Default::default()` to fill new fields.
/// Prefer [`SourceStatus::new(uri)`] for new code to avoid empty-URI construction.
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct SourceStatus {
    /// Source URI (e.g., `"git+https://github.com/org/repo@main"`).
    /// Called `source_uri` in Python; shortened for Rust API consistency.
    pub uri: String,
    /// Currently cached version string, if known.
    /// **Rust-only field** (not in Python's `SourceStatus`). Use `cached_commit`
    /// for the Python-equivalent cached commit SHA.
    pub current_version: Option<String>,
    /// Latest known remote version string, if known.
    /// **Rust-only field** (not in Python's `SourceStatus`). Use `remote_commit`
    /// for the Python-equivalent remote commit SHA.
    pub latest_version: Option<String>,
    /// Whether an update is available: `Some(true)` = yes, `Some(false)` = no,
    /// `None` = unknown/unsupported.
    pub has_update: Option<bool>,
    /// Whether the source has been cached locally.
    pub is_cached: bool,
    /// Timestamp of when the source was cached.
    /// Stored as a string (typically ISO 8601) for serialization simplicity.
    /// Python uses `datetime | None`; String avoids forcing a specific chrono
    /// dependency on consumers.
    pub cached_at: Option<String>,
    /// Cached ref name (e.g., `"main"`, `"v1.0.0"`).
    pub cached_ref: Option<String>,
    /// Cached commit SHA (full 40-char hex).
    pub cached_commit: Option<String>,
    /// Remote ref name from status check.
    pub remote_ref: Option<String>,
    /// Remote commit SHA from status check.
    pub remote_commit: Option<String>,
    /// Error message if status check failed.
    pub error: Option<String>,
    /// Human-readable summary of the status.
    pub summary: String,
}

impl SourceStatus {
    /// Create a new `SourceStatus` with the given URI and default fields.
    ///
    /// Prefer this over `Default::default()` to ensure the URI is always set.
    pub fn new(uri: impl Into<String>) -> Self {
        Self {
            uri: uri.into(),
            ..Default::default()
        }
    }

    /// Check if the cached ref is pinned (a specific commit SHA or version tag).
    ///
    /// Returns `true` if `cached_ref` is:
    /// - A 40-character hex string (commit SHA, case-insensitive), or
    /// - A string starting with `"v"` that contains at least one digit (version tag).
    ///
    /// Returns `false` if `cached_ref` is `None`, empty, or doesn't match either pattern.
    ///
    /// Matches Python's `SourceStatus.is_pinned` property, which normalizes
    /// to lowercase before checking hex characters.
    pub fn is_pinned(&self) -> bool {
        let cached_ref = match &self.cached_ref {
            Some(r) if !r.is_empty() => r,
            _ => return false,
        };
        // Check if it's a 40-char hex commit SHA (case-insensitive, matching Python's .lower())
        if cached_ref.len() == 40 && cached_ref.chars().all(|c| c.is_ascii_hexdigit()) {
            return true;
        }
        // Check if it's a version tag (starts with 'v' and contains a digit)
        cached_ref.starts_with('v') && cached_ref.chars().any(|c| c.is_ascii_digit())
    }
}

/// Trait for source handlers that resolve URIs to local paths.
#[async_trait]
pub trait SourceHandler: Send + Sync {
    /// Check if this handler can handle the given parsed URI.
    fn can_handle(&self, parsed: &ParsedURI) -> bool;

    /// Resolve the URI to a local path, using cache_dir for caching.
    async fn resolve(
        &self,
        parsed: &ParsedURI,
        cache_dir: &Path,
    ) -> crate::error::Result<ResolvedSource>;
}
