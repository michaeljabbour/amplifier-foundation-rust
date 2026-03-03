use serde_yaml_ng::{Mapping, Value};

use amplifier_foundation::dicts::merge::{deep_merge, merge_module_lists};
use amplifier_foundation::dicts::nested::{get_nested, get_nested_with_default, set_nested};

// ── helpers ──────────────────────────────────────────────────────────

/// Build a `Value::Mapping` from a list of (key, value) pairs.
fn mapping(pairs: &[(&str, Value)]) -> Value {
    let mut m = Mapping::new();
    for (k, v) in pairs {
        m.insert(Value::String(k.to_string()), v.clone());
    }
    Value::Mapping(m)
}

/// Shorthand: create a `Value::Number` from an integer.
fn int(n: i64) -> Value {
    serde_yaml_ng::to_value(n).unwrap()
}

/// Shorthand: create a `Value::String`.
fn str_val(s: &str) -> Value {
    Value::String(s.to_string())
}

/// Shorthand: create a `Value::Sequence` from a slice of `Value`.
fn seq(items: &[Value]) -> Value {
    Value::Sequence(items.to_vec())
}

// ═════════════════════════════════════════════════════════════════════
// TestDeepMerge
// ═════════════════════════════════════════════════════════════════════

#[test]

fn test_deep_merge_empty_dicts() {
    let parent = mapping(&[]);
    let child = mapping(&[]);
    let result = deep_merge(&parent, &child);
    assert_eq!(result, mapping(&[]));
}

#[test]

fn test_deep_merge_child_overrides_parent_scalars() {
    let parent = mapping(&[("a", int(1)), ("b", int(2))]);
    let child = mapping(&[("b", int(3)), ("c", int(4))]);
    let result = deep_merge(&parent, &child);
    let expected = mapping(&[("a", int(1)), ("b", int(3)), ("c", int(4))]);
    assert_eq!(result, expected);
}

#[test]

fn test_deep_merge_nested_dict_merge() {
    let parent = mapping(&[("config", mapping(&[("a", int(1)), ("b", int(2))]))]);
    let child = mapping(&[("config", mapping(&[("b", int(3)), ("c", int(4))]))]);
    let result = deep_merge(&parent, &child);
    let expected = mapping(&[(
        "config",
        mapping(&[("a", int(1)), ("b", int(3)), ("c", int(4))]),
    )]);
    assert_eq!(result, expected);
}

#[test]

fn test_deep_merge_child_list_replaces_parent_list() {
    let parent = mapping(&[("items", seq(&[int(1), int(2), int(3)]))]);
    let child = mapping(&[("items", seq(&[int(4), int(5)]))]);
    let result = deep_merge(&parent, &child);
    let expected = mapping(&[("items", seq(&[int(4), int(5)]))]);
    assert_eq!(result, expected);
}

#[test]

fn test_deep_merge_parent_unchanged() {
    let parent = mapping(&[("a", mapping(&[("b", int(1))]))]);
    let child = mapping(&[("a", mapping(&[("c", int(2))]))]);
    let original_parent = parent.clone();
    let _result = deep_merge(&parent, &child);
    assert_eq!(parent, original_parent);
}

// ═════════════════════════════════════════════════════════════════════
// TestMergeModuleLists
// ═════════════════════════════════════════════════════════════════════

#[test]

fn test_merge_module_lists_empty_lists() {
    let result = merge_module_lists(&[], &[]);
    assert_eq!(result, Vec::<Value>::new());
}

#[test]

fn test_merge_module_lists_child_adds_new_modules() {
    let parent = [mapping(&[("module", str_val("a"))])];
    let child = [mapping(&[("module", str_val("b"))])];
    let result = merge_module_lists(&parent, &child);
    assert_eq!(result.len(), 2);
    assert!(result.contains(&mapping(&[("module", str_val("a"))])));
    assert!(result.contains(&mapping(&[("module", str_val("b"))])));
}

#[test]

fn test_merge_module_lists_child_config_overrides_parent() {
    let parent = [mapping(&[(
        "module",
        str_val("a"),
    ), (
        "config",
        mapping(&[("x", int(1)), ("y", int(2))]),
    )])];
    let child = [mapping(&[(
        "module",
        str_val("a"),
    ), (
        "config",
        mapping(&[("y", int(3)), ("z", int(4))]),
    )])];
    let result = merge_module_lists(&parent, &child);
    assert_eq!(result.len(), 1);

    let item = &result[0];
    let item_map = item.as_mapping().expect("expected mapping");
    assert_eq!(
        item_map.get(str_val("module")),
        Some(&str_val("a"))
    );
    assert_eq!(
        item_map.get(str_val("config")),
        Some(&mapping(&[("x", int(1)), ("y", int(3)), ("z", int(4))]))
    );
}

