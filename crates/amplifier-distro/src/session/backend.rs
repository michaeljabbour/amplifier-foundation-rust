//! Session backend trait and Foundation-backed implementation.
//!
//! The [`SessionBackend`] trait defines the session lifecycle contract.
//! [`FoundationBackend`] is the primary implementation — it stores sessions
//! in memory and persists transcript/metadata alongside each session directory.
//!
//! # Module instantiation boundary
//!
//! The full [`FoundationBackend::create_session`] flow would:
//! 1. Load the bundle via `amplifier_foundation::load_bundle`
//! 2. Create a runtime session via `amplifier_foundation::AmplifierRuntime`
//! 3. Wire transcript/metadata persistence hooks
//! 4. Return the [`SessionInfo`]
//!
//! Steps 2–3 require a concrete `AmplifierRuntime` implementation that lives
//! in the Python sidecar layer.  Those steps are clearly marked **STUB** and
//! will be completed once the runtime bridge is available.

use std::collections::HashMap;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{conventions, DistroError, Result};

// ---------------------------------------------------------------------------
// SessionInfo
// ---------------------------------------------------------------------------

/// Lightweight record describing an active or past session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    /// Unique session identifier (UUIDv4).
    pub session_id: String,
    /// Project slug this session belongs to.
    pub project_id: String,
    /// Working directory the session was started in.
    pub working_dir: PathBuf,
    /// Whether the session is currently active.
    pub is_active: bool,
    /// Name of the application (or CLI invocation) that created the session.
    pub created_by_app: String,
    /// Human-readable description or initial prompt.
    pub description: String,
    /// UTC timestamp when the session was created.
    pub created_at: DateTime<Utc>,
}

impl SessionInfo {
    /// Create a new [`SessionInfo`] with a fresh UUIDv4 session ID.
    pub fn new(
        project_id: &str,
        working_dir: PathBuf,
        created_by_app: &str,
        description: &str,
    ) -> Self {
        Self {
            session_id: Uuid::new_v4().to_string(),
            project_id: project_id.to_string(),
            working_dir,
            is_active: true,
            created_by_app: created_by_app.to_string(),
            description: description.to_string(),
            created_at: Utc::now(),
        }
    }

    /// The path to this session's directory on disk.
    pub fn session_dir(&self) -> PathBuf {
        conventions::session_dir(&self.project_id, &self.session_id)
    }

    /// Persist this `SessionInfo` as `session-info.json` inside the session directory.
    pub fn persist(&self) -> Result<()> {
        let dir = self.session_dir();
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(conventions::SESSION_INFO_FILENAME);
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Load a `SessionInfo` from its `session-info.json` file.
    pub fn load(project_id: &str, session_id: &str) -> Result<Self> {
        let dir = conventions::session_dir(project_id, session_id);
        let path = dir.join(conventions::SESSION_INFO_FILENAME);
        let content = std::fs::read_to_string(&path)?;
        let info: Self = serde_json::from_str(&content)?;
        Ok(info)
    }
}

// ---------------------------------------------------------------------------
// SessionBackend trait
// ---------------------------------------------------------------------------

/// Contract for session lifecycle management.
pub trait SessionBackend: Send + Sync {
    /// Create a new session for `project_id` in `working_dir`.
    fn create_session(
        &mut self,
        project_id: &str,
        working_dir: PathBuf,
        created_by_app: &str,
        description: &str,
    ) -> Result<SessionInfo>;

    /// Retrieve a session by ID.
    fn get_session(&self, session_id: &str) -> Option<&SessionInfo>;

    /// List sessions, optionally filtered by project.
    fn list_sessions(&self, project_id: Option<&str>) -> Vec<&SessionInfo>;

    /// Mark a session as inactive (does not delete data).
    fn mark_inactive(&mut self, session_id: &str) -> Result<()>;

