use vidodo_ir::{
    CompiledRevision, MusicalTime, OutputBinding, ShowPatchState, ShowSemantic, ShowState,
    ShowTransition,
};

/// Build a [`ShowState`] snapshot from a compiled revision and the current
/// runtime context (active scene, layers, bar position, section id).
pub fn build_show_state(
    compiled: &CompiledRevision,
    active_visual_scene: &str,
    active_audio_layers: &[String],
    bar: u32,
    section_id: &str,
) -> ShowState {
    let phrase = compiled
        .structure_ir
        .sections
        .iter()
        .find(|section| section.section_id == section_id)
        .map(|section| section.order as u32 + 1)
        .unwrap_or(1);
    let section_plan =
        compiled.set_plan.sections.iter().find(|section| section.section_id == section_id);

    ShowState {
        show_id: compiled.show_id.clone(),
        revision: compiled.revision,
        mode: compiled.set_plan.mode.clone(),
        time: MusicalTime::at_bar(bar, phrase, section_id.to_string(), 128.0),
        semantic: ShowSemantic {
            energy: section_plan.and_then(|section| section.energy_target).unwrap_or(0.5),
            density: section_plan.and_then(|section| section.density_target).unwrap_or(0.5),
            tension: 0.6,
            brightness: if active_visual_scene.contains("drop") { 0.8 } else { 0.3 },
            motion: if active_visual_scene.contains("drop") { 0.7 } else { 0.2 },
            intent: compiled.set_plan.goal.intent.clone(),
        },
        transition: ShowTransition {
            state: String::from("steady"),
            from_scene: active_visual_scene.to_string(),
            to_scene: active_visual_scene.to_string(),
            window_open: true,
        },
        visual_output: OutputBinding {
            backend_id: String::from("fake_visual_backend"),
            topology_ref: String::from("flat-display-a"),
            calibration_profile: String::from("default-calibration"),
            active_group: active_visual_scene.to_string(),
        },
        audio_output: OutputBinding {
            backend_id: String::from("fake_audio_backend"),
            topology_ref: String::from("stereo-main"),
            calibration_profile: String::from("default-audio-calibration"),
            active_group: String::from("stereo-main"),
        },
        patch: ShowPatchState {
            allowed: !compiled.constraint_set.allowed_patch_scopes.is_empty(),
            scope: compiled
                .constraint_set
                .allowed_patch_scopes
                .first()
                .cloned()
                .unwrap_or_else(|| String::from("disabled")),
            locked_sections: compiled.constraint_set.locked_sections.clone(),
        },
        adapter_plugins: std::collections::BTreeMap::from([
            (String::from("audio"), String::from("plugin.audio.fake.v0")),
            (String::from("visual"), String::from("plugin.visual.fake.v0")),
        ]),
        resource_hubs: std::collections::BTreeMap::from([
            (String::from("audio"), String::from("hub.audio.fixture.v0")),
            (String::from("visual"), String::from("hub.visual.fixture.v0")),
        ]),
        active_audio_layers: active_audio_layers.to_vec(),
        active_visual_scene: active_visual_scene.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vidodo_compiler::compile_plan;
    use vidodo_ir::PlanBundle;

    #[test]
    fn build_show_state_fields_match_compiled() {
        let compiled =
            compile_plan(&PlanBundle::minimal("show-phase0")).expect("plan should compile");
        let state = build_show_state(&compiled, "scene_intro", &["layer-a".into()], 1, "intro");
        assert_eq!(state.show_id, "show-phase0");
        assert_eq!(state.revision, compiled.revision);
        assert_eq!(state.mode, compiled.set_plan.mode);
        assert_eq!(state.time.bar, 1);
        assert_eq!(state.active_visual_scene, "scene_intro");
        assert_eq!(state.active_audio_layers, vec!["layer-a"]);
    }

    #[test]
    fn snapshot_json_round_trips() {
        let compiled =
            compile_plan(&PlanBundle::minimal("show-phase0")).expect("plan should compile");
        let state = build_show_state(&compiled, "scene_intro", &[], 1, "intro");
        let json = serde_json::to_string_pretty(&state).expect("snapshot should succeed");
        let parsed: ShowState = serde_json::from_str(&json).expect("json should parse");
        assert_eq!(parsed.show_id, state.show_id);
        assert_eq!(parsed.time.bar, state.time.bar);
    }

    #[test]
    fn drop_section_has_high_brightness() {
        let compiled =
            compile_plan(&PlanBundle::minimal("show-phase0")).expect("plan should compile");
        let state = build_show_state(&compiled, "scene_drop_main", &[], 9, "drop");
        assert!(state.semantic.brightness > 0.5);
        assert!(state.semantic.motion > 0.5);
    }
}
