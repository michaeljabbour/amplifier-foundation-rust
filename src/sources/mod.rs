use crate::paths::uri::{ParsedURI, ResolvedSource};
use async_trait::async_trait;
use std::path::Path;

pub mod file;
pub mod git;
pub mod http;
pub mod resolver;
pub mod zip;

/// Status of a bundle source (for update checking).
#[derive(Debug, Clone)]
pub struct SourceStatus {
    pub uri: String,
    pub current_version: Option<String>,
    pub latest_version: Option<String>,
    pub has_update: bool,
}

/// Trait for source handlers that resolve URIs to local paths.
#[async_trait]
pub trait SourceHandler: Send + Sync {
    /// Check if this handler can handle the given parsed URI.
    fn can_handle(&self, parsed: &ParsedURI) -> bool;

    /// Resolve the URI to a local path, using cache_dir for caching.
    async fn resolve(
        &self,
        parsed: &ParsedURI,
        cache_dir: &Path,
    ) -> crate::error::Result<ResolvedSource>;
}
