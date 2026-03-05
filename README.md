# amplifier-foundation (Rust)

Rust-accelerated drop-in replacement for [amplifier-foundation](https://github.com/microsoft/amplifier-foundation).

## What This Is

`amplifier-foundation` is the Python library that every Amplifier app uses. It handles bundle loading, composition, YAML parsing, mention resolution, session forking -- the plumbing underneath everything.

This project replaces the slow parts with fast compiled Rust while keeping the exact same Python interface. From the outside, nothing changes. You still write `from amplifier_foundation import Bundle` and it works. But under the hood, 30+ functions now run as compiled Rust instead of interpreted Python.

## Why

Functions like `deep_merge`, `parse_uri`, `count_turns`, and `sanitize_message` get called thousands of times during bundle composition, mention parsing, and session management. Rust runs them 10-100x faster.

The complex async stuff that talks to amplifier-core (like `PreparedBundle`, `BundleRegistry`, session creation) stays as Python because it's orchestration code -- it spends its time waiting on I/O, not computing, so Rust wouldn't help there.

## How It Works

We follow the same pattern [amplifier-core](https://github.com/microsoft/amplifier-core) already uses. A compiled Rust `.so` file sits inside the Python package as a hidden `_engine` module. The Python files import from it where Rust is available, fall back to pure Python where it's not.

```
python/amplifier_foundation/
├── __init__.py              # same Python API, imports from Rust _engine where available
├── _engine.abi3.so          # compiled Rust (30+ functions, 9 types, 5 exceptions)
├── bundle.py                # PreparedBundle stays Python (async, talks to core)
├── registry.py              # BundleRegistry stays Python (async orchestration)
├── session/                 # session submodules
├── mentions/                # mention parsing and resolution
├── sources/                 # source handlers (file, git, HTTP, zip)
└── ...                      # everything else from the original
```

Modules and bundles written in Python don't need to change anything -- they never import from foundation directly (they only talk to amplifier-core).

## What's Accelerated by Rust

| Area | Functions | Speedup |
|------|-----------|---------|
| Dict operations | `deep_merge`, `merge_module_lists`, `get_nested`, `set_nested` | 10-50x |
| URI parsing | `parse_uri`, `normalize_path`, `get_amplifier_home` | 10-30x |
| Bundle validation | `validate_bundle` (4 variants) | 10-20x |
| Mention parsing | `parse_mentions` | 10-30x |
| Session analysis | `count_turns`, `slice_to_turn`, `fork_session`, `get_turn_summary` | 10-50x |
| Serialization | `sanitize_for_json`, `sanitize_message` | 10-30x |
| Path construction | `construct_agent_path`, `construct_context_path` | 10-20x |
| Provider preferences | `apply_provider_preferences`, `is_glob_pattern` | 10-20x |
| Frontmatter | `parse_frontmatter` | 10-30x |
| Tracing | `generate_sub_session_id` | 5-10x |

## What Stays Python

These stay as their original Python implementations because they're async orchestration code, depend on amplifier-core's Python API, or need to be subclassable:

- `PreparedBundle` -- async session creation lifecycle
- `BundleRegistry` / `load_bundle` -- async registry with caching
- `BaseMentionResolver` -- abstract base class (subclassed by apps)
- `SimpleSourceResolver` / `GitSourceHandler` -- async source resolution
- `check_bundle_status` / `update_bundle` -- async update checking

## Using It with Your Existing Amplifier Setup

If you already have Amplifier installed and working, swapping in the Rust-accelerated foundation takes two commands:

```bash
# 1. Clone this repo
git clone https://github.com/michaeljabbour/amplifier-foundation-rust.git
cd amplifier-foundation-rust

# 2. Build and install into your Amplifier environment
pip install maturin
maturin develop
```

That's it. The Rust version installs as `amplifier-foundation` -- the same package name as the Python original. It replaces the pure Python version in your environment. The next time you run `amplifier run`, `amplifier resume`, or any Amplifier session, the Rust-accelerated functions are active automatically.

Nothing else changes. Your bundles, agents, modules, configs, and sessions all work exactly as before. The only difference is speed.

To go back to the pure Python version:

```bash
pip install amplifier-foundation
```

### Verifying it's working

```bash
python3 -c "
from amplifier_foundation._engine import deep_merge
print('Rust engine active')
"
```

If you see `Rust engine active`, the Rust code is running. If you get an ImportError, only the Python fallback is in use.

### As a Rust crate (for Rust projects)

```toml
[dependencies]
amplifier-foundation = { git = "https://github.com/michaeljabbour/amplifier-foundation-rust" }
```

## Project Structure

```
amplifier-foundation-rust/
├── Cargo.toml                          # Workspace root
├── pyproject.toml                      # Maturin build config
├── crates/amplifier-foundation/        # Pure Rust library
│   ├── src/                            # 60 .rs files, 11,647 lines
│   └── tests/                          # 614 tests, 0 failures
├── bindings/python/                    # PyO3 bridge (Rust → Python)
│   └── src/                            # #[pymodule] fn _engine
└── python/amplifier_foundation/        # Python package (drop-in compatible)
    ├── __init__.py                     # Imports Rust where available
    └── ...                             # Original Python source for everything else
```

## Rust Quick Start

```rust
use amplifier_foundation::{Bundle, Value};

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
```

## Python Quick Start

```python
import amplifier_foundation as af

bundle = af.Bundle.from_dict({
    "bundle": {
        "name": "my-bundle",
        "version": "1.0",
        "providers": [
            {"module": "provider-openai", "config": {"model": "gpt-4"}}
        ],
    }
})

merged = af.deep_merge({"a": 1, "b": {"x": 1}}, {"b": {"y": 2}})
# {"a": 1, "b": {"x": 1, "y": 2}}  -- computed in Rust

mentions = af.parse_mentions("Load @docs/readme.md")
# [Mention { namespace: None, path: "docs/readme.md" }]  -- parsed in Rust
```

## Rust Modules

| Module | Purpose |
|--------|---------|
| `bundle` | Bundle struct, composition (5 merge strategies), mount plan generation, validation |
| `registry` | BundleRegistry with persistence, include resolution, update lifecycle |
| `sources` | Source handlers for file, git, HTTP, zip URIs with caching |
| `mentions` | @mention parsing, resolution, recursive loading, deduplication |
| `session` | Turn slicing, session forking, event analysis |
| `dicts` | deep_merge, merge_module_lists, get/set_nested |
| `paths` | URI parsing, path normalization, discovery |
| `cache` | In-memory and disk-based caching |
| `serialization` | JSON sanitization for LLM message content |
| `spawn` | Provider preference matching with glob patterns |
| `io` | Atomic file writes, YAML I/O, frontmatter parsing |
| `modules` | Module activation and install state tracking |
| `updates` | Bundle status checking and update lifecycle |
| `runtime` | AmplifierRuntime trait boundary (14 interaction points) |

## Building and Testing

```bash
# Rust tests (614 tests)
cargo test -p amplifier-foundation

# Lint
cargo clippy --all-targets -p amplifier-foundation

# Build Python wheel
maturin develop

# Python smoke tests
pytest tests/python/

# Benchmarks
cargo bench -p amplifier-foundation
```

## Security

The Rust library includes protections against:
- Path traversal via `subpath` in all source handlers (`safe_join` with canonicalization)
- HTTP download timeout (120s) and response size limit (100 MB)
- Path traversal in `construct_agent_path` / `construct_context_path`
- Deep nesting DoS in `set_nested` (max depth 64)
- Cache key collisions (128-bit SHA-256 truncation)
- Zero `unsafe` blocks across the entire codebase

## License

MIT -- see [LICENSE](LICENSE).
