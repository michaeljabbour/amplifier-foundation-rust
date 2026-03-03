pub mod glob;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderPreference {
    pub provider: String,
    pub model: String,
}

impl ProviderPreference {
    pub fn new(provider: &str, model: &str) -> Self {
        Self {
            provider: provider.to_string(),
            model: model.to_string(),
        }
    }

    pub fn to_dict(&self) -> serde_yaml_ng::Value {
        todo!()
    }

    pub fn from_dict(data: &serde_yaml_ng::Value) -> Result<Self, String> {
        todo!()
    }
}

#[derive(Debug, Clone)]
pub struct ModelResolutionResult {
    pub resolved_model: String,
    pub pattern: Option<String>,
    pub matched_models: Option<Vec<String>>,
}

pub fn apply_provider_preferences(
    mount_plan: &serde_yaml_ng::Value,
    preferences: &[ProviderPreference],
) -> serde_yaml_ng::Value {
    todo!()
}
