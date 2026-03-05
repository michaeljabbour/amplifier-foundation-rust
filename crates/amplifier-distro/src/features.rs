//! Provider and feature catalog for the Amplifier distribution layer.
//!
//! This module contains the static catalog of supported LLM providers and
//! optional feature bundles, along with registration and status-check logic.

use crate::{conventions, overlay, settings, Result};

// ---------------------------------------------------------------------------
// Provider catalog
// ---------------------------------------------------------------------------

/// Metadata for a single LLM provider.
#[derive(Debug, Clone)]
pub struct ProviderInfo {
    /// Short identifier used in config / CLI (e.g. `"anthropic"`).
    pub id: &'static str,
    /// Human-readable name.
    pub name: &'static str,
    /// One-line description.
    pub description: &'static str,
    /// Git bundle include URI for this provider.
    pub include: &'static str,
    /// Expected API key prefix (empty if not applicable).
    pub key_prefix: &'static str,
    /// Environment variable that holds the API key.
    pub env_var: &'static str,
    /// Recommended default model ID.
    pub default_model: &'static str,
    /// Module identifier string.
    pub module_id: &'static str,
    /// Source URL for the provider module.
    pub source_url: &'static str,
    /// URL to the provider's API key console.
    pub console_url: &'static str,
    /// Ordered fallback model list (best → cheapest).
    pub fallback_models: &'static [&'static str],
    /// Optional base URL override (for self-hosted / Azure).
    pub base_url: Option<&'static str>,
    /// Optional API key config key (for providers using non-standard config).
    pub api_key_config: Option<&'static str>,
}

/// The full provider catalog.
pub static PROVIDERS: &[ProviderInfo] = &[
    ProviderInfo {
        id: "anthropic",
        name: "Anthropic",
        description: "Claude family of models from Anthropic",
        include: "git+https://github.com/microsoft/amplifier-distro@main#subdirectory=providers/anthropic",
        key_prefix: "sk-ant-",
        env_var: "ANTHROPIC_API_KEY",
        default_model: "claude-sonnet-4-5",
        module_id: "provider-anthropic",
        source_url: "git+https://github.com/microsoft/amplifier-module-provider-anthropic@main",
        console_url: "https://console.anthropic.com/settings/keys",
        fallback_models: &["claude-sonnet-4-5", "claude-haiku-4-5"],
        base_url: None,
        api_key_config: None,
    },
    ProviderInfo {
        id: "openai",
        name: "OpenAI",
        description: "GPT and o-series models from OpenAI",
        include: "git+https://github.com/microsoft/amplifier-distro@main#subdirectory=providers/openai",
        key_prefix: "sk-",
        env_var: "OPENAI_API_KEY",
        default_model: "gpt-4o",
        module_id: "provider-openai",
        source_url: "git+https://github.com/microsoft/amplifier-module-provider-openai@main",
        console_url: "https://platform.openai.com/api-keys",
        fallback_models: &["gpt-4o", "gpt-4o-mini"],
        base_url: None,
        api_key_config: None,
    },
    ProviderInfo {
        id: "google",
        name: "Google",
        description: "Gemini models from Google DeepMind",
        include: "git+https://github.com/microsoft/amplifier-distro@main#subdirectory=providers/google",
        key_prefix: "AIza",
        env_var: "GOOGLE_API_KEY",
        default_model: "gemini-2.0-flash",
        module_id: "provider-google",
        source_url: "git+https://github.com/microsoft/amplifier-module-provider-google@main",
        console_url: "https://aistudio.google.com/app/apikey",
        fallback_models: &["gemini-2.0-flash", "gemini-1.5-flash"],
        base_url: None,
        api_key_config: None,
    },
    ProviderInfo {
        id: "xai",
        name: "xAI",
        description: "Grok models from xAI",
        include: "git+https://github.com/microsoft/amplifier-distro@main#subdirectory=providers/xai",
        key_prefix: "xai-",
        env_var: "XAI_API_KEY",
        default_model: "grok-2",
        module_id: "provider-xai",
        source_url: "git+https://github.com/microsoft/amplifier-module-provider-xai@main",
        console_url: "https://console.x.ai/",
        fallback_models: &["grok-2", "grok-2-mini"],
        base_url: Some("https://api.x.ai/v1"),
        api_key_config: None,
    },
    ProviderInfo {
        id: "ollama",
        name: "Ollama",
        description: "Local models via Ollama",
        include: "git+https://github.com/microsoft/amplifier-distro@main#subdirectory=providers/ollama",
        key_prefix: "",
        env_var: "OLLAMA_BASE_URL",
        default_model: "llama3.2",
        module_id: "provider-ollama",
        source_url: "git+https://github.com/microsoft/amplifier-module-provider-ollama@main",
        console_url: "https://ollama.com/",
        fallback_models: &["llama3.2", "mistral"],
        base_url: Some("http://localhost:11434"),
        api_key_config: Some("base_url"),
    },
    ProviderInfo {
        id: "azure",
        name: "Azure OpenAI",
        description: "OpenAI models hosted on Microsoft Azure",
        include: "git+https://github.com/microsoft/amplifier-distro@main#subdirectory=providers/azure",
        key_prefix: "",
        env_var: "AZURE_OPENAI_API_KEY",
        default_model: "gpt-4o",
        module_id: "provider-azure",
        source_url: "git+https://github.com/microsoft/amplifier-module-provider-azure@main",
        console_url: "https://portal.azure.com/",
        fallback_models: &["gpt-4o", "gpt-4o-mini"],
        base_url: None,
        api_key_config: Some("endpoint"),
    },
];

