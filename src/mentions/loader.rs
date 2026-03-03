use std::fs;

use super::dedup::ContentDeduplicator;
use super::models::{ContextFile, MentionResult};
use super::parser::parse_mentions;
use super::utils::format_directory_listing;
use super::MentionResolver;

/// Load and resolve all @mentions from text, with recursive loading and deduplication.
///
/// All mentions are opportunistic — if a file can't be found or read, it is
/// added to `result.failed` (no error raised).
///
/// Recursively resolves @mentions found within loaded files up to `max_depth=3`.
/// Content-based deduplication prevents the same file content from appearing
/// multiple times.
///
/// **Note:** Unlike the Python version which returns per-mention results and
/// accumulates files in an external deduplicator, the Rust version returns an
/// aggregate result containing all loaded files (including recursively
/// discovered ones) and all failed mentions.
///
/// **Note:** The function is async for API compatibility with the Python
/// reference (which uses async `read_with_retry`). The current implementation
/// uses synchronous file I/O internally.
///
/// # Arguments
///
/// * `text` — Text containing @mentions.
/// * `resolver` — Resolver to convert mentions to paths.
///
/// # Returns
///
/// A [`MentionResult`] containing successfully loaded files and failed mentions.
pub async fn load_mentions(text: &str, resolver: &dyn MentionResolver) -> MentionResult {
    let mut result = MentionResult {
        files: Vec::new(),
        failed: Vec::new(),
    };
    let mut dedup = ContentDeduplicator::new();
    let mentions = parse_mentions(text);

    for mention in mentions {
        resolve_mention(
            &mention,
            resolver,
            &mut dedup,
            &mut result,
            3, // max_depth (matches Python default)
            0, // current_depth
        );
    }

    result
}

/// Resolve a single mention and recursively load nested mentions.
///
/// Files are pushed to `result.files` in encounter order (parent before children).
/// Deduplication prevents the same content from being loaded multiple times,
/// which also breaks circular mention chains.
fn resolve_mention(
    mention: &str,
    resolver: &dyn MentionResolver,
    dedup: &mut ContentDeduplicator,
    result: &mut MentionResult,
    max_depth: usize,
    current_depth: usize,
) {
    // Resolve mention to path
    let path = match resolver.resolve(mention) {
        Some(p) => p,
        None => {
            result.failed.push(mention.to_string());
            return;
        }
    };

    // Handle directories: generate listing as content
    if path.is_dir() {
        let content = format_directory_listing(&path);
        if !dedup.is_duplicate(&content) {
            result.files.push(ContextFile {
                path,
                content,
                mention: mention.to_string(),
            });
        }
        return;
    }

    // Read file
    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => {
            // Opportunistic — no error for read failure
            result.failed.push(mention.to_string());
            return;
        }
    };

    // Check for duplicate content
    if dedup.is_duplicate(&content) {
        // Already seen this content, skip
        return;
    }

    // Push parent file FIRST (encounter order), then recurse into children.
    // This ensures files appear in the order they are encountered, matching
    // the expected reading order for context assembly.
    let content_for_recursion = content.clone();
    result.files.push(ContextFile {
        path,
        content,
        mention: mention.to_string(),
    });

    // Recursively load mentions from this file (if not at max depth)
    if current_depth < max_depth {
        let nested_mentions = parse_mentions(&content_for_recursion);
        for nested in nested_mentions {
            resolve_mention(
                &nested,
                resolver,
                dedup,
                result,
                max_depth,
                current_depth + 1,
            );
        }
    }
}
