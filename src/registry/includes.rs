//! Include resolution and composition: find_nearest_bundle_file,
//! resolve_include_source, preload_namespace_bundles, compose_includes.

use super::helpers::{find_resource_path, parse_include, resolve_file_uri};
use super::BundleRegistry;
use crate::bundle::Bundle;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

impl BundleRegistry {
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

            // Clone needed fields from BundleState and drop the read lock
            // before any filesystem I/O (find_resource_path does Path::exists,
            // fs::canonicalize, etc.). This prevents holding the lock during
            // blocking syscalls.
            let (state_uri, state_local_path) = {
                let bundles = self.bundles.read().unwrap_or_else(|e| e.into_inner());
                let state = bundles.get(namespace)?;
                (state.uri.clone(), state.local_path.clone())
            };

            // Branch A: Git-based namespace (URI starts with "git+")
            if state_uri.starts_with("git+") {
                let base_uri = state_uri.split('#').next().unwrap_or(&state_uri);

                if let Some(ref local_path) = state_local_path {
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
            if let Some(ref local_path) = state_local_path {
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

    /// Pre-load namespace bundles to populate their `local_path` before
    /// resolving `namespace:path` includes.
    ///
    /// Port of Python `_preload_namespace_bundles`.
    pub(super) fn preload_namespace_bundles<'a>(
        &'a self,
        includes: &'a [serde_yaml_ng::Value],
        loading_chain: &'a HashSet<String>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + 'a>> {
        Box::pin(async move {
            let mut namespaces_to_load: HashSet<String> = HashSet::new();

            for include in includes {
                let source = match parse_include(include) {
                    Some(s) => s,
                    None => continue,
                };

                // Check for namespace:path syntax (but not URIs like git+https://)
                if source.contains(':') && !source.contains("://") {
                    let namespace = match source.split_once(':') {
                        Some((ns, _)) => ns,
                        None => continue,
                    };

                    // Look up namespace state
                    let needs_preload = {
                        let bundles = self.bundles.read().unwrap_or_else(|e| e.into_inner());
                        if let Some(state) = bundles.get(namespace) {
                            // Only preload if registered but has no local_path yet
                            if state.local_path.is_none() {
                                // Skip if namespace URI is already in loading chain
                                let uri = &state.uri;
                                let base_uri = uri.split('#').next().unwrap_or(uri);
                                let in_chain =
                                    loading_chain.contains(uri) || loading_chain.contains(base_uri);
                                if in_chain {
                                    tracing::debug!(
                                        "Skipping preload of '{}' - already in loading chain",
                                        namespace
                                    );
                                    false
                                } else {
                                    true
                                }
                            } else {
                                false // Already has local_path
                            }
                        } else {
                            false // Not registered
                        }
                    };

                    if needs_preload {
                        namespaces_to_load.insert(namespace.to_string());
                    }
                }
            }

            // Load namespace bundles to populate their local_path
            for namespace in &namespaces_to_load {
                // Look up the URI for this namespace
                let uri = {
                    let bundles = self.bundles.read().unwrap_or_else(|e| e.into_inner());
                    bundles.get(namespace).map(|s| s.uri.clone())
                };
                let uri = match uri {
                    Some(u) => u,
                    None => continue,
                };

                tracing::debug!("Pre-loading namespace bundle: {}", namespace);

                // Lightweight preload: resolve URI to path, load bundle,
                // and update local_path — WITHOUT processing the namespace
                // bundle's own includes (matches Python's auto_include=False).
                match resolve_file_uri(&uri) {
                    Ok(local_path) => match self.load_from_path(&local_path) {
                        Ok(bundle) => {
                            let bundle_dir = if local_path.is_file() {
                                local_path.parent().unwrap_or(&local_path).to_path_buf()
                            } else {
                                local_path.clone()
                            };
                            // Update local_path on the registry state
                            let local_str = bundle_dir.display().to_string();
                            let now = chrono::Utc::now().to_rfc3339();
                            {
                                let mut bundles =
                                    self.bundles.write().unwrap_or_else(|e| e.into_inner());
                                if let Some(state) = bundles.get_mut(namespace) {
                                    state.local_path = Some(local_str);
                                    state.loaded_at = Some(now);
                                }
                            }
                            // Also cache the bundle for subsequent load_single_with_chain
                            if let Ok(mut cache) = self.cache.lock() {
                                cache.insert(uri.clone(), bundle);
                            }
                            tracing::debug!("Pre-loaded namespace '{}' successfully", namespace);
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Cannot resolve includes: namespace '{}' failed to load: {}",
                                namespace,
                                e
                            );
                        }
                    },
                    Err(e) => {
                        tracing::warn!(
                            "Cannot resolve includes: namespace '{}' URI resolution failed: {}",
                            namespace,
                            e
                        );
                    }
                }
            }
        })
    }

    /// Two-phase include composition matching Python's `_compose_includes`.
    ///
    /// Before resolving includes, calls `preload_namespace_bundles` to ensure
    /// namespace bundles have their `local_path` populated for path resolution.
    ///
    /// After successful include loading, automatically calls
    /// `record_include_relationships` to persist the include graph.
    pub(super) fn compose_includes<'a>(
        &'a self,
        bundle: Bundle,
        loading_chain: &'a HashSet<String>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = crate::error::Result<Bundle>> + 'a>>
    {
        Box::pin(async move {
            let includes = bundle.includes.clone();

            // Pre-load namespace bundles to populate their local_path
            // before resolving namespace:path includes (F-058).
            self.preload_namespace_bundles(&includes, loading_chain)
                .await;

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
                            if self
                                .bundles
                                .read()
                                .unwrap_or_else(|e| e.into_inner())
                                .get(namespace)
                                .is_some()
                            {
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

            // Record include relationships in registry state (deferred — no disk write).
            // The caller (load_single_with_chain) is responsible for calling save()
            // once after all includes are composed, avoiding O(depth) disk writes.
            // SAFETY: No self.bundles locks are held at this point.
            let parent_name = &bundle.name;
            if !parent_name.is_empty() {
                let included_names: Vec<String> = loaded_includes
                    .iter()
                    .filter(|b| !b.name.is_empty())
                    .map(|b| b.name.clone())
                    .collect();
                if !included_names.is_empty() {
                    self.record_include_relationships_deferred(parent_name, &included_names);
                }
            }

            // Compose: includes first (as base), then bundle on top (bundle wins)
            let mut result = loaded_includes.remove(0);
            let refs: Vec<&Bundle> = loaded_includes.iter().collect();
            if !refs.is_empty() {
                result = result.compose(&refs);
            }
            result = result.compose(&[&bundle]);

            Ok(result)
        })
    }
}
