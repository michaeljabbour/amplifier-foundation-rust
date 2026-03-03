# Rust Migration Blueprint: amplifier-foundation (Original)

## Source to Target Module Mapping

| Python (`amplifier_foundation/`) | Rust (`src/`) |
|---|---|
| `__init__.py` (82 exports) | `lib.rs` (pub mod + re-exports) |
| `exceptions.py` | `error.rs` |
| `bundle.py` (1,289 LOC) | `bundle/mod.rs`, `compose.rs`, `mount.rs` |
| `registry.py` (1,223 LOC) | `registry/mod.rs`, `persistence.rs` |
| `validator.py` | `bundle/validator.rs` |
| `sources/__init__.py` | `sources/mod.rs` |
| `sources/resolver.py` | `sources/resolver.rs` |
| `sources/file.py` | `sources/file.rs` |
| `sources/git.py` | `sources/git.rs` |
| `sources/http.py` | `sources/http.rs` |
| `sources/zip.py` | `sources/zip.rs` |
| `mentions/__init__.py` | `mentions/mod.rs` |
| `mentions/parser.py` | `mentions/parser.rs` |
| `mentions/resolver.py` | `mentions/resolver.rs` |
| `mentions/dedup.py` | `mentions/dedup.rs` |
| `io/__init__.py` | `io/mod.rs` |
| `io/yaml.py` | `io/yaml.rs` |
| `io/frontmatter.py` | `io/frontmatter.rs` |
| `io/files.py` | `io/files.rs` |
| `dicts/__init__.py` | `dicts/mod.rs` |
| `dicts/merge.py` | `dicts/merge.rs` |
| `dicts/nested.py` | `dicts/nested.rs` |
| `paths/__init__.py` | `paths/mod.rs` |
| `paths/uri.py` | `paths/uri.rs` |
| `paths/normalize.py` | `paths/normalize.rs` |
| `paths/discovery.py` | `paths/discovery.rs` |
| `cache/__init__.py` | `cache/mod.rs` |
| `cache/memory.py` | `cache/memory.rs` |
| `cache/disk.py` | `cache/disk.rs` |
| `session/__init__.py` | `session/mod.rs` |
| `session/capabilities.py` | `session/capabilities.rs` |
| `session/events.py` | `session/events.rs` |
| `session/fork.py` | `session/fork.rs` |
| `session/slice.py` | `session/slice.rs` |
| `spawn_utils.py` | `spawn/mod.rs` |
| `modules/activator.py` | `modules/mod.rs` |
| `modules/install_state.py` | `modules/state.rs` |
| `updates/__init__.py` | `updates/mod.rs` |

## Target Folder Structure

```
amplifier-foundation-rs/
├── Cargo.toml
├── src/
│   ├── lib.rs                    # Public API, re-exports
│   ├── error.rs                  # BundleError enum (thiserror)
│   │
│   ├── bundle/
│   │   ├── mod.rs                # Bundle struct, load_bundle()
│   │   ├── compose.rs            # Bundle::compose(), merge logic
│   │   ├── mount.rs              # MountPlan, section types
│   │   └── validator.rs          # BundleValidator, validate_bundle()
│   │
│   ├── registry/
│   │   ├── mod.rs                # BundleRegistry, in-memory state
│   │   └── persistence.rs        # JSON serialization to ~/.amplifier/
│   │
│   ├── sources/
│   │   ├── mod.rs                # SourceHandler trait, ResolvedSource
│   │   ├── resolver.rs           # SimpleSourceResolver, handler chain
│   │   ├── file.rs               # FileSourceHandler
│   │   ├── git.rs                # GitSourceHandler (tokio::process)
│   │   ├── http.rs               # HttpSourceHandler (reqwest)
│   │   └── zip.rs                # ZipSourceHandler
│   │
│   ├── mentions/
│   │   ├── mod.rs                # parse_mentions(), load_mentions()
│   │   ├── parser.rs             # @namespace:path extraction, code block skip
│   │   ├── resolver.rs           # MentionResolver trait + base impl
│   │   └── dedup.rs              # ContentDeduplicator
│   │
│   ├── io/
│   │   ├── mod.rs
│   │   ├── yaml.rs               # read_yaml(), write_yaml() via serde_yaml
│   │   ├── frontmatter.rs        # parse_frontmatter() -- custom parser
│   │   └── files.rs              # read_with_retry(), write_with_backup()
│   │
│   ├── dicts/
│   │   ├── mod.rs
│   │   ├── merge.rs              # deep_merge() over serde_yaml::Value
│   │   └── nested.rs             # get_nested(), set_nested()
│   │
│   ├── paths/
│   │   ├── mod.rs
│   │   ├── uri.rs                # ParsedUri, parse_uri()
│   │   ├── normalize.rs          # normalize_path(), construct_*_path()
│   │   └── discovery.rs          # find_files(), find_bundle_root()
│   │
│   ├── cache/
│   │   ├── mod.rs                # CacheProvider trait
│   │   ├── memory.rs             # SimpleCache (HashMap + optional TTL)
│   │   └── disk.rs               # DiskCache (fs-backed)
│   │
│   ├── session/
│   │   ├── mod.rs
│   │   ├── capabilities.rs       # get_working_dir(), set_working_dir()
│   │   ├── events.rs             # events.jsonl read/write
│   │   ├── fork.rs               # Session forking
│   │   └── slice.rs              # Transcript slicing
│   │
│   ├── spawn/
│   │   ├── mod.rs                # ProviderPreference, resolve_model_pattern()
│   │   └── glob.rs               # is_glob_pattern(), glob matching
│   │
│   ├── modules/
│   │   ├── mod.rs                # ModuleActivator
│   │   └── state.rs              # InstallStateManager
│   │
│   └── updates/
│       └── mod.rs                # check_bundle_status(), update_bundle()
│
├── tests/
│   ├── bundle_test.rs
│   ├── registry_test.rs
│   ├── session_test.rs
│   ├── spawn_utils_test.rs
│   ├── validator_test.rs
│   ├── sources_test.rs
│   ├── mentions_test.rs
│   ├── dicts_test.rs
│   ├── cache_test.rs
│   ├── paths_test.rs
│   ├── io_test.rs
│   └── integration/
│       ├── compose_test.rs
│       └── load_and_resolve_test.rs
│
├── examples/
│   ├── load_bundle.rs
│   ├── compose_bundles.rs
│   └── resolve_mentions.rs
│
└── benches/
    ├── compose_bench.rs
    └── mention_parse_bench.rs
```

