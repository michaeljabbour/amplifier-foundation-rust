use serde_yaml_ng::{Mapping, Value};

use amplifier_foundation::spawn::glob::is_glob_pattern;
use amplifier_foundation::spawn::{apply_provider_preferences, ProviderPreference};

// -- helpers ----------------------------------------------------------

/// Build a `Value::Mapping` from a list of (key, value) pairs.
fn mapping(pairs: &[(&str, Value)]) -> Value {
    let mut m = Mapping::new();
    for (k, v) in pairs {
        m.insert(Value::String(k.to_string()), v.clone());
    }
    Value::Mapping(m)
}

/// Shorthand: create a `Value::Number` from an integer.
fn int(n: i64) -> Value {
    serde_yaml_ng::to_value(n).unwrap()
}

/// Shorthand: create a `Value::String`.
fn str_val(s: &str) -> Value {
    Value::String(s.to_string())
}

/// Build a provider entry: {"module": module_name, "config": {config_pairs...}}
fn provider_entry(module: &str, config_pairs: &[(&str, Value)]) -> Value {
    let config = mapping(config_pairs);
    mapping(&[("module", str_val(module)), ("config", config)])
}

/// Build a mount plan with the given provider list.
fn make_mount_plan(providers: Vec<Value>) -> Value {
    mapping(&[("providers", Value::Sequence(providers))])
}

// =====================================================================
// TestProviderPreference
// =====================================================================

#[test]
fn test_create_provider_preference() {
    let pref = ProviderPreference::new("anthropic", "claude-haiku-3");
    assert_eq!(pref.provider, "anthropic");
    assert_eq!(pref.model, "claude-haiku-3");
}

#[test]
fn test_to_dict() {
    let pref = ProviderPreference::new("openai", "gpt-4o-mini");
    let result = pref.to_dict();
    let expected = mapping(&[
        ("provider", str_val("openai")),
        ("model", str_val("gpt-4o-mini")),
    ]);
    assert_eq!(result, expected);
}

#[test]
fn test_from_dict() {
    let data = mapping(&[
        ("provider", str_val("anthropic")),
        ("model", str_val("claude-haiku-3")),
    ]);
    let pref = ProviderPreference::from_dict(&data).expect("should parse");
    assert_eq!(pref.provider, "anthropic");
    assert_eq!(pref.model, "claude-haiku-3");
}

#[test]
fn test_from_dict_missing_provider() {
    let data = mapping(&[("model", str_val("gpt-4o-mini"))]);
    let result = ProviderPreference::from_dict(&data);
    assert!(result.is_err(), "expected error when provider is missing");
}

#[test]
fn test_from_dict_missing_model() {
    let data = mapping(&[("provider", str_val("openai"))]);
    let result = ProviderPreference::from_dict(&data);
    assert!(result.is_err(), "expected error when model is missing");
}

// =====================================================================
// TestIsGlobPattern
// =====================================================================

#[test]
fn test_not_a_pattern() {
    assert!(!is_glob_pattern("claude-3-haiku-20240307"));
    assert!(!is_glob_pattern("gpt-4o-mini"));
    assert!(!is_glob_pattern("claude-sonnet-4-20250514"));
}

#[test]
fn test_asterisk_pattern() {
    assert!(is_glob_pattern("claude-haiku-*"));
    assert!(is_glob_pattern("*-haiku-*"));
    assert!(is_glob_pattern("gpt-4*"));
}

#[test]
fn test_question_mark_pattern() {
    assert!(is_glob_pattern("gpt-4?"));
    assert!(is_glob_pattern("claude-?-haiku"));
}

#[test]
fn test_bracket_pattern() {
    assert!(is_glob_pattern("gpt-[45]"));
    assert!(is_glob_pattern("claude-[a-z]-haiku"));
}

// =====================================================================
// TestApplyProviderPreferences
// =====================================================================

#[test]
fn test_empty_preferences() {
    let mount_plan = make_mount_plan(vec![
        provider_entry("provider-anthropic", &[]),
    ]);
    let result = apply_provider_preferences(&mount_plan, &[]);
    assert_eq!(result, mount_plan);
}

#[test]
fn test_no_providers_in_mount_plan() {
    let mount_plan = mapping(&[("orchestrator", mapping(&[("module", str_val("loop-basic"))]))]);
    let prefs = [ProviderPreference::new("anthropic", "claude-haiku-3")];
    let result = apply_provider_preferences(&mount_plan, &prefs);
    assert_eq!(result, mount_plan);
}

#[test]
fn test_first_preference_matches() {
    let mount_plan = make_mount_plan(vec![
        provider_entry("provider-anthropic", &[("priority", int(10))]),
        provider_entry("provider-openai", &[("priority", int(20))]),
    ]);
    let prefs = [
        ProviderPreference::new("anthropic", "claude-haiku-3"),
        ProviderPreference::new("openai", "gpt-4o-mini"),
    ];
    let result = apply_provider_preferences(&mount_plan, &prefs);

    // Providers is a list of dicts
    let providers = result
        .as_mapping().unwrap()
        .get(&str_val("providers")).unwrap()
        .as_sequence().unwrap();

    // Anthropic should be promoted to priority 0 with preferred model
    let anthropic_config = providers[0]
        .as_mapping().unwrap()
        .get(&str_val("config")).unwrap()
        .as_mapping().unwrap();
    assert_eq!(anthropic_config.get(&str_val("priority")), Some(&int(0)));
    assert_eq!(
        anthropic_config.get(&str_val("model")),
        Some(&str_val("claude-haiku-3"))
    );

    // OpenAI should be unchanged
    let openai_config = providers[1]
        .as_mapping().unwrap()
        .get(&str_val("config")).unwrap()
        .as_mapping().unwrap();
    assert_eq!(openai_config.get(&str_val("priority")), Some(&int(20)));
}

