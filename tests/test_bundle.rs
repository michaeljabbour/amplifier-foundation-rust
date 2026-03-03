//! Tests for bundle module.
//!
//! Ported from Python test_bundle.py -- 26 tests across 6 groups.
//! All tests are Wave 3 (ignored until implementations land).

use std::fs;
use std::path::{Path, PathBuf};

use serde_yaml_ng::{Mapping, Value};
use tempfile::tempdir;

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

/// Shorthand: create a `Value::Sequence` from a slice of `Value`.
fn seq(items: &[Value]) -> Value {
    Value::Sequence(items.to_vec())
}

/// Shorthand: create a `Value::Bool`.
fn bool_val(b: bool) -> Value {
    Value::Bool(b)
}

/// Build a minimal bundle dict: `{"bundle": {"name": name}}`.
fn minimal_bundle_dict(name: &str) -> Value {
    mapping(&[("bundle", mapping(&[("name", str_val(name))]))])
}

/// Build a module entry: `{"module": module_name}`.
fn module_entry(module: &str) -> Value {
    mapping(&[("module", str_val(module))])
}

/// Build a module entry with config: `{"module": module_name, "config": {pairs...}}`.
fn module_entry_with_config(module: &str, config_pairs: &[(&str, Value)]) -> Value {
    mapping(&[
        ("module", str_val(module)),
        ("config", mapping(config_pairs)),
    ])
}

// =====================================================================
// TestBundle
// =====================================================================

#[test]

fn test_create_minimal() {
    let bundle = Bundle::new("test");
    assert_eq!(bundle.name, "test");
    assert_eq!(bundle.version, "1.0.0");
    assert!(bundle.providers.is_empty());
}

#[test]

fn test_from_dict_minimal() {
    let data = minimal_bundle_dict("test");
    let bundle = Bundle::from_dict(&data).expect("should parse minimal bundle");
    assert_eq!(bundle.name, "test");
}

#[test]

fn test_from_dict_full() {
    let data = mapping(&[(
        "bundle",
        mapping(&[
            ("name", str_val("full-test")),
            ("version", str_val("2.0.0")),
            ("description", str_val("A full test bundle")),
            (
                "session",
                mapping(&[
                    ("orchestrator", str_val("custom-orchestrator")),
                    ("context", mapping(&[("key", str_val("value"))])),
                ]),
            ),
            (
                "providers",
                seq(&[module_entry_with_config(
                    "provider-a",
                    &[("api_key", str_val("key-a"))],
                )]),
            ),
            (
                "tools",
                seq(&[module_entry_with_config(
                    "tool-a",
                    &[("setting", bool_val(true))],
                )]),
            ),
            ("hooks", seq(&[module_entry("hook-a")])),
            ("includes", seq(&[str_val("other-bundle")])),
        ]),
    )]);

    let bundle = Bundle::from_dict(&data).expect("should parse full bundle");

    assert_eq!(bundle.name, "full-test");
    assert_eq!(bundle.version, "2.0.0");
    assert_eq!(bundle.description, "A full test bundle");

    // Session should be preserved
    let session_map = bundle
        .session
        .as_mapping()
        .expect("session should be mapping");
    assert_eq!(
        session_map.get(&str_val("orchestrator")),
        Some(&str_val("custom-orchestrator"))
    );
    let ctx = session_map
        .get(&str_val("context"))
        .expect("session.context should exist");
    let ctx_map = ctx.as_mapping().expect("context should be mapping");
    assert_eq!(ctx_map.get(&str_val("key")), Some(&str_val("value")));

    // Providers
    assert_eq!(bundle.providers.len(), 1);
    let prov = bundle.providers[0]
        .as_mapping()
        .expect("provider should be mapping");
    assert_eq!(prov.get(&str_val("module")), Some(&str_val("provider-a")));

    // Tools
    assert_eq!(bundle.tools.len(), 1);
    let tool = bundle.tools[0]
        .as_mapping()
        .expect("tool should be mapping");
    assert_eq!(tool.get(&str_val("module")), Some(&str_val("tool-a")));

    // Hooks
    assert_eq!(bundle.hooks.len(), 1);
    let hook = bundle.hooks[0]
        .as_mapping()
        .expect("hook should be mapping");
    assert_eq!(hook.get(&str_val("module")), Some(&str_val("hook-a")));

    // Includes
    assert_eq!(bundle.includes.len(), 1);
    assert_eq!(bundle.includes[0], str_val("other-bundle"));
}

