//! Health checks and auto-fix for the Amplifier distro installation.
//!
//! Checks cover file-system layout, config completeness, and local tool
//! availability.  Checks that require external binaries (gh, tailscale,
//! systemd) are intentionally excluded from this Rust port.

use crate::{conventions, settings};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Outcome of a single health check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckStatus {
    Ok,
    Warning,
    Error,
}

impl std::fmt::Display for CheckStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CheckStatus::Ok => write!(f, "ok"),
            CheckStatus::Warning => write!(f, "warning"),
            CheckStatus::Error => write!(f, "error"),
        }
    }
}

/// A single diagnostic check result.
#[derive(Debug, Clone)]
pub struct DiagnosticCheck {
    pub name: String,
    pub status: CheckStatus,
    pub message: String,
    pub fix_available: bool,
    pub fix_description: Option<String>,
}

impl DiagnosticCheck {
    fn ok(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: CheckStatus::Ok,
            message: message.into(),
            fix_available: false,
            fix_description: None,
        }
    }

    fn warn(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: CheckStatus::Warning,
            message: message.into(),
            fix_available: false,
            fix_description: None,
        }
    }

    fn error(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: CheckStatus::Error,
            message: message.into(),
            fix_available: false,
            fix_description: None,
        }
    }

    fn with_fix(mut self, description: impl Into<String>) -> Self {
        self.fix_available = true;
        self.fix_description = Some(description.into());
        self
    }
}

/// The complete doctor report.
#[derive(Debug)]
pub struct DoctorReport {
    pub checks: Vec<DiagnosticCheck>,
}

impl DoctorReport {
    /// True if all checks passed (no warnings or errors).
    pub fn is_healthy(&self) -> bool {
        self.checks
            .iter()
            .all(|c| c.status == CheckStatus::Ok)
    }

    /// Number of checks with `Error` status.
    pub fn error_count(&self) -> usize {
        self.checks
            .iter()
            .filter(|c| c.status == CheckStatus::Error)
            .count()
    }

    /// Number of checks with `Warning` status.
    pub fn warning_count(&self) -> usize {
        self.checks
            .iter()
            .filter(|c| c.status == CheckStatus::Warning)
            .count()
    }
}

// ---------------------------------------------------------------------------
// Individual checks
// ---------------------------------------------------------------------------

fn check_config_exists() -> DiagnosticCheck {
    let path = conventions::distro_settings_path();
    if path.exists() {
        DiagnosticCheck::ok("config_exists", format!("settings.yaml found at {}", path.display()))
    } else {
        DiagnosticCheck::warn(
            "config_exists",
            format!("settings.yaml not found at {}", path.display()),
        )
    }
}

fn check_identity_set() -> DiagnosticCheck {
    let cfg = settings::load();
    if !cfg.identity.github_handle.is_empty() {
        DiagnosticCheck::ok("identity_set", "github_handle is configured")
    } else {
        DiagnosticCheck::warn(
            "identity_set",
            "identity.github_handle is not set in settings.yaml",
        )
    }
}

fn check_workspace_exists() -> DiagnosticCheck {
    let cfg = settings::load();
    if cfg.workspace_root.is_empty() || cfg.workspace_root == "~" {
        return DiagnosticCheck::warn(
            "workspace_exists",
            "workspace_root is not explicitly configured (using default ~)",
        );
    }
    let expanded = shellexpand(&cfg.workspace_root);
    let path = std::path::Path::new(&expanded);
    if path.exists() {
        DiagnosticCheck::ok("workspace_exists", format!("workspace_root exists: {}", expanded))
    } else {
        DiagnosticCheck::error(
            "workspace_exists",
            format!("workspace_root does not exist: {}", expanded),
        )
    }
}

fn check_memory_dir_exists() -> DiagnosticCheck {
    let path = conventions::memory_dir();
    if path.exists() {
        DiagnosticCheck::ok("memory_dir_exists", format!("{}", path.display()))
    } else {
        DiagnosticCheck::warn(
            "memory_dir_exists",
            format!("memory dir missing: {}", path.display()),
        )
        .with_fix(format!("mkdir -p {}", path.display()))
    }
}