## Crate Dependencies

```toml
[dependencies]
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"
reqwest = { version = "0.12", features = ["rustls-tls"] }
thiserror = "2"
async-trait = "0.1"
glob = "0.3"
regex = "1"
dirs = "6"                    # ~/.amplifier/ resolution
sha2 = "0.10"                 # cache key hashing
zip = "2"                     # zip archive extraction
tracing = "0.1"               # structured logging (replaces print/logging)
tracing-subscriber = "0.3"
uuid = { version = "1", features = ["v4"] }  # session IDs
chrono = { version = "0.4", features = ["serde"] }

[dev-dependencies]
tokio-test = "0.4"
tempfile = "3"
mockall = "0.13"              # trait mocking
assert_matches = "1"
```

## Key Design Decisions

### 1. Dynamic YAML values -- `serde_yaml::Value` as the escape hatch

Python dicts are untyped. Bundle sections mix known fields with arbitrary user config. The Rust approach:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bundle {
    pub name: String,
    pub version: Option<String>,
    pub session: Option<serde_yaml::Value>,     // arbitrary
    pub providers: Option<Vec<ProviderConfig>>,  // typed where possible
    pub tools: Option<Vec<ToolConfig>>,          // typed where possible
    pub hooks: Option<serde_yaml::Value>,        // arbitrary
    pub spawn: Option<SpawnConfig>,              // typed
    pub agents: Option<Vec<AgentRef>>,           // typed
    pub context: Option<ContextConfig>,          // typed
    pub instruction: Option<String>,             // plain text
    #[serde(flatten)]
    pub extra: HashMap<String, serde_yaml::Value>,  // catch-all
}
```

Typed where structure is known, Value where it's truly dynamic, `#[serde(flatten)]` for forward compatibility.

### 2. Protocol to Trait translation

```rust
// Python Protocol -> Rust async trait
#[async_trait]
pub trait SourceHandler: Send + Sync {
    fn scheme(&self) -> &str;
    async fn resolve(&self, uri: &ParsedUri, cache_dir: &Path) -> Result<ResolvedSource>;
    fn supports(&self, uri: &ParsedUri) -> bool;
}

#[async_trait]
pub trait SourceHandlerWithStatus: SourceHandler {
    async fn check_status(&self, uri: &ParsedUri) -> Result<UpdateStatus>;
}

#[async_trait]
pub trait MentionResolver: Send + Sync {
    async fn resolve(&self, mention: &str) -> Result<Option<String>>;
}

pub trait CacheProvider: Send + Sync {
    fn get(&self, key: &str) -> Option<String>;
    fn set(&mut self, key: &str, value: String, ttl: Option<Duration>);
    fn invalidate(&self, key: &str);
}
```

### 3. Deep merge over `serde_yaml::Value`

```rust
pub fn deep_merge(base: &Value, overlay: &Value) -> Value {
    match (base, overlay) {
        (Value::Mapping(b), Value::Mapping(o)) => {
            let mut merged = b.clone();
            for (k, v) in o {
                let entry = merged.get(&k).cloned().unwrap_or(Value::Null);
                merged.insert(k.clone(), deep_merge(&entry, v));
            }
            Value::Mapping(merged)
        }
        (_, overlay) => overlay.clone(),
    }
}
```