// =====================================================================
// TestBundleCompose
// =====================================================================

#[test]

fn test_compose_empty_bundles() {
    let base = Bundle::new("base");
    let child = Bundle::new("child");
    let result = base.compose(&[&child]);

    // Last bundle's name wins
    assert_eq!(result.name, "child");
    assert!(result.providers.is_empty());
}

#[test]

fn test_compose_session_deep_merge() {
    // Base has session with orchestrator + context
    let mut base = Bundle::new("base");
    base.session = mapping(&[
        ("orchestrator", str_val("base-orchestrator")),
        ("context", mapping(&[("key", str_val("base-value"))])),
    ]);

    // Child overrides orchestrator only
    let mut child = Bundle::new("child");
    child.session = mapping(&[("orchestrator", str_val("child-orchestrator"))]);

    let result = base.compose(&[&child]);

    let session_map = result
        .session
        .as_mapping()
        .expect("result session should be mapping");

    // Child's orchestrator wins
    assert_eq!(
        session_map.get(&str_val("orchestrator")),
        Some(&str_val("child-orchestrator"))
    );
    // Base's context preserved
    let ctx = session_map
        .get(&str_val("context"))
        .expect("context should survive merge");
    let ctx_map = ctx.as_mapping().expect("context should be mapping");
    assert_eq!(ctx_map.get(&str_val("key")), Some(&str_val("base-value")));
}

#[test]

fn test_compose_providers_merge_by_module() {
    // Same module "provider-a" in both; configs deep-merged
    let mut base = Bundle::new("base");
    base.providers = vec![module_entry_with_config(
        "provider-a",
        &[("x", str_val("1")), ("y", str_val("2"))],
    )];

    let mut child = Bundle::new("child");
    child.providers = vec![module_entry_with_config(
        "provider-a",
        &[("y", str_val("3")), ("z", str_val("4"))],
    )];

    let result = base.compose(&[&child]);

    // Should have 1 provider (merged by module name)
    assert_eq!(result.providers.len(), 1);

    let prov = result.providers[0]
        .as_mapping()
        .expect("provider should be mapping");
    assert_eq!(prov.get(&str_val("module")), Some(&str_val("provider-a")));

    let config = prov
        .get(&str_val("config"))
        .expect("config should exist")
        .as_mapping()
        .expect("config should be mapping");
    // x from base, y overridden by child, z from child
    assert_eq!(config.get(&str_val("x")), Some(&str_val("1")));
    assert_eq!(config.get(&str_val("y")), Some(&str_val("3")));
    assert_eq!(config.get(&str_val("z")), Some(&str_val("4")));
}

#[test]

fn test_compose_multiple_bundles() {
    let mut base = Bundle::new("base");
    base.providers = vec![module_entry("provider-base")];

    let mut mid = Bundle::new("mid");
    mid.providers = vec![module_entry("provider-mid")];

    let mut top = Bundle::new("top");
    top.providers = vec![module_entry("provider-top")];

    let result = base.compose(&[&mid, &top]);

    // All three modules present
    assert_eq!(result.providers.len(), 3);

    let modules: Vec<&str> = result
        .providers
        .iter()
        .map(|p| {
            p.as_mapping()
                .unwrap()
                .get(&str_val("module"))
                .unwrap()
                .as_str()
                .unwrap()
        })
        .collect();
    assert!(modules.contains(&"provider-base"));
    assert!(modules.contains(&"provider-mid"));
    assert!(modules.contains(&"provider-top"));
}

#[test]

fn test_compose_instruction_replaced() {
    let mut base = Bundle::new("base");
    base.instruction = Some("base instruction".to_string());

    let mut child = Bundle::new("child");
    child.instruction = Some("child instruction".to_string());

    let result = base.compose(&[&child]);

    // Later instruction replaces earlier
    assert_eq!(result.instruction, Some("child instruction".to_string()));
}

// =====================================================================
// TestBundleToMountPlan
// =====================================================================

#[test]

fn test_minimal_mount_plan() {
    let bundle = Bundle::new("empty");
    let plan = bundle.to_mount_plan();

    // Empty bundle produces an empty mapping
    let plan_map = plan.as_mapping().expect("plan should be a mapping");
    assert!(plan_map.is_empty());
}

