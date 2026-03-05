//! Tests for session module.
//!
//! Ported from Python test_session.py — 53 tests total.
//! All tests are Wave 2 (ignored until implementations land).

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use tempfile::tempdir;

use amplifier_foundation::session::{
    add_synthetic_tool_results, count_events, count_turns, find_orphaned_tool_calls, fork_session,
    fork_session_in_memory, get_event_summary, get_fork_preview, get_last_timestamp_for_turn,
    get_session_lineage, get_turn_boundaries, get_turn_summary, list_session_forks,
    slice_events_for_fork, slice_events_to_timestamp, slice_to_turn,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// 6 messages: user/assistant x3 (3 turns).
fn simple_messages() -> Vec<Value> {
    vec![
        json!({"role": "user", "content": "Turn 1"}),
        json!({"role": "assistant", "content": "Response 1"}),
        json!({"role": "user", "content": "Turn 2"}),
        json!({"role": "assistant", "content": "Response 2"}),
        json!({"role": "user", "content": "Turn 3"}),
        json!({"role": "assistant", "content": "Response 3"}),
    ]
}

/// User, assistant with 2 tool_calls (call_1, call_2), 2 tool results,
/// assistant summary, user followup, assistant response. (2 user turns)
fn messages_with_tools() -> Vec<Value> {
    vec![
        json!({"role": "user", "content": "Use tools to help"}),
        json!({
            "role": "assistant",
            "content": null,
            "tool_calls": [
                {
                    "id": "call_1",
                    "type": "function",
                    "function": {"name": "read_file", "arguments": "{\"path\": \"a.txt\"}"}
                },
                {
                    "id": "call_2",
                    "type": "function",
                    "function": {"name": "read_file", "arguments": "{\"path\": \"b.txt\"}"}
                }
            ]
        }),
        json!({"role": "tool", "tool_call_id": "call_1", "content": "contents of a.txt"}),
        json!({"role": "tool", "tool_call_id": "call_2", "content": "contents of b.txt"}),
        json!({"role": "assistant", "content": "Here is a summary of both files."}),
        json!({"role": "user", "content": "Thanks, now do something else"}),
        json!({"role": "assistant", "content": "Sure, here you go."}),
    ]
}

/// User, assistant with tool_call (call_1), tool result, assistant, user,
/// assistant with orphaned tool_call (call_orphan — no result).
fn messages_with_orphaned_tool() -> Vec<Value> {
    vec![
        json!({"role": "user", "content": "Do something"}),
        json!({
            "role": "assistant",
            "content": null,
            "tool_calls": [
                {
                    "id": "call_1",
                    "type": "function",
                    "function": {"name": "read_file", "arguments": "{}"}
                }
            ]
        }),
        json!({"role": "tool", "tool_call_id": "call_1", "content": "file contents"}),
        json!({"role": "assistant", "content": "Done with the file."}),
        json!({"role": "user", "content": "Now do another thing"}),
        json!({
            "role": "assistant",
            "content": null,
            "tool_calls": [
                {
                    "id": "call_orphan",
                    "type": "function",
                    "function": {"name": "write_file", "arguments": "{}"}
                }
            ]
        }),
    ]
}

/// Creates a sample session directory with transcript.jsonl (6 messages,
/// 3 turns with timestamps), metadata.json, and events.jsonl (13 events).
/// Returns the session directory path.
fn create_sample_session(base: &Path) -> PathBuf {
    let session_dir = base.join("sessions").join("parent_session_123");
    fs::create_dir_all(&session_dir).expect("failed to create session dir");

    // transcript.jsonl — 6 messages (3 turns) with timestamps
    let transcript_lines = [
        json!({"role": "user", "content": "Turn 1", "timestamp": "2026-01-05T10:00:00Z"}),
        json!({"role": "assistant", "content": "Response 1", "timestamp": "2026-01-05T10:00:30Z"}),
        json!({"role": "user", "content": "Turn 2", "timestamp": "2026-01-05T10:01:00Z"}),
        json!({"role": "assistant", "content": "Response 2", "timestamp": "2026-01-05T10:01:10Z"}),
        json!({"role": "user", "content": "Turn 3", "timestamp": "2026-01-05T10:02:00Z"}),
        json!({"role": "assistant", "content": "Response 3", "timestamp": "2026-01-05T10:02:30Z"}),
    ];
    let transcript_content: String = transcript_lines
        .iter()
        .map(|v| serde_json::to_string(v).unwrap())
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    fs::write(session_dir.join("transcript.jsonl"), &transcript_content)
        .expect("failed to write transcript");

    // metadata.json
    let metadata = json!({
        "session_id": "parent_session_123",
        "bundle": "foundation",
        "model": "claude-sonnet-4-5",
        "created": "2026-01-05T10:00:00Z",
        "turn_count": 3
    });
    fs::write(
        session_dir.join("metadata.json"),
        serde_json::to_string_pretty(&metadata).unwrap(),
    )
    .expect("failed to write metadata");

    // events.jsonl — 13 events with various types and timestamps
    let events = [
        json!({"event_type": "session_start", "timestamp": "2026-01-05T10:00:00Z", "data": {}}),
        json!({"event_type": "user_message", "timestamp": "2026-01-05T10:00:01Z", "data": {"content": "Turn 1"}}),
        json!({"event_type": "model_request", "timestamp": "2026-01-05T10:00:02Z", "data": {}}),
        json!({"event_type": "model_response", "timestamp": "2026-01-05T10:00:29Z", "data": {}}),
        json!({"event_type": "assistant_message", "timestamp": "2026-01-05T10:00:30Z", "data": {"content": "Response 1"}}),
        json!({"event_type": "user_message", "timestamp": "2026-01-05T10:01:00Z", "data": {"content": "Turn 2"}}),
        json!({"event_type": "model_request", "timestamp": "2026-01-05T10:01:01Z", "data": {}}),
        json!({"event_type": "model_response", "timestamp": "2026-01-05T10:01:09Z", "data": {}}),
        json!({"event_type": "assistant_message", "timestamp": "2026-01-05T10:01:10Z", "data": {"content": "Response 2"}}),
        json!({"event_type": "user_message", "timestamp": "2026-01-05T10:02:00Z", "data": {"content": "Turn 3"}}),
        json!({"event_type": "model_request", "timestamp": "2026-01-05T10:02:01Z", "data": {}}),
        json!({"event_type": "model_response", "timestamp": "2026-01-05T10:02:29Z", "data": {}}),
        json!({"event_type": "assistant_message", "timestamp": "2026-01-05T10:02:30Z", "data": {"content": "Response 3"}}),
    ];
    let events_content: String = events
        .iter()
        .map(|v| serde_json::to_string(v).unwrap())
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    fs::write(session_dir.join("events.jsonl"), &events_content).expect("failed to write events");

    session_dir
}

/// Read a JSONL file and return each line parsed as a serde_json::Value.
fn read_jsonl(path: &Path) -> Vec<Value> {
    let content = fs::read_to_string(path).expect("failed to read JSONL file");
    content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("invalid JSON line"))
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════════
// TestGetTurnBoundaries
// ═══════════════════════════════════════════════════════════════════════════

#[test]

fn test_get_turn_boundaries_empty_messages() {
    let boundaries = get_turn_boundaries(&[]);
    assert!(boundaries.is_empty());
}

#[test]

fn test_get_turn_boundaries_single_turn() {
    let msgs = vec![
        json!({"role": "user", "content": "Hello"}),
        json!({"role": "assistant", "content": "Hi"}),
    ];
    let boundaries = get_turn_boundaries(&msgs);
    assert_eq!(boundaries, vec![0]);
}

#[test]

fn test_get_turn_boundaries_multiple_turns() {
    let msgs = simple_messages();
    let boundaries = get_turn_boundaries(&msgs);
    assert_eq!(boundaries, vec![0, 2, 4]);
}

#[test]

fn test_get_turn_boundaries_only_user_messages() {
    let msgs = vec![
        json!({"role": "user", "content": "First"}),
        json!({"role": "user", "content": "Second"}),
        json!({"role": "user", "content": "Third"}),
    ];
    let boundaries = get_turn_boundaries(&msgs);
    assert_eq!(boundaries, vec![0, 1, 2]);
}

// ═══════════════════════════════════════════════════════════════════════════
// TestCountTurns
// ═══════════════════════════════════════════════════════════════════════════

#[test]

fn test_count_turns_empty() {
    assert_eq!(count_turns(&[]), 0);
}

#[test]

fn test_count_turns_simple() {
    let msgs = simple_messages();
    assert_eq!(count_turns(&msgs), 3);
}

#[test]

fn test_count_turns_with_tools() {
    let msgs = messages_with_tools();
    assert_eq!(count_turns(&msgs), 2);
}

// ═══════════════════════════════════════════════════════════════════════════
// TestSliceToTurn
// ═══════════════════════════════════════════════════════════════════════════

#[test]

fn test_slice_to_turn_1() {
    let msgs = simple_messages();
    let sliced = slice_to_turn(&msgs, 1, None).expect("slice_to_turn failed");
    assert_eq!(sliced.len(), 2);
    assert_eq!(sliced[0]["content"], "Turn 1");
    assert_eq!(sliced[1]["content"], "Response 1");
}

#[test]

fn test_slice_to_turn_2() {
    let msgs = simple_messages();
    let sliced = slice_to_turn(&msgs, 2, None).expect("slice_to_turn failed");
    assert_eq!(sliced.len(), 4);
}

#[test]

fn test_slice_to_last_turn() {
    let msgs = simple_messages();
    let sliced = slice_to_turn(&msgs, 3, None).expect("slice_to_turn failed");
    assert_eq!(sliced.len(), 6);
    assert_eq!(sliced, msgs);
}

#[test]

fn test_slice_to_turn_invalid_turn_zero() {
    let msgs = simple_messages();
    let result = slice_to_turn(&msgs, 0, None);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Turn must be >= 1"),
        "expected 'Turn must be >= 1' in error, got: {err_msg}"
    );
}