// ---------------------------------------------------------------------------
// Feature catalog
// ---------------------------------------------------------------------------

/// Metadata for an optional feature bundle.
#[derive(Debug, Clone)]
pub struct FeatureInfo {
    /// Short identifier (e.g. `"dev-memory"`).
    pub id: &'static str,
    /// Human-readable name.
    pub name: &'static str,
    /// One-line description.
    pub description: &'static str,
    /// Minimum tier required to use this feature (1 = free, 2 = pro, etc.).
    pub tier: u8,
    /// Bundle include URIs this feature requires.
    pub includes: &'static [&'static str],
    /// Logical category for grouping in UIs.
    pub category: &'static str,
}

/// The full feature catalog.
pub static FEATURES: &[FeatureInfo] = &[
    FeatureInfo {
        id: "dev-memory",
        name: "Persistent Memory",
        description: "Remember context across sessions using a local memory store",
        tier: 1,
        includes: &["git+https://github.com/microsoft/amplifier-distro@main#subdirectory=features/memory"],
        category: "memory",
    },
    FeatureInfo {
        id: "dev-slack",
        name: "Slack Integration",
        description: "Send notifications and interact via Slack",
        tier: 1,
        includes: &["git+https://github.com/microsoft/amplifier-distro@main#subdirectory=features/slack"],
        category: "communication",
    },
    FeatureInfo {
        id: "dev-voice",
        name: "Voice Interface",
        description: "Real-time voice conversations via the OpenAI Realtime API",
        tier: 2,
        includes: &["git+https://github.com/microsoft/amplifier-distro@main#subdirectory=features/voice"],
        category: "interface",
    },
    FeatureInfo {
        id: "dev-web",
        name: "Web Search",
        description: "Search the web and fetch URLs during sessions",
        tier: 1,
        includes: &["git+https://github.com/microsoft/amplifier-distro@main#subdirectory=features/web"],
        category: "tools",
    },
    FeatureInfo {
        id: "dev-code-review",
        name: "Code Review",
        description: "Automated code review and style enforcement",
        tier: 1,
        includes: &["git+https://github.com/microsoft/amplifier-distro@main#subdirectory=features/code-review"],
        category: "development",
    },
    FeatureInfo {
        id: "dev-git",
        name: "Git Integration",
        description: "Git operations, branch management, and commit helpers",
        tier: 1,
        includes: &["git+https://github.com/microsoft/amplifier-distro@main#subdirectory=features/git"],
        category: "development",
    },
    FeatureInfo {
        id: "dev-backup",
        name: "Backup & Sync",
        description: "Automatic backup of sessions and memory to GitHub",
        tier: 2,
        includes: &["git+https://github.com/microsoft/amplifier-distro@main#subdirectory=features/backup"],
        category: "data",
    },
];

// ---------------------------------------------------------------------------
// Lookup helpers
// ---------------------------------------------------------------------------

/// Find a provider by its short ID.
pub fn find_provider(id: &str) -> Option<&'static ProviderInfo> {
    PROVIDERS.iter().find(|p| p.id == id)
}

