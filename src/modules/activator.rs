//! Module activation for amplifier-foundation.
//!
//! Provides [`ModuleActivator`], which downloads modules from URIs and
//! makes them available for use. This implements the [`ModuleActivate`]
//! trait defined in [`crate::bundle::module_resolver`].
//!
//! Ported from Python's `modules/activator.py`.
//!
//! # Differences from Python
//!
//! - **No `sys.path` manipulation**: Rust doesn't have Python's import system.
//!   Callers are responsible for using the returned paths.
//! - **No hardcoded `uv pip install`**: Dependency installation is optional
//!   and uses `tokio::process::Command` with a configurable command.
//!   The default install command is `uv pip install -e <path> --quiet`.
//! - **Interior mutability via `std::sync::Mutex`**: The `ModuleActivate` trait
//!   requires `&self`, so mutable state is wrapped in `Mutex`.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use async_trait::async_trait;

use crate::bundle::module_resolver::ModuleActivate;
use crate::modules::state::InstallStateManager;
use crate::paths::uri::get_amplifier_home;
use crate::sources::resolver::SimpleSourceResolver;

/// Activate modules by downloading and making them available.
///
/// This struct handles the basic mechanism of:
/// 1. Downloading module source from git/file/http URIs
/// 2. Optionally installing dependencies (via subprocess command)
/// 3. Returning the activated module path
///
/// Apps provide the policy (which modules to load, from where).
/// This struct provides the mechanism (how to load them).
///
/// # Example
///
/// ```no_run
/// use amplifier_foundation::modules::activator::ModuleActivator;
///
/// # async fn example() -> amplifier_foundation::Result<()> {
/// let activator = ModuleActivator::new(None, false, None);
/// let path = activator.activate("tool-bash", "file:///path/to/module").await?;
/// println!("Module activated at: {}", path.display());
/// # Ok(())
/// # }
/// ```
pub struct ModuleActivator {
    #[allow(dead_code)]
    cache_dir: PathBuf,
    install_deps: bool,
    resolver: SimpleSourceResolver,
    install_state: Mutex<InstallStateManager>,
    activated: Mutex<HashSet<String>>,
    /// Paths to bundle src/ directories tracked for child session inheritance.
    bundle_package_paths: Mutex<Vec<String>>,
}

impl ModuleActivator {
    /// Create a new module activator.
    ///
    /// # Arguments
    ///
    /// * `cache_dir` — Directory for caching downloaded modules. Defaults to
    ///   `~/.amplifier/cache`.
    /// * `install_deps` — Whether to install dependencies (via subprocess).
    /// * `base_path` — Base path for resolving relative module paths.
    ///   For bundles loaded from git, this should be the cloned bundle's
    ///   base_path so relative paths like `./modules/foo` resolve correctly.
    pub fn new(cache_dir: Option<PathBuf>, install_deps: bool, base_path: Option<PathBuf>) -> Self {
        let cache = cache_dir.unwrap_or_else(|| get_amplifier_home().join("cache"));
        let resolver = match base_path {
            Some(bp) => SimpleSourceResolver::with_base_path_and_cache_dir(bp, cache.clone()),
            None => SimpleSourceResolver::with_cache_dir(cache.clone()),
        };
        let install_state = InstallStateManager::new(cache.clone());

        Self {
            cache_dir: cache,
            install_deps,
            resolver,
            install_state: Mutex::new(install_state),
            activated: Mutex::new(HashSet::new()),
            bundle_package_paths: Mutex::new(Vec::new()),
        }
    }

    /// Get list of bundle package paths tracked for child session inheritance.
    ///
    /// These paths need to be shared with child sessions during spawning
    /// to ensure bundle packages remain importable.
    pub fn bundle_package_paths(&self) -> Vec<String> {
        let paths = self
            .bundle_package_paths
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        paths.clone()
    }

    /// Activate multiple modules with parallel execution.
    ///
    /// Modules that fail to activate are logged and skipped (not propagated).
    ///
    /// # Arguments
    ///
    /// * `modules` — Slice of `(module_name, source_uri)` tuples.
    ///
    /// # Returns
    ///
    /// Map of successfully activated module names to their local paths.
    pub async fn activate_all(&self, modules: &[(String, String)]) -> HashMap<String, PathBuf> {
        if modules.is_empty() {
            return HashMap::new();
        }

        // Parallel activation via join_all
        let futures: Vec<_> = modules
            .iter()
            .map(|(name, uri)| async move {
                let result = self.activate(name, uri).await;
                (name.clone(), result)
            })
            .collect();

        let results = futures::future::join_all(futures).await;

        let mut activated = HashMap::new();
        for (name, result) in results {
            match result {
                Ok(path) => {
                    activated.insert(name, path);
                }
                Err(e) => {
                    tracing::error!(module = %name, error = %e, "Failed to activate module");
                }
            }
        }
        activated
    }

