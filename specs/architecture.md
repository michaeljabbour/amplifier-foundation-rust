# Architecture Spec: amplifier-foundation-rs

> Layer 1 Constitution. All implementation must conform to this document.
> Source of truth derived from: docs/rust-migration-blueprint-revised.md

## 1. Project Identity

**Name:** amplifier-foundation-rs
**Purpose:** Rust port of amplifier-foundation Python library (~8,400 LOC, 48 files)
**Philosophy:** Mechanism not policy. Loads bundles from URIs, composes them, produces mount plans.
**Core concept:** Bundle = composable unit that produces mount plans.

**Python source (reference implementation):** `/Users/michaeljabbour/dev/amplifier-foundation/`
**Rust project:** `/Users/michaeljabbour/dev/amplifier-foundation-rust/`

## 2. Technology Stack

| Component | Choice | Rationale |
|-----------|--------|-----------|
| Language | Rust 1.93+ (stable) | Target platform |
| Async runtime | tokio 1 (full features) | Industry standard, required for reqwest |
| Serialization | serde 1 + serde_json 1 + serde_yaml_ng 0.10 | serde_yaml archived, serde_yml UNSOUND |
| HTTP | reqwest 0.12 (rustls-tls, optional) | Async HTTP client |
| Error handling | thiserror 2 | Derive macro for error types |
| Async traits | async-trait 0.1 | Required for dyn-dispatched async traits |
| Ordered maps | indexmap 2 (serde feature) | Module list merge-by-ID preserves order |
| Hashing | sha2 0.10 | Cache key hashing (disk cache) |
| UUID | uuid 1 (v4 feature) | Session ID generation |
| Time | chrono 0.4 (serde feature) | Timestamps for SourceStatus |
| Glob | glob 0.3 | File pattern matching |
| Regex | regex 1 | URI parsing, mention parsing |
| Filesystem | dirs 6 | Platform home directory |
| Zip | zip 2 (optional) | Zip source handler |
| Tracing | tracing 0.1 + tracing-subscriber 0.3 | Structured logging |
| Testing | mockall 0.13, tempfile 3, assert_matches 1, tokio-test 0.4 | Test infrastructure |
| Python interop | pyo3 (optional feature) | PyO3 from day 1 as optional |

### Crate Warnings

- **DO NOT USE `serde_yaml`** -- archived by dtolnay March 2024
- **DO NOT USE `serde_yml`** -- UNSOUND (RUSTSEC-2025-0068), causes segfaults
- **`serde_yaml_ng::Value` + `#[serde(flatten)]`** -- known silent data loss bugs. Test with real YAML from Python repo early. If data loss occurs, switch to manual deserialization.

## 3. Module Map

### Directory Structure

