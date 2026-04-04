//! Scheduler → runtime thread dispatch integration.
//!
//! `SchedulerIpc` wraps the realtime dispatcher and IPC channels,
//! distributing events to per-runtime threads and collecting acks.
//!
//! `ThreadRuntimeContainer` manages spawning and joining runtime worker
//! threads, each consuming from its own channel.

use std::sync::mpsc;
use std::thread::{self, JoinHandle};

/// A runtime event to dispatch (typed payload).
#[derive(Debug, Clone)]
pub struct RuntimeEvent {
    pub correlation_id: String,
    pub channel: String,
    pub payload_json: String,
}

/// An ack from a runtime thread.
#[derive(Debug, Clone)]
pub struct RuntimeAckMsg {
    pub correlation_id: String,
    pub runtime_name: String,
    pub success: bool,
    pub detail: String,
}

/// Per-runtime channel pair for event dispatch and ack collection.
pub struct RuntimeChannel {
    pub name: String,
    pub event_tx: mpsc::SyncSender<RuntimeEvent>,
    pub ack_rx: mpsc::Receiver<RuntimeAckMsg>,
}

/// Manages dispatch of events to multiple runtime threads via IPC channels.
pub struct SchedulerIpc {
    runtimes: Vec<RuntimeChannel>,
}

impl SchedulerIpc {
    pub fn new() -> Self {
        Self { runtimes: Vec::new() }
    }

    /// Register a runtime with its pre-created channels.
    pub fn register(&mut self, channel: RuntimeChannel) {
        self.runtimes.push(channel);
    }

    /// Dispatch an event to the matching runtime by channel name.
    ///
    /// Returns `true` if dispatched, `false` if no matching runtime.
    pub fn dispatch(&self, event: &RuntimeEvent) -> bool {
        for rt in &self.runtimes {
            if rt.name == event.channel {
                return rt.event_tx.try_send(event.clone()).is_ok();
            }
        }
        false
    }

    /// Collect all available acks from all runtimes (non-blocking).
    pub fn collect_acks(&self) -> Vec<RuntimeAckMsg> {
        let mut acks = Vec::new();
        for rt in &self.runtimes {
            while let Ok(ack) = rt.ack_rx.try_recv() {
                acks.push(ack);
            }
        }
        acks
    }

    /// Number of registered runtimes.
    pub fn runtime_count(&self) -> usize {
        self.runtimes.len()
    }
}

impl Default for SchedulerIpc {
    fn default() -> Self {
        Self::new()
    }
}

/// Manages spawning runtime worker threads.
pub struct ThreadRuntimeContainer {
    handles: Vec<(String, JoinHandle<()>)>,
}

impl ThreadRuntimeContainer {
    pub fn new() -> Self {
        Self { handles: Vec::new() }
    }

    /// Spawn a runtime worker thread that processes events from its
    /// channel and sends acks back.
    ///
    /// `handler` is the function that processes each event and returns
    /// `(success, detail)`.
    pub fn spawn<F>(
        &mut self,
        name: impl Into<String>,
        event_rx: mpsc::Receiver<RuntimeEvent>,
        ack_tx: mpsc::SyncSender<RuntimeAckMsg>,
        handler: F,
    ) where
        F: Fn(&RuntimeEvent) -> (bool, String) + Send + 'static,
    {
        let name = name.into();
        let thread_name = name.clone();
        let handle = thread::spawn(move || {
            while let Ok(event) = event_rx.recv() {
                let (success, detail) = handler(&event);
                let ack = RuntimeAckMsg {
                    correlation_id: event.correlation_id.clone(),
                    runtime_name: thread_name.clone(),
                    success,
                    detail,
                };
                if ack_tx.try_send(ack).is_err() {
                    break; // ack channel closed
                }
            }
        });
        self.handles.push((name, handle));
    }

    /// Join all worker threads (blocking until they finish).
    pub fn join_all(self) -> Vec<String> {
        let mut names = Vec::new();
        for (name, handle) in self.handles {
            if handle.join().is_ok() {
                names.push(name);
            }
        }
        names
    }

