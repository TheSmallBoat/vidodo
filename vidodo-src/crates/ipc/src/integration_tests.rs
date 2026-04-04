//! WSAD-06 Integration tests: IPC E2E and stress tests.
//!
//! Verifies:
//! - 3 runtime threads + scheduler dispatch → 8 bar show → all acks collected
//! - Causation chain complete and queryable
//! - Stress: 10000 events → zero loss
//! - Memory stability (no unbounded growth)

#[cfg(test)]
mod ipc_integration {
    use std::thread;
    use std::time::Duration;

    use crate::causation::CausalTracer;
    use crate::resilience::ResilienceMonitor;
    use crate::scheduler_ipc::{
        RuntimeEvent, SchedulerIpc, ThreadRuntimeContainer, create_runtime_channels,
    };

    /// Simulate an 8-bar show with 3 runtime threads (audio, visual, lighting).
    /// Each bar dispatches one event per runtime = 24 events total.
    /// All events must be acked.
    #[test]
    fn eight_bar_show_all_acks() {
        let runtime_names = ["audio", "visual", "lighting"];
        let mut ipc = SchedulerIpc::new();
        let mut container = ThreadRuntimeContainer::new();

        for name in &runtime_names {
            let (rt_channel, event_rx, ack_tx) = create_runtime_channels(*name, 64);
            ipc.register(rt_channel);
            container.spawn(*name, event_rx, ack_tx, |event| {
                (true, format!("ok:{}", event.correlation_id))
            });
        }

        // 8 bars × 3 runtimes = 24 events
        let bars = 8;
        let mut dispatched = 0;
        for bar in 1..=bars {
            for name in &runtime_names {
                let event = RuntimeEvent {
                    correlation_id: format!("corr-bar{bar}-{name}"),
                    channel: name.to_string(),
                    payload_json: format!("{{\"bar\":{bar},\"channel\":\"{name}\"}}"),
                };
                assert!(ipc.dispatch(&event), "dispatch to {name} at bar {bar} failed");
                dispatched += 1;
            }
        }
        assert_eq!(dispatched, 24);

        // Wait for processing
        thread::sleep(Duration::from_millis(200));

        // Collect all acks
        let acks = ipc.collect_acks();
        assert_eq!(acks.len(), 24, "all 24 events must be acked, got {}", acks.len());

        // All acks successful
        assert!(acks.iter().all(|a| a.success));

        // Verify each runtime acked
        let audio_acks = acks.iter().filter(|a| a.runtime_name == "audio").count();
        let visual_acks = acks.iter().filter(|a| a.runtime_name == "visual").count();
        let lighting_acks = acks.iter().filter(|a| a.runtime_name == "lighting").count();
        assert_eq!(audio_acks, 8);
        assert_eq!(visual_acks, 8);
        assert_eq!(lighting_acks, 8);

        // Shutdown
        drop(ipc);
        let joined = container.join_all();
        assert_eq!(joined.len(), 3);
    }

    /// 8-bar show with full causal chain tracing: scheduler → channel → runtime ack.
    /// Each event has a 3-hop chain that can be fully reconstructed.
    #[test]
    fn eight_bar_show_causation_chain() {
        let runtime_names = ["audio", "visual", "lighting"];
        let mut ipc = SchedulerIpc::new();
        let mut container = ThreadRuntimeContainer::new();
        let mut tracer = CausalTracer::new();

        for name in &runtime_names {
            let (rt_channel, event_rx, ack_tx) = create_runtime_channels(*name, 64);
            ipc.register(rt_channel);
            container.spawn(*name, event_rx, ack_tx, |event| {
                (true, format!("ok:{}", event.correlation_id))
            });
        }

        let bars = 8;
        let mut timestamp = 0.0;

        for bar in 1..=bars {
            for name in &runtime_names {
                let corr_id = format!("corr-bar{bar}-{name}");

                // Hop 1: scheduler tick → event
                let msg1 = tracer.next_message_id();
                tracer.record(&msg1, None, &corr_id, "scheduler", timestamp);
                timestamp += 1.0;

                // Hop 2: event dispatch → channel
                let msg2 = tracer.next_message_id();
                tracer.record(&msg2, Some(&msg1), &corr_id, "channel", timestamp);
                timestamp += 1.0;

                let event = RuntimeEvent {
                    correlation_id: corr_id.clone(),
                    channel: name.to_string(),
                    payload_json: format!("{{\"bar\":{bar}}}"),
                };
                ipc.dispatch(&event);
            }
        }

        thread::sleep(Duration::from_millis(200));
        let acks = ipc.collect_acks();

        // Hop 3: record each ack
        for ack in &acks {
            let msg3 = tracer.next_message_id();
            // Find the parent (channel dispatch message)
            let chain = tracer.query_chain(&ack.correlation_id);
            let parent = chain.last().map(|l| l.message_id.clone());
            tracer.record(&msg3, parent.as_deref(), &ack.correlation_id, "runtime_ack", timestamp);
            timestamp += 0.5;
        }

        // Verify: 24 correlation chains, each with 3 hops
        assert_eq!(tracer.chain_count(), 24);
        for bar in 1..=bars {
            for name in &runtime_names {
                let corr_id = format!("corr-bar{bar}-{name}");
                let chain = tracer.query_chain(&corr_id);
                assert_eq!(
                    chain.len(),
                    3,
                    "chain for {corr_id} should be 3 hops, got {}",
                    chain.len()
                );
                assert_eq!(chain[0].hop, "scheduler");
                assert_eq!(chain[1].hop, "channel");
                assert_eq!(chain[2].hop, "runtime_ack");
            }
        }

        assert_eq!(tracer.total_links(), 72); // 24 × 3

        drop(ipc);
        container.join_all();
    }

