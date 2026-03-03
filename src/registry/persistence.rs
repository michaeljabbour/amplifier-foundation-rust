//! Registry persistence: save/load to disk, path validation, relationship recording.
//!
//! `save()` and `record_include_relationships()` are async, using `tokio::fs`
//! for non-blocking disk writes. `load_persisted_state()` remains sync since
//! it's called from `BundleRegistry::new()` (construction, before any async
//! runtime context is needed). `load_persisted_state_async()` is the async
//! counterpart, used by `BundleRegistry::new_async()`.

use super::types::BundleState;
use super::BundleRegistry;

impl BundleRegistry {
    /// Clear stale `local_path` references from registry entries.
    ///
    /// On startup, registry entries may reference cached paths that no longer
    /// exist (e.g., user cleared cache but not registry.json). This clears
    /// those stale references so bundles will be re-fetched when needed.
    ///
    /// Persists the cleanup if any stale entries were found.
    ///
    /// Takes `&self` (not `&mut self`) so it can be called on `Arc<BundleRegistry>`.
    /// Three-phase approach: read lock → async metadata checks → write lock.
    /// The read lock is dropped before any `.await` points.
    pub async fn validate_cached_paths(&self) {
        // Phase 1: Collect (name, local_path) pairs under read lock, then drop.
        let candidates: Vec<(String, String)> = {
            let bundles = self.bundles.read().unwrap_or_else(|e| e.into_inner());
            bundles
                .iter()
                .filter_map(|(name, state)| {
                    state
                        .local_path
                        .as_ref()
                        .map(|lp| (name.clone(), lp.clone()))
                })
                .collect()
        };

        if candidates.is_empty() {
            return;
        }

        // Phase 2: Concurrent async metadata checks (no lock held).
        // Uses futures::future::join_all to check all paths in parallel,
        // avoiding sequential spawn_blocking round-trips.
        // Safe to fan out unbounded: candidate count is bounded by registered
        // bundles (typically < 100), and this runs once at startup.
        let checks = candidates.iter().map(|(name, local_path)| {
            let name = name.clone();
            let local_path = local_path.clone();
            async move {
                let exists = tokio::fs::metadata(&local_path).await.is_ok();
                (name, exists)
            }
        });
        let results = futures::future::join_all(checks).await;

        let stale_names: Vec<String> = results
            .into_iter()
            .filter_map(|(name, exists)| {
                if !exists {
                    tracing::info!("Clearing stale cache reference for '{}'", name);
                    Some(name)
                } else {
                    None
                }
            })
            .collect();

        if stale_names.is_empty() {
            return;
        }

        // Phase 3: Mutate under write lock.
        {
            let mut bundles = self.bundles.write().unwrap_or_else(|e| e.into_inner());
            for name in &stale_names {
                if let Some(state) = bundles.get_mut(name) {
                    state.local_path = None;
                }
            }
        }

        self.save().await;
    }

    /// Record include relationships without persisting to disk.
    ///
    /// Updates the parent's `includes` list and each child's `included_by` list
    /// in memory only. The caller is responsible for calling `save()` when ready.
    ///
    /// This is the batch-optimized version — use when recording multiple
    /// relationships in a loop (e.g., recursive include loading) followed by
    /// a single `save()` at the end.
    pub fn record_include_relationships_deferred(&self, parent_name: &str, child_names: &[String]) {
        {
            let mut bundles = self.bundles.write().unwrap_or_else(|e| e.into_inner());

            // Update parent's includes list
            if let Some(parent_state) = bundles.get_mut(parent_name) {
                for child_name in child_names {
                    if !parent_state.includes.contains(child_name) {
                        parent_state.includes.push(child_name.clone());
                    }
                }
            }

            // Update each child's included_by list
            let parent_owned = parent_name.to_string();
            for child_name in child_names {
                if let Some(child_state) = bundles.get_mut(child_name) {
                    if !child_state.included_by.contains(&parent_owned) {
                        child_state.included_by.push(parent_owned.clone());
                    }
                }
            }
        }

        tracing::debug!(
            "Recorded include relationships (deferred): {} includes {:?}",
            parent_name,
            child_names
        );
    }

    /// Record include relationships between a parent bundle and its children.
    ///
    /// Updates the parent's `includes` list and each child's `included_by` list,
    /// deduplicating entries. Persists the updated state to disk immediately
    /// using async I/O.
    ///
    /// Port of Python `_record_include_relationships`.
    pub async fn record_include_relationships(&self, parent_name: &str, child_names: &[String]) {
        self.record_include_relationships_deferred(parent_name, child_names);
        self.save().await;
    }

