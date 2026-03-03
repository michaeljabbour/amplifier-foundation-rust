//! Tests for bundle module.
//!
//! Ported from Python test_bundle.py -- 26 tests across 6 groups.
//! All tests are Wave 3 (ignored until implementations land).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use serde_yaml_ng::{Mapping, Value};
use tempfile::tempdir;

use amplifier_foundation::bundle::module_resolver::{
    BundleModuleResolver, BundleModuleSource, ModuleActivate,
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
        session_map.get(str_val("orchestrator")),
        Some(&str_val("custom-orchestrator"))
    );
    let ctx = session_map
        .get(str_val("context"))
        .expect("session.context should exist");
    let ctx_map = ctx.as_mapping().expect("context should be mapping");
    assert_eq!(ctx_map.get(str_val("key")), Some(&str_val("value")));

    // Providers
    assert_eq!(bundle.providers.len(), 1);
    let prov = bundle.providers[0]
        .as_mapping()
        .expect("provider should be mapping");
    assert_eq!(prov.get(str_val("module")), Some(&str_val("provider-a")));

    // Tools
    assert_eq!(bundle.tools.len(), 1);
    let tool = bundle.tools[0]
        .as_mapping()
        .expect("tool should be mapping");
    assert_eq!(tool.get(str_val("module")), Some(&str_val("tool-a")));

    // Hooks
    assert_eq!(bundle.hooks.len(), 1);
    let hook = bundle.hooks[0]
        .as_mapping()
        .expect("hook should be mapping");
    assert_eq!(hook.get(str_val("module")), Some(&str_val("hook-a")));

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
        session_map.get(str_val("orchestrator")),
        Some(&str_val("child-orchestrator"))
    );
    // Base's context preserved
    let ctx = session_map
        .get(str_val("context"))
        .expect("context should survive merge");
    let ctx_map = ctx.as_mapping().expect("context should be mapping");
    assert_eq!(ctx_map.get(str_val("key")), Some(&str_val("base-value")));
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
    assert_eq!(prov.get(str_val("module")), Some(&str_val("provider-a")));

    let config = prov
        .get(str_val("config"))
        .expect("config should exist")
        .as_mapping()
        .expect("config should be mapping");
    // x from base, y overridden by child, z from child
    assert_eq!(config.get(str_val("x")), Some(&str_val("1")));
    assert_eq!(config.get(str_val("y")), Some(&str_val("3")));
    assert_eq!(config.get(str_val("z")), Some(&str_val("4")));
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
                .get(str_val("module"))
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
        plan_map.get(str_val("session")).is_some(),
        "mount plan should have session"
    );
    assert!(
        plan_map.get(str_val("providers")).is_some(),
        "mount plan should have providers"
    );
    assert!(
        plan_map.get(str_val("tools")).is_some(),
        "mount plan should have tools"
    );
    assert!(
        plan_map.get(str_val("hooks")).is_some(),
        "mount plan should have hooks"
    );
    assert!(
        plan_map.get(str_val("agents")).is_some(),
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
        bundle.pending_context.is_empty() || !bundle.context.is_empty(),
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

// ---------------------------------------------------------------------------
// BundleModuleSource
// ---------------------------------------------------------------------------

#[test]
fn test_bundle_module_source_resolve() {
    let path = PathBuf::from("/modules/tool-bash");
    let source = BundleModuleSource::new(path.clone());
    assert_eq!(source.resolve(), &path);
}

// ---------------------------------------------------------------------------
// BundleModuleResolver
// ---------------------------------------------------------------------------

#[test]
fn test_bundle_module_resolver_resolve_found() {
    let mut paths = HashMap::new();
    paths.insert("tool-bash".to_string(), PathBuf::from("/modules/tool-bash"));
    paths.insert("tool-web".to_string(), PathBuf::from("/modules/tool-web"));

    let resolver = BundleModuleResolver::new(paths, None);
    let source = resolver.resolve("tool-bash", None).unwrap();
    assert_eq!(source.resolve(), Path::new("/modules/tool-bash"));
}

#[test]
fn test_bundle_module_resolver_resolve_not_found() {
    let paths = HashMap::new();
    let resolver = BundleModuleResolver::new(paths, None);
    let result = resolver.resolve("nonexistent", None);
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("nonexistent"));
    assert!(err_msg.contains("not found"));
}

