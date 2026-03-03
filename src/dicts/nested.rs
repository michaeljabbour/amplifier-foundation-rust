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

/// Set a value in a nested YAML Mapping by path.
/// Creates intermediate Mappings as needed.
/// Empty path is a no-op.
/// Modifies data in place.
pub fn set_nested(data: &mut Value, path: &[&str], value: Value) {
    if path.is_empty() {
        return;
    }

    let mut current = data;
    for key in &path[..path.len() - 1] {
        let key_val = Value::String((*key).to_string());
        // If current is not a mapping or doesn't have the key or key is not a mapping,
        // create a new empty mapping
        if !current.is_mapping()
            || !current
                .as_mapping()
                .unwrap()
                .get(&key_val)
                .is_some_and(|v| v.is_mapping())
        {
            if !current.is_mapping() {
                *current = Value::Mapping(serde_yaml_ng::Mapping::new());
            }
            current
                .as_mapping_mut()
                .unwrap()
                .insert(key_val.clone(), Value::Mapping(serde_yaml_ng::Mapping::new()));
        }
        current = current
            .as_mapping_mut()
            .unwrap()
            .get_mut(&key_val)
            .unwrap();
    }

    // Set the final value
    let last_key = Value::String(path[path.len() - 1].to_string());
    if !current.is_mapping() {
        *current = Value::Mapping(serde_yaml_ng::Mapping::new());
    }
    current
        .as_mapping_mut()
        .unwrap()
        .insert(last_key, value);
}
