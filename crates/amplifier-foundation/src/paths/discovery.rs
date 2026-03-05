use std::path::{Path, PathBuf};

/// Find files matching a glob pattern. SYNC (not async).
///
/// If recursive and pattern doesn't start with "**", prepends "**/".
/// Returns sorted results matching Python behavior.
pub fn find_files(base: &Path, pattern: &str, recursive: bool) -> Vec<PathBuf> {
    let effective_pattern = if recursive && !pattern.starts_with("**") {
        format!("**/{pattern}")
    } else {
        pattern.to_string()
    };

    let full_pattern = base.join(&effective_pattern);
    let pattern_str = full_pattern.to_string_lossy();

    let mut results: Vec<PathBuf> = match glob::glob(&pattern_str) {
        Ok(paths) => paths.filter_map(Result::ok).collect(),
        Err(e) => {
            tracing::warn!("Invalid glob pattern '{}': {}", pattern_str, e);
            Vec::new()
        }
    };

    results.sort();
    results
}

/// Find the bundle root directory containing bundle.md or bundle.yaml.
///
/// Searches from start directory upward to filesystem root. SYNC.
pub fn find_bundle_root(start: &Path) -> Option<PathBuf> {
    // Resolve the start path to an absolute path
    let mut current = if start.is_absolute() {
        start.to_path_buf()
    } else {
        std::fs::canonicalize(start).unwrap_or_else(|_| {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("/"))
                .join(start)
        })
    };

    loop {
        if current.join("bundle.md").exists() || current.join("bundle.yaml").exists() {
            return Some(current);
        }

        match current.parent() {
            Some(parent) if parent != current => {
                current = parent.to_path_buf();
            }
            _ => break,
        }
    }

    None
}
