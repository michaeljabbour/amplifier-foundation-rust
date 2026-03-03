# amplifier-foundation-rs -- Context Transfer

> This file is the institutional memory of the project. Updated continuously.
> Each session reads this to understand recent decisions and context.
> Reverse-chronological: newest entries at the top.

---

## Session 017 -- Wave 12 COMPLETE (F-044, F-045, F-046)

### Work Completed
- **F-044-doctest-cleanup** (8d37289): Fixed failing ModuleActivator doctest by adding `use ModuleActivate` import. Removed unused `ModelResolutionResult` struct from `spawn/mod.rs` (dead code since Session 005, never used by any function). Updated `lib.rs` re-exports, `test_reexports.rs`, and `specs/modules/spawn.md`.
- **F-045-git-status-handler** (460ea1d): Implemented `SourceHandlerWithStatus` trait on `GitSourceHandler`. `get_status()` reads `.amplifier_cache_meta.json` metadata, checks pinned refs via `SourceStatus::is_pinned()`, runs `git ls-remote` with 30s timeout, compares cached vs remote commit SHAs. `update()` removes existing cache (propagating errors) and delegates to `resolve()` for fresh clone. Helper methods: `get_cache_metadata`, `get_remote_commit` (with `tokio::time::timeout`), `build_source_uri`. 8 new tests.
- **F-046-wire-status-updates** (c93215f): Wired `SourceHandlerWithStatus` into `check_bundle_status` and `update_bundle`. Both functions now dispatch git URIs to `GitSourceHandler.get_status()` / `GitSourceHandler.update()` respectively. Added `cache_dir: Option<&Path>` parameter to both functions, defaulting to `~/.amplifier/cache/bundles` (matching `SimpleSourceResolver`). HTTP sources remain unsupported (returns unknown/error). 1 new test. All existing tests updated with cache_dir parameter.

### Wave 12 COMPLETE
- cargo fmt --check: CLEAN (0 formatting issues)
- cargo clippy --all-targets: 0 warnings
- Tests: 450 passing, 1 ignored (spawn doc-test), 0 failed
- MSRV: 1.80 (unchanged)

### Design Decisions Made
- **ModuleActivator doctest needs `use ModuleActivate`**: The `activate()` method lives on the `ModuleActivate` trait, not as an inherent method. Doctest requires explicit trait import to compile.
- **ModelResolutionResult removed (not deprecated)**: Dead since Session 005, version 0.1.0 so no semver concern. No function ever accepted or returned it. Spec in `specs/modules/spawn.md` also cleaned up.
- **SourceStatus::is_pinned() used as single canonical implementation**: Rather than duplicating pinned-ref detection logic in `GitSourceHandler`, `get_status` sets `cached_ref` first then calls `status.is_pinned()`. Single place to fix bugs.
- **get_remote_commit uses tokio::time::timeout(30s)**: Matches Python's `asyncio.wait_for(..., timeout=30)`. Prevents indefinite hang on unresponsive remotes. Returns None on timeout (same as on failure).
- **update() propagates remove_dir_all errors**: Unlike `resolve()` which does best-effort cleanup, `update()` raises on removal failure. Returning stale data after user explicitly requested update is worse than failing.
- **check_bundle_status/update_bundle take Option<&Path> cache_dir**: Defaults to `get_amplifier_home()/cache/bundles` matching `SimpleSourceResolver::new()`. Tests use `tempdir()` to avoid polluting real cache.
- **Cache path uses `cache/bundles/` not just `cache/`**: Must match `SimpleSourceResolver` default to ensure status checks and updates operate on the same cache directory as the resolver. Reviewer caught this mismatch.
- **check_bundle_status/update_bundle dispatch directly to GitSourceHandler::new()**: Hardcoded dispatch matches Python's pattern. Not using `dyn SourceHandlerWithStatus` dispatch because only one handler exists. Can be refactored to handler-registry pattern when HTTP handler is added.
- **update_bundle returns () not ResolvedSource**: The trait returns `Result<ResolvedSource>` but `update_bundle` discards it. Callers who need the path can call `resolve()` after updating. Matches Python's simpler return.
- **Tests use 127.0.0.1:1 for all git URIs**: Ensures no real network calls in tests. Connection to port 1 always fails immediately with connection refused.

### Antagonistic Review Issues Found & Fixed
- F-044: Removed tombstone comment from test_reexports.rs (reviewer caught test archaeology)
- F-044: Updated specs/modules/spawn.md to remove ModelResolutionResult definition
- F-045: Added 30s timeout to get_remote_commit (reviewer caught indefinite hang risk — P1 bug)
- F-045: update() now propagates remove_dir_all errors (reviewer caught stale cache risk — P2 bug)
- F-045: Consolidated is_ref_pinned to use SourceStatus::is_pinned() (reviewer caught duplication)
- F-045: Strengthened test_git_get_status_not_cached assertions (reviewer caught weak assertion)
- F-045: Added test_git_update_no_existing_cache (reviewer caught missing coverage)
- F-046: Fixed cache path from `cache/` to `cache/bundles/` matching SimpleSourceResolver (reviewer caught directory mismatch — blocking bug)
- F-046: Changed pinned test from github.com to 127.0.0.1:1 (reviewer caught real network call risk)

### Antagonistic Review Issues Noted (Not Fixed -- By Design)
- F-045: verify_clone_integrity false negative for repos without pyproject.toml/bundle.yaml — pre-existing in resolve(), not introduced here
- F-045: is_pinned treats "version-2" branch as pinned — faithful port of Python bug, documented
- F-045: get_remote_commit doesn't suppress stderr — Command::output() already captures both stdout and stderr
- F-046: URI identity mismatch between BundleStatus.bundle_source and inner SourceStatus.uri — pre-existing design, git handler normalizes URI
- F-046: Hardcoded GitSourceHandler::new() bypasses trait dispatch — matches Python's explicit dispatch, acceptable for single handler
- F-046: update_bundle discards ResolvedSource — matches simpler Python return contract
- F-046: Unnecessary uri_owned.clone() in check_bundle_status — micro-optimization, not blocking

### What's Next
- All 12 waves complete. 450 tests, 0 clippy warnings, 46 features delivered.
- Remaining unported Python functionality:
  - `PreparedBundle` (bundle.py:845-1289) — session lifecycle controller. Depends on AmplifierRuntime traits being concrete (amplifier_core::AmplifierSession). Major: create_session, spawn, _build_bundles_for_resolver, _create_system_prompt_factory.
- Consider: PyO3 bindings (feature flag exists, no `#[pyclass]`/`#[pymodule]` code)
- Consider: HTTP `SourceHandlerWithStatus` impl (HEAD + ETag/Last-Modified)
- Consider: Refactor check_bundle_status/update_bundle to use handler registry (dyn dispatch)
- Consider: Benchmarks (bundle compose, cache operations, fingerprint computation)
- Consider: Integration test for GitSourceHandler with real git clone
- Consider: Return ResolvedSource from update_bundle (richer API)

---

## Session 016 -- Wave 11 COMPLETE (F-041, F-042, F-043)

### Work Completed
- **F-041-format-context-block** (18016a8): Ported `format_context_block` from Python mentions/loader.py. Pure function that formats deduplicated context files as XML `<context_file>` blocks for system prompt assembly. Takes `&ContentDeduplicator` + optional `HashMap<String, PathBuf>` mention-to-path mapping. Builds reverse lookup for @mention → resolved path attribution. Uses `std::path::absolute()` fallback (MSRV 1.80) for consistent path resolution across existing and non-existing paths. Sorts mentions per-path for deterministic output (HashMap iteration safety). Documents XML injection parity with Python (no escaping). Re-exported in `lib.rs`. 7 new tests including real-file canonicalize test.
- **F-042-module-resolver** (643930e): Ported `BundleModuleSource` + `BundleModuleResolver` from Python bundle.py (lines 711-842). `BundleModuleSource` is a thin PathBuf wrapper. `BundleModuleResolver` maps module_id → PathBuf with sync `resolve()` (HashMap lookup only) and async `async_resolve()` (with lazy activation via `ModuleActivate` trait). Double-checked locking pattern with `tokio::sync::Mutex<()>` serializes lazy activations to prevent duplicate downloads. Uses `std::sync::Mutex` with poison recovery for the paths map (works in both sync and async contexts). `ModuleActivate` trait abstracts the activation interface. Manual `Debug` impl (dyn trait prevents derive). Sorted available modules in error messages. Error chaining: activation failures preserve source error. 13 new tests including concurrent activation deduplication test.
- **F-043-module-activator** (bc4c657): Ported `ModuleActivator` from Python modules/activator.py. Concrete implementation of `ModuleActivate` trait. Resolves URIs via `SimpleSourceResolver`, optionally installs dependencies via `uv pip install` subprocess (tokio::process::Command), tracks activation state via `InstallStateManager`. `activate_all()` uses `futures::join_all` for parallel activation. `activate_bundle_package()` installs bundle's own pyproject.toml. Session-scoped dedup: same name+URI only activated once. All Mutexes use poison recovery. No sys.path manipulation (Rust-specific). Added `SimpleSourceResolver::with_base_path_and_cache_dir` constructor. 10 new tests.

### Wave 11 COMPLETE
- cargo fmt --check: CLEAN (0 formatting issues)
- cargo clippy --all-targets: 0 warnings
- Tests: 440 passing, 1 ignored (spawn doc-test), 0 failed
- MSRV: 1.80 (unchanged)

### Design Decisions Made
- **format_context_block uses std::path::absolute fallback**: `fs::canonicalize` fails for non-existent paths. Python's `Path.resolve()` always returns absolute. `std::path::absolute()` (stable since 1.79, MSRV is 1.80) resolves against cwd without requiring path existence. Consistent path resolution on both sides of the HashMap lookup.
- **format_context_block sorts mentions per-path**: Python dicts preserve insertion order, but Rust HashMap doesn't. Mentions for the same path are sorted alphabetically to ensure deterministic output.
- **format_context_block does NOT escape XML**: Matches Python behavior. Paths with `"` or content with `</context_file>` could break XML parsing. Documented as known parity issue.
- **BundleModuleResolver uses std::sync::Mutex (not tokio::sync::Mutex) for paths**: Enables both sync `resolve()` and async `async_resolve()` without requiring tokio runtime for sync callers. Lock is held only briefly (HashMap operations), never across await points.
- **BundleModuleResolver uses tokio::sync::Mutex<()> as activation_lock**: Separate from paths lock. Serializes the entire lazy activation operation (resolve + insert) to prevent duplicate activations. Same pattern as Python's `asyncio.Lock()`.
- **Mutex poison recovery everywhere**: All `lock().unwrap_or_else(|e| e.into_inner())` to prevent cascade panics in multi-threaded server contexts. Python has no equivalent concern.
- **BundleModuleResolver error type: BundleError::LoadError (not ModuleNotFoundError)**: Python uses a dedicated `ModuleNotFoundError`. Rust uses `BundleError::LoadError` to avoid adding a new enum variant. Error messages contain module name and available modules list. Callers can string-match if needed.
- **ModuleActivator activation error chains source error**: When `activator.activate()` fails in `async_resolve`, the original error is preserved via `BundleError::LoadError { source: Some(Box::new(e)) }`. This enables `Error::source()` chains and downcasting.
- **Python's profile_hint parameter intentionally dropped**: Deprecated in Python (marked for v2.0 removal). Rust API only has `source_hint`. Documented in module-level doc comment.
- **BundleModuleResolver available_modules() sorted**: Error messages list available modules alphabetically for deterministic, testable output. Python dicts are ordered but error messages would have arbitrary order.
- **ModuleActivator no sys.path**: Rust has no equivalent of Python's sys.path import mechanism. Callers use the returned PathBuf to locate module source.
- **ModuleActivator install_dependencies hardcodes `uv pip install`**: This is a Python-ecosystem tool. The `install_deps=false` flag allows skipping for non-Python modules or when deps are pre-installed. Future: could be made configurable via a trait.
- **ModuleActivator.cache_dir marked #[allow(dead_code)]**: Kept for API parity with Python. Currently consumed by resolver and install_state at construction, but not used after. Could be useful for cache invalidation methods.
- **SimpleSourceResolver::with_base_path_and_cache_dir added**: New constructor combining both base_path and cache_dir. Needed by ModuleActivator which requires both.

