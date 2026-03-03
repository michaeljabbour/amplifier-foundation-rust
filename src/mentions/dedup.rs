use sha2::{Digest, Sha256};
use std::collections::HashSet;

/// Deduplicate content by SHA-256 hash.
///
/// Tracks content that has been seen and reports duplicates.
/// Uses SHA-256 hashing to detect identical content regardless of source path.
pub struct ContentDeduplicator {
    seen: HashSet<String>,
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
        }
    }

    /// Check if the given content has been seen before.
    ///
    /// Returns `true` if the content is a duplicate (already seen),
    /// `false` if this is the first time seeing it.
    /// Automatically tracks new content for future duplicate detection.
    pub fn is_duplicate(&mut self, content: &str) -> bool {
        let hash = Self::hash_content(content);
        // insert returns false if already present
        !self.seen.insert(hash)
    }

    /// Compute SHA-256 hex digest of content.
    fn hash_content(content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        format!("{:x}", hasher.finalize())
    }
}
