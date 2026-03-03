# Rust Migration Blueprint: amplifier-foundation (Revised)

This document contains the investigation findings from validating the original blueprint, followed by the corrected blueprint that addresses all discovered issues.

---

## Part 1: Investigation Findings

### Investigation 1: Crate Ecosystem Research

| Crate | Finding |
|---|---|
| `serde_yaml` | Archived and deprecated by dtolnay on March 25, 2024. Repository is read-only. No more fixes. |
| `serde_yml` (originally suggested) | **UNSOUND** -- has a RustSec advisory (RUSTSEC-2025-0068). Can cause segmentation faults. Project archived after unsoundness was raised. Do not use under any circumstances. |
| `serde_yaml_ng` | Maintained fork by acatton, API-compatible with `serde_yaml`. Current at 0.10+. This is the replacement. |
| `async-trait` | Still required for dyn Trait dispatch. Rust 1.75+ stabilized async fn in traits BUT only for concrete implementations -- if you need `Box<dyn MyTrait>` with async methods, you still need async-trait. The Rust team is working on object-safe async traits for 2025. |
| `thiserror 2.0` | Current at 2.0.9. Safe to use. Can coexist with 1.x in the same dep tree -- Cargo handles different major versions independently. |
| `indexmap` | Current at 2.13.0, actively maintained, works well with serde via `features = ["serde"]`. Missing from the blueprint's dependency list. |

### Investigation 2: Python Source Verification

The explorer surveyed all 47 `.py` files in `amplifier_foundation/`. Key findings:

**Export count is wrong.** Blueprint claims 82 exports. Actual `__all__` has 61 entries. The 21-entry gap likely means the blueprint counted all imported names, not just `__all__`.

**LOC counts are off.** `bundle.py` is actually 1,380 lines (+91 over the claimed ~1,289). `registry.py` is 1,261 lines (+38 over the claimed ~1,223). The extra 91 lines in `bundle.py` include the `spawn()` method's child session wiring -- a significant chunk the blueprint doesn't account for.

**Five files/modules the blueprint doesn't mention:**

| Missing | What it does |
|---|---|
| `serialization.py` (139 LOC) | `sanitize_for_json()`, `sanitize_message()` -- exported in `__all__` |
| `tracing.py` (105 LOC) | `generate_sub_session_id()` -- W3C-compatible session ID generation, exported in `__all__` |
| `mentions/loader.py` (199 LOC) | `load_mentions()` -- the actual mention loading pipeline, the main entry point |
| `mentions/models.py` (~35 LOC) | `ContextFile`, `MentionResult` data classes |
| `mentions/protocol.py` + `utils.py` (~60 LOC) | `MentionResolverProtocol`, `format_directory_listing()` |
| `sources/protocol.py` (175 LOC) | `SourceStatus`, `SourceResolverProtocol`, `SourceHandlerProtocol` -- the trait definitions |
| `cache/protocol.py` (~40 LOC) | `CacheProviderProtocol` -- the trait definition |

**The amplifier-core dependency is narrower than expected -- but the trait surface is wider.** All imports are confined to `bundle.py` via 3 lazy imports. But the actual usage is 14 distinct interaction points, not the 2-method trait the blueprint proposes. The explorer found every accessed attribute:

- `AmplifierSession()` constructor with 6 params
- `session.coordinator.mount()`, `.register_capability()`, `.get_capability()`, `.get()`
- `session.coordinator.hooks.register()`
- `session.coordinator.approval_system`, `.display_system`
- `session.initialize()`, `.execute()`, `.cleanup()`, `.session_id`
- `coordinator.get("context")` -> `ContextManager` with `.set_messages()`, `.add_message()`, `.set_system_prompt_factory()`

`spawn_utils.py` accesses `coordinator.get("providers")` but takes coordinator as `Any` -- no direct import. The Rust trait boundary must include a providers component accessor.

**Cross-module dependency map** confirmed the wave ordering is mostly correct but revealed:

- `mentions/loader.py` depends on `io/files.py` -- so mentions and io aren't fully parallel in Wave 2
- `validator.py` has a forward dependency on `bundle.py` types
- `discovery/__init__.py` is empty -- dead code or reserved namespace
- Waves 0-3 (leaf + mid) are parallelizable. `bundle.py` (Wave 3 in the corrected plan) is the true serialization bottleneck

