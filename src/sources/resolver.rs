use crate::paths::uri::ResolvedSource;

pub struct SimpleSourceResolver {
    #[allow(dead_code)]
    handlers: Vec<Box<dyn super::SourceHandler>>,
}

impl Default for SimpleSourceResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl SimpleSourceResolver {
    pub fn new() -> Self {
        todo!()
    }

    pub async fn resolve(&self, _uri: &str) -> crate::error::Result<ResolvedSource> {
        todo!()
    }
}
