/// Check if a string is a glob pattern (contains *, ?, or []).
pub fn is_glob_pattern(pattern: &str) -> bool {
    pattern.contains('*') || pattern.contains('?') || pattern.contains('[')
}

/// Resolve a glob pattern against a list of available model names.
///
/// Uses fnmatch-style glob matching. Returns the first match after
/// sorting matches in descending order (latest date/version wins).
///
/// Returns `None` if no match.
pub fn resolve_model_pattern(pattern: &str, available: &[String]) -> Option<String> {
    // Use glob::Pattern for fnmatch-style matching
    let glob_pat = glob::Pattern::new(pattern).ok()?;

    let mut matched: Vec<&String> = available
        .iter()
        .filter(|model| glob_pat.matches(model))
        .collect();

    if matched.is_empty() {
        return None;
    }

    // Sort descending (latest date/version typically sorts last alphabetically,
    // so reverse sort puts newest first)
    matched.sort_by(|a, b| b.cmp(a));
    Some(matched[0].clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_model_pattern_basic() {
        let models = vec![
            "claude-3-haiku-20240101".to_string(),
            "claude-3-haiku-20240307".to_string(),
            "claude-3-haiku-20240201".to_string(),
        ];
        let result = resolve_model_pattern("claude-3-haiku-*", &models);
        assert_eq!(result, Some("claude-3-haiku-20240307".to_string()));
    }

    #[test]
    fn test_resolve_model_pattern_no_match() {
        let models = vec!["gpt-4o".to_string(), "gpt-4o-mini".to_string()];
        let result = resolve_model_pattern("claude-*", &models);
        assert_eq!(result, None);
    }
}
