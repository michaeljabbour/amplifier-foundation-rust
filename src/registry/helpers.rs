//! Free-standing helper functions for registry operations.
//!
//! These are module-level functions used by multiple registry submodules.

use serde_yaml_ng::Value;
use std::path::{Path, PathBuf};

/// Resolve a file:// URI to a local filesystem path.
pub(super) fn resolve_file_uri(uri: &str) -> crate::error::Result<PathBuf> {
    if let Some(stripped) = uri.strip_prefix("file://") {
        Ok(PathBuf::from(stripped))
    } else if uri.starts_with('/') || uri.starts_with('.') {
        // Already a local path
        Ok(PathBuf::from(uri))
    } else {
        Err(crate::error::BundleError::LoadError {
            reason: format!("Unsupported URI scheme: {}", uri),
            source: None,
        })
    }
}

/// Parse an include value from bundle YAML data.
///
/// Accepts:
/// - String value → returns the string
/// - Mapping with `"bundle"` key → returns the bundle value as string
/// - Anything else → `None`
///
/// Port of Python `_parse_include`.
pub fn parse_include(include: &Value) -> Option<String> {
    match include {
        Value::String(s) => Some(s.clone()),
        Value::Mapping(map) => {
            let key = Value::String("bundle".to_string());
            let bundle_ref = map.get(&key)?;
            // Python uses `str(bundle_ref)` which coerces any truthy value to string.
            // We match by converting the Value to a string representation.
            let s = match bundle_ref {
                Value::String(s) if !s.is_empty() => s.clone(),
                Value::Number(n) => n.to_string(),
                Value::Bool(true) => "true".to_string(),
                Value::Null | Value::Bool(false) => return None,
                Value::String(s) if s.is_empty() => return None,
                other => format!("{:?}", other),
            };
            Some(s)
        }
        _ => None,
    }
}

/// Find a resource path by probing candidate extensions and subdirectories.
///
/// Tries these candidates in order:
/// 1. `base_path` as-is
/// 2. `base_path` with `.yaml` extension
/// 3. `base_path` with `.yml` extension
/// 4. `base_path` with `.md` extension
/// 5. `base_path/bundle.yaml`
/// 6. `base_path/bundle.md`
///
/// Returns the first existing candidate resolved to its canonical (absolute) path,
/// or `None` if none exist.
///
/// Port of Python `_find_resource_path`.
pub fn find_resource_path(base_path: &Path) -> Option<PathBuf> {
    let candidates = [
        base_path.to_path_buf(),
        base_path.with_extension("yaml"),
        base_path.with_extension("yml"),
        base_path.with_extension("md"),
        base_path.join("bundle.yaml"),
        base_path.join("bundle.md"),
    ];

    for candidate in &candidates {
        if candidate.exists() {
            return Some(std::fs::canonicalize(candidate).unwrap_or_else(|_| {
                std::path::absolute(candidate).unwrap_or_else(|_| candidate.clone())
            }));
        }
    }
    None
}

/// Extract a human-readable name from a URI.
///
/// - GitHub URIs: extracts the repo name from `github.com/org/repo@ref#fragment`
/// - `file://` URIs: returns the last path segment
/// - Fallback: last path component, stripping `@ref` and `#fragment`
///
/// Port of Python `_extract_bundle_name`.
pub fn extract_bundle_name(uri: &str) -> String {
    // GitHub URIs: extract repo name
    if uri.contains("github.com") {
        let parts: Vec<&str> = uri.split('/').collect();
        for (i, part) in parts.iter().enumerate() {
            if part.contains("github.com") && i + 2 < parts.len() {
                let name = parts[i + 2].split('@').next().unwrap_or("");
                let name = name.split('#').next().unwrap_or("");
                if !name.is_empty() {
                    return name.to_string();
                }
            }
        }
    }

    // file:// URIs: last path segment
    if uri.starts_with("file://") {
        return uri
            .split('/')
            .next_back()
            .unwrap_or("unknown")
            .split('#')
            .next()
            .unwrap_or("unknown")
            .to_string();
    }

    // Fallback: last path component, stripping @ref and #fragment
    uri.split('/')
        .next_back()
        .unwrap_or("unknown")
        .split('@')
        .next()
        .unwrap_or("unknown")
        .split('#')
        .next()
        .unwrap_or("unknown")
        .to_string()
}
