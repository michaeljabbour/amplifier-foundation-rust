pub mod disk;
pub mod memory;

/// Cache provider trait for storing and retrieving bundles.
pub trait CacheProvider {
    fn get(&self, key: &str) -> Option<serde_yaml_ng::Value>;
    fn set(&mut self, key: &str, value: serde_yaml_ng::Value);
    fn clear(&mut self);
    fn contains(&self, key: &str) -> bool;
}
