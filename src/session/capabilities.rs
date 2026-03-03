/// Capability key for working directory.
pub const WORKING_DIR_CAPABILITY: &str = "working_dir";

/// Get the working directory from session capabilities JSON.
///
/// Returns `Some(path_string)` if the capability is set and is a string,
/// `None` otherwise.
pub fn get_working_dir(capabilities: &serde_json::Value) -> Option<String> {
    capabilities
        .get(WORKING_DIR_CAPABILITY)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Set the working directory in session capabilities JSON.
///
/// Inserts or overwrites the `"working_dir"` key with the given directory path.
/// If `capabilities` is null, it is coerced to an empty object first.
pub fn set_working_dir(capabilities: &mut serde_json::Value, dir: &str) {
    if capabilities.is_null() {
        *capabilities = serde_json::Value::Object(serde_json::Map::new());
    }
    if let Some(obj) = capabilities.as_object_mut() {
        obj.insert(
            WORKING_DIR_CAPABILITY.to_string(),
            serde_json::Value::String(dir.to_string()),
        );
    }
}
