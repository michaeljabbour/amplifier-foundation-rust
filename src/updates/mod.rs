use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde_yaml_ng::Value;

use crate::bundle::Bundle;
use crate::paths::uri::{get_amplifier_home, parse_uri};
use crate::sources::git::GitSourceHandler;
use crate::sources::{SourceHandler, SourceHandlerWithStatus, SourceStatus};

/// Status of a bundle and all its sources.
///
/// Provides aggregate information about update availability across
/// all sources in a bundle (modules, included bundles, etc.).
///
/// Matches Python's `BundleStatus` dataclass with derived properties
/// for filtering sources by update status.
///
/// **Note:** The Rust API diverges from Python's: `check_bundle_status`
/// takes a URI string (not a `Bundle`), checking a single source.
/// Python's version walks the entire bundle component tree. This
/// simplification was an intentional design choice from the stub API.
#[derive(Debug, Clone, PartialEq)]
pub struct BundleStatus {
    /// Name of the bundle.
    pub bundle_name: String,
    /// Source URI of the bundle itself, if loaded from remote.
    pub bundle_source: Option<String>,
    /// Status of each source in the bundle.
    pub sources: Vec<SourceStatus>,
}

impl BundleStatus {
    /// Check if any source has an update available.
    pub fn has_updates(&self) -> bool {
        self.sources.iter().any(|s| s.has_update == Some(true))
    }

    /// Get list of sources that have updates available.
    pub fn updateable_sources(&self) -> Vec<&SourceStatus> {
        self.sources
            .iter()
            .filter(|s| s.has_update == Some(true))
            .collect()
    }

    /// Get list of sources that are up to date.
    pub fn up_to_date_sources(&self) -> Vec<&SourceStatus> {
        self.sources
            .iter()
            .filter(|s| s.has_update == Some(false))
            .collect()
    }

    /// Get list of sources with unknown update status.
    pub fn unknown_sources(&self) -> Vec<&SourceStatus> {
        self.sources
            .iter()
            .filter(|s| s.has_update.is_none())
            .collect()
    }

    /// Human-readable summary of bundle status.
    pub fn summary(&self) -> String {
        let total = self.sources.len();
        let mut updates = 0;
        let mut up_to_date = 0;
        let mut unknown = 0;

        for s in &self.sources {
            match s.has_update {
                Some(true) => updates += 1,
                Some(false) => up_to_date += 1,
                None => unknown += 1,
            }
        }

        if updates > 0 {
            format!("{updates} update(s) available ({up_to_date} up to date, {unknown} unknown)")
        } else if unknown > 0 {
            format!("Up to date ({unknown} source(s) could not be checked)")
        } else {
            format!("All {total} source(s) up to date")
        }
    }
}

/// Default cache directory for source handlers.
///
/// Uses the same path as [`SimpleSourceResolver::new`](crate::sources::resolver::SimpleSourceResolver::new):
/// `~/.amplifier/cache/bundles`. This ensures that status checks and updates
/// operate on the same cache directory as the resolver.
fn default_cache_dir() -> PathBuf {
    get_amplifier_home().join("cache").join("bundles")
}

/// Collect all source URIs from a bundle.
///
/// Extracts sources from:
/// - Bundle's own source (`source_uri`, if loaded from remote)
/// - Session orchestrator and context (`session.orchestrator.source`, `session.context.source`)
/// - Providers, tools, hooks (each item's `source` field)
///
/// Returns a deduplicated list of unique source URIs. The order is not
/// guaranteed (uses `HashSet` internally for deduplication).
///
/// Matches Python's `_collect_source_uris(bundle)` from `updates/__init__.py`.
///
/// # Examples
///
/// ```
/// use amplifier_foundation::Bundle;
/// use amplifier_foundation::updates::collect_source_uris;
///
/// let mut bundle = Bundle::new("my-bundle");
/// bundle.source_uri = Some("git+https://github.com/org/bundle@main".to_string());
///
/// let uris = collect_source_uris(&bundle);
/// assert_eq!(uris.len(), 1);
/// ```
pub fn collect_source_uris(bundle: &Bundle) -> Vec<String> {
    let mut sources: HashSet<String> = HashSet::new();
    let source_key = Value::String("source".to_string());

    // Helper: insert non-empty strings only.
    // Python's `if source_uri:` treats "" as falsy; match that behavior.
    let mut insert = |s: &str| {
        if !s.is_empty() {
            sources.insert(s.to_string());
        }
    };

    // Bundle's own source URI
    if let Some(uri) = bundle.source_uri.as_deref() {
        insert(uri);
    }

    // Session config: orchestrator and context
    // Python: `isinstance(session.get("orchestrator"), dict) and "source" in session["orchestrator"]`
    if let Some(session_map) = bundle.session.as_mapping() {
        for key in &["orchestrator", "context"] {
            if let Some(entry) = session_map.get(Value::String(key.to_string())) {
                if let Some(entry_map) = entry.as_mapping() {
                    if let Some(source_val) = entry_map.get(&source_key) {
                        if let Some(s) = source_val.as_str() {
                            insert(s);
                        }
                    }
                }
            }
        }
    }

    // Module lists: providers, tools, hooks
    // Note: includes are deliberately excluded — they are checked independently
    // as first-class bundles. Matches Python comment in _collect_source_uris.
    for module_list in [&bundle.providers, &bundle.tools, &bundle.hooks] {
        for module_entry in module_list {
            if let Some(mod_map) = module_entry.as_mapping() {
                if let Some(source_val) = mod_map.get(&source_key) {
                    if let Some(s) = source_val.as_str() {
                        insert(s);
                    }
                }
            }
        }
    }

    sources.into_iter().collect()
}

