use std::collections::BTreeMap;

use vidodo_ir::{
    CompiledRevision, CueSet, Diagnostic, EffectiveWindow, MusicalTime, PerformanceAction,
    PerformanceIr, PlanBundle, StructureIr, StructureSection, StructureSpan, TimelineEntry,
    TimelineScheduler, VisualAction, VisualIr,
};
use vidodo_validator::validate_plan;

pub mod analysis_cache;
pub mod revision;

pub fn compile_plan(plan: &PlanBundle) -> Result<CompiledRevision, Vec<Diagnostic>> {
    let diagnostics = validate_plan(plan)
        .into_iter()
        .filter(|diagnostic| diagnostic.severity == "error")
        .collect::<Vec<_>>();
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let revision = plan.base_revision + 1;
    let structure_ir = build_structure(plan);
    let performance_ir = build_performance(plan, &structure_ir);
    let visual_ir = build_visual(plan, &structure_ir);
    let mut timeline = build_timeline(
        plan.show_id(),
        revision,
        &structure_ir,
        &performance_ir,
        &visual_ir,
        &plan.cue_sets,
    );
    sort_timeline(&mut timeline);

    Ok(CompiledRevision {
        show_id: plan.show_id.clone(),
        revision,
        base_revision: plan.base_revision,
        compile_run_id: format!("compile-{}-rev-{revision}", sanitize(plan.show_id())),
        set_plan: plan.set_plan.clone(),
        audio_dsl: plan.audio_dsl.clone(),
        visual_dsl: plan.visual_dsl.clone(),
        constraint_set: plan.constraint_set.clone(),
        asset_records: plan.asset_records.clone(),
        structure_ir,
        performance_ir,
        visual_ir,
        timeline,
        patch_history: Vec::new(),
        lighting_topology: plan.lighting_topology.clone(),
        cue_sets: plan.cue_sets.clone(),
    })
}

/// Compile with analysis cache enrichment.
///
/// Loads analysis hints from `cache_dir` for assets referenced in the plan,
/// and injects `beat_map`, `detected_key`, and `section_boundaries` into the
/// resulting `PerformanceIr`. Returns `(revision, warnings)`.
pub fn compile_plan_with_analysis(
    plan: &PlanBundle,
    cache_dir: &std::path::Path,
) -> Result<(CompiledRevision, Vec<Diagnostic>), Vec<Diagnostic>> {
    let mut revision = compile_plan(plan)?;

    // Collect asset IDs from audio layers
    let asset_ids: Vec<String> =
        plan.audio_dsl.layers.iter().filter_map(|l| l.asset_candidates.first().cloned()).collect();

    let (hints_map, warnings) = analysis_cache::load_analysis_hints(cache_dir, &asset_ids);
    let merged = analysis_cache::merge_hints(&hints_map);

    revision.performance_ir.beat_map = merged.beat_map;
    revision.performance_ir.detected_key = merged.detected_key;
    revision.performance_ir.section_boundaries = merged.section_boundaries;

    Ok((revision, warnings))
}

fn build_structure(plan: &PlanBundle) -> StructureIr {
    let mut next_bar = 1;
    let sections = plan
        .set_plan
        .sections
        .iter()
        .enumerate()
        .map(|(index, section)| {
            let start_bar = next_bar;
            let end_bar = start_bar + section.length_bars.saturating_sub(1);
            next_bar = end_bar + 1;
            StructureSection {
                section_id: section.section_id.clone(),
                order: index,
                span: StructureSpan { start_bar, end_bar },
                targets: BTreeMap::new(),
                locks: BTreeMap::from([(
                    String::from("locked"),
                    plan.constraint_set.locked_sections.contains(&section.section_id).to_string(),
                )]),
            }
        })
        .collect();

    StructureIr {
        r#type: String::from("structure_ir"),
        id: format!("structure-{}", sanitize(plan.show_id())),
        show_id: plan.show_id.clone(),
        sections,
        transitions: Vec::new(),
    }
}

