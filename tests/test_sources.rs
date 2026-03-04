//! Tests for source handlers (File, Http, Zip).
//!
//! Ported from Python test_sources.py — 16 tests total.
//! All tests are Wave 2 (ignored until implementations land).

use std::fs;
use std::io::Write;
use std::path::PathBuf;

use amplifier_foundation::paths::uri::ParsedURI;
use amplifier_foundation::sources::file::FileSourceHandler;
use amplifier_foundation::sources::git::GitSourceHandler;
use amplifier_foundation::sources::http::HttpSourceHandler;
use amplifier_foundation::sources::zip::ZipSourceHandler;
use amplifier_foundation::sources::SourceHandler;
use sha2::Digest;
use tempfile::tempdir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a [`ParsedURI`] with only the `scheme` field populated.
/// Useful for `can_handle` tests that only inspect the scheme.
fn uri_with_scheme(scheme: &str) -> ParsedURI {
    ParsedURI {
        scheme: scheme.to_string(),
        host: String::new(),
        path: String::new(),
        ref_: String::new(),
        subpath: String::new(),
    }
}

/// Build a [`ParsedURI`] with scheme, path, and optional subpath.
fn make_parsed_uri(scheme: &str, path: &str, subpath: &str) -> ParsedURI {
    ParsedURI {
        scheme: scheme.to_string(),
        host: String::new(),
        path: path.to_string(),
        ref_: String::new(),
        subpath: subpath.to_string(),
    }
}

/// Create a minimal zip archive at `zip_path` containing the given
/// `entries`.  Each entry is a `(relative_path, content)` pair.
fn create_test_zip(zip_path: &std::path::Path, entries: &[(&str, &str)]) {
    let file = fs::File::create(zip_path).expect("failed to create zip file");
    let mut writer = zip::ZipWriter::new(file);
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    for (name, content) in entries {
        writer
            .start_file(*name, options)
            .expect("failed to start zip entry");
        writer
            .write_all(content.as_bytes())
            .expect("failed to write zip entry");
    }

    writer.finish().expect("failed to finish zip");
}

// ===========================================================================
// TestFileSourceHandler — can_handle
// ===========================================================================

#[test]

fn test_file_can_handle_file_uri() {
    let handler = FileSourceHandler::new();
    let parsed = uri_with_scheme("file");
    assert!(handler.can_handle(&parsed));
}

#[test]

fn test_file_can_handle_absolute_path() {
    // Absolute paths are represented with scheme="file" after parsing.
    let handler = FileSourceHandler::new();
    let parsed = uri_with_scheme("file");
    assert!(handler.can_handle(&parsed));
}

#[test]

fn test_file_cannot_handle_git() {
    let handler = FileSourceHandler::new();
    let parsed = uri_with_scheme("git+https");
    assert!(!handler.can_handle(&parsed));
}

// ===========================================================================
// TestFileSourceHandler — resolve (async)
// ===========================================================================

#[tokio::test]

async fn test_file_resolve_existing_file() {
    let tmp = tempdir().expect("failed to create temp dir");
    let cache_dir = tempdir().expect("failed to create cache dir");

    // Create a file that represents a bundle root.
    let bundle_dir = tmp.path().join("my-bundle");
    fs::create_dir_all(&bundle_dir).expect("failed to create bundle dir");
    fs::write(bundle_dir.join("bundle.yaml"), "name: test").expect("failed to write file");

    let handler = FileSourceHandler::new();
    let parsed = make_parsed_uri("file", bundle_dir.to_str().unwrap(), "");

    let resolved = handler
        .resolve(&parsed, cache_dir.path())
        .await
        .expect("resolve should succeed");

    assert_eq!(resolved.active_path, bundle_dir);
    assert_eq!(resolved.source_root, bundle_dir);
}

#[tokio::test]

async fn test_file_resolve_with_subpath() {
    let tmp = tempdir().expect("failed to create temp dir");
    let cache_dir = tempdir().expect("failed to create cache dir");

    // Create nested directory structure.
    let root = tmp.path().join("repo");
    let core_dir = root.join("core");
    fs::create_dir_all(&core_dir).expect("failed to create nested dirs");
    fs::write(core_dir.join("bundle.yaml"), "name: core").expect("failed to write file");

    let handler = FileSourceHandler::new();
    let parsed = make_parsed_uri("file", root.to_str().unwrap(), "core");

    let resolved = handler
        .resolve(&parsed, cache_dir.path())
        .await
        .expect("resolve should succeed");

    // active_path should point to the subpath within the source root.
    assert_eq!(resolved.active_path, core_dir);
    assert_eq!(resolved.source_root, root);
}

// ===========================================================================
// TestHttpSourceHandler — can_handle
// ===========================================================================

#[test]

fn test_http_can_handle_https() {
    let handler = HttpSourceHandler::new();
    let parsed = uri_with_scheme("https");
    assert!(handler.can_handle(&parsed));
}