    /// Install a bundle's own package to enable internal imports.
    ///
    /// When a bundle contains both a package (pyproject.toml at root) and
    /// modules that import from that package, the bundle's package needs to
    /// be installed BEFORE activating modules.
    ///
    /// # Arguments
    ///
    /// * `bundle_path` — Path to bundle root directory containing pyproject.toml.
    ///
    /// # Note
    ///
    /// This is a no-op if the bundle has no pyproject.toml or if the path
    /// doesn't exist. Must be called BEFORE `activate_all()` for modules
    /// that need it.
    pub async fn activate_bundle_package(&self, bundle_path: &Path) -> crate::Result<()> {
        if !bundle_path.exists() {
            return Ok(());
        }

        let pyproject = bundle_path.join("pyproject.toml");
        if !pyproject.exists() {
            tracing::debug!(
                path = %bundle_path.display(),
                "No pyproject.toml, skipping bundle package install"
            );
            return Ok(());
        }

        tracing::debug!(
            path = %bundle_path.display(),
            "Installing bundle package"
        );
        self.install_dependencies(bundle_path).await?;

        // Track bundle's src/ directory for child session inheritance
        let src_dir = bundle_path.join("src");
        if src_dir.exists() && src_dir.is_dir() {
            let src_path_str = src_dir.to_string_lossy().into_owned();
            let mut paths = self
                .bundle_package_paths
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if !paths.contains(&src_path_str) {
                paths.push(src_path_str);
            }
        }

        Ok(())
    }

    /// Save any pending state changes.
    ///
    /// Should be called at the end of module activation to persist
    /// the install state to disk.
    pub fn finalize(&self) {
        let mut state = self.install_state.lock().unwrap_or_else(|e| e.into_inner());
        if let Err(e) = state.save() {
            tracing::warn!(error = %e, "Failed to save install state");
        }
    }

    /// Install dependencies for a module.
    ///
    /// Uses `uv pip install -e <path> --quiet` for pyproject.toml,
    /// or `uv pip install -r <requirements.txt> --quiet` for requirements.txt.
    ///
    /// Skips installation if the module is already installed with matching
    /// fingerprint (via `InstallStateManager`).
    async fn install_dependencies(&self, module_path: &Path) -> crate::Result<()> {
        // Check if already installed with matching fingerprint
        {
            let state = self.install_state.lock().unwrap_or_else(|e| e.into_inner());
            if state.is_installed(module_path) {
                tracing::debug!(
                    path = %module_path.display(),
                    "Skipping install (already installed)"
                );
                return Ok(());
            }
        }

        let pyproject = module_path.join("pyproject.toml");
        let requirements = module_path.join("requirements.txt");

        if pyproject.exists() {
            self.run_install_command(&[
                "uv",
                "pip",
                "install",
                "-e",
                &module_path.to_string_lossy(),
                "--quiet",
            ])
            .await?;
        } else if requirements.exists() {
            self.run_install_command(&[
                "uv",
                "pip",
                "install",
                "-r",
                &requirements.to_string_lossy(),
                "--quiet",
            ])
            .await?;
        } else {
            // No dependency files found, nothing to install
            return Ok(());
        }

        // Mark as installed after successful install
        let mut state = self.install_state.lock().unwrap_or_else(|e| e.into_inner());
        state.mark_installed(module_path);

        Ok(())
    }

    /// Run an install command via subprocess.
    async fn run_install_command(&self, args: &[&str]) -> crate::Result<()> {
        let output = tokio::process::Command::new(args[0])
            .args(&args[1..])
            .output()
            .await
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    crate::BundleError::LoadError {
                        reason: format!("{} is not installed. Please install it first.", args[0]),
                        source: Some(Box::new(e)),
                    }
                } else {
                    crate::BundleError::Io(e)
                }
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            return Err(crate::BundleError::LoadError {
                reason: format!(
                    "Install command failed: {}\nstdout: {}\nstderr: {}",
                    args.join(" "),
                    stdout,
                    stderr
                ),
                source: None,
            });
        }

        Ok(())
    }
}

#[async_trait]
impl ModuleActivate for ModuleActivator {
    /// Activate a module by downloading and making it available.
    ///
    /// Resolves the source URI via `SimpleSourceResolver`, optionally
    /// installs dependencies, and returns the module's local path.
    ///
    /// # Deduplication
    ///
    /// Modules that have already been activated this session (same name + URI)
    /// skip the install step and return the cached resolved path.
    async fn activate(&self, module_name: &str, source_uri: &str) -> crate::Result<PathBuf> {
        let cache_key = format!("{}:{}", module_name, source_uri);

        // Check if already activated this session (brief lock, drop before await)
        let already_activated = {
            let activated = self.activated.lock().unwrap_or_else(|e| e.into_inner());
            activated.contains(&cache_key)
        };

        if already_activated {
            let resolved = self.resolver.resolve(source_uri).await?;
            return Ok(resolved.active_path);
        }

        // Resolve source URI to local path
        let resolved = self.resolver.resolve(source_uri).await?;
        let module_path = resolved.active_path;

        // Install dependencies if requested
        if self.install_deps {
            self.install_dependencies(&module_path).await?;
        }

        // Mark as activated (brief lock)
        {
            let mut activated = self.activated.lock().unwrap_or_else(|e| e.into_inner());
            activated.insert(cache_key);
        }

        Ok(module_path)
    }
}
