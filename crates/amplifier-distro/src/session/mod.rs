//! Session management module.
//!
//! Re-exports the public surface of each sub-module.

pub mod backend;
pub mod metadata;
pub mod protocol;
pub mod transcript;

pub use backend::{FoundationBackend, SessionBackend, SessionInfo};
pub use metadata::{read_metadata, write_metadata};
pub use protocol::{ApprovalRequest, ApprovalResponse, ApprovalSystem, QueueDisplaySystem};
pub use transcript::{read_transcript, write_transcript};
