use std::collections::HashMap;
use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use serde_yaml_ng::{Mapping, Value};

use super::helpers::{is_null_or_empty_mapping, value_type_name};
use super::Bundle;

impl Bundle {
    pub fn from_dict(data: &Value) -> crate::error::Result<Self> {
        Self::from_dict_with_base_path_opt(data, None)
    }

    pub fn from_dict_with_base_path(data: &Value, base_path: &Path) -> crate::error::Result<Self> {
        Self::from_dict_with_base_path_opt(data, Some(base_path))
    }

    fn from_dict_with_base_path_opt(
        data: &Value,
        base_path: Option<&Path>,
    ) -> crate::error::Result<Self> {
        let empty_mapping = Mapping::new();
        let bundle_meta = data
            .as_mapping()
            .and_then(|m| m.get(Value::String("bundle".to_string())))
            .and_then(|v| v.as_mapping())
            .unwrap_or(&empty_mapping);

        let bundle_name = bundle_meta
            .get(Value::String("name".to_string()))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let version = bundle_meta
            .get(Value::String("version".to_string()))
            .and_then(|v| v.as_str())
            .unwrap_or("1.0.0")
            .to_string();

        let description = bundle_meta
            .get(Value::String("description".to_string()))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let bundle_identifier = if !bundle_name.is_empty() {
            bundle_name.clone()
        } else if let Some(bp) = base_path {
            bp.display().to_string()
        } else {
            "unknown".to_string()
        };

        // Validate and extract module lists
        let providers = Self::validate_module_list(
            bundle_meta.get(Value::String("providers".to_string())),
            "providers",
            &bundle_identifier,
            base_path,
        )?;
        let tools = Self::validate_module_list(
            bundle_meta.get(Value::String("tools".to_string())),
            "tools",
            &bundle_identifier,
            base_path,
        )?;
        let hooks = Self::validate_module_list(
            bundle_meta.get(Value::String("hooks".to_string())),
            "hooks",
            &bundle_identifier,
            base_path,
        )?;

        // Session
        let session = bundle_meta
            .get(Value::String("session".to_string()))
            .cloned()
            .unwrap_or(Value::Null);

        // Spawn
        let spawn = bundle_meta
            .get(Value::String("spawn".to_string()))
            .cloned()
            .unwrap_or(Value::Null);

        // Includes
        let includes = bundle_meta
            .get(Value::String("includes".to_string()))
            .and_then(|v| v.as_sequence())
            .cloned()
            .unwrap_or_default();

        // Parse context: split into resolved (local) and pending (namespaced)
        let context_config = bundle_meta
            .get(Value::String("context".to_string()))
            .and_then(|v| v.as_mapping());
        let (resolved_context, pending_context) = Self::parse_context(context_config, base_path);

        // Parse agents
        let agents = Self::parse_agents(bundle_meta.get(Value::String("agents".to_string())));

        Ok(Bundle {
            name: bundle_name,
            version,
            description,
            includes,
            session,
            providers,
            tools,
            hooks,
            spawn,
            agents,
            context: resolved_context,
            instruction: None,
            base_path: base_path.map(|p| p.to_path_buf()),
            source_base_paths: HashMap::new(),
            pending_context,
            extra: Value::Null,
            source_uri: None,
        })
    }

    /// Validate that a module list (providers/tools/hooks) contains only mappings.
    /// Rejects bare strings, giving a helpful error message matching the Python behavior.
    fn validate_module_list(
        items: Option<&Value>,
        field_name: &str,
        bundle_identifier: &str,
        base_path: Option<&Path>,
    ) -> crate::error::Result<Vec<Value>> {
        let items = match items {
            None | Some(Value::Null) => return Ok(Vec::new()),
            Some(v) => v,
        };

        let seq = match items.as_sequence() {
            Some(s) => s,
            None => {
                let type_name = value_type_name(items);
                return Err(crate::error::BundleError::LoadError {
                    reason: format!(
                        "Bundle '{}' has malformed {}: expected list, got {}.\n\
                         Correct format: {}: [{{module: 'module-id', source: 'git+https://...'}}]",
                        bundle_identifier, field_name, type_name, field_name
                    ),
                    source: None,
                });
            }
        };

        if seq.is_empty() {
            return Ok(Vec::new());
        }

        let mut result = Vec::with_capacity(seq.len());
        for (i, item) in seq.iter().enumerate() {
            if !item.is_mapping() {
                let type_name = value_type_name(item);
                let item_repr = match item.as_str() {
                    Some(s) => format!("'{}'", s),
                    None => format!("{:?}", item),
                };
                return Err(crate::error::BundleError::LoadError {
                    reason: format!(
                        "Bundle '{}' has malformed {}[{}]: \
                         expected dict with 'module' and 'source' keys, got {} {}.\n\
                         Correct format: {}: [{{module: 'module-id', source: 'git+https://...'}}]",
                        bundle_identifier, field_name, i, type_name, item_repr, field_name
                    ),
                    source: None,
                });
            }

            // Resolve relative source paths to absolute at parse time
            if let Some(bp) = base_path {
                if let Some(map) = item.as_mapping() {
                    let source_key = Value::String("source".to_string());
                    if let Some(Value::String(source)) = map.get(&source_key) {
                        if source.starts_with("./") || source.starts_with("../") {
                            let resolved = bp.join(source);
                            let resolved_str = resolved.display().to_string();
                            let mut new_map = map.clone();
                            new_map.insert(source_key, Value::String(resolved_str));
                            result.push(Value::Mapping(new_map));
                            continue;
                        }
                    }
                }
            }
            result.push(item.clone());
        }

        Ok(result)
    }

