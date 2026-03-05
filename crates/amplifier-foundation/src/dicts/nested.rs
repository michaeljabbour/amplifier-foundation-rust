use serde_yaml_ng::Value;

/// Get a value from a nested YAML Mapping by path.
/// Returns None if path not found or any intermediate value is not a Mapping.
/// Empty path returns a clone of the data itself.
pub fn get_nested(data: &Value, path: &[&str]) -> Option<Value> {
    if path.is_empty() {
        return Some(data.clone());
    }

    let mut current = data;
    for key in path {
        match current.as_mapping() {
            Some(map) => {
                let key_val = Value::String((*key).to_string());
                match map.get(&key_val) {
                    Some(val) => current = val,
                    None => return None,
                }
            }
            None => return None,
        }
    }
    Some(current.clone())
}

/// Get a value from a nested YAML Mapping by path, with a default value.
/// Returns the default if path not found or any intermediate is not a Mapping.
pub fn get_nested_with_default(data: &Value, path: &[&str], default: Value) -> Value {
    get_nested(data, path).unwrap_or(default)
}

/// Maximum nesting depth for `set_nested` to prevent stack overflow.
const MAX_NESTING_DEPTH: usize = 64;

/// Set a value in a nested YAML Mapping by path.
/// Creates intermediate Mappings as needed.
/// Empty path is a no-op. Paths deeper than 64 levels are silently truncated.
/// Modifies data in place.
pub fn set_nested(data: &mut Value, path: &[&str], value: Value) {
    if path.is_empty() || path.len() > MAX_NESTING_DEPTH {
        return;
    }

    let mut current = data;
    for key in &path[..path.len() - 1] {
        let key_val = Value::String((*key).to_string());
        // Ensure current is a mapping
        if !current.is_mapping() {
            *current = Value::Mapping(serde_yaml_ng::Mapping::new());
        }
        // Ensure intermediate key exists and is a mapping
        let needs_create = match current.as_mapping() {
            Some(map) => !map.get(&key_val).is_some_and(|v| v.is_mapping()),
            None => true,
        };
        if needs_create {
            if let Some(map) = current.as_mapping_mut() {
                map.insert(
                    key_val.clone(),
                    Value::Mapping(serde_yaml_ng::Mapping::new()),
                );
            }
        }
        // Navigate into the key
        current = match current.as_mapping_mut() {
            Some(map) => match map.get_mut(&key_val) {
                Some(val) => val,
                None => return, // Should not happen after insert above
            },
            None => return,
        };
    }

    // Set the final value
    let last_key = Value::String(path[path.len() - 1].to_string());
    if !current.is_mapping() {
        *current = Value::Mapping(serde_yaml_ng::Mapping::new());
    }
    if let Some(map) = current.as_mapping_mut() {
        map.insert(last_key, value);
    }
}
