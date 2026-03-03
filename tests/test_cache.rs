//! Tests for cache module (SimpleCache and DiskCache).
//!
//! Ported from Python test_cache.py — 12 tests total.
//! All tests are Wave 1 (ignored until implementations land).

use amplifier_foundation::cache::disk::DiskCache;
use amplifier_foundation::cache::memory::SimpleCache;
use amplifier_foundation::cache::CacheProvider;
use serde_yaml_ng::Value;
use std::fs;
use tempfile::tempdir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Create a simple Value::Mapping that mimics a minimal bundle dict.
fn make_bundle_value(name: &str, version: &str) -> Value {
    let mut map = serde_yaml_ng::Mapping::new();
    map.insert(Value::String("name".into()), Value::String(name.into()));
    map.insert(
        Value::String("version".into()),
        Value::String(version.into()),
    );
    Value::Mapping(map)
}

/// Create a complex Value::Mapping with nested structures, similar to a
/// fully-populated bundle with includes, providers, tools, etc.
fn make_complex_bundle_value() -> Value {
    let mut map = serde_yaml_ng::Mapping::new();
    map.insert(
        Value::String("name".into()),
        Value::String("complex-bundle".into()),
    );
    map.insert(
        Value::String("version".into()),
        Value::String("2.0.0".into()),
    );
    map.insert(
        Value::String("description".into()),
        Value::String("A complex bundle for testing".into()),
    );

    // includes: list of strings
    let includes = Value::Sequence(vec![
        Value::String("@core:base".into()),
        Value::String("@tools:python".into()),
    ]);
    map.insert(Value::String("includes".into()), includes);

    // providers: nested mapping
    let mut providers_map = serde_yaml_ng::Mapping::new();
    let mut openai = serde_yaml_ng::Mapping::new();
    openai.insert(
        Value::String("model".into()),
        Value::String("gpt-4".into()),
    );
    openai.insert(
        Value::String("temperature".into()),
        Value::Number(serde_yaml_ng::Number::from(0)),
    );
    providers_map.insert(Value::String("openai".into()), Value::Mapping(openai));
    map.insert(
        Value::String("providers".into()),
        Value::Mapping(providers_map),
    );

    // tools: list of mappings
    let mut tool1 = serde_yaml_ng::Mapping::new();
    tool1.insert(
        Value::String("name".into()),
        Value::String("read_file".into()),
    );
    tool1.insert(Value::String("enabled".into()), Value::Bool(true));

    let mut tool2 = serde_yaml_ng::Mapping::new();
    tool2.insert(
        Value::String("name".into()),
        Value::String("write_file".into()),
    );
    tool2.insert(Value::String("enabled".into()), Value::Bool(false));

    let tools = Value::Sequence(vec![Value::Mapping(tool1), Value::Mapping(tool2)]);
    map.insert(Value::String("tools".into()), tools);

    // tags: list of strings
    let tags = Value::Sequence(vec![
        Value::String("production".into()),
        Value::String("v2".into()),
    ]);
    map.insert(Value::String("tags".into()), tags);

    Value::Mapping(map)
}

// ===========================================================================
// TestSimpleCache
// ===========================================================================

#[test]
#[ignore = "Wave 1"]
fn test_simple_cache_get_miss() {
    let cache = SimpleCache::new();
    assert!(cache.get("nonexistent").is_none());
}

#[test]
#[ignore = "Wave 1"]
fn test_simple_cache_set_and_get() {
    let mut cache = SimpleCache::new();
    let value = make_bundle_value("test-bundle", "1.0.0");
    cache.set("test-key", value.clone());

    let result = cache.get("test-key");
    assert!(result.is_some());
    let result = result.unwrap();
    assert_eq!(result, value);
}

#[test]
#[ignore = "Wave 1"]
fn test_simple_cache_contains() {
    let mut cache = SimpleCache::new();
    assert!(!cache.contains("test-key"));

    let value = make_bundle_value("test-bundle", "1.0.0");
    cache.set("test-key", value);

    assert!(cache.contains("test-key"));
}

// ===========================================================================
// TestDiskCache
// ===========================================================================

#[test]
#[ignore = "Wave 1"]
fn test_disk_cache_requires_cache_dir() {
    let tmp = tempdir().expect("failed to create temp dir");
    let cache_dir = tmp.path().join("bundles");
    assert!(!cache_dir.exists());

    let _cache = DiskCache::new(&cache_dir);

    assert!(cache_dir.exists());
}

#[test]
#[ignore = "Wave 1"]
fn test_disk_cache_get_miss() {
    let tmp = tempdir().expect("failed to create temp dir");
    let cache = DiskCache::new(tmp.path());

    assert!(cache.get("nonexistent").is_none());
}