#[test]
fn test_bundle_module_resolver_resolve_error_lists_available() {
    let mut paths = HashMap::new();
    paths.insert("tool-bash".to_string(), PathBuf::from("/modules/tool-bash"));

    let resolver = BundleModuleResolver::new(paths, None);
    let result = resolver.resolve("missing-tool", None);
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    // Error should list available modules
    assert!(err_msg.contains("tool-bash"));
}

#[test]
fn test_bundle_module_resolver_get_module_source_found() {
    let mut paths = HashMap::new();
    paths.insert("tool-bash".to_string(), PathBuf::from("/modules/tool-bash"));

    let resolver = BundleModuleResolver::new(paths, None);
    let result = resolver.get_module_source("tool-bash");
    assert!(result.is_some());
    assert!(result.unwrap().contains("tool-bash"));
}

#[test]
fn test_bundle_module_resolver_get_module_source_not_found() {
    let paths = HashMap::new();
    let resolver = BundleModuleResolver::new(paths, None);
    assert_eq!(resolver.get_module_source("missing"), None);
}

// Mock activator for async_resolve tests
struct MockActivator {
    result_path: PathBuf,
}

#[async_trait]
impl ModuleActivate for MockActivator {
    async fn activate(
        &self,
        _module_name: &str,
        _source_uri: &str,
    ) -> amplifier_foundation::Result<PathBuf> {
        Ok(self.result_path.clone())
    }
}

struct FailingActivator;

#[async_trait]
impl ModuleActivate for FailingActivator {
    async fn activate(
        &self,
        module_name: &str,
        _source_uri: &str,
    ) -> amplifier_foundation::Result<PathBuf> {
        Err(amplifier_foundation::BundleError::LoadError {
            reason: format!("Failed to activate {}", module_name),
            source: None,
        })
    }
}

#[tokio::test]
async fn test_bundle_module_resolver_async_resolve_fast_path() {
    let mut paths = HashMap::new();
    paths.insert("tool-bash".to_string(), PathBuf::from("/modules/tool-bash"));

    let resolver = BundleModuleResolver::new(paths, None);
    let result = resolver.async_resolve("tool-bash", None).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap().resolve(), Path::new("/modules/tool-bash"));
}

#[tokio::test]
async fn test_bundle_module_resolver_async_resolve_lazy_activation() {
    let paths = HashMap::new();
    let activator = Arc::new(MockActivator {
        result_path: PathBuf::from("/activated/tool-new"),
    });

    let resolver = BundleModuleResolver::new(paths, Some(activator));
    let result = resolver
        .async_resolve("tool-new", Some("git+https://example.com/tool-new"))
        .await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap().resolve(), Path::new("/activated/tool-new"));

    // After activation, sync resolve should also work
    let sync_result = resolver.resolve("tool-new", None);
    assert!(sync_result.is_ok());
}

#[tokio::test]
async fn test_bundle_module_resolver_async_resolve_no_activator() {
    let paths = HashMap::new();
    let resolver = BundleModuleResolver::new(paths, None);
    let result = resolver
        .async_resolve("missing", Some("git+https://example.com"))
        .await;
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("no activator"));
}

#[tokio::test]
async fn test_bundle_module_resolver_async_resolve_no_hint() {
    let paths = HashMap::new();
    let activator = Arc::new(MockActivator {
        result_path: PathBuf::from("/activated/tool"),
    });
    let resolver = BundleModuleResolver::new(paths, Some(activator));
    let result = resolver.async_resolve("missing", None).await;
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("no source hint"));
}

#[tokio::test]
async fn test_bundle_module_resolver_async_resolve_activation_failure() {
    let paths = HashMap::new();
    let activator: Arc<dyn ModuleActivate> = Arc::new(FailingActivator);
    let resolver = BundleModuleResolver::new(paths, Some(activator));
    let result = resolver
        .async_resolve("failing-tool", Some("git+https://example.com"))
        .await;
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("activation failed"));
}

// Counting activator for concurrency tests
struct CountingActivator {
    call_count: Arc<AtomicUsize>,
    result_path: PathBuf,
}

#[async_trait]
impl ModuleActivate for CountingActivator {
    async fn activate(
        &self,
        _module_name: &str,
        _source_uri: &str,
    ) -> amplifier_foundation::Result<PathBuf> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        // Small delay to simulate real work and increase chance of contention
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        Ok(self.result_path.clone())
    }
}