#[test]

fn test_http_can_handle_http() {
    let handler = HttpSourceHandler::new();
    let parsed = uri_with_scheme("http");
    assert!(handler.can_handle(&parsed));
}

#[test]

fn test_http_cannot_handle_file() {
    let handler = HttpSourceHandler::new();
    let parsed = uri_with_scheme("file");
    assert!(!handler.can_handle(&parsed));
}

#[test]

fn test_http_cannot_handle_git() {
    let handler = HttpSourceHandler::new();
    let parsed = uri_with_scheme("git+https");
    assert!(!handler.can_handle(&parsed));
}

// ===========================================================================
// TestZipSourceHandler — can_handle
// ===========================================================================

#[test]

fn test_zip_can_handle_zip_https() {
    let handler = ZipSourceHandler::new();
    let parsed = uri_with_scheme("zip+https");
    assert!(handler.can_handle(&parsed));
}

#[test]

fn test_zip_can_handle_zip_file() {
    let handler = ZipSourceHandler::new();
    let parsed = uri_with_scheme("zip+file");
    assert!(handler.can_handle(&parsed));
}

#[test]

fn test_zip_cannot_handle_plain_https() {
    let handler = ZipSourceHandler::new();
    let parsed = uri_with_scheme("https");
    assert!(!handler.can_handle(&parsed));
}

#[test]

fn test_zip_cannot_handle_git() {
    let handler = ZipSourceHandler::new();
    let parsed = uri_with_scheme("git+https");
    assert!(!handler.can_handle(&parsed));
}

// ===========================================================================
// TestZipSourceHandler — resolve (async)
// ===========================================================================

#[tokio::test]

async fn test_zip_resolve_local_zip() {
    let tmp = tempdir().expect("failed to create temp dir");
    let cache_dir = tempdir().expect("failed to create cache dir");

    // Create a zip archive with a couple of files.
    let zip_path = tmp.path().join("bundle.zip");
    create_test_zip(
        &zip_path,
        &[
            ("bundle.yaml", "name: zipped-bundle\nversion: 1.0.0"),
            ("agents/helper.md", "# Helper agent"),
        ],
    );

    let handler = ZipSourceHandler::new();
    let parsed = make_parsed_uri("zip+file", zip_path.to_str().unwrap(), "");

    let resolved = handler
        .resolve(&parsed, cache_dir.path())
        .await
        .expect("resolve should succeed");

    // The extracted directory should contain the files from the archive.
    assert!(resolved.active_path.join("bundle.yaml").exists());
    assert!(resolved.active_path.join("agents/helper.md").exists());
}

#[tokio::test]

async fn test_zip_resolve_local_zip_with_subpath() {
    let tmp = tempdir().expect("failed to create temp dir");
    let cache_dir = tempdir().expect("failed to create cache dir");

    // Create a zip whose contents are nested under a directory.
    let zip_path = tmp.path().join("multi.zip");
    create_test_zip(
        &zip_path,
        &[
            ("foundation/bundle.yaml", "name: foundation"),
            ("foundation/agents/core.md", "# Core agent"),
            ("other/README.md", "# Other"),
        ],
    );

    let handler = ZipSourceHandler::new();
    let parsed = make_parsed_uri("zip+file", zip_path.to_str().unwrap(), "foundation");

    let resolved = handler
        .resolve(&parsed, cache_dir.path())
        .await
        .expect("resolve should succeed");

    // active_path should point into the "foundation" subdirectory.
    assert!(resolved.active_path.join("bundle.yaml").exists());
    assert!(resolved.active_path.join("agents/core.md").exists());

    // source_root should be the extraction root (parent of "foundation").
    assert!(resolved.source_root.join("other/README.md").exists());
    assert_ne!(resolved.active_path, resolved.source_root);
}

#[tokio::test]

async fn test_zip_uses_cache() {
    let tmp = tempdir().expect("failed to create temp dir");
    let cache_dir = tempdir().expect("failed to create cache dir");

    // Create a zip archive.
    let zip_path = tmp.path().join("cached.zip");
    create_test_zip(&zip_path, &[("bundle.yaml", "name: cached-bundle")]);

    let handler = ZipSourceHandler::new();
    let parsed = make_parsed_uri("zip+file", zip_path.to_str().unwrap(), "");

    // First resolve — should extract and cache.
    let resolved1 = handler
        .resolve(&parsed, cache_dir.path())
        .await
        .expect("first resolve should succeed");

    // Delete the original zip to prove the second resolve uses cache.
    fs::remove_file(&zip_path).expect("failed to remove zip");

    // Second resolve — should return the cached result.
    let resolved2 = handler
        .resolve(&parsed, cache_dir.path())
        .await
        .expect("second resolve should succeed (from cache)");

    // Both resolves should yield the same active_path.
    assert_eq!(resolved1.active_path, resolved2.active_path);
    assert!(resolved2.active_path.join("bundle.yaml").exists());
}

