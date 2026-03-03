//! Bundle registry: central management of bundle state, loading, persistence,
//! include resolution, and update lifecycle.
//!
//! This module is decomposed into submodules by responsibility:
//! - `types`: Data types (`UpdateInfo`, `BundleState`)
//! - `helpers`: Free-standing utility functions (`parse_include`, `find_resource_path`, etc.)
//! - `persistence`: Save/load to disk, path validation, relationship recording
//! - `includes`: Include resolution and composition
//! - `loader`: Bundle loading pipeline (URI → disk → Bundle)
//! - `updates`: Update check/apply lifecycle

mod helpers;
mod includes;
mod loader;
mod persistence;
mod types;
mod updates;

// Re-export public types
pub use helpers::{extract_bundle_name, find_resource_path, parse_include};
pub use loader::load_bundle;
pub use types::{BundleState, UpdateInfo};

use crate::bundle::Bundle;
use indexmap::IndexMap;
use std::collections::HashMap;
use std::path::PathBuf;

/// Central bundle management.
///
/// Uses `IndexMap` for `bundles` to ensure deterministic ordering in
/// serialized output (registry.json). Insertion order is preserved.
pub struct BundleRegistry {
    home: PathBuf,
    /// Lock ordering: always acquire `bundles` before `cache`.
    /// Never hold either lock across an `.await` point or a `self.method()` call
    /// that may acquire locks internally.
    bundles: std::sync::RwLock<IndexMap<String, BundleState>>,
    cache: std::sync::Mutex<HashMap<String, Bundle>>,
}

impl BundleRegistry {
    pub fn new(home: PathBuf) -> Self {
        let mut registry = BundleRegistry {
            home,
            bundles: std::sync::RwLock::new(IndexMap::new()),
            cache: std::sync::Mutex::new(HashMap::new()),
        };
        registry.load_persisted_state();
        registry
    }

    /// Register bundles by name→URI mapping.
    /// Does NOT persist -- caller must call save().
    pub fn register(&mut self, bundles: &HashMap<String, String>) {
        let map = self.bundles.get_mut().unwrap_or_else(|e| e.into_inner());
        for (name, uri) in bundles {
            if let Some(existing) = map.get_mut(name) {
                existing.uri = uri.clone();
            } else {
                map.insert(name.clone(), BundleState::new(name, uri));
            }
        }
    }

    /// Unregister a bundle by name. Returns true if found and removed.
    /// Performs bidirectional relationship cleanup.
    /// Does NOT persist -- caller must call save().
    pub fn unregister(&mut self, name: &str) -> bool {
        let map = self.bundles.get_mut().unwrap_or_else(|e| e.into_inner());
        let state = match map.shift_remove(name) {
            Some(s) => s,
            None => return false,
        };

        // Clean up forward refs: remove name from each child's included_by
        for child_name in &state.includes {
            if let Some(child) = map.get_mut(child_name) {
                child.included_by.retain(|n| n != name);
            }
        }

        // Clean up backward refs: remove name from each parent's includes
        for parent_name in &state.included_by {
            if let Some(parent) = map.get_mut(parent_name) {
                parent.includes.retain(|n| n != name);
            }
        }

        true
    }

    /// Look up URI for a registered bundle name.
    pub fn find(&self, name: &str) -> Option<String> {
        self.bundles
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(name)
            .map(|state| state.uri.clone())
    }

    /// List all registered bundle names (sorted).
    pub fn list_registered(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .bundles
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .keys()
            .cloned()
            .collect();
        names.sort();
        names
    }

    /// Get mutable reference to a bundle's state.
    /// Creates a default state if the name isn't registered.
    pub fn get_state(&mut self, name: &str) -> &mut BundleState {
        self.bundles
            .get_mut()
            .unwrap_or_else(|e| e.into_inner())
            .entry(name.to_string())
            .or_insert_with(|| BundleState::new(name, ""))
    }

    /// Get all tracked states as a name → BundleState map (cloned snapshot).
    pub fn get_all_states(&self) -> IndexMap<String, BundleState> {
        self.bundles
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Get a cloned snapshot of a bundle's state.
    ///
    /// Returns `None` if the name isn't registered. Unlike `get_state()`,
    /// this does not create a default entry.
    pub fn find_state(&self, name: &str) -> Option<BundleState> {
        self.bundles
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(name)
            .cloned()
    }
}
