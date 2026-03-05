use indexmap::IndexMap;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::models::UniqueFile;

/// Deduplicate content by SHA-256 hash with multi-path attribution.
///
/// Tracks files that have been added and returns only unique content.
/// When the same content is found at multiple paths, all paths are tracked
/// so users/models know all @mentions that resolved to this content.
///
/// Two APIs are available:
/// - `is_duplicate(content)`: Combined check-and-track (mutating predicate).
///   Used by the load_mentions pipeline for simple dedup.
/// - `add_file(path, content)` + `get_unique_files()`: Path-attributed dedup.
///   Used when you need to know WHERE duplicate content was found.
///
/// Both APIs share the same internal hash set, so content tracked via
/// `is_duplicate` is visible to `is_seen` and vice versa.
pub struct ContentDeduplicator {
    seen: HashSet<String>,
    /// Content indexed by hash. Only populated by `add_file`.
    /// Uses IndexMap to preserve insertion order (matches Python dict ordering).
    content_by_hash: IndexMap<String, String>,
    /// Paths indexed by hash. Only populated by `add_file`.
    paths_by_hash: IndexMap<String, Vec<PathBuf>>,
}

impl Default for ContentDeduplicator {
    fn default() -> Self {
        Self::new()
    }
}

impl ContentDeduplicator {
    pub fn new() -> Self {
        ContentDeduplicator {
            seen: HashSet::new(),
            content_by_hash: IndexMap::new(),
            paths_by_hash: IndexMap::new(),
        }
    }

    /// Check if the given content has been seen before.
    ///
    /// Returns `true` if the content is a duplicate (already seen),
    /// `false` if this is the first time seeing it.
    /// Automatically tracks new content for future duplicate detection.
    ///
    /// NOTE: This is a mutating predicate — it both checks and tracks.
    /// Use `is_seen` for a pure query.
    pub fn is_duplicate(&mut self, content: &str) -> bool {
        let hash = Self::hash_content(content);
        // insert returns false if already present
        !self.seen.insert(hash)
    }

    /// Add a file, tracking its path even if content is duplicate.
    ///
    /// Returns `true` if content is new, `false` if duplicate content
    /// (but path is still tracked for attribution).
    pub fn add_file(&mut self, path: &Path, content: &str) -> bool {
        let hash = Self::hash_content(content);
        let is_new = self.seen.insert(hash.clone());

        if is_new {
            // New content
            self.content_by_hash
                .insert(hash.clone(), content.to_string());
            self.paths_by_hash.insert(hash, vec![path.to_path_buf()]);
            true
        } else {
            // Duplicate content — add path if not already tracked
            if let Some(paths) = self.paths_by_hash.get_mut(&hash) {
                let pb = path.to_path_buf();
                if !paths.contains(&pb) {
                    paths.push(pb);
                }
            } else {
                // Content was tracked via is_duplicate, not add_file.
                // Store it now for get_unique_files compatibility.
                self.content_by_hash
                    .insert(hash.clone(), content.to_string());
                self.paths_by_hash.insert(hash, vec![path.to_path_buf()]);
            }
            false
        }
    }

    /// Get list of unique files with all paths where each was found.
    ///
    /// Returns one `UniqueFile` per unique content, each with all paths
    /// where that content was found. Only includes content added via
    /// `add_file` (not via `is_duplicate`).
    pub fn get_unique_files(&self) -> Vec<UniqueFile> {
        self.content_by_hash
            .iter()
            .map(|(hash, content)| UniqueFile {
                content: content.clone(),
                content_hash: hash.clone(),
                paths: self.paths_by_hash.get(hash).cloned().unwrap_or_default(),
            })
            .collect()
    }

    /// Check if content has already been seen (pure query, no mutation).
    pub fn is_seen(&self, content: &str) -> bool {
        let hash = Self::hash_content(content);
        self.seen.contains(&hash)
    }

    /// Return hashes currently tracked by the deduplicator.
    pub fn get_known_hashes(&self) -> HashSet<String> {
        self.seen.clone()
    }

    /// Compute SHA-256 hex digest of content.
    fn hash_content(content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        format!("{:x}", hasher.finalize())
    }
}
