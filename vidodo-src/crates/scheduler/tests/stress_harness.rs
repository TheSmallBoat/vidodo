//! WSW-01: Long-running stress test harness.
//!
//! Generates N-bar timelines and verifies:
//! - No panic during scheduler run
//! - No event loss (event count matches expected)
//! - Clock drift ≤ 1ms between consecutive events

use std::collections::BTreeMap;
use vidodo_compiler::compile_plan;
use vidodo_ir::{PlanBundle, PlanSection, SetPlan, SetPlanGoal};
use vidodo_scheduler::simulate_run;

/// Build a PlanBundle with `n_sections` sections of `bars_per_section` bars each.
fn build_large_plan(n_sections: usize, bars_per_section: u32, show_id: &str) -> PlanBundle {
    let sections: Vec<PlanSection> = (0..n_sections)
        .map(|i| PlanSection {
            section_id: format!("section-{i:03}"),
            length_bars: bars_per_section,
            energy_target: Some(0.2 + (i as f64 / n_sections as f64) * 0.6),
            density_target: Some(0.3),
            visual_intent: Some(format!("scene_{}", if i % 2 == 0 { "intro" } else { "drop" })),
        })
        .collect();

    let section_refs: Vec<String> = sections.iter().map(|s| s.section_id.clone()).collect();

    // Reuse the minimal plan shape but with more sections.
    let mut base = PlanBundle::minimal(show_id);

    base.set_plan = SetPlan {
        r#type: String::from("set_plan"),
        id: String::from("set-stress-main"),
        show_id: show_id.to_string(),
        mode: String::from("live"),
        goal: SetPlanGoal {
            intent: String::from("stress_test"),
            duration_target_sec: Some((n_sections as u32) * bars_per_section * 2),
            style_tags: vec![String::from("stress")],
        },
        asset_pool_refs: vec![String::from("pool-minimal")],
        sections,
        constraints_ref: String::from("constraint-phase0-main"),
        delivery: BTreeMap::from([(String::from("trace_bundle"), true)]),
    };

    // Update audio_dsl section_refs so layers are active across all sections.
    for layer in &mut base.audio_dsl.layers {
        layer.entry_rules.section_refs = section_refs.clone();
    }

    base
}

#[test]
fn stress_128_bar_run_no_panic_no_event_loss() {
    let n_sections = 16; // 16 sections × 8 bars = 128 bars
    let bars_per_section = 8;
    let plan = build_large_plan(n_sections, bars_per_section, "show-stress-128");
    let compiled = compile_plan(&plan).expect("large plan should compile");

    assert_eq!(compiled.final_bar(), (n_sections as u32) * bars_per_section);

    let run = simulate_run(&compiled, "run-stress-128");

    // No panic if we get here.
    // Verify event count: at least one timing event per section + audio/visual events.
    let timing_events: Vec<_> =
        run.events.iter().filter(|e| e.kind.starts_with("timing.")).collect();
    assert_eq!(timing_events.len(), n_sections, "expected one timing event per section");

    // Verify resource samples: one per section.
    assert_eq!(run.resource_samples.len(), n_sections, "expected one resource sample per section");

    // Verify summary is consistent.
    assert_eq!(run.summary.starting_bar, 1);
    assert_eq!(run.summary.final_bar, (n_sections as u32) * bars_per_section);
    assert_eq!(run.summary.event_count, run.events.len());
}

#[test]
fn stress_clock_drift_within_1ms() {
    let plan = build_large_plan(16, 8, "show-stress-drift");
    let compiled = compile_plan(&plan).expect("plan should compile");
    let run = simulate_run(&compiled, "run-stress-drift");

    // Check that scheduler_time_ms is monotonically increasing and
    // wallclock_time_ms tracks scheduler_time_ms (drift ≤ 1ms in deterministic mode).
    for window in run.events.windows(2) {
        let prev = &window[0];
        let curr = &window[1];
        assert!(
            curr.scheduler_time_ms >= prev.scheduler_time_ms,
            "scheduler time must be monotonically increasing: {} -> {}",
            prev.scheduler_time_ms,
            curr.scheduler_time_ms,
        );
        let drift = (curr.wallclock_time_ms as i64 - curr.scheduler_time_ms as i64).unsigned_abs();
        assert!(
            drift <= 1,
            "clock drift exceeds 1ms: wallclock={}, scheduler={}",
            curr.wallclock_time_ms,
            curr.scheduler_time_ms,
        );
    }
}

#[test]
fn stress_256_bar_memory_not_growing() {
    // Run a 256-bar plan and check that event/resource counts scale linearly.
    let n_sections = 32;
    let bars_per_section = 8;
    let plan = build_large_plan(n_sections, bars_per_section, "show-stress-256");
    let compiled = compile_plan(&plan).expect("plan should compile");
    let run = simulate_run(&compiled, "run-stress-256");

    // Basic sanity: we got through 256 bars without panic.
    assert_eq!(compiled.final_bar(), 256);
    assert_eq!(run.resource_samples.len(), n_sections);

    // Resource samples should not show unbounded memory growth.
    // In our deterministic model, memory_mb = 512 + layers * 64.
    // Check first vs last sample: difference should be bounded.
    let first_mem = run.resource_samples.first().unwrap().memory_mb;
    let last_mem = run.resource_samples.last().unwrap().memory_mb;
    let growth = last_mem as i64 - first_mem as i64;
    // Allow some growth from additional layers being activated, but not unlimited.
    assert!(
        growth.unsigned_abs() < 512,
        "memory growth too large: first={first_mem} last={last_mem} growth={growth}"
    );
}