    /// Parse context config into resolved (local paths) and pending (namespaced refs).
    fn parse_context(
        context_config: Option<&Mapping>,
        base_path: Option<&Path>,
    ) -> (IndexMap<String, PathBuf>, HashMap<String, String>) {
        let mut resolved: IndexMap<String, PathBuf> = IndexMap::new();
        let mut pending: HashMap<String, String> = HashMap::new();

        let config = match context_config {
            Some(c) => c,
            None => return (resolved, pending),
        };

        for (key, value) in config {
            let key_str = match key.as_str() {
                Some(s) => s.to_string(),
                None => continue,
            };
            let value_str = match value.as_str() {
                Some(s) => s.to_string(),
                None => continue,
            };

            // Check if key or value contains ":" — indicates namespaced reference
            if key_str.contains(':') || value_str.contains(':') {
                pending.insert(key_str, value_str);
            } else if let Some(bp) = base_path {
                resolved.insert(key_str, bp.join(&value_str));
            } else {
                resolved.insert(key_str, PathBuf::from(&value_str));
            }
        }

        (resolved, pending)
    }

    /// Parse agents config into a name->value map.
    fn parse_agents(agents_config: Option<&Value>) -> IndexMap<String, Value> {
        let mut result = IndexMap::new();
        let config = match agents_config {
            Some(Value::Mapping(m)) => m,
            _ => return result,
        };

        for (key, value) in config {
            if let Some(key_str) = key.as_str() {
                result.insert(key_str.to_string(), value.clone());
            }
        }

        result
    }

    /// Serialize this Bundle to a Value that `from_dict()` can reconstruct.
    ///
    /// All fields are nested under the `"bundle"` key to match `from_dict()` expectations.
    /// `Bundle::from_dict(&bundle.to_dict())` preserves: name, version, description,
    /// providers, tools, hooks, session, spawn, agents, context, includes.
    ///
    /// **Not serialized** (by design): `instruction` (from_dict always sets None),
    /// `pending_context` (unresolved refs should be resolved before serializing),
    /// `base_path`, `source_base_paths`, `extra`, `source_uri` (internal state).
    ///
    /// **Edge case**: empty Mapping values for session/spawn are treated as absent
    /// and will deserialize as Value::Null (semantically equivalent).
    pub fn to_dict(&self) -> Value {
        let mut map = Mapping::new();

        let mut bundle_meta = Mapping::new();
        bundle_meta.insert(
            Value::String("name".to_string()),
            Value::String(self.name.clone()),
        );
        bundle_meta.insert(
            Value::String("version".to_string()),
            Value::String(self.version.clone()),
        );
        if !self.description.is_empty() {
            bundle_meta.insert(
                Value::String("description".to_string()),
                Value::String(self.description.clone()),
            );
        }

        // Module lists
        if !self.providers.is_empty() {
            bundle_meta.insert(
                Value::String("providers".to_string()),
                Value::Sequence(self.providers.clone()),
            );
        }
        if !self.tools.is_empty() {
            bundle_meta.insert(
                Value::String("tools".to_string()),
                Value::Sequence(self.tools.clone()),
            );
        }
        if !self.hooks.is_empty() {
            bundle_meta.insert(
                Value::String("hooks".to_string()),
                Value::Sequence(self.hooks.clone()),
            );
        }

        // Session and spawn configs
        if !is_null_or_empty_mapping(&self.session) {
            bundle_meta.insert(Value::String("session".to_string()), self.session.clone());
        }
        if !is_null_or_empty_mapping(&self.spawn) {
            bundle_meta.insert(Value::String("spawn".to_string()), self.spawn.clone());
        }

        // Agents
        if !self.agents.is_empty() {
            let mut agents_map = Mapping::new();
            for (name, agent) in &self.agents {
                agents_map.insert(Value::String(name.clone()), agent.clone());
            }
            bundle_meta.insert(
                Value::String("agents".to_string()),
                Value::Mapping(agents_map),
            );
        }

        // Context (serialize paths as strings)
        if !self.context.is_empty() {
            let mut context_map = Mapping::new();
            for (name, path) in &self.context {
                context_map.insert(
                    Value::String(name.clone()),
                    Value::String(path.display().to_string()),
                );
            }
            bundle_meta.insert(
                Value::String("context".to_string()),
                Value::Mapping(context_map),
            );
        }

        // Includes
        if !self.includes.is_empty() {
            bundle_meta.insert(
                Value::String("includes".to_string()),
                Value::Sequence(self.includes.clone()),
            );
        }

        map.insert(
            Value::String("bundle".to_string()),
            Value::Mapping(bundle_meta),
        );

        Value::Mapping(map)
    }
}
