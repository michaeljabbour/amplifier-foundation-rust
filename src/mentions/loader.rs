use std::collections::HashMap;
use std::path::{self, PathBuf};

use super::dedup::ContentDeduplicator;
use super::models::{ContextFile, MentionResult};
use super::parser::parse_mentions;
use super::utils::format_directory_listing_async;
use super::MentionResolver;

/// Resolve a path to an absolute path for consistent matching.
///
/// Uses `std::fs::canonicalize` for existing paths (resolves symlinks), falls back
/// to `std::path::absolute` for non-existent paths (matches Python's
/// `Path.resolve()` which always returns an absolute path).
fn resolve_path(p: &PathBuf) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| path::absolute(p).unwrap_or_else(|_| p.clone()))
}

/// Format all loaded files as XML context blocks for prepending to system prompts.
///
/// Creates XML-wrapped context blocks that the LLM sees BEFORE the instruction.
/// The @mentions in the original instruction remain as semantic references.
///
/// # Arguments
///
/// * `deduplicator` — Deduplicator containing loaded context files (via `add_file`).
/// * `mention_to_path` — Optional mapping from @mention strings to resolved paths,
///   used to show both @mention and absolute path in XML attributes.
///
/// # Returns
///
/// Formatted context string with XML blocks, or empty string if no files.
///
/// # Example output
///
/// ```xml
/// <context_file paths="@AGENTS.md → /home/user/project/AGENTS.md">
/// [file content here]
/// </context_file>
/// ```
///
/// **Note:** Path and content values are NOT XML-escaped. This matches the Python
/// reference implementation. Paths containing `"` or content containing
/// `</context_file>` could break XML parsing — same limitation as Python.
pub fn format_context_block(
    deduplicator: &ContentDeduplicator,
    mention_to_path: Option<&HashMap<String, PathBuf>>,
) -> String {
    let unique_files = deduplicator.get_unique_files();
    if unique_files.is_empty() {
        return String::new();
    }

    // Build reverse lookup: resolved_path -> list of @mentions for attribution
    // Mentions are sorted per-path for deterministic output (Python dicts are ordered,
    // but Rust HashMaps are not — sorting ensures reproducibility).
    let mut path_to_mentions: HashMap<PathBuf, Vec<String>> = HashMap::new();
    if let Some(m2p) = mention_to_path {
        for (mention, path) in m2p {
            let resolved = resolve_path(path);
            path_to_mentions
                .entry(resolved)
                .or_default()
                .push(mention.clone());
        }
        // Sort mentions per path for deterministic output
        for mentions in path_to_mentions.values_mut() {
            mentions.sort();
        }
    }

    let mut blocks = Vec::new();
    for uf in &unique_files {
        // Build paths attribute showing @mention → absolute path for ALL paths
        // (UniqueFile tracks multiple paths where same content was found)
        let mut path_displays = Vec::new();
        for p in &uf.paths {
            let resolved = resolve_path(p);
            let mentions = path_to_mentions.get(&resolved);
            if let Some(mentions) = mentions {
                // Show each @mention with its resolved path
                for m in mentions {
                    path_displays.push(format!("{} → {}", m, resolved.display()));
                }
            } else {
                // No @mention tracked, just show path
                path_displays.push(format!("{}", resolved.display()));
            }
        }

        let paths_attr = path_displays.join(", ");
        let block = format!(
            "<context_file paths=\"{}\">\n{}\n</context_file>",
            paths_attr, uf.content
        );
        blocks.push(block);
    }

    blocks.join("\n\n")
}

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
            mention,
            resolver,
            &mut dedup,
            &mut result,
            3, // max_depth (matches Python default)
            0, // current_depth
        )
        .await;
    }

    result
}

/// Resolve a single mention and recursively load nested mentions.
///
/// Files are pushed to `result.files` in encounter order (parent before children).
/// Deduplication prevents the same content from being loaded multiple times,
/// which also breaks circular mention chains.
///
/// Uses `Box::pin` to support recursive async calls. `mention` is taken by value
/// (`String`) so it can be moved into the heap-allocated future without lifetime
/// conflicts on the recursive calls.
fn resolve_mention<'a>(
    mention: String,
    resolver: &'a dyn MentionResolver,
    dedup: &'a mut ContentDeduplicator,
    result: &'a mut MentionResult,
    max_depth: usize,
    current_depth: usize,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'a>> {
    Box::pin(async move {
        // Resolve mention to path
        let path = match resolver.resolve(&mention).await {
            Some(p) => p,
            None => {
                result.failed.push(mention);
                return;
            }
        };

        // Handle directories: generate listing as content
        let is_dir = tokio::fs::metadata(&path)
            .await
            .map(|m| m.is_dir())
            .unwrap_or(false);
        if is_dir {
            let content = format_directory_listing_async(&path).await;
            if !dedup.is_duplicate(&content) {
                result.files.push(ContextFile {
                    path,
                    content,
                    mention,
                });
            }
            return;
        }

        // Read file
        let content = match tokio::fs::read_to_string(&path).await {
            Ok(c) => c,
            Err(_) => {
                // Opportunistic — no error for read failure
                result.failed.push(mention);
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
            mention,
        });

        // Recursively load mentions from this file (if not at max depth)
        if current_depth < max_depth {
            let nested_mentions = parse_mentions(&content_for_recursion);
            for nested in nested_mentions {
                resolve_mention(
                    nested,
                    resolver,
                    dedup,
                    result,
                    max_depth,
                    current_depth + 1,
                )
                .await;
            }
        }
    })
}
