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
        self.inner.scheme == other.inner.scheme
            && self.inner.host == other.inner.host
            && self.inner.path == other.inner.path
            && self.inner.ref_ == other.inner.ref_
            && self.inner.subpath == other.inner.subpath
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
        let bundle = crate::bundle::Bundle::from_dict(&yaml_val).map_err(|e| {
            pyo3::exceptions::PyValueError::new_err(format!("Failed to parse bundle: {e}"))
        })?;
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
        .map_err(|e| {
            pyo3::exceptions::PyValueError::new_err(format!("Failed to parse bundle: {e}"))
        })?;
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

/// Validate a bundle, raising ValueError on failure.
///
/// Raises ValueError if the bundle has validation errors.
#[pyfunction]
fn validate_bundle_or_raise(bundle: &PyBundle) -> PyResult<()> {
    crate::bundle::validator::validate_bundle_or_raise(&bundle.inner)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(format!("{e}")))
}

/// Validate a bundle for completeness, raising ValueError on failure.
///
/// Raises ValueError if the bundle is incomplete for mounting.
#[pyfunction]
fn validate_bundle_completeness_or_raise(bundle: &PyBundle) -> PyResult<()> {
    crate::bundle::validator::validate_bundle_completeness_or_raise(&bundle.inner)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(format!("{e}")))
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
// Module definition
// =============================================================================

/// Python module definition.
#[pymodule]
fn amplifier_foundation(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;

    // Types
    m.add_class::<PyParsedURI>()?;
    m.add_class::<PyBundle>()?;
    m.add_class::<PyValidationResult>()?;

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
    Ok(())
}

// Tests for pyo3_bindings require Python dev headers for linking.
// The helper functions (pyobject_to_yaml, yaml_to_pyobject) are
// compile-checked via `cargo check --features pyo3-bindings` and
// exercised through the Python test harness when built with maturin.
//
// Pure conversion logic tests that don't need pyo3 are in
// tests/test_pyo3_helpers.rs (always compiled without pyo3 feature).
