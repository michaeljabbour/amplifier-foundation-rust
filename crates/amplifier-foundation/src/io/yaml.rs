use std::path::Path;

use serde_yaml_ng::Value;

/// Read and parse a YAML file.
///
/// Returns `None` if the file doesn't exist.
pub async fn read_yaml(path: &Path) -> crate::error::Result<Option<Value>> {
    if !path.exists() {
        return Ok(None);
    }

    let content = super::files::read_with_retry(path, 3).await?;
    let value: Value = serde_yaml_ng::from_str(&content)?;

    // Python's yaml.safe_load returns None for empty content -> or {} returns {}
    let value = if value.is_null() {
        Value::Mapping(serde_yaml_ng::Mapping::new())
    } else {
        value
    };

    Ok(Some(value))
}

/// Write a Value as YAML to a file.
pub async fn write_yaml(path: &Path, value: &Value) -> crate::error::Result<()> {
    let content = serde_yaml_ng::to_string(value)?;
    super::files::write_with_retry(path, &content, 3).await
}
