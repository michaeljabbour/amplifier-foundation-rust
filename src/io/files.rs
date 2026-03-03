use std::path::Path;

/// Read a file with retry logic.
pub async fn read_with_retry(path: &Path, max_retries: u32) -> crate::error::Result<String> {
    todo!()
}

/// Write a file with retry logic.
pub async fn write_with_retry(
    path: &Path,
    content: &str,
    max_retries: u32,
) -> crate::error::Result<()> {
    todo!()
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
    todo!()
}

/// Write binary data with backup.
pub fn write_with_backup_bytes(
    path: &Path,
    data: &[u8],
    backup_suffix: Option<&str>,
) -> crate::error::Result<()> {
    todo!()
}
