//! Module infrastructure for amplifier-foundation.
//!
//! This module provides:
//! - [`state::InstallStateManager`]: Fingerprint-based module install tracking
//!
//! Future: `ModuleActivator` (async module activation via subprocess install)
//! will be added when the runtime layer supports it. The activator depends on
//! `SimpleSourceResolver` (already ported) and external tooling (uv/pip).

pub mod state;
