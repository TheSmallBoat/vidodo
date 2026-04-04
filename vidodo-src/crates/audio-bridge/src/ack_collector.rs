//! Ack collection from scsynth `/done` and `/fail` OSC replies.
//!
//! Associates incoming reply messages with the `action_id` that
//! originated the command, tracking per-action status.

use std::collections::HashMap;

use crate::osc::OscMessage;

/// Status of a dispatched action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionStatus {
    /// Command sent, waiting for ack.
    Pending,
    /// `/done` received.
    Loaded,
    /// `/fail` received or timeout.
    Failed,
}

/// Tracks in-flight actions and correlates scsynth acks.
pub struct AckCollector {
    /// action_id → (node_id, status, sent_timestamp_ms).
    actions: HashMap<String, (i32, ActionStatus, f64)>,
    /// node_id → action_id reverse lookup.
    node_to_action: HashMap<i32, String>,
    /// Timeout threshold in milliseconds.
    timeout_ms: f64,
}

impl AckCollector {
    pub fn new(timeout_ms: f64) -> Self {
        Self { actions: HashMap::new(), node_to_action: HashMap::new(), timeout_ms }
    }

    /// Register a new pending action.
    pub fn register(&mut self, action_id: &str, node_id: i32, sent_at_ms: f64) {
        self.actions.insert(action_id.to_string(), (node_id, ActionStatus::Pending, sent_at_ms));
        self.node_to_action.insert(node_id, action_id.to_string());
    }

    /// Process an incoming OSC reply from scsynth.
    ///
    /// Recognizes `/done` (→ Loaded) and `/fail` (→ Failed) addresses.
    /// The first integer arg is treated as the node/buffer id for correlation.
    pub fn process_reply(&mut self, msg: &OscMessage) -> Option<(String, ActionStatus)> {
        let new_status = match msg.address.as_str() {
            "/done" => ActionStatus::Loaded,
            "/fail" => ActionStatus::Failed,
            _ => return None,
        };

        // Try to extract a node_id from the first int arg
        let node_id = msg
            .args
            .first()
            .and_then(|arg| if let crate::osc::OscArg::Int(id) = arg { Some(*id) } else { None })?;

        let action_id = self.node_to_action.get(&node_id)?.clone();
        if let Some(entry) = self.actions.get_mut(&action_id) {
            entry.1 = new_status;
        }
        Some((action_id, new_status))
    }

    /// Check for timed-out actions and mark them as failed.
    /// Returns the list of action_ids that just timed out.
    pub fn check_timeouts(&mut self, now_ms: f64) -> Vec<String> {
        let mut timed_out = Vec::new();
        for (action_id, (_, status, sent_at)) in &mut self.actions {
            if *status == ActionStatus::Pending && (now_ms - *sent_at) >= self.timeout_ms {
                *status = ActionStatus::Failed;
                timed_out.push(action_id.clone());
            }
        }
        timed_out
    }

    /// Get the status of an action.
    pub fn status(&self, action_id: &str) -> Option<ActionStatus> {
        self.actions.get(action_id).map(|(_, s, _)| *s)
    }

    /// Number of actions still pending.
    pub fn pending_count(&self) -> usize {
        self.actions.values().filter(|(_, s, _)| *s == ActionStatus::Pending).count()
    }

    /// Remove completed (non-pending) actions and return them.
    pub fn drain_completed(&mut self) -> Vec<(String, ActionStatus)> {
        let completed: Vec<_> = self
            .actions
            .iter()
            .filter(|(_, (_, s, _))| *s != ActionStatus::Pending)
            .map(|(id, (_, s, _))| (id.clone(), *s))
            .collect();

        for (id, _) in &completed {
            if let Some((node_id, _, _)) = self.actions.remove(id) {
                self.node_to_action.remove(&node_id);
            }
        }
        completed
    }
}

#[cfg(test)]
mod ack_tests {
    use super::*;
    use crate::osc::{OscArg, OscMessage};

    fn done_msg(node_id: i32) -> OscMessage {
        OscMessage::new("/done", vec![OscArg::Int(node_id)])
    }

    fn fail_msg(node_id: i32) -> OscMessage {
        OscMessage::new("/fail", vec![OscArg::Int(node_id)])
    }

    #[test]
    fn register_and_receive_done() {
        let mut collector = AckCollector::new(5000.0);
        collector.register("bass", 1000, 0.0);
        assert_eq!(collector.status("bass"), Some(ActionStatus::Pending));

        let result = collector.process_reply(&done_msg(1000));
        assert_eq!(result, Some(("bass".into(), ActionStatus::Loaded)));
        assert_eq!(collector.status("bass"), Some(ActionStatus::Loaded));
    }

    #[test]
    fn receive_fail_marks_failed() {
        let mut collector = AckCollector::new(5000.0);
        collector.register("synth-1", 1001, 0.0);

        let result = collector.process_reply(&fail_msg(1001));
        assert_eq!(result, Some(("synth-1".into(), ActionStatus::Failed)));
    }

    #[test]
    fn timeout_marks_pending_as_failed() {
        let mut collector = AckCollector::new(5000.0);
        collector.register("pad", 1002, 100.0);
        assert_eq!(collector.pending_count(), 1);

        let timed_out = collector.check_timeouts(5100.0);
        assert_eq!(timed_out, vec!["pad"]);
        assert_eq!(collector.status("pad"), Some(ActionStatus::Failed));
        assert_eq!(collector.pending_count(), 0);
    }

    #[test]
    fn unknown_node_id_ignored() {
        let mut collector = AckCollector::new(5000.0);
        collector.register("a", 1000, 0.0);

        let result = collector.process_reply(&done_msg(9999));
        assert!(result.is_none());
        assert_eq!(collector.status("a"), Some(ActionStatus::Pending));
    }

    #[test]
    fn drain_completed_removes_finished_actions() {
        let mut collector = AckCollector::new(5000.0);
        collector.register("a", 1000, 0.0);
        collector.register("b", 1001, 0.0);
        collector.process_reply(&done_msg(1000));

        let completed = collector.drain_completed();
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].0, "a");
        assert!(collector.status("a").is_none());
        assert_eq!(collector.pending_count(), 1);
    }
}
