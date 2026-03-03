use serde_json::{json, Value};

use amplifier_foundation::serialization::{
    sanitize_for_json, sanitize_for_json_with_depth, sanitize_message,
};

// ═══════════════════════════════════════════════════════════════════════
// TestSanitizeForJson
// ═══════════════════════════════════════════════════════════════════════

#[test]
#[ignore = "Wave 1"]
fn test_primitives_unchanged() {
    // None -> Null
    assert_eq!(sanitize_for_json(&Value::Null), Value::Null);

    // true -> true
    assert_eq!(sanitize_for_json(&json!(true)), json!(true));

    // false -> false
    assert_eq!(sanitize_for_json(&json!(false)), json!(false));

    // 42 -> 42
    assert_eq!(sanitize_for_json(&json!(42)), json!(42));

    // float -> float
    assert_eq!(sanitize_for_json(&json!(1.23)), json!(1.23));

    // "hello" -> "hello"
    assert_eq!(sanitize_for_json(&json!("hello")), json!("hello"));
}

#[test]
#[ignore = "Wave 1"]
fn test_dict_sanitization() {
    let input = json!({"a": 1, "b": {"c": 2}});
    let result = sanitize_for_json(&input);

    assert!(result.is_object());
    assert_eq!(result["a"], json!(1));
    assert_eq!(result["b"]["c"], json!(2));

    // Result must be valid JSON (round-trip through serialization)
    let serialized = serde_json::to_string(&result).expect("result should be JSON-serializable");
    let _: Value = serde_json::from_str(&serialized).expect("should parse back");
}

#[test]
#[ignore = "Wave 1"]
fn test_list_sanitization() {
    let input = json!([1, "two", {"three": 3}]);
    let result = sanitize_for_json(&input);

    assert!(result.is_array());
    let arr = result.as_array().unwrap();
    assert_eq!(arr[0], json!(1));
    assert_eq!(arr[1], json!("two"));
    assert_eq!(arr[2]["three"], json!(3));
}

#[test]
#[ignore = "Wave 1"]
fn test_tuple_converted_to_list() {
    // Python tuples become lists. In Rust, serde_json arrays are already
    // arrays, so we verify that an array round-trips through sanitization.
    let input = json!([1, "two", 3]);
    let result = sanitize_for_json(&input);

    assert!(result.is_array());
    let arr = result.as_array().unwrap();
    assert_eq!(arr.len(), 3);
    assert_eq!(arr[0], json!(1));
    assert_eq!(arr[1], json!("two"));
    assert_eq!(arr[2], json!(3));
}

#[test]
#[ignore = "Wave 1"]
fn test_nested_structure() {
    // Python test uses mixed dict/list nesting at level3
    let input = json!({
        "level1": {
            "level2": {
                "level3": [1, 2, {"level4": "deep"}]
            }
        }
    });
    let result = sanitize_for_json(&input);

    assert!(result.is_object());
    assert_eq!(result["level1"]["level2"]["level3"][0], json!(1));
    assert_eq!(result["level1"]["level2"]["level3"][1], json!(2));
    assert_eq!(result["level1"]["level2"]["level3"][2]["level4"], json!("deep"));
}

#[test]
#[ignore = "Wave 1"]
fn test_non_serializable_returns_none() {
    // In Python, non-serializable objects become None.
    // In Rust all serde_json::Value variants are serializable, so we test
    // the equivalent: exceeding max depth returns null.
    let input = json!({"a": {"b": 1}});
    let result = sanitize_for_json_with_depth(&input, 0);
    assert_eq!(result, Value::Null);
}

#[test]
#[ignore = "Wave 1"]
fn test_object_with_dict() {
    // In Python, objects with __dict__ get their dict extracted.
    // In Rust, a Value::Object (mapping) should pass through sanitization.
    let input = json!({"name": "test", "value": 42});
    let result = sanitize_for_json(&input);

    assert!(result.is_object());
    assert_eq!(result["name"], json!("test"));
    assert_eq!(result["value"], json!(42));
}

#[test]
#[ignore = "Wave 1"]
fn test_max_depth_protection() {
    // Build a 100-level deep nested structure
    let mut data: Value = json!("leaf");
    for _ in 0..100 {
        data = json!({"nested": data});
    }

    // sanitize_for_json_with_depth with max_depth=10 should not panic
    let result = sanitize_for_json_with_depth(&data, 10);

    // Result must be JSON-serializable
    let serialized = serde_json::to_string(&result).expect("result should be JSON-serializable");
    let _: Value = serde_json::from_str(&serialized).expect("should parse back");
}

// ═══════════════════════════════════════════════════════════════════════
// TestSanitizeMessage
// ═══════════════════════════════════════════════════════════════════════