fn check_keys_permissions() -> DiagnosticCheck {
    let path = conventions::keys_env_path();
    if !path.exists() {
        return DiagnosticCheck::ok("keys_permissions", "keys.env does not exist (not required)");
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        match std::fs::metadata(&path) {
            Ok(meta) => {
                let mode = meta.mode() & 0o777;
                if mode == 0o600 {
                    DiagnosticCheck::ok("keys_permissions", "keys.env permissions are 600")
                } else {
                    DiagnosticCheck::warn(
                        "keys_permissions",
                        format!("keys.env permissions are {:o}, expected 600", mode),
                    )
                    .with_fix(format!("chmod 600 {}", path.display()))
                }
            }
            Err(e) => DiagnosticCheck::error("keys_permissions", format!("cannot stat keys.env: {e}")),
        }
    }

    #[cfg(not(unix))]
    DiagnosticCheck::ok("keys_permissions", "keys.env permission check skipped on non-Unix")
}

fn check_bundle_cache_exists() -> DiagnosticCheck {
    let path = conventions::cache_dir();
    if path.exists() {
        DiagnosticCheck::ok("bundle_cache_exists", format!("{}", path.display()))
    } else {
        DiagnosticCheck::warn(
            "bundle_cache_exists",
            format!("cache dir missing: {}", path.display()),
        )
        .with_fix(format!("mkdir -p {}", path.display()))
    }
}

fn check_server_dir_exists() -> DiagnosticCheck {
    let path = conventions::server_dir();
    if path.exists() {
        DiagnosticCheck::ok("server_dir_exists", format!("{}", path.display()))
    } else {
        DiagnosticCheck::warn(
            "server_dir_exists",
            format!("server dir missing: {}", path.display()),
        )
        .with_fix(format!("mkdir -p {}", path.display()))
    }
}

fn check_git_configured() -> DiagnosticCheck {
    let name_ok = std::process::Command::new("git")
        .args(["config", "user.name"])
        .output()
        .map(|o| o.status.success() && !o.stdout.is_empty())
        .unwrap_or(false);

    let email_ok = std::process::Command::new("git")
        .args(["config", "user.email"])
        .output()
        .map(|o| o.status.success() && !o.stdout.is_empty())
        .unwrap_or(false);

    match (name_ok, email_ok) {
        (true, true) => DiagnosticCheck::ok("git_configured", "git user.name and user.email are set"),
        (false, _) => DiagnosticCheck::warn(
            "git_configured",
            "git user.name is not configured; run: git config --global user.name \"Your Name\"",
        ),
        (_, false) => DiagnosticCheck::warn(
            "git_configured",
            "git user.email is not configured; run: git config --global user.email you@example.com",
        ),
    }
}

// ---------------------------------------------------------------------------
// Expand ~ helper
// ---------------------------------------------------------------------------

fn shellexpand(path: &str) -> String {
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
// Public API
// ---------------------------------------------------------------------------

/// Run all diagnostic checks and return a report.
pub fn run_diagnostics() -> DoctorReport {
    let checks = vec![
        check_config_exists(),
        check_identity_set(),
        check_workspace_exists(),
        check_memory_dir_exists(),
        check_keys_permissions(),
        check_bundle_cache_exists(),
        check_server_dir_exists(),
        check_git_configured(),
    ];
    DoctorReport { checks }
}

/// Apply all available auto-fixes from the report.
///
/// Returns the names of checks that were successfully fixed.
pub fn run_fixes(report: &DoctorReport) -> Vec<String> {
    let mut fixed = Vec::new();

    for check in &report.checks {
        if !check.fix_available {
            continue;
        }

        let result = apply_fix(check);
        match result {
            Ok(()) => {
                log::info!("Fixed: {}", check.name);
                fixed.push(check.name.clone());
            }
            Err(e) => {
                log::warn!("Could not fix {}: {e}", check.name);
            }
        }
    }

    fixed
}

/// Apply the auto-fix for a single check.
fn apply_fix(check: &DiagnosticCheck) -> crate::Result<()> {
    match check.name.as_str() {
        "memory_dir_exists" => {
            std::fs::create_dir_all(conventions::memory_dir())?;
        }
        "bundle_cache_exists" => {
            std::fs::create_dir_all(conventions::cache_dir())?;
        }
        "server_dir_exists" => {
            std::fs::create_dir_all(conventions::server_dir())?;
        }
        "keys_permissions" => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let path = conventions::keys_env_path();
                if path.exists() {
                    let perms = std::fs::Permissions::from_mode(0o600);
                    std::fs::set_permissions(&path, perms)?;
                }
            }
        }
        _ => {
            log::debug!("No fix implementation for check: {}", check.name);
        }
    }
    Ok(())
}
