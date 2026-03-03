//! Installation state tracking for fast module startup.
//!
//! Tracks fingerprints of installed modules to skip redundant dependency
//! installation calls. When a module's pyproject.toml/requirements.txt
//! hasn't changed, we can skip the install step entirely, significantly
//! speeding up startup.
//!
//! Self-healing: corrupted JSON or schema version mismatch silently
//! resets to a fresh state file.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// Tracks module installation state for fast startup.
///
/// Stores fingerprints (pyproject.toml + requirements.txt hash) for
/// installed modules. If the fingerprint matches, we can skip dependency
/// installation entirely.
///
/// Self-healing: corrupted JSON or schema mismatch creates fresh state.
pub struct InstallStateManager {
    state_file: PathBuf,
    dirty: bool,
    state: InstallState,
}

/// Internal state schema.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct InstallState {
    version: u32,
    modules: HashMap<String, ModuleEntry>,
}

/// Per-module install tracking entry.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct ModuleEntry {
    pyproject_hash: String,
}

/// Current schema version. Bumping this invalidates all cached state.
const STATE_VERSION: u32 = 1;

/// State file name.
const STATE_FILENAME: &str = "install-state.json";

impl InstallStateManager {
    /// Create a new install state manager.
    ///
    /// Loads existing state from `cache_dir/install-state.json` if present
    /// and valid. Creates fresh state on corruption, version mismatch, or
    /// missing file.
    pub fn new(cache_dir: PathBuf) -> Self {
        let state_file = cache_dir.join(STATE_FILENAME);
        let (state, dirty) = Self::load(&state_file);
        Self {
            state_file,
            dirty,
            state,
        }
    }

    /// Load state from disk, creating fresh state if needed.
    fn load(state_file: &Path) -> (InstallState, bool) {
        if !state_file.exists() {
            // Fresh state is dirty so it gets persisted on first save()
            return (Self::fresh_state(), true);
        }

        let content = match fs::read_to_string(state_file) {
            Ok(c) => c,
            Err(e) => {
                tracing::debug!("Creating fresh install state (read failed: {e})");
                return (Self::fresh_state(), true);
            }
        };

        let data: serde_json::Value = match serde_json::from_str(&content) {
            Ok(d) => d,
            Err(e) => {
                tracing::debug!("Creating fresh install state (JSON parse failed: {e})");
                return (Self::fresh_state(), true);
            }
        };

        // Version mismatch - create fresh
        if data.get("version").and_then(|v| v.as_u64()) != Some(STATE_VERSION as u64) {
            tracing::debug!(
                "Creating fresh install state (version {} != {STATE_VERSION})",
                data.get("version")
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "null".to_string())
            );
            return (Self::fresh_state(), true);
        }

        // Deserialize the full state
        match serde_json::from_value::<InstallState>(data) {
            Ok(state) => (state, false),
            Err(e) => {
                tracing::debug!("Creating fresh install state (deserialize failed: {e})");
                (Self::fresh_state(), true)
            }
        }
    }

    /// Create a fresh empty state.
    fn fresh_state() -> InstallState {
        InstallState {
            version: STATE_VERSION,
            modules: HashMap::new(),
        }
    }

    /// Compute fingerprint for a module's dependency files.
    ///
    /// Hashes `pyproject.toml` and `requirements.txt` if present.
    /// Returns `"none"` if no dependency files exist.
    fn compute_fingerprint(module_path: &Path) -> String {
        let mut hasher = Sha256::new();
        let mut files_hashed = 0;

        for filename in &["pyproject.toml", "requirements.txt"] {
            let filepath = module_path.join(filename);
            if filepath.exists() {
                match fs::read(&filepath) {
                    Ok(content) => {
                        hasher.update(filename.as_bytes());
                        hasher.update(&content);
                        files_hashed += 1;
                    }
                    Err(_) => {
                        // Skip unreadable files (permission denied, etc.)
                    }
                }
            }
        }

        if files_hashed == 0 {
            return "none".to_string();
        }

        format!("sha256:{:x}", hasher.finalize())
    }

    /// Resolve a module path to a canonical string key.
    ///
    /// Uses `canonicalize` (resolves symlinks) when the path exists on disk.
    /// Falls back to `std::path::absolute()` (resolves against cwd, like
    /// Python's `Path.resolve()`) for non-existent paths. This ensures
    /// relative paths are always resolved to absolute, matching Python behavior.
    fn path_key(module_path: &Path) -> String {
        fs::canonicalize(module_path)
            .or_else(|_| std::path::absolute(module_path))
            .unwrap_or_else(|_| module_path.to_path_buf())
            .display()
            .to_string()
    }

    /// Check if module is already installed with matching fingerprint.
    ///
    /// Returns `true` if the module is installed and its dependency files
    /// have not changed since installation.
    pub fn is_installed(&self, module_path: &Path) -> bool {
        let key = Self::path_key(module_path);
        let entry = match self.state.modules.get(&key) {
            Some(e) => e,
            None => return false,
        };

        let current = Self::compute_fingerprint(module_path);
        if current != entry.pyproject_hash {
            tracing::debug!(
                "Fingerprint mismatch for {}: {} -> {}",
                module_path.display(),
                entry.pyproject_hash,
                current
            );
            return false;
        }

        true
    }

    /// Record that a module was successfully installed.
    pub fn mark_installed(&mut self, module_path: &Path) {
        let key = Self::path_key(module_path);
        let fingerprint = Self::compute_fingerprint(module_path);
        self.state.modules.insert(
            key,
            ModuleEntry {
                pyproject_hash: fingerprint,
            },
        );
        self.dirty = true;
    }

    /// Persist state to disk if changed.
    ///
    /// Uses atomic write (write to unique temp file, then rename) to avoid
    /// corruption from concurrent processes.
    ///
    /// Note: Unlike Python which swallows OSError and logs a warning, this
    /// method propagates errors to the caller. This is idiomatic Rust --
    /// callers decide error handling policy.
    pub fn save(&mut self) -> io::Result<()> {
        if !self.dirty {
            return Ok(());
        }

        // Ensure parent directory exists
        let parent = self
            .state_file
            .parent()
            .ok_or_else(|| io::Error::other("state file has no parent directory"))?;
        fs::create_dir_all(parent)?;

        // Atomic write: write to unique temp file, then rename (persist)
        let json = serde_json::to_string_pretty(&self.state).map_err(io::Error::other)?;

        let mut temp_file = tempfile::NamedTempFile::new_in(parent)?;
        io::Write::write_all(&mut temp_file, json.as_bytes())?;
        temp_file.persist(&self.state_file).map_err(|e| e.error)?;

        self.dirty = false;
        Ok(())
    }

    /// Clear state for one module or all modules.
    ///
    /// Pass `Some(path)` to invalidate a specific module, or `None` to
    /// invalidate all modules.
    pub fn invalidate(&mut self, module_path: Option<&Path>) {
        match module_path {
            None => {
                if !self.state.modules.is_empty() {
                    self.state.modules.clear();
                    self.dirty = true;
                    tracing::debug!("Invalidated all module install states");
                }
            }
            Some(path) => {
                let key = Self::path_key(path);
                if self.state.modules.remove(&key).is_some() {
                    self.dirty = true;
                    tracing::debug!("Invalidated install state for {}", path.display());
                }
            }
        }
    }

    /// Returns whether the state has unsaved changes.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }
}
