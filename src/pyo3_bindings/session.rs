//! PyO3 bindings for session slice functions.
//!
//! Exposes session slice utility functions to Python.
//! All session functions operate on `serde_json::Value` (chat messages are JSON),
//! so we use `pyobject_to_json` / `json_to_pyobject` for conversions.

use pyo3::prelude::*;

use super::exceptions::bundle_error_to_pyerr;
use super::helpers::{json_to_pyobject, pyobject_to_json};

// =============================================================================
// Session slice functions
// =============================================================================

/// Count the number of turns (user messages) in a conversation.
///
/// A turn is defined as a user message plus all subsequent non-user messages
/// until the next user message.
///
/// Args:
///     messages: List of chat message dicts (each with "role" and "content" keys).
///
/// Returns:
///     Number of turns (user messages) in the conversation.
#[pyfunction]
pub(super) fn count_turns(messages: &Bound<'_, PyAny>) -> PyResult<usize> {
    let msgs = pyobject_to_json_list(messages)?;
    Ok(crate::session::count_turns(&msgs))
}

/// Return 0-indexed positions of each turn boundary (user message positions).
///
/// Args:
///     messages: List of chat message dicts.
///
/// Returns:
///     List of 0-indexed positions where each user message starts a new turn.
#[pyfunction]
pub(super) fn get_turn_boundaries(messages: &Bound<'_, PyAny>) -> PyResult<Vec<usize>> {
    let msgs = pyobject_to_json_list(messages)?;
    Ok(crate::session::get_turn_boundaries(&msgs))
}

/// Slice messages to include only up to turn N (1-indexed).
///
/// Turn N includes the Nth user message and all responses until the
/// next user message (or end of conversation).
///
/// Args:
///     messages: List of chat message dicts.
///     turn: 1-indexed turn number to slice to.
///     handle_orphaned_tools: How to handle tool calls without results:
///         - "complete" or None: Add synthetic error result (default)
///         - "remove": Remove the orphaned tool_use content
///         - "error": Raise an error
///
/// Returns:
///     List of message dicts up to and including the specified turn.
///
/// Raises:
///     BundleLoadError: If turn < 1, turn exceeds the number of turns,
///         no user messages found, or handle_orphaned_tools="error" and
///         orphaned tools exist.
#[pyfunction]
#[pyo3(signature = (messages, turn, handle_orphaned_tools=None))]
pub(super) fn slice_to_turn(
    py: Python<'_>,
    messages: &Bound<'_, PyAny>,
    turn: isize,
    handle_orphaned_tools: Option<&str>,
) -> PyResult<PyObject> {
    if turn < 1 {
        return Err(bundle_error_to_pyerr(
            crate::error::BundleError::LoadError {
                reason: format!("Turn must be >= 1, got {turn}"),
                source: None,
            },
        ));
    }
    let msgs = pyobject_to_json_list(messages)?;
    let result = crate::session::slice_to_turn(&msgs, turn as usize, handle_orphaned_tools)
        .map_err(bundle_error_to_pyerr)?;
    json_list_to_pyobject(py, result)
}

/// Find tool call IDs that have no matching tool result.
///
/// Detects both OpenAI-format (tool_calls array in assistant messages)
/// and Anthropic-format (content blocks with type=tool_use) orphaned calls.
///
/// Args:
///     messages: List of chat message dicts.
///
/// Returns:
///     List of orphaned tool call ID strings.
#[pyfunction]
pub(super) fn find_orphaned_tool_calls(messages: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    let msgs = pyobject_to_json_list(messages)?;
    Ok(crate::session::find_orphaned_tool_calls(&msgs))
}

/// Add synthetic tool result messages for orphaned tool calls.
///
/// For each orphaned tool call ID, appends a synthetic tool result message
/// with an error indicating the session was forked/interrupted.
///
/// Args:
///     messages: List of chat message dicts.
///     orphaned_ids: List of tool call ID strings to add synthetic results for.
///
/// Returns:
///     New list of message dicts with synthetic results appended.
#[pyfunction]
pub(super) fn add_synthetic_tool_results(
    py: Python<'_>,
    messages: &Bound<'_, PyAny>,
    orphaned_ids: Vec<String>,
) -> PyResult<PyObject> {
    let msgs = pyobject_to_json_list(messages)?;
    let result = crate::session::add_synthetic_tool_results(&msgs, &orphaned_ids);
    json_list_to_pyobject(py, result)
}

/// Get a summary of a specific turn in the conversation.
///
/// Returns a dict with: turn, user_content, assistant_content,
/// tool_count, message_count.
///
/// Args:
///     messages: List of chat message dicts.
///     turn: 1-indexed turn number.
///
/// Returns:
///     Dict with turn summary information.
///
/// Raises:
///     BundleLoadError: If turn is out of range or no user messages found.
#[pyfunction]
pub(super) fn get_turn_summary(
    py: Python<'_>,
    messages: &Bound<'_, PyAny>,
    turn: isize,
) -> PyResult<PyObject> {
    if turn < 1 {
        return Err(bundle_error_to_pyerr(
            crate::error::BundleError::LoadError {
                reason: format!("Turn must be >= 1, got {turn}"),
                source: None,
            },
        ));
    }
    let msgs = pyobject_to_json_list(messages)?;
    let result =
        crate::session::get_turn_summary(&msgs, turn as usize).map_err(bundle_error_to_pyerr)?;
    json_to_pyobject(py, &result)
}

// =============================================================================
// Internal helpers
// =============================================================================

/// Convert a Python list to Vec<serde_json::Value>.
fn pyobject_to_json_list(obj: &Bound<'_, PyAny>) -> PyResult<Vec<serde_json::Value>> {
    let json_val = pyobject_to_json(obj)?;
    match json_val {
        serde_json::Value::Array(arr) => Ok(arr),
        _ => Err(pyo3::exceptions::PyTypeError::new_err(
            "Expected a list of message dicts",
        )),
    }
}

/// Convert owned Vec<serde_json::Value> to a Python list (zero-copy move).
fn json_list_to_pyobject(py: Python<'_>, items: Vec<serde_json::Value>) -> PyResult<PyObject> {
    let json_arr = serde_json::Value::Array(items);
    json_to_pyobject(py, &json_arr)
}
