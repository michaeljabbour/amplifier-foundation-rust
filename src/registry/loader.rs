//! Bundle loading pipeline: load_single, load_from_path, load_yaml_bundle,
//! load_markdown_bundle, and the convenience load_bundle function.

use super::helpers::{extract_bundle_name, resolve_file_uri};
use super::BundleRegistry;
use crate::bundle::Bundle;
use serde_yaml_ng::{Mapping, Value};
use std::collections::HashSet;
use std::path::Path;

impl BundleRegistry {
    /// Load a single bundle from a URI.
    /// Handles file:// URIs, subdirectory detection, and includes.
    ///
    /// This is the public entry point that triggers a single batch save
    /// after the entire recursive include tree has been loaded. Internal
    /// mutations (state updates, include relationships) are deferred
    /// until this save, avoiding O(depth) disk writes.
    pub async fn load_single(&self, uri: &str) -> crate::error::Result<Bundle> {
        let bundle = self.load_single_with_chain(uri, &HashSet::new()).await?;
        // Single batch save after entire recursive tree completes.
        // Persists: deferred include relationships from compose_includes,
        // local_path/loaded_at updates from load_single_with_chain.
        self.save();
        Ok(bundle)
    }

    /// Internal: load with cycle detection chain.
    pub(super) fn load_single_with_chain<'a>(
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
            let mut bundle = self.load_from_path(&local_path).await?;

            // Detect subdirectory bundle
            let is_file = tokio::fs::metadata(&local_path)
                .await
                .map(|m| m.is_file())
                .unwrap_or(false);
            let bundle_dir = if is_file {
                local_path.parent().unwrap_or(&local_path).to_path_buf()
            } else {
                local_path.clone()
            };

            // Look for a root bundle ABOVE this one
            if let Some(parent_dir) = bundle_dir.parent() {
                if let Some(root_bundle_path) =
                    self.find_nearest_bundle_file(parent_dir, &self.home).await
                {
                    let root_dir = root_bundle_path
                        .parent()
                        .unwrap_or(&root_bundle_path)
                        .to_path_buf();

                    // Only if root is in a DIFFERENT directory (not the same bundle)
                    if root_dir != bundle_dir {
                        if let Ok(root_bundle) = self.load_from_path(&root_bundle_path).await {
                            bundle
                                .source_base_paths
                                .insert(root_bundle.name.clone(), root_dir.clone());

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
            bundle.base_path = Some(bundle_dir.clone());

            // Update registry state with local_path if bundle is registered.
            if !bundle.name.is_empty() {
                let local_str = bundle_dir.display().to_string();
                let now = chrono::Utc::now().to_rfc3339();
                let mut bundles = self.bundles.write().unwrap_or_else(|e| e.into_inner());
                if let Some(state) = bundles.get_mut(&bundle.name) {
                    state.local_path = Some(local_str);
                    state.loaded_at = Some(now);
                }
            }

            // Handle includes recursively
            if !bundle.includes.is_empty() {
                let mut new_chain = loading_chain.clone();
                new_chain.insert(uri.to_string());

                bundle = self.compose_includes(bundle, &new_chain).await?;
                // Note: compose_includes uses record_include_relationships_deferred
                // (no save). The public entry point load_single() is responsible
                // for a single batch save after the entire recursive tree completes.
            }

            // Cache the result
            if let Ok(mut cache) = self.cache.lock() {
                cache.insert(uri.to_string(), bundle.clone());
            }

            Ok(bundle)
        })
    }

    /// Load a bundle from a local filesystem path.
    ///
    /// Uses `tokio::fs` for non-blocking I/O (file reads and metadata checks).
    pub(super) async fn load_from_path(&self, path: &Path) -> crate::error::Result<Bundle> {
        let is_dir = tokio::fs::metadata(path)
            .await
            .map(|m| m.is_dir())
            .unwrap_or(false);

        if is_dir {
            let bundle_md = path.join("bundle.md");
            if tokio::fs::metadata(&bundle_md).await.is_ok() {
                return self.load_markdown_bundle(&bundle_md).await;
            }
            let bundle_yaml = path.join("bundle.yaml");
            if tokio::fs::metadata(&bundle_yaml).await.is_ok() {
                return self.load_yaml_bundle(&bundle_yaml).await;
            }
            return Err(crate::error::BundleError::NotFound {
                uri: path.display().to_string(),
            });
        }

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        match ext {
            "md" => self.load_markdown_bundle(path).await,
            "yaml" | "yml" => self.load_yaml_bundle(path).await,
            _ => self.load_yaml_bundle(path).await,
        }
    }

    /// Load a YAML bundle file using non-blocking I/O.
    async fn load_yaml_bundle(&self, path: &Path) -> crate::error::Result<Bundle> {
        let content = tokio::fs::read_to_string(path).await.map_err(|e| {
            crate::error::BundleError::LoadError {
                reason: format!("Failed to read bundle file: {}", path.display()),
                source: Some(Box::new(e)),
            }
        })?;

        let raw: Value = serde_yaml_ng::from_str(&content)?;

        let mut wrapper = Mapping::new();
        wrapper.insert(Value::String("bundle".to_string()), raw);

        let base_path = path.parent().unwrap_or(path);
        Bundle::from_dict_with_base_path(&Value::Mapping(wrapper), base_path)
    }

    /// Load a markdown bundle file (with YAML frontmatter) using non-blocking I/O.
    async fn load_markdown_bundle(&self, path: &Path) -> crate::error::Result<Bundle> {
        let content = tokio::fs::read_to_string(path).await.map_err(|e| {
            crate::error::BundleError::LoadError {
                reason: format!("Failed to read bundle file: {}", path.display()),
                source: Some(Box::new(e)),
            }
        })?;

        let (frontmatter, body) = crate::io::frontmatter::parse_frontmatter(&content)?;

        let base_path = path.parent().unwrap_or(path);

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

        let trimmed = body.trim();
        if !trimmed.is_empty() {
            bundle.instruction = Some(trimmed.to_string());
        }

        Ok(bundle)
    }

    pub async fn load(&self, _uri: &str) -> crate::error::Result<Bundle> {
        self.load_single(_uri).await
    }
}

/// Standalone convenience entry point for loading a bundle.
pub async fn load_bundle(uri: &str) -> crate::error::Result<Bundle> {
    let home = crate::paths::uri::get_amplifier_home();
    let registry = BundleRegistry::new(home);
    registry.load_single(uri).await
}
