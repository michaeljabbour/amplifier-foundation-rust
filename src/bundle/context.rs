use std::path::PathBuf;

use super::Bundle;
use crate::paths::normalize::construct_context_path;

impl Bundle {
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
