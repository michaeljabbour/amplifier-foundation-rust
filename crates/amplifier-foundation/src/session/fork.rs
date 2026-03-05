//! Session fork operations for Amplifier.
//!
//! This module provides file-based and in-memory session forking with turn-aware
//! slicing and lineage tracking.
//!
//! Key concepts:
//! - Fork: Create a new session from an existing session at a specific turn
//! - Turn: A user message + all subsequent responses until the next user message
//! - Lineage: Parent-child relationship tracked via parent_id in metadata

use std::collections::HashSet;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde_json::{json, Value};
use uuid::Uuid;

use super::events::slice_events_for_fork;
use super::slice::{count_turns, find_orphaned_tool_calls, get_turn_boundaries, slice_to_turn};

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

/// Fork a stored session from a specific turn.
///
/// Creates a new session directory with:
/// - transcript.jsonl sliced to turn N
/// - metadata.json with parent lineage information
/// - events.jsonl sliced to turn boundary (if include_events=true)
pub fn fork_session(
    session_dir: &Path,
    turn: Option<usize>,
    new_session_id: Option<&str>,
    target_dir: Option<&Path>,
    include_events: bool,
) -> crate::error::Result<ForkResult> {
    let session_dir =
        fs::canonicalize(session_dir).map_err(|_| crate::error::BundleError::LoadError {
            reason: format!(
                "No transcript.jsonl in {}. This doesn't appear to be a valid session directory.",
                session_dir.display()
            ),
            source: None,
        })?;

    let transcript_path = session_dir.join("transcript.jsonl");
    let metadata_path = session_dir.join("metadata.json");
    let events_path = session_dir.join("events.jsonl");

    if !transcript_path.exists() {
        return Err(crate::error::BundleError::LoadError {
            reason: format!(
                "No transcript.jsonl in {}. This doesn't appear to be a valid session directory.",
                session_dir.display()
            ),
            source: None,
        });
    }

    let messages = load_transcript(&transcript_path)?;
    let parent_metadata = if metadata_path.exists() {
        load_metadata(&metadata_path)?
    } else {
        json!({})
    };
    let parent_id = parent_metadata
        .get("session_id")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| {
            session_dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
        })
        .to_string();

    let max_turns = count_turns(&messages);
    if max_turns == 0 {
        return Err(crate::error::BundleError::LoadError {
            reason: "Cannot fork: session has no user messages".to_string(),
            source: None,
        });
    }

    let turn = turn.unwrap_or(max_turns);

    if turn < 1 || turn > max_turns {
        return Err(crate::error::BundleError::LoadError {
            reason: format!("Turn {turn} out of range. Valid range: 1-{max_turns}"),
            source: None,
        });
    }

    let sliced = slice_to_turn(&messages, turn, Some("complete"))?;

    let session_id = new_session_id
        .map(|s| s.to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    let base_dir = match target_dir {
        Some(dir) => dir.to_path_buf(),
        None => session_dir.parent().unwrap_or(&session_dir).to_path_buf(),
    };

    let new_session_dir = base_dir.join(&session_id);
    fs::create_dir_all(&new_session_dir)?;

    // Write transcript
    write_transcript(&new_session_dir.join("transcript.jsonl"), &sliced)?;

    // Write metadata
    let now = Utc::now().to_rfc3339();
    let new_metadata = json!({
        "session_id": session_id,
        "parent_id": parent_id,
        "forked_from_turn": turn,
        "forked_at": now,
        "created": now,
        "turn_count": count_turns(&sliced),
        "bundle": parent_metadata.get("bundle"),
        "model": parent_metadata.get("model"),
    });
    write_metadata(&new_session_dir.join("metadata.json"), &new_metadata)?;

    // Handle events
    let mut events_count = 0;
    if include_events && events_path.exists() {
        match slice_events_for_fork(
            &events_path,
            &transcript_path,
            turn,
            &new_session_dir.join("events.jsonl"),
        ) {
            Ok(count) => events_count = count,
            Err(_) => {
                // On error, create empty events file
                fs::write(new_session_dir.join("events.jsonl"), "")?;
            }
        }
    } else if include_events {
        fs::write(new_session_dir.join("events.jsonl"), "")?;
    }

    Ok(ForkResult {
        session_id,
        session_dir: Some(new_session_dir),
        parent_id,
        forked_from_turn: turn,
        message_count: sliced.len(),
        messages: None,
        events_count,
    })
}