#[test]
fn test_second_preference_matches_when_first_unavailable() {
    // Only openai is in the mount plan; anthropic pref listed first but unavailable.
    let mount_plan = make_mount_plan(vec![
        provider_entry("provider-openai", &[("priority", int(10))]),
    ]);
    let prefs = [
        ProviderPreference::new("anthropic", "claude-haiku-3"),
        ProviderPreference::new("openai", "gpt-4o-mini"),
    ];
    let result = apply_provider_preferences(&mount_plan, &prefs);

    let providers = result
        .as_mapping().unwrap()
        .get(&str_val("providers")).unwrap()
        .as_sequence().unwrap();

    // OpenAI should be promoted since anthropic isn't available
    let openai_config = providers[0]
        .as_mapping().unwrap()
        .get(&str_val("config")).unwrap()
        .as_mapping().unwrap();
    assert_eq!(openai_config.get(&str_val("priority")), Some(&int(0)));
    assert_eq!(
        openai_config.get(&str_val("model")),
        Some(&str_val("gpt-4o-mini"))
    );
}

#[test]
fn test_no_preferences_match() {
    let mount_plan = make_mount_plan(vec![
        provider_entry("provider-azure", &[("priority", int(10))]),
    ]);
    let prefs = [
        ProviderPreference::new("anthropic", "claude-haiku-3"),
        ProviderPreference::new("openai", "gpt-4o-mini"),
    ];
    let result = apply_provider_preferences(&mount_plan, &prefs);

    // Should be unchanged
    let providers = result
        .as_mapping().unwrap()
        .get(&str_val("providers")).unwrap()
        .as_sequence().unwrap();
    let config = providers[0]
        .as_mapping().unwrap()
        .get(&str_val("config")).unwrap()
        .as_mapping().unwrap();
    assert_eq!(config.get(&str_val("priority")), Some(&int(10)));
    assert!(config.get(&str_val("model")).is_none());
}

#[test]
fn test_flexible_provider_matching_short_name() {
    // Short name "anthropic" should match module "provider-anthropic".
    let mount_plan = make_mount_plan(vec![
        provider_entry("provider-anthropic", &[]),
    ]);
    let prefs = [ProviderPreference::new("anthropic", "claude-haiku-3")];
    let result = apply_provider_preferences(&mount_plan, &prefs);

    let providers = result
        .as_mapping().unwrap()
        .get(&str_val("providers")).unwrap()
        .as_sequence().unwrap();
    let config = providers[0]
        .as_mapping().unwrap()
        .get(&str_val("config")).unwrap()
        .as_mapping().unwrap();
    assert_eq!(config.get(&str_val("priority")), Some(&int(0)));
    assert_eq!(
        config.get(&str_val("model")),
        Some(&str_val("claude-haiku-3"))
    );
}

#[test]
fn test_flexible_provider_matching_full_name() {
    // Full module name "provider-anthropic" should match directly.
    let mount_plan = make_mount_plan(vec![
        provider_entry("provider-anthropic", &[]),
    ]);
    let prefs = [ProviderPreference::new("provider-anthropic", "claude-haiku-3")];
    let result = apply_provider_preferences(&mount_plan, &prefs);

    let providers = result
        .as_mapping().unwrap()
        .get(&str_val("providers")).unwrap()
        .as_sequence().unwrap();
    let config = providers[0]
        .as_mapping().unwrap()
        .get(&str_val("config")).unwrap()
        .as_mapping().unwrap();
    assert_eq!(config.get(&str_val("priority")), Some(&int(0)));
}

#[test]
fn test_mount_plan_not_mutated() {
    let mount_plan = make_mount_plan(vec![
        provider_entry("provider-anthropic", &[("priority", int(10))]),
    ]);
    let original = mount_plan.clone();
    let prefs = [ProviderPreference::new("anthropic", "claude-haiku-3")];

    let result = apply_provider_preferences(&mount_plan, &prefs);

    // Original should be unchanged (apply takes &Value)
    assert_eq!(mount_plan, original);

    // Result should have new values
    let providers = result
        .as_mapping().unwrap()
        .get(&str_val("providers")).unwrap()
        .as_sequence().unwrap();
    let config = providers[0]
        .as_mapping().unwrap()
        .get(&str_val("config")).unwrap()
        .as_mapping().unwrap();
    assert_eq!(config.get(&str_val("priority")), Some(&int(0)));
    assert_eq!(
        config.get(&str_val("model")),
        Some(&str_val("claude-haiku-3"))
    );

    // But original mount plan should still have priority 10, no model
    let orig_providers = mount_plan
        .as_mapping().unwrap()
        .get(&str_val("providers")).unwrap()
        .as_sequence().unwrap();
    let orig_config = orig_providers[0]
        .as_mapping().unwrap()
        .get(&str_val("config")).unwrap()
        .as_mapping().unwrap();
    assert_eq!(orig_config.get(&str_val("priority")), Some(&int(10)));
    assert!(orig_config.get(&str_val("model")).is_none());
}
