use crate::paths::uri::parse_uri;
use crate::sources::SourceStatus;

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

/// Check update status for a bundle source URI.
///
/// This is a MECHANISM that has no side effects — it only checks
/// whether updates are available without downloading anything.
///
/// **API Note:** Unlike Python's `check_bundle_status(bundle)` which walks
/// the entire bundle component tree, the Rust version takes a single URI
/// and returns status for that one source. This is intentionally simpler.
///
/// Currently supported:
/// - `file://` and local paths: always reported as up to date
/// - `git+https://`: reported as unknown (git status checking not yet implemented)
/// - `https://`, `http://`: reported as unknown
///
/// # Arguments
///
/// * `uri` — Source URI to check.
///
/// # Returns
///
/// A [`BundleStatus`] with the status of the URI.
pub async fn check_bundle_status(uri: &str) -> crate::error::Result<BundleStatus> {
    let parsed = parse_uri(uri);
    let uri_owned = uri.to_string();

    let source_status = if parsed.is_file() {
        // Local files are always "current"
        SourceStatus {
            uri: uri_owned.clone(),
            current_version: None,
            latest_version: None,
            has_update: Some(false),
        }
    } else {
        // Git, HTTP, and other remote sources: can't check without
        // implementing the respective handlers' get_status methods
        SourceStatus {
            uri: uri_owned.clone(),
            current_version: None,
            latest_version: None,
            has_update: None,
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
/// Currently supported:
/// - `file://` and local paths: no-op (nothing to update for local files)
/// - `git+https://`: not yet implemented (returns error)
/// - `https://`, `http://`: not yet implemented (returns error)
///
/// # Arguments
///
/// * `uri` — Source URI to update.
///
/// # Errors
///
/// Returns [`BundleError::LoadError`](crate::error::BundleError::LoadError)
/// if the source type does not support updating. A dedicated `UpdateError`
/// variant could be added when git/http update support is implemented.
pub async fn update_bundle(uri: &str) -> crate::error::Result<()> {
    let parsed = parse_uri(uri);

    if parsed.is_file() {
        // Local files: nothing to update
        return Ok(());
    }

    // Git, HTTP, and other remote sources: not yet implemented
    Err(crate::error::BundleError::LoadError {
        reason: format!(
            "Update not yet implemented for URI scheme '{}': {uri}",
            parsed.scheme
        ),
        source: None,
    })
}
