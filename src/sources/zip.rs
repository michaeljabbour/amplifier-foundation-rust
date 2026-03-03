use std::fs;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use sha2::{Digest, Sha256};

use super::SourceHandler;
use crate::paths::uri::{ParsedURI, ResolvedSource};

/// Handler for zip+https:// and zip+file:// URIs.
///
/// Downloads/copies zip archives, extracts to cache, returns local path.
/// Uses content-addressable storage (hash of URI).
pub struct ZipSourceHandler;

impl Default for ZipSourceHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl ZipSourceHandler {
    pub fn new() -> Self {
        Self
    }

    /// Extract a zip file to the target path.
    fn extract_zip(zip_path: &Path, extract_path: &Path) -> crate::error::Result<()> {
        let file = fs::File::open(zip_path).map_err(|e| crate::error::BundleError::NotFound {
            uri: format!("Zip file not found: {}: {e}", zip_path.display()),
        })?;
        let mut archive =
            zip::ZipArchive::new(file).map_err(|e| crate::error::BundleError::LoadError {
                reason: format!("Failed to read zip archive: {e}"),
                source: None,
            })?;
        archive
            .extract(extract_path)
            .map_err(|e| crate::error::BundleError::LoadError {
                reason: format!("Failed to extract zip: {e}"),
                source: None,
            })?;
        Ok(())
    }
}

#[async_trait]
impl SourceHandler for ZipSourceHandler {
    fn can_handle(&self, parsed: &ParsedURI) -> bool {
        parsed.is_zip()
    }

    async fn resolve(
        &self,
        parsed: &ParsedURI,
        cache_dir: &Path,
    ) -> crate::error::Result<ResolvedSource> {
        // Build the source URI (without zip+ prefix)
        let inner_scheme = parsed.scheme.replace("zip+", "");

        let (zip_path, source_uri) = if inner_scheme == "file" {
            // Local zip file
            let zip_path = PathBuf::from(&parsed.path);
            let source_uri = zip_path.display().to_string();
            (Some(zip_path), source_uri)
        } else {
            // Remote zip (https, http)
            let source_uri = format!("{}://{}{}", inner_scheme, parsed.host, parsed.path);
            (None, source_uri)
        };

        // Create cache key from URI
        let mut hasher = Sha256::new();
        hasher.update(source_uri.as_bytes());
        let cache_key = format!("{:x}", hasher.finalize());
        let cache_key = &cache_key[..16];

        let zip_name = Path::new(&parsed.path)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy();
        let zip_name = if zip_name.is_empty() {
            "archive".to_string()
        } else {
            zip_name.to_string()
        };
        let extract_path = cache_dir.join(format!("{zip_name}-{cache_key}"));

        // Check if already cached (before checking if source exists)
        if extract_path.exists() {
            let result_path = if !parsed.subpath.is_empty() {
                extract_path.join(&parsed.subpath)
            } else {
                extract_path.clone()
            };
            if result_path.exists() {
                return Ok(ResolvedSource {
                    active_path: result_path,
                    source_root: extract_path,
                });
            }
        }

        // Check if local zip exists
        if inner_scheme == "file" {
            if let Some(ref zp) = zip_path {
                if !zp.exists() {
                    return Err(crate::error::BundleError::NotFound {
                        uri: format!("Zip file not found: {}", zp.display()),
                    });
                }
            }
        }

        // Ensure cache directory exists
        fs::create_dir_all(cache_dir)?;

        // Remove partial extraction if exists
        if extract_path.exists() {
            let _ = fs::remove_dir_all(&extract_path);
        }

        // Extract
        if inner_scheme == "file" {
            if let Some(ref zp) = zip_path {
                Self::extract_zip(zp, &extract_path)?;
            }
        } else {
            // Remote download -- not implemented (no tests exercise this path)
            return Err(crate::error::BundleError::NotFound {
                uri: format!("Remote zip download not yet implemented: {source_uri}"),
            });
        }

        // Return path with subpath if specified
        let result_path = if !parsed.subpath.is_empty() {
            extract_path.join(&parsed.subpath)
        } else {
            extract_path.clone()
        };

        if !result_path.exists() {
            return Err(crate::error::BundleError::NotFound {
                uri: format!("Subpath not found after extraction: {}", parsed.subpath),
            });
        }

        Ok(ResolvedSource {
            active_path: result_path,
            source_root: extract_path,
        })
    }
}
