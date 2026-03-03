# F-001: Cargo Scaffold and Module Structure

## 1. Overview

**Module:** root
**Priority:** P0
**Depends on:** none

Create the Rust project scaffold: Cargo.toml with all dependencies, .gitignore, src/lib.rs with module declarations, and all module directory stubs with empty mod.rs files. After this feature, `cargo check` must pass.

## 2. Requirements

### Interfaces

No public interfaces in this feature. This is structural scaffolding.

### Behavior

- Cargo.toml must include all dependencies from the architecture spec section 2
- `src/lib.rs` must declare all 12 top-level modules as `pub mod`
- Each module directory must have a `mod.rs` that declares its submodules
- All submodule files must exist (can be empty or contain `// TODO`)
- `.gitignore` must exclude standard Rust artifacts
- `crate-type = ["cdylib", "rlib"]` for PyO3 from day 1
- `pyo3` as optional dependency with `pyo3-bindings` feature

### Cargo.toml Structure

```toml
[package]
name = "amplifier-foundation"
version = "0.1.0"
edition = "2021"
rust-version = "1.75"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml_ng = "0.10"
reqwest = { version = "0.12", features = ["rustls-tls"], optional = true }
thiserror = "2"
async-trait = "0.1"
glob = "0.3"
regex = "1"
dirs = "6"
sha2 = "0.10"
zip = { version = "2", optional = true }
tracing = "0.1"
tracing-subscriber = "0.3"
uuid = { version = "1", features = ["v4"] }
chrono = { version = "0.4", features = ["serde"] }
indexmap = { version = "2", features = ["serde"] }
futures = "0.3"
pyo3 = { version = "0.24", features = ["extension-module"], optional = true }

[features]
default = ["git", "http-sources", "zip-sources"]
git = []
http-sources = ["reqwest"]
zip-sources = ["dep:zip"]
pyo3-bindings = ["dep:pyo3"]

[dev-dependencies]
tokio-test = "0.4"
tempfile = "3"
mockall = "0.13"
assert_matches = "1"
tokio = { version = "1", features = ["full", "test-util"] }
```

### Module Declaration in lib.rs

```rust
pub mod error;
pub mod runtime;
pub mod serialization;
pub mod tracing_utils;

pub mod bundle;
pub mod cache;
pub mod dicts;
pub mod io;
pub mod mentions;
pub mod modules;
pub mod paths;
pub mod registry;
pub mod session;
pub mod sources;
pub mod spawn;
pub mod updates;
```

## 3. Acceptance Criteria

| # | Criterion | Verification |
|---|-----------|-------------|
| AC-1 | `cargo check` passes with zero errors | Run `cargo check` |
| AC-2 | `cargo test` passes (0 tests, 0 failures) | Run `cargo test` |
| AC-3 | `cargo clippy --all-targets` has no errors | Run clippy |
| AC-4 | All 12 module directories exist under src/ | File system check |
| AC-5 | Each module's submodules are declared in mod.rs | Compilation proves this |
| AC-6 | .gitignore excludes /target, Cargo.lock (library) | File check |

## 4. Edge Cases

| Case | Expected Behavior |
|------|-------------------|
| Empty module files | Must compile. Use `// TODO` comments, not code. |
| Optional deps (reqwest, zip, pyo3) | Must compile with and without features |

## 5. Files to Create/Modify