    /// Persist registry to disk as JSON using non-blocking I/O.
    ///
    /// Uses `tokio::fs::create_dir_all` and `tokio::fs::write` to avoid
    /// blocking the async runtime. The bundles RwLock is held only briefly
    /// to serialize the state — it is dropped before any `.await` point.
    pub async fn save(&self) {
        if let Err(e) = tokio::fs::create_dir_all(&self.home).await {
            tracing::warn!(
                "Failed to create registry directory {}: {}",
                self.home.display(),
                e
            );
            return;
        }
        let registry_path = self.home.join("registry.json");

        // Serialize state under read lock, then drop lock before async write.
        let content = {
            let mut bundles_map = serde_json::Map::new();
            {
                let bundles = self.bundles.read().unwrap_or_else(|e| e.into_inner());
                for (name, state) in bundles.iter() {
                    bundles_map.insert(name.clone(), state.to_dict());
                }
            }

            let data = serde_json::json!({
                "version": 1,
                "bundles": serde_json::Value::Object(bundles_map),
            });

            match serde_json::to_string_pretty(&data) {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!("Failed to serialize registry state: {}", e);
                    return;
                }
            }
        };

        if let Err(e) = tokio::fs::write(&registry_path, content).await {
            tracing::warn!(
                "Failed to write registry file {}: {}",
                registry_path.display(),
                e
            );
        }
    }

    /// Apply parsed JSON content to the registry's bundle map.
    ///
    /// Shared implementation used by both `load_persisted_state` (sync) and
    /// `load_persisted_state_async` (async). The I/O differs between the two
    /// callers, but parsing and insertion logic is identical.
    ///
    /// Returns:
    /// - `Ok(())` if bundles were applied (including zero bundles from `{}`).
    /// - `Err(None)` if the "bundles" key is absent or not an object (valid
    ///   JSON, just no bundle data — not an error condition).
    /// - `Err(Some(e))` if JSON parsing failed (corrupt content).
    fn apply_persisted_content(&mut self, content: &str) -> Result<(), Option<serde_json::Error>> {
        let data: serde_json::Value = serde_json::from_str(content).map_err(Some)?;

        if let Some(bundles) = data.get("bundles").and_then(|v| v.as_object()) {
            let map = self.bundles.get_mut().unwrap_or_else(|e| e.into_inner());
            for (name, bundle_data) in bundles {
                map.insert(name.clone(), BundleState::from_dict(name, bundle_data));
            }
            Ok(())
        } else {
            Err(None)
        }
    }

    /// Load persisted state from registry.json.
    ///
    /// Remains sync because it's called from `BundleRegistry::new()`, which
    /// is a synchronous constructor (called before the async runtime is
    /// needed). For async contexts, use `load_persisted_state_async()`.
    pub(super) fn load_persisted_state(&mut self) {
        let registry_path = self.home.join("registry.json");
        if !registry_path.exists() {
            return;
        }

        let content = match std::fs::read_to_string(&registry_path) {
            Ok(c) => c,
            Err(_) => return,
        };

        // Silently swallow all errors (pre-existing behavior, matches Python).
        let _ = self.apply_persisted_content(&content);
    }

    /// Async version of [`load_persisted_state`](Self::load_persisted_state).
    ///
    /// Uses `tokio::fs` for non-blocking file I/O. Logs warnings on I/O and
    /// parse errors (unlike the sync version which silently swallows them).
    /// Missing "bundles" key is treated as valid (not an error).
    /// Returns empty state on any error (self-healing behavior matching Python).
    pub(super) async fn load_persisted_state_async(&mut self) {
        let registry_path = self.home.join("registry.json");

        let content = match tokio::fs::read_to_string(&registry_path).await {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
            Err(e) => {
                tracing::warn!(
                    "Failed to read registry file {}: {}",
                    registry_path.display(),
                    e
                );
                return;
            }
        };

        match self.apply_persisted_content(&content) {
            Ok(()) => {}
            Err(None) => {
                // Valid JSON but no "bundles" key — not an error (e.g., {"version": 1}).
            }
            Err(Some(e)) => {
                tracing::warn!(
                    "Failed to parse registry JSON {}: {}",
                    registry_path.display(),
                    e
                );
            }
        }
    }
}