### Investigation 3: Antagonistic Architecture Review

The zen-architect read every source file and came back with 3 fatal flaws, 4 serious errors:

**Fatal 1 -- AmplifierRuntime trait is fantasy.** Covers ~20% of actual usage. The proposed Session trait has 3 methods; the actual code uses 14 interaction points. The "mock it" strategy fails because you can't mock what you haven't discovered you need. Wave 3 would block on discovering missing trait methods.

**Fatal 2 -- PreparedBundle is invisible in the blueprint.** 445 lines in `bundle.py` (lines 845-1289) that handle the entire session creation lifecycle, spawning, system prompt factories, mention resolution integration, and capability inheritance. The blueprint mentions Bundle but never PreparedBundle. This class alone has more amplifier-core surface than the entire proposed trait boundary. Also missing: `BundleModuleResolver` and `BundleModuleSource` (100 lines in `bundle.py` implementing the kernel's `ModuleSourceResolver` protocol).

**Fatal 3 -- Wave plan misjudges the bottleneck.** Wave 3 is described as "mostly sequential" but `bundle.py` + `registry.py` = 2,641 lines = the vast majority of the real work. Everything else is preamble. The blueprint undersells this as just another wave.

**Serious Error 1 -- Single BundleError enum loses match granularity.** The Python code explicitly catches `BundleNotFoundError` vs `BundleDependencyError` in different code paths. The Rust enum needs variant-specific matching -- which the blueprint's design actually supports, but calling it "a single enum" obscures the need.

**Serious Error 2 -- `serde_yaml::Value` + `#[serde(flatten)]` has known bugs.** The combination can cause silent data loss. Four specific pain points: `Bundle.session` is deeply nested dynamic config, `Bundle.agents` inner dicts have no fixed schema, module lists (providers/tools/hooks) are `list[dict[str, Any]]`, and includes can be strings OR dicts.

**Serious Error 3 -- Tests in Wave 5 is backwards.** 124KB of Python tests exist. They define the exact behavior Rust must reproduce. They should be ported in Wave 0 as `#[ignore]` tests and un-ignored module by module as each wave completes.

**Serious Error 4 -- 60% of modules are pure sync.** `dicts/`, `paths/`, `cache/simple`, `mentions/parser`, `mentions/resolver`, `serialization`, `tracing`, `validator`, `session/slice`, `session/events`, `session/capabilities` -- none of these do I/O. Making everything async forces tokio as a dep for pure computation.

**Additional findings:**

- Composition uses 5 distinct merge strategies, not just `deep_merge` (deep merge for session/spawn, merge-by-ID for module lists, dict update for agents, accumulate-with-namespace for context, later-wins for instruction)
- git source handler is not optional -- `registry.py` and `updates/` import it directly
- Frontmatter parser has 5 edge cases: `\r\n`, empty frontmatter, no trailing newline, multiple `---`, trailing whitespace after delimiters
- Dynamic attribute injection (`bundle._source_uri = uri  # type: ignore`) needs to become a proper field in Rust
- `PreparedBundle._create_system_prompt_factory()` returns an async closure -- in Rust that's `Box<dyn Fn() -> BoxFuture<'_, String>>`, non-trivial lifetime management

---

## Part 2: Corrected Blueprint

All corrections from the three investigations are applied below.

### Corrected Crate Dependencies

```toml
[dependencies]
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml_ng = "0.10"           # CHANGED: serde_yaml is archived
reqwest = { version = "0.12", features = ["rustls-tls"], optional = true }
thiserror = "2"
async-trait = "0.1"              # Still needed for dyn-dispatched async traits
glob = "0.3"
regex = "1"
dirs = "6"
sha2 = "0.10"
zip = { version = "2", optional = true }
tracing = "0.1"
tracing-subscriber = "0.3"
uuid = { version = "1", features = ["v4"] }
chrono = { version = "0.4", features = ["serde"] }
indexmap = { version = "2", features = ["serde"] }  # ADDED: ordered maps for module list merging

[features]
default = ["git", "http-sources", "zip-sources"]
git = []                          # GitSourceHandler (mandatory for registry, but gate-able for pure parse use)
http-sources = ["reqwest"]        # RENAMED: HttpSourceHandler
zip-sources = ["dep:zip"]         # RENAMED: ZipSourceHandler

[dev-dependencies]
tokio-test = "0.4"
tempfile = "3"
mockall = "0.13"
assert_matches = "1"
```

### Corrected Module Mapping (47 Python files to Rust)

| Python | Rust | Notes |
|---|---|---|
| `__init__.py` (61 exports) | `lib.rs` | CORRECTED: 61, not 82 |
| `exceptions.py` | `error.rs` | |
| `bundle.py` (1,380 LOC) | `bundle/mod.rs` | Bundle struct, from_dict |
| | `bundle/compose.rs` | compose(), 5 merge strategies |
| | `bundle/mount.rs` | MountPlan, section types |
| | `bundle/prepared.rs` | **NEW:** PreparedBundle (445 LOC) |
| | `bundle/module_resolver.rs` | **NEW:** BundleModuleResolver, BundleModuleSource |
| | `bundle/prompt.rs` | **NEW:** system prompt factory logic |
| | `bundle/validator.rs` | BundleValidator, validate_bundle() |
| `serialization.py` | `serialization.rs` | **NEW:** sanitize_for_json, sanitize_message |
| `tracing.py` | `tracing_utils.rs` | **NEW:** generate_sub_session_id (avoid name clash with tracing crate) |
| `registry.py` (1,261 LOC) | `registry/mod.rs` | BundleRegistry |
| | `registry/persistence.rs` | JSON serialization |
| | `registry/includes.rs` | **NEW:** include parsing, cycle detection |
| `sources/protocol.py` | `sources/mod.rs` | SourceHandler trait, ResolvedSource, SourceStatus |
| `sources/resolver.py` | `sources/resolver.rs` | |
| `sources/file.py` | `sources/file.rs` | |
| `sources/git.py` | `sources/git.rs` | |
| `sources/http.py` | `sources/http.rs` | |
| `sources/zip.py` | `sources/zip.rs` | |
| `mentions/protocol.py` | `mentions/mod.rs` | MentionResolver trait |
| `mentions/models.py` | `mentions/models.rs` | **NEW:** ContextFile, MentionResult |
| `mentions/parser.py` | `mentions/parser.rs` | |
| `mentions/resolver.py` | `mentions/resolver.rs` | BaseMentionResolver |
| `mentions/loader.py` | `mentions/loader.rs` | **NEW:** load_mentions pipeline |
| `mentions/deduplicator.py` | `mentions/dedup.rs` | |
| `mentions/utils.py` | `mentions/utils.rs` | **NEW:** format_directory_listing |
| `io/yaml.py` | `io/yaml.rs` | |
| `io/frontmatter.py` | `io/frontmatter.rs` | |
| `io/files.py` | `io/files.rs` | |
| `dicts/merge.py` | `dicts/merge.rs` | deep_merge + merge_module_lists |
| `dicts/navigation.py` | `dicts/nested.rs` | RENAMED: navigation.py, not nested.py |
| `paths/resolution.py` | `paths/uri.rs` | ParsedURI, parse_uri, ResolvedSource |
| `paths/construction.py` | `paths/normalize.rs` | RENAMED: construction.py, not normalize.py |
| `paths/discovery.py` | `paths/discovery.rs` | |
| `cache/protocol.py` | `cache/mod.rs` | CacheProvider trait |
| `cache/simple.py` | `cache/memory.rs` | RENAMED: simple.py to memory.rs |
| `cache/disk.py` | `cache/disk.rs` | |
| `session/capabilities.py` | `session/capabilities.rs` | |
| `session/events.py` | `session/events.rs` | |
| `session/fork.py` | `session/fork.rs` | |
| `session/slice.py` | `session/slice.rs` | |
| `spawn_utils.py` | `spawn/mod.rs` | |
| | `spawn/glob.rs` | |
| `modules/activator.py` | `modules/mod.rs` | |
| `modules/install_state.py` | `modules/state.rs` | |
| `updates/__init__.py` | `updates/mod.rs` | |

### Corrected AmplifierRuntime Trait Boundary

Derived from all 14 interaction points in `bundle.py`:

```rust
/// What amplifier-foundation actually needs from amplifier-core.
/// Derived from all 14 interaction points in bundle.py.

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
    fn register(&mut self, event: &str, handler: Box<dyn HookHandler>, priority: i32, name: &str);
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

### Corrected Async/Sync Boundary

| Module | Async? | Rationale |
|---|---|---|
| `dicts/` | Sync | Pure computation |
| `paths/` | Sync | Path manipulation only |
| `cache/memory` | Sync | HashMap ops |
| `cache/disk` | Sync | Use `std::fs`, not `tokio::fs` (small files) |
| `mentions/parser` | Sync | Regex extraction |
| `mentions/models` | Sync | Data structs |
| `mentions/dedup` | Sync | Deduplication logic |
| `session/slice` | Sync | Message list manipulation |
| `session/events` | Sync | Line-based JSONL I/O |
| `session/capabilities` | Sync | Coordinator capability access |
| `serialization` | Sync | String sanitization |
| `tracing_utils` | Sync | UUID generation |
| `validator` | Sync | Rule evaluation |
| `spawn/` | Mostly sync | Only `apply_provider_preferences_with_resolution` is async |
| `io/files` | Async | Retry with sleep |
| `io/yaml` | Async | Wraps async file I/O |
| `mentions/loader` | Async | File I/O pipeline |
| `mentions/resolver` | Sync | Path resolution (base impl) |
| `sources/*` | Async | Network, subprocess |
| `bundle/compose` | Sync | Pure dict manipulation |
| `bundle/prepared` | Async | Session creation, spawn |
| `registry/` | Async | Network loading, parallel compose |
| `modules/` | Async | Subprocess install |
| `updates/` | Async | Network status checks |

### Corrected Wave Plan

```
WAVE 0 -- Scaffold + Test Porting
  Agent S: Cargo.toml, src/lib.rs (empty mods), src/error.rs
  Agent T: Port ALL Python test files as #[ignore] Rust tests
           (124KB of tests = the behavioral specification)
  Output: Compiling skeleton + full ignored test suite

WAVE 1 -- Leaf modules (zero internal deps, true parallel)
  Agent A: dicts/           (merge.rs with deep_merge + merge_module_lists, nested.rs)    [SYNC]
  Agent B: paths/           (uri.rs with ParsedURI + ResolvedSource, normalize.rs, discovery.rs) [SYNC]
  Agent C: cache/           (memory.rs, disk.rs, CacheProvider trait)                      [SYNC]
  Agent D: spawn/           (mod.rs, glob.rs)                                              [SYNC]
  Agent E: serialization.rs (sanitize_for_json, sanitize_message)                          [SYNC, NEW]
  Agent F: tracing_utils.rs (generate_sub_session_id)                                      [SYNC, NEW]
  --- All 6 run simultaneously ---
  Gate: un-ignore Wave 1 tests, all must pass

WAVE 2 -- Mid-tier (depend on Wave 1)
  Agent G: io/              (yaml.rs, frontmatter.rs with edge cases, files.rs)            [ASYNC]
  Agent H: sources/         (all 5 handlers + resolver + protocol)                         [ASYNC]
  Agent I: session/         (capabilities [SYNC], events [SYNC], fork [SYNC], slice [SYNC])
  Agent J: mentions/        (models [SYNC], parser [SYNC], resolver [SYNC],
                             dedup [SYNC], loader [ASYNC], utils [SYNC])
           depends on: io/files.rs for loader
  --- All 4 run simultaneously (J waits for G if loader needs io/) ---
  Gate: un-ignore Wave 2 tests, all must pass

WAVE 3 -- Core (the actual migration -- 2,641 lines)
  Agent K: bundle/mod.rs + compose.rs (Bundle struct, from_dict, 5 merge strategies)       [SYNC]
           This is ~650 lines and needs: dicts/, paths/, mentions/models
  Agent L: bundle/validator.rs                                                              [SYNC]
           depends on: bundle/mod.rs types only
  --- K runs first, L can overlap ---
  Gate: bundle composition tests pass

  Agent M: bundle/prepared.rs + module_resolver.rs + prompt.rs                              [ASYNC]
           This is ~445 lines, the hardest code -- needs the full AmplifierRuntime trait
           depends on: bundle/mod.rs, sources/, mentions/, spawn/
  Gate: prepared bundle tests pass (against mocked AmplifierRuntime)

  Agent N: registry/        (mod.rs, persistence.rs, includes.rs)                           [ASYNC]
           depends on: bundle/, io/, paths/, sources/
           ~1,261 lines including cycle detection, parallel include loading
  Gate: registry tests pass

  Agent O: modules/         (activator, state)                                              [ASYNC]
           depends on: sources/, io/, paths/
  Agent P: updates/         (mod.rs)                                                        [ASYNC]
           depends on: sources/, registry/
  --- O and P run after N ---

WAVE 4 -- Integration surface
  Agent Q: lib.rs           (61 pub use re-exports, top-level convenience functions)
  Agent R: examples/        (3 example binaries)

WAVE 5 -- Integration tests + roundtrip
  Agent S: integration tests (load real .yaml/.md bundles from amplifier-foundation repo)
  Agent T: roundtrip tests (every agents/, behaviors/, providers/, bundles/ file must parse)
  Agent U: cargo clippy --all-targets, cargo fmt --check, fix all warnings
```

Peak parallelism: 6 agents (Wave 1). Total agents: ~21 across 6 waves. Sequential depth: 6 waves.

### Corrected Frontmatter Parser

```rust
pub fn parse_frontmatter(content: &str) -> Result<(Option<serde_yaml_ng::Value>, &str)> {
    // Normalize line endings for Windows compat
    let normalized;
    let content = if content.contains("\r\n") {
        normalized = content.replace("\r\n", "\n");
        normalized.as_str()
    } else {
        content
    };

    // Check for opening delimiter (with optional trailing whitespace)
    if !content.starts_with("---") {
        return Ok((None, content));
    }
    let after_open = &content[3..];
    let after_open = after_open.strip_prefix(|c: char| c == ' ' || c == '\t')
        .map(|s| s.strip_prefix('\n'))
        .flatten()
        .or_else(|| after_open.strip_prefix('\n'));

    let after_open = match after_open {
        Some(s) => s,
        None => return Ok((None, content)), // "---" followed by non-whitespace, not frontmatter
    };

    // Find closing delimiter
    let close_pattern = "\n---";
    let end = after_open.find(close_pattern)
        .ok_or_else(|| BundleError::LoadError {
            reason: "unclosed frontmatter".into(),
            source: None,
        })?;

    let yaml_str = &after_open[..end];

    // Handle empty frontmatter: yaml_str is "" or whitespace-only
    let frontmatter = if yaml_str.trim().is_empty() {
        Some(serde_yaml_ng::Value::Mapping(Default::default()))
    } else {
        Some(serde_yaml_ng::from_str(yaml_str)?)
    };

    // Body starts after closing "---" + optional whitespace + newline
    let rest = &after_open[end + close_pattern.len()..];
    let body = rest.strip_prefix(|c: char| c.is_ascii_whitespace() && c != '\n')
        .unwrap_or(rest);
    let body = body.strip_prefix('\n').unwrap_or(body);

    Ok((frontmatter, body))
}
```

### Corrected Bundle Composition (5 strategies, not 1)

```rust
impl Bundle {
    pub fn compose(base: &Bundle, overlay: &Bundle) -> Bundle {
        Bundle {
            name: overlay.name.clone().or(base.name.clone()),
            version: overlay.version.clone().or(base.version.clone()),

            // Strategy 1: Deep merge for session/spawn
            session: deep_merge_option(&base.session, &overlay.session),
            spawn: deep_merge_option(&base.spawn, &overlay.spawn),

            // Strategy 2: Merge by module ID for module lists
            providers: merge_module_lists_option(&base.providers, &overlay.providers),
            tools: merge_module_lists_option(&base.tools, &overlay.tools),
            hooks: merge_module_lists_option(&base.hooks, &overlay.hooks),

            // Strategy 3: Dict update for agents (later wins by key)
            agents: merge_agents(&base.agents, &overlay.agents),

            // Strategy 4: Accumulate with namespace for context
            context: accumulate_context(&base.context, &overlay.context, &overlay.name),

            // Strategy 5: Later replaces entirely
            instruction: overlay.instruction.clone().or(base.instruction.clone()),
            base_path: overlay.base_path.clone().or(base.base_path.clone()),

            // First-write-wins per key
            source_base_paths: merge_first_wins(&base.source_base_paths, &overlay.source_base_paths),

            // Accumulate
            pending_context: merge_maps(&base.pending_context, &overlay.pending_context),

            // Forward compat
            extra: deep_merge_maps(&base.extra, &overlay.extra),
        }
    }
}
```

### Corrected Error Type

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
    ValidationError(ValidationResult),  // Carries full result with errors + warnings

    #[error("dependency error: {0}")]
    DependencyError(String),            // Matchable variant for cycle detection

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Yaml(#[from] serde_yaml_ng::Error),

    #[error(transparent)]
    Http(#[from] reqwest::Error),

    #[error("git error: {0}")]
    Git(String),
}

// Registry code matches on specific variants:
// match result {
//     Err(BundleError::NotFound { .. }) => warn and skip,
//     Err(BundleError::DependencyError(msg)) => log circular dep warning,
//     Err(e) => propagate,
// }
```

---

## Summary of All Changes from Original to Revised

### Fatal Issues Fixed
1. **AmplifierRuntime trait expanded** from 2-method fantasy to full 14-interaction-point boundary with `AmplifierSession`, `Coordinator`, `HookRegistry`, `ContextManager`, and marker traits
2. **PreparedBundle added** as `bundle/prepared.rs` + `bundle/module_resolver.rs` + `bundle/prompt.rs` (445 LOC of the hardest code)
3. **Wave plan rebalanced** -- Wave 3 explicitly called out as the bottleneck (2,641 lines), tests moved to Wave 0

### Serious Issues Fixed
4. **Error type corrected** -- `ValidationError` now carries `ValidationResult`, variant matching documented
5. **serde_yaml replaced** with `serde_yaml_ng` (serde_yaml archived, serde_yml UNSOUND)
6. **Tests moved to Wave 0** as `#[ignore]` specs, un-ignored per wave
7. **Sync/async boundary defined** -- 60% of modules are pure sync, no unnecessary tokio dependency

### Missing Modules Added
8. `serialization.rs` -- sanitize_for_json, sanitize_message
9. `tracing_utils.rs` -- generate_sub_session_id
10. `mentions/loader.rs` -- load_mentions pipeline
11. `mentions/models.rs` -- ContextFile, MentionResult
12. `mentions/utils.rs` -- format_directory_listing
13. `registry/includes.rs` -- include parsing, cycle detection
14. `bundle/prepared.rs` -- PreparedBundle session lifecycle
15. `bundle/module_resolver.rs` -- BundleModuleResolver, BundleModuleSource
16. `bundle/prompt.rs` -- system prompt factory

### Dependency Corrections
17. `serde_yaml = "0.9"` replaced with `serde_yaml_ng = "0.10"`
18. `indexmap = { version = "2", features = ["serde"] }` added
19. `reqwest` and `zip` made properly optional
20. Feature flags renamed for clarity (`http` -> `http-sources`, `zip` -> `zip-sources`)

### Composition Semantics Corrected
21. Single `deep_merge` replaced with 5 distinct strategies: deep merge (session/spawn), merge-by-ID (module lists), dict update (agents), accumulate-with-namespace (context), later-wins (instruction)

### Frontmatter Parser Hardened
22. Windows `\r\n` handling added
23. Empty frontmatter edge case handled
24. Trailing whitespace after delimiters handled
25. Missing trailing newline handled

### Naming Corrections
26. `dicts/nested.py` is actually `dicts/navigation.py`
27. `paths/normalize.py` is actually `paths/construction.py`
28. `cache/memory.py` is actually `cache/simple.py`
29. Export count corrected from 82 to 61
30. `bundle.py` LOC corrected from 1,289 to 1,380
31. `registry.py` LOC corrected from 1,223 to 1,261
