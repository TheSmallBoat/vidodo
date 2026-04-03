use vidodo_ir::{BackendHealthSnapshot, DegradeMode};

/// Aggregated health decision produced by the health monitor.
#[derive(Debug, Clone, PartialEq)]
pub struct DegradeDecision {
    pub should_degrade: bool,
    pub modes: Vec<DegradeMode>,
}

/// Default thresholds for triggering degradation.
pub struct HealthThresholds {
    pub max_latency_ms: f64,
    pub max_error_count: u64,
}

impl Default for HealthThresholds {
    fn default() -> Self {
        Self { max_latency_ms: 200.0, max_error_count: 10 }
    }
}

/// Collect and aggregate health snapshots into a degradation decision.
///
/// For each snapshot with status `degraded` or `error`/`offline`,
/// or whose latency or error count exceeds thresholds, a `DegradeMode`
/// entry is produced.  If any mode is emitted, `should_degrade` is true.
pub fn degrade_decision(
    snapshots: &[BackendHealthSnapshot],
    thresholds: &HealthThresholds,
) -> DegradeDecision {
    let mut modes = Vec::new();

    for snapshot in snapshots {
        let status_bad = matches!(snapshot.status.as_str(), "degraded" | "error" | "offline");
        let latency_exceeded =
            snapshot.latency_ms.map(|ms| ms > thresholds.max_latency_ms).unwrap_or(false);
        let errors_exceeded =
            snapshot.error_count.map(|count| count > thresholds.max_error_count).unwrap_or(false);

        if status_bad || latency_exceeded || errors_exceeded {
            let reason = if status_bad {
                format!("backend status is '{}'", snapshot.status)
            } else if latency_exceeded {
                format!(
                    "latency {:.1}ms exceeds {:.1}ms threshold",
                    snapshot.latency_ms.unwrap_or(0.0),
                    thresholds.max_latency_ms,
                )
            } else {
                format!(
                    "error count {} exceeds {} threshold",
                    snapshot.error_count.unwrap_or(0),
                    thresholds.max_error_count,
                )
            };

            let fallback = snapshot
                .degrade_reason
                .clone()
                .or_else(|| Some(format!("bypass_{}", snapshot.backend_ref)));

            modes.push(DegradeMode {
                mode: format!("degrade_{}", snapshot.backend_ref),
                reason,
                affected_backends: vec![snapshot.backend_ref.clone()],
                fallback_action: fallback,
            });
        }
    }

    let should_degrade = !modes.is_empty();
    DegradeDecision { should_degrade, modes }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(
        backend: &str,
        status: &str,
        latency: Option<f64>,
        errors: Option<u64>,
    ) -> BackendHealthSnapshot {
        BackendHealthSnapshot {
            backend_ref: backend.to_string(),
            plugin_ref: format!("plugin-{backend}"),
            status: status.to_string(),
            timestamp: String::from("2026-01-01T00:00:00Z"),
            latency_ms: latency,
            error_count: errors,
            last_ack_lag_ms: None,
            degrade_reason: None,
        }
    }

    #[test]
    fn healthy_snapshots_produce_no_degradation() {
        let snaps = vec![
            snapshot("audio", "healthy", Some(10.0), Some(0)),
            snapshot("visual", "healthy", Some(20.0), Some(1)),
        ];
        let decision = degrade_decision(&snaps, &HealthThresholds::default());
        assert!(!decision.should_degrade);
        assert!(decision.modes.is_empty());
    }

    #[test]
    fn degraded_status_triggers_degrade_mode() {
        let snaps = vec![
            snapshot("audio", "healthy", Some(10.0), Some(0)),
            snapshot("visual", "degraded", Some(20.0), Some(0)),
        ];
        let decision = degrade_decision(&snaps, &HealthThresholds::default());
        assert!(decision.should_degrade);
        assert_eq!(decision.modes.len(), 1);
        assert_eq!(decision.modes[0].affected_backends, vec!["visual"]);
        assert!(decision.modes[0].reason.contains("degraded"));
    }

    #[test]
    fn high_latency_triggers_degrade_mode() {
        let snaps = vec![snapshot("audio", "healthy", Some(500.0), Some(0))];
        let thresholds = HealthThresholds { max_latency_ms: 200.0, max_error_count: 10 };
        let decision = degrade_decision(&snaps, &thresholds);
        assert!(decision.should_degrade);
        assert_eq!(decision.modes.len(), 1);
        assert!(decision.modes[0].reason.contains("latency"));
    }

    #[test]
    fn high_error_count_triggers_degrade_mode() {
        let snaps = vec![snapshot("lighting", "healthy", Some(10.0), Some(50))];
        let thresholds = HealthThresholds { max_latency_ms: 200.0, max_error_count: 10 };
        let decision = degrade_decision(&snaps, &thresholds);
        assert!(decision.should_degrade);
        assert!(decision.modes[0].reason.contains("error count"));
    }

    #[test]
    fn multiple_bad_backends_produce_multiple_modes() {
        let snaps = vec![
            snapshot("audio", "offline", None, None),
            snapshot("visual", "error", None, None),
            snapshot("lighting", "healthy", Some(5.0), Some(0)),
        ];
        let decision = degrade_decision(&snaps, &HealthThresholds::default());
        assert!(decision.should_degrade);
        assert_eq!(decision.modes.len(), 2);
    }
}
