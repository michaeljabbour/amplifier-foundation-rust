use std::collections::HashMap;
use std::path::PathBuf;

pub struct BaseMentionResolver {
    pub base_path: Option<PathBuf>,
    pub bundles: Option<HashMap<String, PathBuf>>,
}

impl Default for BaseMentionResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl BaseMentionResolver {
    pub fn new() -> Self {
        todo!()
    }

    pub fn with_base_path(_base_path: PathBuf) -> Self {
        todo!()
    }

    pub fn with_bundles(_bundles: HashMap<String, PathBuf>) -> Self {
        todo!()
    }

    /// Resolve a mention string to a file path.
    /// Returns None if the file doesn't exist.
    pub fn resolve(&self, _mention: &str) -> Option<PathBuf> {
        todo!()
    }
}
