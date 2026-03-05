use crate::paths::uri::{ParsedURI, ResolvedSource};
use async_trait::async_trait;
use std::path::{Path, PathBuf};

/// Maximum allowed response body size for HTTP downloads (100 MB).
pub const MAX_DOWNLOAD_BYTES: u64 = 100 * 1024 * 1024;

/// Default timeout for HTTP GET downloads (120 seconds).
pub const HTTP_DOWNLOAD_TIMEOUT_SECS: u64 = 120;

/// Safely join a base path with a user-supplied subpath, preventing directory traversal.
///
/// Returns an error if the resolved path escapes the base directory (e.g., via `../`).
/// Handles both existing and non-existing paths by normalizing components.
pub fn safe_join(base: &Path, subpath: &str) -> crate::error::Result<PathBuf> {
    if subpath.is_empty() {
        return Ok(base.to_path_buf());
    }

    let joined = base.join(subpath);

    // Try canonicalize for existing paths (resolves symlinks + ../)
    let resolved = if joined.exists() {
        joined
            .canonicalize()
            .unwrap_or_else(|_| normalize_components(&joined))
    } else {
        normalize_components(&joined)
    };

    let base_resolved = if base.exists() {
        base.canonicalize()
            .unwrap_or_else(|_| normalize_components(base))
    } else {
        normalize_components(base)
    };

    if !resolved.starts_with(&base_resolved) {
        return Err(crate::error::BundleError::LoadError {
            reason: format!(
                "Subpath '{}' escapes base directory '{}'",
                subpath,
                base.display()
            ),
            source: None,
        });
    }

    Ok(joined)
}

/// Normalize path components without touching the filesystem.
///
/// Resolves `.` and `..` segments purely lexically.
fn normalize_components(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                result.pop();
            }
            std::path::Component::CurDir => {}
            other => result.push(other),
        }
    }
    result
}

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
///
/// Matches Python's `SourceHandlerProtocol` in `sources/protocol.py`.
/// Implementations: [`FileSourceHandler`](file::FileSourceHandler),
/// [`GitSourceHandler`](git::GitSourceHandler),
/// [`HttpSourceHandler`](http::HttpSourceHandler),
/// [`ZipSourceHandler`](zip::ZipSourceHandler).
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

/// Extended trait for source handlers that support update status checking.
///
/// Ports Python's `SourceHandlerWithStatusProtocol` from `sources/protocol.py`.
/// Adds non-destructive status checking ([`get_status`](Self::get_status)) and
/// forced re-download ([`update`](Self::update)) on top of the base
/// [`SourceHandler`] trait.
///
/// **Implementations:**
/// - [`GitSourceHandler`](git::GitSourceHandler): uses `git ls-remote` for status
///   checking and cache removal + re-clone for updates.
/// - [`HttpSourceHandler`](http::HttpSourceHandler): uses HEAD with conditional
///   `If-None-Match`/`If-Modified-Since` headers for status checking and cache
///   removal + re-download for updates.
///
/// The `check_bundle_status()` and `update_bundle()` functions in the `updates`
/// module dispatch to this trait for git and HTTP URIs. File handlers do not
/// implement this trait (local files are always current).
///
/// **Return type divergence:** Python's `update()` returns `Path`; Rust returns
/// `Result<ResolvedSource>`. This matches the Rust `SourceHandler::resolve()`
/// return type for internal consistency — callers get both `active_path` and
/// `source_root` from a single call.
#[async_trait]
pub trait SourceHandlerWithStatus: SourceHandler {
    /// Check update status without side effects.
    ///
    /// For git sources: uses `ls-remote` to compare cached vs remote HEAD.
    /// For HTTP sources: uses `HEAD` + `ETag`/`Last-Modified` headers.
    /// For file sources: checks mtime.
    async fn get_status(
        &self,
        parsed: &ParsedURI,
        cache_dir: &Path,
    ) -> crate::error::Result<SourceStatus>;

    /// Force re-download, ignoring cache.
    ///
    /// Returns [`ResolvedSource`] after fresh download (not `PathBuf` as in
    /// Python's protocol — Rust returns the richer type for consistency with
    /// [`SourceHandler::resolve`]).
    async fn update(
        &self,
        parsed: &ParsedURI,
        cache_dir: &Path,
    ) -> crate::error::Result<ResolvedSource>;
}

/// Trait for resolving source URIs to local paths.
///
/// Ports Python's `SourceResolverProtocol` from `sources/protocol.py`.
/// Higher-level than [`SourceHandler`]: takes a raw URI string instead of
/// a pre-parsed [`ParsedURI`], and dispatches to the appropriate handler
/// internally.
///
/// **Forward-declared protocol:** This trait formalizes the resolver contract.
/// The reference implementation is [`SimpleSourceResolver`](resolver::SimpleSourceResolver).
#[async_trait]
pub trait SourceResolver: Send + Sync {
    /// Resolve a URI to local paths.
    ///
    /// Returns [`ResolvedSource`] with `active_path` and `source_root`.
    /// Returns [`BundleError::NotFound`](crate::error::BundleError::NotFound)
    /// if the URI cannot be resolved by any handler.
    async fn resolve(&self, uri: &str) -> crate::error::Result<ResolvedSource>;
}
