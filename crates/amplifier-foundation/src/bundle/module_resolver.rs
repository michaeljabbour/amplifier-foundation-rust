//! Module resolution for prepared bundles.
//!
//! Provides [`BundleModuleSource`] (simple path wrapper) and [`BundleModuleResolver`]
//! (module ID to path resolution with optional lazy activation).
//!
//! Ported from Python's `BundleModuleSource` and `BundleModuleResolver` classes
//! in `bundle.py`.
//!
//! # Migration Note
//!
//! Python's `profile_hint` parameter (deprecated in Python, marked for removal
//! in v2.0) is not ported. Callers should pass the activation URI as
//! `source_hint` directly.

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;

/// Trait for lazy module activation.
///
/// Implementors provide the mechanism to download, install, and activate
/// a module from a source URI. Used by [`BundleModuleResolver`] for
/// on-demand module activation.
///
/// # Note
///
/// The `&self` receiver uses interior mutability (e.g., `Mutex`) when
/// the implementor needs to track state. This enables `Arc<dyn ModuleActivate>`
/// sharing across async tasks.
#[async_trait]
pub trait ModuleActivate: Send + Sync {
    /// Activate a module by downloading and making it available.
    ///
    /// # Arguments
    ///
    /// * `module_name` — Name of the module (e.g., "tool-bash").
    /// * `source_uri` — URI to download from (e.g., "git+https://...").
    ///
    /// # Returns
    ///
    /// Local path to the activated module.
    async fn activate(&self, module_name: &str, source_uri: &str) -> crate::Result<PathBuf>;
}

/// Simple module source that returns a pre-resolved path.
///
/// This is a thin wrapper around a `PathBuf`, implementing the module source
/// protocol. Created by [`BundleModuleResolver::resolve`] and returned to
/// the kernel's module loading system.
#[derive(Debug, Clone)]
pub struct BundleModuleSource {
    path: PathBuf,
}

impl BundleModuleSource {
    /// Create a new module source with the given path.
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Return the pre-resolved module path.
    pub fn resolve(&self) -> &Path {
        &self.path
    }
}

/// Module resolver for prepared bundles with lazy activation support.
///
/// Maps module IDs to their activated paths. Implements the kernel's
/// ModuleSourceResolver protocol.
///
/// Supports on-demand module activation for agent-specific modules that
/// weren't in the parent bundle's initial activation set.
///
/// # Sync vs Async Resolution
///
/// - [`resolve`](Self::resolve): Sync. Only checks the in-memory map.
///   Returns an error if the module is not already activated.
/// - [`async_resolve`](Self::async_resolve): Async. Falls back to lazy
///   activation via the optional [`ModuleActivate`] implementor.
///
/// # Thread Safety
///
/// The paths map uses `std::sync::Mutex` for both sync and async access.
/// Lazy activation is serialized by a `tokio::sync::Mutex<()>` to prevent
/// duplicate activations when multiple tasks request the same module
/// concurrently (double-checked locking pattern).
///
/// Mutex poisoning is recovered from via `unwrap_or_else(|e| e.into_inner())`
/// to prevent cascade panics in multi-threaded server contexts.
pub struct BundleModuleResolver {
    /// Module paths protected by std::sync::Mutex for sync and async access.
    /// The lock is held only briefly (HashMap lookup/insert), never across await points.
    paths: Mutex<HashMap<String, PathBuf>>,
    activator: Option<Arc<dyn ModuleActivate>>,
    /// Serializes lazy activation to prevent duplicate activations.
    activation_lock: tokio::sync::Mutex<()>,
}

impl fmt::Debug for BundleModuleResolver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let paths = self.lock_paths();
        f.debug_struct("BundleModuleResolver")
            .field("module_count", &paths.len())
            .field("modules", &paths.keys().collect::<Vec<_>>())
            .field(
                "activator",
                &if self.activator.is_some() {
                    "Some(<activator>)"
                } else {
                    "None"
                },
            )
            .finish()
    }
}

impl BundleModuleResolver {
    /// Create a new resolver with pre-activated module paths.
    ///
    /// # Arguments
    ///
    /// * `module_paths` — Map of module ID to local path.
    /// * `activator` — Optional activator for lazy activation of missing modules.
    pub fn new(
        module_paths: HashMap<String, PathBuf>,
        activator: Option<Arc<dyn ModuleActivate>>,
    ) -> Self {
        Self {
            paths: Mutex::new(module_paths),
            activator,
            activation_lock: tokio::sync::Mutex::new(()),
        }
    }

    /// Lock paths with poison recovery.
    fn lock_paths(&self) -> std::sync::MutexGuard<'_, HashMap<String, PathBuf>> {
        self.paths.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Get sorted list of available module IDs (for deterministic error messages).
    fn available_modules(paths: &HashMap<String, PathBuf>) -> Vec<String> {
        let mut keys: Vec<String> = paths.keys().cloned().collect();
        keys.sort();
        keys
    }