#[test]

fn test_full_mount_plan() {
    let mut bundle = Bundle::new("full");
    bundle.session = mapping(&[("orchestrator", str_val("default"))]);
    bundle.providers = vec![module_entry("provider-a")];
    bundle.tools = vec![module_entry("tool-a")];
    bundle.hooks = vec![module_entry("hook-a")];
    bundle.agents.insert(
        "agent-1".to_string(),
        mapping(&[("role", str_val("helper"))]),
    );

    let plan = bundle.to_mount_plan();
    let plan_map = plan.as_mapping().expect("plan should be a mapping");

    assert!(
        plan_map.get(&str_val("session")).is_some(),
        "mount plan should have session"
    );
    assert!(
        plan_map.get(&str_val("providers")).is_some(),
        "mount plan should have providers"
    );
    assert!(
        plan_map.get(&str_val("tools")).is_some(),
        "mount plan should have tools"
    );
    assert!(
        plan_map.get(&str_val("hooks")).is_some(),
        "mount plan should have hooks"
    );
    assert!(
        plan_map.get(&str_val("agents")).is_some(),
        "mount plan should have agents"
    );
}

// =====================================================================
// TestBundleResolveContext
// =====================================================================

#[test]

fn test_resolve_registered_context() {
    let mut bundle = Bundle::new("ctx-test");
    bundle
        .context
        .insert("myfile".to_string(), PathBuf::from("/tmp/myfile.md"));

    let resolved = bundle.resolve_context_path("myfile");
    assert_eq!(resolved, Some(PathBuf::from("/tmp/myfile.md")));
}

#[test]

fn test_resolve_from_base_path() {
    let dir = tempdir().expect("failed to create tempdir");
    let context_dir = dir.path().join("context");
    fs::create_dir_all(&context_dir).expect("failed to create context dir");
    let test_file = context_dir.join("test.md");
    fs::write(&test_file, "# Test").expect("failed to write test file");

    let mut bundle = Bundle::new("ctx-test");
    bundle.base_path = Some(dir.path().to_path_buf());

    let resolved = bundle.resolve_context_path("context/test.md");
    assert!(resolved.is_some(), "should resolve relative to base_path");
    assert!(
        resolved.unwrap().exists(),
        "resolved path should point to existing file"
    );
}

#[test]

fn test_resolve_not_found() {
    let bundle = Bundle::new("empty");
    let resolved = bundle.resolve_context_path("unknown");
    assert_eq!(resolved, None);
}

// =====================================================================
// TestBundlePendingContext
// =====================================================================

#[test]

fn test_parse_context_defers_namespaced_refs() {
    // Context includes local and namespaced references.
    // Local entries go into context, namespaced into pending_context.
    let data = mapping(&[(
        "bundle",
        mapping(&[
            ("name", str_val("ns-test")),
            (
                "context",
                mapping(&[
                    ("local_file", str_val("context/readme.md")),
                    (
                        "other_ns:remote_file",
                        str_val("other_ns:context/remote.md"),
                    ),
                ]),
            ),
        ]),
    )]);

    let bundle = Bundle::from_dict(&data).expect("should parse bundle with context refs");

    // Local reference should be in context (as a path)
    assert!(
        bundle.context.contains_key("local_file")
            || bundle.context.contains_key("context/readme.md"),
        "local context entry should be in context map"
    );

    // Namespaced reference should be in pending_context
    assert!(
        !bundle.pending_context.is_empty(),
        "namespaced ref should be deferred to pending_context"
    );
}

#[test]

fn test_resolve_pending_context_with_source_base_paths() {
    let dir = tempdir().expect("failed to create tempdir");
    let context_dir = dir.path().join("context");
    fs::create_dir_all(&context_dir).expect("failed to create context dir");
    let remote_file = context_dir.join("remote.md");
    fs::write(&remote_file, "# Remote").expect("failed to write remote file");

    let mut bundle = Bundle::new("resolve-test");
    bundle.pending_context.insert(
        "other_ns:context/remote.md".to_string(),
        "other_ns:context/remote.md".to_string(),
    );
    bundle
        .source_base_paths
        .insert("other_ns".to_string(), dir.path().to_path_buf());

    bundle.resolve_pending_context();

    // After resolution, pending_context should be resolved into context
    assert!(
        bundle.pending_context.is_empty() || bundle.context.len() > 0,
        "pending context should be resolved"
    );
    // The resolved path should exist
    let has_resolved = bundle.context.values().any(|p| p.exists());
    assert!(
        has_resolved,
        "at least one context path should be resolved and exist"
    );
}