```
src/
  lib.rs                    # pub mod declarations + 61 re-exports
  error.rs                  # BundleError enum (5 variants + Io, Yaml, Http, Git)
  runtime.rs                # AmplifierRuntime trait boundary (7 traits)
  serialization.rs          # sanitize_for_json, sanitize_message [SYNC]
  tracing_utils.rs          # generate_sub_session_id [SYNC]
  bundle/
    mod.rs                  # Bundle struct, from_dict, to_dict
    compose.rs              # compose() with 5 merge strategies [SYNC]
    mount.rs                # MountPlan, section types
    prepared.rs             # PreparedBundle (session lifecycle) [ASYNC]
    module_resolver.rs      # BundleModuleResolver, BundleModuleSource
    prompt.rs               # System prompt factory logic [ASYNC]
    validator.rs            # BundleValidator, validate_bundle [SYNC]
  registry/
    mod.rs                  # BundleRegistry [ASYNC]
    persistence.rs          # JSON serialization
    includes.rs             # Include parsing, cycle detection
  sources/
    mod.rs                  # SourceHandler trait, SourceStatus, protocols
    resolver.rs             # SimpleSourceResolver
    file.rs                 # FileSourceHandler [ASYNC]
    git.rs                  # GitSourceHandler [ASYNC]
    http.rs                 # HttpSourceHandler [ASYNC]
    zip.rs                  # ZipSourceHandler [ASYNC]
  mentions/
    mod.rs                  # MentionResolver trait [ASYNC]
    models.rs               # ContextFile, MentionResult [SYNC]
    parser.rs               # parse_mentions [SYNC]
    resolver.rs             # BaseMentionResolver [ASYNC]
    loader.rs               # load_mentions pipeline [ASYNC]
    dedup.rs                # ContentDeduplicator [SYNC]
    utils.rs                # format_directory_listing [SYNC]
  io/
    mod.rs                  # Module re-exports
    yaml.rs                 # read_yaml, write_yaml [ASYNC]
    frontmatter.rs          # parse_frontmatter [SYNC]
    files.rs                # read_with_retry, write_with_retry, write_with_backup [ASYNC]
  dicts/
    mod.rs                  # Module re-exports
    merge.rs                # deep_merge, merge_module_lists [SYNC]
    nested.rs               # get_nested, set_nested [SYNC]
  paths/
    mod.rs                  # Module re-exports
    uri.rs                  # ParsedURI, parse_uri, ResolvedSource, get_amplifier_home [SYNC]
    normalize.rs            # construct_agent_path, construct_context_path [SYNC]
    discovery.rs            # find_files, find_bundle_root [SYNC]
  cache/
    mod.rs                  # CacheProvider trait
    memory.rs               # SimpleCache [SYNC]
    disk.rs                 # DiskCache [SYNC]
  session/
    mod.rs                  # Module re-exports
    capabilities.rs         # get_working_dir, set_working_dir [SYNC]
    events.rs               # Session event JSONL I/O [SYNC]
    fork.rs                 # Session forking [SYNC]
    slice.rs                # Message list manipulation [SYNC]
  spawn/
    mod.rs                  # ProviderPreference, apply_provider_preferences [SYNC]
    glob.rs                 # is_glob_pattern, resolve_model_pattern [SYNC]
  modules/
    mod.rs                  # Module activator [ASYNC]
    state.rs                # ModuleInstallState
  updates/
    mod.rs                  # check_bundle_status, update_bundle [ASYNC]
```

### Python to Rust File Mapping (Complete)

| Python File | Rust File | LOC |
|-------------|-----------|-----|
| `__init__.py` (61 `__all__` exports) | `lib.rs` | 182 |
| `exceptions.py` | `error.rs` | 21 |
| `bundle.py` | `bundle/{mod,compose,mount,prepared,module_resolver,prompt}.rs` | 1,289 |
| `validator.py` | `bundle/validator.rs` | 295 |
| `registry.py` | `registry/{mod,persistence,includes}.rs` | 1,223 |
| `serialization.py` | `serialization.rs` | 139 |
| `tracing.py` | `tracing_utils.rs` | 105 |
| `spawn_utils.py` | `spawn/{mod,glob}.rs` | 457 |
| `sources/protocol.py` | `sources/mod.rs` | 175 |
| `sources/resolver.py` | `sources/resolver.rs` | 81 |
| `sources/file.py` | `sources/file.rs` | 138 |
| `sources/git.py` | `sources/git.rs` | 326 |
| `sources/http.py` | `sources/http.rs` | 78 |
| `sources/zip.py` | `sources/zip.rs` | 128 |
| `mentions/protocol.py` | `mentions/mod.rs` | 25 |
| `mentions/models.py` | `mentions/models.rs` | 36 |
| `mentions/parser.py` | `mentions/parser.rs` | 68 |
| `mentions/resolver.py` | `mentions/resolver.rs` | 86 |
| `mentions/loader.py` | `mentions/loader.rs` | 199 |
| `mentions/deduplicator.py` | `mentions/dedup.rs` | 85 |
| `mentions/utils.py` | `mentions/utils.rs` | 46 |
| `io/yaml.py` | `io/yaml.rs` | 45 |
| `io/frontmatter.py` | `io/frontmatter.rs` | 38 |
| `io/files.py` | `io/files.rs` | 202 |
| `dicts/merge.py` | `dicts/merge.rs` | 87 |
| `dicts/navigation.py` | `dicts/nested.rs` | 68 |
| `paths/resolution.py` | `paths/uri.rs` | 257 |
| `paths/construction.py` | `paths/normalize.rs` | 53 |
| `paths/discovery.py` | `paths/discovery.rs` | 56 |
| `cache/protocol.py` | `cache/mod.rs` | 41 |
| `cache/simple.py` | `cache/memory.rs` | 50 |
| `cache/disk.py` | `cache/disk.rs` | 121 |
| `session/capabilities.py` | `session/capabilities.rs` | 81 |
| `session/events.py` | `session/events.rs` | 331 |
| `session/fork.py` | `session/fork.rs` | 514 |
| `session/slice.py` | `session/slice.rs` | 331 |
| `modules/activator.py` | `modules/mod.rs` | 282 |
| `modules/install_state.py` | `modules/state.rs` | 193 |
| `updates/__init__.py` | `updates/mod.rs` | 275 |