#[tokio::test]
async fn test_bundle_module_resolver_concurrent_activation_deduplicates() {
    let call_count = Arc::new(AtomicUsize::new(0));
    let activator: Arc<dyn ModuleActivate> = Arc::new(CountingActivator {
        call_count: call_count.clone(),
        result_path: PathBuf::from("/activated/tool-x"),
    });

    let resolver = Arc::new(BundleModuleResolver::new(HashMap::new(), Some(activator)));

    // Spawn 10 tasks all requesting the same unactivated module
    let mut handles = vec![];
    for _ in 0..10 {
        let r = resolver.clone();
        handles.push(tokio::spawn(async move {
            r.async_resolve("tool-x", Some("git+https://example.com/tool-x"))
                .await
        }));
    }

    let results: Vec<_> = futures::future::join_all(handles).await;

    // All 10 tasks should succeed
    for result in &results {
        let inner = result.as_ref().unwrap();
        assert!(
            inner.is_ok(),
            "Expected Ok, got: {:?}",
            inner.as_ref().err()
        );
        assert_eq!(
            inner.as_ref().unwrap().resolve(),
            Path::new("/activated/tool-x")
        );
    }

    // Activation should have been called exactly ONCE (double-checked locking works)
    assert_eq!(
        call_count.load(Ordering::SeqCst),
        1,
        "Activation should be called exactly once, but was called {} times",
        call_count.load(Ordering::SeqCst)
    );
}

#[test]
fn test_bundle_module_resolver_debug() {
    let mut paths = HashMap::new();
    paths.insert("tool-bash".to_string(), PathBuf::from("/modules/tool-bash"));
    let resolver = BundleModuleResolver::new(paths, None);
    let debug_str = format!("{:?}", resolver);
    assert!(debug_str.contains("BundleModuleResolver"));
    assert!(debug_str.contains("tool-bash"));
}

// ---------------------------------------------------------------------------
// resolve_agent_path tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_resolve_agent_path_simple_name() {
    let dir = tempdir().unwrap();
    let agents_dir = dir.path().join("agents");
    fs::create_dir_all(&agents_dir).unwrap();
    fs::write(agents_dir.join("bug-hunter.md"), "# Bug Hunter").unwrap();

    let mut bundle = Bundle::new("test-bundle");
    bundle.base_path = Some(dir.path().to_path_buf());

    let result = bundle.resolve_agent_path("bug-hunter").await;
    assert!(result.is_some());
    assert_eq!(result.unwrap(), agents_dir.join("bug-hunter.md"));
}

#[tokio::test]
async fn test_resolve_agent_path_simple_not_found() {
    let dir = tempdir().unwrap();
    let agents_dir = dir.path().join("agents");
    fs::create_dir_all(&agents_dir).unwrap();

    let mut bundle = Bundle::new("test-bundle");
    bundle.base_path = Some(dir.path().to_path_buf());

    let result = bundle.resolve_agent_path("nonexistent").await;
    assert!(result.is_none());
}

#[tokio::test]
async fn test_resolve_agent_path_no_base_path() {
    let bundle = Bundle::new("test-bundle");
    let result = bundle.resolve_agent_path("bug-hunter").await;
    assert!(result.is_none());
}

#[tokio::test]
async fn test_resolve_agent_path_namespaced() {
    let dir = tempdir().unwrap();
    let foundation_agents = dir.path().join("agents");
    fs::create_dir_all(&foundation_agents).unwrap();
    fs::write(foundation_agents.join("explorer.md"), "# Explorer").unwrap();

    let mut bundle = Bundle::new("my-app");
    bundle
        .source_base_paths
        .insert("foundation".to_string(), dir.path().to_path_buf());

    let result = bundle.resolve_agent_path("foundation:explorer").await;
    assert!(result.is_some());
    assert_eq!(result.unwrap(), foundation_agents.join("explorer.md"));
}

#[tokio::test]
async fn test_resolve_agent_path_namespaced_not_found() {
    let dir = tempdir().unwrap();
    let agents_dir = dir.path().join("agents");
    fs::create_dir_all(&agents_dir).unwrap();

    let mut bundle = Bundle::new("my-app");
    bundle
        .source_base_paths
        .insert("foundation".to_string(), dir.path().to_path_buf());

    let result = bundle.resolve_agent_path("foundation:nonexistent").await;
    assert!(result.is_none());
}

#[tokio::test]
async fn test_resolve_agent_path_namespaced_self_fallback() {
    let dir = tempdir().unwrap();
    let agents_dir = dir.path().join("agents");
    fs::create_dir_all(&agents_dir).unwrap();
    fs::write(agents_dir.join("helper.md"), "# Helper").unwrap();

    let mut bundle = Bundle::new("my-app");
    bundle.base_path = Some(dir.path().to_path_buf());

    // Namespace matches bundle name -- should fall back to base_path
    let result = bundle.resolve_agent_path("my-app:helper").await;
    assert!(result.is_some());
    assert_eq!(result.unwrap(), agents_dir.join("helper.md"));
}

