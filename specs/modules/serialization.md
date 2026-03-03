# Module Spec: serialization

> Layer 2 Component Spec. Implements serialization.rs (Wave 1).

## Overview

**Rust module:** `src/serialization.rs`
**Python source:** `amplifier_foundation/serialization.py` (139 LOC)
**Sync/Async:** Fully sync. Pure computation.
**Tests:** 16 tests in `tests/test_serialization.rs`
**Internal dependencies:** None (leaf module)

## Public API

```rust
use serde_json::Value;

/// Recursively sanitize a value to ensure it's JSON-serializable.
/// Handles: nested objects/arrays, non-serializable values (returns null/skips),
/// objects with fields (extracts to map).
///
/// max_depth prevents infinite recursion (default 50).
pub fn sanitize_for_json(value: &Value, max_depth: u32) -> Value;

/// Sanitize a chat message for persistence.
/// Special handling for known LLM API fields:
/// - "thinking_block": extracts .text -> "thinking_text"  
/// - "content_blocks": skipped entirely
/// - Other fields: recursively sanitized
pub fn sanitize_message(message: &Value) -> Value;
```

## Translation Decisions

| Python | Rust | Rationale |
|--------|------|-----------|
| `Any` input type | `serde_json::Value` | JSON Value handles all JSON types |
| `isinstance(value, (bool, int, float, str))` | `Value::is_boolean/number/string` | Value type checks |
| `isinstance(value, dict)` | `Value::is_object()` | JSON object |
| `isinstance(value, list)` | `Value::is_array()` | JSON array |
| `isinstance(value, tuple)` | N/A | No tuples in JSON — skip this case |
| `hasattr(value, "__dict__")` | N/A | No dynamic objects in Rust — skip |
| `hasattr(value, "model_dump")` | N/A | No Pydantic in Rust — skip |
| `json.dumps(value)` fallback | N/A | All Value types are already serializable |
| `None` filtering in dicts | Filter out `Value::Null` entries | Same semantics |

## Key Behaviors to Preserve

1. **Null/primitives pass through:** `null`, `bool`, `number`, `string` are returned as-is.
2. **Objects sanitized recursively:** Each key-value pair is sanitized. Null values are filtered OUT.
3. **Arrays sanitized recursively:** Each element is sanitized. Null elements are filtered OUT.
4. **Max depth protection:** At depth 0, return `Value::Null`.
5. **sanitize_message special fields:**
   - `"thinking_block"` key: if value is object with `"text"` key, extract as `"thinking_text"`. Otherwise skip.
   - `"content_blocks"` key: always skipped.
   - Other keys: sanitized normally.
6. **sanitize_message non-dict input:** If input is not an object, sanitize it and return empty object if result is not an object.

## Rust-Specific Simplifications

The Python version handles many dynamic Python types (objects with `__dict__`, Pydantic models, tuples). In Rust, `serde_json::Value` already covers all JSON types. The sanitization is simpler:

- No need for `hasattr(value, "__dict__")` — no dynamic objects
- No need for `hasattr(value, "model_dump")` — no Pydantic
- No need for `isinstance(value, tuple)` — no tuples in JSON
- No need for `json.dumps(value)` fallback — all Value variants are serializable

The Rust implementation is essentially: recursive walk over Value, filter out Nulls, enforce max depth.

## Notes

- This module uses `serde_json::Value`, NOT `serde_yaml_ng::Value`. It's specifically for JSON serialization.
- The Python `logging.getLogger(__name__)` maps to `tracing::debug!()` for the skip messages.
- Test file has 16 tests covering: primitives, nested objects, arrays, null filtering, depth limit, thinking_block extraction, content_blocks skipping, non-dict message input.
