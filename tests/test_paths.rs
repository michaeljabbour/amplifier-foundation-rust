use std::path::Path;

use amplifier_foundation::paths::normalize::{construct_agent_path, construct_context_path};
use amplifier_foundation::paths::uri::{normalize_path, parse_uri};

// ---------------------------------------------------------------------------
// TestParseUri
// ---------------------------------------------------------------------------

#[test]
#[ignore = "Wave 1"]
fn test_git_https_uri() {
    let uri = parse_uri("git+https://github.com/user/repo@main");
    assert_eq!(uri.scheme, "git+https");
    assert_eq!(uri.host, "github.com");
    assert_eq!(uri.path, "/user/repo");
    assert_eq!(uri.ref_, "main");
}

#[test]
#[ignore = "Wave 1"]
fn test_git_uri_with_subdirectory_fragment() {
    let uri = parse_uri(
        "git+https://github.com/org/repo@main#subdirectory=bundles/foundation",
    );
    assert_eq!(uri.scheme, "git+https");
    assert_eq!(uri.host, "github.com");
    assert_eq!(uri.path, "/org/repo");
    assert_eq!(uri.ref_, "main");
    assert_eq!(uri.subpath, "bundles/foundation");
}

#[test]
#[ignore = "Wave 1"]
fn test_zip_https_uri() {
    let uri = parse_uri(
        "zip+https://releases.example.com/bundle.zip#subdirectory=foundation",
    );
    assert_eq!(uri.scheme, "zip+https");
    assert_eq!(uri.host, "releases.example.com");
    assert_eq!(uri.path, "/bundle.zip");
    assert_eq!(uri.subpath, "foundation");
    assert!(uri.is_zip());
}

#[test]
#[ignore = "Wave 1"]
fn test_zip_file_uri() {
    let uri =
        parse_uri("zip+file:///local/archive.zip#subdirectory=my-bundle");
    assert_eq!(uri.scheme, "zip+file");
    assert_eq!(uri.path, "/local/archive.zip");
    assert_eq!(uri.subpath, "my-bundle");
    assert!(uri.is_zip());
}

#[test]
#[ignore = "Wave 1"]
fn test_file_uri() {
    let uri = parse_uri("file:///home/user/bundle");
    assert_eq!(uri.scheme, "file");
    assert_eq!(uri.path, "/home/user/bundle");
}

#[test]
#[ignore = "Wave 1"]
fn test_https_uri() {
    let uri = parse_uri("https://example.com/bundle.yaml");
    assert_eq!(uri.scheme, "https");
    assert_eq!(uri.host, "example.com");
    assert_eq!(uri.path, "/bundle.yaml");
}

#[test]
#[ignore = "Wave 1"]
fn test_local_path() {
    let uri = parse_uri("/home/user/bundle");
    assert_eq!(uri.scheme, "file");
    assert_eq!(uri.path, "/home/user/bundle");
}

#[test]
#[ignore = "Wave 1"]
fn test_relative_path() {
    let uri = parse_uri("./bundles/my-bundle");
    assert_eq!(uri.scheme, "file");
    assert_eq!(uri.path, "./bundles/my-bundle");
}

// ---------------------------------------------------------------------------
// TestNormalizePath
// ---------------------------------------------------------------------------

#[test]
#[ignore = "Wave 1"]
fn test_absolute_path() {
    let result = normalize_path("/home/user/file.txt", None);
    assert_eq!(result, Path::new("/home/user/file.txt"));
}

#[test]
#[ignore = "Wave 1"]
fn test_relative_path_with_base() {
    let result =
        normalize_path("file.txt", Some(Path::new("/home/user")));
    assert_eq!(result, Path::new("/home/user/file.txt"));
}

#[test]
#[ignore = "Wave 1"]
fn test_relative_path_without_base() {
    let result = normalize_path("file.txt", None);
    assert!(result.is_absolute());
}

#[test]
#[ignore = "Wave 1"]
fn test_path_object_input() {
    let result = normalize_path("/home/user/file.txt", None);
    assert_eq!(result, Path::new("/home/user/file.txt"));
}

// ---------------------------------------------------------------------------
// TestConstructPaths
// ---------------------------------------------------------------------------

#[test]
#[ignore = "Wave 1"]
fn test_construct_agent_path() {
    let result = construct_agent_path(Path::new("/bundle"), "code-reviewer");
    assert_eq!(result, Path::new("/bundle/agents/code-reviewer.md"));
}

#[test]
#[ignore = "Wave 1"]
fn test_construct_context_path() {
    let base = Path::new("/bundle");

    // Paths are relative to bundle root - explicit, no implicit prefix
    let result = construct_context_path(base, "context/philosophy.md");
    assert_eq!(result, Path::new("/bundle/context/philosophy.md"));

    // Works with any extension and directory
    let result = construct_context_path(base, "context/config.yaml");
    assert_eq!(result, Path::new("/bundle/context/config.yaml"));

    // Works with nested paths
    let result = construct_context_path(base, "context/examples/snippet.py");
    assert_eq!(result, Path::new("/bundle/context/examples/snippet.py"));

    // Works with non-context directories too
    let result = construct_context_path(base, "providers/anthropic.yaml");
    assert_eq!(result, Path::new("/bundle/providers/anthropic.yaml"));

    let result = construct_context_path(base, "agents/explorer.md");
    assert_eq!(result, Path::new("/bundle/agents/explorer.md"));
}

#[test]
#[ignore = "Wave 1"]
fn test_paths_are_standardized() {
    let base = Path::new("/test");

    let agent = construct_agent_path(base, "agent");
    assert!(
        agent.to_str().unwrap().contains("agents"),
        "agent path should contain 'agents' directory"
    );

    // Context path is now explicit - must include context/ prefix
    let context = construct_context_path(base, "context/ctx");
    assert!(
        context.to_str().unwrap().contains("context"),
        "context path should contain 'context' directory"
    );
}