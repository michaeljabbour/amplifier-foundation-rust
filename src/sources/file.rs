use std::path::{Path, PathBuf};
use async_trait::async_trait;
use crate::paths::uri::{ParsedURI, ResolvedSource};
use super::SourceHandler;

pub struct FileSourceHandler {
    pub base_path: Option<PathBuf>,
}

impl FileSourceHandler {
    pub fn new() -> Self {
        todo!()
    }

    pub fn with_base_path(base_path: PathBuf) -> Self {
        todo!()
    }
}

#[async_trait]
impl SourceHandler for FileSourceHandler {
    fn can_handle(&self, parsed: &ParsedURI) -> bool {
        todo!()
    }

    async fn resolve(
        &self,
        parsed: &ParsedURI,
        cache_dir: &Path,
    ) -> crate::error::Result<ResolvedSource> {
        todo!()
    }
}
