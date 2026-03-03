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
            ..Default::default()
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
            has_update: Some(false),
            ..Default::default()
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
            has_update: None,
            ..Default::default()
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
                ..Default::default()
            },
            SourceStatus {
                uri: "file:///local/bundle".to_string(),
                has_update: Some(false),
                ..Default::default()
            },
            SourceStatus {
                uri: "https://example.com".to_string(),
                has_update: None,
                ..Default::default()
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
    let status = check_bundle_status("file:///some/path", None)
        .await
        .unwrap();

    assert_eq!(status.bundle_name, "file:///some/path");
    assert_eq!(status.bundle_source, Some("file:///some/path".to_string()));
    assert!(!status.has_updates());
    assert_eq!(status.sources.len(), 1);
    assert_eq!(status.sources[0].has_update, Some(false));
}

#[tokio::test]
async fn test_check_bundle_status_local_path() {
    // Absolute paths are also file-type
    let status = check_bundle_status("/some/local/path", None).await.unwrap();
    assert_eq!(status.sources[0].has_update, Some(false));
}

#[tokio::test]
async fn test_check_bundle_status_git_uri() {
    // Git URIs now dispatch to GitSourceHandler.get_status().
    // With an unreachable host, remote check fails → has_update is None.
    let cache_dir = tempfile::tempdir().expect("failed to create cache dir");
    let status = check_bundle_status(
        "git+https://127.0.0.1:1/org/bundle@main",
        Some(cache_dir.path()),
    )
    .await
    .unwrap();

    assert_eq!(status.sources.len(), 1);
    // Remote check fails → None (unknown)
    assert_eq!(status.sources[0].has_update, None);
}

#[tokio::test]
async fn test_check_bundle_status_git_pinned() {
    // Git URI with a pinned ref → no remote check, has_update = false.
    // Uses unreachable host to ensure no real network call even if
    // is_pinned() has a bug (defense in depth).
    let cache_dir = tempfile::tempdir().expect("failed to create cache dir");
    let status = check_bundle_status(
        "git+https://127.0.0.1:1/org/bundle@v1.0.0",
        Some(cache_dir.path()),
    )
    .await
    .unwrap();

    assert_eq!(status.sources.len(), 1);
    assert_eq!(status.sources[0].has_update, Some(false));
    assert!(
        status.sources[0].summary.contains("Pinned"),
        "pinned ref should say Pinned: {}",
        status.sources[0].summary
    );
}