#[tokio::test]
async fn test_resolve_agent_path_namespaced_unknown_namespace() {
    let mut bundle = Bundle::new("my-app");
    bundle.base_path = Some(PathBuf::from("/some/path"));

    // Unknown namespace, not matching bundle name
    let result = bundle.resolve_agent_path("unknown:agent").await;
    assert!(result.is_none());
}

#[tokio::test]
async fn test_resolve_agent_path_source_base_paths_priority() {
    // source_base_paths should be checked before self-name fallback
    let sbp_dir = tempdir().unwrap();
    let sbp_agents = sbp_dir.path().join("agents");
    fs::create_dir_all(&sbp_agents).unwrap();
    fs::write(sbp_agents.join("agent.md"), "# SBP Agent").unwrap();

    let bp_dir = tempdir().unwrap();
    let bp_agents = bp_dir.path().join("agents");
    fs::create_dir_all(&bp_agents).unwrap();
    fs::write(bp_agents.join("agent.md"), "# BP Agent").unwrap();

    let mut bundle = Bundle::new("my-app");
    bundle.base_path = Some(bp_dir.path().to_path_buf());
    bundle
        .source_base_paths
        .insert("my-app".to_string(), sbp_dir.path().to_path_buf());

    // source_base_paths["my-app"] should win over base_path
    let result = bundle.resolve_agent_path("my-app:agent").await;
    assert!(result.is_some());
    assert_eq!(result.unwrap(), sbp_agents.join("agent.md"));
}

#[tokio::test]
async fn test_resolve_agent_path_sbp_miss_self_fallthrough() {
    // Scenario B: source_base_paths has the namespace, but file doesn't exist there.
    // Should fall through to base_path since namespace == self.name.
    let sbp_dir = tempdir().unwrap();
    let sbp_agents = sbp_dir.path().join("agents");
    fs::create_dir_all(&sbp_agents).unwrap();
    // NOTE: no agent.md in sbp_agents

    let bp_dir = tempdir().unwrap();
    let bp_agents = bp_dir.path().join("agents");
    fs::create_dir_all(&bp_agents).unwrap();
    fs::write(bp_agents.join("agent.md"), "# BP Agent").unwrap();

    let mut bundle = Bundle::new("my-app");
    bundle.base_path = Some(bp_dir.path().to_path_buf());
    bundle
        .source_base_paths
        .insert("my-app".to_string(), sbp_dir.path().to_path_buf());

    // SBP lookup finds namespace but file missing -> falls through to self.name check
    let result = bundle.resolve_agent_path("my-app:agent").await;
    assert!(result.is_some());
    assert_eq!(result.unwrap(), bp_agents.join("agent.md"));
}

#[tokio::test]
async fn test_resolve_agent_path_multiple_colons() {
    // "ns:sub:path" -> namespace="ns", simple_name="sub:path"
    // This matches Python's split(":", 1) behavior
    let dir = tempdir().unwrap();
    let agents_dir = dir.path().join("agents");
    fs::create_dir_all(&agents_dir).unwrap();
    // Can't create "sub:path.md" on all platforms, so test returns None
    let mut bundle = Bundle::new("my-app");
    bundle
        .source_base_paths
        .insert("ns".to_string(), dir.path().to_path_buf());

    let result = bundle.resolve_agent_path("ns:sub:path").await;
    // File "sub:path.md" doesn't exist, so None
    assert!(result.is_none());
}

// ---------------------------------------------------------------------------
// get_system_instruction tests
// ---------------------------------------------------------------------------

#[test]
fn test_get_system_instruction_none() {
    let bundle = Bundle::new("test");
    assert!(bundle.get_system_instruction().is_none());
}

#[test]
fn test_get_system_instruction_some() {
    let mut bundle = Bundle::new("test");
    bundle.instruction = Some("You are a helpful assistant.".to_string());
    assert_eq!(
        bundle.get_system_instruction(),
        Some("You are a helpful assistant.")
    );
}

