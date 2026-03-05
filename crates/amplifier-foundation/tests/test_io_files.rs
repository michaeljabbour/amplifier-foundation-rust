use std::fs;

use amplifier_foundation::io::files::{
    read_with_retry, write_with_backup, write_with_backup_bytes, write_with_retry,
};
use tempfile::tempdir;

// ---------------------------------------------------------------------------
// TestWriteWithBackup
// ---------------------------------------------------------------------------

#[test]
fn test_creates_backup_of_existing_file() {
    let tmp = tempdir().unwrap();
    let file_path = tmp.path().join("test.txt");

    // Create an existing file with original content.
    fs::write(&file_path, "original").unwrap();

    // Overwrite via write_with_backup.
    write_with_backup(&file_path, "new content", None).unwrap();

    // The file should now contain the new content.
    assert_eq!(fs::read_to_string(&file_path).unwrap(), "new content");

    // A .backup copy of the original should exist.
    let backup_path = file_path.with_extension("txt.backup");
    assert!(backup_path.exists(), "backup file should exist");
    assert_eq!(fs::read_to_string(&backup_path).unwrap(), "original");
}

#[test]
fn test_no_backup_for_new_file() {
    let tmp = tempdir().unwrap();
    let file_path = tmp.path().join("new.txt");

    // File does not exist yet — no backup should be created.
    write_with_backup(&file_path, "fresh content", None).unwrap();

    assert_eq!(fs::read_to_string(&file_path).unwrap(), "fresh content");

    // No backup file should have been created.
    let backup_path = file_path.with_extension("txt.backup");
    assert!(
        !backup_path.exists(),
        "backup file should not exist for a new file"
    );
}

#[test]
fn test_custom_backup_suffix() {
    let tmp = tempdir().unwrap();
    let file_path = tmp.path().join("test.txt");

    fs::write(&file_path, "original").unwrap();

    write_with_backup(&file_path, "updated", Some(".bak")).unwrap();

    assert_eq!(fs::read_to_string(&file_path).unwrap(), "updated");

    // Backup should use the custom suffix.
    let backup_path = file_path.with_extension("txt.bak");
    assert!(
        backup_path.exists(),
        "custom-suffix backup file should exist"
    );
    assert_eq!(fs::read_to_string(&backup_path).unwrap(), "original");

    // Default .backup should NOT exist.
    let default_backup = file_path.with_extension("txt.backup");
    assert!(
        !default_backup.exists(),
        "default .backup should not exist when custom suffix is used"
    );
}

#[test]
fn test_creates_parent_directories() {
    let tmp = tempdir().unwrap();
    let nested_path = tmp.path().join("a").join("b").join("test.txt");

    // Parent directories do not exist yet.
    assert!(!nested_path.parent().unwrap().exists());

    write_with_backup(&nested_path, "nested content", None).unwrap();

    assert!(nested_path.exists());
    assert_eq!(fs::read_to_string(&nested_path).unwrap(), "nested content");
}

#[test]
fn test_unicode_content() {
    let tmp = tempdir().unwrap();
    let file_path = tmp.path().join("unicode.txt");

    let content = "Hello 世界 🌍";
    write_with_backup(&file_path, content, None).unwrap();

    assert_eq!(fs::read_to_string(&file_path).unwrap(), content);
}

#[test]
fn test_binary_mode() {
    let tmp = tempdir().unwrap();
    let file_path = tmp.path().join("binary.bin");

    let data: &[u8] = &[0x00, 0x01, 0x02, 0xff];

    // Write original binary data so a backup is triggered.
    fs::write(&file_path, b"old bytes").unwrap();

    write_with_backup_bytes(&file_path, data, None).unwrap();

    assert_eq!(fs::read(&file_path).unwrap(), data);

    // Backup should contain the previous binary content.
    let backup_path = file_path.with_extension("bin.backup");
    assert!(backup_path.exists(), "binary backup file should exist");
    assert_eq!(fs::read(&backup_path).unwrap(), b"old bytes");
}

// ---------------------------------------------------------------------------
// TestReadWithRetry (async)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_read_with_retry_success() {
    let tmp = tempdir().unwrap();
    let file_path = tmp.path().join("readable.txt");
    fs::write(&file_path, "hello async").unwrap();

    let content = read_with_retry(&file_path, 3).await.unwrap();
    assert_eq!(content, "hello async");
}

#[tokio::test]
async fn test_read_with_retry_missing_file() {
    let tmp = tempdir().unwrap();
    let file_path = tmp.path().join("nonexistent.txt");

    let result = read_with_retry(&file_path, 1).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_read_with_retry_unicode() {
    let tmp = tempdir().unwrap();
    let file_path = tmp.path().join("unicode.txt");
    let content = "Hello 世界 🌍";
    fs::write(&file_path, content).unwrap();

    let result = read_with_retry(&file_path, 1).await.unwrap();
    assert_eq!(result, content);
}

// ---------------------------------------------------------------------------
// TestWriteWithRetry (async)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_write_with_retry_success() {
    let tmp = tempdir().unwrap();
    let file_path = tmp.path().join("writable.txt");

    write_with_retry(&file_path, "async content", 3)
        .await
        .unwrap();

    assert_eq!(fs::read_to_string(&file_path).unwrap(), "async content");
}

#[tokio::test]
async fn test_write_with_retry_creates_parent_dirs() {
    let tmp = tempdir().unwrap();
    let nested_path = tmp.path().join("x").join("y").join("z").join("deep.txt");

    // Parent directories do not exist yet.
    assert!(!nested_path.parent().unwrap().exists());

    write_with_retry(&nested_path, "deep content", 1)
        .await
        .unwrap();

    assert!(nested_path.exists());
    assert_eq!(fs::read_to_string(&nested_path).unwrap(), "deep content");
}

#[tokio::test]
async fn test_write_with_retry_overwrites() {
    let tmp = tempdir().unwrap();
    let file_path = tmp.path().join("overwrite.txt");

    write_with_retry(&file_path, "first", 1).await.unwrap();
    write_with_retry(&file_path, "second", 1).await.unwrap();

    assert_eq!(fs::read_to_string(&file_path).unwrap(), "second");
}
