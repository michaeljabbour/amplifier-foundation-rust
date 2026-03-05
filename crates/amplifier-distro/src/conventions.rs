//! Path constants for the Amplifier distribution layer.
//!
//! All directory and file paths used by amplifier-distro are defined here.
//! Paths are returned as functions (not static strings) because `~` expansion
//! requires runtime evaluation.

use std::path::PathBuf;

/// Get the Amplifier home directory (delegates to amplifier_foundation).
pub fn amplifier_home() -> PathBuf {
    amplifier_foundation::get_amplifier_home()
}

/// Get the Amplifier distro home directory.
///
/// Resolved in order:
/// 1. `AMPLIFIER_DISTRO_HOME` env var (with `~` expansion)
/// 2. `~/.amplifier-distro` (default)
pub fn distro_home() -> PathBuf {
    if let Ok(val) = std::env::var("AMPLIFIER_DISTRO_HOME") {
        PathBuf::from(shellexpand_tilde(&val))
    } else {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".amplifier-distro")
    }
}

/// Expand a leading `~` to the home directory.
fn shellexpand_tilde(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return format!("{}/{rest}", home.display());
        }
    }
    if path == "~" {
        if let Some(home) = dirs::home_dir() {
            return home.to_string_lossy().into_owned();
        }
    }
    path.to_string()
}

// ---------------------------------------------------------------------------
// File name constants
// ---------------------------------------------------------------------------

pub const KEYS_FILENAME: &str = "keys.env";
pub const SETTINGS_FILENAME: &str = "settings.yaml";
pub const DISTRO_SETTINGS_FILENAME: &str = "settings.yaml";
pub const TRANSCRIPT_FILENAME: &str = "transcript.jsonl";
pub const SESSION_INFO_FILENAME: &str = "session-info.json";
pub const METADATA_FILENAME: &str = "metadata.json";
pub const PROJECTS_DIR: &str = "projects";
pub const CACHE_DIR: &str = "cache";
pub const MEMORY_DIR: &str = "memory";
pub const MEMORY_STORE_FILENAME: &str = "memory-store.yaml";
pub const WORK_LOG_FILENAME: &str = "work-log.yaml";
pub const SERVER_DIR: &str = "server";
pub const SERVER_DEFAULT_PORT: u16 = 8400;

// ---------------------------------------------------------------------------
// Derived path functions
// ---------------------------------------------------------------------------

/// Path to the global keys.env file: `$AMPLIFIER_HOME/keys.env`
pub fn keys_env_path() -> PathBuf {
    amplifier_home().join(KEYS_FILENAME)
}

/// Path to the global settings file: `$AMPLIFIER_HOME/settings.yaml`
pub fn global_settings_path() -> PathBuf {
    amplifier_home().join(SETTINGS_FILENAME)
}

/// Path to the distro settings file: `$AMPLIFIER_DISTRO_HOME/settings.yaml`
pub fn distro_settings_path() -> PathBuf {
    distro_home().join(DISTRO_SETTINGS_FILENAME)
}

/// Path to the distro bundle overlay directory: `$AMPLIFIER_DISTRO_HOME/bundle`
pub fn distro_overlay_dir() -> PathBuf {
    distro_home().join("bundle")
}

/// Path to the distro bundle.yaml overlay: `$AMPLIFIER_DISTRO_HOME/bundle/bundle.yaml`
pub fn distro_overlay_bundle_path() -> PathBuf {
    distro_overlay_dir().join("bundle.yaml")
}

/// Path to the distro sessions directory: `$AMPLIFIER_DISTRO_HOME/sessions`
pub fn distro_sessions_dir() -> PathBuf {
    distro_home().join("sessions")
}

/// Path to the distro certificates directory: `$AMPLIFIER_DISTRO_HOME/certs`
pub fn distro_certs_dir() -> PathBuf {
    distro_home().join("certs")
}

/// Path to the bundle/source cache: `$AMPLIFIER_HOME/cache`
pub fn cache_dir() -> PathBuf {
    amplifier_home().join(CACHE_DIR)
}

/// Path to the projects directory: `$AMPLIFIER_HOME/projects`
pub fn projects_dir() -> PathBuf {
    amplifier_home().join(PROJECTS_DIR)
}

/// Path to the memory directory: `$AMPLIFIER_HOME/memory`
pub fn memory_dir() -> PathBuf {
    amplifier_home().join(MEMORY_DIR)
}

/// Path to the server directory: `$AMPLIFIER_DISTRO_HOME/server`
pub fn server_dir() -> PathBuf {
    distro_home().join(SERVER_DIR)
}

/// Path to a specific session directory:
/// `$AMPLIFIER_HOME/projects/<project_slug>/sessions/<session_id>`
pub fn session_dir(project_slug: &str, session_id: &str) -> PathBuf {
    projects_dir()
        .join(project_slug)
        .join("sessions")
        .join(session_id)
}
