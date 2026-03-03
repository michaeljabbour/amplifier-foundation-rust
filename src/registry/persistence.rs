//! Registry persistence: save/load to disk, path validation, relationship recording.

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
    pub fn validate_cached_paths(&mut self) {
        let has_stale = {
            let bundles = self.bundles.get_mut().unwrap_or_else(|e| e.into_inner());
            let stale_names: Vec<String> = bundles
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

            for name in &stale_names {
                if let Some(state) = bundles.get_mut(name) {
                    state.local_path = None;
                }
            }
            !stale_names.is_empty()
        };

        if has_stale {
            self.save();
        }
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
    /// deduplicating entries. Persists the updated state to disk immediately.
    ///
    /// Port of Python `_record_include_relationships`.
    pub fn record_include_relationships(&self, parent_name: &str, child_names: &[String]) {
        self.record_include_relationships_deferred(parent_name, child_names);
        self.save();
    }

    /// Persist registry to disk as JSON.
    pub fn save(&self) {
        let _ = std::fs::create_dir_all(&self.home);
        let registry_path = self.home.join("registry.json");

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

        if let Ok(content) = serde_json::to_string_pretty(&data) {
            let _ = std::fs::write(&registry_path, content);
        }
    }

    /// Load persisted state from registry.json.
    pub(super) fn load_persisted_state(&mut self) {
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
            let map = self.bundles.get_mut().unwrap_or_else(|e| e.into_inner());
            for (name, bundle_data) in bundles {
                map.insert(name.clone(), BundleState::from_dict(name, bundle_data));
            }
        }
    }
}
