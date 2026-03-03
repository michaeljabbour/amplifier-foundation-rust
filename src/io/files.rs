use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use tracing::warn;

/// Read a file with retry logic for cloud sync delays.
///
/// Retries with exponential backoff when encountering I/O errors (errno 5).
pub async fn read_with_retry(path: &Path, max_retries: u32) -> crate::error::Result<String> {
    let max_retries = max_retries.max(1); // At least one attempt
    let mut delay_ms: u64 = 100;

    for attempt in 0..max_retries {
        match fs::read_to_string(path) {
            Ok(content) => return Ok(content),
            Err(e) => {
                let is_io_error = e.raw_os_error() == Some(5);
                if is_io_error && attempt < max_retries - 1 {
                    if attempt == 0 {
                        warn!(
                            "File I/O error reading {} - retrying. \
                            This may be due to cloud-synced files (OneDrive, Dropbox, etc.).",
                            path.display()
                        );
                    }
                    tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                    delay_ms *= 2;
                } else {
                    return Err(e.into());
                }
            }
        }
    }

    unreachable!("loop always returns")
}

/// Write a file with retry logic for cloud sync delays.
pub async fn write_with_retry(
    path: &Path,
    content: &str,
    max_retries: u32,
) -> crate::error::Result<()> {
    let max_retries = max_retries.max(1); // At least one attempt
    let mut delay_ms: u64 = 100;

    for attempt in 0..max_retries {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                let _ = fs::create_dir_all(parent);
            }
        }

        match fs::write(path, content) {
            Ok(()) => return Ok(()),
            Err(e) => {
                let is_io_error = e.raw_os_error() == Some(5);
                if is_io_error && attempt < max_retries - 1 {
                    if attempt == 0 {
                        warn!(
                            "File I/O error writing to {} - retrying. \
                            This may be due to cloud-synced files (OneDrive, Dropbox, etc.).",
                            path.display()
                        );
                    }
                    tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                    delay_ms *= 2;
                } else {
                    return Err(e.into());
                }
            }
        }
    }

    unreachable!("loop always returns")
}

// ---------------------------------------------------------------------------
// Synchronous atomic write with backup
// ---------------------------------------------------------------------------

/// Write file atomically using temp file + rename pattern.
///
/// Ensures the file is never partially written - either the old content
/// exists or the new content exists, never a mix.
fn write_atomic_bytes(path: &Path, data: &[u8]) -> crate::error::Result<()> {
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Create temp file in same directory (same filesystem for atomic rename)
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let stem = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();
    let prefix = format!(".{stem}_");

    let mut tmp_path = None;
    let result = (|| -> std::io::Result<()> {
        let mut tmp_file = tempfile::Builder::new()
            .prefix(&prefix)
            .suffix(".tmp")
            .tempfile_in(parent)?;
        let tp = tmp_file.path().to_path_buf();
        tmp_path = Some(tp);
        tmp_file.write_all(data)?;
        tmp_file.flush()?;
        // Persist by renaming to target path
        tmp_file.persist(path).map_err(|e| e.error)?;
        Ok(())
    })();

    match result {
        Ok(()) => Ok(()),
        Err(e) => {
            // Clean up temp file on failure
            if let Some(tp) = tmp_path {
                let _ = fs::remove_file(&tp);
            }
            Err(e.into())
        }
    }
}

/// Compute the backup path for a file.
///
/// Matches Python's `path.with_suffix(path.suffix + backup_suffix)`:
/// - `test.txt` + `.backup` -> `test.txt.backup`
/// - `Makefile` + `.backup` -> `Makefile.backup`
fn compute_backup_path(path: &Path, suffix: &str) -> PathBuf {
    if let Some(ext) = path.extension() {
        // Has extension: e.g., "test.txt" -> "test.txt.backup"
        let ext = ext.to_string_lossy();
        path.with_extension(format!("{ext}{suffix}"))
    } else {
        // No extension: append suffix directly
        // e.g., "Makefile" -> "Makefile.backup"
        let mut p = path.as_os_str().to_owned();
        p.push(suffix);
        PathBuf::from(p)
    }
}

/// Write a file with backup of the original.
/// Creates a .backup copy of the existing file before overwriting.
/// If backup_suffix is provided, uses that instead of ".backup".
/// Creates parent directories if they don't exist.
pub fn write_with_backup(
    path: &Path,
    content: &str,
    backup_suffix: Option<&str>,
) -> crate::error::Result<()> {
    let suffix = backup_suffix.unwrap_or(".backup");
    let backup_path = compute_backup_path(path, suffix);

    // Create backup if file exists (best effort - don't fail the write)
    if path.exists() {
        let _ = fs::copy(path, &backup_path);
    }

    // Write atomically
    write_atomic_bytes(path, content.as_bytes())
}

/// Write binary data with backup.
pub fn write_with_backup_bytes(
    path: &Path,
    data: &[u8],
    backup_suffix: Option<&str>,
) -> crate::error::Result<()> {
    let suffix = backup_suffix.unwrap_or(".backup");
    let backup_path = compute_backup_path(path, suffix);

    // Create backup if file exists (best effort - don't fail the write)
    if path.exists() {
        let _ = fs::copy(path, &backup_path);
    }

    // Write atomically
    write_atomic_bytes(path, data)
}
