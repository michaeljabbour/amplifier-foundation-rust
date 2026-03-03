use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ContextFile {
    pub path: PathBuf,
    pub content: String,
    pub mention: String,
}

#[derive(Debug, Clone)]
pub struct MentionResult {
    pub files: Vec<ContextFile>,
    pub failed: Vec<String>,
}