// ---------------------------------------------------------------------------
// load_agent_metadata tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_load_agent_metadata_basic() {
    let dir = tempdir().unwrap();
    let agents_dir = dir.path().join("agents");
    fs::create_dir_all(&agents_dir).unwrap();

    fs::write(
        agents_dir.join("helper.md"),
        "---\nmeta:\n  name: helper\n  description: A helpful agent\n---\nYou help with things.\n",
    )
    .unwrap();

    let mut bundle = Bundle::new("test-bundle");
    bundle.base_path = Some(dir.path().to_path_buf());
    bundle
        .agents
        .insert("helper".to_string(), Value::Mapping(Mapping::new()));

    bundle.load_agent_metadata().await;

    let agent = bundle.agents.get("helper").unwrap();
    let agent_map = agent.as_mapping().unwrap();
    assert_eq!(
        agent_map
            .get(Value::String("description".to_string()))
            .and_then(|v| v.as_str()),
        Some("A helpful agent")
    );
    assert_eq!(
        agent_map
            .get(Value::String("instruction".to_string()))
            .and_then(|v| v.as_str()),
        Some("You help with things.")
    );
}

#[tokio::test]
async fn test_load_agent_metadata_fills_gaps_only() {
    let dir = tempdir().unwrap();
    let agents_dir = dir.path().join("agents");
    fs::create_dir_all(&agents_dir).unwrap();

    fs::write(
        agents_dir.join("agent.md"),
        "---\nmeta:\n  name: agent\n  description: From file\n---\nFile instruction.\n",
    )
    .unwrap();

    // Agent already has a description set inline
    let mut existing = Mapping::new();
    existing.insert(
        Value::String("description".to_string()),
        Value::String("Inline description".to_string()),
    );

    let mut bundle = Bundle::new("test-bundle");
    bundle.base_path = Some(dir.path().to_path_buf());
    bundle
        .agents
        .insert("agent".to_string(), Value::Mapping(existing));

    bundle.load_agent_metadata().await;

    let agent = bundle.agents.get("agent").unwrap();
    let agent_map = agent.as_mapping().unwrap();
    // Inline description should be preserved (not overridden by file)
    assert_eq!(
        agent_map
            .get(Value::String("description".to_string()))
            .and_then(|v| v.as_str()),
        Some("Inline description")
    );
    // But instruction should be filled from file since it wasn't set inline
    assert_eq!(
        agent_map
            .get(Value::String("instruction".to_string()))
            .and_then(|v| v.as_str()),
        Some("File instruction.")
    );
}

#[tokio::test]
async fn test_load_agent_metadata_no_agents() {
    let mut bundle = Bundle::new("test-bundle");
    // Should not panic with no agents
    bundle.load_agent_metadata().await;
    assert!(bundle.agents.is_empty());
}

#[tokio::test]
async fn test_load_agent_metadata_agent_file_not_found() {
    let dir = tempdir().unwrap();
    let agents_dir = dir.path().join("agents");
    fs::create_dir_all(&agents_dir).unwrap();
    // No agent .md file exists

    let mut bundle = Bundle::new("test-bundle");
    bundle.base_path = Some(dir.path().to_path_buf());
    bundle
        .agents
        .insert("missing".to_string(), Value::Mapping(Mapping::new()));

    // Should not panic, just skip the agent
    bundle.load_agent_metadata().await;

    let agent = bundle.agents.get("missing").unwrap();
    // Agent should remain unchanged (empty mapping)
    assert!(agent.as_mapping().unwrap().is_empty());
}

#[tokio::test]
async fn test_load_agent_metadata_flat_frontmatter() {
    // Some agents have flat frontmatter (name/description at top level, not under meta:)
    let dir = tempdir().unwrap();
    let agents_dir = dir.path().join("agents");
    fs::create_dir_all(&agents_dir).unwrap();

    fs::write(
        agents_dir.join("flat.md"),
        "---\nname: flat-agent\ndescription: Flat description\n---\nFlat instruction.\n",
    )
    .unwrap();

    let mut bundle = Bundle::new("test-bundle");
    bundle.base_path = Some(dir.path().to_path_buf());
    bundle
        .agents
        .insert("flat".to_string(), Value::Mapping(Mapping::new()));

    bundle.load_agent_metadata().await;

    let agent = bundle.agents.get("flat").unwrap();
    let agent_map = agent.as_mapping().unwrap();
    assert_eq!(
        agent_map
            .get(Value::String("description".to_string()))
            .and_then(|v| v.as_str()),
        Some("Flat description")
    );
}