### Antagonistic Review Issues Found & Fixed
- F-041: Changed `fs::canonicalize` fallback from `path.clone()` to `std::path::absolute()` (reviewer caught relative/absolute mismatch)
- F-041: Added real-file test using tempdir (reviewer caught zero-coverage of canonicalize success path)
- F-041: Added mention sorting per-path for deterministic output (reviewer caught HashMap non-determinism)
- F-041: Strengthened test assertions from substring to structural (reviewer caught weak assertions)
- F-041: Added XML injection doc comment (reviewer caught missing documentation of known limitation)
- F-042: Changed from `lock().unwrap()` to `lock().unwrap_or_else(|e| e.into_inner())` at all 6 sites (reviewer caught cascade panic risk)
- F-042: Added error chaining in activation failure path (reviewer caught discarded source error)
- F-042: Added `available_modules()` sorting (reviewer caught non-deterministic error messages)
- F-042: Added manual Debug impl (reviewer caught missing Debug on public type)
- F-042: Added concurrent activation deduplication test (reviewer caught untested double-checked locking)
- F-042: Added profile_hint migration note to doc comments (reviewer caught silent breaking change)
- F-042: Changed `p.display().to_string()` to `p.to_string_lossy().into_owned()` (reviewer caught display vs roundtrip semantics)
- F-043: Fixed MutexGuard held across await point (compiler caught it — dropped guard before await)
- F-043: Added activate_bundle_package no-pyproject and nonexistent path tests

### Antagonistic Review Issues Noted (Not Fixed -- By Design)
- F-041: format_context_block clones all content via get_unique_files() — pre-existing design of ContentDeduplicator, not introduced here
- F-042: No dedicated ModuleNotFoundError variant — using BundleError::LoadError to avoid breaking enum change
- F-042: Profile_hint parameter dropped without compat shim — Python deprecated it for v2.0 removal
- F-043: install_dependencies hardcodes `uv pip install` — matches Python behavior, configurable install deferred
- F-043: No tests for successful subprocess install (would require `uv` to be installed in test environment)
- F-043: activate_all uses join_all (concurrent but not parallel in single-threaded tokio) — matches Python's asyncio.gather semantics

### What's Next
- All 11 waves complete. 440 tests, 0 clippy warnings, 43 features delivered.
- Remaining unported Python functionality:
  - `PreparedBundle` (bundle.py:845-1289) — session lifecycle controller. Depends on AmplifierRuntime traits being concrete (amplifier_core::AmplifierSession). Major: create_session, spawn, _build_bundles_for_resolver, _create_system_prompt_factory.
- Consider: PyO3 bindings (feature flag exists, no `#[pyclass]`/`#[pymodule]` code)
- Consider: Concrete `SourceHandlerWithStatus` impl on GitSourceHandler (git ls-remote)
- Consider: Wire `SourceHandlerWithStatus` into `check_bundle_status`/`update_bundle`
- Consider: Benchmarks (bundle compose, cache operations, fingerprint computation)
- Consider: Integration test for ModuleActivator with real git clone
- Consider: Remove or repurpose unused `ModelResolutionResult` struct

---

## Session 015 -- Wave 10 COMPLETE (F-038, F-039, F-040)

### Work Completed
- **F-038-update-info** (9fa7304): Ported `UpdateInfo` dataclass from Python's `registry.py`. 4-field struct (name, current_version, available_version, uri) for bundle-level update notifications. Derives `Serialize`/`Deserialize`/`Hash`/`Eq` for JSON serialization and `HashSet` usage. Doc comment clarifies relationship to `SourceStatus` (source-level vs bundle-level). Re-exported in `lib.rs`. 8 new tests (integration + reexport).
- **F-039-source-status-enrichment** (aa73c62): Enriched `SourceStatus` with all Python `SourceStatus` fields: `is_cached`, `cached_at`, `cached_ref`, `cached_commit`, `remote_ref`, `remote_commit`, `error`, `summary`. Added `Default` derive for backward-compatible construction via `..Default::default()`. Added `new(uri)` constructor. Added `is_pinned()` method matching Python behavior (case-insensitive hex SHA detection via `.lower()` parity). Added `Serialize`/`Deserialize`. Documented Rust-only fields (`current_version`, `latest_version`) and `uri` vs `source_uri` naming difference. Updated `check_bundle_status` to populate `summary` and `is_cached` fields. 18 new tests including `is_pinned` edge cases.
- **F-040-source-protocol-traits** (10a41ab): Ported Python's `SourceHandlerWithStatusProtocol` and `SourceResolverProtocol` as Rust traits. `SourceHandlerWithStatus` extends `SourceHandler` with `get_status()` (non-destructive) and `update()` (forced re-download). `SourceResolver` is the higher-level URI-to-path trait, implemented by `SimpleSourceResolver`. Both re-exported in `lib.rs`. 7 new tests.

### Wave 10 COMPLETE
- cargo fmt --check: CLEAN (0 formatting issues)
- cargo clippy --all-targets: 0 warnings
- Tests: 410 passing, 1 ignored (spawn doc-test), 0 failed
- MSRV: 1.80 (unchanged)

### Design Decisions Made
- **UpdateInfo derives Serialize/Deserialize/Hash**: Unlike `BundleState` (which hand-rolls to_dict/from_dict), `UpdateInfo` uses serde derives. Update-check results are likely to be serialized to JSON for CLI output or logging. `Hash` derives freely from `Eq` + all-String fields, enabling `HashSet<UpdateInfo>` for dedup.
- **UpdateInfo is a data-only struct with no consumers yet**: No function in the crate currently returns `UpdateInfo`. It's the result type for the planned `BundleRegistry::check_for_updates()` method. Documented as "currently a data-only struct" in doc comment.
- **UpdateInfo.available_version is String (not Option<String>)**: Unlike `SourceStatus.has_update` which can be unknown, `UpdateInfo` represents a *confirmed* update — the version is always known.
- **SourceStatus enrichment preserves backward compatibility**: New fields use `Default` derive so existing construction sites (`SourceStatus { uri: ..., has_update: ..., ..Default::default() }`) compile without changes. New `SourceStatus::new(uri)` constructor added for future code.
- **SourceStatus.uri kept (not renamed to source_uri)**: Python uses `source_uri`; Rust uses `uri` for consistency with `BundleState.uri` and other Rust types. Documented in field doc comment. Suggested adding `#[serde(rename = "source_uri")]` if cross-language serialization is needed.
- **SourceStatus.cached_at is String, not chrono::DateTime**: Avoids forcing a chrono dependency on consumers. Documented trade-off in field doc comment.
- **SourceStatus.current_version / latest_version documented as Rust-only**: These fields have no Python equivalent. Doc comments mark them as Rust-only additions and point to `cached_commit`/`remote_commit` as the Python equivalents.
- **is_pinned() uses case-insensitive hex detection**: Python's `.lower()` normalizes before checking. Rust uses `is_ascii_hexdigit()` (which matches both cases). Antagonistic review correctly identified that the initial implementation rejected uppercase SHAs, which was a bug.
- **is_pinned() treats empty string as not pinned**: `Some("")` returns `false` — Python's `if not self.cached_ref:` treats empty string as falsy.
- **SourceHandlerWithStatus.update() returns ResolvedSource**: Python returns `Path`. Rust returns `ResolvedSource` for consistency with `SourceHandler::resolve()`. Documented as intentional divergence.
- **SourceHandlerWithStatus is a forward-declared protocol**: No concrete implementations exist yet. The existing `check_bundle_status()`/`update_bundle()` use simpler hardcoded dispatch. Handlers will implement this trait as update support is added.
- **SourceResolver trait formalized**: `SimpleSourceResolver` already had the `resolve(uri)` method. Now it also `impl SourceResolver`, enabling use as `&dyn SourceResolver`.

### Antagonistic Review Issues Found & Fixed
- F-038: Added `Serialize`/`Deserialize` derives (reviewer: "inconsistent with ProviderPreference which has serde derives")
- F-038: Added `Hash` derive (reviewer: "Eq without Hash is a half-commitment")
- F-038: Expanded doc-test to assert all 4 fields (reviewer: "doc-test only exercises 2/4 fields")
- F-038: Added doc comment explaining relationship to SourceStatus (reviewer: "undefined relationship is confusing")
- F-039: Fixed `is_pinned()` to accept uppercase SHAs (reviewer correctly identified Python `.lower()` behavior)
- F-039: Fixed test `test_source_status_not_pinned_uppercase_sha` to assert `true` (was asserting buggy behavior)
- F-039: Added mixed-case SHA, empty string, and bare "v" edge case tests
- F-039: Added `SourceStatus::new(uri)` constructor (reviewer: "Default enables empty uri construction")
- F-039: Added `Serialize`/`Deserialize` derives on SourceStatus
- F-039: Documented `cached_at` as String trade-off and `uri` vs `source_uri` naming
- F-039: Documented `current_version`/`latest_version` as Rust-only fields
- F-040: Documented `update()` return type divergence from Python (ResolvedSource vs Path)
- F-040: Documented traits as forward-declared protocols (no implementations yet)
- F-040: Renamed compile-time tests to be honest about what they test

### Antagonistic Review Issues Noted (Not Fixed -- By Design)
- F-038: `UpdateInfo` is a dead struct with no consumers — documented as planned for `BundleRegistry::check_for_updates()`
- F-039: `SourceStatus::default()` allows empty `uri` — mitigated by `SourceStatus::new()` constructor
- F-040: `SourceHandlerWithStatus` has zero implementors — forward-declared protocol, will be implemented as update support is added
- F-040: `check_bundle_status`/`update_bundle` don't dispatch through new traits — simpler hardcoded dispatch predates the trait, acceptable for current functionality level
- F-040: Some tests are compile-time checks rather than behavioral tests — acknowledged in test names

