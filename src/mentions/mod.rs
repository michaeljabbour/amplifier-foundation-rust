use async_trait::async_trait;
use std::path::PathBuf;

pub mod dedup;
pub mod loader;
pub mod models;
pub mod parser;
pub mod resolver;
pub mod utils;

/// Trait for mention resolvers.
#[async_trait]
pub trait MentionResolver: Send + Sync {
    async fn resolve(&self, mention: &str) -> Option<PathBuf>;
}
