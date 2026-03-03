//! Tests for bundle validator module.
//!
//! Ported from Python test_validator.py — 18 tests total.
//! All tests are Wave 3 (ignored until implementations land).

use serde_yaml_ng::{Mapping, Value};

use amplifier_foundation::bundle::validator::{
    validate_bundle, validate_bundle_completeness, validate_bundle_completeness_or_raise,
    validate_bundle_or_raise, BundleValidator, ValidationResult,
};
use amplifier_foundation::bundle::Bundle;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a `Value::Mapping` from a list of (key, value) pairs.
fn mapping(pairs: &[(&str, Value)]) -> Value {
    let mut m = Mapping::new();
    for (k, v) in pairs {
        m.insert(Value::String(k.to_string()), v.clone());
    }
    Value::Mapping(m)
}

/// Shorthand: create a `Value::String`.
fn str_val(s: &str) -> Value {
    Value::String(s.to_string())
}

/// Build a minimal "complete" bundle that has a session (with orchestrator
/// and context) plus at least one provider entry.
///
/// NOTE: Bundle::new only accepts a name. We mutate public fields directly
/// to set up the required structure. This may need adjustment once
/// Bundle::new is fully implemented.
fn make_complete_bundle() -> Bundle {
    let mut bundle = Bundle::new("complete-test");

    // session with orchestrator and context
    bundle.session = mapping(&[
        ("orchestrator", mapping(&[("module", str_val("openai"))])),
        ("context", mapping(&[("module", str_val("default"))])),
    ]);

    // At least one valid provider
    bundle.providers = vec![mapping(&[
        ("module", str_val("some-provider")),
        ("config", mapping(&[])),
    ])];

    bundle
}

// ═══════════════════════════════════════════════════════════════════════════
// TestValidationResult
// ═══════════════════════════════════════════════════════════════════════════

#[test]

fn test_initial_state() {
    let result = ValidationResult::new();
    assert!(result.valid);
    assert!(result.errors.is_empty());
    assert!(result.warnings.is_empty());
}

#[test]

fn test_add_error_marks_invalid() {
    let mut result = ValidationResult::new();
    result.add_error("test error");
    assert!(!result.valid);
    assert!(result.errors.iter().any(|e| e.contains("test error")));
}

#[test]

fn test_add_warning_keeps_valid() {
    let mut result = ValidationResult::new();
    result.add_warning("test warning");
    assert!(result.valid);
    assert!(result.warnings.iter().any(|w| w.contains("test warning")));
}

// ═══════════════════════════════════════════════════════════════════════════
// TestBundleValidator
// ═══════════════════════════════════════════════════════════════════════════

#[test]

fn test_validate_minimal_bundle() {
    let bundle = Bundle::new("test");
    let validator = BundleValidator::new();
    let result = validator.validate(&bundle);
    assert!(result.valid);
}

#[test]

fn test_validate_missing_name() {
    let bundle = Bundle::new("");
    let validator = BundleValidator::new();
    let result = validator.validate(&bundle);
    assert!(!result.valid);
    assert!(result.errors.iter().any(|e| e.to_lowercase().contains("name")));
}

#[test]

fn test_validate_module_entry_missing_module() {
    // Provider entry without a "module" key — should fail validation.
    // NOTE: We mutate the bundle's providers directly since Bundle::new
    // only takes a name.
    let mut bundle = Bundle::new("test");
    bundle.providers = vec![mapping(&[("config", mapping(&[]))])];

    let validator = BundleValidator::new();
    let result = validator.validate(&bundle);
    assert!(!result.valid);
    assert!(result
        .errors
        .iter()
        .any(|e| e.to_lowercase().contains("module")));
}

#[test]

fn test_validate_module_entry_invalid_config() {
    // Provider entry where config is a string instead of a mapping.
    // NOTE: We mutate the bundle's providers directly.
    let mut bundle = Bundle::new("test");
    bundle.providers = vec![mapping(&[
        ("module", str_val("test")),
        ("config", str_val("string")),
    ])];

    let validator = BundleValidator::new();
    let result = validator.validate(&bundle);
    assert!(!result.valid);
    assert!(result
        .errors
        .iter()
        .any(|e| e.to_lowercase().contains("config")));
}