    /// 8-bar show with resilience monitoring integrated.
    /// All runtimes heartbeat every bar, so no hangs detected.
    #[test]
    fn eight_bar_show_with_resilience_monitoring() {
        let runtime_names = ["audio", "visual", "lighting"];
        let mut ipc = SchedulerIpc::new();
        let mut container = ThreadRuntimeContainer::new();
        let mut monitor = ResilienceMonitor::new(500.0);

        for name in &runtime_names {
            let (rt_channel, event_rx, ack_tx) = create_runtime_channels(*name, 64);
            ipc.register(rt_channel);
            monitor.register(name, 0.0);
            container.spawn(*name, event_rx, ack_tx, |event| {
                (true, format!("ok:{}", event.correlation_id))
            });
        }

        let bars = 8;
        let ms_per_bar = 2000.0; // 120 BPM → 2s per bar
        for bar in 1..=bars {
            let now = bar as f64 * ms_per_bar;

            // Dispatch events
            for name in &runtime_names {
                let event = RuntimeEvent {
                    correlation_id: format!("corr-{bar}-{name}"),
                    channel: name.to_string(),
                    payload_json: String::new(),
                };
                ipc.dispatch(&event);
            }

            // Simulate heartbeats from all runtimes
            for name in &runtime_names {
                monitor.heartbeat(name, now);
            }

            // Check resilience — should find no hangs
            let notices = monitor.check(now);
            assert!(notices.is_empty(), "bar {bar}: unexpected degrade notices: {notices:?}");
        }

        // Final health: all healthy
        for name in &runtime_names {
            assert_eq!(monitor.health(name), Some(crate::resilience::RuntimeHealth::Healthy));
        }

        thread::sleep(Duration::from_millis(200));
        let acks = ipc.collect_acks();
        assert_eq!(acks.len(), 24);

        drop(ipc);
        container.join_all();
    }

    /// Stress test: 10000 events dispatched to 3 runtime threads.
    /// All events must be acked with zero loss.
    #[test]
    fn stress_ten_thousand_events_zero_loss() {
        let runtime_names = ["audio", "visual", "lighting"];
        let mut ipc = SchedulerIpc::new();
        let mut container = ThreadRuntimeContainer::new();

        for name in &runtime_names {
            let (rt_channel, event_rx, ack_tx) = create_runtime_channels(*name, 4096);
            ipc.register(rt_channel);
            container.spawn(*name, event_rx, ack_tx, |event| {
                (true, format!("ok:{}", event.correlation_id))
            });
        }

        let total_events = 10_000;
        let mut dispatched = 0;
        for i in 0..total_events {
            let name = runtime_names[i % 3];
            let event = RuntimeEvent {
                correlation_id: format!("stress-{i:05}"),
                channel: name.to_string(),
                payload_json: format!("{{\"seq\":{i}}}"),
            };
            // Retry with brief yield on back-pressure
            let mut sent = false;
            for _ in 0..20 {
                if ipc.dispatch(&event) {
                    sent = true;
                    break;
                }
                thread::yield_now();
            }
            assert!(sent, "dispatch {i} to {name} failed after retries");
            dispatched += 1;
        }
        assert_eq!(dispatched, total_events);

        // Wait for processing — allow more time for stress
        thread::sleep(Duration::from_millis(1000));

        // Collect all acks
        let acks = ipc.collect_acks();
        assert_eq!(acks.len(), total_events, "expected {total_events} acks, got {}", acks.len());
        assert!(acks.iter().all(|a| a.success));

        // Verify we got acks from all 3 runtimes
        let per_runtime: Vec<usize> = runtime_names
            .iter()
            .map(|n| acks.iter().filter(|a| a.runtime_name == *n).count())
            .collect();
        // 10000 / 3 = 3333 or 3334
        for (name, count) in runtime_names.iter().zip(&per_runtime) {
            assert!(*count >= 3333 && *count <= 3334, "{name} processed {count} events");
        }

        drop(ipc);
        container.join_all();
    }

    /// Memory stability: dispatching 10000 events should not cause
    /// unbounded growth in ack collection.
    #[test]
    fn stress_memory_stability() {
        let (rt_channel, event_rx, ack_tx) = create_runtime_channels("test", 2048);
        let mut ipc = SchedulerIpc::new();
        ipc.register(rt_channel);

        let mut container = ThreadRuntimeContainer::new();
        container.spawn("test", event_rx, ack_tx, |event| {
            (true, format!("ok:{}", event.correlation_id))
        });

        // Dispatch + collect in batches to check memory doesn't grow
        let events_per_batch = 1000;
        let batches = 10;
        let mut total_acks = 0;

        for batch in 0..batches {
            // Dispatch a batch
            for i in 0..events_per_batch {
                let seq = batch * events_per_batch + i;
                let event = RuntimeEvent {
                    correlation_id: format!("mem-{seq:05}"),
                    channel: String::from("test"),
                    payload_json: format!("{{\"seq\":{seq}}}"),
                };
                assert!(ipc.dispatch(&event));
            }

            thread::sleep(Duration::from_millis(100));

            // Collect and discard acks immediately — this keeps memory bounded
            let acks = ipc.collect_acks();
            total_acks += acks.len();
        }

        // All events acked
        assert_eq!(total_acks, events_per_batch * batches);

        drop(ipc);
        container.join_all();
    }
}