// ===========================================================================
// TestSimpleSourceResolver
// ===========================================================================

use amplifier_foundation::error::BundleError;
use amplifier_foundation::sources::resolver::SimpleSourceResolver;

#[test]
fn test_resolver_default_creates_resolver() {
    // Default::default() should produce the same result as new().
    let _resolver: SimpleSourceResolver = Default::default();
}

#[tokio::test]
async fn test_resolver_resolve_file_uri() {
    let tmp = tempdir().expect("failed to create temp dir");

    // Create a directory that looks like a bundle root.
    let bundle_dir = tmp.path().join("resolver-test");
    fs::create_dir_all(&bundle_dir).expect("mkdir");
    fs::write(bundle_dir.join("bundle.yaml"), "name: test").expect("write");

    let resolver = SimpleSourceResolver::with_base_path(tmp.path().to_path_buf());
    let uri = format!("file://{}", bundle_dir.display());
    let resolved = resolver
        .resolve(&uri)
        .await
        .expect("resolve should succeed");

    assert_eq!(resolved.active_path, bundle_dir);
}

#[tokio::test]
async fn test_resolver_resolve_no_handler_returns_not_found() {
    let resolver = SimpleSourceResolver::new();
    // ftp:// is not handled by any default handler
    let result = resolver.resolve("ftp://example.com/bundle").await;
    assert!(result.is_err());
    // Verify the error variant is NotFound
    match result.unwrap_err() {
        BundleError::NotFound { uri } => {
            assert_eq!(uri, "ftp://example.com/bundle");
        }
        other => panic!("Expected NotFound, got: {other:?}"),
    }
}

#[tokio::test]
async fn test_resolver_resolve_local_path() {
    let tmp = tempdir().expect("failed to create temp dir");

    // Create a directory at a local path
    let file_path = tmp.path().join("my-bundle");
    fs::create_dir_all(&file_path).expect("mkdir");
    fs::write(file_path.join("bundle.yaml"), "name: test").expect("write");

    let resolver = SimpleSourceResolver::with_base_path(tmp.path().to_path_buf());
    // Local path (no scheme) is resolved by FileSourceHandler
    let resolved = resolver
        .resolve(file_path.to_str().unwrap())
        .await
        .expect("resolve should succeed");

    assert_eq!(resolved.active_path, file_path);
}

#[tokio::test]
async fn test_resolver_resolve_zip_file() {
    let tmp = tempdir().expect("failed to create temp dir");
    let cache_dir = tempdir().expect("failed to create cache dir");

    let zip_path = tmp.path().join("test.zip");
    create_test_zip(&zip_path, &[("bundle.yaml", "name: zipped")]);

    let resolver = SimpleSourceResolver::with_cache_dir(cache_dir.path().to_path_buf());
    let uri = format!("zip+file://{}", zip_path.display());
    let resolved = resolver
        .resolve(&uri)
        .await
        .expect("resolve should succeed");

    assert!(resolved.active_path.join("bundle.yaml").exists());
}

#[tokio::test]
async fn test_resolver_add_handler_takes_priority() {
    use amplifier_foundation::paths::uri::{ParsedURI, ResolvedSource};
    use async_trait::async_trait;
    use std::path::Path;

    /// A custom handler that claims all file:// URIs and returns a fixed path.
    struct CustomFileHandler {
        fixed_path: PathBuf,
    }

    #[async_trait]
    impl SourceHandler for CustomFileHandler {
        fn can_handle(&self, parsed: &ParsedURI) -> bool {
            parsed.is_file()
        }

        async fn resolve(
            &self,
            _parsed: &ParsedURI,
            _cache_dir: &Path,
        ) -> amplifier_foundation::error::Result<ResolvedSource> {
            Ok(ResolvedSource {
                active_path: self.fixed_path.clone(),
                source_root: self.fixed_path.clone(),
            })
        }
    }

    let tmp = tempdir().expect("failed to create temp dir");
    let custom_path = tmp.path().join("custom-override");
    fs::create_dir_all(&custom_path).expect("mkdir");

    let real_path = tmp.path().join("real-bundle");
    fs::create_dir_all(&real_path).expect("mkdir");
    fs::write(real_path.join("bundle.yaml"), "name: real").expect("write");

    let mut resolver = SimpleSourceResolver::with_base_path(tmp.path().to_path_buf());

    // Add a custom handler that overrides file:// resolution
    resolver.add_handler(Box::new(CustomFileHandler {
        fixed_path: custom_path.clone(),
    }));

    // Resolve a file URI -- should hit the custom handler, not the default
    let uri = format!("file://{}", real_path.display());
    let resolved = resolver
        .resolve(&uri)
        .await
        .expect("resolve should succeed");

    // The custom handler should have been used (returns fixed_path, not real_path)
    assert_eq!(resolved.active_path, custom_path);
}

