// Note: The architecture spec lists registry/includes.rs and registry/persistence.rs
// as separate files. In practice, include resolution (compose_includes, cycle detection)
// and persistence (save/load, BundleState to_dict/from_dict) are implemented directly
// in this file because they are tightly coupled to BundleRegistry internals.

use crate::bundle::Bundle;
use indexmap::IndexMap;
use serde_yaml_ng::{Mapping, Value};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Information about an available update for a registered bundle.
///
/// Returned by registry update-checking operations. Matches Python's
/// `UpdateInfo` dataclass in `registry.py`.
///
/// This is the **bundle-level** update notification, produced when the registry
/// determines that a newer version is available. It is distinct from
/// [`SourceStatus`](crate::sources::SourceStatus) which is a **source-level**
/// status check (may be unknown/tri-state). `UpdateInfo` represents a *confirmed*
/// update — `available_version` is always known (non-optional).
///
/// Currently a data-only struct. Will be returned by `BundleRegistry` update-checking
/// methods when full update workflow is implemented.
///
/// # Examples
///
/// ```
/// use amplifier_foundation::UpdateInfo;
///
/// let info = UpdateInfo {
///     name: "my-bundle".to_string(),
///     current_version: Some("1.0.0".to_string()),
///     available_version: "2.0.0".to_string(),
///     uri: "git+https://github.com/org/my-bundle@main".to_string(),
/// };
/// assert_eq!(info.name, "my-bundle");
/// assert_eq!(info.current_version.as_deref(), Some("1.0.0"));
/// assert_eq!(info.available_version, "2.0.0");
/// assert!(info.uri.starts_with("git+"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct UpdateInfo {
    /// Name of the bundle with an update available.
    pub name: String,
    /// Currently installed version, if known.
    pub current_version: Option<String>,
    /// Version available for update (always known for confirmed updates).
    pub available_version: String,
    /// Source URI of the bundle.
    pub uri: String,
}

/// Tracked state for a registered bundle.
///
/// Terminology:
///   Root bundle: A bundle at /bundle.md or /bundle.yaml at the root of a repo
///       or directory tree. Establishes the namespace and root directory for
///       path resolution. Tracked via is_root=True.
///
///   Nested bundle: A bundle loaded via #subdirectory= URIs or @namespace:path
///       references. Shares the namespace with its root bundle and resolves
///       paths relative to its own location. Tracked via is_root=False.
#[derive(Debug, Clone)]
pub struct BundleState {
    pub uri: String,
    pub name: String,
    pub version: Option<String>,
    /// When this bundle was last loaded (ISO 8601 string).
    /// Stored as String to avoid forcing a chrono dependency on consumers.
    pub loaded_at: Option<String>,
    /// When this bundle was last checked for updates (ISO 8601 string).
    pub checked_at: Option<String>,
    pub local_path: Option<String>,
    pub includes: Vec<String>,
    pub included_by: Vec<String>,
    pub is_root: bool,
    pub root_name: Option<String>,
    pub explicitly_requested: bool,
    pub app_bundle: bool,
}

impl BundleState {
    pub fn new(name: &str, uri: &str) -> Self {
        BundleState {
            uri: uri.to_string(),
            name: name.to_string(),
            version: None,
            loaded_at: None,
            checked_at: None,
            local_path: None,
            includes: Vec::new(),
            included_by: Vec::new(),
            is_root: true,
            root_name: None,
            explicitly_requested: false,
            app_bundle: false,
        }
    }

