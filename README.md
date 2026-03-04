# amplifier-foundation

Rust port of the [Amplifier Foundation](https://github.com/amplifier-dev/amplifier-foundation) Python library. Provides the mechanism layer for bundle composition in the Amplifier ecosystem.

**Core concept:** `Bundle` = composable unit that produces mount plans.

## Features

- **Bundle loading** from YAML, Markdown (frontmatter), file/git/http/zip URIs
- **Bundle composition** with 5 merge strategies (deep merge, merge-by-ID, dict update, accumulate, replace)
- **Mount plan generation** for providers, tools, hooks, session config
- **Bundle registry** with persistence, include resolution, and update lifecycle
- **Session utilities** for turn slicing, forking, and event analysis
- **Source resolution** for file, git, HTTP, and zip URIs with caching
- **Mention parsing** and recursive `@mention` resolution
- **PyO3 bindings** for Python interop (9 types, 36 functions, 5 exceptions)

## Installation

### As a Rust crate (not yet on crates.io)

```toml
[dependencies]
amplifier-foundation = { git = "https://github.com/amplifier-dev/amplifier-foundation-rust" }
```

### As a Python package (build from source)

```bash
pip install maturin
maturin develop
```

## Quick Start (Rust)

```rust
use amplifier_foundation::Bundle;
use serde_yaml_ng::Value;

// Parse a bundle from YAML
let yaml = r#"
bundle:
  name: my-bundle
  version: "1.0"
  providers:
    - module: provider-openai
      config:
        model: gpt-4
"#;
let data: Value = serde_yaml_ng::from_str(yaml).unwrap();
let bundle = Bundle::from_dict(&data).unwrap();
assert_eq!(bundle.name, "my-bundle");

// Compose bundles (child overrides parent)
let child_yaml = r#"
bundle:
  name: child
  providers:
    - module: provider-anthropic
      config:
        model: claude-3
"#;
let child_data: Value = serde_yaml_ng::from_str(child_yaml).unwrap();
let child = Bundle::from_dict(&child_data).unwrap();
let composed = bundle.compose(&[&child]);

// Generate mount plan
let plan = composed.to_mount_plan();
```

## Quick Start (Python)

```python
import amplifier_foundation as af

# Parse a bundle from a dict
bundle = af.Bundle.from_dict({
    "bundle": {
        "name": "my-bundle",
        "version": "1.0",
        "providers": [
            {"module": "provider-openai", "config": {"model": "gpt-4"}}
        ],
    }
})

print(bundle.name)          # "my-bundle"
print(bundle.provider_count) # 1

# Compose bundles
child = af.Bundle.from_dict({
    "bundle": {
        "name": "child",
        "providers": [
            {"module": "provider-anthropic", "config": {"model": "claude-3"}}
        ],
    }
})
composed = bundle.compose([child])

# Generate mount plan
plan = composed.to_mount_plan()

# Deep merge dicts
merged = af.deep_merge({"a": 1}, {"b": 2})

# Parse @mentions from text
mentions = af.parse_mentions("Load @docs/readme.md and @config/settings.yaml")

# Validate bundles
result = af.validate_bundle(bundle)
print(result.is_valid)  # True
```

## Module Overview

| Module | Purpose |
|--------|---------|
| `bundle` | Bundle struct, from_dict/to_dict, compose, mount plan, validation |
| `registry` | BundleRegistry with persistence, include resolution, update lifecycle |
| `sources` | Source handlers for file, git, HTTP, zip URIs with caching |
| `mentions` | @mention parsing, resolution, recursive loading, deduplication |
| `session` | Turn slicing, session forking, event analysis |
| `dicts` | deep_merge, merge_module_lists, get/set_nested |
| `paths` | URI parsing, path normalization, Amplifier home directory |
| `cache` | SimpleCache (in-memory) and DiskCache (filesystem) |
| `serialization` | sanitize_for_json, sanitize_message |
| `spawn` | ProviderPreference, apply_provider_preferences |
| `io` | Atomic file writes, YAML I/O, frontmatter parsing |
| `modules` | Module activation, install state management |
| `updates` | Bundle status checking and update lifecycle |
| `runtime` | AmplifierRuntime, Coordinator, and session traits (trait boundary) |

## PyO3 Bindings

The Python bindings are built with [PyO3](https://pyo3.rs) and [maturin](https://www.maturin.rs/). They expose:

- **9 types:** `ParsedURI`, `Bundle`, `ValidationResult`, `SourceStatus`, `ResolvedSource`, `ProviderPreference`, `SimpleCache`, `DiskCache`, `ForkResult`
- **36 functions:** covering dicts, paths, mentions, session, validation, serialization, and more
- **5 exceptions:** `BundleError`, `BundleNotFoundError`, `BundleLoadError`, `BundleValidationError`, `BundleDependencyError`
- **Type stubs:** `.pyi` file included for IDE autocomplete and type checking
- **Compatible:** Python 3.9+ via abi3

## Building from Source

### Rust

```bash
cargo build --release
cargo test
```

### Python wheel

```bash
pip install maturin
maturin build --release
```

### Running tests

```bash
# Rust tests
cargo test

# Python smoke tests
maturin develop
pytest tests/python/

# Linting
cargo fmt --check
cargo clippy --all-targets
cargo clippy --all-targets --features pyo3-bindings
```

### Benchmarks

```bash
cargo bench
```

## Architecture

This crate is a faithful port of the Python `amplifier-foundation` library. Key design principles:

- **Mechanism not policy** -- loads bundles, composes them, produces mount plans
- **`serde_yaml_ng::Value`** for dynamic YAML data (not the archived `serde_yaml`)
- **Async where needed** -- registry loading, source resolution, mention loading use `tokio`
- **Sync where possible** -- dicts, paths, cache, serialization, session are all sync
- **IndexMap** for deterministic ordering where Python dict insertion order matters

## License

MIT -- see [LICENSE](LICENSE).