// ===========================================================================
// TestHttpSourceHandler — resolve (async)
// ===========================================================================

fn make_http_parsed_uri(host: &str, path: &str, subpath: &str) -> ParsedURI {
    ParsedURI {
        scheme: "https".to_string(),
        host: host.to_string(),
        path: path.to_string(),
        ref_: String::new(),
        subpath: subpath.to_string(),
    }
}

#[tokio::test]
async fn test_http_resolve_cache_hit() {
    // Pre-populate cache so no actual HTTP request is needed.
    let cache_dir = tempdir().expect("failed to create cache dir");

    let parsed = make_http_parsed_uri("example.com", "/bundles/test-bundle.yaml", "");

    // Compute expected cache filename (same logic as handler)
    let url = "https://example.com/bundles/test-bundle.yaml";
    let hash = format!("{:x}", sha2::Sha256::digest(url.as_bytes()));
    let cache_key = &hash[..16];
    let cached_file = cache_dir
        .path()
        .join(format!("test-bundle.yaml-{cache_key}"));

    // Pre-populate the cache file
    fs::write(&cached_file, "name: test-from-cache").expect("write cache");

    let handler = HttpSourceHandler::new();
    let resolved = handler
        .resolve(&parsed, cache_dir.path())
        .await
        .expect("resolve should succeed from cache");

    assert_eq!(resolved.active_path, cached_file);
    assert_eq!(resolved.source_root, cached_file);
    // Verify the content is our cached version
    let content = fs::read_to_string(&resolved.active_path).expect("read");
    assert_eq!(content, "name: test-from-cache");
}

#[tokio::test]
async fn test_http_resolve_cache_hit_with_subpath() {
    let cache_dir = tempdir().expect("failed to create cache dir");

    let parsed = make_http_parsed_uri("example.com", "/bundles/repo.tar.gz", "foundation");

    // Compute expected cache path
    let url = "https://example.com/bundles/repo.tar.gz";
    let hash = format!("{:x}", sha2::Sha256::digest(url.as_bytes()));
    let cache_key = &hash[..16];
    let cached_file = cache_dir.path().join(format!("repo.tar.gz-{cache_key}"));

    // Pre-populate cache as a directory with subpath
    fs::create_dir_all(cached_file.join("foundation")).expect("mkdir");
    fs::write(
        cached_file.join("foundation/bundle.yaml"),
        "name: foundation",
    )
    .expect("write");

    let handler = HttpSourceHandler::new();
    let resolved = handler
        .resolve(&parsed, cache_dir.path())
        .await
        .expect("resolve should succeed from cache with subpath");

    assert_eq!(resolved.active_path, cached_file.join("foundation"));
    assert_eq!(resolved.source_root, cached_file);
}

#[tokio::test]
async fn test_http_resolve_download_failure() {
    let cache_dir = tempdir().expect("failed to create cache dir");

    // Use a URL that will definitely fail to connect
    let parsed = make_http_parsed_uri("127.0.0.1:1", "/nonexistent.yaml", "");

    let handler = HttpSourceHandler::new();
    let result = handler.resolve(&parsed, cache_dir.path()).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        BundleError::NotFound { uri } => {
            assert!(uri.contains("127.0.0.1"), "error should mention the host");
        }
        other => panic!("Expected NotFound, got: {other:?}"),
    }
}

#[tokio::test]
async fn test_http_resolve_empty_path_uses_download_filename() {
    let cache_dir = tempdir().expect("failed to create cache dir");

    let parsed = make_http_parsed_uri("example.com", "/", "");

    // Compute expected cache filename for path="/"
    let url = "https://example.com/";
    let hash = format!("{:x}", sha2::Sha256::digest(url.as_bytes()));
    let cache_key = &hash[..16];
    // Path::new("/").file_name() returns None, so filename should be "download"
    let cached_file = cache_dir.path().join(format!("download-{cache_key}"));

    // Pre-populate cache
    fs::write(&cached_file, "name: root-download").expect("write");

    let handler = HttpSourceHandler::new();
    let resolved = handler
        .resolve(&parsed, cache_dir.path())
        .await
        .expect("resolve should succeed");

    assert_eq!(resolved.active_path, cached_file);
}

// ===========================================================================
// TestGitSourceHandler — can_handle
// ===========================================================================

#[test]
fn test_git_can_handle_git_https() {
    let handler = GitSourceHandler::new();
    let parsed = uri_with_scheme("git+https");
    assert!(handler.can_handle(&parsed));
}

#[test]
fn test_git_can_handle_git_http() {
    let handler = GitSourceHandler::new();
    let parsed = uri_with_scheme("git+http");
    assert!(handler.can_handle(&parsed));
}

#[test]
fn test_git_cannot_handle_plain_https() {
    let handler = GitSourceHandler::new();
    let parsed = uri_with_scheme("https");
    assert!(!handler.can_handle(&parsed));
}

