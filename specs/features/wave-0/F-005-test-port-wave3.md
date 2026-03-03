# F-005: Port Wave 3 Tests as #[ignore] + Module Stubs

## 1. Overview

**Module:** bundle, registry, validator
**Priority:** P0
**Depends on:** F-001, F-002, F-003, F-004

Port all Python tests for Wave 3 core modules as `#[ignore]` Rust integration tests. Create minimal stubs for Bundle struct, BundleRegistry, and BundleValidator. After this feature, ALL 235 Python tests exist as #[ignore] Rust tests. Wave 0 is complete.

## 2. Requirements

### Python Test Files to Port

| Python Test File | Rust Test File | Test Count |
|-----------------|----------------|------------|
| `tests/test_bundle.py` | `tests/test_bundle.rs` | 26 |
| `tests/test_registry.py` | `tests/test_registry.rs` | 13 |
| `tests/test_validator.py` | `tests/test_validator.rs` | 18 |
| **Total** | | **57** |

### Module Stubs Required

**bundle/mod.rs:** (Bundle struct — the central type)
```rust
use serde_yaml_ng::Value;
use std::path::PathBuf;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Bundle {
    pub name: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub includes: Vec<Value>,       // String or dict
    pub session: Value,             // Deep-merged session config
    pub spawn: Value,               // Deep-merged spawn config
    pub providers: Vec<Value>,      // Module list (merge by ID)
    pub tools: Vec<Value>,          // Module list (merge by ID)
    pub hooks: Vec<Value>,          // Module list (merge by ID)
    pub agents: HashMap<String, Value>,  // Dict update (later wins)
    pub context: HashMap<String, PathBuf>, // Accumulate with namespace
    pub instruction: Option<String>,  // Later replaces entirely
    pub base_path: Option<PathBuf>,
    pub source_base_paths: HashMap<String, PathBuf>,
    pub pending_context: HashMap<String, Value>,
    pub extra: Value,               // Forward compat
    // Dynamic fields that Python sets via type:ignore
    pub source_uri: Option<String>,
}

impl Bundle {
    pub fn new() -> Self { todo!() }
    pub fn from_dict(data: &Value) -> crate::error::Result<Self> { todo!() }
    pub fn to_dict(&self) -> Value { todo!() }
}
```

**bundle/compose.rs:**
```rust
use super::Bundle;

pub fn compose(base: &Bundle, overlay: &Bundle) -> Bundle { todo!() }
```

**bundle/validator.rs:**
```rust
use crate::error::ValidationResult;

pub struct BundleValidator { /* rules */ }

impl BundleValidator {
    pub fn new() -> Self { todo!() }
}

pub fn validate_bundle(bundle: &super::Bundle) -> ValidationResult { todo!() }
pub fn validate_bundle_or_raise(bundle: &super::Bundle) -> crate::error::Result<()> { todo!() }
```

**registry/mod.rs:**
```rust
use crate::bundle::Bundle;
use serde_yaml_ng::Value;

#[derive(Debug, Clone)]
pub struct BundleState {
    pub bundle: Bundle,
    pub uri: String,
}

#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub name: String,
    pub has_update: bool,
}

pub struct BundleRegistry { /* fields */ }

impl BundleRegistry {
    pub fn new() -> Self { todo!() }
    pub async fn load(&self, uri: &str) -> crate::error::Result<Bundle> { todo!() }
}

pub async fn load_bundle(uri: &str) -> crate::error::Result<Bundle> { todo!() }
```

**modules/mod.rs + state.rs:**
```rust
// Minimal stubs for module activation
```

**updates/mod.rs:**
```rust
use crate::sources::SourceStatus;

#[derive(Debug, Clone)]
pub struct BundleStatus {
    pub name: String,
    pub source_status: Option<SourceStatus>,
}

pub async fn check_bundle_status(uri: &str) -> crate::error::Result<BundleStatus> { todo!() }
pub async fn update_bundle(uri: &str) -> crate::error::Result<()> { todo!() }
```

