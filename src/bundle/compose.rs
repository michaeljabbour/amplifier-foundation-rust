use super::helpers::is_null_or_empty_mapping;
use super::Bundle;
use crate::dicts::merge::{deep_merge, merge_module_lists};

impl Bundle {
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
}
