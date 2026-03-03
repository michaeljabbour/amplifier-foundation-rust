# F-004: Port Wave 2 Tests as #[ignore] + Module Stubs

## 1. Overview

**Module:** io, sources, mentions, session
**Priority:** P0
**Depends on:** F-001, F-002, F-003

Port all Python tests for Wave 2 mid-tier modules as `#[ignore]` Rust integration tests. Create minimal function signature stubs so tests compile. After this feature, Wave 2 tests are added to the ignored test suite.

## 2. Requirements

### Python Test Files to Port

| Python Test File | Rust Test File | Test Count |
|-----------------|----------------|------------|
| `tests/test_io_files.py` | `tests/test_io_files.rs` | 6 |
| `tests/test_sources.py` | `tests/test_sources.rs` | 11 |
| `tests/test_mentions.py` | `tests/test_mentions.rs` | 21 |
| `tests/test_session.py` | `tests/test_session.rs` | 53 |
| **Total** | | **91** |

### Test Porting Rules

Same rules as F-003:
1. Read the Python test file from `/Users/michaeljabbour/dev/amplifier-foundation/tests/`
2. 1:1 mapping of Python test methods to Rust test functions
3. All tests marked `#[ignore = "Wave 2"]`
4. Test bodies contain real assertion logic calling stubbed functions
5. Python `@pytest.mark.asyncio` tests become `#[tokio::test]` + `#[ignore]` in Rust

### Module Stubs Required

**io/files.rs:** (ASYNC)
```rust
use std::path::Path;

pub async fn read_with_retry(path: &Path, max_retries: u32) -> crate::error::Result<String> { todo!() }
pub async fn write_with_retry(path: &Path, content: &str, max_retries: u32) -> crate::error::Result<()> { todo!() }
pub async fn write_with_backup(path: &Path, content: &str) -> crate::error::Result<()> { todo!() }
```

**io/yaml.rs:** (ASYNC)
```rust
use std::path::Path;
use serde_yaml_ng::Value;

pub async fn read_yaml(path: &Path) -> crate::error::Result<Value> { todo!() }
pub async fn write_yaml(path: &Path, value: &Value) -> crate::error::Result<()> { todo!() }
```

**io/frontmatter.rs:** (SYNC)
```rust
use serde_yaml_ng::Value;

pub fn parse_frontmatter(content: &str) -> crate::error::Result<(Option<Value>, &str)> { todo!() }
```

**sources/mod.rs:** (traits + re-exports)
```rust
// Re-export SourceStatus from paths or define here
pub use crate::paths::uri::{ParsedURI, ResolvedSource};

pub trait SourceHandler: Send + Sync {
    fn can_handle(&self, parsed: &ParsedURI) -> bool;
    async fn resolve(&self, parsed: &ParsedURI, cache_dir: &std::path::Path)
        -> crate::error::Result<ResolvedSource>;
}
```

**sources/resolver.rs:**
```rust
pub struct SimpleSourceResolver { /* fields */ }
impl SimpleSourceResolver {
    pub fn new() -> Self { todo!() }
    pub async fn resolve(&self, uri: &str) -> crate::error::Result<crate::paths::uri::ResolvedSource> { todo!() }
}
```

**mentions/models.rs:** (SYNC)
```rust
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ContextFile {
    pub path: PathBuf,
    pub content: String,
    pub mention: String,
}

#[derive(Debug, Clone)]
pub struct MentionResult {
    pub files: Vec<ContextFile>,
    pub failed: Vec<String>,
}
```

**mentions/parser.rs:** (SYNC)
```rust
pub fn parse_mentions(text: &str) -> Vec<String> { todo!() }
```

**mentions/resolver.rs:** (SYNC)
```rust
pub struct BaseMentionResolver { /* fields */ }
```

**mentions/dedup.rs:** (SYNC)
```rust
pub struct ContentDeduplicator { /* fields */ }
impl ContentDeduplicator {
    pub fn new() -> Self { todo!() }
}
```

**mentions/loader.rs:** (ASYNC)
```rust
pub async fn load_mentions(
    text: &str,
    resolver: &dyn crate::mentions::resolver::MentionResolver,
) -> crate::mentions::models::MentionResult { todo!() }
```

