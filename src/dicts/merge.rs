use serde_yaml_ng::Value;

/// Deep merge two YAML values.
/// Child values override parent values.
/// For nested Mappings, merge recursively.
/// For other types (including Sequences), child replaces parent.
/// Returns new Value — inputs not modified.
pub fn deep_merge(parent: &Value, child: &Value) -> Value {
    match (parent, child) {
        (Value::Mapping(parent_map), Value::Mapping(child_map)) => {
            let mut result = parent_map.clone();
            for (key, child_value) in child_map {
                if let Some(parent_value) = result.get(key) {
                    let merged = deep_merge(parent_value, child_value);
                    result.insert(key.clone(), merged);
                } else {
                    result.insert(key.clone(), child_value.clone());
                }
            }
            Value::Mapping(result)
        }
        // For all non-Mapping+Mapping cases, child wins entirely
        (_, child) => child.clone(),
    }
}

/// Merge two lists of module configs by module ID.
/// Module configs are Mappings with a "module" key as identifier.
/// Same module ID: deep merge (child overrides parent).
/// New module: append.
/// Preserves insertion order (parent first, then new child modules).
///
/// Panics if any element is not a Mapping.
pub fn merge_module_lists(parent: &[Value], child: &[Value]) -> Vec<Value> {
    let module_key = Value::String("module".to_string());

    // Index parent configs by module ID, preserving insertion order
    let mut by_id: indexmap::IndexMap<String, Value> = indexmap::IndexMap::new();

    for (i, config) in parent.iter().enumerate() {
        let map = config.as_mapping().unwrap_or_else(|| {
            panic!(
                "Malformed module config at index {}: expected dict with 'module' key, got {:?}",
                i, config
            )
        });
        if let Some(module_id) = map.get(&module_key).and_then(|v| v.as_str()) {
            by_id.insert(module_id.to_string(), config.clone());
        }
    }

    // Process child configs
    for (i, config) in child.iter().enumerate() {
        let map = config.as_mapping().unwrap_or_else(|| {
            panic!(
                "Malformed module config at index {}: expected dict with 'module' key, got {:?}",
                i, config
            )
        });
        let module_id = match map.get(&module_key).and_then(|v| v.as_str()) {
            Some(id) => id.to_string(),
            None => continue, // Child without module key is skipped
        };

        if let Some(existing) = by_id.get(&module_id) {
            // Deep merge with existing
            by_id.insert(module_id, deep_merge(existing, config));
        } else {
            // Add new module
            by_id.insert(module_id, config.clone());
        }
    }

    by_id.into_values().collect()
}
