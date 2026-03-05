//! Transcript persistence for Amplifier sessions.
//!
//! Writes a JSONL file containing one sanitised message per line.
//! The file is always fully rewritten (not appended) so the on-disk snapshot
//! matches the in-memory message list exactly.

use std::io::Write;
use std::path::Path;

use amplifier_foundation::sanitize_message;
use serde_json::Value;

use crate::conventions::TRANSCRIPT_FILENAME;

/// Roles that are filtered out before persisting (not useful for replay).
const FILTERED_ROLES: &[&str] = &["system", "developer"];

/// Write the full message list to `<session_dir>/transcript.jsonl`.
///
/// - Filters messages with `role` equal to `"system"` or `"developer"`.
/// - Sanitises each message via [`amplifier_foundation::sanitize_message`].
/// - Performs an atomic write (temp file + rename) so readers never see a
///   partial file.
pub fn write_transcript(session_dir: &Path, messages: &[Value]) -> crate::Result<()> {
    let path = session_dir.join(TRANSCRIPT_FILENAME);
    std::fs::create_dir_all(session_dir)?;

    let mut lines = String::new();
    for msg in messages {
        // Filter by role.
        if let Some(role) = msg.get("role").and_then(|r| r.as_str()) {
            if FILTERED_ROLES.contains(&role) {
                continue;
            }
        }
        let sanitised = sanitize_message(msg);
        match serde_json::to_string(&sanitised) {
            Ok(line) => {
                lines.push_str(&line);
                lines.push('\n');
            }
            Err(e) => {
                log::warn!("Skipping un-serialisable message in transcript: {e}");
            }
        }
    }

    // Atomic write via temp file + rename.
    let dir = session_dir;
    let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
    tmp.write_all(lines.as_bytes())?;
    tmp.flush()?;
    tmp.persist(&path)
        .map_err(|e| crate::DistroError::Session(format!("persist transcript: {e}")))?;

    Ok(())
}

/// Read the transcript back as a `Vec<Value>`.
///
/// Returns an empty vec on missing file or parse errors (never panics).
pub fn read_transcript(session_dir: &Path) -> Vec<Value> {
    let path = session_dir.join(TRANSCRIPT_FILENAME);
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}
