//! PyO3 bindings for amplifier-foundation.
//!
//! Provides Python-accessible types and functions when the `pyo3-bindings`
//! feature is enabled. The module is importable as `amplifier_foundation`
//! from Python.
//!
//! ## Exposed types
//!
//! - `ParsedURI` -- URI parsing result
//! - `Bundle` -- core composable unit (PyBundle)
//! - `ValidationResult` -- validation result with errors/warnings
//! - `SourceStatus` -- source update status (frozen)
//! - `ResolvedSource` -- resolved filesystem paths (frozen)
//! - `ProviderPreference` -- provider+model preference (frozen)
//! - `SimpleCache` -- in-memory key-value cache
//! - `DiskCache` -- filesystem-backed key-value cache
//!
//! ## Exposed functions
//!
//! - `parse_uri(uri)` -- parse a URI string into components
//! - `normalize_path(path)` -- normalize a filesystem path
//! - `deep_merge(base, overlay)` -- deep merge two dicts (native Python dicts
//!   via pythonize). Also available as `deep_merge_json` for JSON string interface.
//! - `parse_mentions(text)` -- extract @mentions from text
//! - `generate_sub_session_id(...)` -- generate child session ID
//! - `validate_bundle(bundle)` -- validate a bundle
//! - `validate_bundle_completeness(bundle)` -- strict validation for mountable bundles
//! - `apply_provider_preferences(mount_plan, prefs)` -- apply provider preferences
//! - `is_glob_pattern(pattern)` -- check for glob pattern characters
//! - `sanitize_for_json(data)` -- recursively sanitize data for JSON (removes nulls)
//! - `sanitize_message(message)` -- sanitize a chat message for persistence
//! - `merge_module_lists(parent, child)` -- merge module lists by module ID
//! - `format_directory_listing(path)` -- format directory contents listing
//!
//! ## Exposed exceptions
//!
//! - `BundleError` -- base exception for all bundle operations
//! - `BundleNotFoundError` -- bundle could not be located
//! - `BundleLoadError` -- bundle could not be loaded
//! - `BundleValidationError` -- bundle validation failed
//! - `BundleDependencyError` -- dependency could not be resolved

use pyo3::prelude::*;

// =============================================================================
// Conversion helpers: Python <-> serde_yaml_ng::Value via pythonize (direct)
// =============================================================================

/// Convert a Python object (dict/list/str/int/float/bool/None) to serde_yaml_ng::Value.
///
/// Uses pythonize to deserialize directly into serde_yaml_ng::Value.
/// No JSON intermediary -- preserves YAML-specific types (Tagged values,
/// non-string mapping keys) through the conversion.
fn pyobject_to_yaml(obj: &Bound<'_, PyAny>) -> PyResult<serde_yaml_ng::Value> {
    pythonize::depythonize(obj).map_err(|e| {
        pyo3::exceptions::PyValueError::new_err(format!(
            "Failed to convert Python object to Rust value: {e}"
        ))
    })
}

/// Convert a serde_yaml_ng::Value to a Python object (dict/list/str/int/float/bool/None).
///
/// Uses pythonize to serialize directly from serde_yaml_ng::Value to Python.
/// No JSON intermediary -- supports all YAML value types including Tagged values.
fn yaml_to_pyobject(py: Python<'_>, v: &serde_yaml_ng::Value) -> PyResult<PyObject> {
    let bound = pythonize::pythonize(py, v).map_err(|e| {
        pyo3::exceptions::PyValueError::new_err(format!(
            "Failed to convert Rust value to Python object: {e}"
        ))
    })?;
    Ok(bound.unbind())
}

// =============================================================================
// Python exception hierarchy
// =============================================================================
//
// NOTE: `create_exception!` puts the generated struct into this module's scope.
// The name `BundleError` shadows `crate::error::BundleError` (the Rust enum).
// Inside functions that need both types, alias the Rust enum:
//   `use crate::error::BundleError as BE;`

