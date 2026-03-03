//! Tests for the updates module (BundleStatus, check_bundle_status, update_bundle).

use amplifier_foundation::sources::SourceStatus;
use amplifier_foundation::updates::{check_bundle_status, update_bundle, BundleStatus};

// ===========================================================================
// BundleStatus
// ===========================================================================

#[test]
fn test_bundle_status_no_sources() {
    let status = BundleStatus {
        bundle_name: "test-bundle".to_string(),
        bundle_source: None,
        sources: vec![],
    };

    assert!(!status.has_updates());
    assert!(status.updateable_sources().is_empty());
    assert!(status.up_to_date_sources().is_empty());
    assert!(status.unknown_sources().is_empty());
    assert_eq!(status.summary(), "All 0 source(s) up to date");
}

#[test]
fn test_bundle_status_with_update() {
    let status = BundleStatus {
        bundle_name: "test-bundle".to_string(),
        bundle_source: Some("git+https://github.com/org/bundle".to_string()),
        sources: vec![SourceStatus {
            uri: "git+https://github.com/org/bundle".to_string(),
            current_version: Some("abc123".to_string()),
            latest_version: Some("def456".to_string()),
            has_update: Some(true),
        }],
    };

    assert!(status.has_updates());
    assert_eq!(status.updateable_sources().len(), 1);
    assert!(status.up_to_date_sources().is_empty());
    assert_eq!(
        status.summary(),
        "1 update(s) available (0 up to date, 0 unknown)"
    );
}

#[test]
fn test_bundle_status_up_to_date() {
    let status = BundleStatus {
        bundle_name: "test-bundle".to_string(),
        bundle_source: None,
        sources: vec![SourceStatus {
            uri: "file:///path/to/bundle".to_string(),
            current_version: None,
            latest_version: None,
            has_update: Some(false),
        }],
    };

    assert!(!status.has_updates());
    assert!(status.updateable_sources().is_empty());
    assert_eq!(status.up_to_date_sources().len(), 1);
    assert_eq!(status.summary(), "All 1 source(s) up to date");
}

#[test]
fn test_bundle_status_unknown() {
    let status = BundleStatus {
        bundle_name: "test-bundle".to_string(),
        bundle_source: None,
        sources: vec![SourceStatus {
            uri: "https://example.com/bundle".to_string(),
            current_version: None,
            latest_version: None,
            has_update: None,
        }],
    };

    assert!(!status.has_updates());
    assert_eq!(status.unknown_sources().len(), 1);
    assert_eq!(
        status.summary(),
        "Up to date (1 source(s) could not be checked)"
    );
}

#[test]
fn test_bundle_status_mixed() {
    let status = BundleStatus {
        bundle_name: "mixed-bundle".to_string(),
        bundle_source: None,
        sources: vec![
            SourceStatus {
                uri: "git+https://github.com/org/a".to_string(),
                current_version: Some("abc".to_string()),
                latest_version: Some("def".to_string()),
                has_update: Some(true),
            },
            SourceStatus {
                uri: "file:///local/bundle".to_string(),
                current_version: None,
                latest_version: None,
                has_update: Some(false),
            },
            SourceStatus {
                uri: "https://example.com".to_string(),
                current_version: None,
                latest_version: None,
                has_update: None,
            },
        ],
    };

    assert!(status.has_updates());
    assert_eq!(status.updateable_sources().len(), 1);
    assert_eq!(status.up_to_date_sources().len(), 1);
    assert_eq!(status.unknown_sources().len(), 1);
    assert_eq!(
        status.summary(),
        "1 update(s) available (1 up to date, 1 unknown)"
    );
}

#[test]
fn test_bundle_status_partial_eq() {
    let a = BundleStatus {
        bundle_name: "test".to_string(),
        bundle_source: None,
        sources: vec![],
    };
    let b = a.clone();
    assert_eq!(a, b);
}

// ===========================================================================
// check_bundle_status
// ===========================================================================

#[tokio::test]
async fn test_check_bundle_status_file_uri() {
    // check_bundle_status only parses the URI, it doesn't touch the filesystem
    let status = check_bundle_status("file:///some/path").await.unwrap();

    assert_eq!(status.bundle_name, "file:///some/path");
    assert_eq!(status.bundle_source, Some("file:///some/path".to_string()));
    assert!(!status.has_updates());
    assert_eq!(status.sources.len(), 1);
    assert_eq!(status.sources[0].has_update, Some(false));
}

#[tokio::test]
async fn test_check_bundle_status_local_path() {
    // Absolute paths are also file-type
    let status = check_bundle_status("/some/local/path").await.unwrap();
    assert_eq!(status.sources[0].has_update, Some(false));
}

#[tokio::test]
async fn test_check_bundle_status_git_uri() {
    // Git status checking isn't implemented yet, so it returns unknown
    let status = check_bundle_status("git+https://github.com/org/bundle@main")
        .await
        .unwrap();

    assert_eq!(status.sources.len(), 1);
    assert_eq!(status.sources[0].has_update, None);
}

#[tokio::test]
async fn test_check_bundle_status_http_uri() {
    let status = check_bundle_status("https://example.com/bundle.yaml")
        .await
        .unwrap();

    assert_eq!(status.sources.len(), 1);
    assert_eq!(status.sources[0].has_update, None);
}

// ===========================================================================
// update_bundle
// ===========================================================================

#[tokio::test]
async fn test_update_bundle_file_uri() {
    // File URIs have nothing to update (always local)
    let result = update_bundle("file:///some/bundle").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_update_bundle_local_path() {
    // Local paths also nothing to update
    let result = update_bundle("/some/local/path").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_update_bundle_git_uri() {
    // Git update isn't implemented, should return error
    let result = update_bundle("git+https://github.com/org/bundle@main").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_update_bundle_http_uri() {
    // HTTP update isn't implemented, should return error
    let result = update_bundle("https://example.com/bundle.yaml").await;
    assert!(result.is_err());
}
