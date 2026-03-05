//! Update lifecycle: check_update_single/all, update_single/all.

use super::types::UpdateInfo;
use super::BundleRegistry;
use crate::bundle::Bundle;
use std::collections::HashMap;

impl BundleRegistry {
    /// Check a single bundle for updates.
    ///
    /// Updates the `checked_at` timestamp. Currently a stub that always
    /// returns `None` — no actual version comparison is performed.
    ///
    /// Port of Python `_check_update_single`.
    pub async fn check_update_single(&mut self, name: &str) -> Option<UpdateInfo> {
        let now = chrono::Utc::now().to_rfc3339();
        let found = {
            let bundles = self.bundles.get_mut().unwrap_or_else(|e| e.into_inner());
            if let Some(state) = bundles.get_mut(name) {
                state.checked_at = Some(now.clone());
                true
            } else {
                false
            }
        };
        if !found {
            return None;
        }
        tracing::debug!("Checked for updates: {} (checked_at={})", name, now);
        None // stub — no actual version comparison
    }

    /// Check all registered bundles for updates.
    ///
    /// Returns a list of available updates. Currently a stub that only
    /// updates `checked_at` timestamps.
    ///
    /// Port of Python `check_update` with `name=None`.
    pub async fn check_update_all(&mut self) -> Vec<UpdateInfo> {
        let names = self.list_registered();
        if names.is_empty() {
            return Vec::new();
        }

        let mut updates = Vec::new();
        for n in names {
            if let Some(info) = self.check_update_single(&n).await {
                updates.push(info);
            }
        }
        updates
    }

    /// Update a single registered bundle by reloading it.
    ///
    /// Bypasses the in-memory cache to force a fresh load from disk.
    /// Returns the reloaded `Bundle`. Updates `version`, `loaded_at`, and
    /// `checked_at` on the bundle's tracked state.
    ///
    /// Port of Python `_update_single`.
    pub async fn update_single(&mut self, name: &str) -> crate::error::Result<Bundle> {
        // Clone the URI before any async work to release the borrow on self.
        let uri = {
            let bundles = self.bundles.get_mut().unwrap_or_else(|e| e.into_inner());
            bundles
                .get(name)
                .ok_or_else(|| crate::error::BundleError::NotFound {
                    uri: format!("Bundle '{}' not registered", name),
                })?
                .uri
                .clone()
        };

        // Clear cache entry to force a fresh load (Python uses refresh=True)
        if let Ok(mut cache) = self.cache.lock() {
            cache.remove(&uri);
        }

        // Load the bundle. No borrow on self.bundles across await.
        let bundle = self.load_single(&uri).await?;

        // Update state timestamps
        let now = chrono::Utc::now().to_rfc3339();
        {
            let bundles = self.bundles.get_mut().unwrap_or_else(|e| e.into_inner());
            if let Some(state) = bundles.get_mut(name) {
                state.version = Some(bundle.version.clone());
                state.loaded_at = Some(now.clone());
                state.checked_at = Some(now);
            }
        }

        Ok(bundle)
    }

    /// Update all registered bundles by reloading them.
    ///
    /// Returns a map of name → `Bundle` for successfully updated bundles.
    /// Failures are logged as warnings and skipped.
    ///
    /// Port of Python `update` with `name=None`.
    pub async fn update_all(&mut self) -> HashMap<String, Bundle> {
        let names = self.list_registered();
        let mut bundles = HashMap::new();

        for name in names {
            match self.update_single(&name).await {
                Ok(bundle) => {
                    bundles.insert(name, bundle);
                }
                Err(e) => {
                    tracing::warn!("Failed to update bundle '{}': {}", name, e);
                }
            }
        }

        bundles
    }
}