### Behavior

- Bundle struct fields match the 5 composition strategies from the architecture spec
- `Bundle::from_dict` parses a `serde_yaml_ng::Value` into a Bundle (stub for now)
- `ValidationResult` is already defined in `error.rs` from F-002
- Registry tests may use mocks for source resolution — note in test comments
- All tests use `#[ignore = "Wave 3"]`
- Async tests use `#[tokio::test]` + `#[ignore = "Wave 3"]`

## 3. Acceptance Criteria

| # | Criterion | Verification |
|---|-----------|-------------|
| AC-1 | `cargo check --tests` passes | `cargo check --tests` |
| AC-2 | `cargo test -- --ignored` reports exactly 235 ignored tests | Count: 87 + 91 + 57 = 235 |
| AC-3 | `cargo test` reports 0 pass, 0 fail | `cargo test` |
| AC-4 | Bundle struct has all fields matching Python Bundle class | Code review against bundle.py |
| AC-5 | Every Python test across ALL 13 test files has a Rust counterpart | Manual review |
| AC-6 | `cargo clippy --all-targets` has no errors | `cargo clippy --all-targets` |

## 4. Edge Cases

| Case | Expected Behavior |
|------|-------------------|
| Python test uses `Bundle.from_dict({"bundle": {...}})` | Rust test calls `Bundle::from_dict(&value)` with equivalent YAML Value |
| Python test creates temp directories | Use `tempfile::tempdir()` |
| Python test uses async registry loading | `#[tokio::test]` + `#[ignore]` |
| Python test checks specific error types | Note which `BundleError` variant is expected |
| Python test uses `conftest.py` fixtures | Inline as helper functions in Rust test file |

## 5. Files to Create/Modify

| File | Action | Contents |
|------|--------|----------|
| `src/bundle/mod.rs` | Modify | Bundle struct definition + submodule declarations |
| `src/bundle/compose.rs` | Modify | compose() stub |
| `src/bundle/validator.rs` | Modify | BundleValidator + validate functions |
| `src/bundle/mount.rs` | Modify | MountPlan stub |
| `src/bundle/prepared.rs` | Modify | PreparedBundle stub |
| `src/bundle/module_resolver.rs` | Modify | BundleModuleResolver stub |
| `src/bundle/prompt.rs` | Modify | Prompt factory stub |
| `src/registry/mod.rs` | Modify | BundleRegistry + BundleState + load_bundle |
| `src/registry/persistence.rs` | Modify | Persistence stubs |
| `src/registry/includes.rs` | Modify | Include parsing stubs |
| `src/modules/mod.rs` | Modify | Module activation stubs |
| `src/modules/state.rs` | Modify | ModuleInstallState stub |
| `src/updates/mod.rs` | Modify | BundleStatus + check/update stubs |
| `tests/test_bundle.rs` | Create | 26 #[ignore] tests |
| `tests/test_registry.rs` | Create | 13 #[ignore] tests |
| `tests/test_validator.rs` | Create | 18 #[ignore] tests |

## 6. Dependencies

No new dependencies.

## 7. Notes

- This is the FINAL Wave 0 feature. After this, the gate check runs.
- **Wave 0 Gate:** `cargo check` passes, `cargo test` = 0 pass / 0 fail, `cargo test -- --ignored` = 235 ignored, `cargo clippy --all-targets` clean.
- The Bundle struct is the most important type in the crate. Get the field names and types right — they define the composition contract.
- Read `bundle.py` lines 1-100 carefully for the Bundle class definition.
- Read `registry.py` lines 1-50 for BundleRegistry and BundleState.
- `SourceStatus` is defined in `sources/mod.rs` (from F-004) — reference it in `updates/mod.rs`.
- After this feature completes and gate passes: HUMAN APPROVAL required before proceeding to Wave 1.