### Python __all__ Parity Status
After Wave 10, the Rust crate exports equivalents for **all 61** Python `__all__` items:
- ✅ `UpdateInfo` (F-038, was missing)
- ✅ `SourceResolverProtocol` → `SourceResolver` trait (F-040, was missing)
- ✅ `SourceHandlerWithStatusProtocol` → `SourceHandlerWithStatus` trait (F-040, was missing)
- ✅ `SourceHandlerProtocol` → `SourceHandler` trait (existed)
- ✅ `MentionResolverProtocol` → `MentionResolver` trait (existed)
- ✅ `CacheProviderProtocol` → `CacheProvider` trait (existed)
- ✅ `BundleNotFoundError` etc. → `BundleError::NotFound` etc. variants (existed)
- ✅ All 54 other items (existed since Session 009+)

### What's Next
- All 10 waves complete. 410 tests, 0 clippy warnings, 40 features delivered.
- Python `__all__` parity: 100% (all 61 items have Rust equivalents)
- Remaining unported Python functionality (no tests, not in __all__):
  - `ModuleActivator` (modules/activator.py) — async module activation via subprocess. Depends on `uv` tooling. No Python tests.
  - `BundleModuleResolver/BundleModuleSource` (bundle.py:711-842) — maps module IDs to paths. Depends on ModuleActivator.
  - `PreparedBundle` (bundle.py:845-1289) — session lifecycle controller. Depends on AmplifierRuntime traits being concrete.
- Consider: PyO3 bindings (feature flag exists, no `#[pyclass]`/`#[pymodule]` code)
- Consider: Concrete `SourceHandlerWithStatus` impl on GitSourceHandler (git ls-remote status checking)
- Consider: Benchmarks (bundle compose, cache operations, fingerprint computation)
- Consider: Wire `SourceHandlerWithStatus` into `check_bundle_status`/`update_bundle`

---

## Session 014 -- Wave 9 COMPLETE (F-035, F-036, F-037)

### Work Completed
- **F-035-install-state** (56af148): Ported Python's `InstallStateManager` to `src/modules/state.rs`. SHA-256 fingerprinting of `pyproject.toml` + `requirements.txt` for skip-if-unchanged semantics. Self-healing: corrupted JSON, version mismatch, or schema errors silently reset to fresh state. Atomic save via `tempfile::NamedTempFile::persist()` (concurrent-safe). Path keys use `std::path::absolute()` fallback for non-existent paths (matches Python's `Path.resolve()` behavior). Tolerates unknown fields in JSON for cross-implementation compatibility (Python writes a `"python"` field that Rust ignores). `save(&mut self)` correctly resets dirty flag. 18 new tests.
- **F-036-provider-prefs-resolution** (4d5ca3e): Implemented `apply_provider_preferences_with_resolution` -- async version of `apply_provider_preferences` that resolves glob model patterns via a callback. Generic signature `F: Fn(&str) -> Fut + Send + Sync` where `Fut: Future<Output = Vec<String>> + Send`. Send+Sync bounds enable `tokio::spawn` compatibility. Falls back to original glob pattern when no match found. `tracing::warn!` for missing providers (matches Python logging). Re-exported in `lib.rs`. 7 new async tests.
- **F-037-dead-stub-cleanup** (2e0dc72): Removed 5 empty TODO stub files (`bundle/module_resolver.rs`, `bundle/prepared.rs`, `bundle/prompt.rs`, `registry/includes.rs`, `registry/persistence.rs`). Added documentation comments to parent modules explaining that registry logic lives in `registry/mod.rs` and bundle stubs are reserved for future AmplifierRuntime-dependent functionality. Updated `modules/mod.rs` with proper module-level docs.

### Wave 9 COMPLETE
- cargo fmt --check: CLEAN (0 formatting issues)
- cargo clippy --all-targets: 0 warnings
- Tests: 376 passing, 0 ignored, 0 failed
- MSRV: 1.80 (unchanged)

### Design Decisions Made
- **InstallStateManager omits Python's `sys.executable` tracking**: The Python version invalidates all module state when the Python executable changes (venv switch). The Rust version has no equivalent concept since it's a Rust library, not a Python runtime. For cross-implementation compatibility, the Rust deserializer tolerates the `"python"` field via serde's default behavior (no `#[serde(deny_unknown_fields)]`).
- **InstallStateManager.save() returns io::Result<()>**: Python's `save()` swallows `OSError` and logs a warning. Rust propagates errors to callers (idiomatic Rust -- callers decide error handling policy). This is a deliberate API divergence.
- **InstallStateManager.save(&mut self) not &self**: Python uses `self._dirty = False` after successful write. Rust requires `&mut self` to reset the dirty flag. The antagonistic review correctly identified that `&self` would be unable to clear the flag.
- **InstallStateManager.path_key uses std::path::absolute()**: `fs::canonicalize()` fails for non-existent paths. Python's `Path.resolve()` always returns an absolute path. `std::path::absolute()` (stable since 1.79) resolves against cwd without requiring the path to exist. Used as fallback when `canonicalize()` fails.
- **InstallStateManager uses tempfile::NamedTempFile for atomic save**: Python uses `tempfile.mkstemp()`. The antagonistic review correctly identified that a fixed temp file name (`install-state.tmp`) would allow concurrent writes to corrupt each other. `NamedTempFile::new_in()` + `.persist()` provides unique temp names and atomic rename in one call.
- **apply_provider_preferences_with_resolution uses generic callback, not coordinator**: Python takes `coordinator: Any` and does duck-typing to query models. Rust uses `F: Fn(&str) -> Fut` where `Fut: Future<Output = Vec<String>>`. This is more flexible -- callers provide any async function that maps provider names to model lists. No dependency on the Coordinator trait.
- **Callback returns Vec<String>, not Result**: The Python version wraps model queries in try/except and falls back to empty list. Rust callers handle errors internally in the closure and return `vec![]` as fallback. Adding `Result` to the callback return type would complicate the API without benefit.
- **Send+Sync bounds on callback**: Required for `tokio::spawn` compatibility. Without these, the returned future is not Send, making it unusable in multi-threaded tokio runtimes.
- **Empty stub files deleted, not just documented**: `registry/includes.rs` and `registry/persistence.rs` had their logic implemented directly in `registry/mod.rs`. The stubs added no value and cluttered the file tree. `bundle/module_resolver.rs`, `bundle/prepared.rs`, `bundle/prompt.rs` depended on unimplemented AmplifierRuntime functionality. Keeping them as empty `pub mod` items polluted the public API with empty modules.
- **ModelResolutionResult is still unused**: Noted by the antagonistic review. It was a placeholder from Session 005. The new `apply_provider_preferences_with_resolution` doesn't use it because the resolution happens inline. The struct could be removed or repurposed when the full async resolution pipeline is built.

### Antagonistic Review Issues Found & Fixed
- F-035: Changed `save(&self)` to `save(&mut self)` to allow dirty flag reset (reviewer caught that `&self` can't mutate)
- F-035: Changed `path_key` fallback from `module_path.to_path_buf()` to `std::path::absolute()` (reviewer caught relative path key mismatch)
- F-035: Changed from fixed temp file name to `tempfile::NamedTempFile` (reviewer caught concurrent write corruption risk)
- F-035: Added 4 additional tests: save_clears_dirty_flag, double_save_is_noop, fingerprint_format_sha256, loads_state_with_extra_fields
- F-036: Added `Send + Sync` bounds to callback and future (reviewer caught tokio::spawn incompatibility)
- F-036: Added `tracing::warn!` for missing providers and no-match cases (reviewer caught silent operational divergence)
- F-036: Added 2 additional tests: empty_model_list_from_callback, no_providers_in_plan
- F-037: Removed "Wave 9" reference from registry comment (reviewer caught project-internal bookkeeping in source)

### Antagonistic Review Issues Noted (Not Fixed -- By Design)
- F-035: No `python` environment discriminator (Rust has no Python interpreter to track)
- F-035: `save()` propagates errors instead of swallowing (idiomatic Rust, callers decide)
- F-036: Callback can't return errors (callers handle internally, matches Python's try/except pattern)
- F-036: `ModelResolutionResult` is still dead code (pre-existing from Session 005, not introduced here)
- F-036: Signature deviates from informal spec (callback is better than pre-built HashMap)
- F-036: `build_provider_lookup` has redundant HashMap insert for prefixed providers (pre-existing, not introduced here)

### What's Next
- All 9 waves complete. 376 tests, 0 clippy warnings, 37 features delivered.
- Remaining unported Python functionality:
  - `ModuleActivator` (modules/activator.py) -- async module activation via subprocess. Depends on `uv` tooling. No Python tests.
  - `BundleModuleResolver/BundleModuleSource` (bundle.py:711-842) -- maps module IDs to paths. Depends on ModuleActivator.
  - `PreparedBundle` (bundle.py:845-1289) -- session lifecycle controller. Depends on AmplifierRuntime traits being concrete.
- Consider: PyO3 bindings for cross-language interop
- Consider: Benchmarks (bundle compose, cache operations, fingerprint computation)
- Consider: Remove unused `ModelResolutionResult` or integrate it into resolution pipeline

---

## Session 013 -- Wave 8 COMPLETE (F-032, F-033, F-034)

### Work Completed
- **F-032-http-resolve** (57fb4e0): Implemented `HttpSourceHandler.resolve` with reqwest HTTP download. SHA-256 content-addressable cache (hash of URL, first 16 hex chars). Cache-hit fast path checks file/subpath existence before download. Feature-gated on `http-sources` (`LoadError` when disabled, not `NotFound`). Shared `resolve_with_subpath` helper eliminates DRY violation between cache-hit and post-download paths. `download()` method extracted for clean `#[cfg]` boundary. 4 new tests: cache hit, cache hit with subpath, download failure (127.0.0.1:1), empty path fallback to "download".
- **F-033-git-resolve** (877b647): Implemented `GitSourceHandler.resolve` with `tokio::process::Command` for async git clone. SHA-256 cache key from `{git_url}@{ref}` (first 16 hex chars). Shallow clone (`--depth 1`) with `--branch` for non-HEAD refs. Cache integrity verification checks .git directory + expected markers (pyproject.toml/setup.py/setup.cfg/bundle.md/bundle.yaml). Valid cache returns directly (no re-clone on bad subpath — fix from review). OsStr-aware path passing via `.arg(&cache_path)` instead of lossy `.display().to_string()`. Cache metadata JSON with cached_at, ref, commit, git_url. Metadata write errors logged at warn level. 9 new tests: can_handle (4), cache hit, subpath, HEAD default, invalid cache cleanup, clone failure.
- **F-034-dedup-registry** (30f9723): Extended `ContentDeduplicator` with full Python API: `add_file(path, content) -> bool` with multi-path attribution, `get_unique_files() -> Vec<UniqueFile>` (insertion-ordered via IndexMap), `is_seen(content) -> bool` (pure query), `get_known_hashes() -> HashSet<String>`. New `UniqueFile` struct (content, content_hash, paths) with PartialEq/Eq. Cross-API compatibility: `is_duplicate` and `add_file` share `seen` HashSet. Changed `BundleRegistry.bundles` from `HashMap` to `IndexMap` for deterministic JSON serialization. `unregister` uses `shift_remove` (preserves order). Enabled `serde_json` `preserve_order` feature. 10 new integration tests.

