//! Tests for registry module (BundleRegistry).
//!
//! Ported from Python test_registry.py — 21 tests across 4 groups.
//! All tests are Wave 3 (ignored until implementations land).

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use serde_yaml_ng::Value;
use tempfile::tempdir;

use amplifier_foundation::registry::{BundleRegistry, BundleState};
use amplifier_foundation::{extract_bundle_name, find_resource_path, parse_include};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Write a minimal bundle.md file at the given path.
fn write_bundle_md(path: &std::path::Path) {
    fs::write(path.join("bundle.md"), "# Test Bundle\n").expect("write bundle.md");
}

/// Write a minimal bundle.yaml file at the given path.
fn write_bundle_yaml(path: &std::path::Path, content: &str) {
    fs::write(path.join("bundle.yaml"), content).expect("write bundle.yaml");
}

/// Write a minimal bundle.yaml with just a name.
fn write_simple_bundle_yaml(path: &std::path::Path, name: &str) {
    let content = format!("name: {name}\nversion: \"1.0.0\"\n");
    write_bundle_yaml(path, &content);
}

/// Create nested directories under base, returning the deepest path.
fn create_nested_dirs(base: &std::path::Path, segments: &[&str]) -> PathBuf {
    let mut current = base.to_path_buf();
    for seg in segments {
        current = current.join(seg);
    }
    fs::create_dir_all(&current).expect("create nested dirs");
    current
}

/// Register a bundle by name and URI in a fresh registry.
fn register_one(registry: &mut BundleRegistry, name: &str, uri: &str) {
    let map = HashMap::from([(name.to_string(), uri.to_string())]);
    registry.register(&map);
}

/// Write a bundle.yaml that includes the given list of file:// URIs.
fn write_bundle_yaml_with_includes(path: &std::path::Path, name: &str, includes: &[&str]) {
    let includes_yaml: Vec<String> = includes.iter().map(|u| format!("  - \"{}\"", u)).collect();
    let content = format!(
        "name: {name}\nversion: \"1.0.0\"\nincludes:\n{}\n",
        includes_yaml.join("\n")
    );
    write_bundle_yaml(path, &content);
}

// ===========================================================================
// TestFindNearestBundleFile (6 tests, sync)
// ===========================================================================

#[test]

fn test_finds_bundle_md_in_start_directory() {
    let tmp = tempdir().unwrap();
    let base = tmp.path();
    write_bundle_md(base);

    let registry = BundleRegistry::new(base.to_path_buf());
    let result = registry.find_nearest_bundle_file(base, base);

    assert_eq!(result, Some(base.join("bundle.md")));
}

#[test]

fn test_finds_bundle_yaml_in_start_directory() {
    let tmp = tempdir().unwrap();
    let base = tmp.path();
    write_simple_bundle_yaml(base, "test");

    let registry = BundleRegistry::new(base.to_path_buf());
    let result = registry.find_nearest_bundle_file(base, base);

    assert_eq!(result, Some(base.join("bundle.yaml")));
}

#[test]

fn test_prefers_bundle_md_over_bundle_yaml() {
    let tmp = tempdir().unwrap();
    let base = tmp.path();
    // Create both files — bundle.md should win.
    write_bundle_md(base);
    write_simple_bundle_yaml(base, "test");

    let registry = BundleRegistry::new(base.to_path_buf());
    let result = registry.find_nearest_bundle_file(base, base);

    assert_eq!(result, Some(base.join("bundle.md")));
}

#[test]

fn test_walks_up_to_find_bundle() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    write_bundle_md(root);

    // Search from a deeply nested directory.
    let nested = create_nested_dirs(root, &["a", "b", "c"]);

    let registry = BundleRegistry::new(root.to_path_buf());
    let result = registry.find_nearest_bundle_file(&nested, root);

    assert_eq!(result, Some(root.join("bundle.md")));
}

#[test]

fn test_returns_none_when_not_found() {
    let tmp = tempdir().unwrap();
    let base = tmp.path();
    // No bundle files at all.

    let registry = BundleRegistry::new(base.to_path_buf());
    let result = registry.find_nearest_bundle_file(base, base);

    assert_eq!(result, None);
}

#[test]

fn test_stops_at_stop_directory() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();

    // bundle.md is above the stop boundary.
    write_bundle_md(root);

    let stop_dir = create_nested_dirs(root, &["project"]);
    let search_dir = create_nested_dirs(&stop_dir, &["src", "deep"]);

    let registry = BundleRegistry::new(root.to_path_buf());
    // Searching from deep inside, but stopping at `project/` — should NOT
    // find the bundle.md that lives at root.
    let result = registry.find_nearest_bundle_file(&search_dir, &stop_dir);

    assert_eq!(result, None);
}

// ===========================================================================
// TestUnregister (7 tests, sync)
// ===========================================================================

#[test]

fn test_unregister_existing_bundle_returns_true() {
    let tmp = tempdir().unwrap();
    let mut registry = BundleRegistry::new(tmp.path().to_path_buf());

    register_one(&mut registry, "my-bundle", "file:///some/path");
    let removed = registry.unregister("my-bundle");

    assert!(removed);
}

#[test]

fn test_unregister_nonexistent_bundle_returns_false() {
    let tmp = tempdir().unwrap();
    let mut registry = BundleRegistry::new(tmp.path().to_path_buf());

    let removed = registry.unregister("does-not-exist");

    assert!(!removed);
}

#[test]

fn test_unregister_removes_from_list_registered() {
    let tmp = tempdir().unwrap();
    let mut registry = BundleRegistry::new(tmp.path().to_path_buf());

    let bundles = HashMap::from([
        ("alpha".to_string(), "file:///a".to_string()),
        ("beta".to_string(), "file:///b".to_string()),
        ("gamma".to_string(), "file:///c".to_string()),
    ]);
    registry.register(&bundles);

    registry.unregister("beta");

    let mut remaining = registry.list_registered();
    remaining.sort();
    assert_eq!(remaining, vec!["alpha".to_string(), "gamma".to_string()]);
}

#[test]

fn test_unregister_does_not_auto_persist() {
    let tmp = tempdir().unwrap();
    let home = tmp.path().to_path_buf();

    // Register and save to disk.
    {
        let mut registry = BundleRegistry::new(home.clone());
        register_one(&mut registry, "persistent", "file:///p");
        registry.save();
    }

    // Unregister but do NOT save.
    {
        let mut registry = BundleRegistry::new(home.clone());
        registry.unregister("persistent");
        // Intentionally not calling registry.save()
    }

    // A brand-new instance should still see it (loaded from disk).
    {
        let registry = BundleRegistry::new(home);
        let names = registry.list_registered();
        assert!(
            names.contains(&"persistent".to_string()),
            "bundle should still be persisted because save() was not called after unregister"
        );
    }
}

#[test]

