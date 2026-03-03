use std::path::Path;

use async_trait::async_trait;

use super::SourceHandler;
use crate::paths::uri::{ParsedURI, ResolvedSource};

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
}

#[async_trait]
impl SourceHandler for GitSourceHandler {
    fn can_handle(&self, parsed: &ParsedURI) -> bool {
        parsed.is_git()
    }

    async fn resolve(
        &self,
        _parsed: &ParsedURI,
        _cache_dir: &Path,
    ) -> crate::error::Result<ResolvedSource> {
        // Git clone implementation deferred -- no tests exercise the resolve path.
        todo!("Git resolve not yet implemented")
    }
}
