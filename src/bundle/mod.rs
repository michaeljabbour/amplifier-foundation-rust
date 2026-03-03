pub mod compose;
pub mod module_resolver;
pub mod mount;
pub mod prepared;
pub mod prompt;
pub mod validator;

use serde_yaml_ng::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

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
    pub agents: HashMap<String, Value>,
    pub context: HashMap<String, PathBuf>,
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
    pub fn new(_name: &str) -> Self {
        todo!()
    }

    pub fn from_dict(_data: &Value) -> crate::error::Result<Self> {
        todo!()
    }

    pub fn from_dict_with_base_path(_data: &Value, _base_path: &Path) -> crate::error::Result<Self> {
        todo!()
    }

    pub fn to_dict(&self) -> Value {
        todo!()
    }

    pub fn to_mount_plan(&self) -> Value {
        todo!()
    }

    pub fn compose(&self, _others: &[&Bundle]) -> Bundle {
        todo!()
    }

    pub fn resolve_context_path(&self, _name: &str) -> Option<PathBuf> {
        todo!()
    }

    pub fn resolve_pending_context(&mut self) {
        todo!()
    }
}