    /// Number of spawned threads.
    pub fn thread_count(&self) -> usize {
        self.handles.len()
    }
}

impl Default for ThreadRuntimeContainer {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a paired channel set for a runtime: (RuntimeChannel for scheduler, event_rx for thread, ack_tx for thread).
pub fn create_runtime_channels(
    name: impl Into<String>,
    capacity: usize,
) -> (RuntimeChannel, mpsc::Receiver<RuntimeEvent>, mpsc::SyncSender<RuntimeAckMsg>) {
    let name = name.into();
    let (event_tx, event_rx) = mpsc::sync_channel(capacity);
    let (ack_tx, ack_rx) = mpsc::sync_channel(capacity);
    (RuntimeChannel { name, event_tx, ack_rx }, event_rx, ack_tx)
}

#[cfg(test)]
mod scheduler_ipc_tests {
    use super::*;
    use std::time::Duration;

    fn make_event(id: &str, channel: &str) -> RuntimeEvent {
        RuntimeEvent {
            correlation_id: id.to_string(),
            channel: channel.to_string(),
            payload_json: format!("{{\"id\":\"{id}\"}}"),
        }
    }

    #[test]
    fn dispatch_to_mock_runtime_and_collect_ack() {
        let (rt_channel, event_rx, ack_tx) = create_runtime_channels("audio", 16);

        let mut ipc = SchedulerIpc::new();
        ipc.register(rt_channel);

        let mut container = ThreadRuntimeContainer::new();
        container.spawn("audio", event_rx, ack_tx, |event| {
            (true, format!("processed {}", event.correlation_id))
        });

        // Dispatch an event
        assert!(ipc.dispatch(&make_event("evt-1", "audio")));

        // Wait briefly for processing
        thread::sleep(Duration::from_millis(50));

        let acks = ipc.collect_acks();
        assert_eq!(acks.len(), 1);
        assert!(acks[0].success);
        assert_eq!(acks[0].correlation_id, "evt-1");
        assert_eq!(acks[0].runtime_name, "audio");

        // Drop sender to shut down thread
        drop(ipc);
        container.join_all();
    }

    #[test]
    fn three_runtimes_dispatch_and_collect() {
        let runtime_names = ["audio", "visual", "lighting"];
        let mut ipc = SchedulerIpc::new();
        let mut container = ThreadRuntimeContainer::new();

        for name in &runtime_names {
            let (rt_channel, event_rx, ack_tx) = create_runtime_channels(*name, 16);
            ipc.register(rt_channel);
            container.spawn(*name, event_rx, ack_tx, |event| {
                (true, format!("ok:{}", event.correlation_id))
            });
        }

        // Dispatch 4 events: 2 audio, 1 visual, 1 lighting
        assert!(ipc.dispatch(&make_event("a1", "audio")));
        assert!(ipc.dispatch(&make_event("a2", "audio")));
        assert!(ipc.dispatch(&make_event("v1", "visual")));
        assert!(ipc.dispatch(&make_event("l1", "lighting")));

        thread::sleep(Duration::from_millis(100));

        let acks = ipc.collect_acks();
        assert_eq!(acks.len(), 4, "all 4 events should be acked");

        // Drop sender channels to let threads exit
        drop(ipc);
        let joined = container.join_all();
        assert_eq!(joined.len(), 3);
    }

    #[test]
    fn unknown_channel_returns_false() {
        let ipc = SchedulerIpc::new();
        assert!(!ipc.dispatch(&make_event("x", "nonexistent")));
    }

    #[test]
    fn threads_shutdown_gracefully() {
        let (rt_channel, event_rx, ack_tx) = create_runtime_channels("test", 4);

        let mut ipc = SchedulerIpc::new();
        ipc.register(rt_channel);

        let mut container = ThreadRuntimeContainer::new();
        container.spawn("test", event_rx, ack_tx, |_| (true, String::new()));

        assert_eq!(container.thread_count(), 1);

        // Drop the IPC (drops senders) to signal threads to exit
        drop(ipc);

        let joined = container.join_all();
        assert_eq!(joined, vec!["test"]);
    }
}
