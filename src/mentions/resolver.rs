use std::collections::HashMap;
use std::path::PathBuf;

/// Base implementation of MentionResolver.
///
/// Supports patterns:
/// - `@bundle-name:context-name` -- From bundle's context namespace (returns None if not found)
/// - `@path` -- Relative to base_path (project/workspace directory)
/// - `@~/path` -- Relative to user's home directory
/// - `@./path` -- Explicit relative path from base_path
pub struct BaseMentionResolver {
    pub base_path: PathBuf,
    pub bundles: HashMap<String, PathBuf>,
}

impl Default for BaseMentionResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl BaseMentionResolver {
    /// Create a new resolver using CWD as base path.
    pub fn new() -> Self {
        Self {
            base_path: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            bundles: HashMap::new(),
        }
    }

    /// Create a resolver with an explicit base path.
    pub fn with_base_path(base_path: PathBuf) -> Self {
        Self {
            base_path,
            bundles: HashMap::new(),
        }
    }

    /// Create a resolver with bundle mappings.
    pub fn with_bundles(bundles: HashMap<String, PathBuf>) -> Self {
        Self {
            base_path: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            bundles,
        }
    }

    /// Resolve a mention string to a file path.
    /// Returns None if the file doesn't exist.
    pub fn resolve(&self, mention: &str) -> Option<PathBuf> {
        if !mention.starts_with('@') {
            return None;
        }

        let mention_body = &mention[1..]; // Remove @ prefix

        // Pattern 1: @namespace:path — resolve relative to namespace base path
        if mention_body.contains(':') {
            let (namespace, rel_path) = match mention_body.split_once(':') {
                Some((ns, path)) => (ns, path),
                None => return None,
            };
            if let Some(ns_base) = self.bundles.get(namespace) {
                let path = ns_base.join(rel_path);
                if path.exists() {
                    return Some(path);
                }
                // Try with .md extension
                let path_md = ns_base.join(format!("{rel_path}.md"));
                if path_md.exists() {
                    return Some(path_md);
                }
            }
            return None;
        }

        // Pattern 2: @~/path (home directory)
        if mention_body.starts_with('~') {
            let home = dirs::home_dir()?;
            let rest = mention_body.strip_prefix("~/").unwrap_or(mention_body);
            let path = home.join(rest);
            if path.exists() {
                return Some(path);
            }
            // Try with .md extension
            let path_md = home.join(format!("{rest}.md"));
            if path_md.exists() {
                return Some(path_md);
            }
            return None;
        }

        // Pattern 3: @./path or @path (relative to base_path)
        let path = self.base_path.join(mention_body);
        if path.exists() {
            return Some(path);
        }

        // Try with .md extension
        let path_md = self.base_path.join(format!("{mention_body}.md"));
        if path_md.exists() {
            return Some(path_md);
        }

        None
    }
}

impl super::MentionResolver for BaseMentionResolver {
    fn resolve(&self, mention: &str) -> Option<PathBuf> {
        self.resolve(mention)
    }
}
