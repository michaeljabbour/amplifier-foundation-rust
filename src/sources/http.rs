use std::path::{Path, PathBuf};

use async_trait::async_trait;
use sha2::{Digest, Sha256};

use super::{SourceHandler, SourceHandlerWithStatus, SourceStatus};
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
    /// Return the metadata sidecar path for a cached file.
    ///
    /// Given `/cache/filename-abc123` returns `/cache/filename-abc123.meta.json`.
    fn meta_path(cached_file: &Path) -> PathBuf {
        let mut s = cached_file.as_os_str().to_owned();
        s.push(".meta.json");
        PathBuf::from(s)
    }

    /// Save HTTP cache metadata as a JSON sidecar file next to the cached file.
    ///
    /// Stores `etag`, `last_modified`, `cached_at`, and `url` for use in
    /// subsequent conditional HEAD requests.
    fn save_cache_metadata(
        cached_file: &Path,
        url: &str,
        etag: Option<&str>,
        last_modified: Option<&str>,
    ) {
        let meta_path = Self::meta_path(cached_file);
        let metadata = serde_json::json!({
            "etag": etag,
            "last_modified": last_modified,
            "cached_at": chrono::Utc::now().to_rfc3339(),
            "url": url,
        });
        match serde_json::to_string_pretty(&metadata) {
            Ok(content) => {
                if let Err(e) = std::fs::write(&meta_path, content) {
                    tracing::warn!(
                        "Failed to write HTTP cache metadata {}: {e}",
                        meta_path.display()
                    );
                }
            }
            Err(e) => {
                tracing::warn!("Failed to serialize HTTP cache metadata: {e}");
            }
        }
    }

    /// Read HTTP cache metadata from the JSON sidecar file.
    ///
    /// Returns a JSON object with keys: `etag`, `last_modified`, `cached_at`, `url`.
    /// Returns `None` if the sidecar file doesn't exist or can't be parsed.
    fn get_cache_metadata(cached_file: &Path) -> Option<serde_json::Value> {
        let meta_path = Self::meta_path(cached_file);
        let content = std::fs::read_to_string(&meta_path).ok()?;
        serde_json::from_str(&content).ok()
    }

    /// Download a URL to a local cache file.
    ///
    /// Captures ETag and Last-Modified response headers and saves them as
    /// metadata for future conditional requests in `get_status()`.
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

            // Capture ETag and Last-Modified from the GET response headers
            // BEFORE consuming the body (headers are available on the response).
            let etag = response
                .headers()
                .get("etag")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());
            let last_modified = response
                .headers()
                .get("last-modified")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());

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

            // Save metadata from the GET response for future conditional requests.
            Self::save_cache_metadata(cached_file, url, etag.as_deref(), last_modified.as_deref());

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

