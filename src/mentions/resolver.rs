use std::collections::HashMap;
use std::path::PathBuf;

pub struct BaseMentionResolver {
    pub base_path: Option<PathBuf>,
    pub bundles: Option<HashMap<String, PathBuf>>,
}

impl BaseMentionResolver {
    pub fn new() -> Self {
        todo!()
    }

    pub fn with_base_path(base_path: PathBuf) -> Self {
        todo!()
    }

    pub fn with_bundles(bundles: HashMap<String, PathBuf>) -> Self {
        todo!()
    }

    /// Resolve a mention string to a file path.
    /// Returns None if the file doesn't exist.
    pub fn resolve(&self, mention: &str) -> Option<PathBuf> {
        todo!()
    }
}