#[test]

fn test_resolve_pending_context_self_reference() {
    // Bundle name is "myns", pending_context has "myns:context/file.md"
    // Should resolve using own base_path.
    let dir = tempdir().expect("failed to create tempdir");
    let context_dir = dir.path().join("context");
    fs::create_dir_all(&context_dir).expect("failed to create context dir");
    let file = context_dir.join("file.md");
    fs::write(&file, "# Self").expect("failed to write file");

    let mut bundle = Bundle::new("myns");
    bundle.base_path = Some(dir.path().to_path_buf());
    bundle.pending_context.insert(
        "myns:context/file.md".to_string(),
        "myns:context/file.md".to_string(),
    );

    bundle.resolve_pending_context();

    // Self-reference should be resolved using base_path
    let has_resolved = bundle.context.values().any(|p| p.exists());
    assert!(
        has_resolved,
        "self-referencing pending context should resolve via base_path"
    );
}

#[test]

fn test_compose_merges_pending_context() {
    let mut base = Bundle::new("base");
    base.pending_context
        .insert("ns_a:file_a.md".to_string(), "ns_a:file_a.md".to_string());

    let mut child = Bundle::new("child");
    child
        .pending_context
        .insert("ns_b:file_b.md".to_string(), "ns_b:file_b.md".to_string());

    let result = base.compose(&[&child]);

    // Both pending_context entries should be present in result
    assert!(
        result.pending_context.contains_key("ns_a:file_a.md"),
        "base pending_context should survive compose"
    );
    assert!(
        result.pending_context.contains_key("ns_b:file_b.md"),
        "child pending_context should survive compose"
    );
}

#[test]

fn test_pending_context_resolved_after_compose() {
    let dir_a = tempdir().expect("failed to create tmpdir a");
    let file_a = dir_a.path().join("file_a.md");
    fs::write(&file_a, "# A").expect("failed to write file_a");

    let dir_b = tempdir().expect("failed to create tmpdir b");
    let file_b = dir_b.path().join("file_b.md");
    fs::write(&file_b, "# B").expect("failed to write file_b");

    let mut base = Bundle::new("base");
    base.pending_context
        .insert("ns_a:file_a.md".to_string(), "ns_a:file_a.md".to_string());

    let mut child = Bundle::new("child");
    child
        .pending_context
        .insert("ns_b:file_b.md".to_string(), "ns_b:file_b.md".to_string());

    let mut result = base.compose(&[&child]);

    // Provide source base paths for resolution
    result
        .source_base_paths
        .insert("ns_a".to_string(), dir_a.path().to_path_buf());
    result
        .source_base_paths
        .insert("ns_b".to_string(), dir_b.path().to_path_buf());

    result.resolve_pending_context();

    // Both should be resolved
    let resolved_paths: Vec<&PathBuf> = result.context.values().collect();
    assert!(
        resolved_paths.len() >= 2,
        "both pending contexts should be resolved after compose + resolve"
    );
    assert!(
        resolved_paths.iter().all(|p| p.exists()),
        "all resolved context paths should exist"
    );
}

// =====================================================================
// TestBundleValidation
// =====================================================================

#[test]

fn test_raises_on_string_tools() {
    // tools list contains bare strings instead of dicts
    let data = mapping(&[(
        "bundle",
        mapping(&[
            ("name", str_val("bad-tools")),
            (
                "tools",
                seq(&[str_val("m365_collab"), str_val("sharepoint")]),
            ),
        ]),
    )]);

    let result = Bundle::from_dict(&data);
    assert!(result.is_err(), "should reject string entries in tools");

    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("bad-tools") || err_msg.contains("tools"),
        "error should mention the bundle name or field: {err_msg}"
    );
    assert!(
        err_msg.contains("tools[0]") || err_msg.contains("tools"),
        "error should reference the position: {err_msg}"
    );
    assert!(
        err_msg.to_lowercase().contains("expected")
            && (err_msg.to_lowercase().contains("dict") || err_msg.to_lowercase().contains("map")),
        "error should say expected dict/map: {err_msg}"
    );
    assert!(
        err_msg.to_lowercase().contains("str"),
        "error should say got str: {err_msg}"
    );
    assert!(
        err_msg.contains("m365_collab"),
        "error should include the offending value: {err_msg}"
    );
    assert!(
        err_msg.contains("module"),
        "error should hint about correct format with 'module' key: {err_msg}"
    );
}