**Totals:** 48 Python files -> ~42 Rust files. 8,425 Python LOC. 235 Python tests across 13 test files.

## 4. Error Handling

### Error Type Hierarchy

```rust
#[derive(Debug, thiserror::Error)]
pub enum BundleError {
    #[error("bundle not found: {uri}")]
    NotFound { uri: String },

    #[error("failed to load bundle: {reason}")]
    LoadError {
        reason: String,
        #[source] source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("validation failed: {0}")]
    ValidationError(ValidationResult),

    #[error("dependency error: {0}")]
    DependencyError(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Yaml(#[from] serde_yaml_ng::Error),

    #[error(transparent)]
    Http(#[from] reqwest::Error),

    #[error("git error: {0}")]
    Git(String),
}
```

### Error Matching Rules

Python code matches on specific exception types. Rust code MUST match on specific variants:

```rust
// Registry cycle detection:
match result {
    Err(BundleError::NotFound { .. }) => { /* warn and skip */ },
    Err(BundleError::DependencyError(msg)) => { /* log circular dep */ },
    Err(e) => return Err(e),
}
```

**Rule:** Never use a catch-all `BundleError` match where the Python code distinguishes between `BundleNotFoundError` and `BundleDependencyError`.

### Result Type Alias

```rust
pub type Result<T> = std::result::Result<T, BundleError>;
```

## 5. Async/Sync Boundary

**Rule:** 60% of modules are pure sync. Do NOT make sync modules async. Do NOT add tokio as a dependency for pure computation.

| Module | Sync/Async | Rationale |
|--------|-----------|-----------|
| `dicts/` | **Sync** | Pure computation |
| `paths/` | **Sync** | Path manipulation only |
| `cache/memory` | **Sync** | HashMap ops |
| `cache/disk` | **Sync** | `std::fs` (small files, no benefit from async) |
| `mentions/parser` | **Sync** | Regex extraction |
| `mentions/models` | **Sync** | Data structs |
| `mentions/dedup` | **Sync** | Deduplication logic |
| `mentions/resolver` | **Sync** | Path resolution (base impl) |
| `mentions/utils` | **Sync** | String formatting |
| `session/slice` | **Sync** | Message list manipulation |
| `session/events` | **Sync** | Line-based JSONL I/O |
| `session/capabilities` | **Sync** | Coordinator capability access |
| `session/fork` | **Sync** | Session state manipulation |
| `serialization` | **Sync** | String sanitization |
| `tracing_utils` | **Sync** | UUID generation |
| `bundle/validator` | **Sync** | Rule evaluation |
| `bundle/compose` | **Sync** | Pure dict manipulation |
| `spawn/` | **Mostly sync** | Only `apply_provider_preferences_with_resolution` is async |
| `io/files` | **Async** | Retry with sleep |
| `io/yaml` | **Async** | Wraps async file I/O |
| `io/frontmatter` | **Sync** | String parsing |
| `mentions/loader` | **Async** | File I/O pipeline |
| `sources/*` | **Async** | Network, subprocess |
| `bundle/prepared` | **Async** | Session creation, spawn |
| `bundle/prompt` | **Async** | Async closure for prompt factory |
| `registry/` | **Async** | Network loading, parallel compose |
| `modules/` | **Async** | Subprocess install |
| `updates/` | **Async** | Network status checks |

**Translation rule for Python `async def` in sync modules:** If a Python function is `async def` but does no actual I/O (like `paths/discovery.py::find_files` which just calls `Path.glob`), make it sync in Rust.

## 6. AmplifierRuntime Trait Boundary

This is the interface between amplifier-foundation and amplifier-core. All 14 interaction points from `bundle.py` are captured here. Implementation uses mocks (mockall) until real integration.

