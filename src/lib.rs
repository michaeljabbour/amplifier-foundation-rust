//! # Amplifier Foundation
//!
//! Bundle composition mechanism layer for Amplifier.
//!
//! Foundation provides an ultra-thin mechanism layer for bundle composition
//! that sits between amplifier-core (kernel) and applications.
//!
//! **Core concept:** `Bundle` = composable unit that produces mount plans.
//!
//! **One mechanism:** `includes:` (declarative) + `compose()` (imperative)
//!
//! **Philosophy:** Mechanism not policy, ruthless simplicity.
//!
//! # Quick Start
//!
//! ```rust
//! use amplifier_foundation::{Bundle, BundleError};
//! use serde_yaml_ng::Value;
//!
//! // Create a bundle from YAML data
//! let yaml = r#"
//! bundle:
//!   name: my-bundle
//!   version: "1.0"
//!   providers:
//!     - module: provider-openai
//!       config:
//!         model: gpt-4
//! "#;
//! let data: Value = serde_yaml_ng::from_str(yaml).unwrap();
//! let bundle = Bundle::from_dict(&data).unwrap();
//! assert_eq!(bundle.name, "my-bundle");
//! ```

// =============================================================================
// Module declarations
// =============================================================================
pub mod error;
pub mod runtime;
pub mod serialization;
pub mod tracing_utils;

pub mod bundle;
pub mod cache;
pub mod dicts;
pub mod io;
pub mod mentions;
pub mod modules;
pub mod paths;
pub mod registry;
pub mod session;
pub mod sources;
pub mod spawn;
pub mod updates;

// =============================================================================
// Re-exports -- Flat public API matching Python's __init__.py __all__
// =============================================================================
// Users can write `use amplifier_foundation::Bundle` instead of
// `use amplifier_foundation::bundle::Bundle`.

// -- Core classes --
pub use bundle::Bundle;
pub use registry::{BundleRegistry, BundleState, UpdateInfo};

// -- Errors --
pub use error::{BundleError, Result};
// Note: Python exports BundleNotFoundError, BundleLoadError, BundleValidationError,
// BundleDependencyError as separate exception classes. In Rust these are variants
// of the BundleError enum: BundleError::NotFound, BundleError::LoadError,
// BundleError::ValidationError, BundleError::DependencyError.

// -- Validator --
pub use bundle::validator::{
    validate_bundle, validate_bundle_completeness, validate_bundle_completeness_or_raise,
    validate_bundle_or_raise, BundleValidator, ValidationResult,
};

// -- Registry --
pub use registry::load_bundle;

// -- Protocols / Traits --
pub use cache::CacheProvider;
pub use mentions::MentionResolver;
pub use sources::{SourceHandler, SourceHandlerWithStatus, SourceResolver};

// -- Reference implementations --
pub use cache::{disk::DiskCache, memory::SimpleCache};
pub use mentions::resolver::BaseMentionResolver;
pub use sources::resolver::SimpleSourceResolver;

// -- Source types --
#[cfg(feature = "zip-sources")]
pub use sources::zip::ZipSourceHandler;
pub use sources::SourceStatus;
pub use sources::{file::FileSourceHandler, git::GitSourceHandler, http::HttpSourceHandler};

// -- Mentions --
pub use mentions::dedup::ContentDeduplicator;
pub use mentions::loader::{format_context_block, load_mentions};
pub use mentions::models::{ContextFile, MentionResult, UniqueFile};
pub use mentions::parser::parse_mentions;
pub use mentions::utils::format_directory_listing;

// -- I/O utilities --
pub use io::files::{
    read_with_retry, write_with_backup, write_with_backup_bytes, write_with_retry,
};
pub use io::frontmatter::parse_frontmatter;
pub use io::yaml::{read_yaml, write_yaml};

// -- Serialization --
pub use serialization::{sanitize_for_json, sanitize_for_json_with_depth, sanitize_message};

// -- Tracing --
pub use tracing_utils::generate_sub_session_id;

// -- Dict utilities --
pub use dicts::merge::{deep_merge, merge_module_lists};
pub use dicts::nested::{get_nested, get_nested_with_default, set_nested};

// -- Path utilities --
pub use paths::discovery::{find_bundle_root, find_files};
pub use paths::normalize::{construct_agent_path, construct_context_path};
pub use paths::uri::{get_amplifier_home, normalize_path, parse_uri, ParsedURI, ResolvedSource};

// -- Session --
pub use session::capabilities::{get_working_dir, set_working_dir, WORKING_DIR_CAPABILITY};
pub use session::{
    add_synthetic_tool_results, count_events, count_turns, find_orphaned_tool_calls, fork_session,
    fork_session_in_memory, get_event_summary, get_fork_preview, get_last_timestamp_for_turn,
    get_session_lineage, get_turn_boundaries, get_turn_summary, list_session_forks,
    slice_events_for_fork, slice_events_to_timestamp, slice_to_turn, ForkResult,
};

// -- Spawn utilities --
pub use spawn::glob::{is_glob_pattern, resolve_model_pattern};
pub use spawn::{
    apply_provider_preferences, apply_provider_preferences_with_resolution, ModelResolutionResult,
    ProviderPreference,
};

// -- Updates --
pub use updates::{check_bundle_status, update_bundle, BundleStatus};

// -- Modules --
pub use modules::state::InstallStateManager;

// -- Runtime traits --
pub use runtime::{
    AmplifierRuntime, AmplifierSession, ApprovalSystem, ContextManager, Coordinator, DisplaySystem,
    HookHandler, HookRegistry, SessionOptions, SystemPromptFactory,
};
