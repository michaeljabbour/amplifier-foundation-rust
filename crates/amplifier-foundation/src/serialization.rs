use serde_json::Value;

const DEFAULT_MAX_DEPTH: usize = 50;

/// Recursively sanitize a value to ensure it's JSON-serializable.
///
/// Handles: nested objects/arrays, null values (filtered from containers).
/// Default max_depth of 50 prevents infinite recursion.
pub fn sanitize_for_json(value: &Value) -> Value {
    sanitize_for_json_with_depth(value, DEFAULT_MAX_DEPTH)
}

/// Recursively sanitize a value with a custom max depth.
///
/// At depth 0, returns Value::Null.
pub fn sanitize_for_json_with_depth(value: &Value, max_depth: usize) -> Value {
    sanitize_impl(value, max_depth)
}

/// Sanitize a chat message for persistence.
///
/// Special handling for known LLM API fields:
/// - "thinking_block": extracts .text -> "thinking_text"
/// - "content_blocks": skipped entirely
/// - Other fields: recursively sanitized
pub fn sanitize_message(message: &Value) -> Value {
    // Non-object input: sanitize and return empty object if result is not an object
    if !message.is_object() {
        let result = sanitize_for_json(message);
        return if result.is_object() {
            result
        } else {
            Value::Object(serde_json::Map::new())
        };
    }

    let map = message.as_object().unwrap();
    let mut sanitized = serde_json::Map::new();

    for (key, value) in map {
        // Handle thinking_block: extract text as thinking_text
        if key == "thinking_block" {
            if let Some(obj) = value.as_object() {
                if let Some(text) = obj.get("text") {
                    sanitized.insert("thinking_text".to_string(), text.clone());
                }
            }
            continue;
        }

        // Skip content_blocks
        if key == "content_blocks" {
            continue;
        }

        // Sanitize other fields
        let clean_value = sanitize_for_json(value);
        if !clean_value.is_null() {
            sanitized.insert(key.clone(), clean_value);
        }
    }

    Value::Object(sanitized)
}

// ---------------------------------------------------------------------------
// Internal implementation
// ---------------------------------------------------------------------------

fn sanitize_impl(value: &Value, depth: usize) -> Value {
    if depth == 0 {
        return Value::Null;
    }

    match value {
        // Primitives pass through unchanged
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => value.clone(),

        // Objects: sanitize recursively, filter out nulls
        Value::Object(map) => {
            let mut result = serde_json::Map::new();
            for (k, v) in map {
                let sanitized = sanitize_impl(v, depth - 1);
                if !sanitized.is_null() {
                    result.insert(k.clone(), sanitized);
                }
            }
            Value::Object(result)
        }

        // Arrays: sanitize recursively, filter out nulls
        Value::Array(arr) => {
            let items: Vec<Value> = arr
                .iter()
                .map(|v| sanitize_impl(v, depth - 1))
                .filter(|v| !v.is_null())
                .collect();
            Value::Array(items)
        }
    }
}
