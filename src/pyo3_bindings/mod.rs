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
//! - `ForkResult` -- result of a session fork operation (frozen)
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
//! - `validate_bundle_or_raise(bundle)` -- validate a bundle, raising on failure
//! - `validate_bundle_completeness_or_raise(bundle)` -- strict validation, raising on failure
//! - `apply_provider_preferences(mount_plan, prefs)` -- apply provider preferences
//! - `is_glob_pattern(pattern)` -- check for glob pattern characters
//! - `sanitize_for_json(data)` -- recursively sanitize data for JSON (removes nulls)
//! - `sanitize_message(message)` -- sanitize a chat message for persistence
//! - `merge_module_lists(parent, child)` -- merge module lists by module ID
//! - `format_directory_listing(path)` -- format directory contents listing
//! - `get_amplifier_home()` -- return the Amplifier home directory
//! - `construct_agent_path(base, name)` -- construct path to agent file
//! - `construct_context_path(base, name)` -- construct path to bundle resource
//! - `get_nested(data, path)` -- get value from nested dict by path
//! - `get_nested_with_default(data, path, default)` -- get with fallback default
//! - `set_nested(data, path, value)` -- set value in nested dict by path
//! - `count_turns(messages)` -- count user turns in a conversation
//! - `get_turn_boundaries(messages)` -- get 0-indexed positions of user messages
//! - `slice_to_turn(messages, turn, handle_orphaned_tools=None)` -- slice messages up to turn N
//! - `find_orphaned_tool_calls(messages)` -- find tool calls without results
//! - `add_synthetic_tool_results(messages, ids)` -- add synthetic results for orphans
//! - `get_turn_summary(messages, turn)` -- get summary dict for a turn
//! - `fork_session(session_dir, ...)` -- fork a stored session at a turn
//! - `fork_session_in_memory(messages, ...)` -- fork in-memory without disk I/O
//! - `get_fork_preview(session_dir, turn)` -- preview a fork without creating files
//! - `list_session_forks(session_dir)` -- list all forks of a session
//! - `get_session_lineage(session_dir)` -- get full ancestor/child lineage
//! - `count_events(events_path)` -- count events in events.jsonl
//! - `get_event_summary(events_path)` -- get summary of events file
//!
//! ## Exposed exceptions
//!
//! - `BundleError` -- base exception for all bundle operations
//! - `BundleNotFoundError` -- bundle could not be located
//! - `BundleLoadError` -- bundle could not be loaded
//! - `BundleValidationError` -- bundle validation failed
//! - `BundleDependencyError` -- dependency could not be resolved

mod exceptions;
mod functions;
mod helpers;
mod session;
mod types;

use pyo3::prelude::*;

use exceptions::{
    BundleDependencyError, BundleError, BundleLoadError, BundleNotFoundError, BundleValidationError,
};
use functions::{
    apply_provider_preferences, construct_agent_path, construct_context_path, deep_merge,
    deep_merge_json, format_directory_listing, generate_sub_session_id, get_amplifier_home,
    get_nested, get_nested_with_default, is_glob_pattern, merge_module_lists, normalize_path,
    parse_mentions, parse_uri, sanitize_for_json, sanitize_message, set_nested, validate_bundle,
    validate_bundle_completeness, validate_bundle_completeness_or_raise, validate_bundle_or_raise,
};
use session::{
    add_synthetic_tool_results, count_events, count_turns, find_orphaned_tool_calls, fork_session,
    fork_session_in_memory, get_event_summary, get_fork_preview, get_session_lineage,
    get_turn_boundaries, get_turn_summary, list_session_forks, slice_to_turn, PyForkResult,
};
use types::{
    PyBundle, PyDiskCache, PyParsedURI, PyProviderPreference, PyResolvedSource, PySimpleCache,
    PySourceStatus, PyValidationResult,
};

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
    m.add_function(wrap_pyfunction!(get_amplifier_home, m)?)?;
    m.add_function(wrap_pyfunction!(construct_agent_path, m)?)?;
    m.add_function(wrap_pyfunction!(construct_context_path, m)?)?;
    m.add_function(wrap_pyfunction!(get_nested, m)?)?;
    m.add_function(wrap_pyfunction!(get_nested_with_default, m)?)?;
    m.add_function(wrap_pyfunction!(set_nested, m)?)?;

    // Session types
    m.add_class::<PyForkResult>()?;

    // Session slice functions
    m.add_function(wrap_pyfunction!(count_turns, m)?)?;
    m.add_function(wrap_pyfunction!(get_turn_boundaries, m)?)?;
    m.add_function(wrap_pyfunction!(slice_to_turn, m)?)?;
    m.add_function(wrap_pyfunction!(find_orphaned_tool_calls, m)?)?;
    m.add_function(wrap_pyfunction!(add_synthetic_tool_results, m)?)?;
    m.add_function(wrap_pyfunction!(get_turn_summary, m)?)?;

    // Session fork functions
    m.add_function(wrap_pyfunction!(fork_session, m)?)?;
    m.add_function(wrap_pyfunction!(fork_session_in_memory, m)?)?;
    m.add_function(wrap_pyfunction!(get_fork_preview, m)?)?;
    m.add_function(wrap_pyfunction!(list_session_forks, m)?)?;
    m.add_function(wrap_pyfunction!(get_session_lineage, m)?)?;

    // Session events functions
    m.add_function(wrap_pyfunction!(count_events, m)?)?;
    m.add_function(wrap_pyfunction!(get_event_summary, m)?)?;
    Ok(())
}

// Tests for pyo3 bindings require Python dev headers for linking.
// The helper functions (pyobject_to_yaml, yaml_to_pyobject) are
// compile-checked via `cargo check --features pyo3-bindings` and
// exercised through the Python test harness when built with maturin.
