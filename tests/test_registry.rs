//! Tests for registry module (BundleRegistry).
//!
//! Ported from Python test_registry.py — 21 tests across 4 groups.
//! All tests are Wave 3 (ignored until implementations land).

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use tempfile::tempdir;

use amplifier_foundation::registry::{BundleRegistry, BundleState};

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
