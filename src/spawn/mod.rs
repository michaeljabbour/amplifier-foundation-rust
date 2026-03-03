pub mod glob;

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_yaml_ng::{Mapping, Value};

/// A provider/model preference for ordered selection.
///
/// Used with `provider_preferences` to specify fallback order when spawning
/// sub-sessions. The system tries each preference in order until finding
/// an available provider.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

    /// Convert to a YAML Value mapping with "provider" and "model" keys.
    pub fn to_dict(&self) -> Value {
        let mut m = Mapping::new();
        m.insert(
            Value::String("provider".to_string()),
            Value::String(self.provider.clone()),
        );
        m.insert(
            Value::String("model".to_string()),
            Value::String(self.model.clone()),
        );
        Value::Mapping(m)
    }

    /// Create from a YAML Value mapping. Expects "provider" and "model" keys.
    pub fn from_dict(data: &Value) -> Result<Self, String> {
        let map = data
            .as_mapping()
            .ok_or_else(|| "ProviderPreference requires a mapping".to_string())?;

        let provider = map
            .get(Value::String("provider".to_string()))
            .and_then(|v| v.as_str())
            .ok_or_else(|| "ProviderPreference requires 'provider' key".to_string())?;

        let model = map
            .get(Value::String("model".to_string()))
            .and_then(|v| v.as_str())
            .ok_or_else(|| "ProviderPreference requires 'model' key".to_string())?;

        Ok(Self {
            provider: provider.to_string(),
            model: model.to_string(),
        })
    }

    /// Parse a list of YAML Value mappings into a Vec<ProviderPreference>.
    /// Silently skips entries that fail to parse.
    pub fn from_list(data: &[Value]) -> Vec<Self> {
        data.iter()
            .filter_map(|v| Self::from_dict(v).ok())
            .collect()
    }
}

/// Result of resolving a model pattern.
#[derive(Debug, Clone)]
pub struct ModelResolutionResult {
    pub resolved_model: String,
    pub pattern: Option<String>,
    pub available_models: Option<Vec<String>>,
    pub matched_models: Option<Vec<String>>,
}

/// Build a lookup dict mapping provider names to indices.
///
/// For each provider, indexes by:
/// - Full module ID (e.g., "provider-anthropic")
/// - Short name (e.g., "anthropic")
/// - With provider- prefix (e.g., "provider-anthropic")
fn build_provider_lookup(providers: &[Value]) -> HashMap<String, usize> {
    let mut lookup = HashMap::new();
    for (i, p) in providers.iter().enumerate() {
        let module_id = p
            .as_mapping()
            .and_then(|m| m.get(Value::String("module".to_string())))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        lookup.insert(module_id.to_string(), i);

        // Also index by short name (strip "provider-" prefix)
        let short_name = module_id.strip_prefix("provider-").unwrap_or(module_id);
        if short_name != module_id {
            lookup.insert(short_name.to_string(), i);
        }

        // And with provider- prefix
        lookup.insert(format!("provider-{short_name}"), i);
    }
    lookup
}

/// Apply a single provider/model override to the mount plan.
///
/// Clones the mount plan and providers list, sets priority=0 and model
/// on the target provider.
fn apply_single_override(
    mount_plan: &Value,
    providers: &[Value],
    target_idx: usize,
    model: &str,
) -> Value {
    let mut new_plan = mount_plan.as_mapping().cloned().unwrap_or_default();

    let mut new_providers = Vec::new();
    for (i, p) in providers.iter().enumerate() {
        let mut p_map = p.as_mapping().cloned().unwrap_or_default();

        // Clone the config mapping
        let mut config = p_map
            .get(Value::String("config".to_string()))
            .and_then(|v| v.as_mapping().cloned())
            .unwrap_or_default();

        if i == target_idx {
            // Promote to priority 0 (highest)
            config.insert(
                Value::String("priority".to_string()),
                Value::Number(serde_yaml_ng::Number::from(0)),
            );
            config.insert(
                Value::String("model".to_string()),
                Value::String(model.to_string()),
            );
        }

        p_map.insert(Value::String("config".to_string()), Value::Mapping(config));
        new_providers.push(Value::Mapping(p_map));
    }

    new_plan.insert(
        Value::String("providers".to_string()),
        Value::Sequence(new_providers),
    );
    Value::Mapping(new_plan)
}

