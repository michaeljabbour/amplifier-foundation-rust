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
#[derive(Debug, Clone)]
pub struct BundleState {
    pub uri: String,
    pub name: String,
    pub version: Option<String>,
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
    fn compose_includes<'a>(
        &'a self,
        bundle: Bundle,
        loading_chain: &'a HashSet<String>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = crate::error::Result<Bundle>> + 'a>>
    {
        Box::pin(async move {
            let includes = bundle.includes.clone();
            let mut loaded_includes: Vec<Bundle> = Vec::new();

            for include in &includes {
                let include_uri = match include.as_str() {
                    Some(uri) => uri.to_string(),
                    None => continue,
                };

                match self
                    .load_single_with_chain(&include_uri, loading_chain)
                    .await
                {
                    Ok(included_bundle) => {
                        loaded_includes.push(included_bundle);
                    }
                    Err(crate::error::BundleError::DependencyError(msg)) => {
                        tracing::warn!("Skipping circular dependency: {}", msg);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to load include {}: {}", include_uri, e);
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

/// Extract a human-readable name from a URI.
fn extract_bundle_name(uri: &str) -> String {
    // Try to get the last path segment
    uri.rsplit('/')
        .find(|s| !s.is_empty())
        .unwrap_or("unknown")
        .to_string()
}

pub async fn load_bundle(uri: &str) -> crate::error::Result<Bundle> {
    let home = crate::paths::uri::get_amplifier_home();
    let registry = BundleRegistry::new(home);
    registry.load_single(uri).await
}
