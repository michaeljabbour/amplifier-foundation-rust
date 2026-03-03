use std::path::Path;
use serde_yaml_ng::Value;

/// Read and parse a YAML file.
pub async fn read_yaml(path: &Path) -> crate::error::Result<Value> {
    todo!()
}

/// Write a Value as YAML to a file.
pub async fn write_yaml(path: &Path, value: &Value) -> crate::error::Result<()> {
    todo!()
}
