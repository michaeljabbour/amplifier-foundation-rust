use std::sync::LazyLock;

use regex::Regex;
use serde_yaml_ng::Value;

/// Compiled frontmatter regex (lazy-initialized).
static FRONTMATTER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)^---[ \t]*\n(.*?)\n---[ \t]*\n?").unwrap()
});

/// Parse YAML frontmatter from a string.
///
/// Extracts YAML between `---` delimiters at the start of the text.
/// Returns `(frontmatter, body)` where frontmatter is the parsed YAML between
/// `---` delimiters and body is the remaining content.
///
/// If no frontmatter is found, returns `(None, original_text)`.
///
/// Edge cases handled:
/// - Windows line endings (`\r\n`) -- normalized to `\n`
/// - Empty frontmatter (`---\n---`) -- returns empty Mapping
/// - No trailing newline
/// - Multiple `---` -- only first pair is delimiter
/// - Trailing whitespace after delimiters
pub fn parse_frontmatter(content: &str) -> crate::error::Result<(Option<Value>, String)> {
    // Normalize Windows line endings
    let normalized = content.replace("\r\n", "\n");

    let Some(caps) = FRONTMATTER_RE.captures(&normalized) else {
        return Ok((None, normalized));
    };

    let frontmatter_str = &caps[1];
    let body = &normalized[caps.get(0).unwrap().end()..];

    // Parse YAML frontmatter
    let frontmatter: Value = serde_yaml_ng::from_str(frontmatter_str)?;

    // yaml.safe_load("") returns None in Python -> or {} -> empty Mapping
    let frontmatter = if frontmatter.is_null() {
        Value::Mapping(serde_yaml_ng::Mapping::new())
    } else {
        frontmatter
    };

    Ok((Some(frontmatter), body.to_string()))
}
