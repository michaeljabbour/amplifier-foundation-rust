use std::path::{Path, PathBuf};

use super::CacheProvider;

pub struct DiskCache {
    pub cache_dir: PathBuf,
}

impl DiskCache {
    pub fn new(_cache_dir: &Path) -> Self {
        todo!()
    }

    pub fn cache_key_to_path(&self, _key: &str) -> PathBuf {
        todo!()
    }
}

impl CacheProvider for DiskCache {
    fn get(&self, _key: &str) -> Option<serde_yaml_ng::Value> {
        todo!()
    }

    fn set(&mut self, _key: &str, _value: serde_yaml_ng::Value) {
        todo!()
    }

    fn clear(&mut self) {
        todo!()
    }

    fn contains(&self, _key: &str) -> bool {
        todo!()
    }
}
