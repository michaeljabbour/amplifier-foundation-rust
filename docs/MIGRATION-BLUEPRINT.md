# Rust Migration Blueprint: amplifier-foundation

**Status:** Validated and corrected
**Last validated:** 2025-01-28
**Validated against:** amplifier-foundation Python source (47 files, 8,780 LOC)

---

## Table of Contents

1. [Validation Summary](#validation-summary)
2. [Source to Target Module Mapping](#source-to-target-module-mapping)
3. [Target Folder Structure](#target-folder-structure)
4. [Crate Dependencies](#crate-dependencies)
5. [Key Design Decisions](#key-design-decisions)
6. [AmplifierRuntime Trait Boundary](#amplifier-core-interface-boundary)
7. [Async/Sync Boundary](#asyncsync-boundary)
8. [Bundle Composition — Five Merge Strategies](#bundle-composition--five-merge-strategies)
9. [Error Type Design](#error-type-design)
10. [Frontmatter Parser](#frontmatter-parser)
11. [Wave Plan](#wave-plan)
12. [Agent Contract Per Module](#agent-contract-per-module)
13. [Risks and Mitigations](#risks-and-mitigations)
14. [Feature Flags](#feature-flags)
15. [Additional Considerations](#additional-considerations)
16. [Validation Findings Log](#validation-findings-log)

---

## Validation Summary

This blueprint was validated through three parallel investigations:

1. **Crate ecosystem research** — checked maintenance status of all dependencies
2. **Python source exploration** — surveyed all 47 `.py` files to verify mapping completeness
3. **Antagonistic architecture review** — stress-tested every design decision against actual source

### Corrections Applied

| Issue | Severity | Correction |
|---|---|---|
| `serde_yaml` is archived/deprecated | Critical | Replaced with `serde_yaml_ng = "0.10"` |
| `serde_yml` has RUSTSEC advisory (segfaults) | Critical | Excluded entirely |
| AmplifierRuntime trait covers ~20% of actual usage | Fatal | Expanded to 14 interaction points across 7 traits |
| 5 modules missing from blueprint | Fatal | Added `serialization`, `tracing_utils`, `PreparedBundle`, `mentions/loader`, `mentions/models` |
| Wave plan misjudges bottleneck | Fatal | Restructured: tests in Wave 0, bundle.py/registry.py are the real migration |
| Export count wrong (claimed 82, actual 61) | High | Corrected |
| Composition is 5 strategies, not just deep_merge | High | Documented all 5 strategies with correct Rust mapping |
| 60% of modules are pure sync | Serious | Established explicit async/sync boundary |
| Tests deferred to Wave 5 | Serious | Moved to Wave 0 (tests ARE the spec) |
| `indexmap` missing from dependencies | Medium | Added |
| `git` source handler marked optional | Medium | Corrected: git is mandatory |
| Frontmatter parser ignores edge cases | Medium | Rewritten with 5 edge case handlers |
| LOC counts off (bundle.py +91, registry.py +38) | Low | Corrected |

---

## Source to Target Module Mapping

47 Python files to ~42 Rust source files across 13 modules.

```
Python                              Rust                              Notes
-----                               ----                              -----
__init__.py (61 exports)       ->   lib.rs                            61 pub use re-exports
exceptions.py                  ->   error.rs                          BundleError enum
bundle.py (1,380 LOC)         ->   bundle/mod.rs                     Bundle struct, from_dict
                               ->   bundle/compose.rs                 compose(), 5 merge strategies
                               ->   bundle/mount.rs                   MountPlan, section types
                               ->   bundle/prepared.rs                PreparedBundle (445 LOC)
                               ->   bundle/module_resolver.rs         BundleModuleResolver, BundleModuleSource
                               ->   bundle/prompt.rs                  System prompt factory logic
validator.py (305 LOC)         ->   bundle/validator.rs               BundleValidator, validate_bundle()
serialization.py (139 LOC)    ->   serialization.rs                  sanitize_for_json, sanitize_message
tracing.py (105 LOC)          ->   tracing_utils.rs                  generate_sub_session_id
registry.py (1,261 LOC)       ->   registry/mod.rs                   BundleRegistry
                               ->   registry/persistence.rs           JSON serialization to ~/.amplifier/
                               ->   registry/includes.rs              Include parsing, cycle detection
sources/protocol.py (175 LOC) ->   sources/mod.rs                    SourceHandler trait, ResolvedSource, SourceStatus
sources/resolver.py            ->   sources/resolver.rs               SimpleSourceResolver, handler chain
sources/file.py                ->   sources/file.rs                   FileSourceHandler
sources/git.py (326 LOC)      ->   sources/git.rs                    GitSourceHandler (tokio::process)
sources/http.py                ->   sources/http.rs                   HttpSourceHandler (reqwest)
sources/zip.py                 ->   sources/zip.rs                    ZipSourceHandler
mentions/protocol.py           ->   mentions/mod.rs                   MentionResolver trait
mentions/models.py             ->   mentions/models.rs                ContextFile, MentionResult
mentions/parser.py             ->   mentions/parser.rs                @namespace:path extraction
mentions/resolver.py           ->   mentions/resolver.rs              BaseMentionResolver
mentions/loader.py (199 LOC)  ->   mentions/loader.rs                load_mentions pipeline
mentions/deduplicator.py       ->   mentions/dedup.rs                 ContentDeduplicator
mentions/utils.py              ->   mentions/utils.rs                 format_directory_listing
io/yaml.py                     ->   io/yaml.rs                        read_yaml(), write_yaml()
io/frontmatter.py              ->   io/frontmatter.rs                 parse_frontmatter() -- custom parser
io/files.py (202 LOC)         ->   io/files.rs                       read_with_retry(), write_with_backup()
dicts/merge.py                 ->   dicts/merge.rs                    deep_merge + merge_module_lists
dicts/navigation.py            ->   dicts/nested.rs                   get_nested(), set_nested()
paths/resolution.py (265 LOC) ->   paths/uri.rs                      ParsedURI, parse_uri, ResolvedSource
paths/construction.py          ->   paths/normalize.rs                normalize_path(), construct_*_path()
paths/discovery.py             ->   paths/discovery.rs                find_files(), find_bundle_root()
cache/protocol.py              ->   cache/mod.rs                      CacheProvider trait
cache/simple.py                ->   cache/memory.rs                   SimpleCache (HashMap + optional TTL)
cache/disk.py                  ->   cache/disk.rs                     DiskCache (fs-backed)
session/capabilities.py        ->   session/capabilities.rs           get/set_working_dir
session/events.py (331 LOC)   ->   session/events.rs                 events.jsonl read/write
session/fork.py (514 LOC)     ->   session/fork.rs                   Session forking
session/slice.py (360 LOC)    ->   session/slice.rs                  Transcript slicing
spawn_utils.py (463 LOC)      ->   spawn/mod.rs                      ProviderPreference, resolve_model_pattern
                               ->   spawn/glob.rs                     is_glob_pattern(), glob matching
modules/activator.py (410 LOC)->   modules/mod.rs                    ModuleActivator
modules/install_state.py       ->   modules/state.rs                  InstallStateManager
updates/__init__.py (275 LOC) ->   updates/mod.rs                    check_bundle_status(), update_bundle()
```

---

## Target Folder Structure

```
amplifier-foundation-rs/
|-- Cargo.toml
|-- src/
|   |-- lib.rs                    # Public API, 61 re-exports
|   |-- error.rs                  # BundleError enum (thiserror)
|   |-- serialization.rs          # sanitize_for_json, sanitize_message
|   |-- tracing_utils.rs          # generate_sub_session_id (name avoids clash with tracing crate)
|   |
|   |-- bundle/
|   |   |-- mod.rs                # Bundle struct, from_dict()
|   |   |-- compose.rs            # Bundle::compose(), 5 merge strategies
|   |   |-- mount.rs              # MountPlan, section types
|   |   |-- prepared.rs           # PreparedBundle — session creation, spawn lifecycle
|   |   |-- module_resolver.rs    # BundleModuleResolver, BundleModuleSource
|   |   |-- prompt.rs             # System prompt factory (async closure)
|   |   +-- validator.rs          # BundleValidator, validate_bundle()
|   |
|   |-- registry/
|   |   |-- mod.rs                # BundleRegistry, in-memory state
|   |   |-- persistence.rs        # JSON serialization to ~/.amplifier/
|   |   +-- includes.rs           # Include parsing, cycle detection, parallel loading
|   |
|   |-- sources/
|   |   |-- mod.rs                # SourceHandler trait, ResolvedSource, SourceStatus
|   |   |-- resolver.rs           # SimpleSourceResolver, handler chain
|   |   |-- file.rs               # FileSourceHandler
|   |   |-- git.rs                # GitSourceHandler (tokio::process)
|   |   |-- http.rs               # HttpSourceHandler (reqwest)
|   |   +-- zip.rs                # ZipSourceHandler
|   |
|   |-- mentions/
|   |   |-- mod.rs                # MentionResolver trait, re-exports
|   |   |-- models.rs             # ContextFile, MentionResult
|   |   |-- parser.rs             # @namespace:path extraction, code block skip
|   |   |-- resolver.rs           # BaseMentionResolver
|   |   |-- loader.rs             # load_mentions() async pipeline
|   |   |-- dedup.rs              # ContentDeduplicator
|   |   +-- utils.rs              # format_directory_listing
|   |
|   |-- io/
|   |   |-- mod.rs
|   |   |-- yaml.rs               # read_yaml(), write_yaml() via serde_yaml_ng
|   |   |-- frontmatter.rs        # parse_frontmatter() -- custom parser with edge cases
|   |   +-- files.rs              # read_with_retry(), write_with_backup()
|   |
|   |-- dicts/
|   |   |-- mod.rs
|   |   |-- merge.rs              # deep_merge() + merge_module_lists()
|   |   +-- nested.rs             # get_nested(), set_nested()
|   |
|   |-- paths/
|   |   |-- mod.rs
|   |   |-- uri.rs                # ParsedUri, parse_uri(), ResolvedSource
|   |   |-- normalize.rs          # normalize_path(), construct_*_path()
|   |   +-- discovery.rs          # find_files(), find_bundle_root()
|   |
|   |-- cache/
|   |   |-- mod.rs                # CacheProvider trait
|   |   |-- memory.rs             # SimpleCache (HashMap + optional TTL)
|   |   +-- disk.rs               # DiskCache (fs-backed)
|   |
|   |-- session/
|   |   |-- mod.rs
|   |   |-- capabilities.rs       # get_working_dir(), set_working_dir()
|   |   |-- events.rs             # events.jsonl read/write
|   |   |-- fork.rs               # Session forking
|   |   +-- slice.rs              # Transcript slicing
|   |
|   |-- spawn/
|   |   |-- mod.rs                # ProviderPreference, resolve_model_pattern()
|   |   +-- glob.rs               # is_glob_pattern(), glob matching
|   |
|   |-- modules/
|   |   |-- mod.rs                # ModuleActivator
|   |   +-- state.rs              # InstallStateManager
|   |
|   +-- updates/
|       +-- mod.rs                # check_bundle_status(), update_bundle()
|
|-- tests/
|   |-- bundle_test.rs
|   |-- registry_test.rs
|   |-- session_test.rs
|   |-- spawn_utils_test.rs
|   |-- validator_test.rs
|   |-- sources_test.rs
|   |-- mentions_test.rs
|   |-- dicts_test.rs
|   |-- cache_test.rs
|   |-- paths_test.rs
|   |-- io_test.rs
|   |-- serialization_test.rs
|   |-- tracing_test.rs
|   +-- integration/
|       |-- compose_test.rs
|       +-- load_and_resolve_test.rs
|
|-- examples/
|   |-- load_bundle.rs
|   |-- compose_bundles.rs
|   +-- resolve_mentions.rs
|
+-- benches/
    |-- compose_bench.rs
    +-- mention_parse_bench.rs
```

---

## Crate Dependencies

```toml
[dependencies]
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml_ng = "0.10"             # serde_yaml is archived; serde_yml is UNSOUND
reqwest = { version = "0.12", features = ["rustls-tls"], optional = true }
thiserror = "2"
async-trait = "0.1"                # Still needed for dyn-dispatched async traits
glob = "0.3"
regex = "1"
dirs = "6"                         # ~/.amplifier/ resolution
sha2 = "0.10"                      # Cache key hashing
zip = { version = "2", optional = true }
tracing = "0.1"                    # Structured logging
tracing-subscriber = "0.3"
uuid = { version = "1", features = ["v4"] }
chrono = { version = "0.4", features = ["serde"] }
indexmap = { version = "2", features = ["serde"] }  # Ordered maps for module list merging

[features]
default = ["git", "http-sources", "zip-sources"]
git = []                           # GitSourceHandler — mandatory for registry, gate-able for parse-only
http-sources = ["reqwest"]         # HttpSourceHandler
zip-sources = ["dep:zip"]          # ZipSourceHandler

[dev-dependencies]
tokio-test = "0.4"
tempfile = "3"
mockall = "0.13"                   # Trait mocking
assert_matches = "1"
```

### Dependency Rationale

| Crate | Why |
|---|---|
| `serde_yaml_ng` | `serde_yaml` archived Mar 2024. `serde_yml` has RUSTSEC-2025-0068 (segfaults). `serde_yaml_ng` is the maintained, API-compatible fork. |
| `async-trait` | Rust 1.75+ has native `async fn` in traits BUT only for concrete impls. `dyn` dispatch (needed for `SourceHandler`, `MentionResolver`) still requires `async-trait`. Drop when object-safe async traits stabilize (~2025 H2). |
| `thiserror 2` | Current, no compat issues with 1.x in dep tree. Better `no_std` support. |
| `indexmap` | Needed for `merge_module_lists()` — preserves insertion order during ID-based merging. |

---

## Key Design Decisions

### 1. Dynamic YAML values — serde_yaml_ng::Value as the escape hatch

Python dicts are untyped. Bundle sections mix known fields with arbitrary user config.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bundle {
    pub name: Option<String>,
    pub version: Option<String>,
    pub base_path: Option<PathBuf>,
    pub source_uri: Option<String>,       // Was monkey-patched in Python; proper field now

    // Typed where structure is known
    pub providers: Option<Vec<ProviderConfig>>,
    pub tools: Option<Vec<ToolConfig>>,
    pub spawn: Option<SpawnConfig>,
    pub agents: Option<IndexMap<String, AgentConfig>>,
    pub context: Option<ContextConfig>,
    pub instruction: Option<String>,

    // Value where truly dynamic
    pub session: Option<serde_yaml_ng::Value>,
    pub hooks: Option<Vec<serde_yaml_ng::Value>>,

    // Internal state
    pub pending_context: Option<HashMap<String, String>>,
    pub source_base_paths: Option<HashMap<String, PathBuf>>,

    // Forward compatibility catch-all
    #[serde(flatten)]
    pub extra: HashMap<String, serde_yaml_ng::Value>,
}
```

**Warning:** `#[serde(flatten)]` with `serde_yaml_ng::Value` can cause silent data loss in edge cases. Mitigate with roundtrip tests against every real bundle file in the repo.

### 2. Protocol to Trait translation

Use `async-trait` only for traits that need `dyn` dispatch. Use native `async fn` for concrete implementations.

```rust
// Needs dyn dispatch (multiple handler types) -> async-trait
#[async_trait]
pub trait SourceHandler: Send + Sync {
    fn scheme(&self) -> &str;
    async fn resolve(&self, uri: &ParsedUri, cache_dir: &Path) -> Result<ResolvedSource>;
    fn supports(&self, uri: &ParsedUri) -> bool;
}

#[async_trait]
pub trait SourceHandlerWithStatus: SourceHandler {
    async fn check_status(&self, uri: &ParsedUri) -> Result<SourceStatus>;
}

// Needs dyn dispatch (pluggable resolvers) -> async-trait
#[async_trait]
pub trait MentionResolver: Send + Sync {
    async fn resolve(&self, mention: &str) -> Result<Option<PathBuf>>;
}

// Simple, no dyn dispatch needed -> no async-trait
pub trait CacheProvider: Send + Sync {
    fn get(&self, key: &str) -> Option<String>;
    fn set(&mut self, key: &str, value: String, ttl: Option<Duration>);
    fn invalidate(&self, key: &str);
}
```

### 3. Clone-first for Bundle::compose()

Bundle composition borrows from multiple sources. Don't fight lifetimes in v1.

- `Bundle` derives `Clone`
- All compose operations clone freely
- Profile after correctness is established
- Optimize with references only if profiling shows allocation pressure

---

## amplifier-core Interface Boundary

The critical unknown. Derived from **all 14 actual interaction points** in `bundle.py`.

All `amplifier-core` imports are confined to `bundle.py` via 3 lazy (deferred) imports:
- `bundle.py:1073` -> `from amplifier_core import AmplifierSession`
- `bundle.py:1274` -> `from amplifier_core import AmplifierSession`
- `bundle.py:1347` -> `from amplifier_core.models import HookResult`

```rust
/// What amplifier-foundation actually needs from amplifier-core.
/// Derived from all 14 interaction points in bundle.py lines 981-1380.

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
    fn mount(&mut self, name: &str, component: Box<dyn Any + Send + Sync>);
    fn get(&self, name: &str) -> Option<&(dyn Any + Send + Sync)>;
    fn register_capability(&mut self, key: &str, value: serde_json::Value);
    fn get_capability(&self, key: &str) -> Option<&serde_json::Value>;
    fn approval_system(&self) -> Option<&dyn ApprovalSystem>;
    fn display_system(&self) -> Option<&dyn DisplaySystem>;
    fn hooks(&self) -> &dyn HookRegistry;
    fn hooks_mut(&mut self) -> &mut dyn HookRegistry;
}

pub trait HookRegistry: Send + Sync {
    fn register(
        &mut self,
        event: &str,
        handler: Box<dyn HookHandler>,
        priority: i32,
        name: &str,
    );
}

pub trait ContextManager: Send + Sync {
    fn set_system_prompt_factory(&mut self, factory: Box<dyn SystemPromptFactory>);
    fn set_messages(&mut self, messages: Vec<serde_json::Value>);
    fn add_message(&mut self, message: serde_json::Value);
}

// Marker traits for type-erased systems passed through from parent sessions
pub trait ApprovalSystem: Send + Sync {}
pub trait DisplaySystem: Send + Sync {}
pub trait HookHandler: Send + Sync {}

pub trait SystemPromptFactory: Send + Sync {
    fn create(&self) -> BoxFuture<'_, String>;
}
```

**Strategy:** Define these traits first. Implement mocks for testing. Port everything against mocks. The runtime integration becomes a separate concern -- could be PyO3 FFI, gRPC, or a future Rust port of amplifier-core.

**Note:** `spawn_utils.py` also accesses `coordinator.get("providers")` but takes coordinator as `Any` -- the `Coordinator::get()` method covers this.

---

## Async/Sync Boundary

Not everything is async. 60% of modules do zero I/O.

| Module | Async? | Rationale |
|---|---|---|
| `dicts/` | **Sync** | Pure computation |
| `paths/` | **Sync** | Path manipulation only |
| `cache/memory` | **Sync** | HashMap operations |
| `cache/disk` | **Sync** | `std::fs` (small files, not worth async overhead) |
| `mentions/parser` | **Sync** | Regex extraction |
| `mentions/models` | **Sync** | Data structs |
| `mentions/dedup` | **Sync** | Deduplication logic |
| `mentions/resolver` | **Sync** | Path resolution (base impl) |
| `mentions/utils` | **Sync** | Directory listing |
| `session/slice` | **Sync** | Message list manipulation |
| `session/events` | **Sync** | Line-based JSONL I/O (BufReader, small files) |
| `session/capabilities` | **Sync** | Coordinator capability get/set |
| `serialization` | **Sync** | String sanitization |
| `tracing_utils` | **Sync** | UUID generation |
| `validator` | **Sync** | Rule evaluation |
| `spawn/mod` | **Mostly sync** | Only `apply_provider_preferences_with_resolution` is async |
| `spawn/glob` | **Sync** | Pattern matching |
| `error` | **Sync** | Type definitions |
| `io/frontmatter` | **Sync** | String parsing |
| `io/files` | **Async** | Retry with sleep |
| `io/yaml` | **Async** | Wraps async file I/O |
| `mentions/loader` | **Async** | File I/O pipeline |
| `sources/*` | **Async** | Network, subprocess |
| `bundle/compose` | **Sync** | Pure dict manipulation |
| `bundle/mod` | **Sync** | Struct definitions, from_dict |
| `bundle/prepared` | **Async** | Session creation, spawn (amplifier-core interaction) |
| `bundle/prompt` | **Async** | System prompt factory returns future |
| `bundle/module_resolver` | **Async** | Source resolution |
| `bundle/validator` | **Sync** | Rule evaluation |
| `registry/*` | **Async** | Network loading, parallel compose with join |
| `modules/*` | **Async** | Subprocess install |
| `updates/*` | **Async** | Network status checks |

---

## Bundle Composition -- Five Merge Strategies

The `compose()` method uses five distinct merge strategies for different fields. A simple `deep_merge` covers only 2 of 9 field categories.

```rust
impl Bundle {
    /// Compose two bundles. Overlay takes precedence per field-specific strategy.
    pub fn compose(base: &Bundle, overlay: &Bundle) -> Bundle {
        Bundle {
            // Strategy: later wins (with fallback to earlier if empty)
            name: overlay.name.clone().or(base.name.clone()),
            version: overlay.version.clone().or(base.version.clone()),
            base_path: overlay.base_path.clone().or(base.base_path.clone()),

            // Strategy 1: Deep merge (recursive dict merge) for session/spawn
            session: deep_merge_option(&base.session, &overlay.session),
            spawn: deep_merge_option(&base.spawn, &overlay.spawn),

            // Strategy 2: Merge by module ID for module lists
            //   - Index items by `module` key
            //   - Deep-merge per-module configs
            //   - Preserve order (IndexMap)
            providers: merge_module_lists_option(&base.providers, &overlay.providers),
            tools: merge_module_lists_option(&base.tools, &overlay.tools),
            hooks: merge_module_lists_option(&base.hooks, &overlay.hooks),

            // Strategy 3: Dict update for agents (later wins by agent key)
            agents: merge_agents(&base.agents, &overlay.agents),

            // Strategy 4: Accumulate with namespace prefix for context
            context: accumulate_context(&base.context, &overlay.context, &overlay.name),

            // Strategy 5: Later replaces entirely
            instruction: overlay.instruction.clone().or(base.instruction.clone()),

            // First-write-wins per key
            source_base_paths: merge_first_wins(
                &base.source_base_paths,
                &overlay.source_base_paths,
            ),

            // Accumulate (update)
            pending_context: merge_maps(&base.pending_context, &overlay.pending_context),

            // Forward compat: deep merge extra fields
            extra: deep_merge_maps(&base.extra, &overlay.extra),

            // Not carried through compose
            source_uri: overlay.source_uri.clone(),
        }
    }
}

/// Deep merge over serde_yaml_ng::Value
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

/// Merge module lists by ID. Items indexed by `module` key.
/// Later items deep-merge into earlier items with same ID.
/// Order preserved via IndexMap.
pub fn merge_module_lists(
    base: &[ModuleConfig],
    overlay: &[ModuleConfig],
) -> Vec<ModuleConfig> {
    let mut index: IndexMap<String, ModuleConfig> = IndexMap::new();

    for item in base.iter().chain(overlay.iter()) {
        let key = item.module_id();
        if let Some(existing) = index.get_mut(&key) {
            existing.deep_merge_from(item);
        } else {
            index.insert(key, item.clone());
        }
    }

    index.into_values().collect()
}
```

---

## Error Type Design

Python uses a class hierarchy with specific catch patterns. The Rust enum must support variant-specific matching.

```rust
#[derive(Debug, thiserror::Error)]
pub enum BundleError {
    #[error("bundle not found: {uri}")]
    NotFound { uri: String },

    #[error("failed to load bundle: {reason}")]
    LoadError {
        reason: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
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

pub type Result<T> = std::result::Result<T, BundleError>;
```

**Matching patterns used in the Python code (must work in Rust):**

```rust
// registry.py line 622: catch NotFound specifically, warn and skip
match result {
    Err(BundleError::NotFound { .. }) => {
        tracing::warn!("Include not found (skipping): {}", include_source);
    }
    Err(BundleError::DependencyError(msg)) => {
        self.log_circular_dependency_warning(&msg);
    }
    Err(e) => return Err(e),
    Ok(bundle) => { /* use it */ }
}
```

---

## Frontmatter Parser

No mature Rust crate for YAML frontmatter. Custom parser with edge case handling:

```rust
pub fn parse_frontmatter(content: &str) -> Result<(Option<serde_yaml_ng::Value>, &str)> {
    // Handle Windows line endings
    // Note: if \r\n detected, we must allocate a normalized copy.
    // For the common case (Unix), we work on the original &str.
    let normalized;
    let work = if content.contains("\r\n") {
        normalized = content.replace("\r\n", "\n");
        normalized.as_str()
    } else {
        content
    };

    // Check for opening delimiter: "---" followed by optional whitespace then newline
    if !work.starts_with("---") {
        return Ok((None, content));
    }
    let after_dashes = &work[3..];

    // Must be followed by whitespace-only then newline (or just newline)
    let after_open = match after_dashes.find('\n') {
        Some(pos) => {
            let between = &after_dashes[..pos];
            if !between.trim().is_empty() {
                // "---something" is not frontmatter
                return Ok((None, content));
            }
            &after_dashes[pos + 1..]
        }
        None => return Ok((None, content)),
    };

    // Find closing delimiter: "\n---" (newline then three dashes)
    let close_pattern = "\n---";
    let end = after_open
        .find(close_pattern)
        .ok_or_else(|| BundleError::LoadError {
            reason: "unclosed frontmatter".into(),
            source: None,
        })?;

    let yaml_str = &after_open[..end];

    // Handle empty frontmatter: serde_yaml_ng::from_str("") returns Err, not None
    let frontmatter = if yaml_str.trim().is_empty() {
        Some(serde_yaml_ng::Value::Mapping(Default::default()))
    } else {
        Some(serde_yaml_ng::from_str(yaml_str)?)
    };

    // Body starts after closing "---" + optional trailing whitespace + newline
    let rest = &after_open[end + close_pattern.len()..];
    // Skip optional whitespace on the "---" line, then consume the newline
    let body_start = rest
        .find('\n')
        .map(|pos| {
            // Verify only whitespace between --- and newline
            if rest[..pos].trim().is_empty() {
                pos + 1
            } else {
                0
            }
        })
        .unwrap_or(rest.len()); // No newline = end of file after ---

    let body = &rest[body_start..];

    // If we normalized, we need to return offsets into the original content.
    // For the normalized case, recalculate body offset in original.
    if content.contains("\r\n") {
        // Re-derive the body from the original content
        // Count the frontmatter portion length in original encoding
        let orig_end_marker = content
            .find("\n---\n")
            .or_else(|| content.find("\r\n---\r\n"))
            .or_else(|| content.find("\r\n---\n"))
            .ok_or_else(|| BundleError::LoadError {
                reason: "unclosed frontmatter in original".into(),
                source: None,
            })?;
        // Find body start in original
        let after_close = &content[orig_end_marker..];
        let body_offset = after_close
            .find('\n')
            .map(|p| orig_end_marker + p + 1)
            .unwrap_or(content.len());
        return Ok((frontmatter, &content[body_offset..]));
    }

    // For the common Unix case, body is a slice of the original content
    let offset = (body.as_ptr() as usize) - (content.as_ptr() as usize);
    Ok((frontmatter, &content[offset..]))
}
```

**Edge cases handled:**
1. Windows `\r\n` line endings
2. Empty frontmatter (`---\n---\n` yields empty Mapping, not Err)
3. No trailing newline after closing `---`
4. Trailing whitespace after `---` delimiter
5. Non-greedy: first `\n---` after open is the close

---

## Wave Plan

Tests are the specification. Port them first.

```
WAVE 0 --- Scaffold + Test Porting
  Agent S: Cargo.toml, src/lib.rs (empty mods), src/error.rs
  Agent T: Port ALL Python test files as #[ignore] Rust tests
           Source: 13 test files, ~124KB total
           Output: Full ignored test suite that defines the behavioral spec
  Gate: cargo build succeeds, all tests are #[ignore]

WAVE 1 --- Leaf modules (zero internal deps, max parallelism)      [ALL SYNC]
  Agent A: dicts/           (merge.rs with deep_merge + merge_module_lists, nested.rs)
  Agent B: paths/           (uri.rs with ParsedURI + ResolvedSource, normalize.rs, discovery.rs)
  Agent C: cache/           (memory.rs, disk.rs, CacheProvider trait)
  Agent D: spawn/           (mod.rs, glob.rs)
  Agent E: serialization.rs (sanitize_for_json, sanitize_message)
  Agent F: tracing_utils.rs (generate_sub_session_id)
  --- All 6 run simultaneously, only depend on error.rs ---
  Gate: un-ignore Wave 1 tests, cargo test passes

WAVE 2 --- Mid-tier (depend on Wave 1 outputs)
  Agent G: io/              (yaml.rs [ASYNC], frontmatter.rs [SYNC], files.rs [ASYNC])
           depends on: paths/
  Agent H: sources/         (all 5 handlers + resolver + protocol)                [ASYNC]
           depends on: paths/, cache/
  Agent I: session/         (capabilities, events, fork, slice)                   [ALL SYNC]
           depends on: paths/
  Agent J: mentions/        (models, parser, resolver, dedup, utils [SYNC],
                             loader [ASYNC])
           depends on: paths/, io/files (for loader)
  --- All 4 run simultaneously ---
  --- J.loader waits for G if needed ---
  Gate: un-ignore Wave 2 tests, cargo test passes

WAVE 3 --- Core (the actual migration -- 2,641 lines of Python)
  Phase 3a:
    Agent K: bundle/mod.rs + compose.rs                                           [SYNC]
             Bundle struct, from_dict, 5 merge strategies
             ~650 lines. Needs: dicts/, paths/, mentions/models
    Agent L: bundle/validator.rs                                                  [SYNC]
             depends on: bundle/mod.rs types only
    --- K and L can partially overlap ---
    Gate: bundle composition tests pass

  Phase 3b:
    Agent M: bundle/prepared.rs + module_resolver.rs + prompt.rs                  [ASYNC]
             PreparedBundle -- the hardest code (~445 lines)
             Full session creation lifecycle, spawn, prompt factory
             Needs: the full AmplifierRuntime trait boundary (mocked)
             depends on: bundle/mod.rs, sources/, mentions/, spawn/
    Gate: prepared bundle tests pass against mocked AmplifierRuntime

  Phase 3c:
    Agent N: registry/       (mod.rs, persistence.rs, includes.rs)                [ASYNC]
             ~1,261 lines including cycle detection, parallel include loading
             depends on: bundle/, io/, paths/, sources/
    Gate: registry tests pass

  Phase 3d:
    Agent O: modules/        (activator, state)                                   [ASYNC]
             depends on: sources/, io/, paths/
    Agent P: updates/        (mod.rs)                                             [ASYNC]
             depends on: sources/, registry/
    --- O and P run after N ---
    Gate: all module/updates tests pass

WAVE 4 --- Integration surface
  Agent Q: lib.rs            (61 pub use re-exports, top-level convenience functions)
  Agent R: examples/         (3 example binaries)
  Gate: cargo build --examples succeeds

WAVE 5 --- Integration tests + stabilization
  Agent S: Integration tests (load real .yaml/.md bundles from amplifier-foundation repo)
  Agent T: Roundtrip tests   (every agents/, behaviors/, providers/, bundles/ file must parse)
  Agent U: cargo clippy --all-targets -- -D warnings, cargo fmt --check
  --- All 3 run simultaneously ---
  Gate: zero warnings, all roundtrip tests pass
```

**Peak parallelism:** 6 agents (Wave 1)
**Total agents:** ~21 across 6 waves
**Sequential depth:** 6 waves (Wave 3 has 4 internal phases)
**True bottleneck:** Wave 3 phases 3b (PreparedBundle) and 3c (Registry) -- 1,700+ lines

---

## Agent Contract Per Module

Each agent receives:

1. The Python source file(s) for their module
2. The `src/error.rs` file (shared error types)
3. The trait/type definitions from modules they depend on (public API only, not implementation)
4. The corresponding Python test file(s) (the behavioral specification)
5. Explicit instructions:

```
- Write idiomatic Rust, not transliterated Python
- All public items get /// doc comments matching Python docstrings
- All pub fn gets a unit test in the same file (#[cfg(test)] mod tests)
- Use Result<T, BundleError> for all fallible operations
- Use tracing::debug! / tracing::warn! instead of print/logging
- No unwrap() in library code -- propagate errors with ?
- Sync functions stay sync. Do not add async to sync modules.
- Use serde_yaml_ng, not serde_yaml or serde_yml
- Clone freely in v1. Do not fight lifetimes.
```

---

## Risks and Mitigations

| Risk | Severity | Mitigation |
|---|---|---|
| amplifier-core dependency -- Python-only, no Rust port | Critical | Define 7-trait boundary (above). Mock everything. Port all foundation code against mocks. Decide later: PyO3 FFI, full port, or gRPC bridge. |
| Lifetime complexity in Bundle::compose() | High | Clone aggressively in v1. Bundle derives Clone. Profile later. |
| PreparedBundle._create_system_prompt_factory() returns async closure | High | In Rust: `Box<dyn SystemPromptFactory>` with `fn create(&self) -> BoxFuture<'_, String>`. Non-trivial lifetime management -- isolate in bundle/prompt.rs. |
| #[serde(flatten)] + serde_yaml_ng::Value silent data loss | High | Roundtrip tests against every real bundle file in the repo. If flatten bugs surface, fall back to manual deserialization for Bundle struct. |
| async-trait + dyn dispatch overhead | Medium | Accept Box<dyn Future> allocation. Replace with native async traits when object-safe version stabilizes. |
| Dynamic dict access patterns -- get_nested("a.b.c") | Medium | Implement via serde_yaml_ng::Value traversal. Consider a `value_path!` macro. |
| Module list merging by ID | Medium | Implement MergeById via IndexMap. Items need module_id() method. |
| Test mock ergonomics | Medium | Use mockall. Define all extensibility points as traits from day 1. |
| YAML schema drift | Medium | `#[serde(default)]` liberally. Roundtrip tests. Permissive deserialization. |
| Cloud sync retry logic (OneDrive/Dropbox) | Low | Direct port with tokio::time::sleep. |
| Glob pattern matching for model resolution | Low | glob crate's Pattern::matches(). |
| Session fork -- events.jsonl slicing | Low | JSONL is line-based, maps to BufReader::lines(). |

---

## Feature Flags

```toml
[features]
default = ["git", "http-sources", "zip-sources"]
git = []                    # GitSourceHandler -- mandatory for registry/updates
http-sources = ["reqwest"]  # HttpSourceHandler -- truly optional
zip-sources = ["dep:zip"]   # ZipSourceHandler -- truly optional
```

**Why `git` is not truly optional:** `registry.py` uses `GitSourceHandler` directly in `_compose_includes`. The `updates/` module imports `GitSourceHandler` directly. Git is the primary source type. However, a `parse-only` use case (just load YAML, don't resolve sources) could disable it.

**Potential additional flags:**
- `module-activation` -- gate the entire `modules/` directory (subprocess dependency management) for pure bundle-loading use cases
- `session-management` -- gate `session/` for consumers that only need bundle composition

---

## Additional Considerations

### 1. WASM target

If WASM is a goal, avoid `tokio::process` (no subprocess in WASM). Gate git/zip handlers behind `#[cfg(not(target_arch = "wasm32"))]`. Use reqwest with wasm feature for HTTP in browser.

### 2. Python interop via PyO3

If the migration is incremental, expose the Rust crate as a Python module:

```toml
[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
pyo3 = { version = "0.22", features = ["extension-module"], optional = true }
```

This lets Python code call Rust bundle loading/composition while the rest stays Python.

### 3. Bundle file compatibility

The YAML/markdown bundle files in `agents/`, `behaviors/`, `providers/`, `bundles/` must load identically. Write roundtrip tests that load every `.yaml` and `bundle.md` in the repo and assert parse success. This is the acceptance criterion.

### 4. Registry JSON compatibility

`~/.amplifier/registry.json` must be readable by both Python and Rust versions during any transition period. Use `#[serde(default)]` for permissive deserialization. Avoid `#[serde(deny_unknown_fields)]`.

### 5. Async runtime choice

Tokio is the default. But if this crate is used as a library embedded in other runtimes (e.g., a Tauri app), don't spawn a runtime internally -- let the caller own it. All public async functions should be runtime-agnostic where possible.

### 6. compose() semantics must be exact

Bundle composition has specific override rules documented in the Five Merge Strategies section. Port the Python tests first as the spec, then implement until they pass. This is the most semantically dense code in the crate.

### 7. discovery/ module

`discovery/__init__.py` is empty in the current Python source. Determine if it's a planned extension point or dead code before allocating Rust module space. Currently omitted from the Rust target.

---

## Validation Findings Log

Full record of what the three investigations found, preserved for reference.

### Crate Research Findings (2025-01-28)

- `serde_yaml 0.9.34`: Archived by dtolnay on 2024-03-25. Read-only repo. No further maintenance.
- `serde_yml`: **RUSTSEC-2025-0068** -- `Serializer.emitter` causes segfaults. Archived after unsoundness discovered.
- `serde_yaml_ng 0.10+`: Maintained fork by acatton. API-compatible. Recommended replacement.
- `serde_norway 0.9.42`: Alternative fork using libyaml-norway.
- `async-trait 0.1.89`: Still actively maintained. Required for dyn-dispatched async traits until Rust stabilizes object-safe async traits (~2025 H2).
- `thiserror 2.0.9`: Current. Coexists with 1.x in dep trees. Better no_std support.
- `indexmap 2.13.0`: Actively maintained. Works well with serde. Needed for ordered module list merging.

### Python Source Exploration Findings (2025-01-28)

- **47 .py files** in amplifier_foundation/, **8,780 total LOC**
- `__all__` has **61 exports** (blueprint claimed 82 -- off by 21)
- `bundle.py` is **1,380 LOC** (blueprint claimed ~1,289 -- off by +91)
- `registry.py` is **1,261 LOC** (blueprint claimed ~1,223 -- off by +38)
- All `amplifier_core` imports confined to `bundle.py` via 3 lazy imports
- Actual core interaction: 14 distinct points (constructor with 6 params, coordinator.mount/register_capability/get_capability/get, hooks.register, initialize/execute/cleanup, session_id, approval_system/display_system, ContextManager sub-interface)
- `session/` submodule exports (fork_session, ForkResult, etc.) not re-exported from top-level `__init__.py`
- `modules/` subpackage (ModuleActivator, InstallStateManager) not exported from public API -- internal only
- `discovery/__init__.py` is empty

### Architecture Review Findings (2025-01-28)

**3 fatal flaws identified:**
1. AmplifierRuntime trait covers ~20% of actual core usage (3 methods vs 14 interaction points)
2. PreparedBundle (445 LOC), BundleModuleResolver, BundleModuleSource missing from blueprint entirely
3. Wave 3 described as "mostly sequential" but contains 2,641 lines = the actual migration

**4 serious errors identified:**
1. Single BundleError description obscures need for variant-specific matching
2. `#[serde(flatten)]` + `Value` has known silent data loss bugs
3. Tests in Wave 5 is backwards -- should be Wave 0 (tests are the spec)
4. Making everything async wastes compile time for 60% of modules that do zero I/O

**Medium issues:**
- Composition uses 5 strategies, not just deep_merge
- Git source handler is not optional (used directly by registry and updates)
- Frontmatter parser has 5 unhandled edge cases
- Dynamic attribute injection (`bundle._source_uri`) needs proper field
- `PreparedBundle._create_system_prompt_factory()` is an async closure -- complex Rust lifetime