#[test]
#[ignore = "Wave 1"]
fn test_disk_cache_set_and_get() {
    let tmp = tempdir().expect("failed to create temp dir");
    let mut cache = DiskCache::new(tmp.path());

    let value = make_bundle_value("test-bundle", "1.0.0");
    cache.set("test-key", value.clone());

    let result = cache.get("test-key");
    assert!(result.is_some());
    let result = result.unwrap();
    assert_eq!(result, value);
}

#[test]
#[ignore = "Wave 1"]
fn test_disk_cache_persists_across_instances() {
    let tmp = tempdir().expect("failed to create temp dir");
    let cache_dir = tmp.path().to_path_buf();

    // Write with one instance
    {
        let mut cache = DiskCache::new(&cache_dir);
        let value = make_bundle_value("persistent-bundle", "1.0.0");
        cache.set("persist-key", value);
    }

    // Read with a new instance
    {
        let cache = DiskCache::new(&cache_dir);
        let result = cache.get("persist-key");
        assert!(result.is_some());

        let result = result.unwrap();
        // Verify the value survived round-trip through disk
        if let Value::Mapping(ref map) = result {
            assert_eq!(
                map.get(&Value::String("name".into())),
                Some(&Value::String("persistent-bundle".into()))
            );
            assert_eq!(
                map.get(&Value::String("version".into())),
                Some(&Value::String("1.0.0".into()))
            );
        } else {
            panic!("Expected Value::Mapping, got {:?}", result);
        }
    }
}

#[test]
#[ignore = "Wave 1"]
fn test_disk_cache_contains() {
    let tmp = tempdir().expect("failed to create temp dir");
    let mut cache = DiskCache::new(tmp.path());

    assert!(!cache.contains("test-key"));

    let value = make_bundle_value("test-bundle", "1.0.0");
    cache.set("test-key", value);

    assert!(cache.contains("test-key"));
}

#[test]
#[ignore = "Wave 1"]
fn test_disk_cache_clear() {
    let tmp = tempdir().expect("failed to create temp dir");
    let mut cache = DiskCache::new(tmp.path());

    let value1 = make_bundle_value("bundle-1", "1.0.0");
    let value2 = make_bundle_value("bundle-2", "2.0.0");
    cache.set("key-1", value1);
    cache.set("key-2", value2);

    // Both keys present before clear
    assert!(cache.contains("key-1"));
    assert!(cache.contains("key-2"));

    cache.clear();

    // Both keys gone after clear
    assert!(!cache.contains("key-1"));
    assert!(!cache.contains("key-2"));
    assert!(cache.get("key-1").is_none());
    assert!(cache.get("key-2").is_none());
}

#[test]
#[ignore = "Wave 1"]
fn test_disk_cache_handles_complex_bundle() {
    let tmp = tempdir().expect("failed to create temp dir");
    let mut cache = DiskCache::new(tmp.path());

    let value = make_complex_bundle_value();
    cache.set("complex-key", value.clone());

    let result = cache.get("complex-key");
    assert!(result.is_some());
    let result = result.unwrap();
    assert_eq!(result, value);
}

#[test]
#[ignore = "Wave 1"]
fn test_disk_cache_invalid_cache_returns_none() {
    let tmp = tempdir().expect("failed to create temp dir");
    let mut cache = DiskCache::new(tmp.path());

    // First, set a valid entry so we know the cache file path
    let value = make_bundle_value("test-bundle", "1.0.0");
    cache.set("bad-key", value);

    // Overwrite the cache file with invalid JSON
    let cache_path = cache.cache_key_to_path("bad-key");
    fs::write(&cache_path, "this is not valid json {{{").expect("failed to write bad json");

    // get should return None for corrupted cache entries
    let result = cache.get("bad-key");
    assert!(result.is_none());
}

#[test]
#[ignore = "Wave 1"]
fn test_disk_cache_cache_key_to_path_safe_filename() {
    let tmp = tempdir().expect("failed to create temp dir");
    let cache = DiskCache::new(tmp.path());

    // URI-like key with special characters
    let path = cache.cache_key_to_path("https://example.com/bundles/my-bundle@1.0");

    let filename = path
        .file_name()
        .expect("path should have a filename")
        .to_string_lossy();

    // The filename should not contain URI-unsafe characters like :, /, @
    assert!(
        !filename.contains(':'),
        "filename should not contain ':': {filename}"
    );
    assert!(
        !filename.contains('/'),
        "filename should not contain '/': {filename}"
    );

    // The path should be under the cache directory
    assert!(
        path.starts_with(tmp.path()),
        "cache path should be under cache_dir"
    );

    // The filename should have a .json extension
    assert_eq!(
        path.extension().and_then(|e| e.to_str()),
        Some("json"),
        "cache file should have .json extension"
    );
}