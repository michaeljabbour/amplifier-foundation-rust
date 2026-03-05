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

- **Dict operations** -- `deep_merge`, `merge_module_lists`, `get_nested`, `set_nested`
- **URI parsing** -- `parse_uri`, `normalize_path`, `get_amplifier_home`
- **Bundle validation** -- `validate_bundle` (4 variants)
- **Mention parsing** -- `parse_mentions`
- **Session analysis** -- `count_turns`, `slice_to_turn`, `fork_session`, `get_turn_summary`
- **Serialization** -- `sanitize_for_json`, `sanitize_message`
- **Path construction** -- `construct_agent_path`, `construct_context_path`
- **Provider preferences** -- `apply_provider_preferences`, `is_glob_pattern`
- **Frontmatter** -- `parse_frontmatter`
- **Tracing** -- `generate_sub_session_id`

### Measure the speedup yourself

No claims without evidence. Run this to compare Python vs Rust on your own machine:

```bash
# 1. Benchmark with the Python version first
pip install amplifier-foundation
python3 -c "
import timeit
from amplifier_foundation import deep_merge, parse_mentions

a = {'session': {'model': 'gpt-4', 'temperature': 0.7, 'nested': {'a': 1, 'b': 2}}}
b = {'session': {'temperature': 0.9, 'nested': {'b': 3, 'c': 4}}, 'extra': True}
text = 'Load @docs/readme.md and @config/settings.yaml and @agents/helper.md'

t1 = timeit.timeit(lambda: deep_merge(a, b), number=10000)
t2 = timeit.timeit(lambda: parse_mentions(text), number=10000)
print(f'deep_merge:     {t1:.3f}s for 10k calls')
print(f'parse_mentions: {t2:.3f}s for 10k calls')
print('--- Python version ---')
"

# 2. Install the Rust version and run the same benchmark
cd amplifier-foundation-rust
maturin develop
python3 -c "
import timeit
from amplifier_foundation import deep_merge, parse_mentions

a = {'session': {'model': 'gpt-4', 'temperature': 0.7, 'nested': {'a': 1, 'b': 2}}}
b = {'session': {'temperature': 0.9, 'nested': {'b': 3, 'c': 4}}, 'extra': True}
text = 'Load @docs/readme.md and @config/settings.yaml and @agents/helper.md'

t1 = timeit.timeit(lambda: deep_merge(a, b), number=10000)
t2 = timeit.timeit(lambda: parse_mentions(text), number=10000)
print(f'deep_merge:     {t1:.3f}s for 10k calls')
print(f'parse_mentions: {t2:.3f}s for 10k calls')
print('--- Rust version ---')
"
```

## What Stays Python

These stay as their original Python implementations because they're async orchestration code, depend on amplifier-core's Python API, or need to be subclassable:

- `PreparedBundle` -- async session creation lifecycle
- `BundleRegistry` / `load_bundle` -- async registry with caching
- `BaseMentionResolver` -- abstract base class (subclassed by apps)
- `SimpleSourceResolver` / `GitSourceHandler` -- async source resolution
- `check_bundle_status` / `update_bundle` -- async update checking

## Why the Package Name Is `amplifier-foundation` (Not `amplifier-foundation-rust`)

This installs as `amplifier-foundation` -- the same package name as the Python original. That's intentional. Everything in the ecosystem does `from amplifier_foundation import Bundle`. If we named it differently, nothing would find it.

When you install the Rust version, it replaces the Python version in that environment. That's the whole point -- drop-in replacement.

## Testing It Safely

Use an isolated virtual environment so your main Amplifier installation is never touched.

### Test with amplifier-app-cli

```bash
# Create a throwaway test environment
cd ~/dev/amplifier-app-cli
uv venv .venv-rust-test
source .venv-rust-test/bin/activate

# Install app-cli (pulls in Python foundation + core as deps)
uv pip install -e .

# Now swap foundation with the Rust version (overwrites the Python one)
cd ~/dev/amplifier-foundation-rust
pip install maturin
maturin develop

# Verify Rust is active
python -c "from amplifier_foundation._engine import deep_merge; print('Rust engine active')"

# Run app-cli tests
cd ~/dev/amplifier-app-cli
python -m pytest tests/

# Try a real session
amplifier run

# When done, just delete the test env
deactivate
rm -rf ~/dev/amplifier-app-cli/.venv-rust-test
```

### Test with Kepler (amplifier-distro-kepler)

```bash
# Create a throwaway test environment
cd ~/dev/amplifier-distro-kepler/sidecar
uv venv .venv-rust-test
source .venv-rust-test/bin/activate

# Install sidecar (pulls in amplifier-distro which pulls in foundation)
uv pip install -e .

# Swap foundation with Rust version
cd ~/dev/amplifier-foundation-rust
maturin develop

# Verify
python -c "from amplifier_foundation._engine import deep_merge; print('Rust engine active')"

# Run sidecar tests
cd ~/dev/amplifier-distro-kepler/sidecar
python -m pytest tests/

# Clean up
deactivate
rm -rf ~/dev/amplifier-distro-kepler/sidecar/.venv-rust-test
```

### Going back to normal

Your real Amplifier installation is untouched. The test venvs are isolated. Delete them and nothing changes. If you ever want to permanently switch, just run `maturin develop` in your real environment instead of a test one.

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
