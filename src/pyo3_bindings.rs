//! PyO3 bindings for amplifier-foundation.
//!
//! Provides Python-accessible types and functions when the `pyo3-bindings`
//! feature is enabled. The module is importable as `amplifier_foundation`
//! from Python.
//!
//! ## Exposed types
//!
//! - `ParsedURI` — URI parsing result
//!
//! ## Exposed functions
//!
//! - `parse_uri(uri)` — parse a URI string into components (always succeeds;
//!   unrecognized URIs are treated as package names)
//! - `normalize_path(path)` — normalize a filesystem path
//! - `deep_merge(base, overlay)` — deep merge two dicts (as JSON strings).
//!   v1 limitation: accepts JSON strings, not native Python dicts. Use
//!   `json.dumps()`/`json.loads()` for now. A future version will accept
//!   dicts directly via `pythonize` crate.
//! - `parse_mentions(text)` — extract @mentions from text
//! - `generate_sub_session_id(agent_name, session_id, trace_id)` — generate child session ID

use pyo3::prelude::*;

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

/// Parse a URI string into its components.
///
/// Handles git+, zip+, file://, http/https, and local paths.
/// Always succeeds — unrecognized URIs are treated as package names.
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

/// Deep merge two dicts (as JSON strings).
///
/// v1 limitation: accepts JSON strings, not native Python dicts.
/// Usage:
///   ```python
///   import json
///   result = json.loads(deep_merge(json.dumps(base), json.dumps(overlay)))
///   ```
///
/// A future version will accept Python dicts directly.
#[pyfunction]
fn deep_merge(base_json: &str, overlay_json: &str) -> PyResult<String> {
    let base: serde_yaml_ng::Value = serde_json::from_str(base_json)
        .map(json_to_yaml)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(format!("Invalid base JSON: {e}")))?;
    let overlay: serde_yaml_ng::Value = serde_json::from_str(overlay_json)
        .map(json_to_yaml)
        .map_err(|e| {
            pyo3::exceptions::PyValueError::new_err(format!("Invalid overlay JSON: {e}"))
        })?;

    let merged = crate::dicts::merge::deep_merge(&base, &overlay);

    yaml_to_json(&merged)
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

/// Convert serde_json::Value to serde_yaml_ng::Value.
///
/// Note: Large u64 values (> i64::MAX) fall back to f64 representation,
/// which may lose precision for values > 2^53. The final else branch
/// (numbers that are neither i64 nor f64) is unreachable for standard
/// serde_json without the `arbitrary_precision` feature.
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
                // Unreachable without serde_json's `arbitrary_precision` feature.
                unreachable!("serde_json::Number should always be representable as i64 or f64")
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

/// Convert serde_yaml_ng::Value to a JSON string.
///
/// After `json_to_yaml`, the value should always be JSON-safe (no YAML-specific
/// types like NaN, Infinity, or non-string keys). Uses `serde_json::to_string`
/// which cannot fail for JSON-safe values.
fn yaml_to_json(v: &serde_yaml_ng::Value) -> PyResult<String> {
    serde_json::to_string(v).map_err(|e| {
        pyo3::exceptions::PyValueError::new_err(format!("Failed to serialize result: {e}"))
    })
}

/// Python module definition.
#[pymodule]
fn amplifier_foundation(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add_class::<PyParsedURI>()?;
    m.add_function(wrap_pyfunction!(parse_uri, m)?)?;
    m.add_function(wrap_pyfunction!(normalize_path, m)?)?;
    m.add_function(wrap_pyfunction!(deep_merge, m)?)?;
    m.add_function(wrap_pyfunction!(parse_mentions, m)?)?;
    m.add_function(wrap_pyfunction!(generate_sub_session_id, m)?)?;
    Ok(())
}

// Tests for pyo3_bindings require Python dev headers for linking.
// The helper functions (json_to_yaml, yaml_to_json) are compile-checked
// via `cargo check --features pyo3-bindings` and exercised through the
// Python test harness when built with maturin.
//
// Pure conversion logic tests that don't need pyo3 are in
// tests/test_pyo3_helpers.rs (always compiled without pyo3 feature).
