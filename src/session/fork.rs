use std::path::{Path, PathBuf};
use serde_json::Value;

/// Result of a fork operation.
#[derive(Debug, Clone)]
pub struct ForkResult {
    pub session_id: String,
    pub session_dir: Option<PathBuf>,
    pub parent_id: String,
    pub forked_from_turn: usize,
    pub message_count: usize,
    pub messages: Option<Vec<Value>>,
    pub events_count: usize,
}

/// Fork a session on disk at a given turn.
pub fn fork_session(
    session_dir: &Path,
    turn: Option<usize>,
    new_session_id: Option<&str>,
    target_dir: Option<&Path>,
    include_events: bool,
) -> crate::error::Result<ForkResult> {
    todo!()
}

/// Fork a session in memory (no disk I/O).
pub fn fork_session_in_memory(
    messages: &[Value],
    turn: Option<usize>,
    parent_id: Option<&str>,
) -> crate::error::Result<ForkResult> {
    todo!()
}

/// Get a preview of what a fork would produce.
pub fn get_fork_preview(session_dir: &Path, turn: usize) -> crate::error::Result<Value> {
    todo!()
}

/// List all forks of a session.
pub fn list_session_forks(session_dir: &Path) -> crate::error::Result<Vec<Value>> {
    todo!()
}

/// Get the lineage (ancestry chain) of a session.
pub fn get_session_lineage(session_dir: &Path) -> crate::error::Result<Value> {
    todo!()
}