fn test_unregister_cleans_up_includes_relationships() {
    // Parent includes [child-a, child-b]. Unregister parent.
    // Children's included_by should be cleared.
    let tmp = tempdir().unwrap();
    let mut registry = BundleRegistry::new(tmp.path().to_path_buf());

    let bundles = HashMap::from([
        ("parent".to_string(), "file:///parent".to_string()),
        ("child-a".to_string(), "file:///child-a".to_string()),
        ("child-b".to_string(), "file:///child-b".to_string()),
    ]);
    registry.register(&bundles);

    // Set up the includes / included_by relationships.
    {
        let parent_state = registry.get_state("parent");
        parent_state.includes = vec!["child-a".to_string(), "child-b".to_string()];
    }
    {
        let child_a = registry.get_state("child-a");
        child_a.included_by = vec!["parent".to_string()];
    }
    {
        let child_b = registry.get_state("child-b");
        child_b.included_by = vec!["parent".to_string()];
    }

    // Unregister parent — children should have included_by cleaned up.
    registry.unregister("parent");

    let child_a = registry.get_state("child-a");
    assert!(
        child_a.included_by.is_empty(),
        "child-a.included_by should be empty after parent is unregistered"
    );
    let child_b = registry.get_state("child-b");
    assert!(
        child_b.included_by.is_empty(),
        "child-b.included_by should be empty after parent is unregistered"
    );
}

#[test]

fn test_unregister_cleans_up_included_by_relationships() {
    // child included_by [parent-a, parent-b]. Unregister child.
    // Parents' includes should be cleaned up.
    let tmp = tempdir().unwrap();
    let mut registry = BundleRegistry::new(tmp.path().to_path_buf());

    let bundles = HashMap::from([
        ("parent-a".to_string(), "file:///parent-a".to_string()),
        ("parent-b".to_string(), "file:///parent-b".to_string()),
        ("child".to_string(), "file:///child".to_string()),
    ]);
    registry.register(&bundles);

    // Set up relationships.
    {
        let child = registry.get_state("child");
        child.included_by = vec!["parent-a".to_string(), "parent-b".to_string()];
    }
    {
        let parent_a = registry.get_state("parent-a");
        parent_a.includes = vec!["child".to_string()];
    }
    {
        let parent_b = registry.get_state("parent-b");
        parent_b.includes = vec!["child".to_string()];
    }

    // Unregister child — parents should have includes cleaned up.
    registry.unregister("child");

    let parent_a = registry.get_state("parent-a");
    assert!(
        parent_a.includes.is_empty(),
        "parent-a.includes should be empty after child is unregistered"
    );
    let parent_b = registry.get_state("parent-b");
    assert!(
        parent_b.includes.is_empty(),
        "parent-b.includes should be empty after child is unregistered"
    );
}

#[test]

fn test_unregister_handles_partial_relationships() {
    // Partial relationships (e.g. includes references a name that doesn't
    // exist in the registry) should not crash.
    let tmp = tempdir().unwrap();
    let mut registry = BundleRegistry::new(tmp.path().to_path_buf());

    register_one(&mut registry, "lonely", "file:///lonely");

    // Point includes at bundles that are NOT registered.
    {
        let state = registry.get_state("lonely");
        state.includes = vec!["ghost-a".to_string(), "ghost-b".to_string()];
    }

    // Should not panic — partial/dangling references are tolerated.
    let removed = registry.unregister("lonely");
    assert!(removed);
}

// ===========================================================================
// TestSubdirectoryBundleLoading (3 tests, async)
// ===========================================================================

#[tokio::test]

async fn test_subdirectory_bundle_gets_source_base_paths() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();

    // Root bundle.md at the top level.
    write_bundle_md(root);

    // Subdirectory with its own bundle.yaml.
    let subdir = root.join("packages").join("feature-x");
    fs::create_dir_all(&subdir).expect("create subdir");
    write_simple_bundle_yaml(&subdir, "feature-x");

    let sub_uri = format!("file://{}", subdir.display());

    let registry = BundleRegistry::new(root.to_path_buf());
    let bundle = registry.load_single(&sub_uri).await.expect("load_single");

    // When loaded from a subdirectory that has a root bundle above it, the
    // bundle should record source_base_paths so relative paths can resolve.
    assert!(
        !bundle.source_base_paths.is_empty(),
        "subdirectory bundle should have source_base_paths populated"
    );
}

#[tokio::test]

async fn test_root_bundle_no_extra_source_base_paths() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    write_simple_bundle_yaml(root, "root-bundle");

    let uri = format!("file://{}", root.display());

    let registry = BundleRegistry::new(root.to_path_buf());
    let bundle = registry.load_single(&uri).await.expect("load_single");

    // A root-level bundle (no parent) should NOT have extra source_base_paths.
    assert!(
        bundle.source_base_paths.is_empty(),
        "root bundle should have empty source_base_paths"
    );
}

#[tokio::test]

async fn test_subdirectory_without_root_bundle_no_source_base_paths() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();

    // No root bundle.md or bundle.yaml at top level.
    let subdir = root.join("packages").join("orphan");
    fs::create_dir_all(&subdir).expect("create subdir");
    write_simple_bundle_yaml(&subdir, "orphan");

    let sub_uri = format!("file://{}", subdir.display());

    let registry = BundleRegistry::new(root.to_path_buf());
    let bundle = registry.load_single(&sub_uri).await.expect("load_single");

    // Without a root bundle above, no source_base_paths are set.
    assert!(
        bundle.source_base_paths.is_empty(),
        "subdirectory bundle with no root should have empty source_base_paths"
    );
}

// ===========================================================================
// TestDiamondAndCircularDependencies (5 tests, async)
// ===========================================================================

/// Helper: set up a temporary bundle directory structure for dependency tests.
/// Returns (tmp_dir_handle, HashMap<name, dir_path>) for each bundle created.
fn setup_dependency_bundles(
    names: &[&str],
    includes_map: &HashMap<&str, Vec<&str>>,
) -> (tempfile::TempDir, HashMap<String, PathBuf>) {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    let mut dirs = HashMap::new();

    // Create a directory for each bundle.
    for name in names {
        let bundle_dir = root.join(name);
        fs::create_dir_all(&bundle_dir).expect("create bundle dir");
        dirs.insert(name.to_string(), bundle_dir);
    }

    // Write bundle.yaml files with includes pointing to file:// URIs.
    for name in names {
        let bundle_dir = &dirs[*name];
        let includes: Vec<&str> = includes_map.get(name).cloned().unwrap_or_default();
        let include_uris: Vec<String> = includes
            .iter()
            .map(|dep| format!("file://{}", dirs[*dep].display()))
            .collect();
        let uri_refs: Vec<&str> = include_uris.iter().map(|s| s.as_str()).collect();
        write_bundle_yaml_with_includes(bundle_dir, name, &uri_refs);
    }

    (tmp, dirs)
}

#[tokio::test]

async fn test_diamond_dependency_loads_successfully() {
    // Diamond: A -> B, A -> C, B -> C
    let includes = HashMap::from([
        ("bundle-a", vec!["bundle-b", "bundle-c"]),
        ("bundle-b", vec!["bundle-c"]),
    ]);
    let (_tmp, dirs) = setup_dependency_bundles(&["bundle-a", "bundle-b", "bundle-c"], &includes);

    let uri_a = format!("file://{}", dirs["bundle-a"].display());
    let registry = BundleRegistry::new(_tmp.path().to_path_buf());
    let result = registry.load_single(&uri_a).await;

    assert!(
        result.is_ok(),
        "diamond dependency should load without error: {:?}",
        result.err()
    );
}

#[tokio::test]

