//! Bundle overlay CRUD for `~/.amplifier-distro/bundle/bundle.yaml`.
//!
//! The overlay is a `bundle.yaml` that lives in the distro home and acts as the
//! top-level includes manifest.  Applications read this file to know which
//! provider/feature bundles to load.

use serde_yaml_ng::{Mapping, Value};

use crate::{conventions, DistroError, Result};

/// The default distro start bundle URI.
pub const AMPLIFIER_START_URI: &str =
    "git+https://github.com/microsoft/amplifier-distro@main#subdirectory=bundle";

// ---------------------------------------------------------------------------
// Primitives
// ---------------------------------------------------------------------------

/// Returns `true` if the overlay bundle.yaml exists on disk.
pub fn overlay_exists() -> bool {
    conventions::distro_overlay_bundle_path().exists()
}

/// Read the overlay from disk.
///
/// Returns an empty YAML mapping on missing or corrupt file (never errors).
pub fn read_overlay() -> Value {
    let path = conventions::distro_overlay_bundle_path();
    if !path.exists() {
        return Value::Mapping(Mapping::new());
    }
    match std::fs::read_to_string(&path) {
        Ok(content) => {
            serde_yaml_ng::from_str(&content).unwrap_or_else(|_| Value::Mapping(Mapping::new()))
        }
        Err(_) => Value::Mapping(Mapping::new()),
    }
}

/// Write `data` to the overlay bundle.yaml, creating parent directories as needed.
pub fn write_overlay(data: &Value) -> Result<()> {
    let path = conventions::distro_overlay_bundle_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content =
        serde_yaml_ng::to_string(data).map_err(|e| DistroError::Overlay(format!("{e}")))?;
    std::fs::write(&path, content)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Include list helpers
// ---------------------------------------------------------------------------

/// Extract the flat list of include URIs from a bundle.yaml value.
///
/// Handles both plain string entries and `{bundle: "…"}` object entries.
pub fn get_includes(data: &Value) -> Vec<String> {
    let arr = match data
        .get("bundle")
        .and_then(|b| b.get("includes"))
        .and_then(|i| i.as_sequence())
    {
        Some(a) => a,
        None => return vec![],
    };

    arr.iter()
        .filter_map(|item| {
            if let Some(s) = item.as_str() {
                Some(s.to_string())
            } else if let Some(obj) = item.as_mapping() {
                obj.get("bundle")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            } else {
                None
            }
        })
        .collect()
}

/// Replace the include list inside `data` in-place.
fn set_includes(data: &mut Value, includes: Vec<String>) {
    let yaml_includes: Vec<Value> = includes.into_iter().map(Value::String).collect();
    let includes_val = Value::Sequence(yaml_includes);

    if data.get("bundle").is_some() {
        if let Some(bundle) = data.get_mut("bundle") {
            if let Some(map) = bundle.as_mapping_mut() {
                map.insert(
                    Value::String("includes".to_string()),
                    includes_val,
                );
            }
        }
    } else {
        // Build a fresh bundle structure.
        let mut bundle_map = Mapping::new();
        bundle_map.insert(
            Value::String("name".to_string()),
            Value::String("distro-overlay".to_string()),
        );
        bundle_map.insert(Value::String("includes".to_string()), includes_val);

        if let Some(root) = data.as_mapping_mut() {
            root.insert(
                Value::String("bundle".to_string()),
                Value::Mapping(bundle_map),
            );
        } else {
            // data is not a mapping; replace entirely.
            let mut root = Mapping::new();
            root.insert(
                Value::String("bundle".to_string()),
                Value::Mapping(bundle_map),
            );
            *data = Value::Mapping(root);
        }
    }
}

// ---------------------------------------------------------------------------
// High-level operations
// ---------------------------------------------------------------------------

/// Idempotently add a URI to the overlay include list.
pub fn add_include(uri: &str) -> Result<()> {
    let mut data = read_overlay();
    let mut includes = get_includes(&data);
    if includes.iter().any(|u| u == uri) {
        return Ok(()); // already present
    }
    includes.push(uri.to_string());
    set_includes(&mut data, includes);
    write_overlay(&data)
}

/// Remove a URI from the overlay include list (no-op if not present).
pub fn remove_include(uri: &str) -> Result<()> {
    let mut data = read_overlay();
    let includes: Vec<String> = get_includes(&data)
        .into_iter()
        .filter(|u| u != uri)
        .collect();
    set_includes(&mut data, includes);
    write_overlay(&data)
}

/// Ensure an overlay exists, optionally adding a provider include URI.
///
/// - If no overlay exists: creates one with the default distro start URI.
/// - If the overlay already exists: optionally appends `provider_include`.
pub fn ensure_overlay(provider_include: Option<&str>) -> Result<()> {
    if !overlay_exists() {
        let mut includes = vec![AMPLIFIER_START_URI.to_string()];
        if let Some(p) = provider_include {
            includes.push(p.to_string());
        }

        let mut bundle_map = Mapping::new();
        bundle_map.insert(
            Value::String("name".to_string()),
            Value::String("distro-overlay".to_string()),
        );
        bundle_map.insert(
            Value::String("includes".to_string()),
            Value::Sequence(includes.into_iter().map(Value::String).collect()),
        );

        let mut root = Mapping::new();
        root.insert(
            Value::String("bundle".to_string()),
            Value::Mapping(bundle_map),
        );
        write_overlay(&Value::Mapping(root))?;
    } else if let Some(p) = provider_include {
        add_include(p)?;
    }
    Ok(())
}

/// Migrate the overlay: replace stale URIs with canonical ones.
///
/// Currently a no-op stub — full migration logic is on the Python side.
pub fn migrate_overlay() -> Result<()> {
    // STUB: URI replacement + stale-entry removal
    Ok(())
}

/// Return the raw text of the overlay file, or `None` if it doesn't exist.
pub fn snapshot_overlay() -> Option<String> {
    std::fs::read_to_string(conventions::distro_overlay_bundle_path()).ok()
}

/// Restore the overlay from a raw-text snapshot.
///
/// Passing `None` removes the overlay file entirely.
pub fn restore_overlay(snapshot: Option<&str>) -> Result<()> {
    let path = conventions::distro_overlay_bundle_path();
    match snapshot {
        Some(content) => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&path, content)?;
        }
        None => {
            if path.exists() {
                std::fs::remove_file(&path)?;
            }
        }
    }
    Ok(())
}
