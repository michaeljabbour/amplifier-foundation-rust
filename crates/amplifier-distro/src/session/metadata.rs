//! Session metadata persistence.
//!
//! Stores structured JSON metadata alongside each session.  On repeated
//! writes the new fields are **merged** into the existing file so callers can
//! update individual keys without clobbering the rest.

use std::io::Write;
use std::path::Path;

use serde_json::{Map, Value};

use crate::conventions::METADATA_FILENAME;

/// Write `metadata` to `<session_dir>/metadata.json`.
///
/// If the file already exists, the incoming object is **shallow-merged**
/// (top-level keys from `metadata` overwrite existing keys; other existing
/// keys are preserved).  Atomic write via temp file + rename.
pub fn write_metadata(session_dir: &Path, metadata: &Value) -> crate::Result<()> {
    let path = session_dir.join(METADATA_FILENAME);
    std::fs::create_dir_all(session_dir)?;

    // Load existing content (if any) and merge.
    let mut combined: Map<String, Value> = if path.exists() {
        match std::fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => Map::new(),
        }
    } else {
        Map::new()
    };

    // Shallow merge: new keys overwrite existing.
    if let Some(new_map) = metadata.as_object() {
        for (k, v) in new_map {
            combined.insert(k.clone(), v.clone());
        }
    }

    let serialised = serde_json::to_string_pretty(&Value::Object(combined))?;

    // Atomic write.
    let mut tmp = tempfile::NamedTempFile::new_in(session_dir)?;
    tmp.write_all(serialised.as_bytes())?;
    tmp.flush()?;
    tmp.persist(&path)
        .map_err(|e| crate::DistroError::Session(format!("persist metadata: {e}")))?;

    Ok(())
}

/// Read session metadata from disk.
///
/// Returns an empty JSON object on missing file or parse errors.
pub fn read_metadata(session_dir: &Path) -> Value {
    let path = session_dir.join(METADATA_FILENAME);
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or(Value::Object(Map::new())),
        Err(_) => Value::Object(Map::new()),
    }
}