```rust
// src/runtime.rs

#[async_trait]
pub trait AmplifierRuntime: Send + Sync {
    async fn create_session(&self, opts: SessionOptions) -> Result<Box<dyn AmplifierSession>>;
}

pub struct SessionOptions {
    pub mount_plan: serde_yaml_ng::Value,
    pub session_id: Option<String>,
    pub parent_id: Option<String>,
    pub approval_system: Option<Box<dyn ApprovalSystem>>,
    pub display_system: Option<Box<dyn DisplaySystem>>,
    pub is_resumed: bool,
}

#[async_trait]
pub trait AmplifierSession: Send + Sync {
    fn session_id(&self) -> &str;
    fn coordinator(&self) -> &dyn Coordinator;
    fn coordinator_mut(&mut self) -> &mut dyn Coordinator;
    async fn initialize(&mut self) -> Result<()>;
    async fn execute(&mut self, instruction: &str) -> Result<String>;
    async fn cleanup(&mut self) -> Result<()>;
}

pub trait Coordinator: Send + Sync {
    fn mount(&mut self, name: &str, component: Box<dyn std::any::Any + Send + Sync>);
    fn get(&self, name: &str) -> Option<&(dyn std::any::Any + Send + Sync)>;
    fn register_capability(&mut self, key: &str, value: serde_json::Value);
    fn get_capability(&self, key: &str) -> Option<&serde_json::Value>;
    fn approval_system(&self) -> Option<&dyn ApprovalSystem>;
    fn display_system(&self) -> Option<&dyn DisplaySystem>;
    fn hooks(&self) -> &dyn HookRegistry;
    fn hooks_mut(&mut self) -> &mut dyn HookRegistry;
}

pub trait HookRegistry: Send + Sync {
    fn register(&mut self, event: &str, handler: Box<dyn HookHandler>, priority: i32, name: &str);
}

pub trait ContextManager: Send + Sync {
    fn set_system_prompt_factory(&mut self, factory: Box<dyn SystemPromptFactory>);
    fn set_messages(&mut self, messages: Vec<serde_json::Value>);
    fn add_message(&mut self, message: serde_json::Value);
}

pub trait ApprovalSystem: Send + Sync {}
pub trait DisplaySystem: Send + Sync {}
pub trait HookHandler: Send + Sync {}

pub trait SystemPromptFactory: Send + Sync {
    fn create(&self) -> futures::future::BoxFuture<'_, String>;
}
```

**Mock strategy:** Use `mockall` for `AmplifierRuntime`, `AmplifierSession`, and `Coordinator`. Marker traits (`ApprovalSystem`, `DisplaySystem`, `HookHandler`) use simple empty struct implementations for testing.

## 7. Bundle Composition (5 Strategies)

Bundle composition uses 5 DISTINCT merge strategies. This is NOT a single deep_merge.

| Strategy | Applied To | Behavior |
|----------|-----------|----------|
| 1. Deep merge | `session`, `spawn` | Recursive dict merge (child overrides parent at leaf level) |
| 2. Merge by module ID | `providers`, `tools`, `hooks` | Match by `module` key, deep merge matching entries, append new |
| 3. Dict update | `agents` | Later wins by key (agent name) |
| 4. Accumulate with namespace | `context` | Merge contexts, namespace if overlay has a name |
| 5. Later replaces entirely | `instruction`, `base_path` | Overlay value replaces base value completely |

Additional: `source_base_paths` uses first-write-wins. `pending_context` uses accumulate (merge maps). `extra` uses deep merge.

**Rule:** Every composition test must verify the correct strategy is applied to the correct field.

## 8. Data Types — The `Value` Question

Python uses `dict[str, Any]` extensively. In Rust, the equivalent is `serde_yaml_ng::Value` for dynamic YAML data and `serde_json::Value` for JSON data.

**Rules:**
1. Use concrete structs with named fields where the schema is known (e.g., `Bundle`, `ParsedURI`, `SourceStatus`)
2. Use `serde_yaml_ng::Value` for truly dynamic data (e.g., `session` config, `spawn` config, module configs)
3. Use `serde_json::Value` for JSON-specific data (e.g., capabilities, messages)
4. Use `indexmap::IndexMap<String, V>` where insertion order matters (e.g., module lists after merge)
5. Dynamic attribute injection in Python (`bundle._source_uri = uri  # type: ignore`) becomes a proper `Option<String>` field in Rust

## 9. Testing Strategy

### Test Porting (Wave 0)

