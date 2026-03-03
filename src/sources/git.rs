use std::path::{Path, PathBuf};

use async_trait::async_trait;
use sha2::{Digest, Sha256};

use super::SourceHandler;
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
        let cache_key = &hash[..16];

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
    fn resolve_with_subpath(
        cache_path: PathBuf,
        subpath: &str,
    ) -> crate::error::Result<ResolvedSource> {
        if !subpath.is_empty() {
            let result_path = cache_path.join(subpath);
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
            return Err(crate::error::BundleError::NotFound {
                uri: format!("Failed to clone {git_url}@{ref_}: {stderr}"),
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
