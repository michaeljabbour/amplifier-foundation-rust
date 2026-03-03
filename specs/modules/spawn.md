# Module Spec: spawn

> Layer 2 Component Spec. Implements spawn/ module (Wave 1).

## Overview

**Rust module:** `src/spawn/` (`mod.rs`, `glob.rs`)
**Python source:** `amplifier_foundation/spawn_utils.py` (457 LOC)
**Sync/Async:** Mostly sync. Only `apply_provider_preferences_with_resolution` is async.
**Tests:** 17 tests in `tests/test_spawn.rs`
**Internal dependencies:** None (leaf module for Wave 1 sync portion)

## Public API

### spawn/mod.rs

```rust
use serde_yaml_ng::Value;

/// A provider/model preference for ordered selection.
#[derive(Debug, Clone, PartialEq)]
pub struct ProviderPreference {
    pub provider: String,
    pub model: String,
}

impl ProviderPreference {
    pub fn to_dict(&self) -> Value;
    pub fn from_dict(data: &Value) -> Option<Self>;
    pub fn from_list(data: &[Value]) -> Vec<Self>;
}

/// Apply provider preferences to session config (sync version).
/// Modifies session config to inject provider/model selection.
/// Tries each preference in order until finding an available provider.
pub fn apply_provider_preferences(
    session_config: &Value,
    preferences: &[ProviderPreference],
) -> Value;

/// Apply provider preferences with model resolution (async version).
/// Resolves glob patterns against available models from providers.
/// This is the only async function in spawn.
pub async fn apply_provider_preferences_with_resolution(
    session_config: &Value,
    preferences: &[ProviderPreference],
    available_models: &std::collections::HashMap<String, Vec<String>>,
) -> Value;
```

### spawn/glob.rs

```rust
/// Check if a string is a glob pattern (contains *, ?, or []).
pub fn is_glob_pattern(pattern: &str) -> bool;

/// Resolve a glob pattern against a list of available model names.
/// Returns the first match, or None if no match.
/// Uses fnmatch-style glob matching.
pub fn resolve_model_pattern(pattern: &str, available: &[String]) -> Option<String>;
```

## Translation Decisions

| Python | Rust | Rationale |
|--------|------|-----------|
| `@dataclass ProviderPreference` | `#[derive(Debug, Clone, PartialEq)] struct` | Standard Rust |
| `fnmatch.fnmatch(model, pattern)` | Manual glob matching or `glob::Pattern` | fnmatch semantics |
| `dict[str, str]` for preferences | `Value` for to_dict/from_dict | YAML interop |
| `list[dict]` for preference lists | `&[Value]` for from_list | Parsing from config |
| `logging.getLogger(__name__)` | `tracing::debug!()` / `tracing::warn!()` | Structured logging |

## Key Behaviors to Preserve

1. **ProviderPreference.from_dict:** Parses `{"provider": "...", "model": "..."}` from Value.
2. **ProviderPreference.from_list:** Parses list of dicts into Vec<ProviderPreference>.
3. **is_glob_pattern:** Returns true if pattern contains `*`, `?`, or `[`.
4. **resolve_model_pattern:** fnmatch semantics — `"claude-haiku-*"` matches `"claude-haiku-20240307"`.
5. **apply_provider_preferences:** Injects provider/model into session config. Tries preferences in order.
6. **Flexible provider matching:** `"anthropic"` matches `"provider-anthropic"`. Strip `"provider-"` prefix for comparison.

## Notes

- `spawn_utils.py` is 457 LOC — one of the larger leaf modules. Read it carefully.
- The async function `apply_provider_preferences_with_resolution` depends on knowing available models from providers. In Wave 1, implement the sync portions first. The async resolution can be deferred or stubbed.
- Tests cover: ProviderPreference construction, glob pattern detection, model pattern resolution, preference application to session config.
- Python uses `fnmatch.fnmatch` — in Rust, `glob::Pattern::matches` provides similar semantics.