#[tokio::test]
async fn test_load_agent_metadata_with_mount_plan_sections() {
    // Agents can define their own tools/providers/hooks/session
    let dir = tempdir().unwrap();
    let agents_dir = dir.path().join("agents");
    fs::create_dir_all(&agents_dir).unwrap();

    fs::write(
        agents_dir.join("tooled.md"),
        "---\nmeta:\n  name: tooled\n  description: Agent with tools\ntools:\n  - module: tool-bash\nproviders:\n  - module: provider-openai\n---\nTooled instruction.\n",
    )
    .unwrap();

    let mut bundle = Bundle::new("test-bundle");
    bundle.base_path = Some(dir.path().to_path_buf());
    bundle
        .agents
        .insert("tooled".to_string(), Value::Mapping(Mapping::new()));

    bundle.load_agent_metadata().await;

    let agent = bundle.agents.get("tooled").unwrap();
    let agent_map = agent.as_mapping().unwrap();
    assert!(agent_map.get(Value::String("tools".to_string())).is_some());
    assert!(agent_map
        .get(Value::String("providers".to_string()))
        .is_some());
}

#[tokio::test]
async fn test_load_agent_metadata_non_mapping_config_preserved() {
    // If agent_config is not a mapping (e.g., null or string), it should be
    // preserved as-is (matching Python where TypeError is caught)
    let dir = tempdir().unwrap();
    let agents_dir = dir.path().join("agents");
    fs::create_dir_all(&agents_dir).unwrap();
    fs::write(
        agents_dir.join("nullagent.md"),
        "---\nmeta:\n  name: nullagent\n  description: From file\n---\nInstruction.\n",
    )
    .unwrap();

    let mut bundle = Bundle::new("test-bundle");
    bundle.base_path = Some(dir.path().to_path_buf());
    bundle.agents.insert("nullagent".to_string(), Value::Null);

    bundle.load_agent_metadata().await;

    // Non-mapping agent should be preserved (not replaced by file metadata)
    assert!(bundle.agents.get("nullagent").unwrap().is_null());
}

