# amplifier-foundation-rs -- Context Transfer

> This file is the institutional memory of the project. Updated continuously.
> Each session reads this to understand recent decisions and context.
> Reverse-chronological: newest entries at the top.

---

## Session 001 -- Wave 0 Scaffold (F-001, F-002, F-003)

### Work Completed
- **F-001** (5b6ccc8): Full Cargo scaffold -- Cargo.toml with all deps, 52 source files across 12 module directories, .gitignore, cdylib+rlib crate types. cargo check/test/clippy all pass.
- **F-002** (54d92be): BundleError enum (8 variants) + 7 runtime traits (AmplifierRuntime, AmplifierSession, Coordinator, HookRegistry, ContextManager, ApprovalSystem/DisplaySystem/HookHandler markers, SystemPromptFactory). Http variant uses String not reqwest::Error (optional dep).
- **F-003** (5fb8c21): 87 #[ignore = "Wave 1"] tests across 6 test files + function stubs with todo!() bodies in all Wave 1 modules.

### Test Counts Verified
- test_dicts.rs: 18 tests (deep_merge 5, merge_module_lists 7, get_nested 3, set_nested 3)
- test_paths.rs: 15 tests (parse_uri 8, normalize_path 4, construct_paths 3)
- test_cache.rs: 12 tests (SimpleCache 3, DiskCache 9)
- test_serialization.rs: 16 tests (sanitize_for_json 8, sanitize_message 8)
- test_tracing.rs: 9 tests (generate_sub_session_id 9)
- test_spawn.rs: 17 tests (ProviderPreference 5, is_glob_pattern 4, apply_provider_preferences 8)
- Total: 87 (matches spec)

### Design Decisions Made
- `construct_context_path` uses simple path join (base / name) -- no implicit "context/" prefix. The caller passes full relative path. This matches Python behavior exactly.
- `apply_provider_preferences` mount_plan.providers is a **list of dicts** (Vec<Value>), each with "module" and "config" keys. NOT a mapping keyed by provider name.
- `sanitize_for_json` and `sanitize_message` both take `&serde_json::Value` (not serde_yaml_ng::Value). Serialization module works with JSON Values.
- `DiskCache.cache_key_to_path` is public (Python used `_cache_key_to_path` but needed in tests).
- `get_nested_with_default` is a separate function (not an optional param like Python) since Rust doesn't have default arguments.
- Async spawn tests (resolve_model_pattern, apply_provider_preferences_with_resolution -- 7 tests) excluded from Wave 1 count. Will be added in Wave 2.
- `ModelResolutionResult` needs `available_models: Option<Vec<String>>` field (currently missing). Add in Wave 1 implementation.

### Antagonistic Review Issues Fixed
- Fixed construct_context_path tests to use simple join semantics (was encoding wrong implicit prefix behavior)
- Fixed apply_provider_preferences tests to use list-of-dicts for providers (was using mapping)
- Added missing assertions to git/zip URI parse tests (scheme, host, path, subpath)
- Fixed test_nested_structure to use mixed dict/list nesting matching Python test
- Fixed test_filters_none_values_in_dict to test sanitize_for_json (not sanitize_message)

### Known Minor Issues (Not Blocking)
- test_path_object_input is redundant (identical to test_absolute_path in Rust since normalize_path only takes &str)
- #[should_panic] tests don't check error message content yet (will validate in Wave 1 implementation)
- test_non_serializable_returns_none tests max-depth instead of truly unserializable values (Rust-specific adaptation)

---

## Founding Session -- Wave 0

### Architecture Decisions
- Rust port of amplifier-foundation Python library (8,425 LOC across 48 files -> ~42 Rust files)
- 6-wave progressive architecture: scaffold -> leaf -> mid-tier -> core -> integration -> polish
- PyO3 interop enabled from Wave 0 (`crate-type = ["cdylib", "rlib"]`)
- No WASM target (per Amplifier ecosystem analysis -- zero WASM targets exist)
- `serde_yaml_ng` for YAML (not `serde_yaml` which is archived, not `serde_yml` which is UNSOUND)
- `indexmap` for ordered maps where Python dict ordering matters
- Test porting: 1:1 from Python tests, splitting fine but merging forbidden

### Module Structure
- **Wave 0 (scaffold):** Cargo.toml, 42 empty module files, error types, 235 `#[ignore]` tests
- **Wave 1 (leaf, all sync):** dicts, paths, cache, serialization, tracing_utils, spawn
- **Wave 2 (mid-tier, mixed):** io, sources, mentions, session
- **Wave 3 (core, mostly async):** bundle, registry, modules, updates, validator
- **Wave 4 (integration):** lib.rs re-exports, examples
- **Wave 5 (polish):** integration tests, roundtrip tests, cleanup

### Technology Choices
- `serde_yaml_ng::Value` for dynamic YAML data (replaces Python's dict[str, Any])
- `tokio` for async runtime
- `thiserror` for error types
- `globset` for glob pattern matching
- `reqwest` for HTTP
- `git2` for Git operations
- `uuid` for trace ID generation

### Known Constraints
- `PreparedBundle` async closure pattern (`Box<dyn Fn() -> BoxFuture>`) -- spike in Wave 3
- `serde_yaml_ng::Value` + `#[serde(flatten)]` may have silent data loss -- test in Wave 1
- AmplifierRuntime mock has 14 interaction points -- flag divergence as blocker
- bundle.py + registry.py must be serialized in Wave 3 (accept slower wave)
- PyO3 surface annotations deferred to Wave 4

### Reference Sources
- Python source: `/Users/michaeljabbour/dev/amplifier-foundation/`
- Python tests: `/Users/michaeljabbour/dev/amplifier-foundation/tests/`
- Every session MUST read corresponding Python source before implementing Rust module

### First Batch of Work
- F-001: Cargo scaffold (Cargo.toml, .gitignore, 42 empty module files)
- F-002: Error types + runtime traits (BundleError enum, 7 AmplifierRuntime traits)
- F-003: Port Wave 1 tests + stubs (87 #[ignore] tests for leaf modules)
- F-004: Port Wave 2 tests + stubs (91 #[ignore] tests for mid-tier modules)
- F-005: Port Wave 3 tests + stubs (57 #[ignore] tests for core modules)