    /// Enumerate session IDs from the on-disk sessions directory.
    fn list_session_ids(&self, project_id: &str) -> Vec<String>;
}

// ---------------------------------------------------------------------------
// FoundationBackend
// ---------------------------------------------------------------------------

/// Foundation-backed session backend.
///
/// Maintains an in-memory map of [`SessionInfo`] records that is hydrated from
/// the on-disk sessions directories on first access.
///
/// # STUB
///
/// The session *execution* path (load bundle → create runtime session → wire
/// hooks) is stubbed pending availability of the runtime bridge.  All
/// file-system bookkeeping (create directory, persist session-info.json) is
/// fully implemented.
pub struct FoundationBackend {
    sessions: HashMap<String, SessionInfo>,
}

impl FoundationBackend {
    /// Create a new empty backend.
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    /// Hydrate the in-memory map from the on-disk sessions directories.
    ///
    /// Walk `$AMPLIFIER_HOME/projects/*/sessions/*/session-info.json` and
    /// load each [`SessionInfo`].  Missing or corrupt files are silently
    /// skipped.
    pub fn load_from_disk(&mut self) {
        let projects = conventions::projects_dir();
        if !projects.exists() {
            return;
        }
        let Ok(project_entries) = std::fs::read_dir(&projects) else {
            return;
        };
        for project_entry in project_entries.flatten() {
            let sessions_dir = project_entry.path().join("sessions");
            if !sessions_dir.is_dir() {
                continue;
            }
            let Ok(session_entries) = std::fs::read_dir(&sessions_dir) else {
                continue;
            };
            for session_entry in session_entries.flatten() {
                let info_path = session_entry.path().join(conventions::SESSION_INFO_FILENAME);
                if let Ok(content) = std::fs::read_to_string(&info_path) {
                    if let Ok(info) = serde_json::from_str::<SessionInfo>(&content) {
                        self.sessions.insert(info.session_id.clone(), info);
                    }
                }
            }
        }
    }
}

impl Default for FoundationBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionBackend for FoundationBackend {
    fn create_session(
        &mut self,
        project_id: &str,
        working_dir: PathBuf,
        created_by_app: &str,
        description: &str,
    ) -> Result<SessionInfo> {
        let info = SessionInfo::new(project_id, working_dir, created_by_app, description);

        // Create the session directory and persist session-info.json.
        info.persist()?;

        // STUB: Full flow would continue here:
        // 1. load_bundle(overlay_path) via amplifier_foundation::load_bundle
        // 2. runtime.create_session(SessionOptions { mount_plan, session_id, … })
        //    — requires a concrete AmplifierRuntime impl from the Python sidecar
        // 3. Register transcript hook:
        //    coordinator.hooks_mut().register("message", transcript_hook, 10, "transcript")
        // 4. Register metadata hook:
        //    coordinator.hooks_mut().register("session_end", metadata_hook, 10, "metadata")
        //
        // Until the runtime bridge is wired, session execution is handled by
        // the Python layer that reads session-info.json from the directory we
        // just created.

        log::debug!("Created session {} for project {}", info.session_id, project_id);
        self.sessions.insert(info.session_id.clone(), info.clone());
        Ok(info)
    }

    fn get_session(&self, session_id: &str) -> Option<&SessionInfo> {
        self.sessions.get(session_id)
    }

    fn list_sessions(&self, project_id: Option<&str>) -> Vec<&SessionInfo> {
        self.sessions
            .values()
            .filter(|s| project_id.map_or(true, |p| s.project_id == p))
            .collect()
    }

    fn mark_inactive(&mut self, session_id: &str) -> Result<()> {
        match self.sessions.get_mut(session_id) {
            Some(s) => {
                s.is_active = false;
                s.persist().map_err(|e| {
                    DistroError::Session(format!("persist after mark_inactive: {e}"))
                })?;
                Ok(())
            }
            None => Err(DistroError::Session(format!(
                "session not found: {session_id}"
            ))),
        }
    }

    fn list_session_ids(&self, project_id: &str) -> Vec<String> {
        let sessions_dir = conventions::projects_dir()
            .join(project_id)
            .join("sessions");
        if !sessions_dir.is_dir() {
            return vec![];
        }
        std::fs::read_dir(&sessions_dir)
            .map(|entries| {
                entries
                    .flatten()
                    .filter(|e| e.path().is_dir())
                    .filter_map(|e| e.file_name().into_string().ok())
                    .collect()
            })
            .unwrap_or_default()
    }
}
