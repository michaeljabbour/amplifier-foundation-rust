//! Tests for the modules crate -- InstallStateManager.
//!
//! Ported from Python's InstallStateManager behavior (no Python test file exists;
//! tests written from behavioral specification in install_state.py).

use std::fs;
use tempfile::TempDir;

use amplifier_foundation::InstallStateManager;

// ── Construction & Fresh State ──────────────────────────────────────────

#[test]
fn test_new_creates_fresh_state_when_no_file() {
    let tmp = TempDir::new().unwrap();
    let mgr = InstallStateManager::new(tmp.path().to_path_buf());
    // Should not panic, state is fresh
    // Fresh state IS dirty (matches Python: _fresh_state sets _dirty=True)
    assert!(mgr.is_dirty());
    // State file should NOT exist yet (not saved)
    assert!(!tmp.path().join("install-state.json").exists());
}

#[test]
fn test_fresh_state_has_correct_version() {
    let tmp = TempDir::new().unwrap();
    let mut mgr = InstallStateManager::new(tmp.path().to_path_buf());
    mgr.save().unwrap();

    let content = fs::read_to_string(tmp.path().join("install-state.json")).unwrap();
    let data: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(data["version"], 1);
    assert!(data["modules"].is_object());
}

// ── is_installed / mark_installed ───────────────────────────────────────

#[test]
fn test_is_installed_returns_false_for_unknown_module() {
    let tmp = TempDir::new().unwrap();
    let mgr = InstallStateManager::new(tmp.path().to_path_buf());

    let module_dir = tmp.path().join("some-module");
    fs::create_dir_all(&module_dir).unwrap();
    assert!(!mgr.is_installed(&module_dir));
}

#[test]
fn test_mark_installed_then_is_installed() {
    let tmp = TempDir::new().unwrap();
    let mut mgr = InstallStateManager::new(tmp.path().to_path_buf());

    let module_dir = tmp.path().join("my-module");
    fs::create_dir_all(&module_dir).unwrap();
    // Create a pyproject.toml so we get a real fingerprint
    fs::write(
        module_dir.join("pyproject.toml"),
        b"[project]\nname = \"test\"",
    )
    .unwrap();

    mgr.mark_installed(&module_dir);
    assert!(mgr.is_installed(&module_dir));
    assert!(mgr.is_dirty());
}

#[test]
fn test_fingerprint_change_invalidates_installed() {
    let tmp = TempDir::new().unwrap();
    let mut mgr = InstallStateManager::new(tmp.path().to_path_buf());

    let module_dir = tmp.path().join("my-module");
    fs::create_dir_all(&module_dir).unwrap();
    fs::write(module_dir.join("pyproject.toml"), b"version1").unwrap();

    mgr.mark_installed(&module_dir);
    assert!(mgr.is_installed(&module_dir));

    // Change the file content -> fingerprint changes
    fs::write(module_dir.join("pyproject.toml"), b"version2").unwrap();
    assert!(!mgr.is_installed(&module_dir));
}

#[test]
fn test_module_with_no_dep_files_gets_none_fingerprint() {
    let tmp = TempDir::new().unwrap();
    let mut mgr = InstallStateManager::new(tmp.path().to_path_buf());

    // Module dir with NO pyproject.toml or requirements.txt
    let module_dir = tmp.path().join("bare-module");
    fs::create_dir_all(&module_dir).unwrap();

    mgr.mark_installed(&module_dir);
    assert!(mgr.is_installed(&module_dir));
}

#[test]
fn test_fingerprint_includes_requirements_txt() {
    let tmp = TempDir::new().unwrap();
    let mut mgr = InstallStateManager::new(tmp.path().to_path_buf());

    let module_dir = tmp.path().join("my-module");
    fs::create_dir_all(&module_dir).unwrap();
    fs::write(module_dir.join("requirements.txt"), b"requests==2.31.0").unwrap();

    mgr.mark_installed(&module_dir);
    assert!(mgr.is_installed(&module_dir));

    // Change requirements.txt -> fingerprint changes
    fs::write(module_dir.join("requirements.txt"), b"requests==2.32.0").unwrap();
    assert!(!mgr.is_installed(&module_dir));
}