#[test]

fn test_merge_module_lists_preserves_order() {
    let parent = [
        mapping(&[("module", str_val("a"))]),
        mapping(&[("module", str_val("b"))]),
    ];
    let child = [mapping(&[("module", str_val("c"))])];
    let result = merge_module_lists(&parent, &child);

    let modules: Vec<&str> = result
        .iter()
        .map(|m| {
            m.as_mapping()
                .unwrap()
                .get(str_val("module"))
                .unwrap()
                .as_str()
                .unwrap()
        })
        .collect();
    assert_eq!(modules, vec!["a", "b", "c"]);
}

#[test]

#[should_panic]
fn test_merge_module_lists_raises_on_string_in_parent() {
    // Expected: panic/error with "Malformed module config at index 0"
    let parent = [
        str_val("tool-bash"),
        mapping(&[("module", str_val("tool-file"))]),
    ];
    merge_module_lists(&parent, &[]);
}

#[test]

#[should_panic]
fn test_merge_module_lists_raises_on_string_in_child() {
    // Expected: panic/error with "Malformed module config at index 1"
    let parent = [mapping(&[("module", str_val("tool-file"))])];
    let child = [
        mapping(&[("module", str_val("tool-bash"))]),
        str_val("provider-anthropic"),
    ];
    merge_module_lists(&parent, &child);
}

#[test]

fn test_merge_module_lists_raises_on_non_dict_types() {
    // Expected: panic/error for non-dict types (integer, list) in module list
    // Sub-case 1: integer in parent list
    let result = std::panic::catch_unwind(|| {
        merge_module_lists(&[int(42)], &[]);
    });
    assert!(result.is_err(), "expected panic for integer in module list");

    // Sub-case 2: sequence/list in parent list
    let result = std::panic::catch_unwind(|| {
        merge_module_lists(&[seq(&[int(1), int(2)])], &[]);
    });
    assert!(result.is_err(), "expected panic for list in module list");
}

// ═════════════════════════════════════════════════════════════════════
// TestGetNested
// ═════════════════════════════════════════════════════════════════════

#[test]

fn test_get_nested_simple_path() {
    let data = mapping(&[(
        "a",
        mapping(&[("b", mapping(&[("c", int(1))]))]),
    )]);
    let result = get_nested(&data, &["a", "b", "c"]);
    assert_eq!(result, Some(int(1)));
}

#[test]

fn test_get_nested_missing_path_returns_default() {
    let data = mapping(&[("a", int(1))]);

    // Missing path returns None
    assert_eq!(get_nested(&data, &["a", "b", "c"]), None);

    // Missing path with explicit default
    assert_eq!(
        get_nested_with_default(&data, &["x", "y"], str_val("missing")),
        str_val("missing")
    );
}

#[test]

fn test_get_nested_empty_path_returns_data() {
    let data = mapping(&[("a", int(1))]);
    let result = get_nested(&data, &[]);
    assert_eq!(result, Some(data));
}

// ═════════════════════════════════════════════════════════════════════
// TestSetNested
// ═════════════════════════════════════════════════════════════════════

#[test]

fn test_set_nested_simple_path() {
    let mut data = mapping(&[]);
    set_nested(&mut data, &["a", "b", "c"], int(1));
    let expected = mapping(&[(
        "a",
        mapping(&[("b", mapping(&[("c", int(1))]))]),
    )]);
    assert_eq!(data, expected);
}

#[test]

fn test_set_nested_overwrites_existing() {
    let mut data = mapping(&[("a", mapping(&[("b", int(1))]))]);
    set_nested(&mut data, &["a", "b"], int(2));
    let expected = mapping(&[("a", mapping(&[("b", int(2))]))]);
    assert_eq!(data, expected);
}

#[test]

fn test_set_nested_creates_intermediate_dicts() {
    let mut data = mapping(&[]);
    set_nested(&mut data, &["a", "b", "c", "d"], str_val("value"));

    // Traverse to verify the deeply nested value
    let a = data.as_mapping().unwrap().get(str_val("a")).unwrap();
    let b = a.as_mapping().unwrap().get(str_val("b")).unwrap();
    let c = b.as_mapping().unwrap().get(str_val("c")).unwrap();
    let d = c.as_mapping().unwrap().get(str_val("d")).unwrap();
    assert_eq!(d, &str_val("value"));
}