#[test]

fn test_slice_to_turn_invalid_turn_exceeds_max() {
    let msgs = simple_messages();
    let result = slice_to_turn(&msgs, 10, None);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("exceeds max turns"),
        "expected 'exceeds max turns' in error, got: {err_msg}"
    );
}

#[test]

fn test_slice_to_turn_with_tool_messages() {
    let msgs = messages_with_tools();
    let sliced = slice_to_turn(&msgs, 1, None).expect("slice_to_turn failed");
    assert_eq!(sliced.len(), 5);
    assert_eq!(sliced[0]["role"], "user");
    assert_eq!(sliced.last().unwrap()["role"], "assistant");
}

#[test]

fn test_slice_to_turn_empty_messages() {
    let result = slice_to_turn(&[], 1, None);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("No user messages"),
        "expected 'No user messages' in error, got: {err_msg}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// TestFindOrphanedToolCalls
// ═══════════════════════════════════════════════════════════════════════════

#[test]

fn test_find_orphaned_tool_calls_no_tool_calls() {
    let msgs = simple_messages();
    let orphaned = find_orphaned_tool_calls(&msgs);
    assert!(orphaned.is_empty());
}

#[test]

fn test_find_orphaned_tool_calls_paired_tool_calls() {
    let msgs = messages_with_tools();
    let orphaned = find_orphaned_tool_calls(&msgs);
    assert!(orphaned.is_empty());
}

#[test]

fn test_find_orphaned_tool_calls_orphaned_tool_call() {
    let msgs = messages_with_orphaned_tool();
    let orphaned = find_orphaned_tool_calls(&msgs);
    assert_eq!(orphaned, vec!["call_orphan"]);
}

#[test]

fn test_find_orphaned_tool_calls_multiple_orphaned() {
    let msgs = vec![
        json!({"role": "user", "content": "Do stuff"}),
        json!({
            "role": "assistant",
            "content": null,
            "tool_calls": [
                {"id": "a", "type": "function", "function": {"name": "tool_a", "arguments": "{}"}},
                {"id": "b", "type": "function", "function": {"name": "tool_b", "arguments": "{}"}},
                {"id": "c", "type": "function", "function": {"name": "tool_c", "arguments": "{}"}}
            ]
        }),
        json!({"role": "tool", "tool_call_id": "a", "content": "result a"}),
    ];
    let orphaned: HashSet<String> = find_orphaned_tool_calls(&msgs).into_iter().collect();
    let expected: HashSet<String> = ["b", "c"].iter().map(|s| s.to_string()).collect();
    assert_eq!(orphaned, expected);
}

// ═══════════════════════════════════════════════════════════════════════════
// TestAddSyntheticToolResults
// ═══════════════════════════════════════════════════════════════════════════

#[test]

fn test_add_synthetic_tool_results_no_orphans() {
    let msgs = simple_messages();
    let result = add_synthetic_tool_results(&msgs, &[]);
    assert_eq!(result, msgs);
}

#[test]

fn test_add_synthetic_tool_results_adds_synthetic_result() {
    let msgs = vec![json!({
        "role": "assistant",
        "content": null,
        "tool_calls": [
            {"id": "call_123", "type": "function", "function": {"name": "some_tool", "arguments": "{}"}}
        ]
    })];
    let orphaned = vec!["call_123".to_string()];
    let result = add_synthetic_tool_results(&msgs, &orphaned);
    assert_eq!(result.len(), 2);

    let synthetic = &result[1];
    assert_eq!(synthetic["role"], "tool");
    assert_eq!(synthetic["tool_call_id"], "call_123");
    let content = synthetic["content"].as_str().unwrap_or("");
    assert!(
        content.contains("forked"),
        "expected 'forked' in synthetic content, got: {content}"
    );
}

#[test]

fn test_add_synthetic_tool_results_multiple_orphans() {
    let msgs = vec![json!({
        "role": "assistant",
        "content": null,
        "tool_calls": [
            {"id": "a", "type": "function", "function": {"name": "t1", "arguments": "{}"}},
            {"id": "b", "type": "function", "function": {"name": "t2", "arguments": "{}"}},
            {"id": "c", "type": "function", "function": {"name": "t3", "arguments": "{}"}}
        ]
    })];
    let orphaned: Vec<String> = vec!["a".into(), "b".into(), "c".into()];
    let result = add_synthetic_tool_results(&msgs, &orphaned);
    // Original message + 3 synthetic tool results
    assert_eq!(result.len(), 4);
}

// ═══════════════════════════════════════════════════════════════════════════
// TestSliceToTurnWithOrphanedTools
// ═══════════════════════════════════════════════════════════════════════════

#[test]

fn test_slice_to_turn_complete_orphaned_tools() {
    let msgs = messages_with_orphaned_tool();
    let result = slice_to_turn(&msgs, 2, Some("complete")).expect("slice_to_turn failed");

    // Should include all 6 original messages + 1 synthetic tool result
    assert!(
        result.len() > msgs.len(),
        "expected synthetic result to be added, got len {}",
        result.len()
    );

    // Find the synthetic tool result for "call_orphan"
    let synthetic = result.iter().find(|m| {
        m.get("tool_call_id")
            .and_then(|v| v.as_str())
            .map(|s| s == "call_orphan")
            .unwrap_or(false)
    });
    assert!(
        synthetic.is_some(),
        "expected synthetic result for call_orphan"
    );
    let content = synthetic.unwrap()["content"].as_str().unwrap_or("");
    assert!(
        content.contains("forked"),
        "expected 'forked' in synthetic content, got: {content}"
    );
}

#[test]

fn test_slice_to_turn_error_on_orphaned_tools() {
    let msgs = messages_with_orphaned_tool();
    let result = slice_to_turn(&msgs, 2, Some("error"));
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Orphaned tool calls"),
        "expected 'Orphaned tool calls' in error, got: {err_msg}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// TestGetTurnSummary
// ═══════════════════════════════════════════════════════════════════════════

#[test]

fn test_get_turn_summary_basic() {
    let msgs = simple_messages();
    let summary = get_turn_summary(&msgs, 1).expect("get_turn_summary failed");

    assert_eq!(summary["turn"], 1);
    assert!(summary["user_content"].as_str().is_some());
    assert!(summary["assistant_content"].as_str().is_some());
    assert_eq!(summary["message_count"], 2);
    assert_eq!(summary["tool_count"], 0);
}

#[test]

fn test_get_turn_summary_with_tools() {
    let msgs = messages_with_tools();
    let summary = get_turn_summary(&msgs, 1).expect("get_turn_summary failed");

    assert_eq!(summary["tool_count"], 2);
}

#[test]

fn test_get_turn_summary_invalid_turn() {
    let msgs = simple_messages();
    let result = get_turn_summary(&msgs, 10);
    assert!(result.is_err());
}

// ═══════════════════════════════════════════════════════════════════════════
// TestForkSession
// ═══════════════════════════════════════════════════════════════════════════

#[test]

fn test_fork_session_at_turn_1() {
    let tmp = tempdir().expect("failed to create temp dir");
    let session_dir = create_sample_session(tmp.path());

    let result =
        fork_session(&session_dir, Some(1), None, None, false).expect("fork_session failed");

    assert_eq!(result.forked_from_turn, 1);
    assert_eq!(result.parent_id, "parent_session_123");
    assert_eq!(result.message_count, 2);

    let fork_dir = result.session_dir.as_ref().expect("expected session_dir");
    assert!(fork_dir.exists(), "forked session dir should exist");

    // Verify transcript contents
    let transcript = fs::read_to_string(fork_dir.join("transcript.jsonl"))
        .expect("failed to read forked transcript");
    assert!(transcript.contains("Turn 1"));
    assert!(!transcript.contains("Turn 2"));
}

#[test]

fn test_fork_session_at_turn_2() {
    let tmp = tempdir().expect("failed to create temp dir");
    let session_dir = create_sample_session(tmp.path());

    let result =
        fork_session(&session_dir, Some(2), None, None, false).expect("fork_session failed");

    assert_eq!(result.forked_from_turn, 2);
    assert_eq!(result.message_count, 4);
}

#[test]

fn test_fork_session_default_is_latest() {
    let tmp = tempdir().expect("failed to create temp dir");
    let session_dir = create_sample_session(tmp.path());

    let result = fork_session(&session_dir, None, None, None, false).expect("fork_session failed");

    assert_eq!(result.forked_from_turn, 3);
    assert_eq!(result.message_count, 6);
}

#[test]

fn test_fork_session_preserves_lineage() {
    let tmp = tempdir().expect("failed to create temp dir");
    let session_dir = create_sample_session(tmp.path());

    let result =
        fork_session(&session_dir, Some(2), None, None, false).expect("fork_session failed");

    let fork_dir = result.session_dir.as_ref().expect("expected session_dir");
    let metadata_str =
        fs::read_to_string(fork_dir.join("metadata.json")).expect("failed to read forked metadata");
    let metadata: Value = serde_json::from_str(&metadata_str).expect("invalid metadata JSON");

    assert_eq!(metadata["parent_id"], "parent_session_123");
    assert_eq!(metadata["forked_from_turn"], 2);
    assert!(
        metadata.get("forked_at").is_some(),
        "metadata should contain forked_at timestamp"
    );
}

#[test]

fn test_fork_session_preserves_parent_metadata() {
    let tmp = tempdir().expect("failed to create temp dir");
    let session_dir = create_sample_session(tmp.path());

    let result =
        fork_session(&session_dir, Some(1), None, None, false).expect("fork_session failed");

    let fork_dir = result.session_dir.as_ref().expect("expected session_dir");
    let metadata_str =
        fs::read_to_string(fork_dir.join("metadata.json")).expect("failed to read forked metadata");
    let metadata: Value = serde_json::from_str(&metadata_str).expect("invalid metadata JSON");

    assert_eq!(metadata["bundle"], "foundation");
    assert_eq!(metadata["model"], "claude-sonnet-4-5");
}

#[test]

fn test_fork_session_with_custom_id() {
    let tmp = tempdir().expect("failed to create temp dir");
    let session_dir = create_sample_session(tmp.path());

    let result = fork_session(&session_dir, Some(1), Some("my-custom-fork"), None, false)
        .expect("fork_session failed");

    assert_eq!(result.session_id, "my-custom-fork");
}

#[test]

fn test_fork_session_with_custom_target_dir() {
    let tmp = tempdir().expect("failed to create temp dir");
    let session_dir = create_sample_session(tmp.path());
    let custom_target = tmp.path().join("custom_forks");
    fs::create_dir_all(&custom_target).expect("failed to create custom target dir");

    let result = fork_session(&session_dir, Some(1), None, Some(&custom_target), false)
        .expect("fork_session failed");

    let fork_dir = result.session_dir.as_ref().expect("expected session_dir");
    assert!(
        fork_dir.starts_with(&custom_target),
        "forked session should be under custom target dir"
    );
}

#[test]

fn test_fork_session_includes_events() {
    let tmp = tempdir().expect("failed to create temp dir");
    let session_dir = create_sample_session(tmp.path());

    let result =
        fork_session(&session_dir, Some(2), None, None, true).expect("fork_session failed");

    let fork_dir = result.session_dir.as_ref().expect("expected session_dir");
    let events_path = fork_dir.join("events.jsonl");
    assert!(events_path.exists(), "events.jsonl should exist in fork");
    assert!(
        result.events_count > 0,
        "events_count should be > 0 when include_events is true"
    );
}

#[test]

fn test_fork_session_nonexistent_session() {
    let tmp = tempdir().expect("failed to create temp dir");
    let nonexistent = tmp.path().join("does_not_exist");

    let result = fork_session(&nonexistent, Some(1), None, None, false);
    assert!(result.is_err(), "forking nonexistent session should fail");
}

#[test]

fn test_fork_session_invalid_turn() {
    let tmp = tempdir().expect("failed to create temp dir");
    let session_dir = create_sample_session(tmp.path());

    let result = fork_session(&session_dir, Some(10), None, None, false);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("out of range"),
        "expected 'out of range' in error, got: {err_msg}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// TestForkSessionInMemory
// ═══════════════════════════════════════════════════════════════════════════

#[test]

fn test_fork_session_in_memory_basic() {
    let msgs = simple_messages();
    let result = fork_session_in_memory(&msgs, Some(1), Some("parent_1"))
        .expect("fork_session_in_memory failed");

    assert_eq!(result.forked_from_turn, 1);
    assert_eq!(result.parent_id, "parent_1");
    assert_eq!(result.message_count, 2);
    assert!(
        result.session_dir.is_none(),
        "in-memory fork should have no session_dir"
    );
    assert!(
        result.messages.is_some(),
        "in-memory fork should include messages"
    );
    assert_eq!(result.messages.as_ref().unwrap().len(), 2);
}

#[test]

fn test_fork_session_in_memory_all_turns() {
    let msgs = simple_messages();
    let result = fork_session_in_memory(&msgs, None, Some("parent_1"))
        .expect("fork_session_in_memory failed");

    assert_eq!(result.forked_from_turn, 3);
    assert_eq!(result.message_count, 6);
}

#[test]

fn test_fork_session_in_memory_handles_orphaned_tools() {
    let msgs = messages_with_orphaned_tool();
    let result = fork_session_in_memory(&msgs, Some(2), Some("parent_1"))
        .expect("fork_session_in_memory failed");

    let fork_msgs = result.messages.as_ref().expect("expected messages");

    // Should have a synthetic tool result for "call_orphan"
    let synthetic = fork_msgs.iter().find(|m| {
        m.get("tool_call_id")
            .and_then(|v| v.as_str())
            .map(|s| s == "call_orphan")
            .unwrap_or(false)
    });
    assert!(
        synthetic.is_some(),
        "expected synthetic result for call_orphan"
    );
    let content = synthetic.unwrap()["content"].as_str().unwrap_or("");
    assert!(
        content.contains("forked"),
        "expected 'forked' in synthetic content, got: {content}"
    );
}

#[test]

fn test_fork_session_in_memory_empty_messages() {
    let result = fork_session_in_memory(&[], None, None).expect("fork_session_in_memory failed");

    assert_eq!(result.forked_from_turn, 0);
    assert_eq!(result.message_count, 0);
    assert!(
        result.messages.as_ref().map_or(true, |m| m.is_empty()),
        "empty input should produce empty messages"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// TestGetForkPreview
// ═══════════════════════════════════════════════════════════════════════════

#[test]

fn test_get_fork_preview_basic() {
    let tmp = tempdir().expect("failed to create temp dir");
    let session_dir = create_sample_session(tmp.path());

    let preview = get_fork_preview(&session_dir, 2).expect("get_fork_preview failed");

    assert_eq!(preview["parent_id"], "parent_session_123");
    assert_eq!(preview["turn"], 2);
    assert_eq!(preview["max_turns"], 3);
    assert_eq!(preview["message_count"], 4);
    assert!(
        preview.get("has_orphaned_tools").is_some(),
        "preview should include has_orphaned_tools field"
    );
}

#[test]

fn test_get_fork_preview_invalid_turn() {
    let tmp = tempdir().expect("failed to create temp dir");
    let session_dir = create_sample_session(tmp.path());

    let result = get_fork_preview(&session_dir, 10);
    assert!(result.is_err());
}

// ═══════════════════════════════════════════════════════════════════════════
// TestListSessionForks
// ═══════════════════════════════════════════════════════════════════════════

#[test]

fn test_list_session_forks_no_forks() {
    let tmp = tempdir().expect("failed to create temp dir");
    let session_dir = create_sample_session(tmp.path());

    let forks = list_session_forks(&session_dir).expect("list_session_forks failed");
    assert!(
        forks.is_empty(),
        "session with no forks should return empty list"
    );
}

#[test]

fn test_list_session_forks_finds_forks() {
    let tmp = tempdir().expect("failed to create temp dir");
    let session_dir = create_sample_session(tmp.path());

    // Create a fork
    let fork_result =
        fork_session(&session_dir, Some(1), None, None, false).expect("fork_session failed");

    let forks = list_session_forks(&session_dir).expect("list_session_forks failed");
    assert_eq!(forks.len(), 1, "should find exactly one fork");

    let fork_session_id = forks[0]["session_id"]
        .as_str()
        .expect("fork should have session_id");
    assert_eq!(fork_session_id, fork_result.session_id);
}

// ═══════════════════════════════════════════════════════════════════════════
// TestGetSessionLineage
// ═══════════════════════════════════════════════════════════════════════════

#[test]

fn test_get_session_lineage_root_session() {
    let tmp = tempdir().expect("failed to create temp dir");
    let session_dir = create_sample_session(tmp.path());

    let lineage = get_session_lineage(&session_dir).expect("get_session_lineage failed");

    assert_eq!(lineage["session_id"], "parent_session_123");
    assert!(
        lineage["parent_id"].is_null(),
        "root session should have null parent_id"
    );
    assert!(
        lineage["ancestors"].as_array().unwrap().is_empty(),
        "root session should have empty ancestors"
    );
    assert_eq!(lineage["depth"], 0);
}

#[test]

fn test_get_session_lineage_forked_session() {
    let tmp = tempdir().expect("failed to create temp dir");
    let session_dir = create_sample_session(tmp.path());

    let fork_result =
        fork_session(&session_dir, Some(2), None, None, false).expect("fork_session failed");
    let fork_dir = fork_result
        .session_dir
        .as_ref()
        .expect("expected session_dir");

    let lineage = get_session_lineage(fork_dir).expect("get_session_lineage failed");

    assert_eq!(lineage["parent_id"], "parent_session_123");
    let ancestors = lineage["ancestors"]
        .as_array()
        .expect("ancestors should be array");
    assert!(
        !ancestors.is_empty(),
        "forked session should have non-empty ancestors"
    );
    assert!(
        ancestors
            .iter()
            .any(|a| a["session_id"] == "parent_session_123"),
        "ancestors should contain parent session"
    );
    assert_eq!(lineage["depth"], 1);
}

#[test]

fn test_get_session_lineage_deeply_nested_forks() {
    let tmp = tempdir().expect("failed to create temp dir");
    let session_dir = create_sample_session(tmp.path());

    // First fork
    let fork1 = fork_session(&session_dir, Some(2), None, None, false).expect("first fork failed");
    let fork1_dir = fork1.session_dir.as_ref().expect("expected session_dir");

    // Second fork (fork of fork)
    let fork2 = fork_session(fork1_dir, Some(1), None, None, false).expect("second fork failed");
    let fork2_dir = fork2.session_dir.as_ref().expect("expected session_dir");

    let lineage = get_session_lineage(fork2_dir).expect("get_session_lineage failed");

    assert_eq!(lineage["depth"], 2);
    let ancestors = lineage["ancestors"]
        .as_array()
        .expect("ancestors should be array");
    assert_eq!(ancestors.len(), 2, "depth-2 fork should have 2 ancestors");
}

// ═══════════════════════════════════════════════════════════════════════════
// TestSliceEventsToTimestamp
// ═══════════════════════════════════════════════════════════════════════════

#[test]

fn test_slice_events_to_timestamp_basic() {
    let tmp = tempdir().expect("failed to create temp dir");
    let session_dir = create_sample_session(tmp.path());

    let events_path = session_dir.join("events.jsonl");
    let output_path = tmp.path().join("sliced_events.jsonl");
    let cutoff = "2026-01-05T10:01:00Z";

    let count = slice_events_to_timestamp(&events_path, cutoff, &output_path)
        .expect("slice_events_to_timestamp failed");

    assert!(count > 0, "should have sliced some events");
    assert!(count < 13, "should not include all events");

    // Verify no events after the cutoff timestamp
    let sliced = read_jsonl(&output_path);
    for event in &sliced {
        let ts = event["timestamp"]
            .as_str()
            .expect("event should have timestamp");
        assert!(
            ts <= cutoff,
            "event timestamp {ts} should be <= cutoff {cutoff}"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// TestGetLastTimestampForTurn
// ═══════════════════════════════════════════════════════════════════════════

#[test]

fn test_get_last_timestamp_for_turn() {
    let tmp = tempdir().expect("failed to create temp dir");
    let session_dir = create_sample_session(tmp.path());
    let transcript_path = session_dir.join("transcript.jsonl");

    let ts = get_last_timestamp_for_turn(&transcript_path, 2)
        .expect("get_last_timestamp_for_turn failed");

    assert_eq!(ts, "2026-01-05T10:01:10Z");
}

// ═══════════════════════════════════════════════════════════════════════════
// TestSliceEventsForFork
// ═══════════════════════════════════════════════════════════════════════════

#[test]

fn test_slice_events_for_fork() {
    let tmp = tempdir().expect("failed to create temp dir");
    let session_dir = create_sample_session(tmp.path());

    let events_path = session_dir.join("events.jsonl");
    let transcript_path = session_dir.join("transcript.jsonl");
    let output_path = tmp.path().join("fork_events.jsonl");

    let count = slice_events_for_fork(&events_path, &transcript_path, 2, &output_path)
        .expect("slice_events_for_fork failed");

    assert!(count > 0, "should have sliced some events");
    assert!(output_path.exists(), "output file should exist");
}

// ═══════════════════════════════════════════════════════════════════════════
// TestCountEvents
// ═══════════════════════════════════════════════════════════════════════════

#[test]

fn test_count_events() {
    let tmp = tempdir().expect("failed to create temp dir");
    let session_dir = create_sample_session(tmp.path());
    let events_path = session_dir.join("events.jsonl");

    let count = count_events(&events_path);
    assert_eq!(count, 13);
}

#[test]

fn test_count_events_nonexistent_file() {
    let tmp = tempdir().expect("failed to create temp dir");
    let nonexistent = tmp.path().join("does_not_exist.jsonl");

    let count = count_events(&nonexistent);
    assert_eq!(count, 0);
}

// ═══════════════════════════════════════════════════════════════════════════
// TestGetEventSummary
// ═══════════════════════════════════════════════════════════════════════════

#[test]

fn test_get_event_summary() {
    let tmp = tempdir().expect("failed to create temp dir");
    let session_dir = create_sample_session(tmp.path());
    let events_path = session_dir.join("events.jsonl");

    let summary = get_event_summary(&events_path).expect("get_event_summary failed");

    assert_eq!(summary["total_events"], 13);
    assert!(
        summary.get("event_types").is_some(),
        "summary should include event_types"
    );
    assert!(
        summary.get("timestamps").is_some()
            || summary.get("first_timestamp").is_some()
            || summary.get("time_range").is_some(),
        "summary should include timestamp information"
    );
}
