# Module Spec: paths

> Layer 2 Component Spec. Implements paths/ module (Wave 1).

## Overview

**Rust module:** `src/paths/` (`uri.rs`, `normalize.rs`, `discovery.rs`)
**Python source:** `amplifier_foundation/paths/resolution.py` (257 LOC), `construction.py` (53 LOC), `discovery.py` (56 LOC)
**Sync/Async:** Fully sync. Python has `async def` on discovery functions but they do no I/O.
**Tests:** 15 tests in `tests/test_paths.rs`
**Internal dependencies:** None (leaf module)

## Public API

### paths/uri.rs

```rust
use std::path::{Path, PathBuf};

/// Get the Amplifier home directory.
/// 1. AMPLIFIER_HOME env var (expanded, resolved)
/// 2. ~/.amplifier (default)
pub fn get_amplifier_home() -> PathBuf;

/// Parsed URI components.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedURI {
    pub scheme: String,    // git, file, http, https, zip+..., or empty for package names
    pub host: String,      // github.com, etc.
    pub path: String,      // /org/repo or local path
    pub ref_: String,      // @main, @v1.0.0 (empty if not specified)
    pub subpath: String,   // path inside container (from #subdirectory= fragment)
}

impl ParsedURI {
    pub fn is_git(&self) -> bool;     // scheme == "git" or starts with "git+"
    pub fn is_file(&self) -> bool;    // scheme == "file" or (empty scheme + "/" in path)
    pub fn is_http(&self) -> bool;    // scheme in ["http", "https"]
    pub fn is_zip(&self) -> bool;     // scheme starts with "zip+"
    pub fn is_package(&self) -> bool; // empty scheme + no "/" in path
}

/// Parse a URI string into components.
/// Handles: git+https://..., file://..., /absolute/path, ./relative/path,
/// https://..., zip+https://..., package-name, name@ref
pub fn parse_uri(uri: &str) -> ParsedURI;

/// Normalize a path relative to a base directory.
/// Handles: absolute paths, relative paths, ~/ expansion, ./prefix
pub fn normalize_path(path: &str, base: &str) -> String;

/// Result of resolving a source URI to local paths.
#[derive(Debug, Clone)]
pub struct ResolvedSource {
    pub active_path: PathBuf,    // The requested path (subdirectory or root)
    pub source_root: PathBuf,    // The full clone/extract root
}
```

### paths/normalize.rs

```rust
use std::path::{Path, PathBuf};

/// Construct path to an agent file.
/// Looks in agents/ subdirectory, appends .md if not present.
pub fn construct_agent_path(base: &Path, name: &str) -> PathBuf;

/// Construct path to a bundle resource file.
/// Name is relative to bundle root. Empty name returns base.
/// Strips leading "/" to prevent absolute path creation.
pub fn construct_context_path(base: &Path, name: &str) -> PathBuf;
```

### paths/discovery.rs

```rust
use std::path::{Path, PathBuf};

/// Find files matching a glob pattern. SYNC (not async).
/// If recursive and pattern doesn't start with "**", prepends "**/" .
pub fn find_files(base: &Path, pattern: &str, recursive: bool) -> Vec<PathBuf>;

/// Find the bundle root directory containing bundle.md or bundle.yaml.
/// Searches from start directory upward to filesystem root. SYNC.
pub fn find_bundle_root(start: &Path) -> Option<PathBuf>;
```

## Translation Decisions

| Python | Rust | Rationale |
|--------|------|-----------|
| `Path` (pathlib) | `std::path::PathBuf` / `&Path` | Standard Rust path types |
| `os.environ.get("AMPLIFIER_HOME")` | `std::env::var("AMPLIFIER_HOME")` | Env var access |
| `Path.home()` | `dirs::home_dir()` | Cross-platform home dir |
| `urlparse(uri)` | Manual parsing with regex | No exact urllib equivalent in Rust |
| `async def find_files` | `fn find_files` (sync) | No actual I/O — just `Path::glob` |
| `async def find_bundle_root` | `fn find_bundle_root` (sync) | No actual I/O — just `Path::exists` |
| `@dataclass ParsedURI` | `#[derive(Debug, Clone, PartialEq)] struct` | Standard derives |
| `@property is_git` | `pub fn is_git(&self) -> bool` | Method, not property |

## Key Behaviors to Preserve

1. **parse_uri handles all URI formats:** git+https://, file://, /absolute, ./relative, https://, zip+https://, bare-name, name@ref
2. **Ref extraction:** `repo@main` -> ref_ = "main". `repo@v1.0.0` -> ref_ = "v1.0.0"
3. **Subpath extraction:** `uri#subdirectory=path/to/dir` -> subpath = "path/to/dir"
4. **construct_agent_path auto-appends .md:** `construct_agent_path(base, "explorer")` -> `base/agents/explorer.md`
5. **construct_context_path strips leading /:** Prevents `Path(base) / "/absolute"` = `"/absolute"` bug
6. **find_bundle_root searches upward:** Checks for `bundle.md` OR `bundle.yaml` at each level
7. **get_amplifier_home resolves and expands:** `~` expansion + `resolve()` for canonical path

## Notes

- `resolution.py` is 257 LOC — the largest file in paths/. The `parse_uri` function handles many URI formats and needs careful regex work.
- `ResolvedSource` is used by the sources module (Wave 2) but defined here because it's a path type.
- The Python `normalize_path` function handles several path normalization patterns — read the full source carefully.
- `find_files` returns sorted results in Python. Preserve this in Rust.