/// Apply provider preferences to a mount plan.
///
/// Finds the first preferred provider that exists in the mount plan,
/// promotes it to priority 0 (highest), and sets its model.
///
/// Returns a new mount plan with the first matching provider promoted.
/// Returns a clone of the original mount plan if no preferences match.
/// Returns the original mount plan unchanged if preferences is empty.
/// Apply provider preferences to a mount plan, resolving glob model patterns.
///
/// Like [`apply_provider_preferences`], but also resolves glob patterns in
/// model names (e.g., `"claude-haiku-*"` -> `"claude-3-haiku-20240307"`)
/// by querying available models from the provider.
///
/// The `list_models` callback is called with the provider name (e.g., `"anthropic"`)
/// and should return a list of available model names for that provider. It is only
/// called when the model preference contains a glob pattern.
///
/// # Resolution strategy
///
/// 1. If model is not a glob pattern, use as-is (no callback invoked)
/// 2. If model is a glob, call `list_models(provider_name)` to get available models
/// 3. Filter available models with fnmatch-style glob matching
/// 4. Sort matches descending (latest date/version wins)
/// 5. Use first match, or original pattern if no matches
///
/// # Error handling
///
/// The `list_models` callback returns `Vec<String>` (not `Result`). Callers
/// that query models over the network should handle errors internally and
/// return an empty vec as fallback, matching Python's behavior where
/// `resolve_model_pattern` catches exceptions and falls back gracefully.
///
/// # Example
///
/// ```ignore
/// let result = apply_provider_preferences_with_resolution(
///     &mount_plan,
///     &prefs,
///     |provider| async move {
///         query_provider_models(provider).await.unwrap_or_default()
///     },
/// ).await;
/// ```
pub async fn apply_provider_preferences_with_resolution<F, Fut>(
    mount_plan: &Value,
    preferences: &[ProviderPreference],
    list_models: F,
) -> Value
where
    F: Fn(&str) -> Fut + Send + Sync,
    Fut: std::future::Future<Output = Vec<String>> + Send,
{
    if preferences.is_empty() {
        return mount_plan.clone();
    }

    let providers = mount_plan
        .as_mapping()
        .and_then(|m| m.get(Value::String("providers".to_string())))
        .and_then(|v| v.as_sequence());

    let providers = match providers {
        Some(p) if !p.is_empty() => p,
        _ => {
            tracing::warn!("Provider preferences specified but no providers in mount plan");
            return mount_plan.clone();
        }
    };

    let lookup = build_provider_lookup(providers);

    for pref in preferences {
        if let Some(&target_idx) = lookup.get(&pref.provider) {
            // Resolve model pattern if it's a glob
            let resolved_model = if glob::is_glob_pattern(&pref.model) {
                let available = list_models(&pref.provider).await;
                glob::resolve_model_pattern(&pref.model, &available)
                    .unwrap_or_else(|| pref.model.clone())
            } else {
                pref.model.clone()
            };

            return apply_single_override(mount_plan, providers, target_idx, &resolved_model);
        }
    }

    // No preferences matched
    tracing::warn!(
        preferences = ?preferences.iter().map(|p| &p.provider).collect::<Vec<_>>(),
        "No preferred providers found in mount plan"
    );
    mount_plan.clone()
}

/// Apply provider preferences to a mount plan.
///
/// Finds the first preferred provider that exists in the mount plan,
/// promotes it to priority 0 (highest), and sets its model.
///
/// Returns a new mount plan with the first matching provider promoted.
/// Returns a clone of the original mount plan if no preferences match.
/// Returns the original mount plan unchanged if preferences is empty.
pub fn apply_provider_preferences(mount_plan: &Value, preferences: &[ProviderPreference]) -> Value {
    if preferences.is_empty() {
        return mount_plan.clone();
    }

    let providers = mount_plan
        .as_mapping()
        .and_then(|m| m.get(Value::String("providers".to_string())))
        .and_then(|v| v.as_sequence());

    let providers = match providers {
        Some(p) if !p.is_empty() => p,
        _ => {
            // No providers in mount plan
            return mount_plan.clone();
        }
    };

    // Build lookup for efficient matching
    let lookup = build_provider_lookup(providers);

    // Find first matching preference
    for pref in preferences {
        if let Some(&target_idx) = lookup.get(&pref.provider) {
            return apply_single_override(mount_plan, providers, target_idx, &pref.model);
        }
    }

    // No preferences matched
    mount_plan.clone()
}
