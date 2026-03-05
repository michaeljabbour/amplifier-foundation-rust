use std::path::{Path, PathBuf};

/// Sanitize a user-supplied path component by stripping traversal sequences.
///
/// Removes `../` and `..\\` sequences, and strips leading `/` and `\\`.
/// This prevents directory traversal attacks when the name is joined to a base path.
fn sanitize_name(name: &str) -> String {
    name.trim_start_matches('/')
        .trim_start_matches('\\')
        .replace("../", "")
        .replace("..\\", "")
        .replace("..", "")
}

/// Construct path to an agent file.
///
/// Looks in agents/ subdirectory, appends .md if not present.
/// Sanitizes name to prevent directory traversal.
pub fn construct_agent_path(base: &Path, name: &str) -> PathBuf {
    let safe_name = sanitize_name(name);
    if safe_name.ends_with(".md") {
        base.join("agents").join(safe_name)
    } else {
        base.join("agents").join(format!("{safe_name}.md"))
    }
}

/// Construct path to a bundle resource file.
///
/// Name is relative to bundle root. Empty name returns base.
/// Sanitizes name to prevent directory traversal.
pub fn construct_context_path(base: &Path, name: &str) -> PathBuf {
    let safe_name = sanitize_name(name);
    if safe_name.is_empty() {
        base.to_path_buf()
    } else {
        base.join(safe_name)
    }
}