// ── Persistence ─────────────────────────────────────────────────────────

#[test]
fn test_save_and_reload() {
    let tmp = TempDir::new().unwrap();

    let module_dir = tmp.path().join("persisted-module");
    fs::create_dir_all(&module_dir).unwrap();
    fs::write(
        module_dir.join("pyproject.toml"),
        b"[project]\nname = \"p\"",
    )
    .unwrap();

    // Save state
    {
        let mut mgr = InstallStateManager::new(tmp.path().to_path_buf());
        mgr.mark_installed(&module_dir);
        mgr.save().unwrap();
    }

    // Reload in new instance
    {
        let mgr = InstallStateManager::new(tmp.path().to_path_buf());
        assert!(mgr.is_installed(&module_dir));
    }
}

#[test]
fn test_save_fresh_state_writes_file() {
    let tmp = TempDir::new().unwrap();
    let mut mgr = InstallStateManager::new(tmp.path().to_path_buf());

    // Fresh state IS dirty (matches Python: _fresh_state sets _dirty=True).
    // save() should write the file.
    mgr.save().unwrap();
    assert!(tmp.path().join("install-state.json").exists());
}

#[test]
fn test_save_is_noop_after_clean_load() {
    let tmp = TempDir::new().unwrap();

    // Create initial state
    {
        let mut mgr = InstallStateManager::new(tmp.path().to_path_buf());
        mgr.save().unwrap();
    }

    // Reload -- should NOT be dirty
    {
        let mgr = InstallStateManager::new(tmp.path().to_path_buf());
        assert!(!mgr.is_dirty());
    }
}

// ── Self-Healing ────────────────────────────────────────────────────────

#[test]
fn test_corrupted_json_creates_fresh_state() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("install-state.json"), b"NOT VALID JSON{{").unwrap();

    let mut mgr = InstallStateManager::new(tmp.path().to_path_buf());
    // Should silently recover to fresh state
    mgr.save().unwrap();

    let content = fs::read_to_string(tmp.path().join("install-state.json")).unwrap();
    let data: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(data["version"], 1);
}

#[test]
fn test_version_mismatch_creates_fresh_state() {
    let tmp = TempDir::new().unwrap();
    let old_state = serde_json::json!({
        "version": 99,
        "modules": {"some_module": {"pyproject_hash": "sha256:abc"}}
    });
    fs::write(
        tmp.path().join("install-state.json"),
        serde_json::to_string_pretty(&old_state).unwrap(),
    )
    .unwrap();

    let mgr = InstallStateManager::new(tmp.path().to_path_buf());
    let module_dir = tmp.path().join("some_module");
    fs::create_dir_all(&module_dir).unwrap();
    // Old entries should be gone
    assert!(!mgr.is_installed(&module_dir));
}

// ── Dirty flag management ───────────────────────────────────────────────

#[test]
fn test_save_clears_dirty_flag() {
    let tmp = TempDir::new().unwrap();
    let mut mgr = InstallStateManager::new(tmp.path().to_path_buf());

    let module_dir = tmp.path().join("my-module");
    fs::create_dir_all(&module_dir).unwrap();
    fs::write(
        module_dir.join("pyproject.toml"),
        b"[project]\nname = \"p\"",
    )
    .unwrap();

    mgr.mark_installed(&module_dir);
    assert!(mgr.is_dirty());
    mgr.save().unwrap();
    assert!(!mgr.is_dirty());
}