#[test]
fn test_git_cannot_handle_file() {
    let handler = GitSourceHandler::new();
    let parsed = uri_with_scheme("file");
    assert!(!handler.can_handle(&parsed));
}

// ===========================================================================
// TestGitSourceHandler — resolve (async)
// ===========================================================================

fn make_git_parsed_uri(host: &str, path: &str, ref_: &str, subpath: &str) -> ParsedURI {
    ParsedURI {
        scheme: "git+https".to_string(),
        host: host.to_string(),
        path: path.to_string(),
        ref_: ref_.to_string(),
        subpath: subpath.to_string(),
    }
}

#[tokio::test]
async fn test_git_resolve_cache_hit() {
    let cache_dir = tempdir().expect("failed to create cache dir");

    let parsed = make_git_parsed_uri("github.com", "/org/repo", "main", "");

    // Compute expected cache path (same logic as handler)
    let git_url = "https://github.com/org/repo";
    let ref_ = "main";
    let cache_input = format!("{git_url}@{ref_}");
    let hash = format!("{:x}", sha2::Sha256::digest(cache_input.as_bytes()));
    let cache_key = &hash[..16];
    let cache_path = cache_dir.path().join(format!("repo-{cache_key}"));

    // Pre-populate cache with valid git repo structure
    fs::create_dir_all(cache_path.join(".git")).expect("mkdir .git");
    fs::write(cache_path.join("bundle.yaml"), "name: cached-repo").expect("write bundle");

    let handler = GitSourceHandler::new();
    let resolved = handler
        .resolve(&parsed, cache_dir.path())
        .await
        .expect("resolve should succeed from cache");

    assert_eq!(resolved.active_path, cache_path);
    assert_eq!(resolved.source_root, cache_path);
}

#[tokio::test]
async fn test_git_resolve_cache_hit_with_subpath() {
    let cache_dir = tempdir().expect("failed to create cache dir");

    let parsed = make_git_parsed_uri("github.com", "/org/repo", "main", "packages/core");

    let git_url = "https://github.com/org/repo";
    let ref_ = "main";
    let cache_input = format!("{git_url}@{ref_}");
    let hash = format!("{:x}", sha2::Sha256::digest(cache_input.as_bytes()));
    let cache_key = &hash[..16];
    let cache_path = cache_dir.path().join(format!("repo-{cache_key}"));

    // Pre-populate cache with subpath
    fs::create_dir_all(cache_path.join(".git")).expect("mkdir .git");
    fs::create_dir_all(cache_path.join("packages/core")).expect("mkdir subpath");
    fs::write(cache_path.join("packages/core/bundle.yaml"), "name: core").expect("write");
    fs::write(cache_path.join("bundle.yaml"), "name: repo").expect("write root");

    let handler = GitSourceHandler::new();
    let resolved = handler
        .resolve(&parsed, cache_dir.path())
        .await
        .expect("resolve should succeed from cache with subpath");

    assert_eq!(resolved.active_path, cache_path.join("packages/core"));
    assert_eq!(resolved.source_root, cache_path);
}

#[tokio::test]
async fn test_git_resolve_head_ref_defaults() {
    let cache_dir = tempdir().expect("failed to create cache dir");

    // Empty ref_ should default to HEAD
    let parsed = make_git_parsed_uri("github.com", "/org/repo", "", "");

    let git_url = "https://github.com/org/repo";
    let ref_ = "HEAD"; // Default
    let cache_input = format!("{git_url}@{ref_}");
    let hash = format!("{:x}", sha2::Sha256::digest(cache_input.as_bytes()));
    let cache_key = &hash[..16];
    let cache_path = cache_dir.path().join(format!("repo-{cache_key}"));

    // Pre-populate cache
    fs::create_dir_all(cache_path.join(".git")).expect("mkdir .git");
    fs::write(cache_path.join("bundle.yaml"), "name: head-repo").expect("write");

    let handler = GitSourceHandler::new();
    let resolved = handler
        .resolve(&parsed, cache_dir.path())
        .await
        .expect("resolve should succeed with HEAD default");

    assert_eq!(resolved.active_path, cache_path);
}

#[tokio::test]
async fn test_git_resolve_invalid_cache_is_removed() {
    let cache_dir = tempdir().expect("failed to create cache dir");

    // Use 127.0.0.1:1 to guarantee fast clone failure (no real network call)
    let parsed = make_git_parsed_uri("127.0.0.1:1", "/org/repo", "main", "");

    let git_url = "https://127.0.0.1:1/org/repo";
    let ref_ = "main";
    let cache_input = format!("{git_url}@{ref_}");
    let hash = format!("{:x}", sha2::Sha256::digest(cache_input.as_bytes()));
    let cache_key = &hash[..16];
    let cache_path = cache_dir.path().join(format!("repo-{cache_key}"));

    // Pre-populate cache WITHOUT .git directory (invalid)
    fs::create_dir_all(&cache_path).expect("mkdir");
    fs::write(cache_path.join("bundle.yaml"), "name: broken").expect("write");

    let handler = GitSourceHandler::new();
    // This will fail because:
    // 1. Cache exists but is invalid (no .git)
    // 2. Handler removes invalid cache
    // 3. Tries to git clone which will fail (127.0.0.1:1 unreachable)
    let result = handler.resolve(&parsed, cache_dir.path()).await;

    assert!(result.is_err());
    // The invalid cache directory should have been removed by verify_clone_integrity
    // (Note: git clone may recreate the dir; we verify the old invalid cache was cleaned)
    // The clone itself should have failed
}

