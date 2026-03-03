use std::path::Path;
use async_trait::async_trait;
use crate::paths::uri::{ParsedURI, ResolvedSource};
use super::SourceHandler;

pub struct ZipSourceHandler;

impl Default for ZipSourceHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl ZipSourceHandler {
    pub fn new() -> Self {
        todo!()
    }
}

#[async_trait]
impl SourceHandler for ZipSourceHandler {
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
