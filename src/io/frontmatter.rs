use serde_yaml_ng::Value;

/// Parse YAML frontmatter from a string.
/// Returns (frontmatter, body) where frontmatter is the parsed YAML between --- delimiters
/// and body is the remaining content.
pub fn parse_frontmatter(content: &str) -> crate::error::Result<(Option<Value>, String)> {
    todo!()
}