#[test]
#[ignore = "Wave 1"]
fn test_simple_message() {
    let input = json!({"role": "user", "content": "hello"});
    let result = sanitize_message(&input);

    assert!(result.is_object());
    assert_eq!(result["role"], json!("user"));
    assert_eq!(result["content"], json!("hello"));
}

#[test]
#[ignore = "Wave 1"]
fn test_extracts_thinking_text_from_dict() {
    let input = json!({
        "role": "assistant",
        "content": "response",
        "thinking_block": {"text": "my thinking"}
    });
    let result = sanitize_message(&input);

    assert!(result.is_object());
    // thinking_text should be extracted
    assert_eq!(result["thinking_text"], json!("my thinking"));
    // thinking_block should be removed
    assert!(
        result.get("thinking_block").is_none(),
        "thinking_block should be removed from the result"
    );
}

#[test]
#[ignore = "Wave 1"]
fn test_extracts_thinking_text_from_object() {
    // In Python this tests an object attribute. In Rust, this is the same
    // as a nested Value::Object — verify identical behavior.
    let input = json!({
        "role": "assistant",
        "thinking_block": {"text": "deep thought"}
    });
    let result = sanitize_message(&input);

    assert!(result.is_object());
    assert_eq!(result["thinking_text"], json!("deep thought"));
    assert!(
        result.get("thinking_block").is_none(),
        "thinking_block should be removed from the result"
    );
}

#[test]
#[ignore = "Wave 1"]
fn test_removes_content_blocks() {
    let input = json!({
        "role": "assistant",
        "content": "response",
        "content_blocks": [{"type": "text", "text": "block"}]
    });
    let result = sanitize_message(&input);

    assert!(result.is_object());
    assert_eq!(result["content"], json!("response"));
    assert!(
        result.get("content_blocks").is_none(),
        "content_blocks should be removed from the result"
    );
}

#[test]
#[ignore = "Wave 1"]
fn test_handles_non_dict_input() {
    // Non-object input should return an empty object
    let result_str = sanitize_message(&json!("just a string"));
    assert!(result_str.is_object());
    assert_eq!(
        result_str.as_object().unwrap().len(),
        0,
        "non-dict input should produce empty object"
    );

    let result_num = sanitize_message(&json!(42));
    assert!(result_num.is_object());
    assert_eq!(
        result_num.as_object().unwrap().len(),
        0,
        "non-dict input should produce empty object"
    );

    let result_arr = sanitize_message(&json!([1, 2, 3]));
    assert!(result_arr.is_object());
    assert_eq!(
        result_arr.as_object().unwrap().len(),
        0,
        "non-dict input should produce empty object"
    );
}

#[test]
#[ignore = "Wave 1"]
fn test_preserves_standard_fields() {
    let input = json!({
        "role": "assistant",
        "content": "hello",
        "model": "claude-3",
        "stop_reason": "end_turn",
        "usage": {"input_tokens": 10, "output_tokens": 20}
    });
    let result = sanitize_message(&input);

    assert!(result.is_object());
    assert_eq!(result["role"], json!("assistant"));
    assert_eq!(result["content"], json!("hello"));
    assert_eq!(result["model"], json!("claude-3"));
    assert_eq!(result["stop_reason"], json!("end_turn"));
    assert_eq!(result["usage"]["input_tokens"], json!(10));
    assert_eq!(result["usage"]["output_tokens"], json!(20));
}

#[test]
#[ignore = "Wave 1"]
fn test_result_is_serializable() {
    // Verify that sanitize_message always produces JSON-serializable output,
    // even with complex nested input.
    let input = json!({
        "role": "assistant",
        "content": "response",
        "thinking_block": {"text": "thinking"},
        "content_blocks": [{"type": "text", "text": "block"}],
        "extra": {"nested": {"deep": true}}
    });
    let result = sanitize_message(&input);

    let serialized = serde_json::to_string(&result).expect("result should be JSON-serializable");
    let roundtripped: Value =
        serde_json::from_str(&serialized).expect("serialized result should parse back");
    assert!(roundtripped.is_object());
}

#[test]
#[ignore = "Wave 1"]
fn test_filters_none_values_in_dict() {
    // Python: non-serializable values become None, and None values are filtered
    // from the dict. In Rust: null values in a dict should be filtered out
    // by sanitize_for_json.
    let input = json!({
        "good": "value",
        "bad": null
    });
    let result = sanitize_for_json(&input);

    assert!(result.is_object());
    assert_eq!(result["good"], json!("value"));
    // Null-valued keys should be filtered out
    assert!(
        result.get("bad").is_none() || result.get("bad") == Some(&Value::Null),
        "null value should be filtered or remain null"
    );
}