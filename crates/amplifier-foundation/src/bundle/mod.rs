mod agent_meta;
mod compose;
mod context;
mod helpers;
pub mod module_resolver;
pub mod mount;
pub mod prepared;
mod serde;
pub mod validator;

use indexmap::IndexMap;
use serde_yaml_ng::Value;
use std::collections::HashMap;
use std::path::PathBuf;

/// The core composable unit in amplifier-foundation.
#[derive(Debug, Clone)]
pub struct Bundle {
    // Metadata
    pub name: String,
    pub version: String,
    pub description: String,
    pub includes: Vec<Value>,

    // Mount plan sections
    pub session: Value,
    pub providers: Vec<Value>,
    pub tools: Vec<Value>,
    pub hooks: Vec<Value>,
    pub spawn: Value,

    // Resources
    pub agents: IndexMap<String, Value>,
    pub context: IndexMap<String, PathBuf>,
    pub instruction: Option<String>,

    // Internal
    pub base_path: Option<PathBuf>,
    pub source_base_paths: HashMap<String, PathBuf>,
    pub pending_context: HashMap<String, String>,
    pub extra: Value,

    // Dynamic fields (Python uses type: ignore)
    pub source_uri: Option<String>,
}

impl Bundle {
    pub fn new(name: &str) -> Self {
        Bundle {
            name: name.to_string(),
            version: "1.0.0".to_string(),
            description: String::new(),
            includes: Vec::new(),
            session: Value::Null,
            providers: Vec::new(),
            tools: Vec::new(),
            hooks: Vec::new(),
            spawn: Value::Null,
            agents: IndexMap::new(),
            context: IndexMap::new(),
            instruction: None,
            base_path: None,
            source_base_paths: HashMap::new(),
            pending_context: HashMap::new(),
            extra: Value::Null,
            source_uri: None,
        }
    }
}