pyo3::create_exception!(
    amplifier_foundation,
    BundleError,
    pyo3::exceptions::PyException,
    "Base exception for all bundle-related errors."
);
pyo3::create_exception!(
    amplifier_foundation,
    BundleNotFoundError,
    BundleError,
    "Bundle could not be located at the specified source."
);
pyo3::create_exception!(
    amplifier_foundation,
    BundleLoadError,
    BundleError,
    "Bundle exists but could not be loaded (parse error, invalid format)."
);
pyo3::create_exception!(
    amplifier_foundation,
    BundleValidationError,
    BundleError,
    "Bundle loaded but validation failed (missing required fields, etc)."
);
pyo3::create_exception!(
    amplifier_foundation,
    BundleDependencyError,
    BundleError,
    "Bundle dependency could not be resolved (circular deps, missing deps)."
);

/// Map a `crate::error::BundleError` to the appropriate Python exception subclass.
///
/// - `NotFound`        → `BundleNotFoundError`
/// - `LoadError`       → `BundleLoadError`
/// - `ValidationError` → `BundleValidationError` (formats actual error messages)
/// - `DependencyError` → `BundleDependencyError`
/// - `Io` / `Yaml` / `Http` / `Git` → `BundleLoadError`
fn bundle_error_to_pyerr(e: crate::error::BundleError) -> PyErr {
    use crate::error::BundleError as BE;
    match e {
        BE::NotFound { .. } => BundleNotFoundError::new_err(e.to_string()),
        BE::LoadError { .. } => BundleLoadError::new_err(e.to_string()),
        BE::ValidationError(ref vr) => {
            // Format actual error messages, not just counts.
            // ValidationResult::Display only shows "N errors, M warnings".
            let msg = if vr.errors.is_empty() {
                e.to_string()
            } else {
                format!("validation failed: {}", vr.errors.join("; "))
            };
            BundleValidationError::new_err(msg)
        }
        BE::DependencyError(_) => BundleDependencyError::new_err(e.to_string()),
        BE::Io(_) | BE::Yaml(_) | BE::Http(_) | BE::Git(_) => {
            BundleLoadError::new_err(e.to_string())
        }
    }
}

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
    inner: crate::paths::uri::ParsedURI,
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

    fn is_git(&self) -> bool {
        self.inner.is_git()
    }

    fn is_file(&self) -> bool {
        self.inner.is_file()
    }

    fn is_http(&self) -> bool {
        self.inner.is_http()
    }

    fn is_zip(&self) -> bool {
        self.inner.is_zip()
    }

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
    inner: crate::bundle::Bundle,
}

#[pymethods]
impl PyBundle {
    /// Create a new empty Bundle with the given name.
    #[new]
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
    fn from_dict(data: &Bound<'_, PyAny>) -> PyResult<PyBundle> {
        let yaml_val = pyobject_to_yaml(data)?;
        let bundle = crate::bundle::Bundle::from_dict(&yaml_val).map_err(bundle_error_to_pyerr)?;
        Ok(PyBundle { inner: bundle })
    }

    /// Parse a Bundle from a Python dict with a base_path for context resolution.
    #[staticmethod]
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
    fn compose(&self, others: Vec<PyRef<'_, PyBundle>>) -> PyBundle {
        let other_refs: Vec<&crate::bundle::Bundle> = others.iter().map(|pb| &pb.inner).collect();
        let composed = self.inner.compose(&other_refs);
        PyBundle { inner: composed }
    }