    pub fn to_dict(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        map.insert(
            "uri".to_string(),
            serde_json::Value::String(self.uri.clone()),
        );
        map.insert(
            "name".to_string(),
            serde_json::Value::String(self.name.clone()),
        );
        if let Some(v) = &self.version {
            map.insert("version".to_string(), serde_json::Value::String(v.clone()));
        }
        if let Some(la) = &self.loaded_at {
            map.insert(
                "loaded_at".to_string(),
                serde_json::Value::String(la.clone()),
            );
        }
        if let Some(ca) = &self.checked_at {
            map.insert(
                "checked_at".to_string(),
                serde_json::Value::String(ca.clone()),
            );
        }
        if let Some(lp) = &self.local_path {
            map.insert(
                "local_path".to_string(),
                serde_json::Value::String(lp.clone()),
            );
        }
        map.insert("is_root".to_string(), serde_json::Value::Bool(self.is_root));
        map.insert(
            "explicitly_requested".to_string(),
            serde_json::Value::Bool(self.explicitly_requested),
        );
        map.insert(
            "app_bundle".to_string(),
            serde_json::Value::Bool(self.app_bundle),
        );
        if !self.includes.is_empty() {
            map.insert(
                "includes".to_string(),
                serde_json::Value::Array(
                    self.includes
                        .iter()
                        .map(|s| serde_json::Value::String(s.clone()))
                        .collect(),
                ),
            );
        }
        if !self.included_by.is_empty() {
            map.insert(
                "included_by".to_string(),
                serde_json::Value::Array(
                    self.included_by
                        .iter()
                        .map(|s| serde_json::Value::String(s.clone()))
                        .collect(),
                ),
            );
        }
        if let Some(rn) = &self.root_name {
            map.insert(
                "root_name".to_string(),
                serde_json::Value::String(rn.clone()),
            );
        }
        serde_json::Value::Object(map)
    }

