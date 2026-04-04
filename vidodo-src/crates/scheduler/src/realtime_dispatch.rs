//! Realtime event dispatch: checks the lookahead window each tick
//! and dispatches due events to the appropriate backend channels.

use std::collections::VecDeque;

use vidodo_ir::TimelineEntry;

/// Target channel for a dispatched event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchTarget {
    Audio,
    Visual,
    Lighting,
}

/// An event that has been dispatched at a specific time.
#[derive(Debug, Clone)]
pub struct DispatchedEvent {
    pub entry: TimelineEntry,
    pub target: DispatchTarget,
    pub dispatched_at_ms: f64,
    /// The beat-edge time in ms when this event became due.
    pub due_at_ms: f64,
}

impl DispatchedEvent {
    /// Latency from beat edge to actual dispatch (ms).
    pub fn latency_ms(&self) -> f64 {
        self.dispatched_at_ms - self.due_at_ms
    }
}

/// Classify a timeline entry's channel into a dispatch target.
fn classify_channel(channel: &str) -> DispatchTarget {
    match channel {
        "audio" | "performance" => DispatchTarget::Audio,
        "visual" | "video" => DispatchTarget::Visual,
        "lighting" | "dmx" => DispatchTarget::Lighting,
        _ => DispatchTarget::Audio, // default fallback
    }
}

/// A realtime event dispatcher that works at tick (1 ms) granularity.
///
/// Feed timeline entries in advance; each tick, call [`tick`] with the
/// current elapsed time to get events whose beat-edge has arrived.
pub struct RealtimeDispatcher {
    /// Events queued for future dispatch, ordered by due time in ms.
    queue: VecDeque<(f64, TimelineEntry)>,
    /// Lookahead window in milliseconds — events are considered due
    /// when `now_ms >= due_ms - lookahead_ms`.
    lookahead_ms: f64,
    /// Running count of dispatched events.
    dispatched_count: u64,
}

impl RealtimeDispatcher {
    pub fn new(lookahead_ms: f64) -> Self {
        Self { queue: VecDeque::new(), lookahead_ms, dispatched_count: 0 }
    }

    /// Enqueue timeline entries with their due time in milliseconds.
    ///
    /// `due_ms` should be computed from the entry's `effective_window.from_bar`
    /// and the current tempo: `bar_to_ms(from_bar, tempo, time_sig)`.
    pub fn enqueue(&mut self, due_ms: f64, entry: TimelineEntry) {
        // Insert in sorted order
        let pos = self.queue.iter().position(|(t, _)| *t > due_ms).unwrap_or(self.queue.len());
        self.queue.insert(pos, (due_ms, entry));
    }

    /// Enqueue a batch of entries. Each entry's due time is computed
    /// using `bar_to_ms`.
    pub fn enqueue_batch(&mut self, entries: &[TimelineEntry], tempo: f64, beats_per_bar: u32) {
        let ms_per_beat = 60_000.0 / tempo;
        for entry in entries {
            let bar = entry.effective_window.from_bar;
            // bar is 1-based; beat 0 is bar 1
            let due_ms = (bar.saturating_sub(1) as f64) * (beats_per_bar as f64) * ms_per_beat;
            self.enqueue(due_ms, entry.clone());
        }
    }

    /// Process one tick at the given wall-clock time.
    ///
    /// Returns all events that are now due (within the lookahead window).
    pub fn tick(&mut self, now_ms: f64) -> Vec<DispatchedEvent> {
        let mut dispatched = Vec::new();
        let threshold = now_ms + self.lookahead_ms;

        while let Some(&(due_ms, _)) = self.queue.front() {
            if due_ms <= threshold {
                let (due_ms, entry) = self.queue.pop_front().unwrap();
                let target = classify_channel(&entry.channel);
                dispatched.push(DispatchedEvent {
                    entry,
                    target,
                    dispatched_at_ms: now_ms,
                    due_at_ms: due_ms,
                });
                self.dispatched_count += 1;
            } else {
                break;
            }
        }

        dispatched
    }

    /// Number of events still in the queue.
    pub fn pending(&self) -> usize {
        self.queue.len()
    }

    /// Total events dispatched since creation.
    pub fn dispatched_count(&self) -> u64 {
        self.dispatched_count
    }
}

