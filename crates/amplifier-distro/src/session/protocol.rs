//! Async approval and display primitives for session protocols.
//!
//! `ApprovalSystem` wraps a tokio mpsc channel so that approval requests can
//! be sent from within an async session and handled by an outer event loop.
//!
//! `QueueDisplaySystem` is a stub — streaming output routing is handled by the
//! Python sidecar layer.

use tokio::sync::{mpsc, oneshot};

use crate::DistroError;

// ---------------------------------------------------------------------------
// ApprovalSystem
// ---------------------------------------------------------------------------

/// A pending approval request.
#[derive(Debug)]
pub struct ApprovalRequest {
    /// Human-readable prompt shown to the user.
    pub prompt: String,
    /// Available choices (empty = yes/no).
    pub options: Vec<String>,
    /// Optional context block (e.g. diff or command text).
    pub context: Option<String>,
}

/// The resolved outcome of an approval request.
#[derive(Debug)]
pub struct ApprovalResponse {
    /// Whether the request was approved.
    pub approved: bool,
    /// The choice the user selected, if `options` were provided.
    pub choice: Option<String>,
}

type ApprovalMessage = (ApprovalRequest, oneshot::Sender<ApprovalResponse>);

/// Sends approval requests over a channel and awaits responses.
///
/// Create via [`ApprovalSystem::new`] which returns both the system and the
/// receiver end of the channel.
pub struct ApprovalSystem {
    sender: Option<mpsc::Sender<ApprovalMessage>>,
}

impl ApprovalSystem {
    /// Create a new `ApprovalSystem` together with its request receiver.
    ///
    /// # Example
    /// ```no_run
    /// # use amplifier_distro::session::protocol::ApprovalSystem;
    /// let (approval, mut rx) = ApprovalSystem::new();
    /// // Spawn a handler that reads from `rx` and replies via the oneshot.
    /// ```
    pub fn new() -> (Self, mpsc::Receiver<ApprovalMessage>) {
        let (tx, rx) = mpsc::channel(32);
        (Self { sender: Some(tx) }, rx)
    }

    /// Create a disconnected (no-op) approval system.
    ///
    /// All approval requests will immediately return an error.
    pub fn disconnected() -> Self {
        Self { sender: None }
    }

    /// Send an approval request and wait for a response.
    pub async fn request_approval(
        &self,
        request: ApprovalRequest,
    ) -> crate::Result<ApprovalResponse> {
        let sender = self
            .sender
            .as_ref()
            .ok_or_else(|| DistroError::Session("approval system is disconnected".to_string()))?;

        let (tx, rx) = oneshot::channel();
        sender
            .send((request, tx))
            .await
            .map_err(|_| DistroError::Session("approval channel is closed".to_string()))?;

        rx.await
            .map_err(|_| DistroError::Session("approval response channel dropped".to_string()))
    }

    /// Handle the next pending request (non-blocking, returns `None` if queue is empty).
    ///
    /// Convenience wrapper for callers that own the `Receiver`.
    pub async fn handle_response(
        rx: &mut mpsc::Receiver<ApprovalMessage>,
        approved: bool,
        choice: Option<String>,
    ) -> bool {
        if let Some((_, tx)) = rx.recv().await {
            let _ = tx.send(ApprovalResponse { approved, choice });
            true
        } else {
            false
        }
    }
}

impl Default for ApprovalSystem {
    fn default() -> Self {
        Self::disconnected()
    }
}

// ---------------------------------------------------------------------------
// QueueDisplaySystem
// ---------------------------------------------------------------------------

/// Stub display system for streaming output.
///
/// Full implementation routes tokens to the Python sidecar streaming layer.
pub struct QueueDisplaySystem;

impl QueueDisplaySystem {
    pub fn new() -> Self {
        Self
    }

    /// Write a text chunk to the display queue.
    ///
    /// Currently a no-op — streaming is handled by the Python sidecar.
    pub fn write(&self, _chunk: &str) {}

    /// Flush any buffered output.
    pub fn flush(&self) {}
}

impl Default for QueueDisplaySystem {
    fn default() -> Self {
        Self::new()
    }
}