    pub fn from_dict(name: &str, data: &serde_json::Value) -> Self {
        let obj = data.as_object();
        BundleState {
            uri: obj
                .and_then(|o| o.get("uri"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            name: name.to_string(),
            version: obj
                .and_then(|o| o.get("version"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            loaded_at: obj
                .and_then(|o| o.get("loaded_at"))
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string()),
            checked_at: obj
                .and_then(|o| o.get("checked_at"))
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string()),
            local_path: obj
                .and_then(|o| o.get("local_path"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            includes: obj
                .and_then(|o| o.get("includes"))
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default(),
            included_by: obj
                .and_then(|o| o.get("included_by"))
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default(),
            is_root: obj
                .and_then(|o| o.get("is_root"))
                .and_then(|v| v.as_bool())
                .unwrap_or(true),
            root_name: obj
                .and_then(|o| o.get("root_name"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            explicitly_requested: obj
                .and_then(|o| o.get("explicitly_requested"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            app_bundle: obj
                .and_then(|o| o.get("app_bundle"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        }
    }
}

/// Central bundle management.
///
/// Uses `IndexMap` for `bundles` to ensure deterministic ordering in
/// serialized output (registry.json). Insertion order is preserved.
pub struct BundleRegistry {
    home: PathBuf,
    bundles: IndexMap<String, BundleState>,
    cache: std::sync::Mutex<HashMap<String, Bundle>>,
}

impl BundleRegistry {
    pub fn new(home: PathBuf) -> Self {
        let mut registry = BundleRegistry {
            home,
            bundles: IndexMap::new(),
            cache: std::sync::Mutex::new(HashMap::new()),
        };
        registry.load_persisted_state();
        registry
    }

    /// Register bundles by name→URI mapping.
    /// Does NOT persist -- caller must call save().
    pub fn register(&mut self, bundles: &HashMap<String, String>) {
        for (name, uri) in bundles {
            if let Some(existing) = self.bundles.get_mut(name) {
                existing.uri = uri.clone();
            } else {
                self.bundles
                    .insert(name.clone(), BundleState::new(name, uri));
            }
        }
    }

    /// Unregister a bundle by name. Returns true if found and removed.
    /// Performs bidirectional relationship cleanup.
    /// Does NOT persist -- caller must call save().
    pub fn unregister(&mut self, name: &str) -> bool {
        let state = match self.bundles.shift_remove(name) {
            Some(s) => s,
            None => return false,
        };

        // Clean up forward refs: remove name from each child's included_by
        for child_name in &state.includes {
            if let Some(child) = self.bundles.get_mut(child_name) {
                child.included_by.retain(|n| n != name);
            }
        }

        // Clean up backward refs: remove name from each parent's includes
        for parent_name in &state.included_by {
            if let Some(parent) = self.bundles.get_mut(parent_name) {
                parent.includes.retain(|n| n != name);
            }
        }

        true
    }

    /// Look up URI for a registered bundle name.
    ///
    /// Returns the URI string if found, or `None` if not registered.
    pub fn find(&self, name: &str) -> Option<String> {
        self.bundles.get(name).map(|state| state.uri.clone())
    }

    /// List all registered bundle names (sorted).
    pub fn list_registered(&self) -> Vec<String> {
        let mut names: Vec<String> = self.bundles.keys().cloned().collect();
        names.sort();
        names
    }

    /// Get mutable reference to a bundle's state.
    /// Creates a default state if the name isn't registered.
    pub fn get_state(&mut self, name: &str) -> &mut BundleState {
        self.bundles
            .entry(name.to_string())
            .or_insert_with(|| BundleState::new(name, ""))
    }

    /// Get all tracked states as a name → BundleState map (read-only reference).
    ///
    /// Matches Python's `get_state(None)` which returns `dict(self._registry)`.
    ///
    /// **Divergence from Python:** Returns a reference to the internal map,
    /// not a shallow copy. Mutations require going through `get_state()`.
    pub fn get_all_states(&self) -> &IndexMap<String, BundleState> {
        &self.bundles
    }

    /// Get immutable reference to a bundle's state.
    ///
    /// Returns `None` if the name isn't registered. Unlike `get_state()`,
    /// this does not create a default entry. Matches Python's
    /// `get_state(name)` which returns `self._registry.get(name)`.
    pub fn find_state(&self, name: &str) -> Option<&BundleState> {
        self.bundles.get(name)
    }

    /// Clear stale `local_path` references from registry entries.
    ///
    /// On startup, registry entries may reference cached paths that no longer
    /// exist (e.g., user cleared cache but not registry.json). This clears
    /// those stale references so bundles will be re-fetched when needed.
    ///
    /// Persists the cleanup if any stale entries were found.
    pub fn validate_cached_paths(&mut self) {
        let stale_names: Vec<String> = self
            .bundles
            .iter()
            .filter_map(|(name, state)| {
                if let Some(ref lp) = state.local_path {
                    if !std::path::Path::new(lp).exists() {
                        tracing::info!("Clearing stale cache reference for '{}'", name);
                        return Some(name.clone());
                    }
                }
                None
            })
            .collect();

        if !stale_names.is_empty() {
            for name in &stale_names {
                if let Some(state) = self.bundles.get_mut(name) {
                    state.local_path = None;
                }
            }
            self.save();
        }
    }

    /// Record include relationships between a parent bundle and its children.
    ///
    /// Updates the parent's `includes` list and each child's `included_by` list,
    /// deduplicating entries. Persists the updated state to disk.
    ///
    /// Port of Python `_record_include_relationships`.
    pub fn record_include_relationships(&mut self, parent_name: &str, child_names: &[String]) {
        // Update parent's includes list
        if let Some(parent_state) = self.bundles.get_mut(parent_name) {
            for child_name in child_names {
                if !parent_state.includes.contains(child_name) {
                    parent_state.includes.push(child_name.clone());
                }
            }
        }

        // Update each child's included_by list
        let parent_owned = parent_name.to_string();
        for child_name in child_names {
            if let Some(child_state) = self.bundles.get_mut(child_name) {
                if !child_state.included_by.contains(&parent_owned) {
                    child_state.included_by.push(parent_owned.clone());
                }
            }
        }

        self.save();

        tracing::debug!(
            "Recorded include relationships: {} includes {:?}",
            parent_name,
            child_names
        );
    }

    /// Persist registry to disk as JSON.
    pub fn save(&self) {
        let _ = std::fs::create_dir_all(&self.home);
        let registry_path = self.home.join("registry.json");

        let mut bundles_map = serde_json::Map::new();
        for (name, state) in &self.bundles {
            bundles_map.insert(name.clone(), state.to_dict());
        }

        let data = serde_json::json!({
            "version": 1,
            "bundles": serde_json::Value::Object(bundles_map),
        });

        if let Ok(content) = serde_json::to_string_pretty(&data) {
            let _ = std::fs::write(&registry_path, content);
        }
    }

    /// Load persisted state from registry.json.
    fn load_persisted_state(&mut self) {
        let registry_path = self.home.join("registry.json");
        if !registry_path.exists() {
            return;
        }

        let content = match std::fs::read_to_string(&registry_path) {
            Ok(c) => c,
            Err(_) => return,
        };

        let data: serde_json::Value = match serde_json::from_str(&content) {
            Ok(d) => d,
            Err(_) => return,
        };

        if let Some(bundles) = data.get("bundles").and_then(|v| v.as_object()) {
            for (name, bundle_data) in bundles {
                self.bundles
                    .insert(name.clone(), BundleState::from_dict(name, bundle_data));
            }
        }
    }

    /// Walk up from `start` toward `stop`, looking for bundle.md or bundle.yaml.
    /// bundle.md is preferred over bundle.yaml. Returns None if not found.
    pub fn find_nearest_bundle_file(&self, start: &Path, stop: &Path) -> Option<PathBuf> {
        let mut current = start.to_path_buf();
        // Canonicalize stop for comparison (if possible)
        let stop_canonical = std::fs::canonicalize(stop).unwrap_or_else(|_| stop.to_path_buf());

        loop {
            let current_canonical =
                std::fs::canonicalize(&current).unwrap_or_else(|_| current.clone());

            // Check for bundle.md first (preferred)
            let bundle_md = current.join("bundle.md");
            if bundle_md.exists() {
                return Some(bundle_md);
            }

            // Then bundle.yaml
            let bundle_yaml = current.join("bundle.yaml");
            if bundle_yaml.exists() {
                return Some(bundle_yaml);
            }

            // Stop if we've reached the stop boundary
            if current_canonical == stop_canonical {
                break;
            }

            // Move up to parent
            match current.parent() {
                Some(parent) if parent != current => {
                    current = parent.to_path_buf();
                }
                _ => break, // Reached filesystem root
            }
        }

        None
    }

    /// Resolve an include source string to a loadable URI.
    ///
    /// Three-tier resolution:
    /// 1. Already a URI (`://` or starts with `git+`) → return as-is
    /// 2. `namespace:path` syntax → look up namespace in registry,
    ///    construct appropriate URI (file:// or git+...#subdirectory=)
    /// 3. Plain name → return as-is
    ///
    /// Returns `None` only when a `namespace:path` reference cannot be
    /// resolved (namespace not registered, or path not found within namespace).
    ///
    /// Port of Python `_resolve_include_source`.
    pub fn resolve_include_source(&self, source: &str) -> Option<String> {
        // Tier 1: Already a URI — return as-is
        if source.contains("://") || source.starts_with("git+") {
            return Some(source.to_string());
        }

        // Tier 2: namespace:path syntax (contains ':' but not '://')
        if source.contains(':') {
            let (namespace, rel_path) = source.split_once(':')?;

            let state = self.bundles.get(namespace)?;

            // Branch A: Git-based namespace (URI starts with "git+")
            if state.uri.starts_with("git+") {
                let base_uri = state.uri.split('#').next().unwrap_or(&state.uri);

                if let Some(ref local_path) = state.local_path {
                    // local_path exists — try to find resource on disk
                    let namespace_path = Path::new(local_path);
                    let resource_path = if namespace_path.is_file() {
                        namespace_path
                            .parent()
                            .unwrap_or(namespace_path)
                            .join(rel_path)
                    } else {
                        namespace_path.join(rel_path)
                    };

                    if let Some(found) = find_resource_path(&resource_path) {
                        // Compute relative path from namespace root for subdirectory fragment
                        let namespace_root = if namespace_path.is_file() {
                            namespace_path.parent().unwrap_or(namespace_path)
                        } else {
                            namespace_path
                        };
                        if let Ok(rel_from_root) = found.strip_prefix(namespace_root) {
                            return Some(format!(
                                "{}#subdirectory={}",
                                base_uri,
                                rel_from_root.display()
                            ));
                        }
                        // Fallback: use found path as relative (shouldn't happen if canonicalize worked)
                        return Some(format!("{}#subdirectory={}", base_uri, found.display()));
                    }

                    // Resource not found locally — return None (not a URI guess)
                    tracing::debug!(
                        "Namespace '{}' is git-based but path '{}' not found locally",
                        namespace,
                        rel_path
                    );
                    return None;
                }

                // No local_path yet (namespace being loaded) — construct URI directly
                tracing::debug!(
                    "Namespace '{}' has no local_path yet, constructing URI directly for '{}'",
                    namespace,
                    rel_path
                );
                return Some(format!("{}#subdirectory={}", base_uri, rel_path));
            }

            // Branch B: Non-git namespace — fall back to file:// path
            if let Some(ref local_path) = state.local_path {
                let namespace_path = Path::new(local_path);
                let resource_path = if namespace_path.is_file() {
                    namespace_path
                        .parent()
                        .unwrap_or(namespace_path)
                        .join(rel_path)
                } else {
                    namespace_path.join(rel_path)
                };

                if let Some(found) = find_resource_path(&resource_path) {
                    return Some(format!("file://{}", found.display()));
                }

                tracing::debug!(
                    "Namespace '{}' found but path '{}' not found within it",
                    namespace,
                    rel_path
                );
            } else {
                tracing::debug!("Namespace '{}' has no local_path", namespace);
            }

            return None;
        }

        // Tier 3: Plain name — return as-is for registry lookup
        Some(source.to_string())
    }

    /// Load a single bundle from a URI.
    /// Handles file:// URIs, subdirectory detection, and includes.
    pub async fn load_single(&self, uri: &str) -> crate::error::Result<Bundle> {
        self.load_single_with_chain(uri, &HashSet::new()).await
    }

    /// Internal: load with cycle detection chain.
    fn load_single_with_chain<'a>(
        &'a self,
        uri: &'a str,
        loading_chain: &'a HashSet<String>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = crate::error::Result<Bundle>> + 'a>>
    {
        Box::pin(async move {
            // Check cache
            if let Ok(cache) = self.cache.lock() {
                if let Some(bundle) = cache.get(uri) {
                    return Ok(bundle.clone());
                }
            }

            // Cycle detection
            if loading_chain.contains(uri) {
                tracing::warn!("Circular dependency detected: {}", uri);
                // Return a minimal bundle to break the cycle gracefully
                return Ok(Bundle::new(&extract_bundle_name(uri)));
            }

            // Resolve URI to local path
            let local_path = resolve_file_uri(uri)?;

            // Load bundle from disk
            let mut bundle = self.load_from_path(&local_path)?;

            // Detect subdirectory bundle
            let bundle_dir = if local_path.is_file() {
                local_path.parent().unwrap_or(&local_path).to_path_buf()
            } else {
                local_path.clone()
            };

            // Look for a root bundle ABOVE this one
            // Start searching from parent of bundle directory
            if let Some(parent_dir) = bundle_dir.parent() {
                if let Some(root_bundle_path) =
                    self.find_nearest_bundle_file(parent_dir, &self.home)
                {
                    let root_dir = root_bundle_path
                        .parent()
                        .unwrap_or(&root_bundle_path)
                        .to_path_buf();

                    // Only if root is in a DIFFERENT directory (not the same bundle)
                    if root_dir != bundle_dir {
                        // Load root bundle to get its name
                        if let Ok(root_bundle) = self.load_from_path(&root_bundle_path) {
                            // Map root namespace → root's directory (source_root)
                            bundle
                                .source_base_paths
                                .insert(root_bundle.name.clone(), root_dir.clone());

                            // Also map nested bundle name → root dir if different
                            if !bundle.name.is_empty() && bundle.name != root_bundle.name {
                                bundle
                                    .source_base_paths
                                    .insert(bundle.name.clone(), root_dir);
                            }
                        }
                    }
                }
            }

            // Set base_path
            bundle.base_path = Some(bundle_dir);

            // Handle includes recursively
            if !bundle.includes.is_empty() {
                let mut new_chain = loading_chain.clone();
                new_chain.insert(uri.to_string());

                bundle = self.compose_includes(bundle, &new_chain).await?;
            }

            // Cache the result
            if let Ok(mut cache) = self.cache.lock() {
                cache.insert(uri.to_string(), bundle.clone());
            }

            Ok(bundle)
        }) // end Box::pin(async move { ... })
    }

    /// Load a bundle from a local filesystem path.
    fn load_from_path(&self, path: &Path) -> crate::error::Result<Bundle> {
        if path.is_dir() {
            // Look for bundle.md first, then bundle.yaml
            let bundle_md = path.join("bundle.md");
            if bundle_md.exists() {
                return self.load_markdown_bundle(&bundle_md);
            }
            let bundle_yaml = path.join("bundle.yaml");
            if bundle_yaml.exists() {
                return self.load_yaml_bundle(&bundle_yaml);
            }
            return Err(crate::error::BundleError::NotFound {
                uri: path.display().to_string(),
            });
        }

        // It's a file
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        match ext {
            "md" => self.load_markdown_bundle(path),
            "yaml" | "yml" => self.load_yaml_bundle(path),
            _ => self.load_yaml_bundle(path), // default to YAML
        }
    }

    /// Load a YAML bundle file.
    fn load_yaml_bundle(&self, path: &Path) -> crate::error::Result<Bundle> {
        let content =
            std::fs::read_to_string(path).map_err(|e| crate::error::BundleError::LoadError {
                reason: format!("Failed to read bundle file: {}", path.display()),
                source: Some(Box::new(e)),
            })?;

        let raw: Value = serde_yaml_ng::from_str(&content)?;

        // Wrap in {"bundle": raw} to match from_dict expected format
        let mut wrapper = Mapping::new();
        wrapper.insert(Value::String("bundle".to_string()), raw);

        let base_path = path.parent().unwrap_or(path);
        Bundle::from_dict_with_base_path(&Value::Mapping(wrapper), base_path)
    }

    /// Load a markdown bundle file (with YAML frontmatter).
    fn load_markdown_bundle(&self, path: &Path) -> crate::error::Result<Bundle> {
        let content =
            std::fs::read_to_string(path).map_err(|e| crate::error::BundleError::LoadError {
                reason: format!("Failed to read bundle file: {}", path.display()),
                source: Some(Box::new(e)),
            })?;

        let (frontmatter, body) = crate::io::frontmatter::parse_frontmatter(&content)?;

        let base_path = path.parent().unwrap_or(path);

        // If frontmatter exists, use it; otherwise create minimal bundle
        let mut bundle = if let Some(fm) = frontmatter {
            let mut wrapper = Mapping::new();
            wrapper.insert(Value::String("bundle".to_string()), fm);
            Bundle::from_dict_with_base_path(&Value::Mapping(wrapper), base_path)?
        } else {
            let name = path
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");
            let mut b = Bundle::new(name);
            b.base_path = Some(base_path.to_path_buf());
            b
        };

        // Set instruction from markdown body
        let trimmed = body.trim();
        if !trimmed.is_empty() {
            bundle.instruction = Some(trimmed.to_string());
        }

        Ok(bundle)
    }

    /// Compose a bundle with its includes.
    ///
    /// Two-phase approach matching Python's `_compose_includes`:
    /// - **Phase 1:** Parse and resolve all include sources (using `parse_include`
    ///   and `resolve_include_source`)
    /// - **Phase 2:** Load all resolved includes sequentially
    ///
    /// Note: Python loads includes in parallel via `asyncio.gather`. This Rust
    /// implementation loads sequentially. Parallelism can be added later with
    /// `futures::join_all` if needed.
    ///
    /// **Known limitation:** `record_include_relationships` is not called
    /// automatically during composition because `compose_includes` takes `&self`
    /// while `record_include_relationships` requires `&mut self`. Callers with
    /// `&mut self` access should call `record_include_relationships` after loading
    /// to persist the include graph. This matches the architectural constraint
    /// that the load pipeline uses `&self` for the cache `Mutex` pattern.
    fn compose_includes<'a>(
        &'a self,
        bundle: Bundle,
        loading_chain: &'a HashSet<String>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = crate::error::Result<Bundle>> + 'a>>
    {
        Box::pin(async move {
            let includes = bundle.includes.clone();

            // Phase 1: Parse and resolve all include sources
            let mut include_sources: Vec<String> = Vec::new();
            for include in &includes {
                let source = match parse_include(include) {
                    Some(s) => s,
                    None => {
                        tracing::debug!("Skipping unparseable include: {:?}", include);
                        continue;
                    }
                };

                match self.resolve_include_source(&source) {
                    Some(uri) => include_sources.push(uri),
                    None => {
                        // Distinguish: namespace exists but path not found (error)
                        // vs namespace not registered (optional skip)
                        if source.contains(':') && !source.contains("://") {
                            let namespace = source.split(':').next().unwrap_or("");
                            if self.bundles.get(namespace).is_some() {
                                return Err(crate::error::BundleError::DependencyError(
                                    format!(
                                        "Include resolution failed: '{}'. Namespace '{}' is registered but the path doesn't exist.",
                                        source, namespace
                                    ),
                                ));
                            }
                        }
                        tracing::warn!("Include skipped (unregistered namespace): {}", source);
                    }
                }
            }

            if include_sources.is_empty() {
                return Ok(bundle);
            }

            // Phase 2: Load all resolved includes
            let mut loaded_includes: Vec<Bundle> = Vec::new();
            for include_uri in &include_sources {
                match self
                    .load_single_with_chain(include_uri, loading_chain)
                    .await
                {
                    Ok(included_bundle) => {
                        loaded_includes.push(included_bundle);
                    }
                    Err(crate::error::BundleError::DependencyError(msg)) => {
                        tracing::warn!("Skipping circular dependency: {}", msg);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to load include '{}': {}", include_uri, e);
                    }
                }
            }

            if loaded_includes.is_empty() {
                return Ok(bundle);
            }

            // Compose: includes first (as base), then bundle on top (bundle wins)
            // Python: includes[0].compose(includes[1])...compose(bundle)
            let mut result = loaded_includes.remove(0);
            let refs: Vec<&Bundle> = loaded_includes.iter().collect();
            if !refs.is_empty() {
                result = result.compose(&refs);
            }
            result = result.compose(&[&bundle]);

            Ok(result)
        }) // end Box::pin(async move { ... })
    }

    pub async fn load(&self, _uri: &str) -> crate::error::Result<Bundle> {
        self.load_single(_uri).await
    }
}

/// Resolve a file:// URI to a local filesystem path.
fn resolve_file_uri(uri: &str) -> crate::error::Result<PathBuf> {
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

pub async fn load_bundle(uri: &str) -> crate::error::Result<Bundle> {
    let home = crate::paths::uri::get_amplifier_home();
    let registry = BundleRegistry::new(home);
    registry.load_single(uri).await
}
