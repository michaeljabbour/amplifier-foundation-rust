pub mod dedup;
pub mod loader;
pub mod models;
pub mod parser;
pub mod resolver;
pub mod utils;

use std::path::PathBuf;

/// Trait for mention resolvers.
pub trait MentionResolver: Send + Sync {
    fn resolve(&self, mention: &str) -> Option<PathBuf>;
}
