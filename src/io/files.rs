use std::path::Path;

/// Read a file with retry logic.
pub async fn read_with_retry(_path: &Path, _max_retries: u32) -> crate::error::Result<String> {
    todo!()
}

/// Write a file with retry logic.
pub async fn write_with_retry(
    _path: &Path,
    _content: &str,
    _max_retries: u32,
) -> crate::error::Result<()> {
    todo!()
}

/// Write a file with backup of the original.
/// Creates a .backup copy of the existing file before overwriting.
/// If backup_suffix is provided, uses that instead of ".backup".
/// Creates parent directories if they don't exist.
pub fn write_with_backup(
    _path: &Path,
    _content: &str,
    _backup_suffix: Option<&str>,
) -> crate::error::Result<()> {
    todo!()
}

/// Write binary data with backup.
pub fn write_with_backup_bytes(
    _path: &Path,
    _data: &[u8],
    _backup_suffix: Option<&str>,
) -> crate::error::Result<()> {
    todo!()
}
