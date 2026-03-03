use serde_yaml_ng::Value;

/// Mount plan produced by a bundle.
#[derive(Debug, Clone)]
pub struct MountPlan {
    pub data: Value,
}