    /// Generate a mount plan dict from this bundle.
    ///
    /// The mount plan contains only non-empty sections:
    /// session, providers, tools, hooks, spawn, agents.
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
// Standalone functions
// =============================================================================

/// Parse a URI string into its components.
///
/// Handles git+, zip+, file://, http/https, and local paths.
/// Always succeeds -- unrecognized URIs are treated as package names.
#[pyfunction]
fn parse_uri(uri: &str) -> PyParsedURI {
    PyParsedURI {
        inner: crate::paths::uri::parse_uri(uri),
    }
}

/// Normalize a filesystem path (resolve . and .., make absolute).
///
/// Uses the current working directory as the base for relative paths.
/// Raises `UnicodeDecodeError` if the resolved path contains non-UTF-8 bytes.
#[pyfunction]
fn normalize_path(path: &str) -> PyResult<String> {
    let p = crate::paths::uri::normalize_path(path, None);
    p.into_os_string().into_string().map_err(|os| {
        pyo3::exceptions::PyUnicodeDecodeError::new_err(format!(
            "Path contains non-UTF-8 bytes: {:?}",
            os
        ))
    })
}

/// Deep merge two Python dicts.
///
/// Accepts native Python dicts. Raises `TypeError` if either argument is not a dict.
/// Uses the same deep_merge algorithm as the Rust core: mappings are merged
/// recursively, sequences are replaced (overlay wins), scalars are replaced.
///
/// Example:
///   ```python
///   result = deep_merge({"a": 1, "b": {"c": 2}}, {"b": {"d": 3}})
///   # result == {"a": 1, "b": {"c": 2, "d": 3}}
///   ```
#[pyfunction]
fn deep_merge<'py>(
    py: Python<'py>,
    base: &Bound<'py, PyAny>,
    overlay: &Bound<'py, PyAny>,
) -> PyResult<PyObject> {
    // Type-check: both arguments must be dicts
    if !base.is_instance_of::<pyo3::types::PyDict>() {
        return Err(pyo3::exceptions::PyTypeError::new_err(
            "deep_merge() base argument must be a dict",
        ));
    }
    if !overlay.is_instance_of::<pyo3::types::PyDict>() {
        return Err(pyo3::exceptions::PyTypeError::new_err(
            "deep_merge() overlay argument must be a dict",
        ));
    }

    let base_yaml = pyobject_to_yaml(base)?;
    let overlay_yaml = pyobject_to_yaml(overlay)?;
    let merged = crate::dicts::merge::deep_merge(&base_yaml, &overlay_yaml);
    yaml_to_pyobject(py, &merged)
}

/// Deep merge two dicts using JSON strings (legacy v1 interface).
///
/// Kept for backward compatibility. Prefer `deep_merge()` which accepts
/// native Python dicts.
#[pyfunction]
fn deep_merge_json(base_json: &str, overlay_json: &str) -> PyResult<String> {
    let base: serde_yaml_ng::Value = serde_json::from_str(base_json)
        .map(json_to_yaml)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(format!("Invalid base JSON: {e}")))?;
    let overlay: serde_yaml_ng::Value = serde_json::from_str(overlay_json)
        .map(json_to_yaml)
        .map_err(|e| {
            pyo3::exceptions::PyValueError::new_err(format!("Invalid overlay JSON: {e}"))
        })?;

    let merged = crate::dicts::merge::deep_merge(&base, &overlay);

    let json_str = serde_json::to_string(&merged).map_err(|e| {
        pyo3::exceptions::PyValueError::new_err(format!("Failed to serialize result: {e}"))
    })?;
    Ok(json_str)
}

/// Extract @mentions from text (excluding code blocks and emails).
#[pyfunction]
fn parse_mentions(text: &str) -> Vec<String> {
    crate::mentions::parser::parse_mentions(text)
}

/// Generate a sub-session ID for agent delegation.
///
/// Args:
///   agent_name: Name of the agent being delegated to (optional).
///   session_id: Parent session ID for lineage tracking (optional).
///   trace_id: Parent trace ID for W3C Trace Context (optional).
#[pyfunction]
#[pyo3(signature = (agent_name=None, session_id=None, trace_id=None))]
fn generate_sub_session_id(
    agent_name: Option<&str>,
    session_id: Option<&str>,
    trace_id: Option<&str>,
) -> String {
    crate::tracing_utils::generate_sub_session_id(agent_name, session_id, trace_id)
}

