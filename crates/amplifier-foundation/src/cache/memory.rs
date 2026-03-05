use std::collections::HashMap;

use super::CacheProvider;

/// Simple in-memory cache. No TTL, no eviction.
/// Bundles cached until clear() or process exit.
pub struct SimpleCache {
    store: HashMap<String, serde_yaml_ng::Value>,
}

impl SimpleCache {
    pub fn new() -> Self {
        SimpleCache {
            store: HashMap::new(),
        }
    }
}

impl Default for SimpleCache {
    fn default() -> Self {
        Self::new()
    }
}

impl CacheProvider for SimpleCache {
    fn get(&self, key: &str) -> Option<serde_yaml_ng::Value> {
        self.store.get(key).cloned()
    }

    fn set(&mut self, key: &str, value: serde_yaml_ng::Value) {
        self.store.insert(key.to_string(), value);
    }

    fn clear(&mut self) {
        self.store.clear();
    }

    fn contains(&self, key: &str) -> bool {
        self.store.contains_key(key)
    }
}
