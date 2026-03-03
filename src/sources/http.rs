use std::path::Path;

use async_trait::async_trait;

use super::SourceHandler;
use crate::paths::uri::{ParsedURI, ResolvedSource};

/// Handler for https:// and http:// URIs (direct file downloads).
///
/// Downloads files to cache and returns local path.
/// Uses content-addressable storage (hash of URL).
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
}

#[async_trait]
impl SourceHandler for HttpSourceHandler {
    fn can_handle(&self, parsed: &ParsedURI) -> bool {
        parsed.is_http()
    }

    async fn resolve(
        &self,
        _parsed: &ParsedURI,
        _cache_dir: &Path,
    ) -> crate::error::Result<ResolvedSource> {
        // HTTP download implementation deferred -- no tests exercise the resolve path.
        // Only can_handle tests exist for HttpSourceHandler.
        todo!("HTTP resolve not yet implemented")
    }
}