    /// Resolve module ID to source (sync).
    ///
    /// Only checks the in-memory map. Does not perform lazy activation.
    /// Use [`async_resolve`](Self::async_resolve) for lazy activation support.
    ///
    /// # Arguments
    ///
    /// * `module_id` — Module identifier (e.g., "tool-bash").
    /// * `_source_hint` — Ignored in sync resolution. Accepted for API compatibility.
    ///
    /// # Errors
    ///
    /// Returns [`BundleError::LoadError`](crate::BundleError::LoadError) if the
    /// module is not in the activated paths.
    pub fn resolve(
        &self,
        module_id: &str,
        _source_hint: Option<&str>,
    ) -> crate::Result<BundleModuleSource> {
        let paths = self.lock_paths();
        match paths.get(module_id) {
            Some(path) => Ok(BundleModuleSource::new(path.clone())),
            None => {
                let available = Self::available_modules(&paths);
                Err(crate::BundleError::LoadError {
                    reason: format!(
                        "Module '{}' not found in prepared bundle. \
                         Available modules: {:?}. \
                         Use async_resolve() for lazy activation support.",
                        module_id, available
                    ),
                    source: None,
                })
            }
        }
    }

    /// Resolve module ID to source with lazy activation support (async).
    ///
    /// Fast path: returns immediately if the module is already activated.
    /// Lazy path: activates the module via the [`ModuleActivate`] implementor
    /// if one was provided at construction time.
    ///
    /// # Arguments
    ///
    /// * `module_id` — Module identifier (e.g., "tool-bash").
    /// * `source_hint` — Optional source URI for lazy activation.
    ///
    /// # Migration Note
    ///
    /// Python's deprecated `profile_hint` parameter is not ported.
    /// Callers should pass the activation URI as `source_hint` directly.
    ///
    /// # Errors
    ///
    /// Returns [`BundleError::LoadError`](crate::BundleError::LoadError) if:
    /// - Module not found and no activator is available
    /// - Module not found and no source hint provided
    /// - Lazy activation fails (original error chained via `source` field)
    pub async fn async_resolve(
        &self,
        module_id: &str,
        source_hint: Option<&str>,
    ) -> crate::Result<BundleModuleSource> {
        // Fast path: already activated (brief lock, no await)
        {
            let paths = self.lock_paths();
            if let Some(path) = paths.get(module_id) {
                return Ok(BundleModuleSource::new(path.clone()));
            }
        }

        // Lazy activation path
        let activator = match &self.activator {
            Some(a) => Arc::clone(a),
            None => {
                let paths = self.lock_paths();
                let available = Self::available_modules(&paths);
                return Err(crate::BundleError::LoadError {
                    reason: format!(
                        "Module '{}' not found in prepared bundle and no activator available. \
                         Available modules: {:?}",
                        module_id, available
                    ),
                    source: None,
                });
            }
        };

        let hint = match source_hint {
            Some(h) if !h.is_empty() => h,
            _ => {
                let paths = self.lock_paths();
                let available = Self::available_modules(&paths);
                return Err(crate::BundleError::LoadError {
                    reason: format!(
                        "Module '{}' not found and no source hint provided for activation. \
                         Available modules: {:?}",
                        module_id, available
                    ),
                    source: None,
                });
            }
        };

        // Serialize activations to prevent duplicate downloads
        let _guard = self.activation_lock.lock().await;

        // Double-check after acquiring activation lock (another task may have activated)
        {
            let paths = self.lock_paths();
            if let Some(path) = paths.get(module_id) {
                return Ok(BundleModuleSource::new(path.clone()));
            }
        }

        tracing::info!(
            module_id = module_id,
            source = hint,
            "Lazy activating module"
        );

        match activator.activate(module_id, hint).await {
            Ok(module_path) => {
                tracing::info!(
                    module_id = module_id,
                    path = %module_path.display(),
                    "Successfully activated module"
                );
                let mut paths = self.lock_paths();
                paths.insert(module_id.to_string(), module_path.clone());
                Ok(BundleModuleSource::new(module_path))
            }
            Err(e) => {
                tracing::error!(
                    module_id = module_id,
                    error = %e,
                    "Failed to lazy-activate module"
                );
                Err(crate::BundleError::LoadError {
                    reason: format!(
                        "Module '{}' not found and activation failed: {}",
                        module_id, e
                    ),
                    source: Some(Box::new(e)),
                })
            }
        }
    }

    /// Get module source path as string.
    ///
    /// Provides compatibility with `StandardModuleSourceResolver`'s
    /// `get_module_source()` interface used by some app layers.
    ///
    /// # Returns
    ///
    /// String path to module, or `None` if not found.
    pub fn get_module_source(&self, module_id: &str) -> Option<String> {
        let paths = self.lock_paths();
        paths
            .get(module_id)
            .map(|p| p.to_string_lossy().into_owned())
    }
}
