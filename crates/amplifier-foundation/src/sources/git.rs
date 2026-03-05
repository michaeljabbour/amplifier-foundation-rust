use std::path::{Path, PathBuf};

use async_trait::async_trait;
use sha2::{Digest, Sha256};

use super::{SourceHandler, SourceHandlerWithStatus, SourceStatus};
use crate::paths::uri::{ParsedURI, ResolvedSource};

/// Metadata file name for tracking cache info.
const CACHE_METADATA_FILE: &str = ".amplifier_cache_meta.json";

/// Handler for git+https:// URIs.
///
/// Clones repositories to a cache directory and returns the local path.
/// Uses shallow clones for efficiency.
pub struct GitSourceHandler;

impl Default for GitSourceHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl GitSourceHandler {
    pub fn new() -> Self {
        Self
    }

    /// Build git URL from parsed URI (without git+ prefix).
    fn build_git_url(parsed: &ParsedURI) -> String {
        let scheme = parsed.scheme.replace("git+", "");
        format!("{scheme}://{}{}", parsed.host, parsed.path)
    }

    /// Get the cache path for a parsed URI.
    ///
    /// Cache key is SHA-256 of "{git_url}@{ref}" (first 16 hex chars).
    /// Directory name includes the repo name for readability.
    fn get_cache_path(parsed: &ParsedURI, cache_dir: &Path) -> PathBuf {
        let git_url = Self::build_git_url(parsed);
        let ref_ = if parsed.ref_.is_empty() {
            "HEAD"
        } else {
            &parsed.ref_
        };
        let cache_input = format!("{git_url}@{ref_}");
        let hash = format!("{:x}", Sha256::digest(cache_input.as_bytes()));
        let cache_key = &hash[..32];

        let repo_name = parsed
            .path
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or("repo");

        cache_dir.join(format!("{repo_name}-{cache_key}"))
    }

    /// Verify that a cloned repository has expected structure.
    ///
    /// Checks for .git directory and presence of expected files (pyproject.toml,
    /// setup.py, setup.cfg, bundle.md, or bundle.yaml).
    fn verify_clone_integrity(cache_path: &Path) -> bool {
        if !cache_path.exists() {
            return false;
        }

        // Must have .git directory
        if !cache_path.join(".git").exists() {
            tracing::warn!("Clone missing .git directory: {}", cache_path.display());
            return false;
        }

        // Check for Python module or amplifier bundle markers
        let has_python_module = cache_path.join("pyproject.toml").exists()
            || cache_path.join("setup.py").exists()
            || cache_path.join("setup.cfg").exists();
        let has_bundle =
            cache_path.join("bundle.md").exists() || cache_path.join("bundle.yaml").exists();

        if !has_python_module && !has_bundle {
            tracing::warn!(
                "Clone missing expected files (pyproject.toml/setup.py/bundle.md): {}",
                cache_path.display()
            );
            return false;
        }

        true
    }

    /// Apply subpath to a cached directory and return ResolvedSource.
    ///
    /// Uses `safe_join` to prevent directory traversal attacks via subpath.
    fn resolve_with_subpath(
        cache_path: PathBuf,
        subpath: &str,
    ) -> crate::error::Result<ResolvedSource> {
        if !subpath.is_empty() {
            let result_path = super::safe_join(&cache_path, subpath)?;
            if !result_path.exists() {
                return Err(crate::error::BundleError::NotFound {
                    uri: format!("Subpath not found after clone: {subpath}"),
                });
            }
            Ok(ResolvedSource {
                active_path: result_path,
                source_root: cache_path,
            })
        } else {
            Ok(ResolvedSource {
                active_path: cache_path.clone(),
                source_root: cache_path,
            })
        }
    }

    /// Save cache metadata as JSON.
    fn save_cache_metadata(cache_path: &Path, git_url: &str, ref_: &str) {
        let meta_path = cache_path.join(CACHE_METADATA_FILE);

        // Try to get commit hash from git
        let commit = Self::get_local_commit(cache_path);

        let metadata = serde_json::json!({
            "cached_at": chrono::Utc::now().to_rfc3339(),
            "ref": ref_,
            "commit": commit,
            "git_url": git_url,
        });

        match serde_json::to_string_pretty(&metadata) {
            Ok(content) => {
                if let Err(e) = std::fs::write(&meta_path, content) {
                    tracing::warn!(
                        "Failed to write cache metadata {}: {e}",
                        meta_path.display()
                    );
                }
            }
            Err(e) => {
                tracing::warn!("Failed to serialize cache metadata: {e}");
            }
        }
    }