All 235 Python tests are ported as `#[ignore]` Rust tests in Wave 0. They serve as the behavioral specification.

| Test File | Tests | Wave |
|-----------|-------|------|
| test_dicts.py | 18 | 1 |
| test_paths.py | 15 | 1 |
| test_cache.py | 12 | 1 |
| test_serialization.py | 16 | 1 |
| test_tracing.py | 9 | 1 |
| test_spawn_utils.py | 17 | 1 |
| test_io_files.py | 6 | 2 |
| test_sources.py | 11 | 2 |
| test_mentions.py | 21 | 2 |
| test_session.py | 53 | 2 |
| test_bundle.py | 26 | 3 |
| test_registry.py | 13 | 3 |
| test_validator.py | 18 | 3 |
| **Total** | **235** | |

### Test Rules

1. **1:1 port:** Every Python test becomes a Rust test with equivalent name and assertions
2. **Splitting is fine, merging is NOT:** A Python test can become multiple Rust tests. Never merge two Python tests into one.
3. **`#[ignore = "Wave N"]`** in Wave 0. Un-ignore when the module is implemented.
4. **`cargo test`** runs passing tests. **`cargo test -- --ignored`** shows what's unimplemented.
5. **Both `cargo test` AND `cargo check` must pass** after every feature.

### Test File Organization

```
tests/
  test_dicts.rs
  test_paths.rs
  test_cache.rs
  test_serialization.rs
  test_tracing.rs
  test_spawn.rs
  test_io_files.rs
  test_sources.rs
  test_mentions.rs
  test_session.rs
  test_bundle.rs
  test_registry.rs
  test_validator.rs
```

Integration tests go in `tests/`. Unit tests can be `#[cfg(test)]` modules within source files where appropriate.

## 10. Wave Plan

```
WAVE 0 -- Scaffold + Test Porting
  Features F-001 through F-005
  Output: Compiling skeleton + full ignored test suite
  Gate: cargo check passes, cargo test reports 0 pass / 235 ignored

WAVE 1 -- Leaf modules (zero internal deps) [ALL SYNC]
  dicts/       (merge.rs, nested.rs)           — 18 tests
  paths/       (uri.rs, normalize.rs, discovery.rs)  — 15 tests
  cache/       (memory.rs, disk.rs)            — 12 tests
  serialization.rs                              — 16 tests
  tracing_utils.rs                              — 9 tests
  spawn/       (mod.rs, glob.rs)               — 17 tests
  Gate: un-ignore Wave 1 tests, all 87 must pass

WAVE 2 -- Mid-tier (depend on Wave 1) [MIXED]
  io/          (yaml.rs, frontmatter.rs, files.rs)    [ASYNC]   — 6 tests
  sources/     (all 5 handlers + resolver)             [ASYNC]   — 11 tests
  session/     (capabilities, events, fork, slice)     [SYNC]    — 53 tests
  mentions/    (models, parser, resolver, dedup,
               loader, utils)                          [MIXED]   — 21 tests
  Gate: un-ignore Wave 2 tests, all 91 must pass

WAVE 3 -- Core (the real migration — 2,641 lines) [MOSTLY ASYNC]
  bundle/mod.rs + compose.rs                    [SYNC]   — partial
  bundle/validator.rs                            [SYNC]   — 18 tests
  bundle/prepared.rs + module_resolver.rs + prompt.rs [ASYNC]
  registry/    (mod.rs, persistence.rs, includes.rs)  [ASYNC] — 13 tests
  modules/     (activator, state)                [ASYNC]
  updates/     (mod.rs)                          [ASYNC]
  Gate: un-ignore Wave 3 tests, all 57 must pass — 26 bundle + 13 registry + 18 validator

WAVE 4 -- Integration surface
  lib.rs re-exports (61 pub use statements)
  examples/ (3 example binaries)

WAVE 5 -- Integration + Polish
  Integration tests (load real .yaml/.md bundles)
  Roundtrip tests
  cargo clippy --all-targets clean
  cargo fmt --check clean
```

## 11. Frontmatter Parser Edge Cases

The frontmatter parser (`io/frontmatter.rs`) MUST handle all 5 edge cases:

