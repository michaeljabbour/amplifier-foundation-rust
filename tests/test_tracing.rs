use std::collections::HashSet;

use regex::Regex;

use amplifier_foundation::tracing_utils::generate_sub_session_id;

// ═══════════════════════════════════════════════════════════════════════
// TestGenerateSubSessionId
// ═══════════════════════════════════════════════════════════════════════

#[test]
#[ignore = "Wave 1"]
fn test_format_with_agent_name() {
    let result = generate_sub_session_id(Some("researcher"), None, None);
    let re = Regex::new(r"^[0-9a-f]{16}-[0-9a-f]{16}_researcher$").unwrap();
    assert!(
        re.is_match(&result),
        "expected format {{parent_span}}-{{child_span}}_researcher, got: {result}"
    );
}

#[test]
#[ignore = "Wave 1"]
fn test_default_agent_name() {
    let result = generate_sub_session_id(None, None, None);
    assert!(
        result.ends_with("_agent"),
        "expected result to end with '_agent', got: {result}"
    );
}

#[test]
#[ignore = "Wave 1"]
fn test_sanitizes_agent_name() {
    let result = generate_sub_session_id(Some("My Agent!"), None, None);
    assert!(
        result.contains("_my-agent"),
        "expected sanitized agent name '_my-agent' in result, got: {result}"
    );
}

#[test]
#[ignore = "Wave 1"]
fn test_root_sub_session_has_zero_parent() {
    let result = generate_sub_session_id(Some("test"), None, None);
    assert!(
        result.starts_with("0000000000000000-"),
        "expected root session to start with 16 zeros, got: {result}"
    );
}

#[test]
#[ignore = "Wave 1"]
fn test_extracts_parent_span_from_parent_session() {
    // Generate a parent session ID (root level).
    let parent_id = generate_sub_session_id(Some("parent"), None, None);

    // The parent ID has format: {parent_span}-{child_span}_{name}
    // Extract the child span from the parent — it becomes the parent span of the child.
    let re = Regex::new(r"^[0-9a-f]{16}-([0-9a-f]{16})_").unwrap();
    let parent_child_span = re
        .captures(&parent_id)
        .expect("parent ID should match expected format")
        .get(1)
        .unwrap()
        .as_str();

    // Generate a child session using the parent session ID.
    let child_id = generate_sub_session_id(Some("child"), Some(&parent_id), None);

    // The child's parent span should equal the parent's child span.
    let child_parent_span = &child_id[..16];
    assert_eq!(
        child_parent_span, parent_child_span,
        "child's parent span should equal parent's child span"
    );
}

#[test]
#[ignore = "Wave 1"]
fn test_derives_parent_span_from_trace_id() {
    let trace_id = "12345678901234567890123456789012";
    let result = generate_sub_session_id(Some("test"), None, Some(trace_id));

    // Parent span should be trace_id[8..24] = "9012345678901234"
    let expected_parent_span = &trace_id[8..24];
    let actual_parent_span = &result[..16];
    assert_eq!(
        actual_parent_span, expected_parent_span,
        "parent span should be derived from trace_id chars 8..24"
    );
}

#[test]
#[ignore = "Wave 1"]
fn test_unique_child_spans() {
    let re = Regex::new(r"^[0-9a-f]{16}-([0-9a-f]{16})_").unwrap();
    let mut spans = HashSet::new();

    for _ in 0..100 {
        let result = generate_sub_session_id(Some("test"), None, None);
        let child_span = re
            .captures(&result)
            .expect("result should match expected format")
            .get(1)
            .unwrap()
            .as_str()
            .to_string();
        spans.insert(child_span);
    }

    assert_eq!(
        spans.len(),
        100,
        "all 100 child spans should be unique, got {} unique",
        spans.len()
    );
}

#[test]
#[ignore = "Wave 1"]
fn test_empty_agent_name_uses_default() {
    let result = generate_sub_session_id(Some(""), None, None);
    assert!(
        result.ends_with("_agent"),
        "expected empty agent name to default to '_agent', got: {result}"
    );
}

#[test]
#[ignore = "Wave 1"]
fn test_hyphenated_agent_name_preserved() {
    let result = generate_sub_session_id(Some("zen-architect"), None, None);
    assert!(
        result.ends_with("_zen-architect"),
        "expected hyphenated name to be preserved as '_zen-architect', got: {result}"
    );
}
