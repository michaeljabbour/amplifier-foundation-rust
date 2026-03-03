# F-003: Port Wave 1 Tests as #[ignore] + Module Stubs

## 1. Overview

**Module:** dicts, paths, cache, serialization, tracing_utils, spawn
**Priority:** P0
**Depends on:** F-001, F-002

Port all Python tests for Wave 1 leaf modules as `#[ignore]` Rust integration tests. Create minimal function signature stubs in each module so the tests compile. The stubs use `todo!()` bodies. After this feature, `cargo test` reports 87 ignored tests and 0 failures.

## 2. Requirements

### Python Test Files to Port

Read each Python test file from `/Users/michaeljabbour/dev/amplifier-foundation/tests/` and create the corresponding Rust test file.

| Python Test File | Rust Test File | Test Count |
|-----------------|----------------|------------|
| `tests/test_dicts.py` | `tests/test_dicts.rs` | 18 |
| `tests/test_paths.py` | `tests/test_paths.rs` | 15 |
| `tests/test_cache.py` | `tests/test_cache.rs` | 12 |
| `tests/test_serialization.py` | `tests/test_serialization.rs` | 16 |
| `tests/test_tracing.py` | `tests/test_tracing.rs` | 9 |
| `tests/test_spawn_utils.py` | `tests/test_spawn.rs` | 17 |
| **Total** | | **87** |

### Test Porting Rules

1. **Read the Python test file first.** Every Python `test_*` method becomes a Rust `#[test] #[ignore = "Wave 1"]` function.
2. **1:1 mapping.** Python `TestDeepMerge::test_empty_dicts` -> Rust `fn test_deep_merge_empty_dicts()`.
3. **Flatten class structure.** Python test classes become Rust module comments or `mod` blocks. Test names are prefixed with the logical group.
4. **Preserve assertion semantics.** `assert x == y` -> `assert_eq!(x, y)`. `pytest.raises(TypeError)` -> comment noting the expected panic/error.
5. **Test bodies call real functions.** Import from `amplifier_foundation::*` and call the real (stub) functions. Since stubs use `todo!()` and tests are `#[ignore]`, this compiles but doesn't run.

### Module Stubs Required

Each module needs enough public API for the tests to compile:

**dicts/merge.rs:**
```rust
use serde_yaml_ng::Value;

pub fn deep_merge(parent: &Value, child: &Value) -> Value { todo!() }
pub fn merge_module_lists(parent: &[Value], child: &[Value]) -> Vec<Value> { todo!() }
```

**dicts/nested.rs:**
```rust
use serde_yaml_ng::Value;

pub fn get_nested(data: &Value, path: &[&str]) -> Option<Value> { todo!() }
pub fn set_nested(data: &mut Value, path: &[&str], value: Value) { todo!() }
```

**paths/uri.rs:**
```rust
use std::path::PathBuf;

pub fn get_amplifier_home() -> PathBuf { todo!() }

#[derive(Debug, Clone)]
pub struct ParsedURI {
    pub scheme: String,
    pub host: String,
    pub path: String,
    pub ref_: String,
    pub subpath: String,
}

impl ParsedURI {
    pub fn is_git(&self) -> bool { todo!() }
    pub fn is_file(&self) -> bool { todo!() }
    pub fn is_http(&self) -> bool { todo!() }
    pub fn is_zip(&self) -> bool { todo!() }
    pub fn is_package(&self) -> bool { todo!() }
}

pub fn parse_uri(uri: &str) -> ParsedURI { todo!() }
pub fn normalize_path(path: &str, base: &str) -> String { todo!() }

#[derive(Debug, Clone)]
pub struct ResolvedSource {
    pub active_path: PathBuf,
    pub source_root: PathBuf,
}
```

**paths/normalize.rs:**
```rust
use std::path::{Path, PathBuf};

pub fn construct_agent_path(base: &Path, name: &str) -> PathBuf { todo!() }
pub fn construct_context_path(base: &Path, name: &str) -> PathBuf { todo!() }
```

**paths/discovery.rs:**
```rust
use std::path::{Path, PathBuf};

pub fn find_files(base: &Path, pattern: &str, recursive: bool) -> Vec<PathBuf> { todo!() }
pub fn find_bundle_root(start: &Path) -> Option<PathBuf> { todo!() }
```

**cache/mod.rs (trait):**
```rust
pub mod memory;
pub mod disk;

// CacheProvider trait - generic over bundle type for now
pub trait CacheProvider {
    fn get(&self, key: &str) -> Option<serde_yaml_ng::Value>;
    fn set(&mut self, key: &str, value: serde_yaml_ng::Value);
    fn clear(&mut self);
    fn contains(&self, key: &str) -> bool;
}
```

**cache/memory.rs:**
```rust
pub struct SimpleCache { /* fields */ }
impl SimpleCache {
    pub fn new() -> Self { todo!() }
}
```

**cache/disk.rs:**
```rust
use std::path::Path;
pub struct DiskCache { /* fields */ }
impl DiskCache {
    pub fn new(cache_dir: &Path) -> Self { todo!() }
}
```