// ═══════════════════════════════════════════════════════════════════════════
// TestCompletenessValidation
// ═══════════════════════════════════════════════════════════════════════════

#[test]

fn test_complete_bundle_is_valid() {
    let bundle = make_complete_bundle();
    let validator = BundleValidator::new();
    let result = validator.validate_completeness(&bundle);
    assert!(result.valid);
}

#[test]

fn test_missing_session_is_incomplete() {
    // Bundle with no session configured (default Null).
    let mut bundle = make_complete_bundle();
    bundle.session = Value::Null;

    let validator = BundleValidator::new();
    let result = validator.validate_completeness(&bundle);
    assert!(!result.valid);
    assert!(result
        .errors
        .iter()
        .any(|e| e.to_lowercase().contains("session")));
}

#[test]

fn test_missing_orchestrator_is_incomplete() {
    // Session present but without an orchestrator key.
    let mut bundle = make_complete_bundle();
    bundle.session = mapping(&[("context", mapping(&[("module", str_val("default"))]))]);

    let validator = BundleValidator::new();
    let result = validator.validate_completeness(&bundle);
    assert!(!result.valid);
}

#[test]

fn test_missing_context_is_incomplete() {
    // Session present but without a context key.
    let mut bundle = make_complete_bundle();
    bundle.session = mapping(&[("orchestrator", mapping(&[("module", str_val("openai"))]))]);

    let validator = BundleValidator::new();
    let result = validator.validate_completeness(&bundle);
    assert!(!result.valid);
}

#[test]

fn test_missing_providers_is_incomplete() {
    // Complete session but empty providers list.
    let mut bundle = make_complete_bundle();
    bundle.providers = vec![];

    let validator = BundleValidator::new();
    let result = validator.validate_completeness(&bundle);
    assert!(!result.valid);
}

#[test]

fn test_partial_bundle_is_expected_incomplete() {
    // A provider-only bundle: basic validate() passes but
    // validate_completeness() fails because session is missing.
    // NOTE: We mutate the bundle's providers directly.
    let mut bundle = Bundle::new("provider-only");
    bundle.providers = vec![mapping(&[
        ("module", str_val("some-provider")),
        ("config", mapping(&[])),
    ])];

    let validator = BundleValidator::new();

    let basic = validator.validate(&bundle);
    assert!(basic.valid, "basic validation should pass for a partial bundle");

    let completeness = validator.validate_completeness(&bundle);
    assert!(
        !completeness.valid,
        "completeness validation should fail for a partial bundle"
    );
}

#[test]

fn test_validate_completeness_or_raise_raises() {
    // An incomplete bundle should cause validate_completeness_or_raise to
    // return Err.
    let bundle = Bundle::new("incomplete");
    let validator = BundleValidator::new();
    let result = validator.validate_completeness_or_raise(&bundle);
    assert!(result.is_err());
}

// ═══════════════════════════════════════════════════════════════════════════
// TestConvenienceFunctions
// ═══════════════════════════════════════════════════════════════════════════

#[test]

fn test_validate_bundle() {
    let bundle = Bundle::new("test");
    let result = validate_bundle(&bundle);
    assert!(result.valid);
}

#[test]

fn test_validate_bundle_or_raise() {
    // Empty name should fail validation; the convenience function should
    // return Err.
    let bundle = Bundle::new("");
    let result = validate_bundle_or_raise(&bundle);
    assert!(result.is_err());
}

#[test]

fn test_validate_bundle_completeness() {
    let bundle = make_complete_bundle();
    let result = validate_bundle_completeness(&bundle);
    assert!(result.valid);
}

#[test]

fn test_validate_bundle_completeness_or_raise() {
    // Incomplete bundle (no session/providers) should return Err.
    let bundle = Bundle::new("incomplete");
    let result = validate_bundle_completeness_or_raise(&bundle);
    assert!(result.is_err());
}
