# Module Spec: tracing_utils

> Layer 2 Component Spec. Implements tracing_utils.rs (Wave 1).

## Overview

**Rust module:** `src/tracing_utils.rs`
**Python source:** `amplifier_foundation/tracing.py` (105 LOC)
**Sync/Async:** Fully sync.
**Tests:** 9 tests in `tests/test_tracing.rs`
**Internal dependencies:** None (leaf module). Uses: uuid, regex.

## Public API

```rust
/// Generate a sub-session ID with W3C Trace Context lineage.
///
/// Format: {parent-span}-{child-span}_{agent-name}
/// - parent-span: 16 hex chars extracted from parent session or trace ID
/// - child-span: 16 hex chars (random UUID prefix)
/// - agent-name: sanitized for filesystem safety
///
/// Examples:
///   generate_sub_session_id(Some("researcher"), Some("abc123-7890abcdef123456_planner"), None)
///   -> "7890abcdef123456-<random16hex>_researcher"
///
///   generate_sub_session_id(Some("analyzer"), None, None)
///   -> "0000000000000000-<random16hex>_analyzer"
pub fn generate_sub_session_id(
    agent_name: Option<&str>,
    parent_session_id: Option<&str>,
    parent_trace_id: Option<&str>,
) -> String;
```

## Translation Decisions

| Python | Rust | Rationale |
|--------|------|-----------|
| `uuid.uuid4().hex[:16]` | `uuid::Uuid::new_v4().simple().to_string()[..16]` | Random hex chars |
| `re.compile(r"pattern")` | `regex::Regex::new(r"pattern")` or `lazy_static` | Compiled regex |
| `re.sub(r"[^a-z0-9]+", "-", name)` | `regex::Regex::replace_all` | Agent name sanitization |
| `str.strip("-").lstrip(".")` | `.trim_matches('-').trim_start_matches('.')` | String trimming |
| Module-level compiled regex | `std::sync::LazyLock<Regex>` or compile in function | Lazy static regex |

## Key Behaviors to Preserve

1. **Agent name sanitization:**
   - Lowercase
   - Replace non-alphanumeric with hyphens
   - Collapse multiple hyphens to one
   - Strip leading/trailing hyphens and leading dots
   - Default to "agent" if empty after sanitization

2. **Parent span extraction:**
   - If `parent_session_id` matches pattern `^([0-9a-f]{16})-([0-9a-f]{16})_`, extract group 2 as parent span
   - If no parent span found and `parent_trace_id` is a valid 32-char hex string, extract chars 8-24
   - Otherwise use "0000000000000000" (16 zeros)

3. **Output format:** `{parent_span}-{child_span}_{sanitized_name}`

4. **Constants:**
   - `SPAN_HEX_LEN = 16`
   - `DEFAULT_PARENT_SPAN = "0" * 16`

## Notes

- Named `tracing_utils.rs` to avoid clash with the `tracing` crate.
- The regex patterns can use `std::sync::LazyLock` (stable since Rust 1.80) or `once_cell::sync::Lazy`.
- Tests should verify: format structure, parent span extraction from session ID, parent span extraction from trace ID, fallback to default span, agent name sanitization, empty agent name.
- Some tests are non-deterministic (random child span) — test the format/structure, not exact values.
