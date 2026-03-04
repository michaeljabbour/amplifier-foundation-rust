//! Conversion helpers between Python objects and Rust types.
//!
//! Provides direct Python <-> serde_yaml_ng::Value and Python <-> serde_json::Value
//! conversions via pythonize (no JSON intermediary for the YAML path).

use pyo3::prelude::*;

// =============================================================================
// Conversion helpers: Python <-> serde_yaml_ng::Value via pythonize (direct)
// =============================================================================

/// Convert a Python object (dict/list/str/int/float/bool/None) to serde_yaml_ng::Value.
///
/// Uses pythonize to deserialize directly into serde_yaml_ng::Value.
/// No JSON intermediary -- preserves YAML-specific types (Tagged values,
/// non-string mapping keys) through the conversion.
pub(super) fn pyobject_to_yaml(obj: &Bound<'_, PyAny>) -> PyResult<serde_yaml_ng::Value> {
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
pub(super) fn yaml_to_pyobject(py: Python<'_>, v: &serde_yaml_ng::Value) -> PyResult<PyObject> {
    let bound = pythonize::pythonize(py, v).map_err(|e| {
        pyo3::exceptions::PyValueError::new_err(format!(
            "Failed to convert Rust value to Python object: {e}"
        ))
    })?;
    Ok(bound.unbind())
}

// =============================================================================
// Internal conversion helpers (for legacy JSON interface only)
// =============================================================================

/// Convert serde_json::Value to serde_yaml_ng::Value.
///
/// Used only by `deep_merge_json` (legacy JSON string interface).
/// The main conversion path uses pythonize directly (no JSON intermediary).
pub(super) fn json_to_yaml(v: serde_json::Value) -> serde_yaml_ng::Value {
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
// JSON conversion helpers (for serialization functions)
// =============================================================================

/// Convert a Python object to serde_json::Value.
///
/// Uses pythonize for direct Python -> serde_json::Value conversion.
pub(super) fn pyobject_to_json(obj: &Bound<'_, PyAny>) -> PyResult<serde_json::Value> {
    pythonize::depythonize(obj).map_err(|e| {
        pyo3::exceptions::PyValueError::new_err(format!(
            "Failed to convert Python object to JSON value: {e}"
        ))
    })
}

/// Convert a serde_json::Value to a Python object.
pub(super) fn json_to_pyobject(py: Python<'_>, v: &serde_json::Value) -> PyResult<PyObject> {
    let bound = pythonize::pythonize(py, v).map_err(|e| {
        pyo3::exceptions::PyValueError::new_err(format!(
            "Failed to convert JSON value to Python object: {e}"
        ))
    })?;
    Ok(bound.unbind())
}