### 4. Error type design

```rust
#[derive(Debug, thiserror::Error)]
pub enum BundleError {
    #[error("bundle not found: {uri}")]
    NotFound { uri: String },

    #[error("failed to load bundle: {reason}")]
    LoadError { reason: String, #[source] source: Option<Box<dyn std::error::Error + Send + Sync>> },

    #[error("validation failed: {violations:?}")]
    ValidationError { violations: Vec<String> },

    #[error("dependency error: {0}")]
    DependencyError(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("YAML parse error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("git error: {0}")]
    Git(String),
}

pub type Result<T> = std::result::Result<T, BundleError>;
```

### 5. Frontmatter parser -- write from scratch

No mature Rust crate for YAML frontmatter. It's ~50 lines:

```rust
pub fn parse_frontmatter(content: &str) -> Result<(Option<Value>, &str)> {
    if !content.starts_with("---\n") {
        return Ok((None, content));
    }
    let end = content[4..].find("\n---\n")
        .ok_or_else(|| BundleError::LoadError { reason: "unclosed frontmatter".into(), source: None })?;
    let yaml_str = &content[4..4 + end];
    let frontmatter: Value = serde_yaml::from_str(yaml_str)?;
    let body = &content[4 + end + 5..];
    Ok((Some(frontmatter), body))
}
```

## Risks and Mitigations

| Risk | Severity | Mitigation |
|---|---|---|
| amplifier-core dependency -- Python-only, no Rust port exists | Critical | Define a Rust trait `AmplifierCore` that mirrors the interface used. Implement a stub/mock first. Enables compilation and testing without the real dep. Decide later: FFI via PyO3, full port, or gRPC bridge. |
| Lifetime complexity in `Bundle::compose()` -- composing borrowed data from multiple bundles | High | Clone aggressively in v1. Bundle is Clone. Don't try to optimize with references until it works. Profile later. |
| async-trait + dyn dispatch -- trait objects with async methods | Medium | Use async-trait crate everywhere. Accept the `Box<dyn Future>` allocation. Upgrade to native async traits when stabilized. |
| Dynamic dict access patterns -- `get_nested(d, "a.b.c")` used heavily | Medium | Implement via `serde_yaml::Value` traversal. Accept the ergonomic cost. Consider a `value_path!` macro for common patterns. |
| Module list merging by ID -- Python does identity-based list merging | Medium | Implement `MergeById` trait. Items need an `id()` method. Use `IndexMap` to preserve order. |
| Cloud sync retry logic -- OneDrive/Dropbox delay handling | Low | Direct port with `tokio::time::sleep`. Same pattern, cleaner with Rust's Result propagation. |
| Glob pattern matching for model resolution -- `claude-haiku-*` | Low | Use glob crate's `Pattern::matches()`. Nearly identical behavior. |
| Test mock ergonomics -- Python mocking is trivial, Rust is not | Medium | Use mockall crate. Define all extensibility points as traits from day 1. Design for testability. |
| YAML schema drift -- Rust types may not match all valid YAML inputs | Medium | Use `#[serde(default)]` liberally. `#[serde(flatten)] extra: HashMap<String, Value>` on every struct. Roundtrip tests against real bundle files. |
| Session fork complexity -- events.jsonl slicing with parent lineage | Low | JSONL is line-based, maps to `BufReader::lines()`. Serde for each line. Straightforward. |

## amplifier-core Interface Boundary

The critical unknown. Based on usage in the Python code, the interface surface is approximately:

```rust
/// What amplifier-foundation actually needs from amplifier-core
#[async_trait]
pub trait AmplifierRuntime: Send + Sync {
    async fn create_session(&self, config: SessionConfig) -> Result<Session>;
    async fn prepare_mount_plan(&self, bundle: &Bundle) -> Result<MountPlan>;
}

pub trait Session: Send + Sync {
    fn id(&self) -> &str;
    fn set_capability(&mut self, key: &str, value: serde_json::Value);
    fn get_capability(&self, key: &str) -> Option<&serde_json::Value>;
}
```

Strategy: Define this trait boundary first. Mock it. Port everything else. The runtime integration becomes a separate concern -- could be PyO3 FFI, gRPC, or a future Rust port of amplifier-core.

## Max Parallelism Approach

Principle: Agents work on modules, not files. Each agent owns a module's `mod.rs` + subfiles + tests. No two agents touch the same file.

### Wave Structure

**WAVE 0 -- Scaffold**
- Agent S: `Cargo.toml`, `src/lib.rs` (empty mods), `src/error.rs`
- Output: Compiling skeleton that every other agent imports from

