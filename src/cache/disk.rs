use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use super::CacheProvider;

/// Disk-based cache using JSON serialization.
/// Cache directory provided by caller (mechanism, not policy).
pub struct DiskCache {
    pub cache_dir: PathBuf,
}

impl DiskCache {
    pub fn new(cache_dir: &Path) -> Self {
        let cache = DiskCache {
            cache_dir: cache_dir.to_path_buf(),
        };
        cache.ensure_cache_dir();
        cache
    }

    /// Convert cache key to filesystem path.
    ///
    /// SHA-256 hash of key (first 16 hex chars) plus first 30 chars of key
    /// as safe prefix (non-alphanumeric chars replaced with `_`).
    pub fn cache_key_to_path(&self, key: &str) -> PathBuf {
        let mut hasher = Sha256::new();
        hasher.update(key.as_bytes());
        let hash = hasher.finalize();
        let hex_hash = format!("{hash:x}");
        let key_hash = &hex_hash[..32];

        let safe_prefix: String = key
            .chars()
            .take(30)
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();

        self.cache_dir
            .join(format!("{safe_prefix}-{key_hash}.json"))
    }

    fn ensure_cache_dir(&self) {
        let _ = std::fs::create_dir_all(&self.cache_dir);
    }
}

impl CacheProvider for DiskCache {
    fn get(&self, key: &str) -> Option<serde_yaml_ng::Value> {
        let path = self.cache_key_to_path(key);
        if !path.exists() {
            return None;
        }

        match std::fs::read_to_string(&path) {
            Ok(content) => match serde_json::from_str::<serde_yaml_ng::Value>(&content) {
                Ok(value) => Some(value),
                Err(_) => {
                    // Invalid cache entry - remove it
                    let _ = std::fs::remove_file(&path);
                    None
                }
            },
            Err(_) => None,
        }
    }

    fn set(&mut self, key: &str, value: serde_yaml_ng::Value) {
        self.ensure_cache_dir();
        let path = self.cache_key_to_path(key);
        if let Ok(json) = serde_json::to_string_pretty(&value) {
            let _ = std::fs::write(&path, json);
        }
    }

    fn clear(&mut self) {
        if self.cache_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&self.cache_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|e| e.to_str()) == Some("json") {
                        let _ = std::fs::remove_file(&path);
                    }
                }
            }
        }
    }

    fn contains(&self, key: &str) -> bool {
        self.cache_key_to_path(key).exists()
    }
}