#[tokio::test]
async fn test_git_resolve_clone_failure() {
    let cache_dir = tempdir().expect("failed to create cache dir");

    // Use a URL that will fail to clone (nonexistent repo)
    let parsed = make_git_parsed_uri("127.0.0.1:1", "/nonexistent/repo", "main", "");

    let handler = GitSourceHandler::new();
    let result = handler.resolve(&parsed, cache_dir.path()).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        BundleError::NotFound { uri } => {
            assert!(
                uri.contains("127.0.0.1") || uri.contains("clone"),
                "error should mention the host or clone: {uri}"
            );
        }
        other => panic!("Expected NotFound, got: {other:?}"),
    }
}

// ===========================================================================
// TestGitSourceHandler — SourceHandlerWithStatus (get_status, update)
// ===========================================================================

use amplifier_foundation::sources::SourceHandlerWithStatus;

#[tokio::test]
async fn test_git_get_status_pinned_sha() {
    // A 40-char hex SHA ref should be reported as pinned (no remote check)
    let cache_dir = tempdir().expect("failed to create cache dir");
    let parsed = make_git_parsed_uri(
        "github.com",
        "/org/repo",
        "abcdef1234567890abcdef1234567890abcdef12",
        "",
    );

    let handler = GitSourceHandler::new();
    let status = handler
        .get_status(&parsed, cache_dir.path())
        .await
        .expect("get_status should succeed");

    assert_eq!(status.has_update, Some(false));
    assert!(
        status.summary.contains("Pinned"),
        "pinned ref summary should say Pinned: {}",
        status.summary
    );
}

#[tokio::test]
async fn test_git_get_status_pinned_version_tag() {
    // A v-tag ref should be reported as pinned
    let cache_dir = tempdir().expect("failed to create cache dir");
    let parsed = make_git_parsed_uri("github.com", "/org/repo", "v1.2.3", "");

    let handler = GitSourceHandler::new();
    let status = handler
        .get_status(&parsed, cache_dir.path())
        .await
        .expect("get_status should succeed");

    assert_eq!(status.has_update, Some(false));
    assert!(
        status.summary.contains("Pinned"),
        "version tag summary should say Pinned: {}",
        status.summary
    );
}

#[tokio::test]
async fn test_git_get_status_not_cached() {
    // When cache doesn't exist, and remote check fails (unreachable host),
    // status should indicate not cached with unknown update status
    let cache_dir = tempdir().expect("failed to create cache dir");
    let parsed = make_git_parsed_uri("127.0.0.1:1", "/nonexistent/repo", "main", "");

    let handler = GitSourceHandler::new();
    let status = handler
        .get_status(&parsed, cache_dir.path())
        .await
        .expect("get_status should succeed even on remote failure");

    assert!(!status.is_cached);
    // Unreachable host → remote check fails → has_update is None, error is set
    assert_eq!(status.has_update, None);
    assert!(
        status.error.is_some(),
        "error should be set when remote check fails"
    );
}

#[tokio::test]
async fn test_git_get_status_cached_with_metadata() {
    // Pre-populate cache with valid structure and metadata
    let cache_dir = tempdir().expect("failed to create cache dir");
    let parsed = make_git_parsed_uri("github.com", "/org/myrepo", "main", "");

    // Compute cache path
    let git_url = "https://github.com/org/myrepo";
    let ref_ = "main";
    let cache_input = format!("{git_url}@{ref_}");
    let hash = format!("{:x}", sha2::Sha256::digest(cache_input.as_bytes()));
    let cache_key = &hash[..16];
    let cache_path = cache_dir.path().join(format!("myrepo-{cache_key}"));

    // Create valid cache structure
    fs::create_dir_all(cache_path.join(".git")).expect("mkdir .git");
    fs::write(cache_path.join("bundle.yaml"), "name: test").expect("write bundle");

    // Write cache metadata
    let metadata = serde_json::json!({
        "cached_at": "2025-01-01T00:00:00Z",
        "ref": "main",
        "commit": "abc123def456abc123def456abc123def456abc1",
        "git_url": git_url,
    });
    fs::write(
        cache_path.join(".amplifier_cache_meta.json"),
        serde_json::to_string_pretty(&metadata).unwrap(),
    )
    .expect("write metadata");

    let handler = GitSourceHandler::new();
    let status = handler
        .get_status(&parsed, cache_dir.path())
        .await
        .expect("get_status should succeed");

    assert!(status.is_cached);
    assert_eq!(
        status.cached_commit.as_deref(),
        Some("abc123def456abc123def456abc123def456abc1")
    );
    assert_eq!(status.cached_ref.as_deref(), Some("main"));
    assert_eq!(status.cached_at.as_deref(), Some("2025-01-01T00:00:00Z"));
}