**serialization.rs:**
```rust
use serde_json::Value;
pub fn sanitize_for_json(value: &Value) -> Value { todo!() }
pub fn sanitize_message(message: &Value) -> Value { todo!() }
```

**tracing_utils.rs:**
```rust
pub fn generate_sub_session_id(
    agent_name: Option<&str>,
    parent_session_id: Option<&str>,
    parent_trace_id: Option<&str>,
) -> String { todo!() }
```

**spawn/mod.rs:**
```rust
pub mod glob;

#[derive(Debug, Clone)]
pub struct ProviderPreference {
    pub provider: String,
    pub model: String,
}

#[derive(Debug, Clone)]
pub struct ModelResolutionResult {
    pub provider: String,
    pub model: String,
    pub was_glob: bool,
}

pub fn apply_provider_preferences(
    session_config: &serde_yaml_ng::Value,
    preferences: &[ProviderPreference],
) -> serde_yaml_ng::Value { todo!() }
```

**spawn/glob.rs:**
```rust
pub fn is_glob_pattern(pattern: &str) -> bool { todo!() }
pub fn resolve_model_pattern(pattern: &str, available: &[String]) -> Option<String> { todo!() }
```

### Behavior

- Every test function has `#[ignore = "Wave 1"]` attribute
- Tests import from the library crate: `use amplifier_foundation::dicts::merge::deep_merge;`
- Test bodies contain the REAL assertion logic (not `todo!()`), calling real (stubbed) functions
- Since tests are `#[ignore]`, the `todo!()` in stubs never executes
- Module `mod.rs` files re-export submodule contents where appropriate

## 3. Acceptance Criteria

| # | Criterion | Verification |
|---|-----------|-------------|
| AC-1 | `cargo check` passes | Run `cargo check` |
| AC-2 | `cargo test` reports 0 pass, 0 fail | Run `cargo test` |
| AC-3 | `cargo test -- --ignored` reports 87 ignored tests | Run `cargo test -- --ignored 2>&1 \| grep "test result"` |
| AC-4 | Every Python test in Wave 1 files has a corresponding Rust test | Manual review |
| AC-5 | All stub functions have correct signatures matching Python API | Code review |
| AC-6 | All test files compile without errors | `cargo check --tests` |

## 4. Edge Cases

| Case | Expected Behavior |
|------|-------------------|
| Python test uses `pytest.raises` | Rust test should use comment noting expected error, or `#[should_panic]` + `#[ignore]` |
| Python test uses fixture/setup | Inline the setup in each Rust test function |
| Python test uses `@pytest.mark.parametrize` | Create individual test functions for each parameter set |
| Python test uses `Any` type | Use `serde_yaml_ng::Value` or `serde_json::Value` as appropriate |
| Python test modifies dict in-place | Use `&mut Value` or `.clone()` pattern |

## 5. Files to Create/Modify

| File | Action | Contents |
|------|--------|----------|
| `src/dicts/merge.rs` | Modify | Function stubs with `todo!()` |
| `src/dicts/nested.rs` | Modify | Function stubs with `todo!()` |
| `src/dicts/mod.rs` | Modify | Re-export public API |
| `src/paths/uri.rs` | Modify | Struct definitions + function stubs |
| `src/paths/normalize.rs` | Modify | Function stubs |
| `src/paths/discovery.rs` | Modify | Function stubs |
| `src/paths/mod.rs` | Modify | Re-export public API |
| `src/cache/mod.rs` | Modify | CacheProvider trait + submodule declarations |
| `src/cache/memory.rs` | Modify | SimpleCache struct stub |
| `src/cache/disk.rs` | Modify | DiskCache struct stub |
| `src/serialization.rs` | Modify | Function stubs |
| `src/tracing_utils.rs` | Modify | Function stub |
| `src/spawn/mod.rs` | Modify | Struct definitions + function stubs |
| `src/spawn/glob.rs` | Modify | Function stubs |
| `tests/test_dicts.rs` | Create | 18 #[ignore] tests |
| `tests/test_paths.rs` | Create | 15 #[ignore] tests |
| `tests/test_cache.rs` | Create | 12 #[ignore] tests |
| `tests/test_serialization.rs` | Create | 16 #[ignore] tests |
| `tests/test_tracing.rs` | Create | 9 #[ignore] tests |
| `tests/test_spawn.rs` | Create | 17 #[ignore] tests |

## 6. Dependencies

No new dependencies beyond F-001.

## 7. Notes

- **Read the Python source files** for each module to get exact function signatures. The stubs in this spec are approximate — the actual Python source is the authority.
- Python source location: `/Users/michaeljabbour/dev/amplifier-foundation/amplifier_foundation/`
- Python test location: `/Users/michaeljabbour/dev/amplifier-foundation/tests/`
- The `Value` type question: For dicts, use `serde_yaml_ng::Value` since the Python code works with `dict[str, Any]`. For serialization, use `serde_json::Value` since that module sanitizes for JSON.
- `paths/discovery.py` has `async def` but does no I/O — make the Rust version sync.
- The cache stubs use `serde_yaml_ng::Value` as placeholder for `Bundle` (which doesn't exist yet). This will be updated in Wave 3 when `Bundle` is defined.
