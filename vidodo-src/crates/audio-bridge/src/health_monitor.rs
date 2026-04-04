//! scsynth health monitoring.
//!
//! Polls scsynth `/status` at a fixed interval and detects
//! unresponsive states. If no `/status.reply` arrives within
//! the configured timeout, the backend is considered degraded.

use crate::osc::{OscMessage, ScynthCmd};

/// Health state of the scsynth backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScynthHealth {
    /// Server is responding normally.
    Healthy,
    /// No status reply within the timeout window.
    Degraded,
    /// Server confirmed offline / unreachable.
    Offline,
}

/// Monitors scsynth liveness via `/status` polling.
pub struct HealthMonitor {
    /// Timeout for status reply (ms).
    timeout_ms: f64,
    /// Timestamp of the last `/status` send (ms).
    last_poll_ms: f64,
    /// Timestamp of the most recent `/status.reply` (ms).
    last_reply_ms: f64,
    /// Current health assessment.
    health: ScynthHealth,
    /// Whether a poll is in-flight (waiting for reply).
    poll_in_flight: bool,
}

impl HealthMonitor {
    /// Create a new monitor with the given timeout (in milliseconds).
    pub fn new(timeout_ms: f64) -> Self {
        Self {
            timeout_ms,
            last_poll_ms: 0.0,
            last_reply_ms: 0.0,
            health: ScynthHealth::Healthy,
            poll_in_flight: false,
        }
    }

    /// Generate a `/status` poll message if it's time.
    ///
    /// Returns `Some(OscMessage)` when a new poll should be sent.
    /// Call this at your desired polling interval.
    pub fn poll(&mut self, now_ms: f64) -> Option<OscMessage> {
        // Don't poll if one is already in-flight and hasn't timed out yet
        if self.poll_in_flight && (now_ms - self.last_poll_ms) < self.timeout_ms {
            return None;
        }

        // Check if the in-flight poll timed out
        if self.poll_in_flight && (now_ms - self.last_poll_ms) >= self.timeout_ms {
            self.health = ScynthHealth::Degraded;
        }

        self.last_poll_ms = now_ms;
        self.poll_in_flight = true;
        Some(ScynthCmd::status())
    }

    /// Process an incoming `/status.reply` message.
    ///
    /// Returns `true` if the message was recognized as a status reply.
    pub fn process_reply(&mut self, msg: &OscMessage, now_ms: f64) -> bool {
        if msg.address != "/status.reply" {
            return false;
        }
        self.last_reply_ms = now_ms;
        self.poll_in_flight = false;
        self.health = ScynthHealth::Healthy;
        true
    }

    /// Evaluate the current health state based on time.
    ///
    /// Should be called periodically (e.g., each tick).
    pub fn evaluate(&mut self, now_ms: f64) -> ScynthHealth {
        if self.poll_in_flight && (now_ms - self.last_poll_ms) >= self.timeout_ms {
            self.health = ScynthHealth::Degraded;
        }
        self.health
    }

    /// Get the current health state.
    pub fn health(&self) -> ScynthHealth {
        self.health
    }

    /// Mark the server as offline (e.g. process crashed).
    pub fn mark_offline(&mut self) {
        self.health = ScynthHealth::Offline;
        self.poll_in_flight = false;
    }

    /// Time since the last successful status reply (ms).
    pub fn time_since_reply(&self, now_ms: f64) -> f64 {
        now_ms - self.last_reply_ms
    }
}

#[cfg(test)]
mod health_tests {
    use super::*;
    use crate::osc::{OscArg, OscMessage};

    fn status_reply() -> OscMessage {
        OscMessage::new(
            "/status.reply",
            vec![
                OscArg::Int(1),     // unused
                OscArg::Int(0),     // num UGens
                OscArg::Int(0),     // num synths
                OscArg::Int(0),     // num groups
                OscArg::Int(0),     // num SynthDefs
                OscArg::Float(0.0), // avg CPU
                OscArg::Float(0.0), // peak CPU
            ],
        )
    }

    #[test]
    fn initial_state_is_healthy() {
        let monitor = HealthMonitor::new(5000.0);
        assert_eq!(monitor.health(), ScynthHealth::Healthy);
    }

    #[test]
    fn poll_generates_status_message() {
        let mut monitor = HealthMonitor::new(5000.0);
        let msg = monitor.poll(0.0);
        assert!(msg.is_some());
        assert_eq!(msg.unwrap().address, "/status");
    }

    #[test]
    fn reply_within_timeout_stays_healthy() {
        let mut monitor = HealthMonitor::new(5000.0);
        monitor.poll(0.0);
        monitor.process_reply(&status_reply(), 1000.0);
        assert_eq!(monitor.evaluate(1000.0), ScynthHealth::Healthy);
    }

    #[test]
    fn no_reply_within_timeout_degrades() {
        let mut monitor = HealthMonitor::new(5000.0);
        monitor.poll(0.0);
        // 5000ms pass with no reply
        assert_eq!(monitor.evaluate(5000.0), ScynthHealth::Degraded);
    }

    #[test]
    fn recover_from_degraded_on_reply() {
        let mut monitor = HealthMonitor::new(5000.0);
        monitor.poll(0.0);
        assert_eq!(monitor.evaluate(5000.0), ScynthHealth::Degraded);

        // Now the reply finally arrives
        monitor.process_reply(&status_reply(), 5500.0);
        assert_eq!(monitor.health(), ScynthHealth::Healthy);
    }

    #[test]
    fn mark_offline() {
        let mut monitor = HealthMonitor::new(5000.0);
        monitor.mark_offline();
        assert_eq!(monitor.health(), ScynthHealth::Offline);
    }
}