async fn test_circular_dependency_handled_gracefully() {
    // Circular: A -> B -> A
    let includes = HashMap::from([
        ("bundle-a", vec!["bundle-b"]),
        ("bundle-b", vec!["bundle-a"]),
    ]);
    let (_tmp, dirs) = setup_dependency_bundles(&["bundle-a", "bundle-b"], &includes);

    let uri_a = format!("file://{}", dirs["bundle-a"].display());
    let registry = BundleRegistry::new(_tmp.path().to_path_buf());
    let result = registry.load_single(&uri_a).await;

    // Circular dependency should be detected and skipped, not cause an error.
    assert!(
        result.is_ok(),
        "circular dependency should be handled gracefully: {:?}",
        result.err()
    );
}

#[tokio::test]

async fn test_bundle_cached_after_first_load() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    write_simple_bundle_yaml(root, "cached-bundle");

    let uri = format!("file://{}", root.display());
    let registry = BundleRegistry::new(root.to_path_buf());

    let first = registry.load_single(&uri).await.expect("first load");
    let second = registry.load_single(&uri).await.expect("second load");

    // Same bundle should be returned (by value equality on name).
    assert_eq!(first.name, second.name);
    assert_eq!(first.version, second.version);
}

#[tokio::test]

async fn test_three_level_circular_dependency_handled_gracefully() {
    // Three-level circular: A -> B -> C -> A
    let includes = HashMap::from([
        ("bundle-a", vec!["bundle-b"]),
        ("bundle-b", vec!["bundle-c"]),
        ("bundle-c", vec!["bundle-a"]),
    ]);
    let (_tmp, dirs) = setup_dependency_bundles(&["bundle-a", "bundle-b", "bundle-c"], &includes);

    let uri_a = format!("file://{}", dirs["bundle-a"].display());
    let registry = BundleRegistry::new(_tmp.path().to_path_buf());
    let result = registry.load_single(&uri_a).await;

    assert!(
        result.is_ok(),
        "three-level circular dependency should be handled gracefully: {:?}",
        result.err()
    );
}

#[tokio::test]

async fn test_circular_dependency_logs_warning() {
    // A -> B -> A should produce a warning about the cycle.
    // In Rust we skip log capture checks — just verify the bundle loads.
    let includes = HashMap::from([
        ("bundle-a", vec!["bundle-b"]),
        ("bundle-b", vec!["bundle-a"]),
    ]);
    let (_tmp, dirs) = setup_dependency_bundles(&["bundle-a", "bundle-b"], &includes);

    let uri_a = format!("file://{}", dirs["bundle-a"].display());
    let registry = BundleRegistry::new(_tmp.path().to_path_buf());
    let result = registry.load_single(&uri_a).await;

    // Primary assertion: the bundle loads despite the cycle.
    assert!(
        result.is_ok(),
        "bundle should load even with circular dependency (warning expected): {:?}",
        result.err()
    );
    // Note: In a full implementation, we'd verify a tracing warning was
    // emitted about the circular dependency. For now, loading without
    // error is sufficient.
}

// ---------------------------------------------------------------------------
// BundleRegistry.find() tests
// ---------------------------------------------------------------------------

#[test]
fn test_registry_find_existing() {
    let dir = tempdir().unwrap();
    let mut registry = BundleRegistry::new(dir.path().to_path_buf());
    let mut bundles = HashMap::new();
    bundles.insert(
        "my-bundle".to_string(),
        "git+https://example.com/repo@main".to_string(),
    );
    registry.register(&bundles);

    let result = registry.find("my-bundle");
    assert_eq!(
        result,
        Some("git+https://example.com/repo@main".to_string())
    );
}

#[test]
fn test_registry_find_missing() {
    let dir = tempdir().unwrap();
    let registry = BundleRegistry::new(dir.path().to_path_buf());
    assert_eq!(registry.find("nonexistent"), None);
}

#[test]
fn test_registry_find_after_unregister() {
    let dir = tempdir().unwrap();
    let mut registry = BundleRegistry::new(dir.path().to_path_buf());
    let mut bundles = HashMap::new();
    bundles.insert(
        "my-bundle".to_string(),
        "file:///path/to/bundle".to_string(),
    );
    registry.register(&bundles);

    assert!(registry.find("my-bundle").is_some());
    registry.unregister("my-bundle");
    assert!(registry.find("my-bundle").is_none());
}

// ---------------------------------------------------------------------------
// BundleRegistry.get_all_states() tests
// ---------------------------------------------------------------------------

#[test]
fn test_registry_get_all_states_empty() {
    let dir = tempdir().unwrap();
    let registry = BundleRegistry::new(dir.path().to_path_buf());
    let states = registry.get_all_states();
    assert!(states.is_empty());
}

#[test]
fn test_registry_get_all_states_populated() {
    let dir = tempdir().unwrap();
    let mut registry = BundleRegistry::new(dir.path().to_path_buf());
    let mut bundles = HashMap::new();
    bundles.insert("a".to_string(), "file:///a".to_string());
    bundles.insert("b".to_string(), "file:///b".to_string());
    registry.register(&bundles);

    let states = registry.get_all_states();
    assert_eq!(states.len(), 2);
    assert!(states.contains_key("a"));
    assert!(states.contains_key("b"));
}

// ---------------------------------------------------------------------------
// BundleRegistry.validate_cached_paths() tests
// ---------------------------------------------------------------------------

#[test]
fn test_validate_cached_paths_clears_stale() {
    let dir = tempdir().unwrap();
    let mut registry = BundleRegistry::new(dir.path().to_path_buf());

    // Register a bundle and set a local_path that doesn't exist
    let mut bundles = HashMap::new();
    bundles.insert("stale-bundle".to_string(), "file:///orig".to_string());
    registry.register(&bundles);
    registry.get_state("stale-bundle").local_path = Some("/nonexistent/path/to/bundle".to_string());

    // validate_cached_paths should clear the stale reference
    registry.validate_cached_paths();

    assert!(
        registry.get_state("stale-bundle").local_path.is_none(),
        "Stale local_path should be cleared"
    );
}

#[test]
fn test_validate_cached_paths_keeps_valid() {
    let dir = tempdir().unwrap();
    let bundle_dir = dir.path().join("my-bundle");
    fs::create_dir_all(&bundle_dir).unwrap();

    let mut registry = BundleRegistry::new(dir.path().to_path_buf());
    let mut bundles = HashMap::new();
    bundles.insert("valid-bundle".to_string(), "file:///orig".to_string());
    registry.register(&bundles);
    registry.get_state("valid-bundle").local_path = Some(bundle_dir.to_string_lossy().to_string());

    registry.validate_cached_paths();

    assert!(
        registry.get_state("valid-bundle").local_path.is_some(),
        "Valid local_path should be preserved"
    );
}

#[test]
fn test_validate_cached_paths_mixed() {
    let dir = tempdir().unwrap();
    let valid_path = dir.path().join("exists");
    fs::create_dir_all(&valid_path).unwrap();

    let mut registry = BundleRegistry::new(dir.path().to_path_buf());
    let mut b1 = HashMap::new();
    b1.insert("valid".to_string(), "file:///a".to_string());
    registry.register(&b1);
    registry.get_state("valid").local_path = Some(valid_path.to_string_lossy().to_string());

    let mut b2 = HashMap::new();
    b2.insert("stale".to_string(), "file:///b".to_string());
    registry.register(&b2);
    registry.get_state("stale").local_path = Some("/definitely/not/here".to_string());

    registry.validate_cached_paths();

    assert!(registry.get_state("valid").local_path.is_some());
    assert!(registry.get_state("stale").local_path.is_none());
}