fn build_performance(plan: &PlanBundle, structure: &StructureIr) -> PerformanceIr {
    let section_lookup = structure
        .sections
        .iter()
        .map(|section| (section.section_id.clone(), section))
        .collect::<BTreeMap<_, _>>();

    let mut performance_actions = Vec::new();
    for layer in &plan.audio_dsl.layers {
        let target_sections = if layer.entry_rules.section_refs.is_empty() {
            structure.sections.iter().map(|section| section.section_id.clone()).collect::<Vec<_>>()
        } else {
            layer.entry_rules.section_refs.clone()
        };

        for section_id in target_sections {
            if let Some(section) = section_lookup.get(&section_id) {
                let phrase = section.order as u32 + 1;
                performance_actions.push(PerformanceAction {
                    action_id: format!("audio-{}-{}", section.section_id, layer.layer_id),
                    layer_id: layer.layer_id.clone(),
                    op: String::from("launch_asset"),
                    target_asset_id: layer.asset_candidates.first().cloned(),
                    musical_time: MusicalTime::at_bar(
                        section.span.start_bar,
                        phrase,
                        section.section_id.clone(),
                        128.0,
                    ),
                    duration_beats: (section.span.end_bar - section.span.start_bar + 1) * 4,
                    quantize: layer.entry_rules.quantize.clone(),
                    priority: if layer.role == "rhythm" { 10 } else { 20 },
                    rollback_token: format!("rollback-{}-{}", section.section_id, layer.layer_id),
                    resource_hint: BTreeMap::from([(
                        String::from("route_group"),
                        layer
                            .route_group_ref
                            .clone()
                            .unwrap_or_else(|| String::from("stereo-main")),
                    )]),
                    output_backend_hint: layer.output_backend_hint.clone(),
                    route_set_ref: layer.route_group_ref.clone(),
                });
            }
        }
    }

    PerformanceIr {
        performance_actions,
        beat_map: Vec::new(),
        detected_key: None,
        section_boundaries: Vec::new(),
    }
}

fn build_visual(plan: &PlanBundle, structure: &StructureIr) -> VisualIr {
    let scene_lookup = plan
        .visual_dsl
        .scenes
        .iter()
        .map(|scene| (scene.scene_id.clone(), scene))
        .collect::<BTreeMap<_, _>>();
    let fallback_scene = plan.visual_dsl.scenes.first();

    let visual_actions = structure
        .sections
        .iter()
        .filter_map(|section| {
            let section_plan = plan
                .set_plan
                .sections
                .iter()
                .find(|candidate| candidate.section_id == section.section_id)?;
            let scene = section_plan
                .visual_intent
                .as_ref()
                .and_then(|scene_id| scene_lookup.get(scene_id))
                .copied()
                .or(fallback_scene)?;
            Some(VisualAction {
                visual_action_id: format!("visual-{}", section.section_id),
                scene_id: scene.scene_id.clone(),
                program_ref: scene.program_ref.clone(),
                uniform_set: scene.uniform_defaults.clone(),
                camera_state: BTreeMap::new(),
                output_backend: scene.output_backend.clone(),
                view_group_ref: scene.view_group_ref.clone(),
                display_topology_ref: scene.display_topology_ref.clone(),
                duration_beats: (section.span.end_bar - section.span.start_bar + 1) * 4,
                blend_mode: Some(String::from("replace")),
                gpu_cost_hint: Some(0.2),
                fallback_scene_id: fallback_scene.map(|fallback| fallback.scene_id.clone()),
            })
        })
        .collect();

    VisualIr { visual_actions }
}

