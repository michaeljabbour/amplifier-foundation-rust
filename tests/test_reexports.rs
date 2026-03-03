//! Tests that verify all pub use re-exports in lib.rs are accessible
//! from the crate root. This ensures the flat API surface works correctly.
//!
//! These are compile-time checks -- if any re-export is broken, this file
//! won't compile. The runtime assertions confirm the items are usable.

use amplifier_foundation::*;
use std::path::PathBuf;

// =============================================================================
// Core classes
// =============================================================================

#[test]
fn test_reexport_bundle() {
    let b = Bundle::new("test");
    assert_eq!(b.name, "test");
}

#[test]
fn test_reexport_bundle_from_dict() {
    let yaml = r#"
bundle:
  name: reexport-test
  version: "1.0"
"#;
    let data: serde_yaml_ng::Value = serde_yaml_ng::from_str(yaml).unwrap();
    let bundle = Bundle::from_dict(&data).unwrap();
    assert_eq!(bundle.name, "reexport-test");
}

#[test]
fn test_reexport_bundle_error() {
    // Verify BundleError variants are accessible
    let err = BundleError::NotFound {
        uri: "test://missing".into(),
    };
    assert!(format!("{}", err).contains("not found"));
}

#[test]
fn test_reexport_result_type() {
    // Verify Result type alias works
    let ok_result: Result<i32> = Ok(42);
    assert!(ok_result.is_ok());

    let err_result: Result<i32> = Err(BundleError::NotFound { uri: "x".into() });
    assert!(err_result.is_err());
}

// =============================================================================
// Validator
// =============================================================================

#[test]
fn test_reexport_validator() {
    let validator = BundleValidator::new();
    let bundle = Bundle::new("test");
    let result: ValidationResult = validator.validate(&bundle);
    // Just verify types work -- not testing validation logic here
    let _ = result.valid;
}

#[test]
fn test_reexport_validate_convenience() {
    let bundle = Bundle::new("test");
    let _result = validate_bundle(&bundle);
    let _result2 = validate_bundle_completeness(&bundle);
}

// =============================================================================
// Dict utilities
// =============================================================================

#[test]
fn test_reexport_dicts() {
    use serde_yaml_ng::Value;

    let a = serde_yaml_ng::from_str::<Value>("a: 1").unwrap();
    let b = serde_yaml_ng::from_str::<Value>("b: 2").unwrap();
    let merged = deep_merge(&a, &b);
    assert!(merged.as_mapping().unwrap().contains_key("a"));
    assert!(merged.as_mapping().unwrap().contains_key("b"));

    let data = serde_yaml_ng::from_str::<Value>("x:\n  y: 42").unwrap();
    let val = get_nested(&data, &["x", "y"]);
    assert!(val.is_some());

    let default_val =
        get_nested_with_default(&data, &["x", "missing"], Value::String("default".into()));
    assert_eq!(default_val, Value::String("default".into()));
}

// =============================================================================
// Path utilities
// =============================================================================

#[test]
fn test_reexport_paths() {
    let parsed: ParsedURI = parse_uri("file:///tmp/test");
    assert!(parsed.is_file());

    let norm: PathBuf = normalize_path("/tmp/foo/../bar", None);
    assert!(norm.to_str().unwrap().contains("bar"));

    let agent_path = construct_agent_path(&PathBuf::from("/base"), "my-agent");
    assert!(agent_path.to_str().unwrap().contains("my-agent"));

    let context_path = construct_context_path(&PathBuf::from("/base"), "ctx");
    assert!(context_path.to_str().unwrap().contains("ctx"));
}

// =============================================================================
// Serialization
// =============================================================================

#[test]
fn test_reexport_serialization() {
    let val = serde_json::json!({"key": "value", "null_key": null});
    let sanitized = sanitize_for_json(&val);
    assert!(!sanitized.as_object().unwrap().contains_key("null_key"));

    let msg = serde_json::json!({"role": "user", "content": "hello"});
    let _sanitized_msg = sanitize_message(&msg);
}

// =============================================================================
// Tracing
// =============================================================================

#[test]
fn test_reexport_tracing() {
    let session_id = generate_sub_session_id(Some("test-agent"), None, None);
    assert!(!session_id.is_empty());
}

// =============================================================================
// Spawn
// =============================================================================

#[test]
fn test_reexport_spawn() {
    let pref = ProviderPreference::new("anthropic", "claude-*");
    assert_eq!(pref.provider, "anthropic");

    assert!(is_glob_pattern("claude-*"));
    assert!(!is_glob_pattern("gpt-4"));

    let _result: ModelResolutionResult = ModelResolutionResult {
        resolved_model: "gpt-4".into(),
        pattern: None,
        available_models: None,
        matched_models: None,
    };
}

// =============================================================================
// Cache
// =============================================================================

#[test]
fn test_reexport_cache_types() {
    // Verify cache types are accessible
    let _cache = SimpleCache::new();
    // DiskCache needs a path - just verify the type is accessible
    let _cache2 = DiskCache::new(&PathBuf::from("/tmp/test-cache"));

    // CacheProvider trait is accessible (used as trait bound)
    fn _accepts_cache(_c: &dyn CacheProvider) {}
}

// =============================================================================
// Source types
// =============================================================================

#[test]
fn test_reexport_source_types() {
    let _status = SourceStatus {
        uri: "file:///test".into(),
        current_version: None,
        latest_version: None,
        has_update: Some(false),
    };
    let _handler = FileSourceHandler::new();
}
