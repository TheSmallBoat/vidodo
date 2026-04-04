//! Core IPC message types.

use serde::{Deserialize, Serialize};

/// Unique correlation ID for request-response tracking.
pub type CorrelationId = String;

/// Envelope wrapping all IPC messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageEnvelope {
    pub correlation_id: CorrelationId,
    /// Unique identifier for this specific message hop.
    #[serde(default)]
    pub message_id: String,
    /// Parent message that caused this message (for causal chains).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    pub timestamp_ms: u64,
    pub payload: RuntimeMessage,
}

/// Messages sent from the controller to runtime processes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RuntimeMessage {
    /// Execute an action (audio/visual/lighting payload).
    Action { payload_json: String },
    /// Apply a live patch.
    Patch { patch_id: String, patch_json: String },
    /// Transport control (play, pause, seek).
    Transport { command: TransportCommand },
    /// Health check request.
    HealthCheck,
    /// Graceful termination.
    Terminate { reason: String },
}

/// Transport control commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransportCommand {
    Play,
    Pause,
    Stop,
    Seek { beat: u64 },
}

/// Acknowledgement message from runtime processes back to controller.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RuntimeAck {
    /// Execution completed.
    ExecutionAck {
        correlation_id: CorrelationId,
        success: bool,
        wall_ms: u64,
        detail: Option<String>,
    },
    /// Health status report.
    HealthStatus {
        correlation_id: CorrelationId,
        process_id: String,
        cpu_percent: f64,
        memory_mb: f64,
        status: ProcessHealth,
    },
    /// Patch applied acknowledgement.
    PatchAck {
        correlation_id: CorrelationId,
        patch_id: String,
        success: bool,
        rollback_available: bool,
    },
    /// Error response.
    Error { correlation_id: CorrelationId, code: String, message: String },
}

/// Health state of a runtime process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProcessHealth {
    Healthy,
    Degraded,
    Unhealthy,
    Unresponsive,
}