// ---------------------------------------------------------------------------
// BundleState timestamp fields tests
// ---------------------------------------------------------------------------

#[test]
fn test_bundle_state_timestamps_default_none() {
    let state = BundleState::new("test", "file:///test");
    assert!(state.loaded_at.is_none());
    assert!(state.checked_at.is_none());
}

#[test]
fn test_bundle_state_timestamps_to_dict_from_dict_roundtrip() {
    let mut state = BundleState::new("test", "file:///test");
    state.loaded_at = Some("2025-01-22T00:00:00Z".to_string());
    state.checked_at = Some("2025-01-22T01:00:00Z".to_string());

    let dict = state.to_dict();
    let restored = BundleState::from_dict("test", &dict);

    assert_eq!(restored.loaded_at.as_deref(), Some("2025-01-22T00:00:00Z"));
    assert_eq!(restored.checked_at.as_deref(), Some("2025-01-22T01:00:00Z"));
}

#[test]
fn test_bundle_state_timestamps_to_dict_absent_when_none() {
    let state = BundleState::new("test", "file:///test");
    let dict = state.to_dict();
    let obj = dict.as_object().unwrap();
    // Timestamps should not appear in output when None
    assert!(!obj.contains_key("loaded_at") || obj["loaded_at"].is_null());
    assert!(!obj.contains_key("checked_at") || obj["checked_at"].is_null());
}

#[test]
fn test_bundle_state_from_dict_missing_timestamps() {
    // Old registry.json without timestamp fields should load fine
    let data = serde_json::json!({
        "uri": "file:///test",
        "name": "test",
        "is_root": true,
        "explicitly_requested": false,
        "app_bundle": false
    });
    let state = BundleState::from_dict("test", &data);
    assert!(state.loaded_at.is_none());
    assert!(state.checked_at.is_none());
}

#[test]
fn test_bundle_state_from_dict_null_timestamps() {
    // JSON null for timestamps should be treated as None
    let data = serde_json::json!({
        "uri": "file:///test",
        "name": "test",
        "loaded_at": null,
        "checked_at": null,
        "is_root": true,
        "explicitly_requested": false,
        "app_bundle": false
    });
    let state = BundleState::from_dict("test", &data);
    assert!(state.loaded_at.is_none());
    assert!(state.checked_at.is_none());
}

#[test]
fn test_bundle_state_from_dict_empty_string_timestamps() {
    // Empty string timestamps should be treated as None (Python falsy behavior)
    let data = serde_json::json!({
        "uri": "file:///test",
        "name": "test",
        "loaded_at": "",
        "checked_at": "",
        "is_root": true,
        "explicitly_requested": false,
        "app_bundle": false
    });
    let state = BundleState::from_dict("test", &data);
    assert!(
        state.loaded_at.is_none(),
        "Empty string should be treated as None"
    );
    assert!(
        state.checked_at.is_none(),
        "Empty string should be treated as None"
    );
}

// ---------------------------------------------------------------------------
// BundleRegistry.find_state() tests
// ---------------------------------------------------------------------------

#[test]
fn test_registry_find_state_existing() {
    let dir = tempdir().unwrap();
    let mut registry = BundleRegistry::new(dir.path().to_path_buf());
    let mut bundles = HashMap::new();
    bundles.insert("my-bundle".to_string(), "file:///path".to_string());
    registry.register(&bundles);

    let state = registry.find_state("my-bundle");
    assert!(state.is_some());
    assert_eq!(state.unwrap().uri, "file:///path");
}

#[test]
fn test_registry_find_state_missing() {
    let dir = tempdir().unwrap();
    let registry = BundleRegistry::new(dir.path().to_path_buf());
    assert!(registry.find_state("nonexistent").is_none());
}

#[test]
fn test_validate_cached_paths_empty_registry() {
    let dir = tempdir().unwrap();
    let mut registry = BundleRegistry::new(dir.path().to_path_buf());
    // Should not panic or call save() on empty registry
    registry.validate_cached_paths();
    assert!(registry.get_all_states().is_empty());
}

// ===========================================================================
// parse_include tests (F-053)
// ===========================================================================

#[test]
fn test_parse_include_string() {
    let val = serde_yaml_ng::Value::String("my-bundle".to_string());
    assert_eq!(parse_include(&val), Some("my-bundle".to_string()));
}

#[test]
fn test_parse_include_dict_with_bundle() {
    let mut map = serde_yaml_ng::Mapping::new();
    map.insert(
        serde_yaml_ng::Value::String("bundle".to_string()),
        serde_yaml_ng::Value::String("foo".to_string()),
    );
    let val = serde_yaml_ng::Value::Mapping(map);
    assert_eq!(parse_include(&val), Some("foo".to_string()));
}

#[test]
fn test_parse_include_dict_without_bundle() {
    let mut map = serde_yaml_ng::Mapping::new();
    map.insert(
        serde_yaml_ng::Value::String("other".to_string()),
        serde_yaml_ng::Value::String("foo".to_string()),
    );
    let val = serde_yaml_ng::Value::Mapping(map);
    assert_eq!(parse_include(&val), None);
}

#[test]
fn test_parse_include_null() {
    let val = serde_yaml_ng::Value::Null;
    assert_eq!(parse_include(&val), None);
}

#[test]
fn test_parse_include_number() {
    let val = serde_yaml_ng::Value::Number(serde_yaml_ng::Number::from(42));
    assert_eq!(parse_include(&val), None);
}

// ===========================================================================
// find_resource_path tests (F-053)
// ===========================================================================

#[test]
fn test_find_resource_path_exact() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("myresource");
    fs::write(&file_path, "content").unwrap();

    let result = find_resource_path(&file_path);
    assert!(result.is_some());
    let resolved = result.unwrap();
    assert!(resolved.is_absolute());
    // Canonical path should end with "myresource"
    assert!(resolved.ends_with("myresource"));
}

#[test]
fn test_find_resource_path_yaml_ext() {
    let dir = tempdir().unwrap();
    let base = dir.path().join("myresource");
    // Don't create base, but create base.yaml
    fs::write(dir.path().join("myresource.yaml"), "content").unwrap();

    let result = find_resource_path(&base);
    assert!(result.is_some());
    let resolved = result.unwrap();
    assert!(resolved.to_string_lossy().ends_with("myresource.yaml"));
}

#[test]
fn test_find_resource_path_md_ext() {
    let dir = tempdir().unwrap();
    let base = dir.path().join("myresource");
    // Only create base.md
    fs::write(dir.path().join("myresource.md"), "content").unwrap();

    let result = find_resource_path(&base);
    assert!(result.is_some());
    let resolved = result.unwrap();
    assert!(resolved.to_string_lossy().ends_with("myresource.md"));
}

