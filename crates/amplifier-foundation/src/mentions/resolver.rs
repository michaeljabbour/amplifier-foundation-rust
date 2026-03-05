use async_trait::async_trait;
use indexmap::IndexMap;
use std::collections::HashMap;
use std::path::PathBuf;

/// Base implementation of MentionResolver.
///
/// Supports patterns:
/// - `@bundle-name:context-name` -- From bundle's context namespace
///   (checks context dict first, then falls back to base path join)
/// - `@path` -- Relative to base_path (project/workspace directory)
/// - `@~/path` -- Relative to user's home directory
/// - `@./path` -- Explicit relative path from base_path
///
/// The `context` field holds the composed bundle's context dict. When resolving
/// `@namespace:name`, the resolver first checks `context.get(name)` for an
/// exact match (matching Python's `Bundle.resolve_context_path` behavior),
/// then falls back to `bundles[namespace].join(name)`.
pub struct BaseMentionResolver {
    pub base_path: PathBuf,
    pub bundles: HashMap<String, PathBuf>,
    /// Shared context dict from the composed bundle.
    /// Uses `IndexMap` to match `Bundle.context` type and preserve insertion order
    /// (Python dict semantics).
    ///
    /// When resolving `@namespace:name`, this dict is checked first for an
    /// exact match on `name` — the lookup is **namespace-agnostic**. This means
    /// `@foundation:overview` and `@otherns:overview` both resolve to the same
    /// context-dict entry. This matches Python behavior where all bundle copies
    /// in the resolver share the SAME composed context dict.
    ///
    /// Populated by `BundleSystemPromptFactory` from the composed bundle's
    /// context field. Empty by default (preserves backward compatibility).
    pub context: IndexMap<String, PathBuf>,
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
            context: IndexMap::new(),
        }
    }

    /// Create a resolver with an explicit base path.
    pub fn with_base_path(base_path: PathBuf) -> Self {
        Self {
            base_path,
            bundles: HashMap::new(),
            context: IndexMap::new(),
        }
    }

    /// Create a resolver with bundle mappings.
    pub fn with_bundles(bundles: HashMap<String, PathBuf>) -> Self {
        Self {
            base_path: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            bundles,
            context: IndexMap::new(),
        }
    }

    /// Resolve a mention string to a file path.
    /// Returns None if the file doesn't exist.
    pub async fn resolve(&self, mention: &str) -> Option<PathBuf> {
        if !mention.starts_with('@') {
            return None;
        }

        let mention_body = &mention[1..]; // Remove @ prefix

        // Pattern 1: @namespace:name — resolve via context dict then namespace base path
        if mention_body.contains(':') {
            let (namespace, rel_path) = match mention_body.split_once(':') {
                Some((ns, path)) => (ns, path),
                None => return None,
            };

            // Check if namespace is registered
            if self.bundles.contains_key(namespace) {
                // Step 1: Check context dict for exact match on rel_path
                // (matches Python's Bundle.resolve_context_path step 1)
                if let Some(path) = self.context.get(rel_path) {
                    return Some(path.clone());
                }

                // Step 2: Try namespace base path join with existence check.
                // Note: Python calls construct_context_path(base_path, name) which strips
                // leading '/' from name. Rust uses PathBuf::join which treats leading '/'
                // as absolute. This is a pre-existing divergence from F-016 (before F-063).
                // The .md extension fallback below is also a Rust-specific enhancement
                // that Python's resolve_context_path does not have.
                if let Some(ns_base) = self.bundles.get(namespace) {
                    let path = ns_base.join(rel_path);
                    if tokio::fs::metadata(&path).await.is_ok() {
                        return Some(path);
                    }
                    // Try with .md extension
                    let path_md = ns_base.join(format!("{rel_path}.md"));
                    if tokio::fs::metadata(&path_md).await.is_ok() {
                        return Some(path_md);
                    }
                }
            }
            return None;
        }

        // Pattern 2: @~/path (home directory)
        if mention_body.starts_with('~') {
            let home = dirs::home_dir()?;
            let rest = mention_body.strip_prefix("~/").unwrap_or(mention_body);
            let path = home.join(rest);
            if tokio::fs::metadata(&path).await.is_ok() {
                return Some(path);
            }
            // Try with .md extension
            let path_md = home.join(format!("{rest}.md"));
            if tokio::fs::metadata(&path_md).await.is_ok() {
                return Some(path_md);
            }
            return None;
        }

        // Pattern 3: @./path or @path (relative to base_path)
        let path = self.base_path.join(mention_body);
        if tokio::fs::metadata(&path).await.is_ok() {
            return Some(path);
        }

        // Try with .md extension
        let path_md = self.base_path.join(format!("{mention_body}.md"));
        if tokio::fs::metadata(&path_md).await.is_ok() {
            return Some(path_md);
        }

        None
    }
}

#[async_trait]
impl super::MentionResolver for BaseMentionResolver {
    async fn resolve(&self, mention: &str) -> Option<PathBuf> {
        self.resolve(mention).await
    }
}