    /// Get the commit SHA of the cached repository.
    fn get_local_commit(cache_path: &Path) -> Option<String> {
        std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(cache_path)
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Read cache metadata from the JSON sidecar file.
    ///
    /// Returns a JSON object with keys: cached_at, ref, commit, git_url.
    /// Returns None if the metadata file doesn't exist or is corrupted.
    fn get_cache_metadata(cache_path: &Path) -> Option<serde_json::Value> {
        let meta_path = cache_path.join(CACHE_METADATA_FILE);
        let content = std::fs::read_to_string(&meta_path).ok()?;
        serde_json::from_str(&content).ok()
    }

    /// Get the remote HEAD commit SHA via `git ls-remote`.
    ///
    /// Runs `git ls-remote <git_url> <ref>` as an async subprocess with a
    /// 30-second timeout. Returns None on any failure (network error, timeout,
    /// parse error). Matches Python's `_get_remote_commit` which wraps in
    /// `asyncio.wait_for(..., timeout=30)` and never raises.
    async fn get_remote_commit(git_url: &str, ref_: &str) -> Option<String> {
        let output = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            tokio::process::Command::new("git")
                .args(["ls-remote", git_url, ref_])
                .output(),
        )
        .await
        .ok()? // timeout elapsed → None
        .ok()?; // spawn/IO error → None

        if !output.status.success() {
            return None;
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        // git ls-remote output format: "<sha>\t<refname>\n"
        // Take the first column (SHA) from the first line
        stdout
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().next())
            .map(|sha| sha.to_string())
    }

    /// Build the source URI string for SourceStatus from a parsed URI.
    fn build_source_uri(parsed: &ParsedURI) -> String {
        let git_url = Self::build_git_url(parsed);
        let ref_ = if parsed.ref_.is_empty() {
            "HEAD"
        } else {
            &parsed.ref_
        };
        format!("{git_url}@{ref_}")
    }

    // NOTE: Pinned-ref detection is done via SourceStatus::is_pinned()
    // (in sources/mod.rs) to avoid duplicating the logic. The get_status
    // method sets cached_ref before checking, then calls status.is_pinned().
}

#[async_trait]
impl SourceHandler for GitSourceHandler {
    fn can_handle(&self, parsed: &ParsedURI) -> bool {
        parsed.is_git()
    }

    /// Resolve git URI to local cached path.
    ///
    /// Clones the repository if not cached. Uses BundleError::NotFound for clone
    /// failures to match Python's BundleNotFoundError behavior.
    async fn resolve(
        &self,
        parsed: &ParsedURI,
        cache_dir: &Path,
    ) -> crate::error::Result<ResolvedSource> {
        let git_url = Self::build_git_url(parsed);
        let ref_ = if parsed.ref_.is_empty() {
            "HEAD".to_string()
        } else {
            parsed.ref_.clone()
        };
        let cache_path = Self::get_cache_path(parsed, cache_dir);

        // Check if already cached and valid
        if cache_path.exists() {
            if !Self::verify_clone_integrity(&cache_path) {
                tracing::warn!(
                    "Cached clone is invalid, removing: {}",
                    cache_path.display()
                );
                let _ = std::fs::remove_dir_all(&cache_path);
            } else {
                // Valid cache — return directly (including subpath errors).
                // Don't destroy a valid cache just because a subpath is wrong.
                return Self::resolve_with_subpath(cache_path, &parsed.subpath);
            }
        }

        // Ensure cache parent directory exists
        if let Some(parent) = cache_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| crate::error::BundleError::LoadError {
                reason: format!("Failed to create cache directory: {}", parent.display()),
                source: Some(Box::new(e)),
            })?;
        }

        // Remove partial clone if exists
        if cache_path.exists() {
            let _ = std::fs::remove_dir_all(&cache_path);
        }

        // Build clone command: shallow clone with specific ref
        // Use .arg() for PathBuf to avoid lossy Display conversion on non-UTF-8 paths
        let mut cmd = tokio::process::Command::new("git");
        cmd.arg("clone").arg("--depth").arg("1");

        // "HEAD" is not a valid --branch argument; omit it to use default branch
        if ref_ != "HEAD" {
            cmd.arg("--branch").arg(&ref_);
        }

        cmd.arg(&git_url).arg(&cache_path);

        // Run git clone via subprocess
        let output = cmd
            .output()
            .await
            .map_err(|e| crate::error::BundleError::NotFound {
                uri: format!("Failed to run git clone for {git_url}@{ref_}: {e}"),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Truncate stderr to avoid leaking internal path/network details
            let stderr_truncated: String = stderr.chars().take(200).collect();
            return Err(crate::error::BundleError::NotFound {
                uri: format!("Failed to clone {git_url}@{ref_}: {stderr_truncated}"),
            });
        }

        // Verify clone completed with expected structure
        if !Self::verify_clone_integrity(&cache_path) {
            let _ = std::fs::remove_dir_all(&cache_path);
            return Err(crate::error::BundleError::NotFound {
                uri: format!(
                    "Clone of {git_url}@{ref_} completed but result is invalid \
                     (missing pyproject.toml/setup.py/bundle.md)"
                ),
            });
        }

        // Save metadata after successful clone
        Self::save_cache_metadata(&cache_path, &git_url, &ref_);

        Self::resolve_with_subpath(cache_path, &parsed.subpath)
    }
}

