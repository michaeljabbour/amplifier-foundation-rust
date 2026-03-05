use std::sync::LazyLock;

use regex::Regex;
use uuid::Uuid;

/// W3C Trace Context uses 16 hex chars (8 bytes) for span IDs.
const SPAN_HEX_LEN: usize = 16;
const DEFAULT_PARENT_SPAN: &str = "0000000000000000";

/// Pattern to extract parent/child spans from sub-session IDs.
/// Matches: {16 hex}-{16 hex}_{name}
static SPAN_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([0-9a-f]{16})-([0-9a-f]{16})_").unwrap());

/// Pattern to validate a 32-char hex trace ID.
static TRACE_ID_PATTERN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[0-9a-f]{32}$").unwrap());

/// Pattern for non-alphanumeric characters (for sanitization).
static NON_ALNUM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^a-z0-9]+").unwrap());

/// Pattern for multiple consecutive hyphens.
static MULTI_HYPHEN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"-{2,}").unwrap());

/// Generate a sub-session ID with W3C Trace Context lineage.
///
/// Format: `{parent-span}-{child-span}_{agent-name}`
/// - parent-span: 16 hex chars extracted from parent session or trace ID
/// - child-span: 16 hex chars (random UUID prefix)
/// - agent-name: sanitized for filesystem safety
///
/// # Examples
///
/// ```
/// use amplifier_foundation::tracing_utils::generate_sub_session_id;
///
/// // First-level sub-session (no parent span)
/// let sub_id = generate_sub_session_id(Some("analyzer"), None, None);
/// assert!(sub_id.starts_with("0000000000000000-"));
/// assert!(sub_id.ends_with("_analyzer"));
/// ```
pub fn generate_sub_session_id(
    agent_name: Option<&str>,
    parent_session_id: Option<&str>,
    parent_trace_id: Option<&str>,
) -> String {
    // Sanitize agent name for filesystem safety
    let raw_name = agent_name.unwrap_or("").to_lowercase();

    // Replace any non-alphanumeric characters with hyphens
    let sanitized = NON_ALNUM.replace_all(&raw_name, "-");
    // Collapse multiple hyphens
    let sanitized = MULTI_HYPHEN.replace_all(&sanitized, "-");
    // Remove leading/trailing hyphens and leading dots
    let sanitized = sanitized.trim_matches('-').trim_start_matches('.');

    // Default to "agent" if empty after sanitization
    let sanitized = if sanitized.is_empty() {
        "agent"
    } else {
        sanitized
    };

    // Extract parent span ID following W3C Trace Context principles
    let mut parent_span = DEFAULT_PARENT_SPAN.to_string();

    if let Some(parent_sid) = parent_session_id {
        // If parent has our format, extract its child span (becomes our parent span)
        if let Some(caps) = SPAN_PATTERN.captures(parent_sid) {
            // Extract the child span from parent (second group)
            parent_span = caps[2].to_string();
        }
    }

    // If no parent span found and we have a trace ID, derive parent span from trace
    // Extract middle 16 chars (positions 8-24) from 32-char trace ID
    if parent_span == DEFAULT_PARENT_SPAN {
        if let Some(trace_id) = parent_trace_id {
            if TRACE_ID_PATTERN.is_match(trace_id) {
                // Take middle 16 characters (8-24) of the 32-char trace ID
                parent_span = trace_id[8..24].to_string();
            }
        }
    }

    // Generate new span ID for this child session
    let child_hex = Uuid::new_v4().simple().to_string();
    let child_span = &child_hex[..SPAN_HEX_LEN];

    format!("{parent_span}-{child_span}_{sanitized}")
}