/// Validate a bundle (basic validation: required fields + module list format).
///
/// Returns a ValidationResult with errors and warnings.
#[pyfunction]
fn validate_bundle(bundle: &PyBundle) -> PyValidationResult {
    let result = crate::bundle::validator::validate_bundle(&bundle.inner);
    result.into()
}

/// Validate a bundle for completeness (strict: requires session, orchestrator, providers).
///
/// Returns a ValidationResult with errors and warnings.
#[pyfunction]
fn validate_bundle_completeness(bundle: &PyBundle) -> PyValidationResult {
    let result = crate::bundle::validator::validate_bundle_completeness(&bundle.inner);
    result.into()
}

/// Validate a bundle, raising BundleValidationError on failure.
///
/// Raises BundleValidationError if the bundle has validation errors.
#[pyfunction]
fn validate_bundle_or_raise(bundle: &PyBundle) -> PyResult<()> {
    crate::bundle::validator::validate_bundle_or_raise(&bundle.inner).map_err(bundle_error_to_pyerr)
}

/// Validate a bundle for completeness, raising BundleValidationError on failure.
///
/// Raises BundleValidationError if the bundle is incomplete for mounting.
#[pyfunction]
fn validate_bundle_completeness_or_raise(bundle: &PyBundle) -> PyResult<()> {
    crate::bundle::validator::validate_bundle_completeness_or_raise(&bundle.inner)
        .map_err(bundle_error_to_pyerr)
}

// =============================================================================
// Internal conversion helpers (for legacy JSON interface only)
// =============================================================================

/// Convert serde_json::Value to serde_yaml_ng::Value.
///
/// Used only by `deep_merge_json` (legacy JSON string interface).
/// The main conversion path uses pythonize directly (no JSON intermediary).
fn json_to_yaml(v: serde_json::Value) -> serde_yaml_ng::Value {
    match v {
        serde_json::Value::Null => serde_yaml_ng::Value::Null,
        serde_json::Value::Bool(b) => serde_yaml_ng::Value::Bool(b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                serde_yaml_ng::Value::Number(i.into())
            } else if let Some(f) = n.as_f64() {
                // Large u64 values (> i64::MAX) land here with potential
                // precision loss for values > 2^53.
                serde_yaml_ng::Value::Number(serde_yaml_ng::Number::from(f))
            } else {
                // serde_json with `arbitrary_precision` can produce numbers
                // that are neither i64 nor f64. Fall back to 0 and log.
                tracing::warn!("json_to_yaml: unrepresentable number, falling back to 0");
                serde_yaml_ng::Value::Number(0.into())
            }
        }
        serde_json::Value::String(s) => serde_yaml_ng::Value::String(s),
        serde_json::Value::Array(arr) => {
            serde_yaml_ng::Value::Sequence(arr.into_iter().map(json_to_yaml).collect())
        }
        serde_json::Value::Object(map) => {
            let mut m = serde_yaml_ng::Mapping::new();
            for (k, v) in map {
                m.insert(serde_yaml_ng::Value::String(k), json_to_yaml(v));
            }
            serde_yaml_ng::Value::Mapping(m)
        }
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
    inner: crate::spawn::ProviderPreference,
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
    fn to_dict(&self, py: Python<'_>) -> PyResult<PyObject> {
        let yaml_val = self.inner.to_dict();
        yaml_to_pyobject(py, &yaml_val)
    }

    /// Parse from a Python dict with "provider" and "model" keys.
    #[staticmethod]
    fn from_dict(data: &Bound<'_, PyAny>) -> PyResult<PyProviderPreference> {
        let yaml_val = pyobject_to_yaml(data)?;
        crate::spawn::ProviderPreference::from_dict(&yaml_val)
            .map(|p| PyProviderPreference { inner: p })
            .map_err(pyo3::exceptions::PyValueError::new_err)
    }

    /// Parse a list of provider preference dicts. Silently skips invalid entries.
    #[staticmethod]
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

/// Apply provider preferences to a mount plan.
///
/// Takes a mount plan dict and a list of ProviderPreference objects.
/// Returns a new mount plan dict with the preferred provider promoted.
///
/// This is the sync version -- does NOT resolve glob patterns. For glob
/// resolution, use the async variant from Python directly.
#[pyfunction]
fn apply_provider_preferences<'py>(
    py: Python<'py>,
    mount_plan: &Bound<'py, PyAny>,
    preferences: Vec<PyRef<'_, PyProviderPreference>>,
) -> PyResult<PyObject> {
    let yaml_plan = pyobject_to_yaml(mount_plan)?;
    let pref_refs: Vec<crate::spawn::ProviderPreference> =
        preferences.iter().map(|p| p.inner.clone()).collect();
    let result = crate::spawn::apply_provider_preferences(&yaml_plan, &pref_refs);
    yaml_to_pyobject(py, &result)
}

