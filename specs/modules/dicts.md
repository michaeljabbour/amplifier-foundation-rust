# Module Spec: dicts

> Layer 2 Component Spec. Implements dicts/ module (Wave 1).

## Overview

**Rust module:** `src/dicts/` (`merge.rs`, `nested.rs`)
**Python source:** `amplifier_foundation/dicts/merge.py` (87 LOC), `amplifier_foundation/dicts/navigation.py` (68 LOC)
**Sync/Async:** Fully sync. Pure computation.
**Tests:** 18 tests in `tests/test_dicts.rs`
**Internal dependencies:** None (leaf module)

## Public API

### dicts/merge.rs

```rust
use serde_yaml_ng::Value;

/// Deep merge two YAML values.
/// Child values override parent values.
/// For nested Mappings, merge recursively.
/// For other types (including Sequences), child replaces parent.
/// Returns new Value — inputs not modified.
pub fn deep_merge(parent: &Value, child: &Value) -> Value;

/// Merge two lists of module configs by module ID.
/// Module configs are Mappings with a "module" key as identifier.
/// Same module ID: deep merge (child overrides parent).
/// New module: append.
/// Preserves insertion order (parent first, then new child modules).
///
/// Returns error if any element is not a Mapping.
pub fn merge_module_lists(
    parent: &[Value],
    child: &[Value],
) -> Result<Vec<Value>, String>;
```

### dicts/nested.rs

```rust
use serde_yaml_ng::Value;

/// Get a value from a nested YAML Mapping by path.
/// Returns None if path not found or any intermediate value is not a Mapping.
/// Empty path returns a clone of the data itself.
pub fn get_nested(data: &Value, path: &[&str]) -> Option<Value>;

/// Set a value in a nested YAML Mapping by path.
/// Creates intermediate Mappings as needed.
/// Empty path is a no-op.
/// Modifies data in place.
pub fn set_nested(data: &mut Value, path: &[&str], value: Value);
```

## Translation Decisions

| Python | Rust | Rationale |
|--------|------|-----------|
| `dict[str, Any]` | `serde_yaml_ng::Value` | Dynamic YAML data |
| `list[dict[str, Any]]` | `&[Value]` / `Vec<Value>` | Module config lists |
| `isinstance(x, dict)` | `value.is_mapping()` | YAML Mapping check |
| `result = parent.copy()` | `parent.clone()` | Value is Clone |
| `raise TypeError(...)` | `Result<Vec<Value>, String>` or custom error | merge_module_lists validation |
| `dict.get("module")` | `value.as_mapping().and_then(\|m\| m.get("module"))` | Key access on Value |

## Key Behaviors to Preserve

1. **deep_merge is non-destructive:** Parent and child must not be modified.
2. **List replacement, not concatenation:** `child["items"] = [4,5]` replaces `parent["items"] = [1,2,3]` entirely.
3. **Only Mapping+Mapping triggers recursion:** If parent has `{"key": Mapping}` and child has `{"key": "string"}`, child wins (no recursion).
4. **merge_module_lists preserves order:** Parent modules come first, new child modules appended after.
5. **merge_module_lists validates types:** Non-Mapping elements in either list produce an error with the index and type name.
6. **Modules without "module" key:** In parent, they're indexed but have no ID. In child, they're skipped (no merge target).

## Test Mapping

| Python Test | Rust Test | Notes |
|-------------|-----------|-------|
| `TestDeepMerge::test_empty_dicts` | `test_deep_merge_empty_dicts` | |
| `TestDeepMerge::test_child_overrides_parent_scalars` | `test_deep_merge_child_overrides_scalars` | |
| `TestDeepMerge::test_nested_dict_merge` | `test_deep_merge_nested` | |
| `TestDeepMerge::test_child_list_replaces_parent_list` | `test_deep_merge_list_replaces` | |
| `TestDeepMerge::test_parent_unchanged` | `test_deep_merge_parent_unchanged` | |
| `TestMergeModuleLists::test_empty_lists` | `test_merge_module_lists_empty` | |
| `TestMergeModuleLists::test_child_adds_new_modules` | `test_merge_module_lists_add_new` | |
| `TestMergeModuleLists::test_child_config_overrides_parent` | `test_merge_module_lists_override` | |
| `TestMergeModuleLists::test_preserves_order` | `test_merge_module_lists_order` | |
| `TestMergeModuleLists::test_raises_typeerror_on_string_in_parent` | `test_merge_module_lists_string_in_parent_errors` | Returns Err, not panic |
| `TestMergeModuleLists::test_raises_typeerror_on_string_in_child` | `test_merge_module_lists_string_in_child_errors` | Returns Err |
| `TestMergeModuleLists::test_raises_typeerror_on_non_dict_types` | `test_merge_module_lists_non_dict_errors` | Returns Err |
| `TestGetNested::test_simple_path` | `test_get_nested_simple` | |
| `TestGetNested::test_missing_path_returns_default` | `test_get_nested_missing_returns_none` | None instead of default param |
| `TestGetNested::test_empty_path_returns_data` | `test_get_nested_empty_path` | |
| `TestSetNested::test_simple_path` | `test_set_nested_simple` | |
| `TestSetNested::test_overwrites_existing` | `test_set_nested_overwrites` | |
| `TestSetNested::test_creates_intermediate_dicts` | `test_set_nested_creates_intermediates` | |

## Notes

- `get_nested` in Python has a `default` parameter. In Rust, return `Option<Value>` — caller uses `.unwrap_or()` if they need a default.
- `set_nested` mutates in place in Python (`data[path[-1]] = value`). Same in Rust (`&mut Value`).
- `deep_merge` takes references and returns owned `Value`. This matches Rust conventions and preserves the non-destructive contract.