/// Fork messages in memory without file I/O.
pub fn fork_session_in_memory(
    messages: &[Value],
    turn: Option<usize>,
    parent_id: Option<&str>,
) -> crate::error::Result<ForkResult> {
    let max_turns = count_turns(messages);

    let turn = turn.unwrap_or(if max_turns > 0 { max_turns } else { 0 });

    if max_turns == 0 {
        return Ok(ForkResult {
            session_id: Uuid::new_v4().to_string(),
            session_dir: None,
            parent_id: parent_id.unwrap_or("unknown").to_string(),
            forked_from_turn: 0,
            message_count: 0,
            messages: Some(vec![]),
            events_count: 0,
        });
    }

    let sliced = slice_to_turn(messages, turn, Some("complete"))?;

    Ok(ForkResult {
        session_id: Uuid::new_v4().to_string(),
        session_dir: None,
        parent_id: parent_id.unwrap_or("unknown").to_string(),
        forked_from_turn: turn,
        message_count: sliced.len(),
        messages: Some(sliced),
        events_count: 0,
    })
}

/// Get a preview of what a fork would produce without actually forking.
pub fn get_fork_preview(session_dir: &Path, turn: usize) -> crate::error::Result<Value> {
    let session_dir = fs::canonicalize(session_dir).map_err(crate::error::BundleError::Io)?;

    let transcript_path = session_dir.join("transcript.jsonl");
    let metadata_path = session_dir.join("metadata.json");

    if !transcript_path.exists() {
        return Err(crate::error::BundleError::LoadError {
            reason: format!("No transcript.jsonl in {}", session_dir.display()),
            source: None,
        });
    }

    let messages = load_transcript(&transcript_path)?;
    let parent_metadata = if metadata_path.exists() {
        load_metadata(&metadata_path)?
    } else {
        json!({})
    };
    let parent_id = parent_metadata
        .get("session_id")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| {
            session_dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
        })
        .to_string();

    let max_turns = count_turns(&messages);
    if turn < 1 || turn > max_turns {
        return Err(crate::error::BundleError::LoadError {
            reason: format!("Turn {turn} out of range (1-{max_turns})"),
            source: None,
        });
    }

    // Get sliced messages WITHOUT handling orphaned tools (to detect them)
    let boundaries = get_turn_boundaries(&messages);
    let end_idx = if turn < max_turns {
        boundaries[turn]
    } else {
        messages.len()
    };
    let sliced = &messages[..end_idx];

    let orphaned = find_orphaned_tool_calls(sliced);

    // Extract last user and assistant content for preview
    let mut last_user = String::new();
    let mut last_assistant = String::new();
    for msg in sliced.iter().rev() {
        let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("");
        let content = msg.get("content");

        if role == "user" && last_user.is_empty() {
            last_user = extract_text_content(content).chars().take(100).collect();
        } else if role == "assistant" && last_assistant.is_empty() {
            last_assistant = extract_text_content(content).chars().take(100).collect();
        }

        if !last_user.is_empty() && !last_assistant.is_empty() {
            break;
        }
    }

    Ok(json!({
        "parent_id": parent_id,
        "turn": turn,
        "max_turns": max_turns,
        "message_count": sliced.len(),
        "has_orphaned_tools": !orphaned.is_empty(),
        "orphaned_tool_count": orphaned.len(),
        "last_user_message": last_user,
        "last_assistant_message": last_assistant,
    }))
}

/// List all sessions forked from a given session.
pub fn list_session_forks(session_dir: &Path) -> crate::error::Result<Vec<Value>> {
    let session_dir = fs::canonicalize(session_dir).map_err(crate::error::BundleError::Io)?;

    let sessions_root = session_dir.parent().unwrap_or(&session_dir).to_path_buf();

    let metadata_path = session_dir.join("metadata.json");
    let parent_id = if metadata_path.exists() {
        let meta = load_metadata(&metadata_path)?;
        meta.get("session_id")
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| {
                session_dir
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
            })
            .to_string()
    } else {
        session_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string()
    };

    let mut forks: Vec<Value> = Vec::new();

    let entries = match fs::read_dir(&sessions_root) {
        Ok(entries) => entries,
        Err(_) => return Ok(vec![]),
    };

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let child_dir = entry.path();
        if !child_dir.is_dir() {
            continue;
        }

        let child_metadata_path = child_dir.join("metadata.json");
        if !child_metadata_path.exists() {
            continue;
        }

        match load_metadata(&child_metadata_path) {
            Ok(child_metadata) => {
                if child_metadata.get("parent_id").and_then(|v| v.as_str()) == Some(&parent_id) {
                    forks.push(json!({
                        "session_id": child_metadata.get("session_id")
                            .and_then(|v| v.as_str())
                            .unwrap_or_else(|| child_dir.file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("unknown")),
                        "session_dir": child_dir.to_string_lossy(),
                        "forked_from_turn": child_metadata.get("forked_from_turn"),
                        "forked_at": child_metadata.get("forked_at"),
                        "turn_count": child_metadata.get("turn_count").and_then(|v| v.as_u64()).unwrap_or(0),
                    }));
                }
            }
            Err(_) => continue,
        }
    }

    // Sort by forked_at descending (newest first)
    forks.sort_by(|a, b| {
        let a_ts = a.get("forked_at").and_then(|v| v.as_str()).unwrap_or("");
        let b_ts = b.get("forked_at").and_then(|v| v.as_str()).unwrap_or("");
        b_ts.cmp(a_ts)
    });

    Ok(forks)
}