#[test]

fn test_raises_on_string_providers() {
    let data = mapping(&[(
        "bundle",
        mapping(&[
            ("name", str_val("bad-providers")),
            ("providers", seq(&[str_val("provider-anthropic")])),
        ]),
    )]);

    let result = Bundle::from_dict(&data);
    assert!(result.is_err(), "should reject string entries in providers");
}

#[test]

fn test_raises_on_string_hooks() {
    // First hook is valid dict, second is bare string
    let data = mapping(&[(
        "bundle",
        mapping(&[
            ("name", str_val("bad-hooks")),
            ("hooks", seq(&[module_entry("hook-a"), str_val("hook-b")])),
        ]),
    )]);

    let result = Bundle::from_dict(&data);
    assert!(result.is_err(), "should reject string entries in hooks");

    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("hooks[1]") || err_msg.contains("hooks"),
        "error should reference hooks position: {err_msg}"
    );
}

#[test]

fn test_error_uses_base_path_when_no_name() {
    // Bundle has no name, but has a base_path -- error should show the path
    let data = mapping(&[("bundle", mapping(&[("tools", seq(&[str_val("bad-tool")]))]))]);

    let result = Bundle::from_dict_with_base_path(&data, Path::new("/path/to/bundle"));
    assert!(result.is_err(), "should reject string entries in tools");

    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("/path/to/bundle"),
        "error should contain the base path when name is absent: {err_msg}"
    );
}

#[test]

fn test_error_shows_correct_format_example() {
    let data = mapping(&[(
        "bundle",
        mapping(&[
            ("name", str_val("fmt-test")),
            ("tools", seq(&[str_val("bad-tool")])),
        ]),
    )]);

    let result = Bundle::from_dict(&data);
    assert!(result.is_err());

    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("Correct format") || err_msg.contains("correct format"),
        "error should show correct format hint: {err_msg}"
    );
    assert!(
        err_msg.contains("module"),
        "format hint should mention 'module' key: {err_msg}"
    );
    assert!(
        err_msg.contains("source"),
        "format hint should mention 'source' key: {err_msg}"
    );
}

#[test]

fn test_valid_config_passes() {
    let data = mapping(&[(
        "bundle",
        mapping(&[
            ("name", str_val("valid-bundle")),
            (
                "providers",
                seq(&[module_entry_with_config(
                    "provider-a",
                    &[("api_key", str_val("key"))],
                )]),
            ),
            (
                "tools",
                seq(&[module_entry_with_config(
                    "tool-a",
                    &[("enabled", bool_val(true))],
                )]),
            ),
            ("hooks", seq(&[module_entry("hook-a")])),
        ]),
    )]);

    let result = Bundle::from_dict(&data);
    assert!(
        result.is_ok(),
        "valid config with proper dicts should succeed: {:?}",
        result.err()
    );
}

#[test]

fn test_empty_lists_pass() {
    let data = mapping(&[(
        "bundle",
        mapping(&[
            ("name", str_val("empty-lists")),
            ("providers", seq(&[])),
            ("tools", seq(&[])),
            ("hooks", seq(&[])),
        ]),
    )]);

    let result = Bundle::from_dict(&data);
    assert!(
        result.is_ok(),
        "empty lists should be accepted: {:?}",
        result.err()
    );

    let bundle = result.unwrap();
    assert!(bundle.providers.is_empty());
    assert!(bundle.tools.is_empty());
    assert!(bundle.hooks.is_empty());
}

#[test]

fn test_missing_lists_pass() {
    let data = minimal_bundle_dict("minimal");

    let result = Bundle::from_dict(&data);
    assert!(
        result.is_ok(),
        "missing lists should default to empty: {:?}",
        result.err()
    );

    let bundle = result.unwrap();
    assert_eq!(bundle.name, "minimal");
    assert!(bundle.providers.is_empty());
    assert!(bundle.tools.is_empty());
    assert!(bundle.hooks.is_empty());
}