/// Check if a string contains glob pattern characters (*, ?, [).
#[pyfunction]
fn is_glob_pattern(pattern: &str) -> bool {
    crate::spawn::glob::is_glob_pattern(pattern)
}

// =============================================================================
// JSON conversion helpers (for serialization functions)
// =============================================================================

/// Convert a Python object to serde_json::Value.
///
/// Uses pythonize for direct Python -> serde_json::Value conversion.
fn pyobject_to_json(obj: &Bound<'_, PyAny>) -> PyResult<serde_json::Value> {
    pythonize::depythonize(obj).map_err(|e| {
        pyo3::exceptions::PyValueError::new_err(format!(
            "Failed to convert Python object to JSON value: {e}"
        ))
    })
}

/// Convert a serde_json::Value to a Python object.
fn json_to_pyobject(py: Python<'_>, v: &serde_json::Value) -> PyResult<PyObject> {
    let bound = pythonize::pythonize(py, v).map_err(|e| {
        pyo3::exceptions::PyValueError::new_err(format!(
            "Failed to convert JSON value to Python object: {e}"
        ))
    })?;
    Ok(bound.unbind())
}

// =============================================================================
// Utility function bindings
// =============================================================================

/// Sanitize a value for JSON serialization.
///
/// Recursively processes the input:
/// - Null values inside dicts/lists are filtered out
/// - Nested structures are recursively sanitized
/// - Default max depth of 50 prevents infinite recursion
///
/// The optional ``max_depth`` parameter limits recursion depth.
/// At depth 0, any input returns ``None``.
///
/// Example:
///   ```python
///   result = sanitize_for_json({"a": 1, "b": None, "c": {"d": None}})
///   # result == {"a": 1, "c": {}}
///   ```
#[pyfunction]
#[pyo3(signature = (data, max_depth=None))]
fn sanitize_for_json<'py>(
    py: Python<'py>,
    data: &Bound<'py, PyAny>,
    max_depth: Option<usize>,
) -> PyResult<PyObject> {
    let json_val = pyobject_to_json(data)?;
    let sanitized = match max_depth {
        Some(depth) => crate::serialization::sanitize_for_json_with_depth(&json_val, depth),
        None => crate::serialization::sanitize_for_json(&json_val),
    };
    json_to_pyobject(py, &sanitized)
}

/// Sanitize a chat message for persistence.
///
/// Special handling for LLM API fields:
/// - ``thinking_block``: extracts ``.text`` as ``thinking_text``
/// - ``content_blocks``: skipped entirely
/// - Other fields: recursively sanitized (nulls removed)
///
/// Non-dict input returns an empty dict.
#[pyfunction]
fn sanitize_message<'py>(py: Python<'py>, message: &Bound<'py, PyAny>) -> PyResult<PyObject> {
    let json_val = pyobject_to_json(message)?;
    let sanitized = crate::serialization::sanitize_message(&json_val);
    json_to_pyobject(py, &sanitized)
}