| File | Action | Contents |
|------|--------|----------|
| `Cargo.toml` | Create | Full dependency manifest |
| `.gitignore` | Create | Rust standard gitignore |
| `src/lib.rs` | Create | Module declarations only (no re-exports yet) |
| `src/error.rs` | Create | Empty placeholder `// TODO` |
| `src/runtime.rs` | Create | Empty placeholder `// TODO` |
| `src/serialization.rs` | Create | Empty placeholder `// TODO` |
| `src/tracing_utils.rs` | Create | Empty placeholder `// TODO` |
| `src/bundle/mod.rs` | Create | `pub mod compose; pub mod mount; pub mod prepared; pub mod module_resolver; pub mod prompt; pub mod validator;` |
| `src/bundle/compose.rs` | Create | Empty placeholder |
| `src/bundle/mount.rs` | Create | Empty placeholder |
| `src/bundle/prepared.rs` | Create | Empty placeholder |
| `src/bundle/module_resolver.rs` | Create | Empty placeholder |
| `src/bundle/prompt.rs` | Create | Empty placeholder |
| `src/bundle/validator.rs` | Create | Empty placeholder |
| `src/registry/mod.rs` | Create | `pub mod persistence; pub mod includes;` |
| `src/registry/persistence.rs` | Create | Empty placeholder |
| `src/registry/includes.rs` | Create | Empty placeholder |
| `src/sources/mod.rs` | Create | `pub mod resolver; pub mod file; pub mod git; pub mod http; pub mod zip;` |
| `src/sources/resolver.rs` | Create | Empty placeholder |
| `src/sources/file.rs` | Create | Empty placeholder |
| `src/sources/git.rs` | Create | Empty placeholder |
| `src/sources/http.rs` | Create | Empty placeholder |
| `src/sources/zip.rs` | Create | Empty placeholder |
| `src/mentions/mod.rs` | Create | `pub mod models; pub mod parser; pub mod resolver; pub mod loader; pub mod dedup; pub mod utils;` |
| `src/mentions/models.rs` | Create | Empty placeholder |
| `src/mentions/parser.rs` | Create | Empty placeholder |
| `src/mentions/resolver.rs` | Create | Empty placeholder |
| `src/mentions/loader.rs` | Create | Empty placeholder |
| `src/mentions/dedup.rs` | Create | Empty placeholder |
| `src/mentions/utils.rs` | Create | Empty placeholder |
| `src/io/mod.rs` | Create | `pub mod yaml; pub mod frontmatter; pub mod files;` |
| `src/io/yaml.rs` | Create | Empty placeholder |
| `src/io/frontmatter.rs` | Create | Empty placeholder |
| `src/io/files.rs` | Create | Empty placeholder |
| `src/dicts/mod.rs` | Create | `pub mod merge; pub mod nested;` |
| `src/dicts/merge.rs` | Create | Empty placeholder |
| `src/dicts/nested.rs` | Create | Empty placeholder |
| `src/paths/mod.rs` | Create | `pub mod uri; pub mod normalize; pub mod discovery;` |
| `src/paths/uri.rs` | Create | Empty placeholder |
| `src/paths/normalize.rs` | Create | Empty placeholder |
| `src/paths/discovery.rs` | Create | Empty placeholder |
| `src/cache/mod.rs` | Create | `pub mod memory; pub mod disk;` |
| `src/cache/memory.rs` | Create | Empty placeholder |
| `src/cache/disk.rs` | Create | Empty placeholder |
| `src/session/mod.rs` | Create | `pub mod capabilities; pub mod events; pub mod fork; pub mod slice;` |
| `src/session/capabilities.rs` | Create | Empty placeholder |
| `src/session/events.rs` | Create | Empty placeholder |
| `src/session/fork.rs` | Create | Empty placeholder |
| `src/session/slice.rs` | Create | Empty placeholder |
| `src/spawn/mod.rs` | Create | `pub mod glob;` |
| `src/spawn/glob.rs` | Create | Empty placeholder |
| `src/modules/mod.rs` | Create | `pub mod state;` |
| `src/modules/state.rs` | Create | Empty placeholder |
| `src/updates/mod.rs` | Create | Empty placeholder |

## 6. Dependencies

All crate dependencies listed in Cargo.toml above. No additional.

## 7. Notes

- Do NOT add any re-exports to lib.rs yet. That's Wave 4.
- The `pyo3` feature should compile cleanly when enabled: `cargo check --features pyo3-bindings`
- Keep module files truly empty — just `// TODO` comments. Stubs come in later features.
- Verify `cargo check --no-default-features` also passes (compiles without optional source handlers).
