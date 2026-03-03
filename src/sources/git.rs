use std::path::Path;
use async_trait::async_trait;
use crate::paths::uri::{ParsedURI, ResolvedSource};
use super::SourceHandler;

pub struct GitSourceHandler;

impl Default for GitSourceHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl GitSourceHandler {
    pub fn new() -> Self {
        todo!()
    }
}

#[async_trait]
impl SourceHandler for GitSourceHandler {
    fn can_handle(&self, _parsed: &ParsedURI) -> bool {
        todo!()
    }

    async fn resolve(
        &self,
        _parsed: &ParsedURI,
        _cache_dir: &Path,
    ) -> crate::error::Result<ResolvedSource> {
        todo!()
    }
}
