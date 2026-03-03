use std::path::PathBuf;

use serde_yaml_ng::{Mapping, Value};

use super::Bundle;
use crate::io::frontmatter::parse_frontmatter;

impl Bundle {
    /// Resolve agent file by name.
    ///
    /// Handles both namespaced and simple names:
    /// - `"foundation:bug-hunter"` → looks in `source_base_paths["foundation"]/agents/`
    /// - `"bug-hunter"` → looks in `self.base_path/agents/`
    ///
    /// For namespaced agents, `source_base_paths` is checked first. If the namespace
    /// matches `self.name` and the source_base_paths lookup fails, falls back to
    /// `self.base_path`.
    ///
    /// Returns `None` if the agent `.md` file does not exist at the resolved path.
    pub fn resolve_agent_path(&self, name: &str) -> Option<PathBuf> {
        if let Some((namespace, simple_name)) = name.split_once(':') {
            // Namespaced agent (e.g., "foundation:bug-hunter")

            // First, try source_base_paths for included bundles
            if let Some(base) = self.source_base_paths.get(namespace) {
                let agent_path = base.join("agents").join(format!("{simple_name}.md"));
                if agent_path.exists() {
                    return Some(agent_path);
                }
            }

            // Fall back to self.base_path if namespace matches self.name
            if namespace == self.name {
                if let Some(bp) = &self.base_path {
                    let agent_path = bp.join("agents").join(format!("{simple_name}.md"));
                    if agent_path.exists() {
                        return Some(agent_path);
                    }
                }
            }
        } else if let Some(bp) = &self.base_path {
            // No namespace -- look in self.base_path
            let agent_path = bp.join("agents").join(format!("{name}.md"));
            if agent_path.exists() {
                return Some(agent_path);
            }
        }

        None
    }

    /// Get the system instruction for this bundle.
    ///
    /// Returns the instruction text, or `None` if not set.
    /// This is the content from the markdown body of the bundle file.
    pub fn get_system_instruction(&self) -> Option<&str> {
        self.instruction.as_deref()
    }

