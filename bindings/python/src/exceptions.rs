//! Python exception hierarchy for bundle operations.
//!
//! Defines custom exception classes that mirror the Rust `BundleError` enum variants,
//! plus a conversion function from Rust errors to Python exceptions.

// =============================================================================
// Python exception hierarchy
// =============================================================================
//
// NOTE: `create_exception!` puts the generated struct into this module's scope.
// The name `BundleError` shadows `amplifier_foundation::error::BundleError` (the Rust enum).
// Inside functions that need both types, alias the Rust enum:
//   `use amplifier_foundation::error::BundleError as BE;`

// First argument is the Python module name for exception __module__ attr.
// Kept as `amplifier_foundation` so tracebacks show the user-facing path.
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

/// Map a `amplifier_foundation::error::BundleError` to the appropriate Python exception subclass.
///
/// - `NotFound`        → `BundleNotFoundError`
/// - `LoadError`       → `BundleLoadError`
/// - `ValidationError` → `BundleValidationError` (formats actual error messages)
/// - `DependencyError` → `BundleDependencyError`
/// - `Io` / `Yaml` / `Http` / `Git` → `BundleLoadError`
pub(crate) fn bundle_error_to_pyerr(e: amplifier_foundation::error::BundleError) -> pyo3::PyErr {
    use amplifier_foundation::error::BundleError as BE;
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
