//! Python-accessible types (#[pyclass] definitions).
//!
//! Contains all PyO3 class wrappers: PyParsedURI, PyBundle, PyValidationResult,
//! PySourceStatus, PyResolvedSource, PyProviderPreference, PySimpleCache, PyDiskCache.

use pyo3::prelude::*;

use super::exceptions::bundle_error_to_pyerr;
use super::helpers::{pyobject_to_yaml, yaml_to_pyobject};

// =============================================================================
// ParsedURI
// =============================================================================

/// Python-accessible ParsedURI.
///
/// Mirrors the Rust `ParsedURI` with all fields exposed as read-only properties.
///
/// Note: The `ref_` property has a trailing underscore because `ref` is a
/// reserved keyword in Rust. In Python, access it as `parsed.ref_`.
#[pyclass(name = "ParsedURI", frozen)]
#[derive(Clone, Debug)]
pub struct PyParsedURI {
    pub(super) inner: crate::paths::uri::ParsedURI,
}

#[pymethods]
impl PyParsedURI {
    #[getter]
    fn scheme(&self) -> &str {
        &self.inner.scheme
    }

    #[getter]
    fn host(&self) -> &str {
        &self.inner.host
    }

    #[getter]
    fn path(&self) -> &str {
        &self.inner.path
    }

    #[getter]
    fn subpath(&self) -> &str {
        &self.inner.subpath
    }

    #[getter]
    fn ref_(&self) -> &str {
        &self.inner.ref_
    }

    #[pyo3(text_signature = "($self)")]
    fn is_git(&self) -> bool {
        self.inner.is_git()
    }

    #[pyo3(text_signature = "($self)")]
    fn is_file(&self) -> bool {
        self.inner.is_file()
    }

    #[pyo3(text_signature = "($self)")]
    fn is_http(&self) -> bool {
        self.inner.is_http()
    }

    #[pyo3(text_signature = "($self)")]
    fn is_zip(&self) -> bool {
        self.inner.is_zip()
    }

    #[pyo3(text_signature = "($self)")]
    fn is_package(&self) -> bool {
        self.inner.is_package()
    }

    fn __repr__(&self) -> String {
        format!(
            "ParsedURI(scheme='{}', host='{}', path='{}', ref_='{}', subpath='{}')",
            self.inner.scheme,
            self.inner.host,
            self.inner.path,
            self.inner.ref_,
            self.inner.subpath
        )
    }

    fn __eq__(&self, other: &PyParsedURI) -> bool {
        self.inner == other.inner
    }

    fn __hash__(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.inner.scheme.hash(&mut hasher);
        self.inner.host.hash(&mut hasher);
        self.inner.path.hash(&mut hasher);
        self.inner.ref_.hash(&mut hasher);
        self.inner.subpath.hash(&mut hasher);
        hasher.finish()
    }
}

// =============================================================================
// Bundle
// =============================================================================

/// Python-accessible Bundle.
///
/// Wraps the Rust `Bundle` struct, providing Python-native dict I/O via pythonize.
/// All dict parameters accept native Python dicts (no JSON string round-tripping).
///
/// `Bundle` is mutable (supports property setters) and therefore intentionally
/// unhashable. It does not implement `__eq__` or `__hash__`. Use `to_dict()`
/// for structural comparison if needed.
#[pyclass(name = "Bundle")]
#[derive(Clone, Debug)]
pub struct PyBundle {
    pub(super) inner: crate::bundle::Bundle,
}

#[pymethods]
impl PyBundle {
    /// Create a new empty Bundle with the given name.
    #[new]
    #[pyo3(text_signature = "(name='')")]
    #[pyo3(signature = (name=""))]
    fn new(name: &str) -> Self {
        PyBundle {
            inner: crate::bundle::Bundle::new(name),
        }
    }

