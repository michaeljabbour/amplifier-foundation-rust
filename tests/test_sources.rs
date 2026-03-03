//! Tests for source handlers (File, Http, Zip).
//!
//! Ported from Python test_sources.py — 16 tests total.
//! All tests are Wave 2 (ignored until implementations land).

use std::fs;
use std::io::Write;
use std::path::PathBuf;

use amplifier_foundation::paths::uri::ParsedURI;
use amplifier_foundation::sources::file::FileSourceHandler;
use amplifier_foundation::sources::http::HttpSourceHandler;
use amplifier_foundation::sources::zip::ZipSourceHandler;
use amplifier_foundation::sources::SourceHandler;
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