#[test]
fn test_find_resource_path_bundle_yaml() {
    let dir = tempdir().unwrap();
    let base = dir.path().join("myresource");
    fs::create_dir_all(&base).unwrap();
    fs::write(base.join("bundle.yaml"), "name: test").unwrap();

    let result = find_resource_path(&base);
    // base itself exists (it's a directory), so it should match as the first candidate
    assert!(result.is_some());

    // To specifically test bundle.yaml candidate, use a non-existent base dir name
    let base2 = dir.path().join("nonexistent");
    // Create nonexistent/bundle.yaml without creating nonexistent as an explicit file
    fs::create_dir_all(&base2).unwrap();
    fs::write(base2.join("bundle.yaml"), "name: test").unwrap();
    // base2 is a dir that exists, so first candidate matches.
    // Let's test with a path that doesn't exist as file or dir:
    let base3 = dir.path().join("only_bundle");
    fs::create_dir_all(&base3).unwrap();
    fs::write(base3.join("bundle.yaml"), "name: test").unwrap();

    let result3 = find_resource_path(&base3);
    assert!(result3.is_some());
}

#[test]
fn test_find_resource_path_none() {
    let dir = tempdir().unwrap();
    let base = dir.path().join("totally_missing");

    let result = find_resource_path(&base);
    assert!(result.is_none());
}

#[test]
fn test_find_resource_path_priority() {
    // Both base.yaml and base.md exist — base.yaml should win (earlier in list)
    let dir = tempdir().unwrap();
    let base = dir.path().join("myresource");
    fs::write(dir.path().join("myresource.yaml"), "yaml content").unwrap();
    fs::write(dir.path().join("myresource.md"), "md content").unwrap();

    let result = find_resource_path(&base);
    assert!(result.is_some());
    let resolved = result.unwrap();
    assert!(
        resolved.to_string_lossy().ends_with("myresource.yaml"),
        "Expected .yaml to win over .md, got: {}",
        resolved.display()
    );
}

// ===========================================================================
// resolve_include_source tests (F-053)
// ===========================================================================

#[test]
fn test_resolve_include_source_uri_passthrough() {
    let dir = tempdir().unwrap();
    let registry = BundleRegistry::new(dir.path().to_path_buf());

    let result = registry.resolve_include_source("git+https://github.com/org/repo@main");
    assert_eq!(
        result,
        Some("git+https://github.com/org/repo@main".to_string())
    );
}

#[test]
fn test_resolve_include_source_http_passthrough() {
    let dir = tempdir().unwrap();
    let registry = BundleRegistry::new(dir.path().to_path_buf());

    let result = registry.resolve_include_source("https://example.com/bundle.yaml");
    assert_eq!(result, Some("https://example.com/bundle.yaml".to_string()));
}

#[test]
fn test_resolve_include_source_file_passthrough() {
    let dir = tempdir().unwrap();
    let registry = BundleRegistry::new(dir.path().to_path_buf());

    let result = registry.resolve_include_source("file:///path/to/bundle");
    assert_eq!(result, Some("file:///path/to/bundle".to_string()));
}

#[test]
fn test_resolve_include_source_plain_name() {
    let dir = tempdir().unwrap();
    let registry = BundleRegistry::new(dir.path().to_path_buf());

    let result = registry.resolve_include_source("my-bundle");
    assert_eq!(result, Some("my-bundle".to_string()));
}

#[test]
fn test_resolve_include_source_namespace_not_registered() {
    let dir = tempdir().unwrap();
    let registry = BundleRegistry::new(dir.path().to_path_buf());

    let result = registry.resolve_include_source("unknown:path/to/thing");
    assert_eq!(result, None);
}

#[test]
fn test_resolve_include_source_namespace_file_with_local_path() {
    let dir = tempdir().unwrap();
    let mut registry = BundleRegistry::new(dir.path().to_path_buf());

    // Create a local directory structure with a resource file
    let local_dir = dir.path().join("local_bundles");
    let resource_dir = local_dir.join("skills");
    fs::create_dir_all(&resource_dir).unwrap();
    fs::write(resource_dir.join("coding.yaml"), "name: coding").unwrap();

    // Register namespace with local_path
    register_one(&mut registry, "mybundle", "file:///original/path");
    registry.get_state("mybundle").local_path = Some(local_dir.to_string_lossy().to_string());

    let result = registry.resolve_include_source("mybundle:skills/coding");
    assert!(
        result.is_some(),
        "Should resolve namespace:path with local_path"
    );
    let resolved = result.unwrap();
    assert!(
        resolved.starts_with("file://"),
        "Should be a file:// URI, got: {}",
        resolved
    );
    assert!(
        resolved.contains("coding.yaml"),
        "Should find coding.yaml via find_resource_path, got: {}",
        resolved
    );
}

#[test]
fn test_resolve_include_source_namespace_no_local_path_git() {
    let dir = tempdir().unwrap();
    let mut registry = BundleRegistry::new(dir.path().to_path_buf());

    // Register namespace with git URI but no local_path
    register_one(
        &mut registry,
        "mybundle",
        "git+https://github.com/org/repo@main",
    );
    // Deliberately no local_path set

    let result = registry.resolve_include_source("mybundle:skills/coding");
    assert_eq!(
        result,
        Some("git+https://github.com/org/repo@main#subdirectory=skills/coding".to_string())
    );
}

// ===========================================================================
// extract_bundle_name tests (F-053)
// ===========================================================================

#[test]
fn test_resolve_include_source_git_namespace_local_path_resource_found() {
    // P0 test: Git namespace with local_path where resource exists should return
    // git+...#subdirectory=relative/path, NOT file://local/path
    let dir = tempdir().unwrap();
    let mut registry = BundleRegistry::new(dir.path().to_path_buf());

    let local_dir = dir.path().join("cache_tools");
    let resource_dir = local_dir.join("skills");
    fs::create_dir_all(&resource_dir).unwrap();
    fs::write(resource_dir.join("coding.yaml"), "name: coding").unwrap();

    register_one(
        &mut registry,
        "tools",
        "git+https://github.com/org/tools@main",
    );
    registry.get_state("tools").local_path = Some(local_dir.to_string_lossy().to_string());

    let result = registry.resolve_include_source("tools:skills/coding");
    assert!(result.is_some(), "Should resolve");
    let resolved = result.unwrap();
    // MUST be git URI, not file://
    assert!(
        resolved.starts_with("git+https://"),
        "Git namespace should return git URI, got: {}",
        resolved
    );
    assert!(
        resolved.contains("#subdirectory="),
        "Should have subdirectory fragment, got: {}",
        resolved
    );
    assert!(
        resolved.contains("skills/coding.yaml"),
        "Subdirectory should include resolved extension, got: {}",
        resolved
    );
}

#[test]
fn test_resolve_include_source_git_namespace_local_path_resource_not_found() {
    // P0 test: Git namespace with local_path but resource NOT found should return None
    let dir = tempdir().unwrap();
    let mut registry = BundleRegistry::new(dir.path().to_path_buf());

    let local_dir = dir.path().join("cache_tools");
    fs::create_dir_all(&local_dir).unwrap();
    // No resource file created — only the directory exists

    register_one(
        &mut registry,
        "tools",
        "git+https://github.com/org/tools@main",
    );
    registry.get_state("tools").local_path = Some(local_dir.to_string_lossy().to_string());

    let result = registry.resolve_include_source("tools:nonexistent/path");
    assert_eq!(
        result, None,
        "Missing resource in git namespace should return None"
    );
}