/// Check update status for a bundle source URI.
///
/// This is a MECHANISM that has no side effects — it only checks
/// whether updates are available without downloading anything.
///
/// **API Note:** Unlike Python's `check_bundle_status(bundle)` which walks
/// the entire bundle component tree, the Rust version takes a single URI
/// and returns status for that one source. This is intentionally simpler.
///
/// Supported source types:
/// - `file://` and local paths: always reported as up to date
/// - `git+https://`, `git+http://`: dispatched to [`GitSourceHandler::get_status`]
///   which uses `git ls-remote` to check for updates
/// - `https://`, `http://`: reported as unknown (HTTP status checking not yet implemented)
///
/// # Arguments
///
/// * `uri` — Source URI to check.
/// * `cache_dir` — Optional cache directory. Defaults to `~/.amplifier/cache`.
///
/// # Returns
///
/// A [`BundleStatus`] with the status of the URI.
pub async fn check_bundle_status(
    uri: &str,
    cache_dir: Option<&Path>,
) -> crate::error::Result<BundleStatus> {
    let parsed = parse_uri(uri);
    let uri_owned = uri.to_string();
    let cache = cache_dir
        .map(|p| p.to_path_buf())
        .unwrap_or_else(default_cache_dir);

    let source_status = if parsed.is_file() {
        // Local files are always "current"
        SourceStatus {
            uri: uri_owned.clone(),
            has_update: Some(false),
            is_cached: true,
            summary: "Local file (always current)".to_string(),
            ..Default::default()
        }
    } else {
        // Try dispatching to a handler that supports status checking
        let git_handler = GitSourceHandler::new();

        if git_handler.can_handle(&parsed) {
            git_handler.get_status(&parsed, &cache).await?
        } else {
            // HTTP and other remote sources: no status handler yet
            SourceStatus {
                uri: uri_owned.clone(),
                has_update: None,
                summary: "Update checking not supported for this source type".to_string(),
                ..Default::default()
            }
        }
    };

    Ok(BundleStatus {
        bundle_name: uri_owned.clone(),
        bundle_source: Some(uri_owned),
        sources: vec![source_status],
    })
}

/// Update a bundle source by re-downloading from remote.
///
/// This is a MECHANISM that has side effects — it removes cached
/// versions and re-downloads fresh content.
///
/// Supported source types:
/// - `file://` and local paths: no-op (nothing to update for local files)
/// - `git+https://`, `git+http://`: dispatched to [`GitSourceHandler::update`]
///   which removes the cache and re-clones
/// - `https://`, `http://`: not yet implemented (returns error)
///
/// # Arguments
///
/// * `uri` — Source URI to update.
/// * `cache_dir` — Optional cache directory. Defaults to `~/.amplifier/cache`.
///
/// # Errors
///
/// Returns [`BundleError::LoadError`](crate::error::BundleError::LoadError)
/// if the source type does not support updating.
pub async fn update_bundle(uri: &str, cache_dir: Option<&Path>) -> crate::error::Result<()> {
    let parsed = parse_uri(uri);
    let cache = cache_dir
        .map(|p| p.to_path_buf())
        .unwrap_or_else(default_cache_dir);

    if parsed.is_file() {
        // Local files: nothing to update
        return Ok(());
    }

    let git_handler = GitSourceHandler::new();
    if git_handler.can_handle(&parsed) {
        // Delegate to GitSourceHandler::update (nuke and reclone)
        git_handler.update(&parsed, &cache).await?;
        return Ok(());
    }

    // HTTP and other remote sources: not yet implemented
    Err(crate::error::BundleError::LoadError {
        reason: format!(
            "Update not yet implemented for URI scheme '{}': {uri}",
            parsed.scheme
        ),
        source: None,
    })
}