/// Merge two module lists by module ID.
///
/// Module lists are arrays of dicts, each with a ``module`` key. Entries with
/// matching module IDs are deep-merged; new entries are appended.
///
/// Raises ``TypeError`` if arguments are not lists or contain non-dict elements.
///
/// Example:
///   ```python
///   parent = [{"module": "provider-openai", "config": {"model": "gpt-4"}}]
///   child = [{"module": "provider-openai", "config": {"temperature": 0.5}}]
///   result = merge_module_lists(parent, child)
///   # result[0]["config"] == {"model": "gpt-4", "temperature": 0.5}
///   ```
#[pyfunction]
fn merge_module_lists<'py>(
    py: Python<'py>,
    parent: &Bound<'py, PyAny>,
    child: &Bound<'py, PyAny>,
) -> PyResult<PyObject> {
    // Type-check: both must be lists
    if !parent.is_instance_of::<pyo3::types::PyList>() {
        return Err(pyo3::exceptions::PyTypeError::new_err(
            "merge_module_lists() parent argument must be a list",
        ));
    }
    if !child.is_instance_of::<pyo3::types::PyList>() {
        return Err(pyo3::exceptions::PyTypeError::new_err(
            "merge_module_lists() child argument must be a list",
        ));
    }

    let parent_yaml = pyobject_to_yaml(parent)?;
    let child_yaml = pyobject_to_yaml(child)?;

    // PyList always deserializes to Value::Sequence via pythonize
    let parent_seq = parent_yaml
        .as_sequence()
        .expect("PyList always deserializes to Sequence");
    let child_seq = child_yaml
        .as_sequence()
        .expect("PyList always deserializes to Sequence");

    // The Rust merge_module_lists panics on non-Mapping elements.
    // Catch the panic and convert to a clean Python TypeError.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        crate::dicts::merge::merge_module_lists(parent_seq, child_seq)
    }));

    match result {
        Ok(merged) => {
            let result_yaml = serde_yaml_ng::Value::Sequence(merged);
            yaml_to_pyobject(py, &result_yaml)
        }
        Err(_) => Err(pyo3::exceptions::PyTypeError::new_err(
            "merge_module_lists() list elements must be dicts with a 'module' key",
        )),
    }
}

