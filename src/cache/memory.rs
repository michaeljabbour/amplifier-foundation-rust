use std::collections::HashMap;

use super::CacheProvider;

pub struct SimpleCache {
    store: HashMap<String, serde_yaml_ng::Value>,
}

impl SimpleCache {
    pub fn new() -> Self {
        todo!()
    }
}

impl Default for SimpleCache {
    fn default() -> Self {
        Self::new()
    }
}

impl CacheProvider for SimpleCache {
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