#[test]
fn test_double_save_is_noop() {
    let tmp = TempDir::new().unwrap();
    let mut mgr = InstallStateManager::new(tmp.path().to_path_buf());

    let module_dir = tmp.path().join("my-module");
    fs::create_dir_all(&module_dir).unwrap();
    mgr.mark_installed(&module_dir);
    mgr.save().unwrap();

    // Get mtime after first save
    let mtime1 = fs::metadata(tmp.path().join("install-state.json"))
        .unwrap()
        .modified()
        .unwrap();

    // Small sleep to ensure different mtime if written
    std::thread::sleep(std::time::Duration::from_millis(50));

    // Second save should be a no-op (not dirty)
    mgr.save().unwrap();
    let mtime2 = fs::metadata(tmp.path().join("install-state.json"))
        .unwrap()
        .modified()
        .unwrap();
    assert_eq!(mtime1, mtime2);
}

// ── Fingerprint format ──────────────────────────────────────────────────

#[test]
fn test_fingerprint_format_sha256() {
    let tmp = TempDir::new().unwrap();
    let mut mgr = InstallStateManager::new(tmp.path().to_path_buf());

    let module_dir = tmp.path().join("fmt-mod");
    fs::create_dir_all(&module_dir).unwrap();
    fs::write(module_dir.join("pyproject.toml"), b"content").unwrap();

    mgr.mark_installed(&module_dir);
    mgr.save().unwrap();

    // Read stored state and check fingerprint format
    let content = fs::read_to_string(tmp.path().join("install-state.json")).unwrap();
    let data: serde_json::Value = serde_json::from_str(&content).unwrap();
    let modules = data["modules"].as_object().unwrap();
    // Should have exactly one entry
    assert_eq!(modules.len(), 1);
    let entry = modules.values().next().unwrap();
    let hash = entry["pyproject_hash"].as_str().unwrap();
    assert!(
        hash.starts_with("sha256:"),
        "Expected sha256: prefix, got: {hash}"
    );
    // SHA-256 hex digest is 64 chars
    assert_eq!(
        hash.len(),
        7 + 64,
        "Expected sha256:<64 hex chars>, got: {hash}"
    );
}

// ── Cross-implementation compatibility ──────────────────────────────────

#[test]
fn test_loads_state_with_extra_fields() {
    // Python state has a "python" field. Rust should tolerate unknown fields.
    let tmp = TempDir::new().unwrap();
    let state_with_python = serde_json::json!({
        "version": 1,
        "python": "/usr/bin/python3",
        "modules": {}
    });
    fs::write(
        tmp.path().join("install-state.json"),
        serde_json::to_string_pretty(&state_with_python).unwrap(),
    )
    .unwrap();

    let mgr = InstallStateManager::new(tmp.path().to_path_buf());
    // Should load successfully without panicking or resetting
    assert!(!mgr.is_dirty());
}

// ── Invalidation ────────────────────────────────────────────────────────

#[test]
fn test_invalidate_specific_module() {
    let tmp = TempDir::new().unwrap();
    let mut mgr = InstallStateManager::new(tmp.path().to_path_buf());

    let mod_a = tmp.path().join("mod-a");
    let mod_b = tmp.path().join("mod-b");
    fs::create_dir_all(&mod_a).unwrap();
    fs::create_dir_all(&mod_b).unwrap();

    mgr.mark_installed(&mod_a);
    mgr.mark_installed(&mod_b);
    assert!(mgr.is_installed(&mod_a));
    assert!(mgr.is_installed(&mod_b));

    mgr.invalidate(Some(&mod_a));
    assert!(!mgr.is_installed(&mod_a));
    assert!(mgr.is_installed(&mod_b));
}

#[test]
fn test_invalidate_all_modules() {
    let tmp = TempDir::new().unwrap();
    let mut mgr = InstallStateManager::new(tmp.path().to_path_buf());

    let mod_a = tmp.path().join("mod-a");
    let mod_b = tmp.path().join("mod-b");
    fs::create_dir_all(&mod_a).unwrap();
    fs::create_dir_all(&mod_b).unwrap();

    mgr.mark_installed(&mod_a);
    mgr.mark_installed(&mod_b);

    mgr.invalidate(None);
    assert!(!mgr.is_installed(&mod_a));
    assert!(!mgr.is_installed(&mod_b));
}
