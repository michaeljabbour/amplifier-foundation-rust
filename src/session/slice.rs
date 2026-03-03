//! Message slicing utilities for session fork operations.
//!
//! This module provides pure functions for slicing conversation messages
//! at turn boundaries. A "turn" is defined as a user message plus all
//! subsequent non-user messages until the next user message.
//!
//! Turns are 1-indexed for user-facing operations (turn 1 = first exchange).

use std::collections::HashSet;

use serde_json::{json, Value};

/// Return indices where each turn starts (user message positions).
///
/// A turn begins with each user message. This returns the 0-indexed
/// positions of all user messages in the conversation.
pub fn get_turn_boundaries(messages: &[Value]) -> Vec<usize> {
    messages
        .iter()
        .enumerate()
        .filter(|(_, msg)| msg.get("role").and_then(|r| r.as_str()) == Some("user"))
        .map(|(i, _)| i)
        .collect()
}

/// Count the number of turns in a conversation.
pub fn count_turns(messages: &[Value]) -> usize {
    get_turn_boundaries(messages).len()
}

/// Slice messages to include only up to turn N (1-indexed).
///
/// Turn N includes the Nth user message and all responses until the
/// next user message (or end of conversation).
///
/// `handle_orphaned_tools`:
/// - `Some("complete")` or `None`: Add synthetic error result (default)
/// - `Some("remove")`: Remove the orphaned tool_use content
/// - `Some("error")`: Return an error
pub fn slice_to_turn(
    messages: &[Value],
    turn: usize,
    handle_orphaned_tools: Option<&str>,
) -> crate::error::Result<Vec<Value>> {
    if turn < 1 {
        return Err(crate::error::BundleError::LoadError {
            reason: format!("Turn must be >= 1, got {turn}"),
            source: None,
        });
    }

    let boundaries = get_turn_boundaries(messages);
    let max_turns = boundaries.len();

    if max_turns == 0 {
        return Err(crate::error::BundleError::LoadError {
            reason: "No user messages found in conversation".to_string(),
            source: None,
        });
    }

    if turn > max_turns {
        return Err(crate::error::BundleError::LoadError {
            reason: format!(
                "Turn {turn} exceeds max turns ({max_turns}). Valid range: 1-{max_turns}"
            ),
            source: None,
        });
    }

    // Find end index: start of turn N+1, or end of messages
    let end_idx = if turn < max_turns {
        boundaries[turn] // Start of next turn (0-indexed, turn N+1 = boundaries[turn])
    } else {
        messages.len()
    };

    let mut sliced: Vec<Value> = messages[..end_idx].to_vec();

    // Handle orphaned tool calls
    let orphaned = find_orphaned_tool_calls(&sliced);
    if !orphaned.is_empty() {
        let mode = handle_orphaned_tools.unwrap_or("complete");
        match mode {
            "error" => {
                return Err(crate::error::BundleError::LoadError {
                    reason: format!(
                        "Orphaned tool calls at fork boundary: {:?}. \
                         These tool_use blocks have no matching tool_result.",
                        orphaned
                    ),
                    source: None,
                });
            }
            "remove" => {
                sliced = remove_orphaned_tool_calls(&sliced, &orphaned);
            }
            _ => {
                // "complete" is default
                sliced = add_synthetic_tool_results(&sliced, &orphaned);
            }
        }
    }

    Ok(sliced)
}

/// Find tool_call IDs that have no corresponding tool result.
///
/// Scans messages for tool_calls in assistant messages and tool results,
/// returning IDs of calls that don't have matching results.
pub fn find_orphaned_tool_calls(messages: &[Value]) -> Vec<String> {
    let mut called_ids: HashSet<String> = HashSet::new();

    for msg in messages {
        if msg.get("role").and_then(|r| r.as_str()) == Some("assistant") {
            // Check tool_calls array (standard/OpenAI format)
            if let Some(tool_calls) = msg.get("tool_calls").and_then(|tc| tc.as_array()) {
                for tc in tool_calls {
                    if let Some(id) = tc.get("id").and_then(|id| id.as_str()) {
                        called_ids.insert(id.to_string());
                    }
                }
            }
            // Check content blocks (Anthropic format)
            if let Some(content) = msg.get("content").and_then(|c| c.as_array()) {
                for block in content {
                    if block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                        if let Some(id) = block.get("id").and_then(|id| id.as_str()) {
                            called_ids.insert(id.to_string());
                        }
                    }
                }
            }
        }
    }

    // Collect all tool_call_ids from tool results
    let mut result_ids: HashSet<String> = HashSet::new();
    for msg in messages {
        if msg.get("role").and_then(|r| r.as_str()) == Some("tool") {
            if let Some(id) = msg.get("tool_call_id").and_then(|id| id.as_str()) {
                result_ids.insert(id.to_string());
            }
        }
    }

    called_ids.difference(&result_ids).cloned().collect()
}