#[cfg(test)]
mod dispatch_tests {
    use super::*;
    use vidodo_ir::{EffectiveWindow, TimelineScheduler};

    fn make_entry(id: &str, from_bar: u32, channel: &str) -> TimelineEntry {
        TimelineEntry {
            r#type: String::from("timeline_entry"),
            id: id.to_string(),
            show_id: String::from("test"),
            revision: 1,
            channel: channel.to_string(),
            target_ref: format!("ref-{id}"),
            effective_window: EffectiveWindow { from_bar, to_bar: from_bar + 4 },
            scheduler: TimelineScheduler {
                lookahead_ms: 0,
                priority: 10,
                conflict_group: String::new(),
            },
            guards: std::collections::BTreeMap::new(),
        }
    }

    #[test]
    fn events_dispatch_at_due_time() {
        let mut dispatcher = RealtimeDispatcher::new(0.0);
        // At 120 bpm, 4/4: bar 1 = 0ms, bar 2 = 2000ms
        dispatcher.enqueue(0.0, make_entry("a", 1, "audio"));
        dispatcher.enqueue(2000.0, make_entry("b", 2, "visual"));

        let events = dispatcher.tick(0.0);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].entry.id, "a");
        assert_eq!(events[0].target, DispatchTarget::Audio);

        let events = dispatcher.tick(1999.0);
        assert!(events.is_empty());

        let events = dispatcher.tick(2000.0);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].entry.id, "b");
        assert_eq!(events[0].target, DispatchTarget::Visual);
    }

    #[test]
    fn lookahead_fires_events_early() {
        let mut dispatcher = RealtimeDispatcher::new(50.0); // 50ms lookahead
        dispatcher.enqueue(1000.0, make_entry("c", 2, "audio"));

        // At 950ms with 50ms lookahead, event at 1000ms is within window
        let events = dispatcher.tick(950.0);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].due_at_ms, 1000.0);
        assert!(events[0].latency_ms() < 0.0); // dispatched early
    }

    #[test]
    fn enqueue_batch_computes_due_times() {
        let mut dispatcher = RealtimeDispatcher::new(0.0);
        let entries = vec![make_entry("bar1", 1, "audio"), make_entry("bar3", 3, "lighting")];
        // 120 bpm, 4 beats/bar → 2000ms per bar
        dispatcher.enqueue_batch(&entries, 120.0, 4);

        // bar 1 = 0ms
        let events = dispatcher.tick(0.0);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].entry.id, "bar1");

        // bar 3 = 4000ms (bars 1,2 = 0ms, 2000ms; bar 3 = 4000ms)
        let events = dispatcher.tick(4000.0);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].entry.id, "bar3");
        assert_eq!(events[0].target, DispatchTarget::Lighting);
    }

    #[test]
    fn all_events_dispatched_in_order() {
        let mut dispatcher = RealtimeDispatcher::new(0.0);
        for i in 1..=4 {
            dispatcher.enqueue(i as f64 * 500.0, make_entry(&format!("e{i}"), i, "audio"));
        }

        let mut all = Vec::new();
        for ms in (0..=2500).step_by(1) {
            all.extend(dispatcher.tick(ms as f64));
        }

        assert_eq!(all.len(), 4);
        assert_eq!(dispatcher.pending(), 0);
        assert_eq!(dispatcher.dispatched_count(), 4);
        // Verify ordering
        for (i, event) in all.iter().enumerate() {
            assert_eq!(event.entry.id, format!("e{}", i + 1));
        }
    }

    #[test]
    fn latency_within_2ms() {
        let mut dispatcher = RealtimeDispatcher::new(0.0);
        // Events at beat edges: every 500ms (120bpm, each beat)
        for beat in 0..16 {
            let due_ms = beat as f64 * 500.0;
            dispatcher.enqueue(due_ms, make_entry(&format!("b{beat}"), 1, "audio"));
        }

        // Simulate ticking at 1ms intervals
        let mut all = Vec::new();
        for ms in 0..8000 {
            let events = dispatcher.tick(ms as f64);
            all.extend(events);
        }

        assert_eq!(all.len(), 16);
        for event in &all {
            assert!(
                event.latency_ms().abs() <= 2.0,
                "latency {}ms exceeds 2ms for {}",
                event.latency_ms(),
                event.entry.id
            );
        }
    }
}