**session/capabilities.rs:** (SYNC)
```rust
pub const WORKING_DIR_CAPABILITY: &str = "working_dir";
pub fn get_working_dir(capabilities: &serde_json::Value) -> Option<String> { todo!() }
pub fn set_working_dir(capabilities: &mut serde_json::Value, dir: &str) { todo!() }
```

**session/events.rs:** (SYNC)
```rust
// Session event types and JSONL serialization stubs
```

**session/fork.rs:** (SYNC)
```rust
// Session forking logic stubs
```

**session/slice.rs:** (SYNC)
```rust
// Message list manipulation stubs
```

### Behavior

- Tests for async functions use `#[tokio::test]` + `#[ignore = "Wave 2"]`
- Tests for sync functions use `#[test]` + `#[ignore = "Wave 2"]`
- `test_session.py` has 53 tests — the largest test file. Port all of them.
- Session tests may reference types from bundle (which doesn't exist yet) — use placeholder types or `serde_yaml_ng::Value` where needed

## 3. Acceptance Criteria

| # | Criterion | Verification |
|---|-----------|-------------|
| AC-1 | `cargo check --tests` passes | Run `cargo check --tests` |
| AC-2 | `cargo test -- --ignored` reports 87 + 91 = 178 ignored tests | Count check |
| AC-3 | `cargo test` still reports 0 pass, 0 fail | Run `cargo test` |
| AC-4 | Every Python test in Wave 2 files has a corresponding Rust test | Manual review |
| AC-5 | Async test stubs use `#[tokio::test]` attribute | Code review |

## 4. Edge Cases

| Case | Expected Behavior |
|------|-------------------|
| Python tests import `Bundle` | Use `serde_yaml_ng::Value` as placeholder until Bundle exists |
| Python tests use `AsyncMock` | Note in comment, use `todo!()` body |
| Python tests use `tmp_path` fixture | Use `tempfile::tempdir()` in Rust |
| `test_session.py` has 53 tests | Port ALL of them — this is the behavioral spec |

## 5. Files to Create/Modify

| File | Action | Contents |
|------|--------|----------|
| `src/io/files.rs` | Modify | Async function stubs |
| `src/io/yaml.rs` | Modify | Async function stubs |
| `src/io/frontmatter.rs` | Modify | Sync function stub |
| `src/io/mod.rs` | Modify | Re-exports |
| `src/sources/mod.rs` | Modify | Trait definition + re-exports |
| `src/sources/resolver.rs` | Modify | SimpleSourceResolver stub |
| `src/sources/file.rs` | Modify | FileSourceHandler stub |
| `src/sources/git.rs` | Modify | GitSourceHandler stub |
| `src/sources/http.rs` | Modify | HttpSourceHandler stub |
| `src/sources/zip.rs` | Modify | ZipSourceHandler stub |
| `src/mentions/models.rs` | Modify | Struct definitions |
| `src/mentions/parser.rs` | Modify | Function stub |
| `src/mentions/resolver.rs` | Modify | BaseMentionResolver stub |
| `src/mentions/dedup.rs` | Modify | ContentDeduplicator stub |
| `src/mentions/loader.rs` | Modify | Async function stub |
| `src/mentions/utils.rs` | Modify | format_directory_listing stub |
| `src/mentions/mod.rs` | Modify | Re-exports + MentionResolver trait |
| `src/session/capabilities.rs` | Modify | Function stubs + constant |
| `src/session/events.rs` | Modify | Type stubs |
| `src/session/fork.rs` | Modify | Type stubs |
| `src/session/slice.rs` | Modify | Type stubs |
| `src/session/mod.rs` | Modify | Re-exports |
| `tests/test_io_files.rs` | Create | 6 #[ignore] tests |
| `tests/test_sources.rs` | Create | 11 #[ignore] tests |
| `tests/test_mentions.rs` | Create | 21 #[ignore] tests |
| `tests/test_session.rs` | Create | 53 #[ignore] tests |

## 6. Dependencies

No new dependencies.

## 7. Notes

- Read the Python source AND test files to understand exact APIs
- `test_session.py` is the largest file (53 tests, 549 LOC). Budget time accordingly.
- Session module has complex types (SessionEvent, etc.) — stub them minimally for compilation
- Some session tests may depend on the full Bundle type. Use `Value` as placeholder.
- `mentions/loader.py` depends on `io/files.py` — the stubs handle this by both using `todo!()`
