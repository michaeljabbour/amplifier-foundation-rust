use crate::paths::uri::ResolvedSource;

pub struct SimpleSourceResolver {
    handlers: Vec<Box<dyn super::SourceHandler>>,
}

impl SimpleSourceResolver {
    pub fn new() -> Self {
        todo!()
    }

    pub async fn resolve(&self, uri: &str) -> crate::error::Result<ResolvedSource> {
        todo!()
    }
}
