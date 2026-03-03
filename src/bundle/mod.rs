mod compose;
pub mod module_resolver;
pub mod mount;
pub mod prepared;
pub mod validator;

// Future modules: These are reserved for functionality that depends on
// AmplifierRuntime/AmplifierSession (declared in src/runtime.rs).
//
// - prepared: PreparedBundle struct + build_bundles_for_resolver + system prompt factory (DONE)
// - Remaining: create_session and spawn depend on AmplifierSession runtime

use indexmap::IndexMap;
use serde_yaml_ng::{Mapping, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::dicts::merge::{deep_merge, merge_module_lists};
use crate::io::frontmatter::parse_frontmatter;
use crate::paths::normalize::construct_context_path;

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

    /// Compose this bundle with one or more overlays using the 5-strategy system.
    ///
    /// Strategy mapping (from architecture spec):
    /// 1. Deep merge: session, spawn
    /// 2. Merge by module ID: providers, tools, hooks
    /// 3. Dict update: agents (later wins by key)
    /// 4. Accumulate with namespace: context
    /// 5. Later replaces entirely: instruction, base_path, name, version, description
    pub fn compose(&self, others: &[&Bundle]) -> Bundle {
        let mut result = self.clone();

        for other in others {
            // Strategy 5: Later replaces entirely
            result.name = other.name.clone();
            result.version = other.version.clone();
            if !other.description.is_empty() {
                result.description = other.description.clone();
            }

            // Strategy 5: instruction -- later replaces if set
            if other.instruction.is_some() {
                result.instruction = other.instruction.clone();
            }

            // Strategy 1: Deep merge for session and spawn
            if !is_null_or_empty_mapping(&other.session) {
                if is_null_or_empty_mapping(&result.session) {
                    result.session = other.session.clone();
                } else {
                    result.session = deep_merge(&result.session, &other.session);
                }
            }
            if !is_null_or_empty_mapping(&other.spawn) {
                if is_null_or_empty_mapping(&result.spawn) {
                    result.spawn = other.spawn.clone();
                } else {
                    result.spawn = deep_merge(&result.spawn, &other.spawn);
                }
            }

            // Strategy 2: Merge by module ID for providers, tools, hooks
            if !other.providers.is_empty() {
                result.providers = merge_module_lists(&result.providers, &other.providers);
            }
            if !other.tools.is_empty() {
                result.tools = merge_module_lists(&result.tools, &other.tools);
            }
            if !other.hooks.is_empty() {
                result.hooks = merge_module_lists(&result.hooks, &other.hooks);
            }

            // Strategy 3: Dict update for agents
            // Note: IndexMap::insert keeps existing keys at their original position
            // and appends new keys at the end. Matches Python dict.update() semantics.
            for (name, agent) in &other.agents {
                result.agents.insert(name.clone(), agent.clone());
            }

            // Strategy 4: Accumulate with namespace for context
            // Note: existing context keys are kept as-is (they were already
            // prefixed by previous compose iterations or set at parse time).
            // New entries from other get prefixed with other's bundle name.
            for (key, path) in &other.context {
                let prefixed_key = if !other.name.is_empty() && !key.contains(':') {
                    format!("{}:{}", other.name, key)
                } else {
                    key.clone()
                };
                result.context.insert(prefixed_key, path.clone());
            }

            // Accumulate pending_context
            for (key, val) in &other.pending_context {
                result.pending_context.insert(key.clone(), val.clone());
            }

            // source_base_paths: merge (other's entries win for conflicts)
            for (ns, path) in &other.source_base_paths {
                result.source_base_paths.insert(ns.clone(), path.clone());
            }
            // Also register other.name -> other.base_path
            if !other.name.is_empty() {
                if let Some(bp) = &other.base_path {
                    result
                        .source_base_paths
                        .entry(other.name.clone())
                        .or_insert_with(|| bp.clone());
                }
            }

            // Strategy 5: base_path -- takes other's if set
            if other.base_path.is_some() {
                result.base_path = other.base_path.clone();
            }

            // Includes -- accumulate
            for inc in &other.includes {
                if !result.includes.contains(inc) {
                    result.includes.push(inc.clone());
                }
            }

            // Extra -- deep merge
            if !other.extra.is_null() {
                if result.extra.is_null() {
                    result.extra = other.extra.clone();
                } else {
                    result.extra = deep_merge(&result.extra, &other.extra);
                }
            }
        }

        result
    }

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

    /// Resolve a context file reference.
    /// 1. Check registered context dict (exact match)
    /// 2. Try constructing path from base_path
    pub fn resolve_context_path(&self, name: &str) -> Option<PathBuf> {
        // Exact match in context dict
        if let Some(path) = self.context.get(name) {
            return Some(path.clone());
        }

        // Try constructing from base_path
        if let Some(bp) = &self.base_path {
            let candidate = construct_context_path(bp, name);
            if candidate.exists() {
                return Some(candidate);
            }
        }

        None
    }

    /// Resolve pending context references using source_base_paths.
    /// Drains pending_context entries that can be resolved into the context map.
    pub fn resolve_pending_context(&mut self) {
        if self.pending_context.is_empty() {
            return;
        }

        let pending: Vec<(String, String)> = self.pending_context.drain().collect();

        for (name, ref_str) in pending {
            if !ref_str.contains(':') {
                // Not a namespaced ref, can't resolve
                self.pending_context.insert(name, ref_str);
                continue;
            }

            let (namespace, path_part) = match ref_str.split_once(':') {
                Some((ns, p)) => (ns, p),
                None => {
                    self.pending_context.insert(name, ref_str);
                    continue;
                }
            };

            if let Some(base) = self.source_base_paths.get(namespace) {
                let resolved_path = construct_context_path(base, path_part);
                self.context.insert(name, resolved_path);
            } else if let Some(bp) = self.base_path.as_ref() {
                if namespace == self.name {
                    // Self-referencing fallback
                    let resolved_path = construct_context_path(bp, path_part);
                    self.context.insert(name, resolved_path);
                } else {
                    // Can't resolve yet, put back
                    self.pending_context.insert(name, ref_str);
                }
            } else {
                // Can't resolve yet, put back
                self.pending_context.insert(name, ref_str);
            }
        }
    }
}

/// Helper: get human-readable type name for a YAML Value.
fn value_type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "str",
        Value::Sequence(_) => "list",
        Value::Mapping(_) => "dict",
        Value::Tagged(_) => "tagged",
    }
}

/// Helper: check if a Value is Null or an empty mapping.
fn is_null_or_empty_mapping(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::Mapping(m) => m.is_empty(),
        _ => false,
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
fn load_agent_file_metadata(
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
fn get_agent_meta(frontmatter: &Value) -> Option<&Mapping> {
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
fn merge_agent_metadata(agent_config: &mut Value, file_metadata: &Value) {
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
fn is_falsy_value(v: &Value) -> bool {
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