    /// Parse a Bundle from a Python dict.
    ///
    /// Expects the same format as the Rust `Bundle::from_dict`:
    /// `{"bundle": {"name": "...", "providers": [...], ...}}`
    #[staticmethod]
    #[pyo3(text_signature = "(data)")]
    fn from_dict(data: &Bound<'_, PyAny>) -> PyResult<PyBundle> {
        let yaml_val = pyobject_to_yaml(data)?;
        let bundle = crate::bundle::Bundle::from_dict(&yaml_val).map_err(bundle_error_to_pyerr)?;
        Ok(PyBundle { inner: bundle })
    }

    /// Parse a Bundle from a Python dict with a base_path for context resolution.
    #[staticmethod]
    #[pyo3(text_signature = "(data, base_path)")]
    fn from_dict_with_base_path(data: &Bound<'_, PyAny>, base_path: &str) -> PyResult<PyBundle> {
        let yaml_val = pyobject_to_yaml(data)?;
        let bundle = crate::bundle::Bundle::from_dict_with_base_path(
            &yaml_val,
            std::path::Path::new(base_path),
        )
        .map_err(bundle_error_to_pyerr)?;
        Ok(PyBundle { inner: bundle })
    }

    /// Serialize the Bundle to a Python dict.
    #[pyo3(text_signature = "($self)")]
    fn to_dict(&self, py: Python<'_>) -> PyResult<PyObject> {
        let yaml_val = self.inner.to_dict();
        yaml_to_pyobject(py, &yaml_val)
    }

    /// Compose this bundle with one or more overlay bundles.
    ///
    /// Uses the 5-strategy merge system:
    /// 1. deep_merge for session/spawn
    /// 2. merge_module_lists for providers/tools/hooks
    /// 3. dict update for agents
    /// 4. accumulate with namespace for context
    /// 5. later replaces for instruction/base_path/name
    #[pyo3(text_signature = "($self, others)")]
    fn compose(&self, others: Vec<PyRef<'_, PyBundle>>) -> PyBundle {
        let other_refs: Vec<&crate::bundle::Bundle> = others.iter().map(|pb| &pb.inner).collect();
        let composed = self.inner.compose(&other_refs);
        PyBundle { inner: composed }
    }

    /// Generate a mount plan dict from this bundle.
    ///
    /// The mount plan contains only non-empty sections:
    /// session, providers, tools, hooks, spawn, agents.
    #[pyo3(text_signature = "($self)")]
    fn to_mount_plan(&self, py: Python<'_>) -> PyResult<PyObject> {
        let yaml_val = self.inner.to_mount_plan();
        yaml_to_pyobject(py, &yaml_val)
    }

    /// Bundle name.
    #[getter]
    fn name(&self) -> &str {
        &self.inner.name
    }

    /// Set the bundle name.
    #[setter]
    fn set_name(&mut self, name: String) {
        self.inner.name = name;
    }

    /// Bundle version.
    #[getter]
    fn version(&self) -> &str {
        &self.inner.version
    }

    /// Set the bundle version.
    #[setter]
    fn set_version(&mut self, version: String) {
        self.inner.version = version;
    }

    /// Bundle description.
    #[getter]
    fn description(&self) -> &str {
        &self.inner.description
    }

    /// System instruction text (if set).
    #[getter]
    fn instruction(&self) -> Option<&str> {
        self.inner.instruction.as_deref()
    }

    /// Source URI (if loaded from a remote source).
    #[getter]
    fn source_uri(&self) -> Option<&str> {
        self.inner.source_uri.as_deref()
    }

    /// Number of providers in the bundle.
    #[getter]
    fn provider_count(&self) -> usize {
        self.inner.providers.len()
    }

    /// Number of tools in the bundle.
    #[getter]
    fn tool_count(&self) -> usize {
        self.inner.tools.len()
    }

    /// Number of hooks in the bundle.
    #[getter]
    fn hook_count(&self) -> usize {
        self.inner.hooks.len()
    }

    /// Create a shallow copy of this bundle.
    fn __copy__(&self) -> PyBundle {
        self.clone()
    }

    /// Create a deep copy of this bundle (same as copy since all data is owned).
    fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> PyBundle {
        self.clone()
    }