**WAVE 1 -- Leaf modules (zero internal deps, max parallelism)**
- Agent A: `dicts/` (merge.rs, nested.rs)
- Agent B: `paths/` (uri.rs, normalize.rs, discovery.rs)
- Agent C: `cache/` (memory.rs, disk.rs, CacheProvider trait)
- Agent D: `spawn/` (mod.rs, glob.rs)
- All 4 run simultaneously, only depend on error.rs

**WAVE 2 -- Mid-tier (depend on Wave 1 outputs)**
- Agent E: `io/` (yaml.rs, frontmatter.rs, files.rs) -- depends on: paths/, error
- Agent F: `sources/` (all 5 handlers + resolver) -- depends on: paths/, cache/, error
- Agent G: `session/` (capabilities, events, fork, slice) -- depends on: paths/, io/, error
- Agent H: `mentions/` (parser, resolver, dedup) -- depends on: paths/, error
- All 4 run simultaneously

**WAVE 3 -- Core (depends on most prior waves)**
- Agent I: `bundle/` (mod.rs, compose.rs, mount.rs, validator.rs) -- depends on: dicts/, paths/, io/, sources/, mentions/, error
- Agent J: `registry/` (mod.rs, persistence.rs) -- depends on: bundle/, paths/, io/, error
- Agent K: `modules/` (activator, state) -- depends on: sources/, io/, error
- Agent L: `updates/` (mod.rs) -- depends on: sources/, registry/, error
- I runs first, J/K/L can partially overlap

**WAVE 4 -- Integration surface**
- Agent M: `lib.rs` (pub use re-exports, top-level functions) -- depends on: everything
- Agent N: `examples/` (3 example binaries) -- depends on: lib.rs

**WAVE 5 -- Tests (max parallelism again)**
- Agent O: tests for dicts/, paths/, cache/, spawn/
- Agent P: tests for io/, sources/
- Agent Q: tests for mentions/, session/
- Agent R: tests for bundle/, registry/, modules/, updates/
- Agent T: integration tests (load real .yaml bundles from repo)
- All 5 run simultaneously

**WAVE 6 -- Stabilization**
- Agent U: cargo clippy, fix all warnings, cargo fmt
- Agent V: roundtrip tests against actual Python bundle files in repo (agents/, behaviors/, providers/, bundles/ directories)

Peak parallelism: 5 agents (Waves 1 and 5). Total agents: ~20 across 7 waves. Sequential depth: 7 waves.

## Agent Contract Per Module

Each agent receives:

1. The Python source file(s) for their module
2. The `src/error.rs` file (shared error types)
3. The trait/type definitions from modules they depend on (just the public API, not implementation)
4. Explicit instructions:
   - Write idiomatic Rust, not transliterated Python
   - All public items get `///` doc comments matching Python docstrings
   - All `pub fn` gets a unit test in the same file (`#[cfg(test)] mod tests`)
   - Use `Result<T, BundleError>` for all fallible operations
   - Use `tracing::debug!` / `tracing::warn!` instead of print / logging
   - No `unwrap()` in library code -- propagate errors

## Things to Consider

### 1. Feature flags for optional deps

```toml
[features]
default = ["git", "http", "zip"]
git = []           # GitSourceHandler
http = ["reqwest"]  # HttpSourceHandler
zip = ["dep:zip"]   # ZipSourceHandler
```

Users who only need local file bundles skip the network deps entirely.

### 2. WASM target

If WASM is a goal, avoid `tokio::process` (no subprocess in WASM). Gate git/zip handlers behind `#[cfg(not(target_arch = "wasm32"))]`. Use reqwest with wasm feature for HTTP in browser.

### 3. Python interop via PyO3

If the migration is incremental, expose the Rust crate as a Python module:

```toml
[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
pyo3 = { version = "0.22", features = ["extension-module"], optional = true }
```

This lets Python code call Rust bundle loading/composition while the rest stays Python.

### 4. Bundle file compatibility

The YAML/markdown bundle files in `agents/`, `behaviors/`, `providers/`, `bundles/` must load identically. Write roundtrip tests that load every `.yaml` and `bundle.md` in the repo and assert parse success. This is the acceptance criterion.

### 5. Registry JSON compatibility

`~/.amplifier/registry.json` must be readable by both Python and Rust versions during any transition period. Use `#[serde(deny_unknown_fields)]` cautiously -- prefer permissive deserialization with `#[serde(default)]`.

### 6. Async runtime choice

Tokio is the default. But if this crate is used as a library embedded in other runtimes (e.g., a Tauri app using its own async runtime), consider making the runtime pluggable or at minimum don't spawn a runtime internally -- let the caller own it.

### 7. The `compose()` semantics must be exact

Bundle composition has specific override rules (later overrides earlier, module lists merge by ID, context gets namespaced). This is the most semantically dense code. Port the Python tests first as the spec, then implement until they pass.