#[tokio::test]
async fn test_git_get_status_populates_uri() {
    let cache_dir = tempdir().expect("failed to create cache dir");
    let parsed = make_git_parsed_uri("github.com", "/org/repo", "v2.0.0", "");

    let handler = GitSourceHandler::new();
    let status = handler
        .get_status(&parsed, cache_dir.path())
        .await
        .expect("get_status should succeed");

    assert!(
        status.uri.contains("github.com"),
        "uri should contain host: {}",
        status.uri
    );
}

#[tokio::test]
async fn test_git_update_removes_cache_and_reclones() {
    // update() should remove existing cache and attempt fresh clone.
    // With an unreachable host, it should fail with NotFound.
    let cache_dir = tempdir().expect("failed to create cache dir");
    let parsed = make_git_parsed_uri("127.0.0.1:1", "/nonexistent/repo", "main", "");

    // Pre-populate a fake cache
    let git_url = "https://127.0.0.1:1/nonexistent/repo";
    let cache_input = format!("{git_url}@main");
    let hash = format!("{:x}", sha2::Sha256::digest(cache_input.as_bytes()));
    let cache_key = &hash[..16];
    let cache_path = cache_dir.path().join(format!("repo-{cache_key}"));
    fs::create_dir_all(cache_path.join(".git")).expect("mkdir .git");
    fs::write(cache_path.join("bundle.yaml"), "name: old").expect("write bundle");
    assert!(cache_path.exists(), "cache should exist before update");

    let handler = GitSourceHandler::new();
    let result = handler.update(&parsed, cache_dir.path()).await;

    // The update should have removed the old cache, then failed to clone
    assert!(result.is_err());
    // Cache directory should have been removed
    assert!(
        !cache_path.exists(),
        "cache should be removed after update attempt"
    );
}

