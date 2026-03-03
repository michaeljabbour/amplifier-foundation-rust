use super::models::MentionResult;
use super::resolver::BaseMentionResolver;

/// Load and resolve all mentions from text.
pub async fn load_mentions(_text: &str, _resolver: &BaseMentionResolver) -> MentionResult {
    todo!()
}