fn build_timeline(
    show_id: &str,
    revision: u64,
    structure: &StructureIr,
    performance: &PerformanceIr,
    visual: &VisualIr,
    cue_sets: &[CueSet],
) -> Vec<TimelineEntry> {
    let section_lookup = structure
        .sections
        .iter()
        .map(|section| (section.section_id.clone(), section))
        .collect::<BTreeMap<_, _>>();
    let mut timeline = Vec::new();

    for action in &performance.performance_actions {
        if let Some(section) = section_lookup.get(&action.musical_time.section) {
            timeline.push(TimelineEntry {
                r#type: String::from("timeline_entry"),
                id: format!("timeline-{}", action.action_id),
                show_id: show_id.to_string(),
                revision,
                channel: String::from("audio"),
                target_ref: action.action_id.clone(),
                effective_window: EffectiveWindow {
                    from_bar: section.span.start_bar,
                    to_bar: section.span.end_bar,
                },
                scheduler: TimelineScheduler {
                    lookahead_ms: 250,
                    priority: action.priority,
                    conflict_group: format!("audio-{}", section.section_id),
                },
                guards: BTreeMap::new(),
            });
        }
    }

    for action in &visual.visual_actions {
        if let Some(section) = section_lookup.get(&action.scene_id.replace("scene_", "")) {
            timeline.push(TimelineEntry {
                r#type: String::from("timeline_entry"),
                id: format!("timeline-{}", action.visual_action_id),
                show_id: show_id.to_string(),
                revision,
                channel: String::from("visual"),
                target_ref: action.visual_action_id.clone(),
                effective_window: EffectiveWindow {
                    from_bar: section.span.start_bar,
                    to_bar: section.span.end_bar,
                },
                scheduler: TimelineScheduler {
                    lookahead_ms: 250,
                    priority: 30,
                    conflict_group: format!("visual-{}", section.section_id),
                },
                guards: BTreeMap::new(),
            });
        }
    }

    // Build lighting timeline entries from cue sets
    for cue_set in cue_sets {
        for (index, cue) in cue_set.entries.iter().enumerate() {
            // Match cue source_ref to a section
            if let Some(section) = section_lookup.get(&cue.source_ref) {
                timeline.push(TimelineEntry {
                    r#type: String::from("timeline_entry"),
                    id: format!("timeline-lighting-{}-{index}", cue_set.cue_set_id),
                    show_id: show_id.to_string(),
                    revision,
                    channel: String::from("lighting"),
                    target_ref: format!("{}:{index}", cue_set.cue_set_id),
                    effective_window: EffectiveWindow {
                        from_bar: section.span.start_bar,
                        to_bar: section.span.end_bar,
                    },
                    scheduler: TimelineScheduler {
                        lookahead_ms: 250,
                        priority: 40,
                        conflict_group: format!("lighting-{}", section.section_id),
                    },
                    guards: BTreeMap::new(),
                });
            }
        }
    }

    timeline
}

fn sort_timeline(timeline: &mut [TimelineEntry]) {
    timeline.sort_by(|left, right| {
        left.effective_window
            .from_bar
            .cmp(&right.effective_window.from_bar)
            .then(left.scheduler.priority.cmp(&right.scheduler.priority))
            .then(left.channel.cmp(&right.channel))
            .then(left.id.cmp(&right.id))
    });
}

fn sanitize(value: &str) -> String {
    value
        .chars()
        .map(|character| match character {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => character,
            _ => '-',
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::compile_plan;
    use vidodo_ir::PlanBundle;

    #[test]
    fn compiles_a_minimal_plan_deterministically() {
        let plan = PlanBundle::minimal("show-phase0");

        let first = compile_plan(&plan).expect("minimal plan should compile");
        let second = compile_plan(&plan).expect("minimal plan should compile");

        assert_eq!(serde_json::to_string(&first).unwrap(), serde_json::to_string(&second).unwrap());
        assert_eq!(first.revision, 1);
        assert!(!first.timeline.is_empty());
        assert_eq!(first.structure_ir.sections.len(), 2);
    }

    #[test]
    fn rejects_plan_with_validation_errors() {
        let mut plan = PlanBundle::minimal("show-phase0");
        plan.audio_dsl.layers[0].asset_candidates = vec![String::from("nonexistent-asset")];

        let result = compile_plan(&plan);
        assert!(result.is_err());
        let diagnostics = result.unwrap_err();
        assert!(diagnostics.iter().any(|d| d.code == "VAL-006"));
    }

    #[test]
    fn rejects_empty_show_id() {
        let plan = PlanBundle::minimal("");
        let result = compile_plan(&plan);
        assert!(result.is_err());
        let diagnostics = result.unwrap_err();
        assert!(diagnostics.iter().any(|d| d.code == "VAL-001"));
    }

    #[test]
    fn timeline_is_sorted_by_bar_then_priority() {
        let plan = PlanBundle::minimal("show-phase0");
        let compiled = compile_plan(&plan).expect("should compile");
        for window in compiled.timeline.windows(2) {
            let a_bar = window[0].effective_window.from_bar;
            let b_bar = window[1].effective_window.from_bar;
            assert!(
                a_bar <= b_bar,
                "timeline entries should be sorted by from_bar: {} > {}",
                a_bar,
                b_bar
            );
        }
    }
}
