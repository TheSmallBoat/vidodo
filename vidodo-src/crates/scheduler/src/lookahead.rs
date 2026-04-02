use vidodo_ir::TimelineEntry;

/// Priority queue of timeline entries ordered by start bar.
///
/// The scheduler feeds all entries from a compiled revision into the
/// queue at the beginning of a run.  As the [`MusicalClock`](super::clock::MusicalClock)
/// advances, [`due_at`](LookaheadQueue::due_at) drains entries whose
/// effective window starts at the given bar.
#[derive(Debug, Clone)]
pub struct LookaheadQueue {
    entries: Vec<TimelineEntry>,
}

impl LookaheadQueue {
    /// Build a queue from the compiled timeline.
    ///
    /// Entries are sorted by `effective_window.from_bar` then by
    /// `-priority` (higher priority first).
    pub fn from_timeline(timeline: &[TimelineEntry]) -> Self {
        let mut entries: Vec<TimelineEntry> = timeline.to_vec();
        entries.sort_by(|a, b| {
            a.effective_window
                .from_bar
                .cmp(&b.effective_window.from_bar)
                .then_with(|| b.scheduler.priority.cmp(&a.scheduler.priority))
        });
        Self { entries }
    }

    /// Return all entries whose effective window starts at `bar`,
    /// removing them from the queue.
    pub fn due_at(&mut self, bar: u32) -> Vec<TimelineEntry> {
        let mut due = Vec::new();
        let mut remaining = Vec::new();
        for entry in self.entries.drain(..) {
            if entry.effective_window.from_bar == bar {
                due.push(entry);
            } else {
                remaining.push(entry);
            }
        }
        self.entries = remaining;
        due
    }

    /// Number of entries still in the queue.
    pub fn remaining(&self) -> usize {
        self.entries.len()
    }

    /// Whether the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vidodo_ir::{EffectiveWindow, TimelineScheduler};

    fn make_entry(id: &str, from_bar: u32, channel: &str, priority: i32) -> TimelineEntry {
        TimelineEntry {
            r#type: String::from("timeline_entry"),
            id: id.to_string(),
            show_id: String::from("test"),
            revision: 1,
            channel: channel.to_string(),
            target_ref: format!("ref-{id}"),
            effective_window: EffectiveWindow { from_bar, to_bar: from_bar + 8 },
            scheduler: TimelineScheduler {
                lookahead_ms: 0,
                priority,
                conflict_group: String::new(),
            },
            guards: std::collections::BTreeMap::new(),
        }
    }

    #[test]
    fn due_at_drains_matching_entries() {
        let entries = vec![
            make_entry("a", 1, "audio", 10),
            make_entry("b", 1, "visual", 5),
            make_entry("c", 9, "audio", 10),
        ];
        let mut queue = LookaheadQueue::from_timeline(&entries);
        assert_eq!(queue.remaining(), 3);

        let bar1 = queue.due_at(1);
        assert_eq!(bar1.len(), 2);
        assert_eq!(queue.remaining(), 1);

        // Higher priority first
        assert_eq!(bar1[0].id, "a");
        assert_eq!(bar1[1].id, "b");

        let bar9 = queue.due_at(9);
        assert_eq!(bar9.len(), 1);
        assert!(queue.is_empty());
    }

    #[test]
    fn due_at_returns_empty_for_no_match() {
        let entries = vec![make_entry("x", 5, "audio", 1)];
        let mut queue = LookaheadQueue::from_timeline(&entries);
        let result = queue.due_at(1);
        assert!(result.is_empty());
        assert_eq!(queue.remaining(), 1);
    }

    #[test]
    fn empty_queue() {
        let mut queue = LookaheadQueue::from_timeline(&[]);
        assert!(queue.is_empty());
        assert!(queue.due_at(1).is_empty());
    }
}
