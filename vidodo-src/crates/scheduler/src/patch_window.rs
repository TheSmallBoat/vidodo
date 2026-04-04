//! Realtime patch window: patches are only activated on beat edges
//! inside a safe zone (≥ 500 ms before the next transport change).
//!
//! Pending patches that don't activate within the window are
//! automatically rolled back.

/// Minimum safe distance (in ms) from the next transport change
/// for a patch to be activated.
const SAFE_ZONE_MS: f64 = 500.0;

/// Status of a pending realtime patch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchWindowStatus {
    /// Waiting for a beat-edge + safe zone.
    Pending,
    /// Activated on a beat edge.
    Activated,
    /// Window closed before activation — auto-rolled-back.
    RolledBack,
}

/// A patch proposal waiting for a safe activation window.
#[derive(Debug, Clone)]
pub struct PendingPatch {
    pub patch_id: String,
    pub submitted_at_ms: f64,
    pub deadline_ms: f64,
    pub status: PatchWindowStatus,
}

/// Trace event emitted for patch window open/close.
#[derive(Debug, Clone)]
pub struct PatchWindowEvent {
    pub patch_id: String,
    pub event_type: PatchWindowEventType,
    pub at_ms: f64,
}

/// Type of patch window event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchWindowEventType {
    /// Window opened: a patch was submitted and is pending.
    WindowOpen,
    /// Patch activated at a beat edge inside the safe zone.
    Activated,
    /// Window closed: deadline expired, patch auto-rolled back.
    WindowClosed,
}

/// Manages pending patches and their activation windows.
pub struct PatchWindow {
    pending: Vec<PendingPatch>,
    /// Duration of the activation window (ms).
    window_duration_ms: f64,
    /// Trace events for diagnostics / recording.
    trace: Vec<PatchWindowEvent>,
}

impl PatchWindow {
    /// Create a new patch window with the given activation window duration (ms).
    pub fn new(window_duration_ms: f64) -> Self {
        Self { pending: Vec::new(), window_duration_ms, trace: Vec::new() }
    }

    /// Submit a patch for activation. The deadline is `now_ms + window_duration_ms`.
    pub fn submit(&mut self, patch_id: &str, now_ms: f64) {
        let deadline = now_ms + self.window_duration_ms;
        self.pending.push(PendingPatch {
            patch_id: patch_id.to_string(),
            submitted_at_ms: now_ms,
            deadline_ms: deadline,
            status: PatchWindowStatus::Pending,
        });
        self.trace.push(PatchWindowEvent {
            patch_id: patch_id.to_string(),
            event_type: PatchWindowEventType::WindowOpen,
            at_ms: now_ms,
        });
    }

    /// Check the given tick for activation opportunities and deadline expiry.
    ///
    /// - `now_ms`: current wall-clock time in ms.
    /// - `is_beat_edge`: whether the current tick is on a beat boundary.
    /// - `ms_to_next_transport_change`: distance to the next transport event.
    ///
    /// Returns a list of patches that were activated or rolled back this tick.
    pub fn tick(
        &mut self,
        now_ms: f64,
        is_beat_edge: bool,
        ms_to_next_transport_change: f64,
    ) -> Vec<PatchWindowEvent> {
        let mut events = Vec::new();
        let in_safe_zone = ms_to_next_transport_change >= SAFE_ZONE_MS;

        for patch in &mut self.pending {
            if patch.status != PatchWindowStatus::Pending {
                continue;
            }

            // Check deadline expiry first
            if now_ms >= patch.deadline_ms {
                patch.status = PatchWindowStatus::RolledBack;
                let evt = PatchWindowEvent {
                    patch_id: patch.patch_id.clone(),
                    event_type: PatchWindowEventType::WindowClosed,
                    at_ms: now_ms,
                };
                events.push(evt.clone());
                self.trace.push(evt);
                continue;
            }

            // Try activation on beat edge + safe zone
            if is_beat_edge && in_safe_zone {
                patch.status = PatchWindowStatus::Activated;
                let evt = PatchWindowEvent {
                    patch_id: patch.patch_id.clone(),
                    event_type: PatchWindowEventType::Activated,
                    at_ms: now_ms,
                };
                events.push(evt.clone());
                self.trace.push(evt);
            }
        }

        events
    }

    /// Drain all completed (activated or rolled-back) patches.
    pub fn drain_completed(&mut self) -> Vec<PendingPatch> {
        let mut completed = Vec::new();
        self.pending.retain(|p| {
            if p.status != PatchWindowStatus::Pending {
                completed.push(p.clone());
                false
            } else {
                true
            }
        });
        completed
    }

    /// Number of pending patches.
    pub fn pending_count(&self) -> usize {
        self.pending.iter().filter(|p| p.status == PatchWindowStatus::Pending).count()
    }

    /// Drain trace events for recording.
    pub fn drain_trace(&mut self) -> Vec<PatchWindowEvent> {
        std::mem::take(&mut self.trace)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patch_activates_on_beat_edge_in_safe_zone() {
        let mut pw = PatchWindow::new(4000.0);
        pw.submit("patch-1", 0.0);

        // Beat edge at t=500, safe zone = 1000 ms to next transport
        let events = pw.tick(500.0, true, 1000.0);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, PatchWindowEventType::Activated);
        assert_eq!(events[0].patch_id, "patch-1");
    }

    #[test]
    fn patch_delayed_when_not_safe_zone() {
        let mut pw = PatchWindow::new(4000.0);
        pw.submit("patch-1", 0.0);

        // Beat edge but only 200 ms to next transport change (< 500)
        let events = pw.tick(500.0, true, 200.0);
        assert!(events.is_empty());
        assert_eq!(pw.pending_count(), 1);

        // Next beat edge in safe zone
        let events = pw.tick(1000.0, true, 800.0);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, PatchWindowEventType::Activated);
    }

    #[test]
    fn patch_auto_rollback_on_deadline() {
        let mut pw = PatchWindow::new(2000.0); // 2s window
        pw.submit("patch-1", 0.0);

        // No beat edges until after deadline
        let events = pw.tick(1000.0, false, 5000.0);
        assert!(events.is_empty());

        // Deadline at 2000 ms
        let events = pw.tick(2000.0, false, 5000.0);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, PatchWindowEventType::WindowClosed);

        let completed = pw.drain_completed();
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].status, PatchWindowStatus::RolledBack);
    }

    #[test]
    fn trace_records_window_open_and_close() {
        let mut pw = PatchWindow::new(1000.0);
        pw.submit("patch-1", 100.0);
        pw.tick(1100.0, false, 5000.0); // deadline expires

        let trace = pw.drain_trace();
        assert_eq!(trace.len(), 2);
        assert_eq!(trace[0].event_type, PatchWindowEventType::WindowOpen);
        assert_eq!(trace[1].event_type, PatchWindowEventType::WindowClosed);
    }
}
