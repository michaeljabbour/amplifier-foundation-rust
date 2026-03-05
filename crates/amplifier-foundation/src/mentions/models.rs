use std::path::PathBuf;

/// A single file loaded from an @mention.
#[derive(Debug, Clone)]
pub struct ContextFile {
    pub path: PathBuf,
    pub content: String,
    pub mention: String,
}

/// Result of the load_mentions pipeline.
#[derive(Debug, Clone)]
pub struct MentionResult {
    pub files: Vec<ContextFile>,
    pub failed: Vec<String>,
}

/// A unique piece of content found during deduplication.
///
/// Tracks the content, its SHA-256 hash, and all paths where the
/// same content was found. Matches Python's ContentDeduplicator.get_unique_files()
/// output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UniqueFile {
    pub content: String,
    pub content_hash: String,
    pub paths: Vec<PathBuf>,
}
