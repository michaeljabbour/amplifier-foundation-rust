//! Events.jsonl slicing utilities for session fork operations.
//!
//! This module provides functions for slicing events.jsonl files when forking
//! sessions. Events are correlated to turns via timestamps -- we find the last
//! event timestamp for the target turn and include all events up to that point.
//!
//! Note: events.jsonl is primarily an audit log and is NOT required for session
//! resume. The transcript.jsonl is the source of truth for conversation state.

use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use serde_json::{json, Value};

/// Slice events.jsonl to include only events up to a timestamp.
///
/// Reads events line by line (memory efficient for large files) and writes
/// events with timestamp <= cutoff_timestamp to the output file.
///
/// Returns the number of events written.
pub fn slice_events_to_timestamp(
    events_path: &Path,
    cutoff_timestamp: &str,
    output_path: &Path,
) -> crate::error::Result<usize> {
    if !events_path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Events file not found: {}", events_path.display()),
        )
        .into());
    }

    let file = fs::File::open(events_path)?;
    let reader = BufReader::new(file);
    let mut out = fs::File::create(output_path)?;
    let mut count = 0;

    for line in reader.lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let event: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => continue, // Skip malformed lines
        };

        // Get timestamp from "ts" or "timestamp" field
        let event_ts = event
            .get("ts")
            .or_else(|| event.get("timestamp"))
            .and_then(|v| v.as_str());

        if let Some(ts) = event_ts {
            // Simple string comparison works for ISO 8601 timestamps
            // (lexicographic order matches chronological order for same-format timestamps)
            if ts <= cutoff_timestamp {
                writeln!(out, "{}", serde_json::to_string(&event).unwrap_or_default())?;
                count += 1;
            }
        } else {
            // Events without timestamp are included (shouldn't happen in practice)
            writeln!(out, "{}", serde_json::to_string(&event).unwrap_or_default())?;
            count += 1;
        }
    }

    Ok(count)
}

/// Get the timestamp of the last message in a turn from a transcript JSONL file.
///
/// Reads the transcript to find the last message belonging to the specified
/// turn and returns its timestamp.
pub fn get_last_timestamp_for_turn(
    transcript_path: &Path,
    turn: usize,
) -> crate::error::Result<String> {
    if !transcript_path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Transcript not found: {}", transcript_path.display()),
        )
        .into());
    }

    let messages = read_jsonl(transcript_path)?;

    // Find turn boundaries (user message indices)
    let boundaries: Vec<usize> = messages
        .iter()
        .enumerate()
        .filter(|(_, msg)| msg.get("role").and_then(|r| r.as_str()) == Some("user"))
        .map(|(i, _)| i)
        .collect();

    if boundaries.is_empty() {
        return Err(crate::error::BundleError::LoadError {
            reason: "No user messages found in transcript".to_string(),
            source: None,
        });
    }

    let max_turns = boundaries.len();
    if turn < 1 || turn > max_turns {
        return Err(crate::error::BundleError::LoadError {
            reason: format!("Turn {turn} out of range (1-{max_turns})"),
            source: None,
        });
    }

    // Find end of turn
    let start_idx = boundaries[turn - 1];
    let end_idx = if turn < max_turns {
        boundaries[turn]
    } else {
        messages.len()
    };

    let turn_messages = &messages[start_idx..end_idx];

    // Search backwards for a message with timestamp
    for msg in turn_messages.iter().rev() {
        let ts = msg
            .get("timestamp")
            .or_else(|| msg.get("ts"))
            .and_then(|v| v.as_str());
        if let Some(timestamp) = ts {
            return Ok(timestamp.to_string());
        }
    }

    Err(crate::error::BundleError::LoadError {
        reason: format!("No timestamp found for turn {turn}"),
        source: None,
    })
}

/// Slice events.jsonl for a fork at a specific turn.
///
/// This is a convenience function that:
/// 1. Finds the last timestamp for the target turn
/// 2. Slices events to that timestamp
pub fn slice_events_for_fork(
    events_path: &Path,
    transcript_path: &Path,
    turn: usize,
    output_path: &Path,
) -> crate::error::Result<usize> {
    let cutoff_ts = get_last_timestamp_for_turn(transcript_path, turn)?;

    slice_events_to_timestamp(events_path, &cutoff_ts, output_path)
}

/// Count the number of events in an events.jsonl file.
///
/// Returns 0 if the file doesn't exist.
pub fn count_events(events_path: &Path) -> usize {
    if !events_path.exists() {
        return 0;
    }

    match read_jsonl(events_path) {
        Ok(events) => events.len(),
        Err(_) => 0,
    }
}

/// Get a summary of events in an events.jsonl file.
///
/// Returns a JSON object with:
/// - `total_events`: Total count
/// - `event_types`: Dict of event type -> count
/// - `first_timestamp`: First event timestamp
/// - `last_timestamp`: Last event timestamp
pub fn get_event_summary(events_path: &Path) -> crate::error::Result<Value> {
    if !events_path.exists() {
        return Ok(json!({
            "total_events": 0,
            "event_types": {},
            "first_timestamp": null,
            "last_timestamp": null,
        }));
    }

    let events = read_jsonl(events_path)?;
    let mut event_types: HashMap<String, usize> = HashMap::new();
    let mut first_ts: Option<String> = None;
    let mut last_ts: Option<String> = None;
    let total = events.len();

    for event in &events {
        // Count event types -- Python uses "event" key, but test data uses "event_type"
        let event_type = event
            .get("event")
            .or_else(|| event.get("event_type"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        *event_types.entry(event_type.to_string()).or_insert(0) += 1;

        // Track timestamps
        let ts = event
            .get("ts")
            .or_else(|| event.get("timestamp"))
            .and_then(|v| v.as_str());
        if let Some(timestamp) = ts {
            if first_ts.is_none() {
                first_ts = Some(timestamp.to_string());
            }
            last_ts = Some(timestamp.to_string());
        }
    }

    // Convert event_types HashMap to JSON object
    let event_types_json: Value = event_types
        .into_iter()
        .map(|(k, v)| (k, json!(v)))
        .collect::<serde_json::Map<String, Value>>()
        .into();

    Ok(json!({
        "total_events": total,
        "event_types": event_types_json,
        "first_timestamp": first_ts,
        "last_timestamp": last_ts,
    }))
}

/// Read a JSONL file and return parsed JSON values.
fn read_jsonl(path: &Path) -> crate::error::Result<Vec<Value>> {
    let file = fs::File::open(path)?;
    let reader = BufReader::new(file);
    let mut results = Vec::new();

    for line in reader.lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match serde_json::from_str(trimmed) {
            Ok(value) => results.push(value),
            Err(_) => continue, // Skip malformed lines
        }
    }

    Ok(results)
}