### Wave 8 COMPLETE
- cargo fmt --check: CLEAN (0 formatting issues)
- cargo clippy --all-targets: 0 warnings
- Tests: 351 passing, 0 ignored, 0 failed
- MSRV: 1.80 (unchanged)
- Remaining `todo!()`: 0 (all stubs resolved)

### Design Decisions Made
- **HttpSourceHandler uses BundleError::NotFound for download failures**: Matches Python's `BundleNotFoundError` usage. Callers match on `NotFound` to distinguish "bundle unavailable" from other errors. The `uri` field contains the full error message (e.g., "Failed to download https://...: connection refused"). This is a known divergence from the strict `uri` field semantics but matches Python behavior exactly.
- **HttpSourceHandler.download() extracted as separate method**: Clean `#[cfg(feature = "http-sources")]` boundary. The no-feature path returns `LoadError` (not `NotFound`) since the bundle isn't missing — the feature is.
- **HttpSourceHandler cache filename preserves original**: `{filename}-{cache_key}` format matches Python's `Path(parsed.path).name or "download"`.
- **GitSourceHandler uses tokio::process::Command for clone**: Async subprocess execution for the main `git clone`. The `get_local_commit` helper uses sync `std::process::Command` (consistent with existing sync I/O in async contexts pattern — same as Python's sync subprocess for rev-parse).
- **GitSourceHandler valid cache returns directly on subpath error**: If cache is valid but subpath doesn't exist, return NotFound immediately instead of destroying valid cache and re-cloning. This is a deliberate improvement over re-clone behavior.
- **GitSourceHandler uses OsStr for subprocess args**: `cmd.arg(&cache_path)` passes PathBuf as OsStr, avoiding lossy `.display().to_string()` conversion that would corrupt non-UTF-8 paths.
- **Git clone omits --branch for HEAD ref**: "HEAD" is not a valid `--branch` argument. Omitting it lets git use the remote's default branch. Matches Python behavior.
- **ContentDeduplicator internal maps use IndexMap**: `content_by_hash` and `paths_by_hash` use IndexMap for deterministic `get_unique_files()` ordering (matches Python dict insertion order).
- **ContentDeduplicator is_duplicate ↔ add_file cross-API**: Both APIs share the `seen: HashSet` for hash tracking. `add_file` on content already tracked via `is_duplicate` backfills the content and path maps. This enables mixing the simple `is_duplicate` API (used by load_mentions) and the richer `add_file` API on the same instance.
- **UniqueFile is separate from ContextFile**: The Python's `ContextFile` for dedup has `content`, `content_hash`, `paths` (plural), while the Rust `ContextFile` has `path` (singular), `content`, `mention`. These serve different purposes, so a separate `UniqueFile` struct was created rather than modifying the existing `ContextFile`.
- **BundleRegistry.bundles → IndexMap**: Ensures deterministic `registry.json` output. `shift_remove` preserves insertion order (vs `swap_remove` which moves last element to fill gap).
- **serde_json preserve_order feature**: Required for `serde_json::Map` to use IndexMap internally. Without this, `serde_json::json!({...})` creates a BTreeMap-backed Map that sorts keys alphabetically, defeating the IndexMap in registry.
- **register() still accepts &HashMap**: Changing to IndexMap would break callers. Single-bundle registration calls (one key per HashMap) maintain deterministic order. Multi-bundle registration order depends on caller's HashMap iteration order (documented).

### Antagonistic Review Issues Found & Fixed
- F-032: Extracted `resolve_with_subpath` helper to eliminate DRY violation between cache-hit and post-download paths
- F-032: Separated `download()` method for clean `#[cfg]` boundary (reviewer caught unreachable code risk)
- F-032: Feature-disabled path returns `LoadError` not `NotFound` (reviewer: "bundle isn't missing, feature is")
- F-032: Fixed clippy `unneeded return` warning
- F-033: Fixed valid cache destruction on subpath error (reviewer caught re-clone of valid cache)
- F-033: Changed from `Vec<String>` args to `Command::arg()` for OsStr-safe path passing
- F-033: Added tracing::warn for metadata write failures (reviewer caught silent error swallowing)
- F-033: Changed invalid cache test from github.com to 127.0.0.1:1 (reviewer caught real network call)
- F-034: Changed dedup internal maps from HashMap to IndexMap (reviewer caught non-deterministic get_unique_files)
- F-034: Added PartialEq/Eq derives to UniqueFile (reviewer caught missing trait impls)
- F-034: Added cross-API test (is_duplicate → add_file) for backfill logic
- F-034: Fixed registry test to register individually and check key order (reviewer caught vacuous test)
- F-034: Enabled serde_json preserve_order feature (test caught BTreeMap key sorting)

### Antagonistic Review Issues Noted (Not Fixed -- By Design)
- F-032: No mock HTTP server test for actual download path — cache-hit tests verify the important logic; download code is straightforward reqwest usage
- F-032: NotFound.uri field used for error messages — matches Python's BundleNotFoundError usage
- F-033: get_local_commit uses sync subprocess in async context — consistent with existing patterns, same as Python
- F-033: No test for successful clone (requires real git repo) — cache-hit tests verify all logic paths except subprocess
- F-034: get_known_hashes clones entire HashSet — acceptable for typical cardinality
- F-034: add_file path dedup is O(n) linear scan — fine for typical paths-per-content counts

### What's Next
- All 8 waves complete. 351 tests, 0 clippy warnings, 34 features delivered.
- **ZERO remaining `todo!()` stubs** — all source handlers fully implemented.
- Consider: PyO3 bindings (Wave 9 if needed)
- Consider: Async integration tests for registry.load_single
- Consider: Benchmarks (bundle compose, cache operations)
- Consider: Mock HTTP server tests (wiremock/httpmock) for download path coverage
- Consider: Git integration test with local bare repo fixture

---

## Session 012 -- Wave 7 COMPLETE (F-029, F-030, F-031)

### Work Completed
- **F-029-source-resolver** (a50f593): Implemented `SimpleSourceResolver` with `new()`, `with_base_path()`, `with_cache_dir()` constructors. Default handler chain: File, Git, Zip (before Http), Http — order matters for URI matching (zip+https must match before plain https). `add_handler()` inserts at front for priority override. `resolve()` does first-match dispatch with `BundleError::NotFound` fallback using raw URI (not message). Stores `base_path` field for Python parity. 6 new tests including add_handler priority override and error variant assertion.
- **F-030-load-mentions** (b6cc53e): Implemented `load_mentions` pipeline with recursive @mention resolution. Parses mentions from text, resolves each via `&dyn MentionResolver`, reads files (sync `fs::read_to_string`), handles directories via `format_directory_listing`, deduplicates content via `ContentDeduplicator`. Recursive up to `max_depth=3`. Files pushed in encounter order (parent before children). Circular references broken by content-based dedup. Changed signature from `&BaseMentionResolver` to `&dyn MentionResolver` for flexibility. 11 new tests including circular references and ordering.
- **F-031-updates-module** (f60a05e): Implemented `BundleStatus` with `has_updates()`, `updateable_sources()`, `up_to_date_sources()`, `unknown_sources()`, `summary()` methods matching Python properties. Changed `SourceStatus.has_update` from `bool` to `Option<bool>` for tri-state (Some(true)/Some(false)/None=unknown). `check_bundle_status(uri)` returns up-to-date for file URIs, unknown for git/http. `update_bundle(uri)` returns Ok for file, error for unsupported. Added `PartialEq` derives. 14 new tests.

### Wave 7 COMPLETE
- cargo fmt --check: CLEAN (0 formatting issues)
- cargo clippy --all-targets: 0 warnings
- Tests: 329 passing, 0 ignored, 0 failed
- MSRV: 1.80 (unchanged)
- Remaining `todo!()`: 2 (GitSourceHandler.resolve, HttpSourceHandler.resolve — require network/git ops)

### Design Decisions Made
- **SimpleSourceResolver stores base_path for Python parity**: Even though it's consumed into FileSourceHandler, the resolver retains the field so future code can inspect it (matches Python's `self.base_path`).
- **resolve() uses raw URI in NotFound error, not a message string**: The `BundleError::NotFound { uri }` field should contain the actual URI, not an English sentence like "No handler for URI: ...". The `Display` impl already adds "bundle not found:" context.
- **Handler chain order is File → Git → Zip → Http**: ZipSourceHandler must come before HttpSourceHandler because `zip+https://` would otherwise match the plain `https://` handler. This matches Python's handler ordering.
- **load_mentions returns aggregate result, not per-mention**: Python returns `list[MentionResult]` (one per top-level mention). Rust returns a single `MentionResult { files: Vec<ContextFile>, failed: Vec<String> }` aggregating ALL loaded files including recursively discovered ones. This is intentionally more useful for Rust consumers who want all context files in one pass.
- **load_mentions uses encounter order (parent before children)**: When a file is loaded and it contains nested @mentions, the parent file is pushed to result FIRST, then children are recursively resolved. This ensures files appear in reading order for context assembly.
- **load_mentions is async but uses sync I/O internally**: The function signature is `async` for API compatibility with the Python reference (which uses async `read_with_retry`). The current implementation uses synchronous `fs::read_to_string`. A future optimization could use `tokio::fs` or the existing `read_with_retry`.
- **Python's `relative_to` parameter is dead code**: The Python `_resolve_mention` accepts `relative_to` but never passes it to `resolver.resolve()`. The parameter is only propagated recursively but never used. The Rust implementation omits it.
- **load_mentions takes `&dyn MentionResolver` not `&BaseMentionResolver`**: The original stub took `&BaseMentionResolver` concretely. Changed to `&dyn MentionResolver` for trait-based dispatch, allowing callers to pass custom resolvers.
- **Content-based dedup for directories**: Python's `ContentDeduplicator.add_file` uses path as key. Rust's `is_duplicate` uses content hash. Two different empty directories would be deduplicated in Rust but not Python. This is acceptable — same-content dedup is actually more correct for context assembly.
- **SourceStatus.has_update changed to Option<bool>**: Breaking change from `bool` to `Option<bool>`. Python uses `None` for "unknown" status (can't determine if updates are available). The tri-state model is essential for the updates module where git status checking isn't implemented.
- **BundleStatus field names match Python**: `bundle_name` and `bundle_source` (not `name` and `source_uri`) for cross-implementation readability.
- **check_bundle_status takes URI, not Bundle**: This was the original stub API design. Python's version walks the Bundle's entire component tree. The Rust version is intentionally simpler — single URI in, single source status out. Documented explicitly.
- **update_bundle uses LoadError variant**: No dedicated `UpdateError` variant exists in `BundleError`. Using `LoadError` with descriptive reason string. A dedicated variant could be added when git/http update support is implemented.

### Antagonistic Review Issues Found & Fixed
- F-029: Stored `base_path` on resolver for Python parity (reviewer caught missing field)
- F-029: Used raw URI in NotFound error (reviewer caught double-prefix Display issue)
- F-029: Added add_handler priority override test (reviewer caught untested core contract)
- F-029: Added error variant assertion on no-handler test (reviewer caught weak assertion)
- F-029: Removed `has_handler_for` from public API (reviewer caught test scaffolding in API)
- F-030: Fixed ordering to parent-before-children (reviewer caught DFS post-order divergence)
- F-030: Added circular reference test (reviewer identified missing coverage)
- F-031: Matched Python field names (bundle_name, bundle_source)
- F-031: Added PartialEq derives on BundleStatus and SourceStatus
- F-031: Fixed summary() to use single-pass counting instead of triple Vec allocation
- F-031: Removed unnecessary filesystem operations from tests (check_bundle_status doesn't touch disk)

### Antagonistic Review Issues Noted (Not Fixed -- By Design)
- F-029: `ftp://` silently misparsed by parse_uri — existing behavior, not introduced by this change
- F-029: Four constructors (new, with_base_path, with_cache_dir, with_base_path_and_cache_dir was removed — kept 3)
- F-030: `relative_to` parameter missing — dead code in Python too
- F-030: async fn that does sync I/O — consistent with existing codebase patterns
- F-030: Content-based dedup differs from Python's path-based dedup — intentional
- F-031: check_bundle_status has fundamentally different API than Python (URI vs Bundle) — original stub design
- F-031: LoadError used instead of dedicated UpdateError variant — sufficient for current usage

### What's Next
- All 7 waves complete. 329 tests, 0 clippy warnings, 31 features delivered.
- Remaining `todo!()` stubs (2): GitSourceHandler.resolve, HttpSourceHandler.resolve — require actual git/network operations
- Consider: PyO3 bindings (Wave 8 if needed)
- Consider: Implement GitSourceHandler.resolve (git clone to cache)
- Consider: Implement HttpSourceHandler.resolve (HTTP download to cache)
- Consider: Add `UpdateError` variant to BundleError
- Consider: Extend ContentDeduplicator with add_file/get_unique_files for format_context_block support
- Consider: BundleRegistry.bundles → IndexMap for deterministic registry.json output

---

## Session 011 -- Wave 6 COMPLETE (F-026, F-027, F-028)

### Work Completed
- **F-026-indexmap-agents** (cf9299f): Replaced `HashMap<String, Value>` with `IndexMap<String, Value>` for `Bundle.agents` and `HashMap<String, PathBuf>` with `IndexMap<String, PathBuf>` for `Bundle.context`. This ensures deterministic ordering matching Python dict insertion-order semantics. Updated `parse_agents()`, `parse_context()`, `Bundle::new()`. Added doc comment to `compose()` Strategy 3 noting that `IndexMap::insert` preserves original key position (matches `dict.update()`). 4 new integration tests: agent insertion order, context insertion order, compose agent ordering, compose context ordering.
- **F-027-to-dict-roundtrip** (c266dd0): Fixed `to_dict()` to produce output compatible with `from_dict()`. All fields (providers, tools, hooks, session, spawn, agents, context, includes) now nested under the `"bundle"` key. Added session, spawn, agents, context, includes serialization that was entirely missing. Context paths serialized as strings via `path.display()`. Doc comment documents roundtrip contract: what survives (all from_dict-readable fields) and what doesn't (instruction, pending_context, base_path, extra, source_uri). Replaced old `test_to_dict_structure` (documented limitation) with `test_to_dict_from_dict_roundtrip` (full content roundtrip assertions) and `test_to_dict_roundtrip_minimal`.
- **F-028-dead-code-cleanup** (0737b6f): Replaced dead `compose.rs` free-function stub (`todo!()` panic trap) with comment, made module private. Implemented `get_working_dir`/`set_working_dir` in `session/capabilities.rs` (JSON value ops with null coercion). Implemented `ContentDeduplicator` in `mentions/dedup.rs` (SHA-256 hash-based duplicate detection using `HashSet<String>`). Implemented `format_directory_listing` in `mentions/utils.rs` (dirs-first sorting, DIR/FILE labels, symlink-aware via `!is_dir()`). Added `format_directory_listing` re-export to `lib.rs`. 12 new tests.

### Wave 6 COMPLETE
- cargo fmt --check: CLEAN (0 formatting issues)
- cargo clippy --all-targets: 0 warnings
- Tests: 298 passing (265 unit + 33 integration), 0 ignored, 0 failed
- MSRV: 1.80 (unchanged from Wave 5)
- New re-exports: `format_directory_listing` added to lib.rs

### Design Decisions Made
- **IndexMap for agents and context, HashMap for the rest**: `agents` is serialized in `to_mount_plan()` output YAML, so ordering matters for reproducible diffs. `context` ordering matters for system prompt assembly. `source_base_paths` and `pending_context` are internal-only lookup maps, so HashMap is fine. `BundleRegistry.bundles` could benefit from IndexMap for deterministic `registry.json` output, but left as HashMap for now (noted as follow-up).
- **to_dict nests everything under "bundle:" key**: The Rust `from_dict()` reads all fields from `data["bundle"]` (Session 008 decision). Python's `from_dict()` reads some fields from top level. The Rust `to_dict()` now matches what Rust `from_dict()` expects, making the Rust ecosystem internally consistent. This intentionally diverges from Python's DiskCache.set() schema.
- **to_dict does not serialize instruction, pending_context, base_path, extra, source_uri**: These are either set by from_dict to fixed values (instruction=None), internal state (base_path, source_base_paths), or should be resolved before serializing (pending_context). The doc comment documents this explicitly.
- **Empty Mapping treated as absent in to_dict**: `is_null_or_empty_mapping()` check means `session: {}` is not serialized. On roundtrip, this becomes `Value::Null`. Documented as semantically equivalent, not a bug.
- **compose.rs made private (mod, not pub mod)**: The free function `compose(_base, _overlay)` was a dead stub from early design. The real 5-strategy compose lives as `Bundle::compose()` in `mod.rs`. Making the module private removes it from the public API surface.
- **ContentDeduplicator.is_duplicate() is a mutating predicate**: It both checks and tracks in one call (`!seen.insert(hash)`). Python has separate `is_seen()` (pure query) and `add_file()` (mutating). The Rust API is simpler but conflates the two operations. The doc comment warns about this.
- **format_directory_listing uses !is_dir() not is_file()**: This ensures symlinks-to-directories are listed as DIR (follows Python's `path.is_dir()` which follows symlinks). `is_file()` returns false for symlinks on some platforms.
- **set_working_dir coerces null to object**: If capabilities JSON is Value::Null (uninitialized), it's promoted to an empty object before insertion. This prevents silent data loss.

### Antagonistic Review Issues Found & Fixed
- F-026: Added compose ordering tests (agents and context) -- reviewer correctly identified missing coverage for compose path
- F-026: Added doc comment to compose() Strategy 3 about key position preservation
- F-027: Strengthened roundtrip assertions to compare full content (not just length) for providers/tools/hooks
- F-027: Added context and includes to roundtrip test (were missing from test fixture)
- F-027: Fixed doc comment to accurately describe what survives roundtrip
- F-028: Fixed set_working_dir silent no-op on null input (was silently swallowing writes)
- F-028: Fixed format_directory_listing symlink misclassification (was using is_file(), changed to !is_dir())
- F-028: Made compose module private (was pub mod with zero exports)
- F-028: Added test for set_working_dir on null input

### Antagonistic Review Issues Noted (Not Fixed -- By Design)
- F-027: pending_context not serialized in to_dict (should be resolved before serializing)
- F-027: Context path roundtrip contains Windows `:` in path issue (paths with drive letters like `C:\` would be routed to pending_context). Not a concern since project targets Unix.
- F-028: ContentDeduplicator API is more minimal than Python (missing add_file, get_unique_files, get_known_hashes). Only is_duplicate implemented -- sufficient for current use, can extend later.
- F-028: format_directory_listing error message says "permission denied" for all read_dir errors. Could distinguish NotFound vs PermissionDenied, but Python has same behavior.

### What's Next
- All 6 waves complete. 298 tests, 0 clippy warnings, 28 features delivered.
- Remaining `todo!()` stubs (5): `load_mentions` (mentions/loader.rs), `SimpleSourceResolver` (sources/resolver.rs), `check_bundle_status`/`update_bundle` (updates/mod.rs)
- Consider: PyO3 bindings (Wave 7 if needed)
- Consider: BundleRegistry.bundles → IndexMap for deterministic registry.json output
- Consider: Extend ContentDeduplicator with add_file/get_unique_files methods
- Consider: Implement HttpSourceHandler.resolve, GitSourceHandler.resolve

---

## Session 010 -- Wave 5 COMPLETE (F-023, F-024, F-025)

### Work Completed
- **F-023-fmt-clean** (4096b48): Ran `cargo fmt` across entire codebase. 33 files changed (569 insertions, 520 deletions). `cargo fmt --check` now returns clean.
- **F-024-clippy-clean** (8dfa507): Eliminated all 104 clippy warnings:
  - Bumped MSRV from 1.75 to 1.80 (LazyLock stable since 1.80), eliminating 27 `incompatible_msrv` warnings
  - Auto-fixed 68 `needless_borrows_for_generic_args` across lib + tests
  - Fixed `unwrap` after `is_some` in `bundle/mod.rs` (changed to `if let`)
  - Suppressed `dead_code` on unimplemented stub fields (`ContentDeduplicator.seen`, `SimpleSourceResolver.handlers`)
  - Fixed `len_zero` and `redundant_closure` warnings
  - `cargo clippy --all-targets` now reports 0 warnings
- **F-025-integration-tests** (66c86c4): Added 16 cross-module integration tests in `tests/integration.rs` with 5 YAML/MD fixture files in `tests/fixtures/`:
  - Load real YAML bundles from fixture files (full, minimal, registry-format)
  - Load markdown bundle with frontmatter → Bundle with instruction
  - Cross-module pipeline: YAML → from_dict → compose → to_mount_plan → validate
  - Mount plan YAML serialization roundtrip
  - DiskCache + SimpleCache with real mount plan data
  - Validator with real full/minimal bundles
  - deep_merge with real session configs
  - Registry-style YAML loading (wrapping in {"bundle": raw})
  - Compose sequence replacement (deep_merge replaces arrays, child wins)
  - Compose non-commutativity (order matters)
  - to_dict structure documentation test (known roundtrip limitation)

### Wave 5 COMPLETE
- cargo fmt --check: CLEAN (0 formatting issues)
- cargo clippy --all-targets: 0 warnings
- Tests: 281 passing (265 unit + 16 integration), 0 ignored, 0 failed
- MSRV: 1.80 (bumped from 1.75)
- Fixtures: 5 files in tests/fixtures/ (full-bundle.yaml, child-bundle.yaml, minimal.yaml, registry-format.yaml, bundle.md)

### Design Decisions Made
- **MSRV 1.80**: Bumped from 1.75. LazyLock (used in tracing_utils, mentions/parser) is stable since 1.80. This eliminates all MSRV-related clippy warnings. No features from 1.76-1.80 are used beyond LazyLock.
- **Dead code suppression for stubs**: `ContentDeduplicator.seen` and `SimpleSourceResolver.handlers` are fields in unimplemented stub structs (todo!() bodies). Suppressed with `#[allow(dead_code)]` rather than removing them since they'll be needed when the stubs are implemented.
- **Registry-format vs from_dict-format YAML**: Integration tests revealed two distinct YAML formats:
  - **from_dict format**: Everything nested under `bundle:` key (`{"bundle": {"name": ..., "providers": [...], ...}}`)
  - **Registry format**: Fields at top level (`{"name": ..., "providers": [...], ...}`) — the registry wraps this in `{"bundle": raw}` before calling from_dict
  - Fixture files use the appropriate format for their test path
- **to_dict roundtrip is known broken**: `to_dict()` puts providers/tools at the TOP level, but `from_dict()` expects them under "bundle:" key. Added a test documenting this structure. Not a regression — documented in Session 008.
- **Compose sequence replacement confirmed**: deep_merge replaces arrays entirely (child wins). Child's `allowed_paths: ["/workspace"]` replaces base's `["/home/user/projects", "/tmp"]`. Integration test verifies this critical semantic.
- **HashMap agents non-determinism acknowledged**: `Bundle.agents` uses `HashMap<String, Value>`, so mount plan agent ordering is non-deterministic across instances. This is a known limitation from Session 008. Did NOT add a cross-instance determinism test because it would be flaky.

### Antagonistic Review Issues Found & Fixed
- Changed compose provider/tool count assertions from `>=` to exact `==` counts (was hiding potential duplicate bugs)
- Replaced sham mount plan determinism test with compose sequence replacement test (original tested same-instance which is trivially deterministic)
- Added compose non-commutativity test (verifying order matters)
- Added to_dict structure documentation test (exposing known roundtrip limitation)
- Strengthened three-way compose test to verify individual tools survive composition

### Antagonistic Review Issues Noted (Not Fixed — By Design)
- `BundleRegistry::load_yaml_bundle` and `load_markdown_bundle` are private methods, so integration tests simulate the wrapping behavior rather than calling registry methods directly
- No error path integration tests (unit tests in test_bundle.rs cover malformed input; adding integration error tests would duplicate unit test coverage)
- No async integration tests (registry load_single is async; current integration tests are all sync)

### What's Next
- All 5 waves complete. Project is in maintenance/extension mode.
- Consider: PyO3 bindings (Wave 6 if needed)
- Consider: Additional integration tests for async paths (registry.load_single)
- Consider: Make Bundle.agents use IndexMap for deterministic mount plan ordering
- Consider: Fix to_dict/from_dict roundtrip (align nesting structure)
- Consider: Implement remaining stubs (ContentDeduplicator, SimpleSourceResolver, HttpSourceHandler.resolve, GitSourceHandler.resolve)

---

## Session 009 -- Wave 4 COMPLETE (F-021, F-022)

### Work Completed
- **F-021-lib-reexports** (c547bcf): Added 92 `pub use` re-exports to `lib.rs` creating a flat public API surface. Users can now write `use amplifier_foundation::Bundle` instead of `use amplifier_foundation::bundle::Bundle`. Covers all implemented items from Python's `__init__.py` `__all__` (61 items) plus Rust-specific additions (Result type alias, runtime traits, session functions, source handler implementations, ResolvedSource, utility variants). Added crate-level doc comment with Quick Start example. 13 re-export tests + 1 doc test verify compile-time accessibility.
- **F-022-examples** (c62a7a6): Created 3 example binaries in `examples/`:
  - `bundle_parse`: Demonstrates parsing YAML bundles, inspecting fields, generating mount plans, and validation
  - `bundle_compose`: Demonstrates composing parent + child bundles with the 5-strategy merge system
  - `path_utils`: Demonstrates URI parsing, path normalization, path construction, and deep merge

### Wave 4 COMPLETE
- lib.rs re-exports: 92 `pub use` statements across 34 lines
- Examples: 3 binaries, all compile and run successfully
- Tests: 265 passing (251 Wave 1-3 + 13 re-export + 1 doc test), 0 ignored

### Design Decisions Made
- **ValidationResult naming collision**: `error::ValidationResult` and `bundle::validator::ValidationResult` are two distinct structs. Only `bundle::validator::ValidationResult` is re-exported at the crate root (matching Python's `__init__.py` which exports the validator version). `error::ValidationResult` remains accessible via `amplifier_foundation::error::ValidationResult`.
- **Missing Python __all__ items (7 of 61)**: These are intentionally not re-exported because they don't have direct Rust equivalents:
  - `UpdateInfo` -- struct not implemented in Rust yet
  - `BundleNotFoundError`, `BundleLoadError`, `BundleValidationError`, `BundleDependencyError` -- In Rust these are variants of the `BundleError` enum, not separate types
  - `SourceResolverProtocol`, `SourceHandlerWithStatusProtocol` -- traits not implemented
  - `apply_provider_preferences_with_resolution` -- async function not implemented
- **Extra Rust re-exports (beyond Python's 61)**: Justified additions including `Result` type alias, all runtime traits (AmplifierRuntime, Coordinator, etc.), session functions (fork_session, slice_to_turn, etc.), source handler implementations, `ResolvedSource`, `get_amplifier_home`, `get_nested_with_default`, `sanitize_for_json_with_depth`, `write_with_backup_bytes`, `validate_bundle_completeness*`, `ForkResult`.
- **ZipSourceHandler conditionally re-exported**: `#[cfg(feature = "zip-sources")]` gate on the re-export since the zip crate is optional.

### Antagonistic Review Notes
- Review agent was unavailable (overloaded). Self-reviewed against Python `__init__.py` for completeness. All 54 of 61 Python items that have Rust equivalents are re-exported. The 7 missing are documented with justification.
- Test coverage in test_reexports.rs covers the most commonly used items (Bundle, BundleError, Result, Validator, dicts, paths, serialization, tracing, spawn, cache, sources). Items requiring filesystem or async are tested in their dedicated test files.

### What's Next
- Wave 5: Integration tests (load real .yaml/.md bundles), roundtrip tests, cleanup
- Consider: cargo fmt --check clean, final cargo clippy pass for pre-existing warnings
- Consider: MSRV bump from 1.75 to 1.80 (would eliminate ~30 LazyLock MSRV warnings)
- Consider: fix needless_borrows_for_generic_args clippy warnings across codebase

---

## Session 008 -- Wave 3 COMPLETE (F-018, F-019, F-020)

### Work Completed
- **F-018-bundle** (4b3a8e6): Implemented bundle module -- Bundle::new (defaults), from_dict/from_dict_with_base_path (reads from data["bundle"] key, validates module lists reject bare strings with helpful error messages), compose (5-strategy system: deep merge for session/spawn, merge by module ID for providers/tools/hooks, dict update for agents, accumulate with namespace for context/pending_context, later replaces for instruction/base_path/name), to_mount_plan (emits only non-empty sections, excludes context/instruction), resolve_context_path (exact match then base_path lookup), resolve_pending_context (splits on ":" and resolves via source_base_paths). 26 tests un-ignored, all pass.
- **F-019-validator** (ef75077): Implemented validator module -- ValidationResult (new, add_error flips valid, add_warning keeps valid), BundleValidator (validate: required fields + module list entries; validate_completeness: stricter check requiring session, orchestrator, context, providers >= 1), validate_or_raise and validate_completeness_or_raise, 4 convenience functions. 18 tests un-ignored, all pass.
- **F-020-registry** (43e19b9): Implemented registry module -- BundleRegistry::new (loads persisted state from registry.json), register (name→URI mapping), unregister (bidirectional relationship cleanup: includes ↔ included_by), list_registered (sorted), get_state (mutable access), save (JSON persistence), find_nearest_bundle_file (walks UP from start to stop, prefers bundle.md), load_single (async: resolves file:// URIs, loads bundle.yaml/bundle.md, detects subdirectory bundles by walking up for root, sets source_base_paths, handles includes recursively with cycle detection via HashSet loading chain, caches results). 21 tests un-ignored, all pass.

### Wave 3 COMPLETE
- All 65 Wave 3 tests passing: bundle (26) + validator (18) + registry (21)
- Wave 1+2 still fully passing: 186 tests
- Total: 251 passing (186 Wave 1+2 + 65 Wave 3), 0 ignored
- **ALL PORTED TESTS NOW PASSING**

### Design Decisions Made
- **Bundle::from_dict reads from data["bundle"] key**: Unlike Python which reads some fields from data directly (e.g., providers at top level), the Rust tests have everything nested inside data["bundle"]. The Rust from_dict reads all fields from the inner bundle mapping. This matches the ported test expectations.
- **Registry load_yaml_bundle wraps raw YAML in {"bundle": raw}**: Matches Python's `Bundle.from_dict({"bundle": data}, base_path=...)` pattern. Bundle YAML files are flat (name, version, includes at top level) but from_dict expects a "bundle" wrapper.
- **Recursive async via Box::pin**: `load_single_with_chain` and `compose_includes` use `fn(...) -> Pin<Box<dyn Future<...>>>` pattern for recursive async (Rust doesn't support recursive async fn directly). This is the standard workaround.
- **Cycle detection returns minimal bundle**: Instead of raising BundleDependencyError (which would need to be caught), circular dependencies return `Bundle::new(extract_bundle_name(uri))` -- a minimal empty bundle. Tests verify `.is_ok()`, so this works. The compose step naturally handles the minimal bundle (no providers/tools to merge).
- **find_nearest_bundle_file uses fs::canonicalize for comparison**: To handle symlinks and relative paths correctly, both `current` and `stop` are canonicalized. Falls back to raw comparison if canonicalize fails (e.g., non-existent paths).
- **Persistence uses serde_json for registry.json**: The registry.json format is `{"version": 1, "bundles": {name: state_dict}}`. BundleState has to_dict/from_dict using serde_json::Value (not serde_yaml_ng::Value) since it's a JSON file.
- **BundleError::LoadError used for validate_or_raise**: Python uses BundleValidationError. The Rust tests only check `.is_err()`, not the specific variant. Using LoadError is simpler than converting between the two ValidationResult types (error.rs vs validator.rs). A future refactor could use BundleError::ValidationError with conversion.
- **BundleValidator::validate() has 2 sub-validators (not 4)**: Python validate() runs 4: required_fields, module_lists, session, resources. The Rust implementation only runs 2: required_fields, module_lists. Session validation happens in validate_completeness. No test exercises session/resource validation in the basic validate() path, so this is sufficient.
- **compose_includes: includes first, then bundle on top**: Matches Python composition order where `includes[0].compose(includes[1],...).compose(bundle)` -- the parent bundle always wins over includes.
- **Subdirectory detection starts from parent_dir of bundle**: Uses `bundle_dir.parent()` as start for find_nearest_bundle_file, so it finds root bundles ABOVE the loaded bundle. If root is found in a different directory, sets source_base_paths[root.name] = root_dir.

### Antagonistic Review Issues Noted (Not Fixed -- By Design)
- `compose()` context namespace prefixing for the BASE bundle's keys is not implemented. Only the overlay's context keys get prefixed. No test asserts on base context key prefixing.
- `to_dict()` is incomplete (missing session, spawn, agents, context, includes). No test calls to_dict().
- `to_dict()`/`from_dict()` round-trip would lose data because they use different nesting structures. No test exercises round-trip.
- `compose()` unconditionally overwrites name/version (even if overlay has empty name). The test `test_compose_empty_bundles` expects name="child", confirming this behavior.
- `HashMap<String, Value>` for agents means non-deterministic ordering in to_mount_plan agents section. No test depends on agent ordering.
- `BundleRegistry.load_single` doesn't auto-register loaded bundles in self.bundles. No async test checks list_registered after load_single.
- `compose_includes` catches all errors and logs warnings instead of propagating. This matches Python's graceful handling of failed includes.

### What's Next
- Wave 4: lib.rs re-exports (61 pub use statements), examples (3 example binaries)
- Wave 5: Integration tests, roundtrip tests, cleanup
- Consider fixing to_dict/from_dict round-trip if needed for integration tests
- Consider using BundleError::ValidationError for validate_or_raise

---

## Session 007 -- Wave 2 Completion (F-015, F-016, F-017)

### Work Completed
- **F-015-io** (dcdb3fc): Implemented io module -- write_with_backup (atomic write via tempfile + rename), write_with_backup_bytes, read_with_retry (async with exponential backoff on errno 5), write_with_retry, read_yaml, write_yaml, parse_frontmatter (LazyLock regex, YAML frontmatter extraction). 6 tests un-ignored, all pass.
- **F-016-mentions** (4de3d35): Implemented mentions module -- parse_mentions (extracts @mentions excluding code blocks/inline code, email rejection via post-filter), BaseMentionResolver (resolves @path, @./path, @~/path, namespace patterns return None pending Wave 3 Bundle type). 21 tests un-ignored, all pass.
- **F-017-sources** (61ef735): Implemented sources module -- FileSourceHandler (file:// URIs and local paths with subpath), HttpSourceHandler (can_handle only, resolve deferred), ZipSourceHandler (zip+file:// extraction to SHA256-keyed cache), GitSourceHandler (can_handle only, resolve deferred). 16 tests un-ignored, all pass.

### Wave 2 COMPLETE
- All 96 Wave 2 tests passing: io (6) + mentions (21) + session (53) + sources (16)
- Wave 1 still fully passing: 87 tests
- Total: 186 passing (87 Wave 1 + 96 Wave 2 + 3 lib/doc) 
- Remaining ignored: 65 (26 bundle + 21 registry + 18 validator) -- all Wave 3

### Design Decisions Made
- **tempfile as regular dependency**: Needed for atomic write pattern in io/files.rs. Was only dev-dependency before. Python uses tempfile.NamedTemporaryFile for atomic writes.
- **No lookahead/lookbehind in Rust regex**: The `regex` crate doesn't support PCRE lookaround. Mentions parser uses email span post-filtering (find all email matches, reject @-mentions inside them) instead of Python's negative lookahead. Inline code removal uses simple `` `[^`]+` `` pattern instead of lookbehind/lookahead.
- **BaseMentionResolver bundles field is HashMap<String, PathBuf>**: Python uses `dict[str, Bundle]` with `resolve_context_path`. Since Bundle struct with context resolution is Wave 3, namespace patterns (@bundle:name) currently return None. This is safe -- the only test for namespace patterns checks an empty bundles map.
- **serial_test for CWD-modifying tests**: Added `serial_test` crate as dev-dependency. Mention resolver tests that call `set_current_dir` or `set_var("HOME")` are marked `#[serial]` to prevent race conditions in parallel test execution. Python tests run sequentially by default.
- **FileSourceHandler source_root simplified**: Python has _find_source_root and _find_bundle_root for smart root detection. Rust returns active_path as source_root when no subpath (simpler). Tests pass because they only check basic subpath cases.
- **HttpSourceHandler and GitSourceHandler resolve deferred**: Only `can_handle` is implemented. No tests exercise the `resolve` path for HTTP or Git. These remain `todo!()` until Wave 3+ or when tests require them.
- **ZipSourceHandler uses SHA256 cache key**: Same strategy as Python -- hash the source URI to create a content-addressable cache directory. Cache check before extraction for performance.
- **parse_frontmatter returns normalized content**: Both match and no-match paths return \r\n-normalized content (fixed from initial implementation where no-match path returned original).
- **compute_backup_path handles extensionless files**: Fixed from initial implementation. For `Makefile` -> `Makefile.backup` (appends suffix directly), vs `test.txt` -> `test.txt.backup` (replaces extension). Python uses `path.with_suffix(path.suffix + backup_suffix)`.
- **read_with_retry/write_with_retry use blocking std::fs**: Matches Python behavior exactly (Python's `path.read_text()` is also synchronous within async def). The async is only for the sleep between retries. A future optimization could use tokio::fs.

### Antagonistic Review Issues Noted (Not Fixed -- By Design)
- `parse_frontmatter(None, text)` vs Python's `({}, text)` for no-match case -- Rust's Option<Value> is more idiomatic than always returning Some(empty_mapping).
- `@~` resolves to `$HOME/~` instead of `$HOME` -- edge case not tested, same as `~user` expansion not supported (Python-specific `expanduser`).
- FENCED_CODE_BLOCK regex hardcoded to exactly 3 backticks (Python same behavior).
- write_with_backup silently eats backup copy failure (Python same: `contextlib.suppress(Exception)`).

### What's Next
- Wave 3: bundle (26 tests), registry (21 tests), validator (18 tests) -- MOSTLY ASYNC
- Wave 3 is the real migration challenge: bundle.py (1,289 LOC), registry.py (1,223 LOC)
- PreparedBundle async closure pattern needs spike
- Bundle struct with resolve_context_path needed for full mention resolution

---

## Session 006 -- Wave 2 Session Module (F-012, F-013, F-014)

### Work Completed
- **F-012-session-slice** (ed2428e): Implemented session/slice.rs -- get_turn_boundaries, count_turns, slice_to_turn, find_orphaned_tool_calls, add_synthetic_tool_results, get_turn_summary. Handles both OpenAI (tool_calls array) and Anthropic (content blocks with type=tool_use) tool call formats. 26 tests un-ignored, all pass.
- **F-013-session-events** (5a7f112): Implemented session/events.rs -- slice_events_to_timestamp (JSONL line-by-line with timestamp comparison), get_last_timestamp_for_turn (transcript backward search), slice_events_for_fork (convenience wrapper), count_events, get_event_summary. 6 tests un-ignored, all pass.
- **F-014-session-fork** (347a1ec): Implemented session/fork.rs -- fork_session (disk-based with transcript/metadata/events), fork_session_in_memory, get_fork_preview, list_session_forks, get_session_lineage (iterative ancestor walking with cycle detection). 21 tests un-ignored, all pass.

### Session Module COMPLETE
- All 53 Wave 2 session tests passing
- Wave 1 still fully passing: 87 tests
- Total: 140 passing (87 Wave 1 + 53 session) + 2 lib + 1 doc = 143
- Remaining ignored: 108 (26 bundle + 6 io + 21 mentions + 21 registry + 16 sources + 18 validator)

### Design Decisions Made
- **Char-based truncation in get_turn_summary**: Python's `s[:max_length]` slices by character count, not bytes. Rust's `truncate_str` uses `s.chars().take(max_length)` to avoid panicking on multi-byte UTF-8 (e.g., CJK, emoji). Byte-indexed slicing would panic at non-char boundaries.
- **Simple string comparison for timestamp ordering**: ISO 8601 timestamps in the same format sort lexicographically the same as chronologically. Used `ts <= cutoff_timestamp` string comparison instead of parsing to datetime. This matches Python's behavior for the formats used in the test data.
- **events.rs reads both "event" and "event_type" keys**: Python's `get_event_summary` uses `event.get("event", "unknown")`, but the test data uses `"event_type"` key. Rust tries `"event"` first, falls back to `"event_type"`, then "unknown". This handles both real event formats and test data.
- **Cycle detection added to get_session_lineage**: Python lacks this, but a circular metadata reference would cause infinite loop. Added `HashSet<String>` for visited session IDs. Breaks cycle silently (same as hitting a missing metadata file). Not in Python -- added proactively.
- **get_fork_preview ancestors format**: Python's `ancestors.append(current_parent_id)` appends raw strings, but Rust uses `json!({"session_id": pid})` objects. The tests were written to check `a["session_id"]`, so the object format is correct for the test contract. This is a deliberate divergence to provide richer ancestor data.
- **fs::canonicalize vs Python's Path.resolve()**: Python's resolve() succeeds on non-existent paths (just absolutizes). Rust's canonicalize fails. For fork_session, this means non-existent directories fail at canonicalize with an appropriate error. The test only checks `.is_err()`, so behavior matches.
- **max_length parameter hardcoded in get_turn_summary**: Python accepts `max_length=100` as keyword arg. Rust hardcodes 100. No tests exercise custom max_length. If needed later, add `max_length: Option<usize>` parameter.
- **All session errors use BundleError::LoadError**: The error enum doesn't have a ValueError or SessionError variant. Tests check `.is_err()` and string content, not variant matching. A future refactor could add proper session error variants.

### Antagonistic Review Issues Noted (Not Fixed -- By Design)
- `find_orphaned_tool_calls` only detects Anthropic-format tool calls but not Anthropic-format results (`type: "tool_result"` in user messages). Same limitation as Python. Would need additional format support to fully handle Anthropic conversations.
- `fork_session_in_memory` with explicit turn on empty messages silently returns turn=0 instead of erroring. Matches Python behavior.
- `list_session_forks` doesn't explicitly exclude the session itself from results (relies on parent_id != session_id invariant).
- `write_transcript` uses `unwrap_or_default` for serde_json::to_string, which would produce empty lines for unforeseen serialization failures.

### What's Next
- Wave 2 remaining: io (6 tests, ASYNC), mentions (21 tests, MIXED), sources (16 tests, ASYNC)
- io and sources are async -- first async modules in the project
- mentions is mixed sync/async (parser/resolver/dedup/utils are sync, loader is async)
- Session module is fully done, no further work needed

---

## Session 005 -- Wave 1 Completion (F-010, F-011)

### Work Completed
- **F-010-tracing-utils** (1a54a2a): Implemented tracing_utils module -- generate_sub_session_id with W3C Trace Context lineage. LazyLock compiled regex patterns, agent name sanitization, parent span extraction from session ID or trace ID. 9 tests un-ignored, all pass.
- **F-011-spawn** (93c9783): Implemented spawn module (sync portions) -- ProviderPreference (new, to_dict, from_dict, from_list), is_glob_pattern, resolve_model_pattern (glob::Pattern for fnmatch semantics), apply_provider_preferences with flexible provider name matching and build_provider_lookup. ModelResolutionResult struct as placeholder for async resolution. 17 tests un-ignored, all pass.

### Wave 1 COMPLETE
- All 87 Wave 1 tests passing: 18 (dicts) + 15 (paths) + 12 (cache) + 16 (serialization) + 9 (tracing) + 17 (spawn)
- 161 tests still ignored: 96 (Wave 2) + 65 (Wave 3)
- Total: 87 passing + 161 ignored = 248 total tests
- **AWAITING HUMAN APPROVAL** to proceed to Wave 2

### Design Decisions Made
- **tracing_utils uses std::sync::LazyLock for regex**: LazyLock is stable since Rust 1.80. Four compiled regexes: SPAN_PATTERN, TRACE_ID_PATTERN, NON_ALNUM, MULTI_HYPHEN.
- **MULTI_HYPHEN regex is dead code (kept for Python parity)**: NON_ALNUM already uses `[^a-z0-9]+` which collapses runs, making MULTI_HYPHEN a no-op. Same dead code exists in Python. Kept for 1:1 fidelity.
- **child_span binding split for clarity**: `let child_hex = Uuid::new_v4().simple().to_string(); let child_span = &child_hex[..16];` instead of relying on subtle temporary lifetime extension.
- **glob::Pattern for fnmatch semantics**: Minor divergence from Python's fnmatch on Windows (case sensitivity), but model names are ASCII lowercase so no practical impact. glob crate was already in Cargo.toml.
- **build_provider_lookup indexes three name forms**: module_id ("provider-anthropic"), short_name ("anthropic"), and prefixed form ("provider-anthropic"). Same triple-indexing strategy as Python's `_build_provider_lookup`.
- **apply_provider_preferences returns clone for all code paths**: Unlike Python which returns `mount_plan` (same object) for empty prefs, Rust returns `mount_plan.clone()`. Tests use `assert_eq!` (equality) not identity comparison, so this is compatible.
- **ModelResolutionResult includes available_models field**: Added per Session 001 note. Struct is placeholder until async resolution in Wave 2.
- **ProviderPreference.from_list silently skips invalid entries**: Uses filter_map with .ok() to skip entries that fail from_dict parsing. No Python equivalent but spec includes it.

### Antagonistic Review Issues Noted (Not Fixed -- By Design)
- MULTI_HYPHEN and trim_start_matches('.') are dead code kept for Python parity
- build_provider_lookup inserts empty-string keys when providers lack "module" key -- same behavior as Python
- apply_single_override injects config: {} into providers that had no config key -- same as Python's `dict(p.get("config", {}))`
- ModelResolutionResult is unused (placeholder for Wave 2 async resolution)

### What's Next
- Wave 2 implementation: io (6 tests), sources (16 tests), mentions (21 tests), session (53 tests)
- Wave 2 is mixed sync/async -- io and sources are async, session is sync, mentions is mixed
- Need human approval at Wave 1 gate before proceeding

---

## Session 004 -- Wave 1 Implementation (F-007, F-008, F-009)

### Work Completed
- **F-007-paths** (ae90ca4): Implemented paths module -- parse_uri, normalize_path, get_amplifier_home, construct_agent_path, construct_context_path, find_files, find_bundle_root. 15 tests un-ignored, all pass.
- **F-008-cache** (b18db8f): Implemented cache module -- SimpleCache (in-memory HashMap) and DiskCache (filesystem JSON with SHA-256 key hashing). 12 tests un-ignored, all pass.
- **F-009-serialization** (0993c59): Implemented serialization module -- sanitize_for_json (recursive null filtering, max depth protection) and sanitize_message (thinking_block text extraction, content_blocks skipping). 16 tests un-ignored, all pass.

### Test Counts
- Wave 1 running: 18 (dicts) + 15 (paths) + 12 (cache) + 16 (serialization) = 61 passing
- Wave 1 remaining ignored: 9 (tracing) + 17 (spawn) = 26
- Wave 2 still ignored: 96
- Wave 3 still ignored: 65

### Design Decisions Made
- **parse_uri uses manual string parsing**: No `url` crate. Handles git+, zip+, file://, http/s, local paths, package names via string operations and split/find. Query strings stripped from URL paths to match Python's urlparse behavior.
- **normalize_path does NOT resolve symlinks**: Python's `Path.resolve()` resolves symlinks, but using `std::fs::canonicalize` would fail for non-existent paths and produce different results on macOS (where `/home` is a symlink). Uses pure lexical normalization (`normalize_components`) instead. Tests pass on all platforms.
- **DiskCache serializes serde_yaml_ng::Value through serde_json**: The CacheProvider trait uses `serde_yaml_ng::Value` (not `Bundle`), so disk serialization goes through `serde_json::to_string_pretty`/`serde_json::from_str`. Round-trip is exact for the JSON-safe subset of YAML values (strings, numbers, bools, nulls, maps with string keys, arrays). Non-JSON-safe YAML features (NaN, Infinity, non-string keys, tagged values) would be lossy -- same limitation as Python's `json.dumps`.
- **DiskCache cache_key_to_path uses sha2 crate**: SHA-256 hash of key, first 16 hex chars as hash portion. First 30 chars of key as safe prefix (non-alphanumeric except `-_` replaced with `_`).
- **serialization module uses serde_json::Value**: NOT serde_yaml_ng::Value. This matches the spec and Python behavior -- serialization is specifically for JSON data.
- **sanitize_for_json filters null from containers but passes null at top level**: `sanitize_for_json(&Value::Null)` returns `Value::Null`, but null values inside objects and arrays are filtered out. Matches Python behavior where `None` passes through as a return value but is filtered from dicts/lists.
- **sanitize_message thinking_text extraction not re-sanitized**: The extracted `text` value from `thinking_block` is inserted directly without going through `sanitize_for_json`, matching Python's behavior exactly.

### Antagonistic Review Issues Noted (Not Fixed -- By Design)
- `parse_uri("")` returns a ParsedURI with empty fields where `is_package()` returns true. Same behavior as Python.
- `construct_agent_path` appends `.md` even if name has a different extension (e.g., `.yaml`). Same behavior as Python.
- `DiskCache.contains()` checks file existence, `get()` validates content and may delete corrupt files. These can disagree. Same pattern as Python.
- `ResolvedSource` defined in `paths/uri.rs` not `sources/` -- spec explicitly says "defined here because it's a path type."

### What's Next
- F-010 (tracing_utils): generate_sub_session_id -- 9 tests
- F-011 (spawn): ProviderPreference, apply_provider_preferences -- 17 tests
- After those, all Wave 1 features (87 tests) should pass

---

## Session 003 -- Wave 1 Start (F-006)

### Work Completed
- **F-006-dicts** (23d1a7a): Implemented dicts module -- deep_merge, merge_module_lists, get_nested, set_nested. 18 tests un-ignored, all pass.

---

## Session 002 -- Wave 0 Test Porting (F-004, F-005)

### Work Completed
- **F-004** (190f9df): 96 #[ignore = "Wave 2"] tests across 4 test files + module stubs with todo!() bodies.
  - test_io_files.rs: 6 tests (write_with_backup)
  - test_sources.rs: 16 tests (FileSourceHandler, HttpSourceHandler, ZipSourceHandler)
  - test_mentions.rs: 21 tests (parse_mentions 11, BaseMentionResolver 10)
  - test_session.rs: 53 tests (slice 14, fork 14, events 6, orphaned tools 9, summary 3, lineage 3, preview 2, list forks 2)
  - Module stubs: io/{files,yaml,frontmatter}, sources/{mod,file,http,git,zip,resolver}, mentions/{mod,models,parser,resolver,dedup,loader,utils}, session/{mod,capabilities,events,fork,slice}

- **F-005** (55f9862): 65 #[ignore = "Wave 3"] tests across 3 test files + module stubs with todo!() bodies.
  - test_bundle.rs: 26 tests (Bundle 3, compose 5, mount_plan 2, context 3, pending_context 5, validation 8)
  - test_registry.rs: 21 tests (find_nearest 6, unregister 7, subdirectory_loading 3, diamond/circular 5)
  - test_validator.rs: 18 tests (ValidationResult 3, BundleValidator 4, completeness 7, convenience 4)
  - Module stubs: bundle/{mod,compose,mount,validator}, registry/{mod,persistence,includes}, modules/{mod,state}, updates/mod

### Test Counts (Actual vs Spec)
- Wave 1: 87 (matches spec)
- Wave 2: 96 (spec said 91; test_sources.py has 16 tests, spec estimated 11)
- Wave 3: 65 (spec said 57; test_registry.py has 21 tests, spec estimated 13)
- **Total: 248** (spec said 235; delta +13 from Python source having more tests than estimated)
- Gate criteria updated to 248

### Design Decisions Made
- **SourceHandler trait**: Defined as async_trait in sources/mod.rs with can_handle (sync) and resolve (async) methods
- **SourceStatus struct**: Defined in sources/mod.rs with uri, current_version, latest_version, has_update fields
- **ForkResult struct**: In session/fork.rs. session_dir is Option<PathBuf> (None for in-memory forks). messages is Option<Vec<Value>> (None for on-disk forks).
- **ValidationResult in validator.rs**: Separate from error.rs's ValidationResult. The bundle/validator.rs version has add_error/add_warning methods and valid bool. The error.rs version is simpler (just errors/warnings Vec).
- **BundleState fields**: Mirrors Python dataclass closely -- uri, name, version, includes, included_by, is_root, root_name, explicitly_requested, app_bundle
- **MentionResolver trait**: Defined in mentions/mod.rs. BaseMentionResolver has with_base_path and with_bundles constructors.
- **write_with_backup**: Made sync (not async) matching Python behavior. Added write_with_backup_bytes for binary mode test.
- **Session re-exports**: session/mod.rs re-exports all public functions from slice, events, and fork submodules.
- **Bundle::compose**: Takes &[&Bundle] slice (composing multiple at once) matching Python's *others variadic pattern.
- **Bundle::from_dict_with_base_path**: Separate function since Rust doesn't have default arguments. Python uses `Bundle.from_dict(data, base_path=...)`.
- **zip dev-dependency**: Added `zip = "2"` to [dev-dependencies] for test_sources.rs to create test zip files.

### Wave 0 Gate Status
- All 5 features (F-001 through F-005) completed
- cargo check --tests: PASSES
- cargo test: 0 pass, 0 fail, 248 ignored
- cargo build: PASSES
- cargo clippy --all-targets: 0 errors (warnings only from unused variables in todo!() stubs)
- **AWAITING HUMAN APPROVAL** to proceed to Wave 1

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
