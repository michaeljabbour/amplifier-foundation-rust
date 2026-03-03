use std::path::{Path, PathBuf};

/// Construct path to an agent file.
///
/// Looks in agents/ subdirectory, appends .md if not present.
pub fn construct_agent_path(base: &Path, name: &str) -> PathBuf {
    if name.ends_with(".md") {
        base.join("agents").join(name)
    } else {
        base.join("agents").join(format!("{name}.md"))
    }
}

/// Construct path to a bundle resource file.
///
/// Name is relative to bundle root. Empty name returns base.
/// Strips leading "/" to prevent absolute path creation.
pub fn construct_context_path(base: &Path, name: &str) -> PathBuf {
    let name = name.trim_start_matches('/');
    if name.is_empty() {
        base.to_path_buf()
    } else {
        base.join(name)
    }
}
