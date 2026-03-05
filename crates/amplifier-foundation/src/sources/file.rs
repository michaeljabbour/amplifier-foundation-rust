use std::path::{Path, PathBuf};

use async_trait::async_trait;

use super::SourceHandler;
use crate::paths::uri::{ParsedURI, ResolvedSource};

/// Handler for file:// URIs and local paths.
pub struct FileSourceHandler {
    pub base_path: PathBuf,
}

impl Default for FileSourceHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl FileSourceHandler {
    pub fn new() -> Self {
        Self {
            base_path: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        }
    }

    pub fn with_base_path(base_path: PathBuf) -> Self {
        Self { base_path }
    }
}

#[async_trait]
impl SourceHandler for FileSourceHandler {
    fn can_handle(&self, parsed: &ParsedURI) -> bool {
        parsed.is_file()
    }

    async fn resolve(
        &self,
        parsed: &ParsedURI,
        _cache_dir: &Path,
    ) -> crate::error::Result<ResolvedSource> {
        let path_str = &parsed.path;

        // Handle relative paths
        let resolved_path = if path_str.starts_with("./") || path_str.starts_with("../") {
            self.base_path.join(path_str)
        } else {
            PathBuf::from(path_str)
        };

        // Apply subpath if specified (with traversal protection)
        let active_path = if !parsed.subpath.is_empty() {
            super::safe_join(&resolved_path, &parsed.subpath)?
        } else {
            resolved_path.clone()
        };

        if !active_path.exists() {
            return Err(crate::error::BundleError::NotFound {
                uri: format!("File not found: {}", active_path.display()),
            });
        }

        // Determine source_root
        let source_root = if !parsed.subpath.is_empty() {
            // Subdirectory URI: source_root is the base path (before subpath)
            resolved_path
        } else {
            active_path.clone()
        };

        Ok(ResolvedSource {
            active_path,
            source_root,
        })
    }
}