#[async_trait]
impl SourceHandlerWithStatus for HttpSourceHandler {
    /// Check HTTP source status without side effects.
    ///
    /// Uses a HEAD request with `If-None-Match` (ETag) or `If-Modified-Since`
    /// conditional headers from cached metadata to determine whether the remote
    /// resource has changed.
    ///
    /// - Not cached → `has_update: None`, summary: "Not cached"
    /// - HEAD returns 304 Not Modified → `has_update: Some(false)`
    /// - HEAD returns 200 OK → `has_update: Some(true)`
    /// - HEAD fails (network error, timeout) → `has_update: None`, error set
    /// - Feature `http-sources` not enabled → `has_update: None`, error set
    async fn get_status(
        &self,
        parsed: &ParsedURI,
        cache_dir: &Path,
    ) -> crate::error::Result<SourceStatus> {
        let url = Self::build_url(parsed);
        let cached_file = Self::cached_file_path(&url, parsed, cache_dir);
        let mut status = SourceStatus::new(&url);

        let is_cached = cached_file.exists();
        status.is_cached = is_cached;

        if !is_cached {
            status.has_update = None;
            status.summary = "Not cached".to_string();
            return Ok(status);
        }

        // Cached: send conditional HEAD request to detect changes.
        #[cfg(feature = "http-sources")]
        {
            let metadata = Self::get_cache_metadata(&cached_file);

            // Populate cached_at from metadata if available.
            if let Some(ref meta) = metadata {
                if let Some(cached_at) = meta.get("cached_at").and_then(|v| v.as_str()) {
                    status.cached_at = Some(cached_at.to_string());
                }
            }

            let client = reqwest::Client::new();
            let mut request = client.head(&url);

            // Apply conditional request headers from stored metadata.
            // Per RFC 7232 §3.3, send both validators when available.
            if let Some(ref meta) = metadata {
                if let Some(etag) = meta.get("etag").and_then(|v| v.as_str()) {
                    if !etag.is_empty() {
                        request = request.header("If-None-Match", etag);
                    }
                }
                if let Some(last_modified) = meta.get("last_modified").and_then(|v| v.as_str()) {
                    if !last_modified.is_empty() {
                        request = request.header("If-Modified-Since", last_modified);
                    }
                }
            }

            let head_result =
                tokio::time::timeout(std::time::Duration::from_secs(30), request.send()).await;

            match head_result {
                Ok(Ok(response)) => {
                    let http_status = response.status().as_u16();
                    if http_status == 304 {
                        status.has_update = Some(false);
                        status.summary = "Up to date (304 Not Modified)".to_string();
                    } else if response.status().is_success() {
                        status.has_update = Some(true);
                        status.summary = format!("Update available (HTTP {http_status})");
                    } else {
                        status.has_update = None;
                        let msg = format!("HEAD request returned HTTP {http_status}");
                        status.error = Some(msg.clone());
                        status.summary = format!("Unknown ({msg})");
                    }
                }
                Ok(Err(e)) => {
                    status.has_update = None;
                    let msg = format!("HEAD request failed: {e}");
                    status.error = Some(msg.clone());
                    status.summary = format!("Unknown ({msg})");
                }
                Err(_) => {
                    status.has_update = None;
                    let msg = "HEAD request timed out".to_string();
                    status.error = Some(msg.clone());
                    status.summary = format!("Unknown ({msg})");
                }
            }

            return Ok(status);
        }

        #[cfg(not(feature = "http-sources"))]
        {
            status.has_update = None;
            status.error = Some("HTTP feature not enabled".to_string());
            status.summary = "HTTP feature not enabled".to_string();
            Ok(status)
        }
    }

    /// Force re-download by removing cache and re-resolving.
    ///
    /// Removes the cached file and its metadata sidecar, then delegates to
    /// `resolve()` for a fresh download. The `download()` method called by
    /// `resolve()` captures ETag/Last-Modified from the GET response headers
    /// and writes `.meta.json` automatically — no separate HEAD needed.
    ///
    /// When `http-sources` feature is not enabled, `resolve()` will return a
    /// `LoadError` explaining that the feature is required.
    async fn update(
        &self,
        parsed: &ParsedURI,
        cache_dir: &Path,
    ) -> crate::error::Result<ResolvedSource> {
        let url = Self::build_url(parsed);
        let cached_file = Self::cached_file_path(&url, parsed, cache_dir);

        // Remove cached content (file or directory) if present.
        if cached_file.exists() {
            if cached_file.is_dir() {
                std::fs::remove_dir_all(&cached_file).map_err(|e| {
                    crate::error::BundleError::LoadError {
                        reason: format!(
                            "Failed to remove cached directory for update: {}",
                            cached_file.display()
                        ),
                        source: Some(Box::new(e)),
                    }
                })?;
            } else {
                std::fs::remove_file(&cached_file).map_err(|e| {
                    crate::error::BundleError::LoadError {
                        reason: format!(
                            "Failed to remove cached file for update: {}",
                            cached_file.display()
                        ),
                        source: Some(Box::new(e)),
                    }
                })?;
            }
        }

        // Remove the metadata sidecar (best-effort).
        let meta = Self::meta_path(&cached_file);
        if meta.exists() {
            let _ = std::fs::remove_file(&meta);
        }

        // Fresh download via resolve(). download() captures ETag/Last-Modified
        // from the GET response and writes .meta.json automatically.
        self.resolve(parsed, cache_dir).await
    }
}