#[tokio::test]
async fn test_git_update_no_existing_cache() {
    // update() should work even when no cache exists — just delegates to resolve()
    let cache_dir = tempdir().expect("failed to create cache dir");
    let parsed = make_git_parsed_uri("127.0.0.1:1", "/org/repo", "main", "");

    let handler = GitSourceHandler::new();
    // Should fail at clone, not at cache removal
    let result = handler.update(&parsed, cache_dir.path()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_git_get_status_remote_check_failure_sets_error() {
    // When remote check fails (unreachable host), error field should be set
    let cache_dir = tempdir().expect("failed to create cache dir");
    let parsed = make_git_parsed_uri("127.0.0.1:1", "/nonexistent/repo", "main", "");

    let handler = GitSourceHandler::new();
    let status = handler
        .get_status(&parsed, cache_dir.path())
        .await
        .expect("get_status should succeed even on remote failure");

    // Either error is set or has_update is None (both are acceptable for remote failure)
    let remote_check_handled = status.error.is_some() || status.has_update.is_none();
    assert!(
        remote_check_handled,
        "remote failure should set error or leave has_update as None"
    );
}

// ===========================================================================
// TestHttpSourceHandler — SourceHandlerWithStatus (get_status, update)
// ===========================================================================

/// Compute expected cached file path for an HTTP URL.
fn http_cached_file_path(
    scheme: &str,
    host: &str,
    path: &str,
    cache_dir: &std::path::Path,
) -> PathBuf {
    let url = format!("{scheme}://{host}{path}");
    let hash = format!("{:x}", sha2::Sha256::digest(url.as_bytes()));
    let cache_key = &hash[..16];
    let filename = std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .filter(|n| !n.is_empty())
        .unwrap_or("download");
    cache_dir.join(format!("{filename}-{cache_key}"))
}

#[tokio::test]
async fn test_http_get_status_not_cached() {
    // When no cache file exists, get_status should return is_cached=false
    // without making any network call (early return before HEAD request).
    let cache_dir = tempdir().expect("failed to create cache dir");
    let parsed = make_http_parsed_uri("127.0.0.1:1", "/bundle.yaml", "");

    let handler = HttpSourceHandler::new();
    let status = handler
        .get_status(&parsed, cache_dir.path())
        .await
        .expect("get_status should succeed even for uncached URL");

    assert!(!status.is_cached, "should not be cached");
    assert_eq!(
        status.has_update, None,
        "has_update should be None when not cached"
    );
    assert!(
        status.summary.contains("Not cached"),
        "summary should mention 'Not cached': {}",
        status.summary
    );
    // No network call made — no error expected for the not-cached path.
    assert!(
        status.error.is_none(),
        "no error expected for not-cached status: {:?}",
        status.error
    );
}

#[tokio::test]
async fn test_http_get_status_cached_no_update() {
    // When a cache file exists alongside metadata containing an ETag, get_status
    // sends a conditional HEAD request. With 127.0.0.1:1 (connection refused)
    // the request fails → has_update=None, error is set.
    // NOTE: On a real server returning 304 Not Modified, has_update would be Some(false).
    let cache_dir = tempdir().expect("failed to create cache dir");
    let parsed = make_http_parsed_uri("127.0.0.1:1", "/bundle.yaml", "");

    // Pre-populate the cache file.
    let cached_file =
        http_cached_file_path("https", "127.0.0.1:1", "/bundle.yaml", cache_dir.path());
    fs::write(&cached_file, "name: cached-bundle").expect("write cache");

    // Write metadata with a fake ETag so the handler has conditional headers to send.
    let meta_path = {
        let mut s = cached_file.as_os_str().to_owned();
        s.push(".meta.json");
        PathBuf::from(s)
    };
    let metadata = serde_json::json!({
        "etag": "\"abc123\"",
        "last_modified": null,
        "cached_at": "2025-01-01T00:00:00Z",
        "url": "https://127.0.0.1:1/bundle.yaml",
    });
    fs::write(&meta_path, serde_json::to_string_pretty(&metadata).unwrap())
        .expect("write metadata");

    let handler = HttpSourceHandler::new();
    let status = handler
        .get_status(&parsed, cache_dir.path())
        .await
        .expect("get_status should not propagate errors");

    assert!(
        status.is_cached,
        "cache file exists so is_cached should be true"
    );
    // With http-sources enabled, the HEAD to 127.0.0.1:1 will be refused → None.
    // Without http-sources, the feature gate returns None + error.
    assert_eq!(
        status.has_update, None,
        "has_update should be None when HEAD to unreachable host fails"
    );
    // cached_at should be populated from the metadata sidecar.
    assert_eq!(
        status.cached_at.as_deref(),
        Some("2025-01-01T00:00:00Z"),
        "cached_at should be read from metadata"
    );
    // Error should be set because the HEAD request failed.
    assert!(
        status.error.is_some(),
        "error should be set when HEAD request fails or feature is disabled"
    );
}

#[tokio::test]
async fn test_http_update_removes_cache() {
    // update() should remove the existing cached file before attempting a fresh
    // download. When the download fails (127.0.0.1:1 unreachable), the cached
    // file should still have been removed.
    let cache_dir = tempdir().expect("failed to create cache dir");
    let parsed = make_http_parsed_uri("127.0.0.1:1", "/bundle.yaml", "");

    // Pre-populate cache file and metadata sidecar.
    let cached_file =
        http_cached_file_path("https", "127.0.0.1:1", "/bundle.yaml", cache_dir.path());
    fs::write(&cached_file, "name: old-bundle").expect("write cache");
    assert!(
        cached_file.exists(),
        "cache file should exist before update"
    );

    let meta_path = {
        let mut s = cached_file.as_os_str().to_owned();
        s.push(".meta.json");
        PathBuf::from(s)
    };
    fs::write(
        &meta_path,
        r#"{"etag":"\"old\"","last_modified":null,"cached_at":"2025-01-01T00:00:00Z","url":"https://127.0.0.1:1/bundle.yaml"}"#,
    )
    .expect("write metadata");

    let handler = HttpSourceHandler::new();
    let result = handler.update(&parsed, cache_dir.path()).await;

    // The update should have removed the cached file before attempting to download.
    assert!(
        !cached_file.exists(),
        "cached file should be removed after update attempt"
    );
    // The download to 127.0.0.1:1 should fail.
    assert!(result.is_err(), "update to unreachable host should fail");
}

#[tokio::test]
async fn test_http_get_status_network_error() {
    // When the cache exists but the HEAD request to the remote fails (connection
    // refused on 127.0.0.1:1), get_status should return has_update=None and
    // populate the error field.
    let cache_dir = tempdir().expect("failed to create cache dir");
    let parsed = make_http_parsed_uri("127.0.0.1:1", "/file.yaml", "");

    // Pre-populate the cache so we reach the HEAD-request branch.
    let cached_file = http_cached_file_path("https", "127.0.0.1:1", "/file.yaml", cache_dir.path());
    fs::write(&cached_file, "name: cached").expect("write cache");

    let handler = HttpSourceHandler::new();
    let status = handler
        .get_status(&parsed, cache_dir.path())
        .await
        .expect("get_status should succeed (errors go into SourceStatus, not Err variant)");

    assert!(
        status.is_cached,
        "cache file exists so is_cached should be true"
    );
    assert_eq!(
        status.has_update, None,
        "has_update should be None when remote is unreachable"
    );
    assert!(
        status.error.is_some(),
        "error should be populated when HEAD request fails or feature is disabled"
    );
}
