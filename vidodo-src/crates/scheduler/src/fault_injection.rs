use vidodo_ir::BackendHealthSnapshot;

/// Trait for injecting faults into the scheduler run loop.
///
/// Implementations return degraded [`BackendHealthSnapshot`]s when the
/// fault condition is met (e.g. a specific bar is reached).  The
/// scheduler merges these with the real backend snapshots so that
/// `health_monitor::degrade_decision` fires as expected.
pub trait FaultInjector {
    /// Evaluate the current bar and return any synthetic health snapshots
    /// that represent injected faults.  An empty vec means no fault.
    fn inject(&self, bar: u32) -> Vec<BackendHealthSnapshot>;
}

/// No-op injector — never injects faults.
pub struct NullFaultInjector;

impl FaultInjector for NullFaultInjector {
    fn inject(&self, _bar: u32) -> Vec<BackendHealthSnapshot> {
        Vec::new()
    }
}

/// Injects a degraded-backend snapshot at a specific bar.
///
/// When the scheduler reaches `trigger_bar`, this injector produces a
/// single snapshot with `status = "degraded"` and latency above the
/// default threshold, causing `degrade_decision` to fire.
pub struct FailAtBarInjector {
    pub trigger_bar: u32,
    pub backend_ref: String,
}

impl FailAtBarInjector {
    pub fn new(trigger_bar: u32, backend_ref: &str) -> Self {
        Self { trigger_bar, backend_ref: backend_ref.to_string() }
    }
}

impl FaultInjector for FailAtBarInjector {
    fn inject(&self, bar: u32) -> Vec<BackendHealthSnapshot> {
        if bar == self.trigger_bar {
            vec![BackendHealthSnapshot {
                backend_ref: self.backend_ref.clone(),
                plugin_ref: format!("plugin-{}", self.backend_ref),
                status: String::from("degraded"),
                timestamp: String::from("2026-01-01T00:00:00Z"),
                latency_ms: Some(500.0),
                error_count: Some(0),
                last_ack_lag_ms: None,
                degrade_reason: Some(format!("fault_injected_at_bar_{}", self.trigger_bar)),
            }]
        } else {
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_injector_never_faults() {
        let injector = NullFaultInjector;
        for bar in 1..=64 {
            assert!(injector.inject(bar).is_empty());
        }
    }

    #[test]
    fn fail_at_bar_fires_at_trigger() {
        let injector = FailAtBarInjector::new(8, "audio_backend_1");
        let snapshots = injector.inject(8);
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].status, "degraded");
        assert_eq!(snapshots[0].backend_ref, "audio_backend_1");
        assert!(snapshots[0].latency_ms.unwrap() > 200.0);
    }

    #[test]
    fn fail_at_bar_silent_at_other_bars() {
        let injector = FailAtBarInjector::new(8, "audio_backend_1");
        assert!(injector.inject(1).is_empty());
        assert!(injector.inject(7).is_empty());
        assert!(injector.inject(9).is_empty());
    }

    #[test]
    fn fail_at_bar_degrade_reason_includes_bar() {
        let injector = FailAtBarInjector::new(16, "visual_backend_1");
        let snaps = injector.inject(16);
        assert_eq!(snaps.len(), 1);
        let reason = snaps[0].degrade_reason.as_ref().unwrap();
        assert!(reason.contains("16"), "expected reason to mention bar 16, got {reason}");
    }
}