    /// Load full metadata for all agents from their `.md` files.
    ///
    /// Updates `self.agents` in-place with description and other meta fields
    /// loaded from agent `.md` files. Uses `resolve_agent_path()` to find files.
    ///
    /// Call after composition when `source_base_paths` is fully populated.
    ///
    /// Agents with inline definitions (description already set) are preserved;
    /// file metadata only fills in missing or falsy fields.
    pub fn load_agent_metadata(&mut self) {
        if self.agents.is_empty() {
            return;
        }

        let agent_names: Vec<String> = self.agents.keys().cloned().collect();

        for agent_name in &agent_names {
            // resolve_agent_path already checks .exists() at every return site.
            // The guard here is a TOCTOU safety belt: the file could be deleted
            // between resolution and read. load_agent_file_metadata handles that
            // via read_to_string returning Err, but this avoids the round-trip.
            let path = match self.resolve_agent_path(agent_name) {
                Some(p) if p.exists() => p,
                _ => continue,
            };

            match load_agent_file_metadata(&path, agent_name) {
                Ok(file_metadata) => {
                    if let Some(agent_config) = self.agents.get_mut(agent_name) {
                        merge_agent_metadata(agent_config, &file_metadata);
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to load metadata for agent '{}': {}", agent_name, e);
                }
            }
        }
    }
}

/// Load agent config from a `.md` file.
///
/// Extracts both metadata (name, description) from the `meta:` section AND
/// mount plan sections (tools, providers, hooks, session) from top-level
/// frontmatter. This allows agents to define their own tools that will be
/// used when the agent is spawned.
///
/// Returns a `Value::Mapping` with name, description, instruction (from body),
/// and optionally tools, providers, hooks, session if defined.
pub(super) fn load_agent_file_metadata(
    path: &std::path::Path,
    fallback_name: &str,
) -> crate::error::Result<Value> {
    let text = std::fs::read_to_string(path).map_err(|e| crate::error::BundleError::LoadError {
        reason: format!("Failed to read agent file {}: {}", path.display(), e),
        source: Some(Box::new(e)),
    })?;

    let (frontmatter_opt, body) = parse_frontmatter(&text)?;
    let frontmatter = frontmatter_opt.unwrap_or(Value::Null);

    // Agents use meta: section (not bundle:)
    let meta = get_agent_meta(&frontmatter);

    let mut result = Mapping::new();

    // Name from meta or fallback
    let name = meta
        .and_then(|m| m.get(Value::String("name".to_string())))
        .and_then(|v| v.as_str())
        .unwrap_or(fallback_name);
    result.insert(
        Value::String("name".to_string()),
        Value::String(name.to_string()),
    );

    // Description from meta or empty
    let description = meta
        .and_then(|m| m.get(Value::String("description".to_string())))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    result.insert(
        Value::String("description".to_string()),
        Value::String(description.to_string()),
    );

    // Extra meta fields (not name/description, not mount plan sections)
    // Mount plan sections are handled separately below to ensure they come
    // from the top-level frontmatter, not the meta section.
    const MOUNT_PLAN_SECTIONS: &[&str] = &["tools", "providers", "hooks", "session"];
    if let Some(meta_map) = meta {
        for (key, value) in meta_map {
            if let Some(key_str) = key.as_str() {
                if key_str != "name"
                    && key_str != "description"
                    && !MOUNT_PLAN_SECTIONS.contains(&key_str)
                {
                    result.insert(key.clone(), value.clone());
                }
            }
        }
    }

    // Extract top-level mount plan sections (siblings to meta:, not nested inside it)
    let fm_map = frontmatter.as_mapping();
    for section in &["tools", "providers", "hooks", "session"] {
        if let Some(fm) = fm_map {
            if let Some(value) = fm.get(Value::String(section.to_string())) {
                result.insert(Value::String(section.to_string()), value.clone());
            }
        }
    }

    // Include instruction from markdown body
    let trimmed = body.trim();
    if !trimmed.is_empty() {
        result.insert(
            Value::String("instruction".to_string()),
            Value::String(trimmed.to_string()),
        );
    }

    Ok(Value::Mapping(result))
}

/// Extract the meta section from agent frontmatter.
///
/// Agents use `meta:` section. If absent, falls back to flat frontmatter
/// if it contains `name` or `description` keys.
pub(super) fn get_agent_meta(frontmatter: &Value) -> Option<&Mapping> {
    let fm_map = frontmatter.as_mapping()?;

    // Try meta: section first
    if let Some(meta_val) = fm_map.get(Value::String("meta".to_string())) {
        if let Some(meta_map) = meta_val.as_mapping() {
            if !meta_map.is_empty() {
                return Some(meta_map);
            }
        }
    }

    // Fall back to flat frontmatter if it has name or description
    if fm_map.contains_key(Value::String("name".to_string()))
        || fm_map.contains_key(Value::String("description".to_string()))
    {
        return Some(fm_map);
    }

    None
}

/// Merge file metadata into an agent config Value.
///
/// File metadata fills gaps: only inserts keys that are missing or have
/// falsy values in the existing agent config. Matches Python's:
/// ```python
/// if key not in agent_config or not agent_config.get(key):
///     agent_config[key] = value
/// ```
pub(super) fn merge_agent_metadata(agent_config: &mut Value, file_metadata: &Value) {
    let file_map = match file_metadata.as_mapping() {
        Some(m) => m,
        None => return,
    };

    // If agent_config is not a mapping, skip merge (matches Python where
    // TypeError on non-dict merge is caught by except Exception and the
    // agent value stays as-is)
    if !agent_config.is_mapping() {
        return;
    }

    let config_map = agent_config.as_mapping_mut().unwrap();

    for (key, value) in file_map {
        let existing = config_map.get(key);
        let should_insert = match existing {
            None => true,
            Some(v) => is_falsy_value(v),
        };
        if should_insert {
            config_map.insert(key.clone(), value.clone());
        }
    }
}

/// Check if a YAML Value is "falsy" (matching Python's truthiness rules).
///
/// Python's `not agent_config.get(key)` returns True for:
/// - None (Null)
/// - "" (empty string)
/// - [] (empty sequence)
/// - {} (empty mapping)
/// - False
/// - 0
pub(super) fn is_falsy_value(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::Bool(b) => !b,
        Value::Number(n) => n.as_f64().map_or(true, |f| f == 0.0),
        Value::String(s) => s.is_empty(),
        Value::Sequence(seq) => seq.is_empty(),
        Value::Mapping(m) => m.is_empty(),
        Value::Tagged(_) => false,
    }
}