#[tokio::test]
async fn test_load_agent_metadata_malformed_yaml() {
    // Malformed YAML frontmatter should be caught and logged, not panic
    let dir = tempdir().unwrap();
    let agents_dir = dir.path().join("agents");
    fs::create_dir_all(&agents_dir).unwrap();
    fs::write(
        agents_dir.join("broken.md"),
        "---\nmeta:\n  name: [unterminated\n---\nBody.\n",
    )
    .unwrap();

    let mut bundle = Bundle::new("test-bundle");
    bundle.base_path = Some(dir.path().to_path_buf());
    bundle
        .agents
        .insert("broken".to_string(), Value::Mapping(Mapping::new()));

    // Should not panic -- error is caught and logged as warning
    bundle.load_agent_metadata().await;

    // Agent should remain unchanged (empty mapping)
    assert!(bundle
        .agents
        .get("broken")
        .unwrap()
        .as_mapping()
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn test_load_agent_metadata_empty_string_overwritten() {
    // Empty string values should be considered falsy and overwritten by file metadata
    let dir = tempdir().unwrap();
    let agents_dir = dir.path().join("agents");
    fs::create_dir_all(&agents_dir).unwrap();
    fs::write(
        agents_dir.join("agent.md"),
        "---\nmeta:\n  name: agent\n  description: From file\n---\n",
    )
    .unwrap();

    let mut existing = Mapping::new();
    existing.insert(
        Value::String("description".to_string()),
        Value::String(String::new()), // empty string = falsy
    );

    let mut bundle = Bundle::new("test-bundle");
    bundle.base_path = Some(dir.path().to_path_buf());
    bundle
        .agents
        .insert("agent".to_string(), Value::Mapping(existing));

    bundle.load_agent_metadata().await;

    let agent = bundle.agents.get("agent").unwrap();
    let agent_map = agent.as_mapping().unwrap();
    // Empty string should be overwritten by file metadata
    assert_eq!(
        agent_map
            .get(Value::String("description".to_string()))
            .and_then(|v| v.as_str()),
        Some("From file")
    );
}

#[tokio::test]
async fn test_load_agent_metadata_no_frontmatter() {
    // Agent .md with no frontmatter -- just body
    let dir = tempdir().unwrap();
    let agents_dir = dir.path().join("agents");
    fs::create_dir_all(&agents_dir).unwrap();

    fs::write(agents_dir.join("plain.md"), "Just a plain instruction.\n").unwrap();

    let mut bundle = Bundle::new("test-bundle");
    bundle.base_path = Some(dir.path().to_path_buf());
    bundle
        .agents
        .insert("plain".to_string(), Value::Mapping(Mapping::new()));

    bundle.load_agent_metadata().await;

    let agent = bundle.agents.get("plain").unwrap();
    let agent_map = agent.as_mapping().unwrap();
    // Should have name (fallback) and instruction from body
    assert_eq!(
        agent_map
            .get(Value::String("name".to_string()))
            .and_then(|v| v.as_str()),
        Some("plain")
    );
    assert_eq!(
        agent_map
            .get(Value::String("instruction".to_string()))
            .and_then(|v| v.as_str()),
        Some("Just a plain instruction.")
    );
}

// ---------------------------------------------------------------------------
// PreparedBundle tests (F-061)
// ---------------------------------------------------------------------------

use amplifier_foundation::PreparedBundle;

#[test]
fn test_prepared_bundle_new() {
    let bundle = amplifier_foundation::Bundle::new("test-bundle");
    let resolver = BundleModuleResolver::new(HashMap::new(), None);
    let mount_plan = Value::Null;

    let prepared = PreparedBundle::new(mount_plan.clone(), resolver, bundle);
    assert_eq!(prepared.bundle.name, "test-bundle");
    assert!(prepared.bundle_package_paths.is_empty());
}

#[test]
fn test_build_bundles_for_resolver_from_source_base_paths() {
    let tmp = tempdir().unwrap();
    let ns_path = tmp.path().join("foundation");
    fs::create_dir_all(&ns_path).unwrap();

    let mut bundle = amplifier_foundation::Bundle::new("my-app");
    bundle
        .source_base_paths
        .insert("foundation".to_string(), ns_path.clone());
    bundle.base_path = Some(tmp.path().to_path_buf());

    let resolver = BundleModuleResolver::new(HashMap::new(), None);
    let prepared = PreparedBundle::new(Value::Null, resolver, bundle.clone());

    let bundles_map = prepared.build_bundles_for_resolver(&bundle);

    // Should contain "foundation" mapped to ns_path
    assert_eq!(bundles_map.get("foundation"), Some(&ns_path));
    // Should also contain "my-app" mapped to bundle.base_path
    assert_eq!(bundles_map.get("my-app"), Some(&tmp.path().to_path_buf()));
}

#[test]
fn test_build_bundles_for_resolver_bundle_name_included() {
    let tmp = tempdir().unwrap();

    let mut bundle = amplifier_foundation::Bundle::new("standalone");
    bundle.base_path = Some(tmp.path().to_path_buf());

    let resolver = BundleModuleResolver::new(HashMap::new(), None);
    let prepared = PreparedBundle::new(Value::Null, resolver, bundle.clone());

    let bundles_map = prepared.build_bundles_for_resolver(&bundle);

    // Bundle name should be included even without source_base_paths
    assert_eq!(
        bundles_map.get("standalone"),
        Some(&tmp.path().to_path_buf())
    );
}

#[test]
fn test_build_bundles_for_resolver_empty_name_skipped() {
    let mut bundle = amplifier_foundation::Bundle::new("");
    bundle.base_path = Some(PathBuf::from("/tmp/base"));

    let resolver = BundleModuleResolver::new(HashMap::new(), None);
    let prepared = PreparedBundle::new(Value::Null, resolver, bundle.clone());

    let bundles_map = prepared.build_bundles_for_resolver(&bundle);

    // Empty name should be skipped
    assert!(bundles_map.is_empty());
}

#[test]
fn test_build_bundles_for_resolver_namespace_already_present() {
    let tmp = tempdir().unwrap();
    let ns_path = tmp.path().join("ns");
    fs::create_dir_all(&ns_path).unwrap();

    let mut bundle = amplifier_foundation::Bundle::new("ns");
    bundle
        .source_base_paths
        .insert("ns".to_string(), ns_path.clone());
    bundle.base_path = Some(tmp.path().to_path_buf());

    let resolver = BundleModuleResolver::new(HashMap::new(), None);
    let prepared = PreparedBundle::new(Value::Null, resolver, bundle.clone());

    let bundles_map = prepared.build_bundles_for_resolver(&bundle);

    // "ns" in source_base_paths should win (not duplicated)
    assert_eq!(bundles_map.len(), 1);
    assert_eq!(bundles_map.get("ns"), Some(&ns_path));
}

#[tokio::test]
async fn test_create_system_prompt_factory_basic() {
    let tmp = tempdir().unwrap();

    // Create a context file
    let ctx_path = tmp.path().join("context.md");
    fs::write(&ctx_path, "Some context content").unwrap();

    let mut bundle = amplifier_foundation::Bundle::new("test");
    bundle.instruction = Some("Test instruction".to_string());
    bundle.context.insert("context".to_string(), ctx_path);
    bundle.base_path = Some(tmp.path().to_path_buf());

    let resolver = BundleModuleResolver::new(HashMap::new(), None);
    let prepared = PreparedBundle::new(Value::Null, resolver, bundle.clone());

    let factory = prepared.create_system_prompt_factory(&bundle, None);
    let prompt = factory.create().await;

    assert!(prompt.contains("Test instruction"));
    assert!(prompt.contains("Some context content"));
}

#[tokio::test]
async fn test_create_system_prompt_factory_no_context() {
    let tmp = tempdir().unwrap();

    let mut bundle = amplifier_foundation::Bundle::new("test");
    bundle.instruction = Some("Just an instruction".to_string());
    bundle.base_path = Some(tmp.path().to_path_buf());

    let resolver = BundleModuleResolver::new(HashMap::new(), None);
    let prepared = PreparedBundle::new(Value::Null, resolver, bundle.clone());

    let factory = prepared.create_system_prompt_factory(&bundle, None);
    let prompt = factory.create().await;

    assert_eq!(prompt, "Just an instruction");
}

#[tokio::test]
async fn test_create_system_prompt_factory_with_mentions() {
    let tmp = tempdir().unwrap();

    // Create a file that will be @mentioned
    let agents_path = tmp.path().join("AGENTS.md");
    fs::write(&agents_path, "Agent list here").unwrap();

    let mut bundle = amplifier_foundation::Bundle::new("test");
    bundle.instruction = Some("Hello @AGENTS.md world".to_string());
    bundle.base_path = Some(tmp.path().to_path_buf());

    let resolver = BundleModuleResolver::new(HashMap::new(), None);
    let prepared = PreparedBundle::new(Value::Null, resolver, bundle.clone());

    let factory = prepared.create_system_prompt_factory(&bundle, None);
    let prompt = factory.create().await;

    // Should contain the instruction and the resolved @mention content
    assert!(prompt.contains("Hello @AGENTS.md world"));
    assert!(prompt.contains("Agent list here"));
}

#[tokio::test]
async fn test_create_system_prompt_factory_rereads_files() {
    let tmp = tempdir().unwrap();

    let ctx_path = tmp.path().join("dynamic.md");
    fs::write(&ctx_path, "Version 1").unwrap();

    let mut bundle = amplifier_foundation::Bundle::new("test");
    bundle
        .context
        .insert("dynamic".to_string(), ctx_path.clone());
    bundle.base_path = Some(tmp.path().to_path_buf());

    let resolver = BundleModuleResolver::new(HashMap::new(), None);
    let prepared = PreparedBundle::new(Value::Null, resolver, bundle.clone());

    let factory = prepared.create_system_prompt_factory(&bundle, None);

    // First call
    let prompt1 = factory.create().await;
    assert!(prompt1.contains("Version 1"));

    // Update the file
    fs::write(&ctx_path, "Version 2").unwrap();

    // Second call should pick up the change
    let prompt2 = factory.create().await;
    assert!(prompt2.contains("Version 2"));
    assert!(!prompt2.contains("Version 1"));
}

// Test for enhanced namespace resolution in BaseMentionResolver
#[tokio::test]
async fn test_mention_resolver_namespace_resolution() {
    let tmp = tempdir().unwrap();
    let ns_path = tmp.path().join("foundation");
    fs::create_dir_all(&ns_path).unwrap();
    fs::write(ns_path.join("context.md"), "Foundation context").unwrap();

    let mut bundles = HashMap::new();
    bundles.insert("foundation".to_string(), ns_path);

    let resolver = amplifier_foundation::BaseMentionResolver {
        base_path: tmp.path().to_path_buf(),
        bundles,
        context: indexmap::IndexMap::new(),
    };

    // @foundation:context should resolve to foundation/context.md
    let resolved = resolver.resolve("@foundation:context").await;
    assert!(resolved.is_some());
    let path = resolved.unwrap();
    assert!(path.to_str().unwrap().contains("context.md"));
}

#[tokio::test]
async fn test_mention_resolver_namespace_not_found() {
    let resolver = amplifier_foundation::BaseMentionResolver {
        base_path: PathBuf::from("/tmp"),
        bundles: HashMap::new(),
        context: indexmap::IndexMap::new(),
    };

    // Unknown namespace should return None
    let resolved = resolver.resolve("@unknown:path").await;
    assert!(resolved.is_none());
}