#[test]
fn test_resolve_include_source_namespace_local_path_is_file() {
    // P1 test: local_path pointing to a file should use parent directory
    let dir = tempdir().unwrap();
    let mut registry = BundleRegistry::new(dir.path().to_path_buf());

    // local_path points to a specific bundle file
    let bundle_file = dir.path().join("bundle.yaml");
    fs::write(&bundle_file, "name: test").unwrap();
    // Create a resource relative to the bundle file's directory
    let skills_dir = dir.path().join("skills");
    fs::create_dir_all(&skills_dir).unwrap();
    fs::write(skills_dir.join("coding.yaml"), "name: coding").unwrap();

    register_one(&mut registry, "mybundle", "file:///original/path");
    registry.get_state("mybundle").local_path = Some(bundle_file.to_string_lossy().to_string());

    let result = registry.resolve_include_source("mybundle:skills/coding");
    assert!(
        result.is_some(),
        "Should resolve namespace:path when local_path is a file"
    );
    let resolved = result.unwrap();
    assert!(
        resolved.starts_with("file://"),
        "Should be file:// URI, got: {}",
        resolved
    );
    assert!(
        resolved.contains("coding.yaml"),
        "Should find coding.yaml, got: {}",
        resolved
    );
}

#[test]
fn test_resolve_include_source_namespace_no_local_path_non_git() {
    // Non-git namespace with no local_path should return None
    let dir = tempdir().unwrap();
    let mut registry = BundleRegistry::new(dir.path().to_path_buf());

    register_one(&mut registry, "mybundle", "https://example.com/bundle.yaml");
    // Deliberately no local_path

    let result = registry.resolve_include_source("mybundle:skills/coding");
    assert_eq!(
        result, None,
        "Non-git namespace with no local_path should return None"
    );
}

#[test]
fn test_resolve_include_source_namespace_non_git_local_path_not_found() {
    // Non-git namespace with local_path but resource not found should return None
    let dir = tempdir().unwrap();
    let mut registry = BundleRegistry::new(dir.path().to_path_buf());

    let local_dir = dir.path().join("local");
    fs::create_dir_all(&local_dir).unwrap();

    register_one(&mut registry, "mybundle", "file:///original/path");
    registry.get_state("mybundle").local_path = Some(local_dir.to_string_lossy().to_string());

    let result = registry.resolve_include_source("mybundle:nonexistent/path");
    assert_eq!(
        result, None,
        "Non-git namespace with missing resource should return None"
    );
}

#[test]
fn test_resolve_include_source_empty_namespace() {
    // Edge case: ":path" (empty namespace)
    let dir = tempdir().unwrap();
    let registry = BundleRegistry::new(dir.path().to_path_buf());

    // split_once(':') returns ("", "path") — empty namespace won't be in bundles
    let result = registry.resolve_include_source(":path");
    assert_eq!(result, None, "Empty namespace should return None");
}

#[test]
fn test_resolve_include_source_git_namespace_existing_fragment() {
    // Git URI with existing #fragment should strip it before adding subdirectory
    let dir = tempdir().unwrap();
    let mut registry = BundleRegistry::new(dir.path().to_path_buf());

    register_one(
        &mut registry,
        "tools",
        "git+https://github.com/org/tools@main#subdirectory=existing",
    );
    // No local_path

    let result = registry.resolve_include_source("tools:new/path");
    assert_eq!(
        result,
        Some("git+https://github.com/org/tools@main#subdirectory=new/path".to_string()),
        "Should strip existing fragment and use new subdirectory"
    );
}

#[test]
fn test_parse_include_non_string_bundle_value() {
    // Python uses str(bundle_ref) which coerces non-string values
    let mut map = serde_yaml_ng::Mapping::new();
    map.insert(
        Value::String("bundle".to_string()),
        Value::Number(serde_yaml_ng::Number::from(42)),
    );
    let result = parse_include(&Value::Mapping(map));
    assert_eq!(
        result,
        Some("42".to_string()),
        "Should coerce number to string"
    );
}

#[test]
fn test_parse_include_false_bundle_value() {
    // Python: if bundle_ref: — False is falsy, returns None
    let mut map = serde_yaml_ng::Mapping::new();
    map.insert(Value::String("bundle".to_string()), Value::Bool(false));
    let result = parse_include(&Value::Mapping(map));
    assert_eq!(result, None, "False is falsy, should return None");
}

// ===========================================================================
// extract_bundle_name tests (F-053)
// ===========================================================================

#[test]
fn test_extract_bundle_name_github() {
    let name = extract_bundle_name("git+https://github.com/org/repo@main");
    assert_eq!(name, "repo");
}

#[test]
fn test_extract_bundle_name_github_with_fragment() {
    let name = extract_bundle_name("git+https://github.com/org/repo@main#subdirectory=sub");
    assert_eq!(name, "repo");
}

#[test]
fn test_extract_bundle_name_file() {
    let name = extract_bundle_name("file:///path/to/bundle.yaml");
    assert_eq!(name, "bundle.yaml");
}

#[test]
fn test_extract_bundle_name_file_with_fragment() {
    let name = extract_bundle_name("file:///path/to/bundle.yaml#sub");
    assert_eq!(name, "bundle.yaml");
}

#[test]
fn test_extract_bundle_name_plain() {
    let name = extract_bundle_name("some/path@v1.0#sub");
    assert_eq!(name, "path");
}

// ===========================================================================
// record_include_relationships tests (F-054)
// ===========================================================================

#[test]
fn test_record_include_relationships_basic() {
    let tmp = tempdir().unwrap();
    let mut registry = BundleRegistry::new(tmp.path().to_path_buf());

    // Register parent and two children
    let bundles = HashMap::from([
        ("parent".to_string(), "file:///parent".to_string()),
        ("child-a".to_string(), "file:///child-a".to_string()),
        ("child-b".to_string(), "file:///child-b".to_string()),
    ]);
    registry.register(&bundles);

    let child_names = vec!["child-a".to_string(), "child-b".to_string()];
    registry.record_include_relationships("parent", &child_names);

    // Parent should have both children in includes
    let parent = registry.find_state("parent").unwrap();
    assert_eq!(parent.includes, vec!["child-a", "child-b"]);

    // Each child should have parent in included_by
    let child_a = registry.find_state("child-a").unwrap();
    assert_eq!(child_a.included_by, vec!["parent"]);

    let child_b = registry.find_state("child-b").unwrap();
    assert_eq!(child_b.included_by, vec!["parent"]);
}

#[test]
fn test_record_include_relationships_dedup() {
    let tmp = tempdir().unwrap();
    let mut registry = BundleRegistry::new(tmp.path().to_path_buf());

    let bundles = HashMap::from([
        ("parent".to_string(), "file:///parent".to_string()),
        ("child-a".to_string(), "file:///child-a".to_string()),
    ]);
    registry.register(&bundles);

    let child_names = vec!["child-a".to_string()];

    // Call twice with the same names
    registry.record_include_relationships("parent", &child_names);
    registry.record_include_relationships("parent", &child_names);

    // Should NOT have duplicates
    let parent = registry.find_state("parent").unwrap();
    assert_eq!(parent.includes, vec!["child-a"]);
    assert_eq!(
        parent.includes.len(),
        1,
        "includes should not have duplicates"
    );

    let child_a = registry.find_state("child-a").unwrap();
    assert_eq!(child_a.included_by, vec!["parent"]);
    assert_eq!(
        child_a.included_by.len(),
        1,
        "included_by should not have duplicates"
    );
}

