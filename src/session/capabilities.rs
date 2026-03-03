/// Capability key for working directory.
pub const WORKING_DIR_CAPABILITY: &str = "working_dir";

/// Get the working directory from session capabilities.
pub fn get_working_dir(capabilities: &serde_json::Value) -> Option<String> {
    todo!()
}

/// Set the working directory in session capabilities.
pub fn set_working_dir(capabilities: &mut serde_json::Value, dir: &str) {
    todo!()
}