/// Add synthetic error results for orphaned tool calls.
///
/// When forking a session mid-turn, some tool calls may not have results.
/// This adds synthetic error results so the conversation remains valid.
pub fn add_synthetic_tool_results(messages: &[Value], orphaned_ids: &[String]) -> Vec<Value> {
    if orphaned_ids.is_empty() {
        return messages.to_vec();
    }

    let mut result: Vec<Value> = messages.to_vec();
    for tool_id in orphaned_ids {
        let synthetic_content = json!({
            "error": "Tool execution interrupted by session fork",
            "forked": true,
            "message": "This tool call was in progress when the session was forked. \
                        The result is not available in this forked session."
        });
        result.push(json!({
            "role": "tool",
            "tool_call_id": tool_id,
            "content": serde_json::to_string(&synthetic_content).unwrap_or_default(),
        }));
    }
    result
}

/// Remove orphaned tool_call entries from messages.
fn remove_orphaned_tool_calls(messages: &[Value], orphaned_ids: &[String]) -> Vec<Value> {
    let orphaned_set: HashSet<&str> = orphaned_ids.iter().map(|s| s.as_str()).collect();
    let mut result = Vec::new();

    for msg in messages {
        if msg.get("role").and_then(|r| r.as_str()) == Some("assistant") {
            let mut new_msg = msg.clone();

            // Filter tool_calls array
            if let Some(tool_calls) = new_msg.get("tool_calls").and_then(|tc| tc.as_array()) {
                let filtered: Vec<Value> = tool_calls
                    .iter()
                    .filter(|tc| {
                        tc.get("id")
                            .and_then(|id| id.as_str())
                            .map(|id| !orphaned_set.contains(id))
                            .unwrap_or(true)
                    })
                    .cloned()
                    .collect();
                if filtered.is_empty() {
                    if let Some(obj) = new_msg.as_object_mut() {
                        obj.remove("tool_calls");
                    }
                } else {
                    new_msg["tool_calls"] = Value::Array(filtered);
                }
            }

            // Filter content blocks (Anthropic format)
            if let Some(content) = new_msg.get("content").and_then(|c| c.as_array()) {
                let filtered: Vec<Value> = content
                    .iter()
                    .filter(|block| {
                        if block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                            block
                                .get("id")
                                .and_then(|id| id.as_str())
                                .map(|id| !orphaned_set.contains(id))
                                .unwrap_or(true)
                        } else {
                            true
                        }
                    })
                    .cloned()
                    .collect();
                new_msg["content"] = Value::Array(filtered);
            }

            result.push(new_msg);
        } else {
            result.push(msg.clone());
        }
    }

    result
}

/// Get a summary of a specific turn for display purposes.
///
/// Returns a JSON object with:
/// - `turn`: Turn number
/// - `user_content`: Truncated user message
/// - `assistant_content`: Truncated assistant response
/// - `tool_count`: Number of tool calls in turn
/// - `message_count`: Total messages in turn
pub fn get_turn_summary(
    messages: &[Value],
    turn: usize,
) -> crate::error::Result<Value> {
    let boundaries = get_turn_boundaries(messages);
    let max_turns = boundaries.len();

    if turn < 1 || turn > max_turns {
        return Err(crate::error::BundleError::LoadError {
            reason: format!("Turn {turn} out of range (1-{max_turns})"),
            source: None,
        });
    }

    let start_idx = boundaries[turn - 1];
    let end_idx = if turn < max_turns {
        boundaries[turn]
    } else {
        messages.len()
    };

    let turn_messages = &messages[start_idx..end_idx];
    let max_length = 100;

    let mut user_content = String::new();
    let mut assistant_content = String::new();
    let mut tool_count: usize = 0;

    for msg in turn_messages {
        let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("");
        let content = msg.get("content");

        match role {
            "user" => {
                if let Some(s) = content.and_then(|c| c.as_str()) {
                    user_content = truncate_str(s, max_length);
                } else if let Some(blocks) = content.and_then(|c| c.as_array()) {
                    for block in blocks {
                        if block.get("type").and_then(|t| t.as_str()) == Some("text") {
                            if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                                user_content = truncate_str(text, max_length);
                                break;
                            }
                        }
                    }
                }
            }
            "assistant" => {
                if let Some(s) = content.and_then(|c| c.as_str()) {
                    if assistant_content.is_empty() {
                        assistant_content = truncate_str(s, max_length);
                    }
                } else if let Some(blocks) = content.and_then(|c| c.as_array()) {
                    for block in blocks {
                        if let Some(block_type) = block.get("type").and_then(|t| t.as_str()) {
                            if block_type == "text" && assistant_content.is_empty() {
                                if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                                    assistant_content = truncate_str(text, max_length);
                                }
                            } else if block_type == "tool_use" {
                                tool_count += 1;
                            }
                        }
                    }
                }

                // Count tool_calls (OpenAI format)
                if let Some(tool_calls) = msg.get("tool_calls").and_then(|tc| tc.as_array()) {
                    tool_count += tool_calls.len();
                }
            }
            _ => {}
        }
    }

    Ok(json!({
        "turn": turn,
        "user_content": user_content,
        "assistant_content": assistant_content,
        "tool_count": tool_count,
        "message_count": turn_messages.len(),
    }))
}

/// Truncate a string to max_length characters, appending "..." if truncated.
/// Uses char count (not byte count) to match Python's `s[:max_length]` behavior.
fn truncate_str(s: &str, max_length: usize) -> String {
    let char_count = s.chars().count();
    if char_count > max_length {
        let truncated: String = s.chars().take(max_length).collect();
        format!("{truncated}...")
    } else {
        s.to_string()
    }
}