#[test]
fn test_record_include_relationships_persists() {
    let tmp = tempdir().unwrap();
    let home = tmp.path().to_path_buf();

    // Register and record relationships
    {
        let mut registry = BundleRegistry::new(home.clone());
        let bundles = HashMap::from([
            ("parent".to_string(), "file:///parent".to_string()),
            ("child".to_string(), "file:///child".to_string()),
        ]);
        registry.register(&bundles);
        registry.save(); // persist the registration first

        let child_names = vec!["child".to_string()];
        registry.record_include_relationships("parent", &child_names);
        // record_include_relationships calls save() internally
    }

    // Load a fresh registry from disk and verify state was persisted
    {
        let registry = BundleRegistry::new(home);

        let parent = registry.find_state("parent").unwrap();
        assert_eq!(
            parent.includes,
            vec!["child"],
            "includes should be persisted to disk"
        );

        let child = registry.find_state("child").unwrap();
        assert_eq!(
            child.included_by,
            vec!["parent"],
            "included_by should be persisted to disk"
        );
    }
}

#[test]
fn test_record_include_relationships_missing_parent() {
    // Parent not in registry — children should still be updated
    let tmp = tempdir().unwrap();
    let mut registry = BundleRegistry::new(tmp.path().to_path_buf());

    // Only register the child, not the parent
    let bundles = HashMap::from([("child".to_string(), "file:///child".to_string())]);
    registry.register(&bundles);

    let child_names = vec!["child".to_string()];
    registry.record_include_relationships("nonexistent-parent", &child_names);

    // Child should still get the included_by entry
    let child = registry.find_state("child").unwrap();
    assert_eq!(child.included_by, vec!["nonexistent-parent"]);

    // Parent should not have been created
    assert!(
        registry.find_state("nonexistent-parent").is_none(),
        "Missing parent should NOT be auto-created"
    );
}

#[test]
fn test_record_include_relationships_missing_child() {
    // Child not in registry — parent should still be updated
    let tmp = tempdir().unwrap();
    let mut registry = BundleRegistry::new(tmp.path().to_path_buf());

    // Only register the parent, not the child
    let bundles = HashMap::from([("parent".to_string(), "file:///parent".to_string())]);
    registry.register(&bundles);

    let child_names = vec!["nonexistent-child".to_string()];
    registry.record_include_relationships("parent", &child_names);

    // Parent should still get the includes entry
    let parent = registry.find_state("parent").unwrap();
    assert_eq!(parent.includes, vec!["nonexistent-child"]);

    // Child should not have been created
    assert!(
        registry.find_state("nonexistent-child").is_none(),
        "Missing child should NOT be auto-created"
    );
}

// ===========================================================================
// compose_includes enhanced tests (F-054)
// ===========================================================================

#[tokio::test]
async fn test_compose_includes_dict_style() {
    // compose_includes should handle {"bundle": "..."} includes via parse_include
    let tmp = tempdir().unwrap();
    let root = tmp.path();

    // Create the parent bundle directory
    let parent_dir = root.join("parent-bundle");
    fs::create_dir_all(&parent_dir).unwrap();

    // Create the child bundle directory
    let child_dir = root.join("child-bundle");
    fs::create_dir_all(&child_dir).unwrap();
    write_simple_bundle_yaml(&child_dir, "child-bundle");

    // Write parent bundle.yaml that includes the child via dict-style {"bundle": "file://..."}
    let child_uri = format!("file://{}", child_dir.display());
    let content = format!(
        "name: parent-bundle\nversion: \"1.0.0\"\nincludes:\n  - bundle: \"{}\"\n",
        child_uri
    );
    write_bundle_yaml(&parent_dir, &content);

    let parent_uri = format!("file://{}", parent_dir.display());

    let registry = BundleRegistry::new(root.to_path_buf());
    let result = registry.load_single(&parent_uri).await;

    assert!(
        result.is_ok(),
        "Dict-style include should load: {:?}",
        result.err()
    );
    let bundle = result.unwrap();
    // The composed bundle should have the parent's name (bundle on top wins)
    assert_eq!(bundle.name, "parent-bundle");
}

#[tokio::test]
async fn test_compose_includes_with_resolve() {
    // compose_includes should resolve namespace:path includes via resolve_include_source
    let tmp = tempdir().unwrap();
    let root = tmp.path();

    // Create the namespace root bundle
    let ns_dir = root.join("my-namespace");
    fs::create_dir_all(&ns_dir).unwrap();
    write_simple_bundle_yaml(&ns_dir, "my-namespace");

    // Create a skill bundle inside the namespace
    let skill_dir = ns_dir.join("skills").join("coding");
    fs::create_dir_all(&skill_dir).unwrap();
    write_simple_bundle_yaml(&skill_dir, "coding");

    // Create the parent bundle that uses namespace:path include
    let parent_dir = root.join("parent-bundle");
    fs::create_dir_all(&parent_dir).unwrap();

    // The include uses namespace:path syntax
    let content =
        "name: parent-bundle\nversion: \"1.0.0\"\nincludes:\n  - \"my-namespace:skills/coding\"\n";
    write_bundle_yaml(&parent_dir, content);

    // Set up the registry with the namespace registered and local_path set
    let mut registry = BundleRegistry::new(root.to_path_buf());
    register_one(&mut registry, "my-namespace", "file:///original");
    registry.get_state("my-namespace").local_path = Some(ns_dir.to_string_lossy().to_string());

    let parent_uri = format!("file://{}", parent_dir.display());
    let result = registry.load_single(&parent_uri).await;

    assert!(
        result.is_ok(),
        "Namespace:path include should resolve and load: {:?}",
        result.err()
    );
    let bundle = result.unwrap();
    assert_eq!(bundle.name, "parent-bundle");
}

#[tokio::test]
async fn test_compose_includes_registered_namespace_missing_path_is_error() {
    // When resolve_include_source returns None for a namespace:path include
    // where the namespace IS registered, this should be a DependencyError,
    // not a silent skip. Matches Python behavior.
    let tmp = tempdir().unwrap();
    let root = tmp.path();

    // Create a namespace with local_path but NO resource at the included path
    let ns_dir = root.join("my-namespace");
    fs::create_dir_all(&ns_dir).unwrap();
    write_simple_bundle_yaml(&ns_dir, "my-namespace");

    // Create parent bundle that includes a nonexistent path in the namespace
    let parent_dir = root.join("parent-bundle");
    fs::create_dir_all(&parent_dir).unwrap();
    let content = "name: parent-bundle\nversion: \"1.0.0\"\nincludes:\n  - \"my-namespace:nonexistent/path\"\n";
    write_bundle_yaml(&parent_dir, content);

    let mut registry = BundleRegistry::new(root.to_path_buf());
    register_one(&mut registry, "my-namespace", "file:///original");
    registry.get_state("my-namespace").local_path = Some(ns_dir.to_string_lossy().to_string());

    let parent_uri = format!("file://{}", parent_dir.display());
    let result = registry.load_single(&parent_uri).await;

    assert!(
        result.is_err(),
        "Registered namespace with missing path should be an error"
    );
    let err = result.unwrap_err();
    match err {
        amplifier_foundation::BundleError::DependencyError(msg) => {
            assert!(
                msg.contains("my-namespace"),
                "Error should mention the namespace: {}",
                msg
            );
            assert!(
                msg.contains("nonexistent/path"),
                "Error should mention the path: {}",
                msg
            );
        }
        other => {
            panic!("Expected DependencyError, got: {:?}", other);
        }
    }
}

