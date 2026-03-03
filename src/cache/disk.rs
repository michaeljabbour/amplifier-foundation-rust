use std::path::{Path, PathBuf};

use super::CacheProvider;

pub struct DiskCache {
    pub cache_dir: PathBuf,
}

impl DiskCache {
    pub fn new(cache_dir: &Path) -> Self {
        todo!()
    }

    pub fn cache_key_to_path(&self, key: &str) -> PathBuf {
        todo!()
    }
}

impl CacheProvider for DiskCache {
    fn get(&self, key: &str) -> Option<serde_yaml_ng::Value> {
        todo!()
    }

    fn set(&mut self, key: &str, value: serde_yaml_ng::Value) {
        todo!()
    }

    fn clear(&mut self) {
        todo!()
    }

    fn contains(&self, key: &str) -> bool {
        todo!()
    }
}
