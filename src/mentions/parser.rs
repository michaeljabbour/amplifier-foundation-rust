use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;

/// Match all @-prefixed tokens (mention candidates).
static MENTION_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"@([a-zA-Z0-9_:./~\-]+)").unwrap());

/// Email addresses to reject from mention matches.
static EMAIL_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[a-zA-Z0-9._%+\-]+@[a-zA-Z0-9.\-]+\.[a-zA-Z]{2,}").unwrap());

static FENCED_CODE_BLOCK: LazyLock<Regex> = LazyLock::new(|| {
    // Fenced code blocks: ``` must be at start of line per CommonMark spec
    Regex::new(r"(?m)(?:^|\n)```[^\n]*\n[\s\S]*?(?:^|\n)```").unwrap()
});

static INLINE_CODE: LazyLock<Regex> = LazyLock::new(|| {
    // Single backtick pairs with non-empty content.
    // Rust regex doesn't support lookbehind/lookahead, so we use a simpler pattern.
    // After fenced code blocks are already removed, `content` matching is sufficient.
    Regex::new(r"`[^`]+`").unwrap()
});

/// Parse @mentions from text, excluding those in code blocks and inline code.
/// Returns deduplicated list of mentions (including @ prefix), preserving order.
pub fn parse_mentions(text: &str) -> Vec<String> {
    // Remove code blocks first
    let text_without_code = remove_code_blocks(text);

    // Find all email address spans so we can reject mentions inside them.
    // This replicates Python's negative lookahead without regex lookbehind support.
    let email_spans: Vec<(usize, usize)> = EMAIL_PATTERN
        .find_iter(&text_without_code)
        .map(|m| (m.start(), m.end()))
        .collect();

    // Find @mentions, rejecting those inside email addresses
    let mut seen = HashSet::new();
    let mut result = Vec::new();

    for m in MENTION_PATTERN.find_iter(&text_without_code) {
        let start = m.start();
        // Skip if this @ is inside an email address span
        if email_spans
            .iter()
            .any(|(es, ee)| start >= *es && start < *ee)
        {
            continue;
        }

        let caps = MENTION_PATTERN
            .captures(&text_without_code[start..])
            .unwrap();
        let mention = format!("@{}", &caps[1]);
        if seen.insert(mention.clone()) {
            result.push(mention);
        }
    }

    result
}

/// Remove fenced code blocks and inline code from text.
fn remove_code_blocks(text: &str) -> String {
    // Remove fenced code blocks first
    let text = FENCED_CODE_BLOCK.replace_all(text, "\n");

    // Remove inline code (single backtick pairs)
    let text = INLINE_CODE.replace_all(&text, "");

    text.into_owned()
}