1. **Windows line endings (`\r\n`)** -- Normalize to `\n` before parsing
2. **Empty frontmatter** (`---\n---`) -- Returns empty Mapping, not None
3. **No trailing newline** -- Body can end without `\n`
4. **Multiple `---`** -- Only first pair is frontmatter delimiter
5. **Trailing whitespace after delimiters** -- `---  \n` is a valid delimiter

## 12. Feature Flag Design

```toml
[features]
default = ["git", "http-sources", "zip-sources"]
git = []                          # GitSourceHandler
http-sources = ["reqwest"]        # HttpSourceHandler
zip-sources = ["dep:zip"]         # ZipSourceHandler
pyo3-bindings = ["dep:pyo3"]      # Python interop (opt-in)
```

**Rule:** Core bundle composition, parsing, and caching work with zero features enabled. Source handlers are opt-in.

## 13. PyO3 Strategy

PyO3 is set up from day 1 as an optional feature but the actual `#[pyclass]`/`#[pyfunction]` annotations are deferred to Wave 4.

**Cargo.toml setup:**
```toml
[lib]
crate-type = ["cdylib", "rlib"]  # cdylib for Python, rlib for Rust

[dependencies]
pyo3 = { version = "0.24", features = ["extension-module"], optional = true }
```

**Types that will get `#[pyclass]`** (deferred, not implemented in early waves):
- `Bundle`, `ParsedURI`, `ResolvedSource`, `SourceStatus`
- `BundleError` (as Python exception hierarchy)
- `BundleRegistry`, `BundleValidator`, `ValidationResult`
- `SimpleCache`, `DiskCache`

## 14. Key Translation Patterns

| Python Pattern | Rust Equivalent |
|---------------|----------------|
| `dict[str, Any]` | `serde_yaml_ng::Value` or concrete struct |
| `Protocol` class | `trait` |
| `@dataclass` | `#[derive(Debug, Clone)] struct` with builder or `Default` |
| `Optional[T]` | `Option<T>` |
| `list[T]` | `Vec<T>` |
| `dict[str, T]` | `HashMap<String, T>` or `IndexMap<String, T>` |
| `async def` (no actual I/O) | sync `fn` |
| `isinstance(x, dict)` | `value.is_mapping()` for Value, pattern match for enums |
| `raise BundleNotFoundError(msg)` | `return Err(BundleError::NotFound { uri: msg.into() })` |
| `try/except SpecificError` | `match result { Err(BundleError::Variant { .. }) => ... }` |
| `logging.getLogger(__name__)` | `tracing::info!()` / `tracing::warn!()` |
| `copy.deepcopy(x)` | `.clone()` |
| Dynamic attribute (`obj._field = val`) | Proper `Option<T>` field on struct |

## 15. Naming Conventions

| Python | Rust |
|--------|------|
| `snake_case` functions | `snake_case` functions |
| `CamelCase` classes | `CamelCase` structs/enums |
| `SCREAMING_CASE` constants | `SCREAMING_CASE` constants |
| `dicts/navigation.py` | `dicts/nested.rs` |
| `paths/construction.py` | `paths/normalize.rs` |
| `cache/simple.py` | `cache/memory.rs` |
| `tracing.py` | `tracing_utils.rs` (avoid name clash with `tracing` crate) |
| `spawn_utils.py` | `spawn/mod.rs` + `spawn/glob.rs` |

## 16. Structural Health Rules

- **Module > 10,000 LOC:** Hard stop. Refactor before continuing.
- **File > 1,000 lines:** Warning. Flag for decomposition.
- **These are checked before every working session.**

## 17. Risk Register

| Risk | Severity | Mitigation |
|------|----------|------------|
| `PreparedBundle` async closure (`Box<dyn Fn() -> BoxFuture<'_, String>>`) | High | Spike in early Wave 3. If lifetimes don't work, use `Arc<dyn Fn()>` + `tokio::spawn` pattern. |
| `serde_yaml_ng::Value` + `#[serde(flatten)]` | Medium | Test with real YAML from Python repo in Wave 1. Switch to manual deser if data loss. |
| AmplifierRuntime mock fidelity | Medium | Keep mocks minimal. 14-point trait from source analysis. Flag divergence as blocker. |
| `bundle.py` + `registry.py` sequential bottleneck | Medium | Wave 3 serializes these. Don't parallelize what can't be parallelized. |
| PyO3 surface underspecified | Low | Defer to Wave 4. Pure Rust lib works first. |
