//! Standalone #[pyfunction] implementations.
//!
//! Contains all Python-callable functions: parse_uri, normalize_path, deep_merge,
//! deep_merge_json, parse_mentions, generate_sub_session_id, validate_bundle,
//! validate_bundle_completeness, validate_bundle_or_raise,
//! validate_bundle_completeness_or_raise, apply_provider_preferences, is_glob_pattern,
//! sanitize_for_json, sanitize_message, merge_module_lists, format_directory_listing,
//! get_amplifier_home, construct_agent_path, construct_context_path,
//! get_nested, get_nested_with_default, set_nested.

use pyo3::prelude::*;

use super::exceptions::bundle_error_to_pyerr;
use super::helpers::{
    json_to_pyobject, json_to_yaml, pyobject_to_json, pyobject_to_yaml, yaml_to_pyobject,
};
use super::types::{PyBundle, PyParsedURI, PyProviderPreference, PyValidationResult};

// =============================================================================
// URI / path functions
// =============================================================================

/// Parse a URI string into its components.
///
/// Handles git+, zip+, file://, http/https, and local paths.
/// Always succeeds -- unrecognized URIs are treated as package names.
#[pyfunction]
pub(super) fn parse_uri(uri: &str) -> PyParsedURI {
    PyParsedURI {
        inner: crate::paths::uri::parse_uri(uri),
    }
}

/// Normalize a filesystem path (resolve . and .., make absolute).
///
/// Uses the current working directory as the base for relative paths.
/// Raises `UnicodeDecodeError` if the resolved path contains non-UTF-8 bytes.
#[pyfunction]
pub(super) fn normalize_path(path: &str) -> PyResult<String> {
    let p = crate::paths::uri::normalize_path(path, None);
    p.into_os_string().into_string().map_err(|os| {
        pyo3::exceptions::PyUnicodeDecodeError::new_err(format!(
            "Path contains non-UTF-8 bytes: {:?}",
            os
        ))
    })
}