    fn __repr__(&self) -> String {
        format!(
            "Bundle(name='{}', version='{}', providers={}, tools={}, hooks={})",
            self.inner.name,
            self.inner.version,
            self.inner.providers.len(),
            self.inner.tools.len(),
            self.inner.hooks.len(),
        )
    }
}

// =============================================================================
// ValidationResult
// =============================================================================

/// Python-accessible ValidationResult.
///
/// Contains validation errors and warnings from bundle validation.
/// Supports truthiness: `if result:` checks whether validation passed.
#[pyclass(name = "ValidationResult", frozen)]
#[derive(Clone, Debug)]
pub struct PyValidationResult {
    valid: bool,
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl From<crate::bundle::validator::ValidationResult> for PyValidationResult {
    fn from(r: crate::bundle::validator::ValidationResult) -> Self {
        PyValidationResult {
            valid: r.valid,
            errors: r.errors,
            warnings: r.warnings,
        }
    }
}

#[pymethods]
impl PyValidationResult {
    /// Whether the bundle passed validation (no errors).
    #[getter]
    fn is_valid(&self) -> bool {
        self.valid
    }

    /// List of validation error messages.
    #[getter]
    fn errors(&self) -> Vec<String> {
        self.errors.clone()
    }

    /// List of validation warning messages.
    #[getter]
    fn warnings(&self) -> Vec<String> {
        self.warnings.clone()
    }

    fn __repr__(&self) -> String {
        format!(
            "ValidationResult(valid={}, errors={}, warnings={})",
            self.valid,
            self.errors.len(),
            self.warnings.len()
        )
    }

    fn __bool__(&self) -> bool {
        self.valid
    }
}

// =============================================================================
// SourceStatus
// =============================================================================

/// Python-accessible SourceStatus.
///
/// Represents the status of a bundle's source (e.g., git repo).
/// Frozen (immutable) since status is a point-in-time snapshot.
///
/// Truthiness: `bool(status)` returns `True` if an update is available.
#[pyclass(name = "SourceStatus", frozen)]
#[derive(Clone, Debug)]
pub struct PySourceStatus {
    inner: crate::sources::SourceStatus,
}

impl From<crate::sources::SourceStatus> for PySourceStatus {
    fn from(s: crate::sources::SourceStatus) -> Self {
        PySourceStatus { inner: s }
    }
}

#[pymethods]
impl PySourceStatus {
    /// Create a new SourceStatus for the given URI.
    #[new]
    #[pyo3(text_signature = "(uri)")]
    fn new(uri: &str) -> Self {
        PySourceStatus {
            inner: crate::sources::SourceStatus::new(uri),
        }
    }

    /// Source URI.
    #[getter]
    fn uri(&self) -> &str {
        &self.inner.uri
    }

    /// Whether an update is available: True, False, or None (unknown).
    #[getter]
    fn has_update(&self) -> Option<bool> {
        self.inner.has_update
    }

    /// Whether the source is cached locally.
    #[getter]
    fn is_cached(&self) -> bool {
        self.inner.is_cached
    }

    /// ISO 8601 timestamp of when the source was cached.
    #[getter]
    fn cached_at(&self) -> Option<&str> {
        self.inner.cached_at.as_deref()
    }

    /// Cached ref (branch/tag name or SHA).
    #[getter]
    fn cached_ref(&self) -> Option<&str> {
        self.inner.cached_ref.as_deref()
    }

    /// Cached commit SHA.
    #[getter]
    fn cached_commit(&self) -> Option<&str> {
        self.inner.cached_commit.as_deref()
    }

    /// Remote ref from status check.
    #[getter]
    fn remote_ref(&self) -> Option<&str> {
        self.inner.remote_ref.as_deref()
    }

    /// Remote commit SHA from status check.
    #[getter]
    fn remote_commit(&self) -> Option<&str> {
        self.inner.remote_commit.as_deref()
    }

    /// Error message if status check failed.
    #[getter]
    fn error(&self) -> Option<&str> {
        self.inner.error.as_deref()
    }

    /// Human-readable status summary.
    #[getter]
    fn summary(&self) -> &str {
        &self.inner.summary
    }