/// Get the full lineage tree for a session.
pub fn get_session_lineage(session_dir: &Path) -> crate::error::Result<Value> {
    let session_dir = fs::canonicalize(session_dir).map_err(crate::error::BundleError::Io)?;

    let sessions_root = session_dir.parent().unwrap_or(&session_dir).to_path_buf();

    let metadata_path = session_dir.join("metadata.json");
    let metadata = if metadata_path.exists() {
        load_metadata(&metadata_path)?
    } else {
        json!({"session_id": session_dir.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")})
    };

    let session_id = metadata
        .get("session_id")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| {
            session_dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
        })
        .to_string();

    let parent_id = metadata
        .get("parent_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let forked_from_turn = metadata.get("forked_from_turn").cloned();

    // Walk ancestor chain iteratively (with cycle detection)
    let mut ancestors: Vec<Value> = Vec::new();
    let mut visited: HashSet<String> = HashSet::new();
    let mut current_parent_id = parent_id.clone();
    while let Some(ref pid) = current_parent_id {
        // Cycle detection: stop if we've seen this ID before
        if !visited.insert(pid.clone()) {
            break;
        }

        ancestors.push(json!({
            "session_id": pid,
        }));

        let parent_dir = sessions_root.join(pid);
        if parent_dir.exists() {
            let parent_meta_path = parent_dir.join("metadata.json");
            if parent_meta_path.exists() {
                match load_metadata(&parent_meta_path) {
                    Ok(parent_meta) => {
                        current_parent_id = parent_meta
                            .get("parent_id")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                    }
                    Err(_) => break,
                }
            } else {
                break;
            }
        } else {
            break;
        }
    }

    let children = list_session_forks(&session_dir)?;
    let depth = ancestors.len();

    Ok(json!({
        "session_id": session_id,
        "parent_id": parent_id,
        "forked_from_turn": forked_from_turn,
        "ancestors": ancestors,
        "children": children,
        "depth": depth,
    }))
}

// --- Private helpers ---

/// Load a JSONL transcript file into a Vec of messages.
fn load_transcript(path: &Path) -> crate::error::Result<Vec<Value>> {
    let file = fs::File::open(path)?;
    let reader = BufReader::new(file);
    let mut messages = Vec::new();

    for line in reader.lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match serde_json::from_str(trimmed) {
            Ok(value) => messages.push(value),
            Err(_) => continue,
        }
    }

    Ok(messages)
}

/// Write messages as a JSONL transcript file.
fn write_transcript(path: &Path, messages: &[Value]) -> crate::error::Result<()> {
    let mut file = fs::File::create(path)?;
    for msg in messages {
        writeln!(file, "{}", serde_json::to_string(msg).unwrap_or_default())?;
    }
    Ok(())
}

/// Load a JSON metadata file.
fn load_metadata(path: &Path) -> crate::error::Result<Value> {
    let content = fs::read_to_string(path)?;
    serde_json::from_str(&content).map_err(|e| crate::error::BundleError::LoadError {
        reason: format!("Invalid metadata JSON: {e}"),
        source: None,
    })
}

/// Write a JSON metadata file (pretty-printed).
fn write_metadata(path: &Path, metadata: &Value) -> crate::error::Result<()> {
    let content = serde_json::to_string_pretty(metadata).unwrap_or_default();
    fs::write(path, content)?;
    Ok(())
}

/// Extract text content from a message's content field.
fn extract_text_content(content: Option<&Value>) -> String {
    match content {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(blocks)) => {
            for block in blocks {
                if block.get("type").and_then(|t| t.as_str()) == Some("text") {
                    if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                        return text.to_string();
                    }
                }
            }
            format!("{blocks:?}")
        }
        Some(Value::Null) | None => String::new(),
        Some(other) => format!("{other}"),
    }
}