#[async_trait]
impl SourceHandlerWithStatus for GitSourceHandler {
    /// Check git source status without side effects.
    ///
    /// Uses `git ls-remote` to compare cached vs remote HEAD commit.
    /// Pinned refs (SHA or version tags) skip the remote check.
    ///
    /// Matches Python's `GitSourceHandler.get_status` behavior:
    /// - Pinned refs → `has_update: Some(false)`, summary: "Pinned to ..."
    /// - Not cached + remote found → `has_update: Some(true)`
    /// - Cached == remote → `has_update: Some(false)`
    /// - Cached != remote → `has_update: Some(true)`
    /// - Remote check failed → `has_update: None`, error set
    async fn get_status(
        &self,
        parsed: &ParsedURI,
        cache_dir: &Path,
    ) -> crate::error::Result<SourceStatus> {
        let git_url = Self::build_git_url(parsed);
        let ref_ = if parsed.ref_.is_empty() {
            "HEAD".to_string()
        } else {
            parsed.ref_.clone()
        };
        let source_uri = Self::build_source_uri(parsed);
        let cache_path = Self::get_cache_path(parsed, cache_dir);

        let mut status = SourceStatus::new(&source_uri);
        status.cached_ref = Some(ref_.clone());

        // Read cache metadata if available
        let is_cached = cache_path.exists() && Self::verify_clone_integrity(&cache_path);
        status.is_cached = is_cached;

        if is_cached {
            if let Some(meta) = Self::get_cache_metadata(&cache_path) {
                status.cached_at = meta
                    .get("cached_at")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                status.cached_commit = meta
                    .get("commit")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
            } else {
                // No metadata file — try git rev-parse as fallback
                status.cached_commit = Self::get_local_commit(&cache_path);
            }
        }

        // Check if ref is pinned (immutable — skip remote check).
        // Uses SourceStatus::is_pinned() which checks cached_ref for
        // 40-char hex SHAs and v-tags. Single canonical implementation.
        if status.is_pinned() {
            status.has_update = Some(false);
            status.summary = format!("Pinned to {ref_} (no updates possible)");
            return Ok(status);
        }

        // Query remote for latest commit
        status.remote_ref = Some(ref_.clone());
        match Self::get_remote_commit(&git_url, &ref_).await {
            Some(remote_commit) => {
                status.remote_commit = Some(remote_commit.clone());

                if !is_cached {
                    // Not cached, but remote is available
                    status.has_update = Some(true);
                    status.summary = format!(
                        "Not cached (remote: {})",
                        &remote_commit[..8.min(remote_commit.len())]
                    );
                } else if let Some(cached_commit) = &status.cached_commit {
                    if cached_commit == &remote_commit {
                        status.has_update = Some(false);
                        status.summary = format!(
                            "Up to date ({})",
                            &remote_commit[..8.min(remote_commit.len())]
                        );
                    } else {
                        status.has_update = Some(true);
                        let old_short = &cached_commit[..8.min(cached_commit.len())];
                        let new_short = &remote_commit[..8.min(remote_commit.len())];
                        status.summary = format!("Update available ({old_short} → {new_short})");
                    }
                } else {
                    // Cached but no commit info — treat as update available
                    status.has_update = Some(true);
                    status.summary = format!(
                        "Update available (no cached commit, remote: {})",
                        &remote_commit[..8.min(remote_commit.len())]
                    );
                }
            }
            None => {
                // Remote check failed — can't determine status
                status.has_update = None;
                let error_msg = format!("Failed to check remote ref '{ref_}' on {git_url}");
                status.error = Some(error_msg.clone());
                if is_cached {
                    status.summary = format!("Cached (remote check failed: {error_msg})");
                } else {
                    status.summary = format!("Not cached (remote check failed: {error_msg})");
                }
            }
        }

        Ok(status)
    }

    /// Force re-download by removing cache and re-resolving.
    ///
    /// This is a destructive operation: removes the existing cache directory
    /// and delegates to `resolve()` for a fresh clone.
    /// Matches Python's `GitSourceHandler.update` behavior: `shutil.rmtree`
    /// + `resolve()`.
    async fn update(
        &self,
        parsed: &ParsedURI,
        cache_dir: &Path,
    ) -> crate::error::Result<ResolvedSource> {
        let cache_path = Self::get_cache_path(parsed, cache_dir);

        // Remove existing cache — propagate errors since returning stale
        // data after a user explicitly called update() is worse than failing.
        // This differs from resolve()'s best-effort cleanup of invalid caches.
        if cache_path.exists() {
            std::fs::remove_dir_all(&cache_path).map_err(|e| {
                crate::error::BundleError::LoadError {
                    reason: format!(
                        "Failed to remove cache for update: {}",
                        cache_path.display()
                    ),
                    source: Some(Box::new(e)),
                }
            })?;
        }

        // Fresh clone via resolve()
        self.resolve(parsed, cache_dir).await
    }
}
