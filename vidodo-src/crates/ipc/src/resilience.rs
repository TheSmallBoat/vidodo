//! Runtime resilience: hang detection, panic recovery, and degrade triggering.
//!
//! `ResilienceMonitor` watches runtime thread heartbeats and detects two
//! failure modes:
//! - **Hang**: a runtime fails to heartbeat within `hang_threshold_ms` →
//!   marked `Degraded`, degrade event emitted.
//! - **Panic**: a runtime thread's `JoinHandle` reports a panic → marked
//!   `Panicked`, remaining runtimes continue unaffected.

use std::collections::HashMap;

/// Status of a monitored runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeHealth {
    Healthy,
    Degraded,
    Panicked,
}

/// A degrade event produced when a runtime is detected as unhealthy.
#[derive(Debug, Clone)]
pub struct DegradeNotice {
    pub runtime_name: String,
    pub reason: String,
    pub health: RuntimeHealth,
    pub detected_at_ms: f64,
}

/// Heartbeat record for a single runtime.
#[derive(Debug)]
struct RuntimeEntry {
    last_heartbeat_ms: f64,
    health: RuntimeHealth,
}

/// Monitors runtime thread health via heartbeat tracking.
///
/// Each runtime periodically calls `heartbeat(name, now_ms)`.
/// The scheduler calls `check(now_ms)` to detect hangs.
pub struct ResilienceMonitor {
    runtimes: HashMap<String, RuntimeEntry>,
    hang_threshold_ms: f64,
}

impl ResilienceMonitor {
    pub fn new(hang_threshold_ms: f64) -> Self {
        Self { runtimes: HashMap::new(), hang_threshold_ms }
    }

    /// Register a runtime for monitoring. Initial heartbeat is set to `now_ms`.
    pub fn register(&mut self, name: &str, now_ms: f64) {
        self.runtimes.insert(
            name.to_string(),
            RuntimeEntry { last_heartbeat_ms: now_ms, health: RuntimeHealth::Healthy },
        );
    }

    /// Record a heartbeat from a runtime.
    pub fn heartbeat(&mut self, name: &str, now_ms: f64) {
        if let Some(entry) = self.runtimes.get_mut(name) {
            entry.last_heartbeat_ms = now_ms;
            // A heartbeat can recover a degraded runtime
            if entry.health == RuntimeHealth::Degraded {
                entry.health = RuntimeHealth::Healthy;
            }
        }
    }

    /// Mark a runtime as panicked (called when JoinHandle detects a panic).
    pub fn mark_panicked(&mut self, name: &str, now_ms: f64) -> Option<DegradeNotice> {
        if let Some(entry) = self.runtimes.get_mut(name) {
            entry.health = RuntimeHealth::Panicked;
            Some(DegradeNotice {
                runtime_name: name.to_string(),
                reason: format!("runtime thread '{name}' panicked"),
                health: RuntimeHealth::Panicked,
                detected_at_ms: now_ms,
            })
        } else {
            None
        }
    }

    /// Check all runtimes for hang (no heartbeat within threshold).
    ///
    /// Returns degrade notices for newly-detected hangs. Does NOT re-emit
    /// for runtimes already in `Degraded` or `Panicked` state.
    pub fn check(&mut self, now_ms: f64) -> Vec<DegradeNotice> {
        let mut notices = Vec::new();
        for (name, entry) in &mut self.runtimes {
            if entry.health == RuntimeHealth::Healthy
                && (now_ms - entry.last_heartbeat_ms) >= self.hang_threshold_ms
            {
                entry.health = RuntimeHealth::Degraded;
                notices.push(DegradeNotice {
                    runtime_name: name.clone(),
                    reason: format!(
                        "runtime '{}' unresponsive for {:.0}ms (threshold: {:.0}ms)",
                        name,
                        now_ms - entry.last_heartbeat_ms,
                        self.hang_threshold_ms,
                    ),
                    health: RuntimeHealth::Degraded,
                    detected_at_ms: now_ms,
                });
            }
        }
        notices
    }

    /// Query health of a specific runtime.
    pub fn health(&self, name: &str) -> Option<RuntimeHealth> {
        self.runtimes.get(name).map(|e| e.health)
    }

    /// Number of monitored runtimes.
    pub fn runtime_count(&self) -> usize {
        self.runtimes.len()
    }

    /// Count of runtimes in a given health state.
    pub fn count_in_state(&self, state: RuntimeHealth) -> usize {
        self.runtimes.values().filter(|e| e.health == state).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hang_detected_within_threshold() {
        let mut monitor = ResilienceMonitor::new(500.0);
        monitor.register("audio", 0.0);
        monitor.register("visual", 0.0);

        // At 400ms: no hang yet
        let notices = monitor.check(400.0);
        assert!(notices.is_empty());

        // At 500ms: audio hangs (exactly at threshold)
        let notices = monitor.check(500.0);
        assert_eq!(notices.len(), 2); // both registered at 0, both hit 500ms
        assert!(notices.iter().all(|n| n.health == RuntimeHealth::Degraded));
    }

    #[test]
    fn heartbeat_prevents_degrade() {
        let mut monitor = ResilienceMonitor::new(500.0);
        monitor.register("audio", 0.0);

        // Heartbeat at 300ms keeps it healthy
        monitor.heartbeat("audio", 300.0);

        // At 700ms: only 400ms since last heartbeat, still within threshold
        let notices = monitor.check(700.0);
        assert!(notices.is_empty());

        // At 801ms: 501ms since last heartbeat → degrade
        let notices = monitor.check(801.0);
        assert_eq!(notices.len(), 1);
        assert_eq!(notices[0].runtime_name, "audio");
    }

    #[test]
    fn panic_detected_and_others_unaffected() {
        let mut monitor = ResilienceMonitor::new(500.0);
        monitor.register("audio", 0.0);
        monitor.register("visual", 0.0);

        // Audio panics
        let notice = monitor.mark_panicked("audio", 100.0);
        assert!(notice.is_some());
        assert_eq!(notice.unwrap().health, RuntimeHealth::Panicked);

        // Visual still healthy
        assert_eq!(monitor.health("audio"), Some(RuntimeHealth::Panicked));
        assert_eq!(monitor.health("visual"), Some(RuntimeHealth::Healthy));

        // Subsequent heartbeats from visual keep it healthy
        monitor.heartbeat("visual", 200.0);
        let notices = monitor.check(600.0);
        // Only visual checked (audio is Panicked, not re-emitted)
        assert!(notices.is_empty());
    }

    #[test]
    fn heartbeat_recovers_degraded_runtime() {
        let mut monitor = ResilienceMonitor::new(500.0);
        monitor.register("audio", 0.0);

        // Trigger degrade
        let notices = monitor.check(600.0);
        assert_eq!(notices.len(), 1);
        assert_eq!(monitor.health("audio"), Some(RuntimeHealth::Degraded));

        // Heartbeat recovers
        monitor.heartbeat("audio", 700.0);
        assert_eq!(monitor.health("audio"), Some(RuntimeHealth::Healthy));

        // No new degrade emitted
        let notices = monitor.check(900.0);
        assert!(notices.is_empty());
    }

    #[test]
    fn degrade_not_re_emitted_for_already_degraded() {
        let mut monitor = ResilienceMonitor::new(500.0);
        monitor.register("audio", 0.0);

        // First check → degrade
        let notices = monitor.check(600.0);
        assert_eq!(notices.len(), 1);

        // Second check → already degraded, no re-emit
        let notices = monitor.check(1200.0);
        assert!(notices.is_empty());
    }
}