#[tokio::test]
async fn test_check_bundle_status_http_uri() {
    let status = check_bundle_status("https://example.com/bundle.yaml", None)
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
    let result = update_bundle("file:///some/bundle", None).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_update_bundle_local_path() {
    // Local paths also nothing to update
    let result = update_bundle("/some/local/path", None).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_update_bundle_git_uri() {
    // Git update now dispatches to GitSourceHandler.update().
    // With an unreachable host, the clone fails → returns error.
    let cache_dir = tempfile::tempdir().expect("failed to create cache dir");
    let result = update_bundle(
        "git+https://127.0.0.1:1/org/bundle@main",
        Some(cache_dir.path()),
    )
    .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_update_bundle_http_uri() {
    // HTTP update isn't implemented, should return error
    let result = update_bundle("https://example.com/bundle.yaml", None).await;
    assert!(result.is_err());
}

// ===========================================================================
// SourceStatus enriched fields + is_pinned
// ===========================================================================

#[test]
fn test_source_status_default() {
    let status = SourceStatus::default();
    assert_eq!(status.uri, "");
    assert!(status.current_version.is_none());
    assert!(status.latest_version.is_none());
    assert!(status.has_update.is_none());
    assert!(!status.is_cached);
    assert!(status.cached_at.is_none());
    assert!(status.cached_ref.is_none());
    assert!(status.cached_commit.is_none());
    assert!(status.remote_ref.is_none());
    assert!(status.remote_commit.is_none());
    assert!(status.error.is_none());
    assert_eq!(status.summary, "");
}

#[test]
fn test_source_status_is_pinned_commit_sha() {
    let status = SourceStatus {
        cached_ref: Some("abc123def456abc123def456abc123def456abc1".to_string()),
        ..Default::default()
    };
    assert!(status.is_pinned());
}

#[test]
fn test_source_status_is_pinned_version_tag() {
    let status = SourceStatus {
        cached_ref: Some("v1.2.3".to_string()),
        ..Default::default()
    };
    assert!(status.is_pinned());
}

#[test]
fn test_source_status_is_pinned_version_tag_no_dots() {
    let status = SourceStatus {
        cached_ref: Some("v2".to_string()),
        ..Default::default()
    };
    assert!(status.is_pinned());
}

#[test]
fn test_source_status_not_pinned_branch() {
    let status = SourceStatus {
        cached_ref: Some("main".to_string()),
        ..Default::default()
    };
    assert!(!status.is_pinned());
}

#[test]
fn test_source_status_not_pinned_none() {
    let status = SourceStatus::default();
    assert!(!status.is_pinned());
}

#[test]
fn test_source_status_not_pinned_short_hex() {
    // 39 chars - not a full SHA
    let status = SourceStatus {
        cached_ref: Some("abc123def456abc123def456abc123def456abc".to_string()),
        ..Default::default()
    };
    assert!(!status.is_pinned());
}

#[test]
fn test_source_status_is_pinned_uppercase_sha() {
    // Python normalizes to lowercase via .lower() before checking hex
    let status = SourceStatus {
        cached_ref: Some("ABC123DEF456ABC123DEF456ABC123DEF456ABC1".to_string()),
        ..Default::default()
    };
    assert!(status.is_pinned());
}

#[test]
fn test_source_status_is_pinned_mixed_case_sha() {
    let status = SourceStatus {
        cached_ref: Some("aBc123def456abc123def456abc123def456abc1".to_string()),
        ..Default::default()
    };
    assert!(status.is_pinned());
}

#[test]
fn test_source_status_not_pinned_v_no_digit() {
    let status = SourceStatus {
        cached_ref: Some("version-latest".to_string()),
        ..Default::default()
    };
    assert!(!status.is_pinned());
}

#[test]
fn test_source_status_not_pinned_bare_v() {
    let status = SourceStatus {
        cached_ref: Some("v".to_string()),
        ..Default::default()
    };
    assert!(!status.is_pinned());
}

#[test]
fn test_source_status_not_pinned_empty_string() {
    let status = SourceStatus {
        cached_ref: Some(String::new()),
        ..Default::default()
    };
    assert!(!status.is_pinned());
}

#[test]
fn test_source_status_new_constructor() {
    let status = SourceStatus::new("git+https://github.com/org/repo");
    assert_eq!(status.uri, "git+https://github.com/org/repo");
    assert!(!status.is_cached);
    assert!(status.has_update.is_none());
}

#[test]
fn test_source_status_enriched_fields() {
    let status = SourceStatus {
        uri: "git+https://github.com/org/repo@main".to_string(),
        is_cached: true,
        cached_at: Some("2025-01-19T00:00:00Z".to_string()),
        cached_ref: Some("main".to_string()),
        cached_commit: Some("abc123".to_string()),
        remote_ref: Some("main".to_string()),
        remote_commit: Some("def456".to_string()),
        has_update: Some(true),
        error: None,
        summary: "Update available (abc123 -> def456)".to_string(),
        ..Default::default()
    };
    assert!(status.is_cached);
    assert_eq!(status.cached_at.as_deref(), Some("2025-01-19T00:00:00Z"));
    assert_eq!(status.cached_commit.as_deref(), Some("abc123"));
    assert_eq!(status.remote_commit.as_deref(), Some("def456"));
    assert!(status.summary.contains("Update available"));
}

#[test]
fn test_source_status_error_field() {
    let status = SourceStatus {
        uri: "git+https://example.com/repo".to_string(),
        error: Some("Connection refused".to_string()),
        summary: "Status check failed".to_string(),
        ..Default::default()
    };
    assert_eq!(status.error.as_deref(), Some("Connection refused"));
    assert!(status.has_update.is_none());
}

#[tokio::test]
async fn test_check_bundle_status_populates_summary() {
    let status = check_bundle_status("file:///some/path", None)
        .await
        .unwrap();
    assert!(!status.sources[0].summary.is_empty());
}

#[tokio::test]
async fn test_check_bundle_status_file_is_cached() {
    let status = check_bundle_status("file:///some/path", None)
        .await
        .unwrap();
    assert!(status.sources[0].is_cached);
}

#[tokio::test]
async fn test_check_bundle_status_git_not_cached() {
    let cache_dir = tempfile::tempdir().expect("failed to create cache dir");
    let status = check_bundle_status(
        "git+https://127.0.0.1:1/org/repo@main",
        Some(cache_dir.path()),
    )
    .await
    .unwrap();
    assert!(!status.sources[0].is_cached);
}