    /// Current cached version string (Rust-only field).
    #[getter]
    fn current_version(&self) -> Option<&str> {
        self.inner.current_version.as_deref()
    }

    /// Latest available version string (Rust-only field).
    #[getter]
    fn latest_version(&self) -> Option<&str> {
        self.inner.latest_version.as_deref()
    }

    /// Whether the cached ref is pinned (exact SHA or version tag).
    #[pyo3(text_signature = "($self)")]
    fn is_pinned(&self) -> bool {
        self.inner.is_pinned()
    }

    /// Truthiness: True only if an update is confirmed available.
    /// Returns False for both "no update" and "unknown" states.
    /// Check `has_update` property for the tri-state value.
    fn __bool__(&self) -> bool {
        self.inner.has_update == Some(true)
    }

    fn __repr__(&self) -> String {
        format!(
            "SourceStatus(uri='{}', has_update={}, is_cached={}, summary='{}')",
            self.inner.uri,
            match self.inner.has_update {
                Some(true) => "True",
                Some(false) => "False",
                None => "None",
            },
            self.inner.is_cached,
            self.inner.summary,
        )
    }

    fn __eq__(&self, other: &PySourceStatus) -> bool {
        self.inner == other.inner
    }

    fn __hash__(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.inner.uri.hash(&mut hasher);
        self.inner.has_update.map(|b| b as u8).hash(&mut hasher);
        self.inner.is_cached.hash(&mut hasher);
        self.inner.cached_at.hash(&mut hasher);
        self.inner.cached_ref.hash(&mut hasher);
        self.inner.cached_commit.hash(&mut hasher);
        self.inner.remote_ref.hash(&mut hasher);
        self.inner.remote_commit.hash(&mut hasher);
        self.inner.error.hash(&mut hasher);
        self.inner.summary.hash(&mut hasher);
        self.inner.current_version.hash(&mut hasher);
        self.inner.latest_version.hash(&mut hasher);
        hasher.finish()
    }
}

// =============================================================================
// ResolvedSource
// =============================================================================

/// Python-accessible ResolvedSource.
///
/// Represents a source that has been resolved to local filesystem paths.
/// Frozen since resolution is immutable.
#[pyclass(name = "ResolvedSource", frozen)]
#[derive(Clone, Debug)]
pub struct PyResolvedSource {
    inner: crate::paths::uri::ResolvedSource,
}

impl From<crate::paths::uri::ResolvedSource> for PyResolvedSource {
    fn from(r: crate::paths::uri::ResolvedSource) -> Self {
        PyResolvedSource { inner: r }
    }
}

#[pymethods]
impl PyResolvedSource {
    /// Create a new ResolvedSource with the given paths.
    #[new]
    #[pyo3(text_signature = "(active_path, source_root)")]
    fn new(active_path: &str, source_root: &str) -> Self {
        PyResolvedSource {
            inner: crate::paths::uri::ResolvedSource {
                active_path: std::path::PathBuf::from(active_path),
                source_root: std::path::PathBuf::from(source_root),
            },
        }
    }

    /// The active path (where the bundle content lives).
    #[getter]
    fn active_path(&self) -> String {
        self.inner.active_path.display().to_string()
    }

    /// The source root (top-level of the resolved source).
    #[getter]
    fn source_root(&self) -> String {
        self.inner.source_root.display().to_string()
    }

    /// Whether the active path is a subdirectory of source_root.
    #[pyo3(text_signature = "($self)")]
    fn is_subdirectory(&self) -> bool {
        self.inner.is_subdirectory()
    }

    fn __repr__(&self) -> String {
        format!(
            "ResolvedSource(active_path='{}', source_root='{}')",
            self.inner.active_path.display(),
            self.inner.source_root.display(),
        )
    }

    fn __eq__(&self, other: &PyResolvedSource) -> bool {
        self.inner == other.inner
    }