/// Format a directory listing for a given path.
///
/// Returns a string with one line per entry, sorted dirs-first.
/// Each line is prefixed with ``DIR`` or ``FILE``.
///
/// Note: This function never raises exceptions. If the directory cannot be
/// read (permission denied, not found), the error is embedded in the
/// returned string. Callers should check for ``(permission denied)`` if
/// error detection is needed.
#[pyfunction]
fn format_directory_listing(path: &str) -> String {
    crate::mentions::utils::format_directory_listing(std::path::Path::new(path))
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
    fn new() -> Self {
        PySimpleCache {
            inner: crate::cache::memory::SimpleCache::new(),
        }
    }

    /// Get a value by key, returning None if not found.
    fn get(&self, py: Python<'_>, key: &str) -> PyResult<Option<PyObject>> {
        use crate::cache::CacheProvider;
        match self.inner.get(key) {
            Some(val) => Ok(Some(yaml_to_pyobject(py, &val)?)),
            None => Ok(None),
        }
    }

    /// Set a value for the given key.
    fn set(&mut self, key: &str, value: &Bound<'_, PyAny>) -> PyResult<()> {
        use crate::cache::CacheProvider;
        let yaml_val = pyobject_to_yaml(value)?;
        self.inner.set(key, yaml_val);
        Ok(())
    }

    /// Check if a key exists in the cache.
    fn contains(&self, key: &str) -> bool {
        use crate::cache::CacheProvider;
        self.inner.contains(key)
    }

    /// Clear all entries from the cache.
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
    fn new(cache_dir: &str) -> Self {
        PyDiskCache {
            inner: crate::cache::disk::DiskCache::new(std::path::Path::new(cache_dir)),
        }
    }

    /// Get a value by key, returning None if not found or corrupt.
    fn get(&self, py: Python<'_>, key: &str) -> PyResult<Option<PyObject>> {
        use crate::cache::CacheProvider;
        match self.inner.get(key) {
            Some(val) => Ok(Some(yaml_to_pyobject(py, &val)?)),
            None => Ok(None),
        }
    }

    /// Set a value for the given key (writes to disk as JSON).
    fn set(&mut self, key: &str, value: &Bound<'_, PyAny>) -> PyResult<()> {
        use crate::cache::CacheProvider;
        let yaml_val = pyobject_to_yaml(value)?;
        self.inner.set(key, yaml_val);
        Ok(())
    }

    /// Check if a key exists in the cache (checks file existence).
    fn contains(&self, key: &str) -> bool {
        use crate::cache::CacheProvider;
        self.inner.contains(key)
    }

    /// Clear all entries from the cache (deletes all .json files).
    fn clear(&mut self) {
        use crate::cache::CacheProvider;
        self.inner.clear();
    }

    /// Get the filesystem path for a cache key (for debugging/inspection).
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

// =============================================================================
// Module definition
// =============================================================================

/// Python module definition.
#[pymodule]
fn amplifier_foundation(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;

    // Exception hierarchy
    m.add("BundleError", m.py().get_type::<BundleError>())?;
    m.add(
        "BundleNotFoundError",
        m.py().get_type::<BundleNotFoundError>(),
    )?;
    m.add("BundleLoadError", m.py().get_type::<BundleLoadError>())?;
    m.add(
        "BundleValidationError",
        m.py().get_type::<BundleValidationError>(),
    )?;
    m.add(
        "BundleDependencyError",
        m.py().get_type::<BundleDependencyError>(),
    )?;

    // Types
    m.add_class::<PyParsedURI>()?;
    m.add_class::<PyBundle>()?;
    m.add_class::<PyValidationResult>()?;
    m.add_class::<PySourceStatus>()?;
    m.add_class::<PyResolvedSource>()?;
    m.add_class::<PyProviderPreference>()?;
    m.add_class::<PySimpleCache>()?;
    m.add_class::<PyDiskCache>()?;

    // Functions
    m.add_function(wrap_pyfunction!(parse_uri, m)?)?;
    m.add_function(wrap_pyfunction!(normalize_path, m)?)?;
    m.add_function(wrap_pyfunction!(deep_merge, m)?)?;
    m.add_function(wrap_pyfunction!(deep_merge_json, m)?)?;
    m.add_function(wrap_pyfunction!(parse_mentions, m)?)?;
    m.add_function(wrap_pyfunction!(generate_sub_session_id, m)?)?;
    m.add_function(wrap_pyfunction!(validate_bundle, m)?)?;
    m.add_function(wrap_pyfunction!(validate_bundle_completeness, m)?)?;
    m.add_function(wrap_pyfunction!(validate_bundle_or_raise, m)?)?;
    m.add_function(wrap_pyfunction!(validate_bundle_completeness_or_raise, m)?)?;
    m.add_function(wrap_pyfunction!(apply_provider_preferences, m)?)?;
    m.add_function(wrap_pyfunction!(is_glob_pattern, m)?)?;
    m.add_function(wrap_pyfunction!(sanitize_for_json, m)?)?;
    m.add_function(wrap_pyfunction!(sanitize_message, m)?)?;
    m.add_function(wrap_pyfunction!(merge_module_lists, m)?)?;
    m.add_function(wrap_pyfunction!(format_directory_listing, m)?)?;
    Ok(())
}

// Tests for pyo3_bindings require Python dev headers for linking.
// The helper functions (pyobject_to_yaml, yaml_to_pyobject) are
// compile-checked via `cargo check --features pyo3-bindings` and
// exercised through the Python test harness when built with maturin.
//
// Pure conversion logic tests that don't need pyo3 are in
// tests/test_pyo3_helpers.rs (always compiled without pyo3 feature).
