//! Distro settings — load, save, update, export.
//!
//! Settings live at `$AMPLIFIER_DISTRO_HOME/settings.yaml`.
//! All writes are atomic (temp-file + rename).

use std::io::Write;
use std::path::Path;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::DistroError;

/// Global lock serialises concurrent load/save on the same process.
static SETTINGS_LOCK: Mutex<()> = Mutex::new(());

// ---------------------------------------------------------------------------
// Sub-structs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IdentitySettings {
    #[serde(default)]
    pub github_handle: String,
    #[serde(default)]
    pub git_email: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BackupSettings {
    #[serde(default)]
    pub repo_name: String,
    #[serde(default)]
    pub repo_owner: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackSettings {
    #[serde(default)]
    pub hub_channel_id: String,
    #[serde(default)]
    pub hub_channel_name: String,
    #[serde(default)]
    pub socket_mode: bool,
    #[serde(default)]
    pub default_working_dir: String,
    #[serde(default)]
    pub simulator_mode: bool,
    #[serde(default)]
    pub thread_per_session: bool,
    #[serde(default)]
    pub allow_breakout: bool,
    #[serde(default)]
    pub channel_prefix: String,
    #[serde(default)]
    pub bot_name: String,
    #[serde(default)]
    pub default_bundle: String,
    #[serde(default = "default_max_message_length")]
    pub max_message_length: u32,
    #[serde(default = "default_response_timeout")]
    pub response_timeout: u32,
}

fn default_max_message_length() -> u32 {
    3900
}
fn default_response_timeout() -> u32 {
    300
}

impl Default for SlackSettings {
    fn default() -> Self {
        Self {
            hub_channel_id: String::new(),
            hub_channel_name: String::new(),
            socket_mode: false,
            default_working_dir: String::new(),
            simulator_mode: false,
            thread_per_session: false,
            allow_breakout: false,
            channel_prefix: String::new(),
            bot_name: String::new(),
            default_bundle: String::new(),
            max_message_length: default_max_message_length(),
            response_timeout: default_response_timeout(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VoiceSettings {
    #[serde(default)]
    pub voice: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub instructions: String,
    #[serde(default)]
    pub tools_enabled: bool,
    #[serde(default)]
    pub assistant_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchdogSettings {
    #[serde(default = "default_check_interval")]
    pub check_interval: u32,
    #[serde(default = "default_restart_after")]
    pub restart_after: u32,
    #[serde(default = "default_max_restarts")]
    pub max_restarts: u32,
}

fn default_check_interval() -> u32 {
    30
}
fn default_restart_after() -> u32 {
    300
}
fn default_max_restarts() -> u32 {
    5
}

impl Default for WatchdogSettings {
    fn default() -> Self {
        Self {
            check_interval: default_check_interval(),
            restart_after: default_restart_after(),
            max_restarts: default_max_restarts(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsSettings {
    #[serde(default = "default_tls_mode")]
    pub mode: String,
    #[serde(default)]
    pub certfile: String,
    #[serde(default)]
    pub keyfile: String,
}

fn default_tls_mode() -> String {
    "off".to_string()
}

impl Default for TlsSettings {
    fn default() -> Self {
        Self {
            mode: default_tls_mode(),
            certfile: String::new(),
            keyfile: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthSettings {
    #[serde(default = "default_auth_enabled")]
    pub enabled: bool,
    #[serde(default = "default_session_timeout")]
    pub session_timeout: u32,
}

fn default_auth_enabled() -> bool {
    true
}
fn default_session_timeout() -> u32 {
    2_592_000 // 30 days
}

impl Default for AuthSettings {
    fn default() -> Self {
        Self {
            enabled: default_auth_enabled(),
            session_timeout: default_session_timeout(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServerSettings {
    #[serde(default)]
    pub tls: TlsSettings,
    #[serde(default)]
    pub auth: AuthSettings,
    #[serde(default)]
    pub allowed_origins: Vec<String>,
}

// ---------------------------------------------------------------------------
// Top-level struct
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DistroSettings {
    #[serde(default = "default_workspace_root")]
    pub workspace_root: String,
    #[serde(default)]
    pub identity: IdentitySettings,
    #[serde(default)]
    pub backup: BackupSettings,
    #[serde(default)]
    pub slack: SlackSettings,
    #[serde(default)]
    pub voice: VoiceSettings,
    #[serde(default)]
    pub watchdog: WatchdogSettings,
    #[serde(default)]
    pub server: ServerSettings,
}

fn default_workspace_root() -> String {
    "~".to_string()
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Load settings from disk, returning defaults on missing or corrupt file.
pub fn load() -> DistroSettings {
    let path = crate::conventions::distro_settings_path();
    if !path.exists() {
        return DistroSettings::default();
    }
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_yaml_ng::from_str(&content).unwrap_or_default(),
        Err(_) => DistroSettings::default(),
    }
}

/// Save settings atomically (temp file + rename).
///
/// Returns the path that was written.
pub fn save(settings: &DistroSettings) -> crate::Result<std::path::PathBuf> {
    let path = crate::conventions::distro_settings_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = serde_yaml_ng::to_string(settings)
        .map_err(|e| DistroError::Config(format!("serialize settings: {e}")))?;

    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
    tmp.write_all(content.as_bytes())?;
    tmp.flush()?;
    tmp.persist(&path)
        .map_err(|e| DistroError::Config(format!("persist settings: {e}")))?;
    Ok(path)
}

/// Load, apply a mutator, then save — all under the process-level lock.
pub fn update<F>(mutator: F) -> crate::Result<DistroSettings>
where
    F: FnOnce(&mut DistroSettings),
{
    let _lock = SETTINGS_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let mut settings = load();
    mutator(&mut settings);
    save(&settings)?;
    Ok(settings)
}

/// Export settings to environment variables (skip if already set).
///
/// Returns the list of variable names that were newly set.
pub fn export_to_env(settings: &DistroSettings) -> Vec<String> {
    let mut exported = Vec::new();

    macro_rules! set_if_empty {
        ($key:expr, $val:expr) => {
            let val: &str = $val;
            if !val.is_empty() && std::env::var($key).is_err() {
                // SAFETY: single-threaded startup context; callers must ensure no
                // concurrent threads are reading env vars at this point.
                #[allow(unused_unsafe)]
                unsafe {
                    std::env::set_var($key, val);
                }
                exported.push($key.to_string());
            }
        };
    }

    set_if_empty!("AMPLIFIER_WORKSPACE_ROOT", &settings.workspace_root);
    set_if_empty!("AMPLIFIER_VOICE_VOICE", &settings.voice.voice);
    set_if_empty!("AMPLIFIER_VOICE_MODEL", &settings.voice.model);
    set_if_empty!("AMPLIFIER_VOICE_INSTRUCTIONS", &settings.voice.instructions);
    set_if_empty!("AMPLIFIER_VOICE_ASSISTANT_NAME", &settings.voice.assistant_name);
    set_if_empty!("AMPLIFIER_IDENTITY_GITHUB_HANDLE", &settings.identity.github_handle);
    set_if_empty!("AMPLIFIER_IDENTITY_GIT_EMAIL", &settings.identity.git_email);
    set_if_empty!("AMPLIFIER_SLACK_BOT_NAME", &settings.slack.bot_name);
    set_if_empty!("AMPLIFIER_SLACK_DEFAULT_BUNDLE", &settings.slack.default_bundle);
    set_if_empty!("AMPLIFIER_BACKUP_REPO_NAME", &settings.backup.repo_name);
    set_if_empty!("AMPLIFIER_BACKUP_REPO_OWNER", &settings.backup.repo_owner);

    exported
}
