//! PyO3 bindings for session utility functions.
//!
//! Exposes session slice, fork, and events functions to Python.
//! All session functions operate on `serde_json::Value` (chat messages are JSON),
//! so we use `pyobject_to_json` / `json_to_pyobject` for conversions.
//!
//! ## Not exposed (by design)
//!
//! - `slice_events_to_timestamp`, `slice_events_for_fork`, `get_last_timestamp_for_turn`:
//!   Low-level event slicing primitives used internally by `fork_session`. Expose if needed.
//! - `get_working_dir`, `set_working_dir`: Thin JSON key accessors, trivial in Python.

use std::path::Path;

use pyo3::prelude::*;

use crate::exceptions::bundle_error_to_pyerr;
use crate::helpers::{json_to_pyobject, pyobject_to_json};

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
#[pyo3(text_signature = "(messages)")]
pub(crate) fn count_turns(messages: &Bound<'_, PyAny>) -> PyResult<usize> {
    let msgs = pyobject_to_json_list(messages)?;
    Ok(amplifier_foundation::session::count_turns(&msgs))
}

/// Return 0-indexed positions of each turn boundary (user message positions).
///
/// Args:
///     messages: List of chat message dicts.
///
/// Returns:
///     List of 0-indexed positions where each user message starts a new turn.
#[pyfunction]
#[pyo3(text_signature = "(messages)")]
pub(crate) fn get_turn_boundaries(messages: &Bound<'_, PyAny>) -> PyResult<Vec<usize>> {
    let msgs = pyobject_to_json_list(messages)?;
    Ok(amplifier_foundation::session::get_turn_boundaries(&msgs))
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
#[pyo3(text_signature = "(messages, turn, handle_orphaned_tools=None)")]
#[pyo3(signature = (messages, turn, handle_orphaned_tools=None))]
pub(crate) fn slice_to_turn(
    py: Python<'_>,
    messages: &Bound<'_, PyAny>,
    turn: isize,
    handle_orphaned_tools: Option<&str>,
) -> PyResult<PyObject> {
    if turn < 1 {
        return Err(bundle_error_to_pyerr(
            amplifier_foundation::error::BundleError::LoadError {
                reason: format!("Turn must be >= 1, got {turn}"),
                source: None,
            },
        ));
    }
    let msgs = pyobject_to_json_list(messages)?;
    let result = amplifier_foundation::session::slice_to_turn(&msgs, turn as usize, handle_orphaned_tools)
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
#[pyo3(text_signature = "(messages)")]
pub(crate) fn find_orphaned_tool_calls(messages: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    let msgs = pyobject_to_json_list(messages)?;
    Ok(amplifier_foundation::session::find_orphaned_tool_calls(&msgs))
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
#[pyo3(text_signature = "(messages, orphaned_ids)")]
pub(crate) fn add_synthetic_tool_results(
    py: Python<'_>,
    messages: &Bound<'_, PyAny>,
    orphaned_ids: Vec<String>,
) -> PyResult<PyObject> {
    let msgs = pyobject_to_json_list(messages)?;
    let result = amplifier_foundation::session::add_synthetic_tool_results(&msgs, &orphaned_ids);
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
#[pyo3(text_signature = "(messages, turn)")]
pub(crate) fn get_turn_summary(
    py: Python<'_>,
    messages: &Bound<'_, PyAny>,
    turn: isize,
) -> PyResult<PyObject> {
    if turn < 1 {
        return Err(bundle_error_to_pyerr(
            amplifier_foundation::error::BundleError::LoadError {
                reason: format!("Turn must be >= 1, got {turn}"),
                source: None,
            },
        ));
    }
    let msgs = pyobject_to_json_list(messages)?;
    let result =
        amplifier_foundation::session::get_turn_summary(&msgs, turn as usize).map_err(bundle_error_to_pyerr)?;
    json_to_pyobject(py, &result)
}

// =============================================================================
// ForkResult type
// =============================================================================

/// Result of a session fork operation (frozen, immutable).
///
/// Contains metadata about the fork: session ID, parent relationship,
/// which turn was forked from, and the resulting messages/events.
///
/// For file-based forks: session_dir is set, messages is None.
/// For in-memory forks: session_dir is None, messages is set.
///
/// No __eq__/__hash__: each ForkResult contains a unique random session_id,
/// so two results are never semantically equal unless they're the same object.
#[pyclass(name = "ForkResult", frozen)]
pub(crate) struct PyForkResult {
    inner: amplifier_foundation::session::ForkResult,
}

#[pymethods]
impl PyForkResult {
    /// The session ID of the new forked session.
    #[getter]
    fn session_id(&self) -> &str {
        &self.inner.session_id
    }

    /// Directory path of the new session (None for in-memory forks).
    #[getter]
    fn session_dir(&self) -> Option<String> {
        self.inner
            .session_dir
            .as_ref()
            .map(|p| p.display().to_string())
    }

    /// Session ID of the parent session.
    #[getter]
    fn parent_id(&self) -> &str {
        &self.inner.parent_id
    }

    /// The turn number the fork was created from (1-indexed).
    #[getter]
    fn forked_from_turn(&self) -> usize {
        self.inner.forked_from_turn
    }

    /// Number of messages in the forked transcript.
    #[getter]
    fn message_count(&self) -> usize {
        self.inner.message_count
    }

    /// The forked messages (only set for in-memory forks).
    ///
    /// Note: Each access deep-copies the message list from Rust to Python.
    /// Store the result in a variable if you need to access it multiple times.
    #[getter]
    fn messages(&self, py: Python<'_>) -> PyResult<Option<PyObject>> {
        match &self.inner.messages {
            Some(msgs) => {
                let json_arr = serde_json::Value::Array(msgs.clone());
                let obj = json_to_pyobject(py, &json_arr)?;
                Ok(Some(obj))
            }
            None => Ok(None),
        }
    }

    /// Number of events sliced (0 for in-memory forks).
    #[getter]
    fn events_count(&self) -> usize {
        self.inner.events_count
    }

    fn __repr__(&self) -> String {
        format!(
            "ForkResult(session_id='{}', parent_id='{}', turn={}, message_count={})",
            self.inner.session_id,
            self.inner.parent_id,
            self.inner.forked_from_turn,
            self.inner.message_count,
        )
    }
}

// =============================================================================
// Session fork functions
// =============================================================================

/// Fork a stored session from a specific turn.
///
/// Creates a new session directory with sliced transcript, metadata with
/// parent lineage, and optionally sliced events.
///
/// Args:
///     session_dir: Path to the source session directory.
///     turn: 1-indexed turn to fork from (None = last turn).
///     new_session_id: UUID for the new session (None = auto-generated).
///     target_dir: Directory to create the new session in (None = sibling of source).
///     include_events: Whether to slice events.jsonl into the fork.
///
/// Returns:
///     ForkResult with fork metadata.
///
/// Raises:
///     BundleLoadError: If session_dir is invalid, transcript is missing,
///         or turn is out of range.
#[pyfunction]
#[pyo3(
    text_signature = "(session_dir, turn=None, new_session_id=None, target_dir=None, include_events=True)"
)]
#[pyo3(signature = (session_dir, turn=None, new_session_id=None, target_dir=None, include_events=true))]
pub(crate) fn fork_session(
    session_dir: &str,
    turn: Option<usize>,
    new_session_id: Option<&str>,
    target_dir: Option<&str>,
    include_events: bool,
) -> PyResult<PyForkResult> {
    let result = amplifier_foundation::session::fork_session(
        Path::new(session_dir),
        turn,
        new_session_id,
        target_dir.map(Path::new),
        include_events,
    )
    .map_err(bundle_error_to_pyerr)?;
    Ok(PyForkResult { inner: result })
}

/// Fork a session in-memory without writing to disk.
///
/// Slices messages and returns the result without creating files.
/// Useful for previewing or manipulating session data.
///
/// Args:
///     messages: List of chat message dicts.
///     turn: 1-indexed turn to fork from (None = last turn).
///     parent_id: Parent session ID (None = "unknown").
///
/// Returns:
///     ForkResult with messages field populated.
///
/// Raises:
///     BundleLoadError: If no user messages found or turn is out of range.
#[pyfunction]
#[pyo3(text_signature = "(messages, turn=None, parent_id=None)")]
#[pyo3(signature = (messages, turn=None, parent_id=None))]
pub(crate) fn fork_session_in_memory(
    messages: &Bound<'_, PyAny>,
    turn: Option<usize>,
    parent_id: Option<&str>,
) -> PyResult<PyForkResult> {
    let msgs = pyobject_to_json_list(messages)?;
    let result = amplifier_foundation::session::fork_session_in_memory(&msgs, turn, parent_id)
        .map_err(bundle_error_to_pyerr)?;
    Ok(PyForkResult { inner: result })
}

/// Preview a fork without creating files.
///
/// Returns metadata about what the fork would produce: turn info,
/// message count, orphaned tool status, and last messages.
///
/// Args:
///     session_dir: Path to the source session directory.
///     turn: 1-indexed turn to preview forking from.
///
/// Returns:
///     Dict with preview information.
///
/// Raises:
///     BundleLoadError: If session_dir is invalid or turn is out of range.
#[pyfunction]
#[pyo3(text_signature = "(session_dir, turn)")]
pub(crate) fn get_fork_preview(
    py: Python<'_>,
    session_dir: &str,
    turn: usize,
) -> PyResult<PyObject> {
    let result = amplifier_foundation::session::get_fork_preview(Path::new(session_dir), turn)
        .map_err(bundle_error_to_pyerr)?;
    json_to_pyobject(py, &result)
}

/// List all sessions that were forked from the given session.
///
/// Scans sibling directories for sessions whose metadata.parent_id
/// matches the given session's ID.
///
/// Args:
///     session_dir: Path to the session directory.
///
/// Returns:
///     List of dicts, each with: session_id, session_dir, forked_from_turn,
///     forked_at, turn_count.
///
/// Raises:
///     BundleLoadError: If session_dir is invalid.
#[pyfunction]
#[pyo3(text_signature = "(session_dir)")]
pub(crate) fn list_session_forks(py: Python<'_>, session_dir: &str) -> PyResult<PyObject> {
    let result = amplifier_foundation::session::list_session_forks(Path::new(session_dir))
        .map_err(bundle_error_to_pyerr)?;
    let json_arr = serde_json::Value::Array(result);
    json_to_pyobject(py, &json_arr)
}

/// Get the full lineage (ancestors + children) of a session.
///
/// Returns a dict with: session_id, parent_id, forked_from_turn,
/// ancestors (list), children (list), depth.
///
/// Args:
///     session_dir: Path to the session directory.
///
/// Returns:
///     Dict with lineage information.
///
/// Raises:
///     BundleLoadError: If session_dir is invalid.
#[pyfunction]
#[pyo3(text_signature = "(session_dir)")]
pub(crate) fn get_session_lineage(py: Python<'_>, session_dir: &str) -> PyResult<PyObject> {
    let result = amplifier_foundation::session::get_session_lineage(Path::new(session_dir))
        .map_err(bundle_error_to_pyerr)?;
    json_to_pyobject(py, &result)
}

// =============================================================================
// Session events functions
// =============================================================================

/// Count the number of events in an events.jsonl file.
///
/// Returns 0 if the file doesn't exist or cannot be read (infallible).
///
/// Args:
///     events_path: Path to the events.jsonl file.
///
/// Returns:
///     Number of events in the file.
#[pyfunction]
#[pyo3(text_signature = "(events_path)")]
pub(crate) fn count_events(events_path: &str) -> usize {
    amplifier_foundation::session::count_events(Path::new(events_path))
}

/// Get a summary of events in an events.jsonl file.
///
/// Returns a dict with: total_events, event_types (type -> count mapping),
/// first_timestamp, last_timestamp.
///
/// Returns a zero-count summary if the file doesn't exist. Raises
/// BundleLoadError if the file exists but cannot be read or parsed.
///
/// Args:
///     events_path: Path to the events.jsonl file.
///
/// Returns:
///     Dict with event summary information.
///
/// Raises:
///     BundleLoadError: If the events file exists but cannot be read or parsed.
#[pyfunction]
#[pyo3(text_signature = "(events_path)")]
pub(crate) fn get_event_summary(py: Python<'_>, events_path: &str) -> PyResult<PyObject> {
    let result =
        amplifier_foundation::session::get_event_summary(Path::new(events_path)).map_err(bundle_error_to_pyerr)?;
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

/// Convert owned Vec<serde_json::Value> to a Python list.
///
/// Moves the Vec into a Value::Array wrapper, then converts to Python
/// via pythonize (which traverses and creates Python objects).
fn json_list_to_pyobject(py: Python<'_>, items: Vec<serde_json::Value>) -> PyResult<PyObject> {
    let json_arr = serde_json::Value::Array(items);
    json_to_pyobject(py, &json_arr)
}
