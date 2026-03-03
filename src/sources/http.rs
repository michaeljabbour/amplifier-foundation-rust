use std::path::{Path, PathBuf};

use async_trait::async_trait;
use sha2::{Digest, Sha256};

use super::SourceHandler;
use crate::paths::uri::{ParsedURI, ResolvedSource};

/// Handler for https:// and http:// URIs (direct file downloads).
///
/// Downloads files to cache and returns local path.
/// Uses content-addressable storage (hash of URL).
///
/// Note: For downloading zip archives, use zip+https:// which extracts.
/// This handler downloads files as-is without extraction.
pub struct HttpSourceHandler;

impl Default for HttpSourceHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpSourceHandler {
    pub fn new() -> Self {
        Self
    }

    /// Build the full URL from parsed URI components.
    fn build_url(parsed: &ParsedURI) -> String {
        format!("{}://{}{}", parsed.scheme, parsed.host, parsed.path)
    }

    /// Compute cached file path for a URL.
    ///
    /// Uses SHA-256 hash of the URL (first 16 hex chars) as cache key.
    /// Preserves the original filename for readability.
    fn cached_file_path(url: &str, parsed: &ParsedURI, cache_dir: &Path) -> PathBuf {
        let hash = format!("{:x}", Sha256::digest(url.as_bytes()));
        let cache_key = &hash[..16];

        // Preserve file extension for proper handling
        let filename = Path::new(&parsed.path)
            .file_name()
            .and_then(|n| n.to_str())
            .filter(|n| !n.is_empty())
            .unwrap_or("download");

        cache_dir.join(format!("{filename}-{cache_key}"))
    }

    /// Apply subpath to a cached file/directory path and return ResolvedSource.
    ///
    /// Shared by cache-hit path and post-download path.
    fn resolve_with_subpath(
        cached_file: PathBuf,
        subpath: &str,
    ) -> crate::error::Result<ResolvedSource> {
        if !subpath.is_empty() {
            let sp = cached_file.join(subpath);
            if !sp.exists() {
                return Err(crate::error::BundleError::NotFound {
                    uri: format!("Subpath not found: {subpath}"),
                });
            }
            Ok(ResolvedSource {
                active_path: sp,
                source_root: cached_file,
            })
        } else {
            Ok(ResolvedSource {
                active_path: cached_file.clone(),
                source_root: cached_file,
            })
        }
    }
}

#[async_trait]
impl SourceHandler for HttpSourceHandler {
    fn can_handle(&self, parsed: &ParsedURI) -> bool {
        parsed.is_http()
    }

    /// Resolve HTTP URI to local cached path.
    ///
    /// Downloads the file if not cached. Uses BundleError::NotFound for download
    /// failures to match Python's BundleNotFoundError behavior — callers match
    /// on NotFound to distinguish "bundle unavailable" from other errors.
    async fn resolve(
        &self,
        parsed: &ParsedURI,
        cache_dir: &Path,
    ) -> crate::error::Result<ResolvedSource> {
        let url = Self::build_url(parsed);
        let cached_file = Self::cached_file_path(&url, parsed, cache_dir);

        // Check if already cached
        if cached_file.exists() {
            let resolved = Self::resolve_with_subpath(cached_file.clone(), &parsed.subpath);
            if resolved.is_ok() {
                return resolved;
            }
            // Cache exists but subpath doesn't — fall through to re-download
        }

        // Download the file
        self.download(&url, &cached_file, cache_dir).await?;

        Self::resolve_with_subpath(cached_file, &parsed.subpath)
    }
}

impl HttpSourceHandler {
    /// Download a URL to a local cache file.
    ///
    /// Gated on the `http-sources` feature (requires reqwest).
    async fn download(
        &self,
        url: &str,
        cached_file: &Path,
        cache_dir: &Path,
    ) -> crate::error::Result<()> {
        // Ensure cache directory exists
        std::fs::create_dir_all(cache_dir).map_err(|e| crate::error::BundleError::LoadError {
            reason: format!("Failed to create cache directory: {}", cache_dir.display()),
            source: Some(Box::new(e)),
        })?;

        #[cfg(feature = "http-sources")]
        {
            let response =
                reqwest::get(url)
                    .await
                    .map_err(|e| crate::error::BundleError::NotFound {
                        uri: format!("Failed to download {url}: {e}"),
                    })?;

            if !response.status().is_success() {
                return Err(crate::error::BundleError::NotFound {
                    uri: format!(
                        "Failed to download {url}: HTTP {}",
                        response.status().as_u16()
                    ),
                });
            }

            let content =
                response
                    .bytes()
                    .await
                    .map_err(|e| crate::error::BundleError::NotFound {
                        uri: format!("Failed to read response from {url}: {e}"),
                    })?;

            std::fs::write(cached_file, &content).map_err(|e| {
                crate::error::BundleError::LoadError {
                    reason: format!("Failed to write cached file: {}", cached_file.display()),
                    source: Some(Box::new(e)),
                }
            })?;

            Ok(())
        }

        #[cfg(not(feature = "http-sources"))]
        {
            Err(crate::error::BundleError::LoadError {
                reason: format!("HTTP source handler requires the 'http-sources' feature: {url}"),
                source: None,
            })
        }
    }
}
