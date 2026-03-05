//! # amplifier-distro
//!
//! Distribution layer for the Amplifier modular agent system.
//!
//! Provides the session backend, bundle overlay management, provider/feature
//! catalog, settings persistence, and health diagnostics — porting the Python
//! `amplifier-distro` package into Rust.
//!
//! ## Module map
//!
//! | Module | Responsibility |
//! |--------|----------------|
//! | [`conventions`] | All path constants and directory functions |
//! | [`error`] | [`DistroError`] and [`Result`] alias |
//! | [`settings`] | [`DistroSettings`] load / save / update / export |
//! | [`overlay`] | Bundle overlay CRUD (`bundle.yaml` management) |
//! | [`features`] | Provider/feature catalog + registration helpers |
//! | [`doctor`] | Health checks and auto-fix |
//! | [`session`] | Session backend, transcript, metadata, protocols |

pub mod conventions;
pub mod doctor;
pub mod error;
pub mod features;
pub mod overlay;
pub mod session;
pub mod settings;

// ---------------------------------------------------------------------------
// Flat re-exports
// ---------------------------------------------------------------------------

pub use error::{DistroError, Result};

// Conventions — all path functions available at the crate root.
pub use conventions::{
    amplifier_home, cache_dir, distro_certs_dir, distro_home, distro_overlay_bundle_path,
    distro_overlay_dir, distro_sessions_dir, distro_settings_path, global_settings_path,
    keys_env_path, memory_dir, projects_dir, server_dir, session_dir,
    // Constants
    CACHE_DIR, DISTRO_SETTINGS_FILENAME, KEYS_FILENAME, MEMORY_DIR, MEMORY_STORE_FILENAME,
    METADATA_FILENAME, PROJECTS_DIR, SERVER_DEFAULT_PORT, SERVER_DIR, SESSION_INFO_FILENAME,
    SETTINGS_FILENAME, TRANSCRIPT_FILENAME, WORK_LOG_FILENAME,
};

// Settings
pub use settings::DistroSettings;

// Session types
pub use session::{FoundationBackend, SessionBackend, SessionInfo};
