//! PreparedBundle: a bundle that has been prepared for execution.
//!
//! Contains the mount plan, module resolver, and original bundle.
//! Provides system prompt factory creation for dynamic @mention resolution.
//!
//! Port of Python `PreparedBundle` from `bundle.py:845-979`.

use crate::bundle::module_resolver::BundleModuleResolver;
use crate::bundle::Bundle;
use crate::mentions::dedup::ContentDeduplicator;
use crate::mentions::loader::{format_context_block, load_mentions};
use crate::mentions::resolver::BaseMentionResolver;
use crate::runtime::SystemPromptFactory;
use futures::future::BoxFuture;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A bundle that has been prepared for execution.
///
/// Contains the mount plan, module resolver, and original bundle for
/// spawning support.
///
/// Port of Python's `PreparedBundle` dataclass (`bundle.py:845-864`).
#[derive(Debug)]
pub struct PreparedBundle {
    /// Configuration for mounting modules.
    pub mount_plan: serde_yaml_ng::Value,
    /// Resolver for finding module paths.
    pub resolver: BundleModuleResolver,
    /// The original Bundle that was prepared.
    pub bundle: Bundle,
    /// Paths to bundle src/ directories added to sys.path.
    /// These need to be shared with child sessions during spawning to ensure
    /// bundle packages remain importable.
    pub bundle_package_paths: Vec<String>,
}

impl PreparedBundle {
    /// Create a new PreparedBundle.
    pub fn new(
        mount_plan: serde_yaml_ng::Value,
        resolver: BundleModuleResolver,
        bundle: Bundle,
    ) -> Self {
        Self {
            mount_plan,
            resolver,
            bundle,
            bundle_package_paths: Vec::new(),
        }
    }

    /// Build bundle registry for mention resolution.
    ///
    /// Maps each namespace to its base path. This allows `@foundation:context/...`
    /// to resolve relative to foundation's source base path.
    ///
    /// Python's version creates `dict[str, Bundle]` with `dataclasses.replace(bundle, base_path=...)`.
    /// The Rust version maps namespace → PathBuf directly since `BaseMentionResolver`
    /// only needs the base path for resolution.
    ///
    /// Takes `bundle` as a parameter (not `self.bundle`) to support spawning scenarios
    /// where the caller may pass a different bundle.
    ///
    /// Port of Python `_build_bundles_for_resolver` (`bundle.py:866-892`).
    pub fn build_bundles_for_resolver(&self, bundle: &Bundle) -> HashMap<String, PathBuf> {
        let mut bundles_for_resolver: HashMap<String, PathBuf> = HashMap::new();

        // Collect namespaces from source_base_paths
        let mut namespaces: Vec<String> = bundle.source_base_paths.keys().cloned().collect();

        // Add the bundle's own name if not already present
        if !bundle.name.is_empty() && !namespaces.contains(&bundle.name) {
            namespaces.push(bundle.name.clone());
        }

        for ns in &namespaces {
            if ns.is_empty() {
                continue;
            }
            // Use source_base_paths entry, or fall back to bundle.base_path
            let ns_base_path = bundle
                .source_base_paths
                .get(ns)
                .cloned()
                .or_else(|| bundle.base_path.clone());

            if let Some(path) = ns_base_path {
                bundles_for_resolver.insert(ns.clone(), path);
            }
        }

        bundles_for_resolver
    }

    /// Create a factory that produces fresh system prompt content on each call.
    ///
    /// The factory re-reads context files and re-processes @mentions each time,
    /// enabling dynamic content like AGENTS.md to be picked up immediately when
    /// modified during a session.
    ///
    /// Takes `bundle` as a parameter (not `self.bundle`) to support spawning
    /// scenarios where the caller may pass a different bundle.
    ///
    /// Port of Python `_create_system_prompt_factory` (`bundle.py:894-979`).
    /// Note: Python's `session` parameter is accepted but never used in the method
    /// body, so it is omitted here.
    pub fn create_system_prompt_factory(
        &self,
        bundle: &Bundle,
        session_cwd: Option<&Path>,
    ) -> Box<dyn SystemPromptFactory> {
        // Capture state for the factory
        let captured_bundle = bundle.clone();
        let bundles_for_resolver = self.build_bundles_for_resolver(bundle);
        let base_path = session_cwd
            .map(|p| p.to_path_buf())
            .or_else(|| bundle.base_path.clone())
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        Box::new(BundleSystemPromptFactory {
            bundle: captured_bundle,
            bundles_for_resolver,
            base_path,
        })
    }
}

/// System prompt factory backed by a bundle.
///
/// Re-reads context files and re-resolves @mentions on every call,
/// enabling dynamic content updates mid-session.
///
/// All fields are owned values (no `Arc` needed — this struct is the sole owner).
/// `Bundle`, `HashMap<String, PathBuf>`, and `PathBuf` are all `Send + Sync`,
/// satisfying the `SystemPromptFactory: Send + Sync` trait bound.
struct BundleSystemPromptFactory {
    bundle: Bundle,
    bundles_for_resolver: HashMap<String, PathBuf>,
    base_path: PathBuf,
}

impl SystemPromptFactory for BundleSystemPromptFactory {
    fn create(&self) -> BoxFuture<'_, String> {
        Box::pin(async move {
            // Build combined instruction: main instruction + all context.include files
            // Re-read files each time to pick up changes
            let mut instruction_parts: Vec<String> = Vec::new();
            if let Some(ref instruction) = self.bundle.instruction {
                instruction_parts.push(instruction.clone());
            }

            // Load and append all context files (re-read each call)
            for (context_name, context_path) in &self.bundle.context {
                if context_path.exists() {
                    if let Ok(content) = std::fs::read_to_string(context_path) {
                        instruction_parts
                            .push(format!("# Context: {}\n\n{}", context_name, content));
                    }
                }
            }

            let combined_instruction = instruction_parts.join("\n\n---\n\n");

            // Build resolver with namespace bundles, shared context dict, and session cwd
            let resolver = BaseMentionResolver {
                bundles: self.bundles_for_resolver.clone(),
                context: self.bundle.context.clone(),
                base_path: self.base_path.clone(),
            };

            // Fresh deduplicator each call (files may have changed)
            let mut deduplicator = ContentDeduplicator::new();

            // Resolve @mentions (re-loads files each call)
            let mention_result = load_mentions(&combined_instruction, &resolver).await;

            // Build mention_to_path map for context block attribution
            let mut mention_to_path: HashMap<String, PathBuf> = HashMap::new();
            for file in &mention_result.files {
                mention_to_path.insert(file.mention.clone(), file.path.clone());
            }

            // Add files to deduplicator for format_context_block
            for file in &mention_result.files {
                deduplicator.add_file(&file.path, &file.content);
            }

            // Format loaded context as XML blocks
            let context_block = format_context_block(&deduplicator, Some(&mention_to_path));

            // Prepend context to instruction
            if !context_block.is_empty() {
                format!("{}\n\n---\n\n{}", context_block, combined_instruction)
            } else {
                combined_instruction
            }
        })
    }
}