// =============================================================================
// Dict merge functions
// =============================================================================

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
pub(super) fn deep_merge<'py>(
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
pub(super) fn deep_merge_json(base_json: &str, overlay_json: &str) -> PyResult<String> {
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

// =============================================================================
// Mention / session functions
// =============================================================================

/// Extract @mentions from text (excluding code blocks and emails).
#[pyfunction]
pub(super) fn parse_mentions(text: &str) -> Vec<String> {
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
pub(super) fn generate_sub_session_id(
    agent_name: Option<&str>,
    session_id: Option<&str>,
    trace_id: Option<&str>,
) -> String {
    crate::tracing_utils::generate_sub_session_id(agent_name, session_id, trace_id)
}

// =============================================================================
// Validation functions
// =============================================================================

/// Validate a bundle (basic validation: required fields + module list format).
///
/// Returns a ValidationResult with errors and warnings.
#[pyfunction]
pub(super) fn validate_bundle(bundle: &PyBundle) -> PyValidationResult {
    let result = crate::bundle::validator::validate_bundle(&bundle.inner);
    result.into()
}

/// Validate a bundle for completeness (strict: requires session, orchestrator, providers).
///
/// Returns a ValidationResult with errors and warnings.
#[pyfunction]
pub(super) fn validate_bundle_completeness(bundle: &PyBundle) -> PyValidationResult {
    let result = crate::bundle::validator::validate_bundle_completeness(&bundle.inner);
    result.into()
}

/// Validate a bundle, raising BundleValidationError on failure.
///
/// Raises BundleValidationError if the bundle has validation errors.
#[pyfunction]
pub(super) fn validate_bundle_or_raise(bundle: &PyBundle) -> PyResult<()> {
    crate::bundle::validator::validate_bundle_or_raise(&bundle.inner).map_err(bundle_error_to_pyerr)
}

/// Validate a bundle for completeness, raising BundleValidationError on failure.
///
/// Raises BundleValidationError if the bundle is incomplete for mounting.
#[pyfunction]
pub(super) fn validate_bundle_completeness_or_raise(bundle: &PyBundle) -> PyResult<()> {
    crate::bundle::validator::validate_bundle_completeness_or_raise(&bundle.inner)
        .map_err(bundle_error_to_pyerr)
}

// =============================================================================
// Provider preference functions
// =============================================================================

/// Apply provider preferences to a mount plan.
///
/// Takes a mount plan dict and a list of ProviderPreference objects.
/// Returns a new mount plan dict with the preferred provider promoted.
///
/// This is the sync version -- does NOT resolve glob patterns. For glob
/// resolution, use the async variant from Python directly.
#[pyfunction]
pub(super) fn apply_provider_preferences<'py>(
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
pub(super) fn is_glob_pattern(pattern: &str) -> bool {
    crate::spawn::glob::is_glob_pattern(pattern)
}

// =============================================================================
// Serialization / utility functions
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
pub(super) fn sanitize_for_json<'py>(
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
pub(super) fn sanitize_message<'py>(
    py: Python<'py>,
    message: &Bound<'py, PyAny>,
) -> PyResult<PyObject> {
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
pub(super) fn merge_module_lists<'py>(
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
pub(super) fn format_directory_listing(path: &str) -> String {
    crate::mentions::utils::format_directory_listing(std::path::Path::new(path))
}

// =============================================================================
// Path utility functions
// =============================================================================

/// Convert a PathBuf to a Python-friendly String, raising UnicodeDecodeError
/// on non-UTF-8 paths (practically unreachable on modern systems).
fn pathbuf_to_pystring(p: std::path::PathBuf) -> PyResult<String> {
    p.into_os_string().into_string().map_err(|os| {
        pyo3::exceptions::PyUnicodeDecodeError::new_err(format!(
            "Path contains non-UTF-8 bytes: {:?}",
            os
        ))
    })
}

/// Return the Amplifier home directory.
///
/// Uses ``$AMPLIFIER_HOME`` if set, otherwise ``~/.amplifier``.
/// Falls back to ``./.amplifier`` (current directory) if the home directory
/// cannot be determined.
#[pyfunction]
pub(super) fn get_amplifier_home() -> PyResult<String> {
    pathbuf_to_pystring(crate::paths::uri::get_amplifier_home())
}

/// Construct the path to an agent file.
///
/// Looks in the ``agents/`` subdirectory of ``base``, appends ``.md`` extension
/// if not already present.
///
/// Args:
///     base: Base directory path.
///     name: Agent name or filename.
///
/// Returns:
///     Path string to the agent file.
#[pyfunction]
pub(super) fn construct_agent_path(base: &str, name: &str) -> PyResult<String> {
    pathbuf_to_pystring(crate::paths::normalize::construct_agent_path(
        std::path::Path::new(base),
        name,
    ))
}

/// Construct the path to a bundle resource file.
///
/// The name is relative to the bundle root. Empty name returns the base path.
/// Leading ``/`` is stripped to prevent absolute path creation.
///
/// Args:
///     base: Base directory path.
///     name: Resource name (relative path).
///
/// Returns:
///     Path string to the resource.
#[pyfunction]
pub(super) fn construct_context_path(base: &str, name: &str) -> PyResult<String> {
    pathbuf_to_pystring(crate::paths::normalize::construct_context_path(
        std::path::Path::new(base),
        name,
    ))
}

// =============================================================================
// Dict navigation functions
// =============================================================================
//
// NOTE: These functions convert Python dicts through serde_yaml_ng::Value.
// This means: (1) returned values are deep copies, not references to original
// objects, (2) only JSON-like types are supported (dicts, lists, strings,
// numbers, bools, None), and (3) integer dict keys are not supported.
// This matches the Rust API which operates on serde_yaml_ng::Value.

/// Get a value from a nested dict by path.
///
/// Traverses the dict using a list of string keys. Returns ``None`` if
/// the path is not found or any intermediate value is not a dict.
///
/// Note: The returned value is a deep copy, not a reference to the original.
///
/// Args:
///     data: A Python dict.
///     path: List of string keys to traverse.
///
/// Returns:
///     The value at the path, or ``None`` if not found.
///
/// Example::
///
///     >>> get_nested({"a": {"b": 42}}, ["a", "b"])
///     42
///     >>> get_nested({"a": 1}, ["a", "b"])  # intermediate not a dict
///     None
#[pyfunction]
pub(super) fn get_nested(data: &Bound<'_, PyAny>, path: Vec<String>) -> PyResult<Option<PyObject>> {
    let yaml_val = pyobject_to_yaml(data)?;
    let path_refs: Vec<&str> = path.iter().map(|s| s.as_str()).collect();
    match crate::dicts::nested::get_nested(&yaml_val, &path_refs) {
        Some(v) => {
            let py = data.py();
            let obj = yaml_to_pyobject(py, &v)?;
            Ok(Some(obj))
        }
        None => Ok(None),
    }
}

/// Get a value from a nested dict by path, with a default.
///
/// Like :func:`get_nested` but returns ``default`` instead of ``None``
/// when the path is not found. The ``default`` is returned as-is (no
/// conversion) when the path is missing.
///
/// Args:
///     data: A Python dict.
///     path: List of string keys to traverse.
///     default: Value to return if path not found (returned as-is).
///
/// Returns:
///     The value at the path (deep copy), or ``default`` if not found.
#[pyfunction]
pub(super) fn get_nested_with_default(
    data: &Bound<'_, PyAny>,
    path: Vec<String>,
    default: &Bound<'_, PyAny>,
) -> PyResult<PyObject> {
    let yaml_val = pyobject_to_yaml(data)?;
    let path_refs: Vec<&str> = path.iter().map(|s| s.as_str()).collect();
    match crate::dicts::nested::get_nested(&yaml_val, &path_refs) {
        Some(v) => {
            let py = data.py();
            yaml_to_pyobject(py, &v)
        }
        // Return the default as-is without YAML round-trip.
        // This preserves object identity and supports non-serializable defaults.
        None => Ok(default.clone().unbind()),
    }
}

/// Set a value in a nested dict by path, returning the modified copy.
///
/// Creates intermediate dicts as needed. Empty path returns a copy of data
/// unchanged.
///
/// NOTE: Does NOT mutate the input dict. Returns a new dict with the change
/// applied (the input goes through Rust conversion and back).
///
/// Args:
///     data: A Python dict.
///     path: List of string keys to traverse.
///     value: Value to set at the path.
///
/// Returns:
///     A new dict with the value set at the given path.
///
/// Example::
///
///     >>> set_nested({"a": {}}, ["a", "b"], 42)
///     {'a': {'b': 42}}
#[pyfunction]
pub(super) fn set_nested(
    data: &Bound<'_, PyAny>,
    path: Vec<String>,
    value: &Bound<'_, PyAny>,
) -> PyResult<PyObject> {
    let mut yaml_val = pyobject_to_yaml(data)?;
    let yaml_value = pyobject_to_yaml(value)?;
    let path_refs: Vec<&str> = path.iter().map(|s| s.as_str()).collect();
    crate::dicts::nested::set_nested(&mut yaml_val, &path_refs, yaml_value);
    let py = data.py();
    yaml_to_pyobject(py, &yaml_val)
}
