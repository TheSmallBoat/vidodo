use serde::{Deserialize, Serialize};
use vidodo_ir::{EventRecord, RunSummary, RuntimePayload, ShowState};
use vidodo_storage::ArtifactLayout;
use vidodo_trace::{load_events, load_manifest};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvaluationReport {
    pub report_id: String,
    pub show_id: String,
    pub revision: u64,
    pub run_id: String,
    pub scores: EvaluationScores,
    pub issues: Vec<EvaluationIssue>,
    pub summary: EvaluationSummary,
    pub facts: Vec<EvaluationFact>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvaluationScores {
    pub sync_score: f64,
    pub transition_score: f64,
    pub resource_stability_score: f64,
    pub patch_safety_score: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvaluationIssue {
    pub issue_id: String,
    pub code: String,
    pub severity: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span: Option<EvaluationSpan>,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvaluationSpan {
    pub from_bar: u32,
    pub to_bar: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvaluationSummary {
    pub verdict: String,
    pub recommended_action: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvaluationFact {
    pub fact_id: String,
    pub dimension: String,
    pub metric: String,
    pub value: f64,
    pub threshold: f64,
    pub status: String,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
}

pub fn evaluate_run(
    layout: &ArtifactLayout,
    run_id: &str,
    _summary: &RunSummary,
    _final_show_state: &ShowState,
) -> Result<EvaluationReport, String> {
    let manifest = load_manifest(layout, run_id)?;
    let events = load_events(layout, run_id)?;

    let mut issues = Vec::new();
    let mut facts = Vec::new();
    let mut next_fact = 1_u32;
    let mut next_issue = 1_u32;

    let sync_score = compute_sync_score(&events, &mut facts, &mut next_fact);
    let transition_score =
        compute_transition_score(&events, &mut issues, &mut facts, &mut next_fact, &mut next_issue);
    let resource_stability_score =
        compute_resource_stability_score(&events, &mut facts, &mut next_fact);
    let patch_safety_score = compute_patch_safety_score(
        &events,
        &mut issues,
        &mut facts,
        &mut next_fact,
        &mut next_issue,
    );

    let verdict = if issues.iter().any(|i| i.severity == "error") {
        "needs_revision"
    } else if issues.iter().any(|i| i.severity == "warning") {
        "usable_with_patch"
    } else {
        "clean"
    };

    let recommended_action = if verdict == "needs_revision" {
        String::from("address error-level issues before next run")
    } else if verdict == "usable_with_patch" {
        String::from("review warnings and consider targeted patches")
    } else {
        String::from("no action needed")
    };

    Ok(EvaluationReport {
        report_id: format!("eval-{}", vidodo_storage::slug(run_id)),
        show_id: manifest.show_id,
        revision: manifest.revision,
        run_id: run_id.to_string(),
        scores: EvaluationScores {
            sync_score,
            transition_score,
            resource_stability_score,
            patch_safety_score,
        },
        issues,
        summary: EvaluationSummary { verdict: verdict.to_string(), recommended_action },
        facts,
    })
}

fn compute_sync_score(
    events: &[EventRecord],
    facts: &mut Vec<EvaluationFact>,
    next_fact: &mut u32,
) -> f64 {
    let timing_events: Vec<&EventRecord> =
        events.iter().filter(|e| e.kind.starts_with("timing.")).collect();
    let audio_events: Vec<&EventRecord> =
        events.iter().filter(|e| e.kind.starts_with("audio.")).collect();
    let visual_events: Vec<&EventRecord> =
        events.iter().filter(|e| e.kind.starts_with("visual.")).collect();

    if timing_events.is_empty() {
        return 0.0;
    }

    // Check that all section entries have corresponding audio/visual events
    let section_enter_count =
        timing_events.iter().filter(|e| e.kind == "timing.section.enter").count();

    // Find sections where audio follows within a reasonable window
    let mut synced_sections = 0_usize;
    for timing in timing_events.iter().filter(|e| e.kind == "timing.section.enter") {
        let bar = timing.musical_time.bar;
        let has_audio = audio_events.iter().any(|e| e.musical_time.bar == bar);
        let has_visual = visual_events.iter().any(|e| e.musical_time.bar == bar);
        if has_audio || has_visual {
            synced_sections += 1;
        }
    }

    let score = if section_enter_count > 0 {
        synced_sections as f64 / section_enter_count as f64
    } else {
        1.0
    };

    facts.push(EvaluationFact {
        fact_id: format!("fact-{:03}", next_fact),
        dimension: String::from("sync"),
        metric: String::from("section_sync_ratio"),
        value: score,
        threshold: 0.8,
        status: if score >= 0.8 { "ok" } else { "below_threshold" }.to_string(),
        evidence_refs: vec![],
    });
    *next_fact += 1;

    score
}

fn compute_transition_score(
    events: &[EventRecord],
    issues: &mut Vec<EvaluationIssue>,
    facts: &mut Vec<EvaluationFact>,
    next_fact: &mut u32,
    next_issue: &mut u32,
) -> f64 {
    let visual_events: Vec<&EventRecord> =
        events.iter().filter(|e| e.kind.starts_with("visual.")).collect();
    let timing_events: Vec<&EventRecord> =
        events.iter().filter(|e| e.kind == "timing.section.enter").collect();

    if timing_events.is_empty() || visual_events.is_empty() {
        return 1.0;
    }

    // Check that visual scene transitions align with section boundaries
    let mut well_aligned = 0_usize;
    let section_bars: Vec<u32> = timing_events.iter().map(|e| e.musical_time.bar).collect();

    for visual in &visual_events {
        let bar = visual.musical_time.bar;
        if section_bars.contains(&bar) {
            well_aligned += 1;
        } else {
            issues.push(EvaluationIssue {
                issue_id: format!("issue-{:03}", next_issue),
                code: String::from("visual_transition_off_boundary"),
                severity: String::from("warning"),
                message: format!(
                    "visual event {} at bar {} is not aligned with a section boundary",
                    visual.event_id, bar
                ),
                span: Some(EvaluationSpan { from_bar: bar, to_bar: bar + 1 }),
                evidence_refs: vec![visual.event_id.clone()],
            });
            *next_issue += 1;
        }
    }

    let score = if visual_events.is_empty() {
        1.0
    } else {
        well_aligned as f64 / visual_events.len() as f64
    };

    facts.push(EvaluationFact {
        fact_id: format!("fact-{:03}", next_fact),
        dimension: String::from("transition"),
        metric: String::from("visual_section_alignment_ratio"),
        value: score,
        threshold: 0.7,
        status: if score >= 0.7 { "ok" } else { "below_threshold" }.to_string(),
        evidence_refs: vec![],
    });
    *next_fact += 1;

    score
}

fn compute_resource_stability_score(
    events: &[EventRecord],
    facts: &mut Vec<EvaluationFact>,
    next_fact: &mut u32,
) -> f64 {
    // Check that all events have successful acks
    let events_with_ack = events.iter().filter(|e| e.ack.is_some()).count();
    let events_with_ok_ack =
        events.iter().filter(|e| e.ack.as_ref().is_some_and(|ack| ack.status == "ok")).count();

    let score =
        if events_with_ack == 0 { 1.0 } else { events_with_ok_ack as f64 / events_with_ack as f64 };

    facts.push(EvaluationFact {
        fact_id: format!("fact-{:03}", next_fact),
        dimension: String::from("resource_stability"),
        metric: String::from("ack_success_ratio"),
        value: score,
        threshold: 0.95,
        status: if score >= 0.95 { "ok" } else { "below_threshold" }.to_string(),
        evidence_refs: vec![],
    });
    *next_fact += 1;

    score
}

fn compute_patch_safety_score(
    events: &[EventRecord],
    issues: &mut Vec<EvaluationIssue>,
    facts: &mut Vec<EvaluationFact>,
    next_fact: &mut u32,
    next_issue: &mut u32,
) -> f64 {
    let patch_events: Vec<&EventRecord> =
        events.iter().filter(|e| e.kind.starts_with("patch.")).collect();

    if patch_events.is_empty() {
        facts.push(EvaluationFact {
            fact_id: format!("fact-{:03}", next_fact),
            dimension: String::from("patch_safety"),
            metric: String::from("patch_applied_count"),
            value: 0.0,
            threshold: 0.0,
            status: String::from("no_patches"),
            evidence_refs: vec![],
        });
        *next_fact += 1;
        return 1.0;
    }

    // Check that all patch events have acks
    let patch_with_ack = patch_events.iter().filter(|e| e.ack.is_some()).count();
    let patch_with_ok_ack = patch_events
        .iter()
        .filter(|e| e.ack.as_ref().is_some_and(|ack| ack.status == "ok"))
        .count();

    // Check for patch events with proper scope
    let patches_with_scope = patch_events
        .iter()
        .filter(|e| matches!(&e.payload, RuntimePayload::Patch(p) if p.fallback_revision > 0))
        .count();

    let score = if patch_events.is_empty() {
        1.0
    } else {
        let ack_ratio =
            if patch_with_ack > 0 { patch_with_ok_ack as f64 / patch_with_ack as f64 } else { 0.5 };
        let scope_ratio = patches_with_scope as f64 / patch_events.len() as f64;
        (ack_ratio + scope_ratio) / 2.0
    };

    if score < 0.8 {
        issues.push(EvaluationIssue {
            issue_id: format!("issue-{:03}", next_issue),
            code: String::from("patch_safety_low"),
            severity: String::from("warning"),
            message: String::from("patch safety score below threshold"),
            span: None,
            evidence_refs: patch_events.iter().map(|e| e.event_id.clone()).collect(),
        });
        *next_issue += 1;
    }

    facts.push(EvaluationFact {
        fact_id: format!("fact-{:03}", next_fact),
        dimension: String::from("patch_safety"),
        metric: String::from("patch_safety_composite"),
        value: score,
        threshold: 0.8,
        status: if score >= 0.8 { "ok" } else { "below_threshold" }.to_string(),
        evidence_refs: vec![],
    });
    *next_fact += 1;

    score
}

#[cfg(test)]
mod tests {
    use super::*;
    use vidodo_ir::{AudioEvent, BackendAck, MusicalTime, RuntimePayload, TimingEvent};

    fn timing_event(event_id: &str, bar: u32, section: &str) -> EventRecord {
        EventRecord {
            event_id: event_id.to_string(),
            show_id: String::from("show-test"),
            revision: 1,
            kind: String::from("timing.section.enter"),
            phase: String::from("executed"),
            source: String::from("scheduler"),
            musical_time: MusicalTime::at_bar(bar, 1, section.to_string(), 128.0),
            scheduler_time_ms: bar as u64 * 1000,
            wallclock_time_ms: bar as u64 * 1000,
            causation_id: String::from("test"),
            payload: RuntimePayload::Timing(TimingEvent {
                phrase: 1,
                section: section.to_string(),
                tempo: 128.0,
                downbeat: true,
                bar: Some(bar),
                beat: Some(bar as f64 * 4.0),
                time_signature: Some([4, 4]),
                transition_window_open: Some(true),
            }),
            ack: None,
        }
    }

    fn audio_event(event_id: &str, bar: u32) -> EventRecord {
        EventRecord {
            event_id: event_id.to_string(),
            show_id: String::from("show-test"),
            revision: 1,
            kind: String::from("audio.launch_asset"),
            phase: String::from("executed"),
            source: String::from("scheduler"),
            musical_time: MusicalTime::at_bar(bar, 1, String::from("intro"), 128.0),
            scheduler_time_ms: bar as u64 * 1000,
            wallclock_time_ms: bar as u64 * 1000,
            causation_id: String::from("test"),
            payload: RuntimePayload::Audio(AudioEvent {
                layer_id: String::from("rhythm-main"),
                op: String::from("launch_asset"),
                output_backend: String::from("fake_audio_backend"),
                route_mode: None,
                route_set_ref: None,
                speaker_group: vec![],
                gain_db: None,
                duration_beats: None,
                filter: None,
                automation: std::collections::BTreeMap::new(),
                target_asset_id: None,
            }),
            ack: Some(BackendAck {
                backend: String::from("fake_audio_backend"),
                target: String::from("rhythm-main"),
                status: String::from("ok"),
                detail: String::from("ok"),
            }),
        }
    }

    #[test]
    fn sync_score_is_perfect_when_all_sections_have_events() {
        let events = vec![
            timing_event("evt-001", 1, "intro"),
            audio_event("evt-002", 1),
            timing_event("evt-003", 9, "build"),
            audio_event("evt-004", 9),
        ];
        let mut facts = Vec::new();
        let mut next_fact = 1;
        let score = compute_sync_score(&events, &mut facts, &mut next_fact);
        assert!((score - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn sync_score_drops_when_sections_lack_content_events() {
        let events = vec![
            timing_event("evt-001", 1, "intro"),
            // no audio/visual at bar 1
            timing_event("evt-003", 9, "build"),
            audio_event("evt-004", 9),
        ];
        let mut facts = Vec::new();
        let mut next_fact = 1;
        let score = compute_sync_score(&events, &mut facts, &mut next_fact);
        assert!((score - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn resource_stability_is_perfect_with_all_ok_acks() {
        let events = vec![audio_event("evt-001", 1), audio_event("evt-002", 9)];
        let mut facts = Vec::new();
        let mut next_fact = 1;
        let score = compute_resource_stability_score(&events, &mut facts, &mut next_fact);
        assert!((score - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn patch_safety_perfect_when_no_patches() {
        let events = vec![timing_event("evt-001", 1, "intro")];
        let mut facts = Vec::new();
        let mut issues = Vec::new();
        let mut nf = 1;
        let mut ni = 1;
        let score = compute_patch_safety_score(&events, &mut issues, &mut facts, &mut nf, &mut ni);
        assert!((score - 1.0).abs() < f64::EPSILON);
        assert!(issues.is_empty());
    }
}