    fn __hash__(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.inner.active_path.hash(&mut hasher);
        self.inner.source_root.hash(&mut hasher);
        hasher.finish()
    }
}

// =============================================================================
// ProviderPreference
// =============================================================================

/// Python-accessible ProviderPreference.
///
/// A preferred provider+model pair used in `apply_provider_preferences`.
/// Frozen (value type).
#[pyclass(name = "ProviderPreference", frozen)]
#[derive(Clone, Debug)]
pub struct PyProviderPreference {
    pub(super) inner: crate::spawn::ProviderPreference,
}

impl From<crate::spawn::ProviderPreference> for PyProviderPreference {
    fn from(p: crate::spawn::ProviderPreference) -> Self {
        PyProviderPreference { inner: p }
    }
}

#[pymethods]
impl PyProviderPreference {
    /// Create a new ProviderPreference.
    #[new]
    #[pyo3(text_signature = "(provider, model)")]
    fn new(provider: &str, model: &str) -> Self {
        PyProviderPreference {
            inner: crate::spawn::ProviderPreference::new(provider, model),
        }
    }

    /// Provider name (e.g., "anthropic", "openai").
    #[getter]
    fn provider(&self) -> &str {
        &self.inner.provider
    }

    /// Model name or glob pattern (e.g., "claude-*", "gpt-4o").
    #[getter]
    fn model(&self) -> &str {
        &self.inner.model
    }

    /// Serialize to a Python dict: {"provider": "...", "model": "..."}.
    #[pyo3(text_signature = "($self)")]
    fn to_dict(&self, py: Python<'_>) -> PyResult<PyObject> {
        let yaml_val = self.inner.to_dict();
        yaml_to_pyobject(py, &yaml_val)
    }

    /// Parse from a Python dict with "provider" and "model" keys.
    #[staticmethod]
    #[pyo3(text_signature = "(data)")]
    fn from_dict(data: &Bound<'_, PyAny>) -> PyResult<PyProviderPreference> {
        let yaml_val = pyobject_to_yaml(data)?;
        crate::spawn::ProviderPreference::from_dict(&yaml_val)
            .map(|p| PyProviderPreference { inner: p })
            .map_err(pyo3::exceptions::PyValueError::new_err)
    }

    /// Parse a list of provider preference dicts. Silently skips invalid entries.
    #[staticmethod]
    #[pyo3(text_signature = "(data)")]
    fn from_list(data: &Bound<'_, PyAny>) -> PyResult<Vec<PyProviderPreference>> {
        let yaml_val = pyobject_to_yaml(data)?;
        match yaml_val {
            serde_yaml_ng::Value::Sequence(seq) => {
                let prefs = crate::spawn::ProviderPreference::from_list(&seq);
                Ok(prefs.into_iter().map(PyProviderPreference::from).collect())
            }
            _ => Err(pyo3::exceptions::PyTypeError::new_err(
                "from_list() argument must be a list",
            )),
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "ProviderPreference(provider='{}', model='{}')",
            self.inner.provider, self.inner.model,
        )
    }

    fn __eq__(&self, other: &PyProviderPreference) -> bool {
        self.inner == other.inner
    }

    fn __hash__(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.inner.provider.hash(&mut hasher);
        self.inner.model.hash(&mut hasher);
        hasher.finish()
    }
}

// =============================================================================
// SimpleCache
// =============================================================================

/// Python-accessible SimpleCache (in-memory key-value store).
///
/// Stores values as native Python objects (converted via pythonize).
/// Mutable -- supports set/clear operations.
#[pyclass(name = "SimpleCache")]
pub struct PySimpleCache {
    inner: crate::cache::memory::SimpleCache,
}

#[pymethods]
impl PySimpleCache {
    /// Create a new empty SimpleCache.
    #[new]
    #[pyo3(text_signature = "()")]
    fn new() -> Self {
        PySimpleCache {
            inner: crate::cache::memory::SimpleCache::new(),
        }
    }

    /// Get a value by key, returning None if not found.
    #[pyo3(text_signature = "($self, key)")]
    fn get(&self, py: Python<'_>, key: &str) -> PyResult<Option<PyObject>> {
        use crate::cache::CacheProvider;
        match self.inner.get(key) {
            Some(val) => Ok(Some(yaml_to_pyobject(py, &val)?)),
            None => Ok(None),
        }
    }