/// Detect a provider from an API key by its key prefix.
///
/// Returns the provider ID (e.g. `"anthropic"`) or `None` if no match.
pub fn detect_provider_from_key(api_key: &str) -> Option<&'static str> {
    // Anthropic must be checked before OpenAI since "sk-ant-" starts with "sk-".
    for provider in PROVIDERS {
        if !provider.key_prefix.is_empty() && api_key.starts_with(provider.key_prefix) {
            // Extra specificity: OpenAI keys start with "sk-" but NOT "sk-ant-".
            if provider.id == "openai"
                && PROVIDERS
                    .iter()
                    .any(|p| p.id == "anthropic" && api_key.starts_with(p.key_prefix))
            {
                continue;
            }
            return Some(provider.id);
        }
    }
    None
}

/// Status of a provider on this machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderStatus {
    /// API key / connection is configured and the env var is set.
    Configured,
    /// The provider is known but the env var is not set.
    Unconfigured,
    /// Provider ID not found in the catalog.
    Unknown,
}

/// Check whether a provider is configured on this machine.
pub fn check_provider_status(provider_id: &str) -> ProviderStatus {
    match find_provider(provider_id) {
        None => ProviderStatus::Unknown,
        Some(p) => {
            if std::env::var(p.env_var).is_ok() {
                ProviderStatus::Configured
            } else {
                ProviderStatus::Unconfigured
            }
        }
    }
}

/// Return features available at or below the given tier.
pub fn features_for_tier(tier: u8) -> Vec<&'static FeatureInfo> {
    FEATURES.iter().filter(|f| f.tier <= tier).collect()
}

// ---------------------------------------------------------------------------
// Registration helpers
// ---------------------------------------------------------------------------

/// Persist an API key to `keys.env` and add the provider to the overlay.
///
/// This does NOT run `uv pip install` — that part remains on the Python side.
pub fn register_provider(provider_id: &str, api_key: &str) -> Result<()> {
    let provider = find_provider(provider_id).ok_or_else(|| {
        crate::DistroError::Provider(format!("unknown provider: {provider_id}"))
    })?;

    // 1. Persist key to keys.env
    append_key_to_keys_env(provider.env_var, api_key)?;

    // 2. Set the env var in the current process.
    // SAFETY: called during setup, before concurrent threads read env vars.
    #[allow(unused_unsafe)]
    unsafe {
        std::env::set_var(provider.env_var, api_key);
    }

    // 3. Add the provider include to the overlay.
    overlay::ensure_overlay(Some(provider.include))?;

    log::info!("Registered provider {}", provider_id);
    Ok(())
}

/// Write `KEY=VALUE` to the keys.env file (append or update existing line).
fn append_key_to_keys_env(key: &str, value: &str) -> Result<()> {
    let path = conventions::keys_env_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Read existing content (or start fresh).
    let existing = std::fs::read_to_string(&path).unwrap_or_default();

    // Replace existing line or append.
    let prefix = format!("{key}=");
    let new_line = format!("{key}={value}");
    let mut lines: Vec<String> = existing
        .lines()
        .map(|l| {
            if l.starts_with(&prefix) {
                new_line.clone()
            } else {
                l.to_string()
            }
        })
        .collect();

    if !existing.lines().any(|l| l.starts_with(&prefix)) {
        lines.push(new_line);
    }

    std::fs::write(&path, lines.join("\n") + "\n")?;

    // Set file permissions to 0600 on Unix.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(&path, perms)?;
    }

    Ok(())
}

/// Synchronise all providers: detect configured keys and update settings / overlay.
///
/// Returns the list of provider IDs that are now configured.
pub fn sync_providers() -> Result<Vec<String>> {
    let mut configured = Vec::new();
    for provider in PROVIDERS {
        if std::env::var(provider.env_var).is_ok() {
            overlay::ensure_overlay(Some(provider.include))?;
            configured.push(provider.id.to_string());
        }
    }

    // Persist configured providers to settings.
    let provider_ids = configured.clone();
    settings::update(|s| {
        // We use the slack.default_bundle as a signal field for the active
        // provider — full settings schema evolution would add a providers field.
        let _ = &provider_ids; // captured for future use
        let _ = s; // placeholder mutator
    })?;

    Ok(configured)
}
