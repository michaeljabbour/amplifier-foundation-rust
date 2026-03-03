# amplifier-foundation-rs -- Context Transfer

> This file is the institutional memory of the project. Updated continuously.
> Each session reads this to understand recent decisions and context.
> Reverse-chronological: newest entries at the top.

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