    /// Set a value for the given key.
    #[pyo3(text_signature = "($self, key, value)")]
    fn set(&mut self, key: &str, value: &Bound<'_, PyAny>) -> PyResult<()> {
        use crate::cache::CacheProvider;
        let yaml_val = pyobject_to_yaml(value)?;
        self.inner.set(key, yaml_val);
        Ok(())
    }

    /// Check if a key exists in the cache.
    #[pyo3(text_signature = "($self, key)")]
    fn contains(&self, key: &str) -> bool {
        use crate::cache::CacheProvider;
        self.inner.contains(key)
    }

    /// Clear all entries from the cache.
    #[pyo3(text_signature = "($self)")]
    fn clear(&mut self) {
        use crate::cache::CacheProvider;
        self.inner.clear();
    }

    /// Python `key in cache` support.
    fn __contains__(&self, key: &str) -> bool {
        use crate::cache::CacheProvider;
        self.inner.contains(key)
    }

    fn __repr__(&self) -> String {
        "SimpleCache()".to_string()
    }
}

// =============================================================================
// DiskCache
// =============================================================================

/// Python-accessible DiskCache (filesystem-backed key-value store).
///
/// Stores values as JSON files in the cache directory.
/// Cache keys are hashed (SHA-256) for filesystem-safe names.
/// Mutable -- supports set/clear operations.
#[pyclass(name = "DiskCache")]
pub struct PyDiskCache {
    inner: crate::cache::disk::DiskCache,
}

#[pymethods]
impl PyDiskCache {
    /// Create a new DiskCache at the given directory path.
    ///
    /// The directory is created immediately if it doesn't exist.
    #[new]
    #[pyo3(text_signature = "(cache_dir)")]
    fn new(cache_dir: &str) -> Self {
        PyDiskCache {
            inner: crate::cache::disk::DiskCache::new(std::path::Path::new(cache_dir)),
        }
    }

    /// Get a value by key, returning None if not found or corrupt.
    #[pyo3(text_signature = "($self, key)")]
    fn get(&self, py: Python<'_>, key: &str) -> PyResult<Option<PyObject>> {
        use crate::cache::CacheProvider;
        match self.inner.get(key) {
            Some(val) => Ok(Some(yaml_to_pyobject(py, &val)?)),
            None => Ok(None),
        }
    }

    /// Set a value for the given key (writes to disk as JSON).
    #[pyo3(text_signature = "($self, key, value)")]
    fn set(&mut self, key: &str, value: &Bound<'_, PyAny>) -> PyResult<()> {
        use crate::cache::CacheProvider;
        let yaml_val = pyobject_to_yaml(value)?;
        self.inner.set(key, yaml_val);
        Ok(())
    }

    /// Check if a key exists in the cache (checks file existence).
    #[pyo3(text_signature = "($self, key)")]
    fn contains(&self, key: &str) -> bool {
        use crate::cache::CacheProvider;
        self.inner.contains(key)
    }

    /// Clear all entries from the cache (deletes all .json files).
    #[pyo3(text_signature = "($self)")]
    fn clear(&mut self) {
        use crate::cache::CacheProvider;
        self.inner.clear();
    }

    /// Get the filesystem path for a cache key (for debugging/inspection).
    #[pyo3(text_signature = "($self, key)")]
    fn cache_key_to_path(&self, key: &str) -> String {
        self.inner.cache_key_to_path(key).display().to_string()
    }

    /// The cache directory path.
    #[getter]
    fn cache_dir(&self) -> String {
        self.inner.cache_dir.display().to_string()
    }

    /// Python `key in cache` support.
    fn __contains__(&self, key: &str) -> bool {
        use crate::cache::CacheProvider;
        self.inner.contains(key)
    }

    fn __repr__(&self) -> String {
        format!("DiskCache(cache_dir='{}')", self.inner.cache_dir.display())
    }
}
