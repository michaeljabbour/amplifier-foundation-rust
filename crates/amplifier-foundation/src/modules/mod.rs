//! Module infrastructure for amplifier-foundation.
//!
//! This module provides:
//! - [`state::InstallStateManager`]: Fingerprint-based module install tracking
//! - [`activator::ModuleActivator`]: Async module activation via URI resolution
//!   and optional dependency installation

pub mod activator;
pub mod state;