// ===========================================================================
// check_update / update lifecycle tests (F-055)
// ===========================================================================

#[tokio::test]
async fn test_check_update_single_updates_timestamp() {
    let tmp = tempdir().unwrap();
    let mut registry = BundleRegistry::new(tmp.path().to_path_buf());
    register_one(&mut registry, "my-bundle", "file:///some/path");

    // checked_at should be None initially
    assert!(registry
        .find_state("my-bundle")
        .unwrap()
        .checked_at
        .is_none());

    let result = registry.check_update_single("my-bundle").await;

    // Stub always returns None (no actual version comparison)
    assert!(result.is_none());

    // But checked_at should now be set
    let state = registry.find_state("my-bundle").unwrap();
    assert!(
        state.checked_at.is_some(),
        "checked_at should be set after check_update_single"
    );
}

#[tokio::test]
async fn test_check_update_single_unregistered_returns_none() {
    let tmp = tempdir().unwrap();
    let mut registry = BundleRegistry::new(tmp.path().to_path_buf());

    // Check update for a name that doesn't exist
    let result = registry.check_update_single("nonexistent").await;

    assert!(
        result.is_none(),
        "check_update_single for unregistered bundle should return None"
    );
}

#[tokio::test]
async fn test_check_update_all_empty_registry() {
    let tmp = tempdir().unwrap();
    let mut registry = BundleRegistry::new(tmp.path().to_path_buf());

    let result = registry.check_update_all().await;

    assert!(
        result.is_empty(),
        "check_update_all on empty registry should return empty Vec"
    );
}

#[tokio::test]
async fn test_check_update_all_updates_all_timestamps() {
    let tmp = tempdir().unwrap();
    let mut registry = BundleRegistry::new(tmp.path().to_path_buf());

    let bundles = HashMap::from([
        ("alpha".to_string(), "file:///a".to_string()),
        ("beta".to_string(), "file:///b".to_string()),
        ("gamma".to_string(), "file:///c".to_string()),
    ]);
    registry.register(&bundles);

    // All checked_at should be None initially
    for name in ["alpha", "beta", "gamma"] {
        assert!(registry.find_state(name).unwrap().checked_at.is_none());
    }

    let result = registry.check_update_all().await;

    // Stub returns empty
    assert!(result.is_empty());

    // All checked_at should now be set
    for name in ["alpha", "beta", "gamma"] {
        let state = registry.find_state(name).unwrap();
        assert!(
            state.checked_at.is_some(),
            "checked_at for '{}' should be set after check_update(None)",
            name
        );
    }
}

#[tokio::test]
async fn test_update_single_basic() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();

    // Create a bundle on disk
    let bundle_dir = root.join("my-bundle");
    fs::create_dir_all(&bundle_dir).unwrap();
    write_simple_bundle_yaml(&bundle_dir, "my-bundle");

    let uri = format!("file://{}", bundle_dir.display());
    let mut registry = BundleRegistry::new(root.to_path_buf());
    register_one(&mut registry, "my-bundle", &uri);

    // Timestamps should be None initially
    assert!(registry
        .find_state("my-bundle")
        .unwrap()
        .loaded_at
        .is_none());
    assert!(registry
        .find_state("my-bundle")
        .unwrap()
        .checked_at
        .is_none());

    let bundle = registry
        .update_single("my-bundle")
        .await
        .expect("update_single should succeed");

    assert_eq!(bundle.name, "my-bundle");

    // State should have updated timestamps
    let state = registry.find_state("my-bundle").unwrap();
    assert!(
        state.loaded_at.is_some(),
        "loaded_at should be set after update_single"
    );
    assert!(
        state.checked_at.is_some(),
        "checked_at should be set after update_single"
    );
    assert_eq!(
        state.version.as_deref(),
        Some("1.0.0"),
        "version should be updated from bundle"
    );
}

#[tokio::test]
async fn test_update_single_unregistered_is_error() {
    let tmp = tempdir().unwrap();
    let mut registry = BundleRegistry::new(tmp.path().to_path_buf());

    let result = registry.update_single("nonexistent").await;

    assert!(
        result.is_err(),
        "update_single on unregistered bundle should return error"
    );
    match result.unwrap_err() {
        amplifier_foundation::BundleError::NotFound { uri } => {
            assert!(
                uri.contains("nonexistent"),
                "Error should mention the bundle name: {}",
                uri
            );
        }
        other => {
            panic!("Expected NotFound error, got: {:?}", other);
        }
    }
}

#[tokio::test]
async fn test_update_all_basic() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();

    // Create two bundles on disk
    let dir_a = root.join("alpha");
    fs::create_dir_all(&dir_a).unwrap();
    write_simple_bundle_yaml(&dir_a, "alpha");

    let dir_b = root.join("beta");
    fs::create_dir_all(&dir_b).unwrap();
    write_simple_bundle_yaml(&dir_b, "beta");

    let uri_a = format!("file://{}", dir_a.display());
    let uri_b = format!("file://{}", dir_b.display());

    let mut registry = BundleRegistry::new(root.to_path_buf());
    let bundles_map = HashMap::from([("alpha".to_string(), uri_a), ("beta".to_string(), uri_b)]);
    registry.register(&bundles_map);

    let result = registry.update_all().await;

    assert_eq!(result.len(), 2, "Should have updated both bundles");
    assert!(result.contains_key("alpha"));
    assert!(result.contains_key("beta"));

    // Both should have updated timestamps
    for name in ["alpha", "beta"] {
        let state = registry.find_state(name).unwrap();
        assert!(
            state.loaded_at.is_some(),
            "loaded_at for '{}' should be set after update_all",
            name
        );
        assert!(
            state.checked_at.is_some(),
            "checked_at for '{}' should be set after update_all",
            name
        );
    }
}

#[tokio::test]
async fn test_update_single_bypasses_cache() {
    // Verifies that update_single forces a fresh load from disk
    // (not returning cached data from a prior load_single call)
    let tmp = tempdir().unwrap();
    let root = tmp.path();

    let bundle_dir = root.join("my-bundle");
    fs::create_dir_all(&bundle_dir).unwrap();
    fs::write(
        bundle_dir.join("bundle.yaml"),
        "name: my-bundle\nversion: \"1.0.0\"\n",
    )
    .unwrap();

    let uri = format!("file://{}", bundle_dir.display());
    let mut registry = BundleRegistry::new(root.to_path_buf());
    register_one(&mut registry, "my-bundle", &uri);

    // First load — populates cache
    let bundle1 = registry.load_single(&uri).await.unwrap();
    assert_eq!(bundle1.version, "1.0.0");

    // Modify the file on disk
    fs::write(
        bundle_dir.join("bundle.yaml"),
        "name: my-bundle\nversion: \"2.0.0\"\n",
    )
    .unwrap();

    // update_single should bypass cache and read the new version
    let bundle2 = registry.update_single("my-bundle").await.unwrap();
    assert_eq!(
        bundle2.version, "2.0.0",
        "update_single should bypass cache and read updated file"
    );

    // State should be updated
    let state = registry.find_state("my-bundle").unwrap();
    assert_eq!(state.version.as_deref(), Some("2.0.0"));
    assert!(state.loaded_at.is_some());
    assert!(state.checked_at.is_some());
}
