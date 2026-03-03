use serde_yaml_ng::{Mapping, Value};

use super::helpers::is_null_or_empty_mapping;
use super::Bundle;

/// Mount plan produced by a bundle.
#[derive(Debug, Clone)]
pub struct MountPlan {
    pub data: Value,
}

impl Bundle {
    /// Produce a mount plan from this bundle.
    /// Only includes non-empty sections.
    /// Does NOT include context or instruction (those go through system prompt factory).
    pub fn to_mount_plan(&self) -> Value {
        let mut map = Mapping::new();

        if !is_null_or_empty_mapping(&self.session) {
            map.insert(Value::String("session".to_string()), self.session.clone());
        }
        if !self.providers.is_empty() {
            map.insert(
                Value::String("providers".to_string()),
                Value::Sequence(self.providers.clone()),
            );
        }
        if !self.tools.is_empty() {
            map.insert(
                Value::String("tools".to_string()),
                Value::Sequence(self.tools.clone()),
            );
        }
        if !self.hooks.is_empty() {
            map.insert(
                Value::String("hooks".to_string()),
                Value::Sequence(self.hooks.clone()),
            );
        }
        if !self.agents.is_empty() {
            let mut agents_map = Mapping::new();
            for (name, agent) in &self.agents {
                agents_map.insert(Value::String(name.clone()), agent.clone());
            }
            map.insert(
                Value::String("agents".to_string()),
                Value::Mapping(agents_map),
            );
        }
        if !is_null_or_empty_mapping(&self.spawn) {
            map.insert(Value::String("spawn".to_string()), self.spawn.clone());
        }

        Value::Mapping(map)
    }
}
