use std::collections::BTreeSet;

use vidodo_ir::{Diagnostic, PlanBundle};

pub fn validate_plan(plan: &PlanBundle) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    if plan.show_id.trim().is_empty() {
        diagnostics.push(Diagnostic::error("VAL-001", "show_id must not be empty"));
    }

    if plan.set_plan.show_id != plan.show_id
        || plan.audio_dsl.show_id != plan.show_id
        || plan.visual_dsl.show_id != plan.show_id
    {
        diagnostics.push(Diagnostic::error(
            "VAL-002",
            "set_plan, audio_dsl, and visual_dsl must share the same show_id",
        ));
    }

    if plan.set_plan.sections.is_empty() {
        diagnostics
            .push(Diagnostic::error("VAL-003", "set_plan must contain at least one section"));
    }

    if plan.audio_dsl.layers.len() as u32 > plan.constraint_set.max_audio_layers {
        diagnostics.push(Diagnostic::error(
            "VAL-004",
            "audio layer count exceeds constraint_set.max_audio_layers",
        ));
    }

    let published_assets: BTreeSet<&str> =
        plan.asset_records.iter().map(|asset| asset.asset_id.as_str()).collect();
    let known_sections: BTreeSet<&str> =
        plan.set_plan.sections.iter().map(|section| section.section_id.as_str()).collect();

    for layer in &plan.audio_dsl.layers {
        if layer.asset_candidates.is_empty() {
            diagnostics.push(Diagnostic::error(
                "VAL-005",
                format!("audio layer {} must declare at least one asset candidate", layer.layer_id),
            ));
        }

        for asset_id in &layer.asset_candidates {
            if !published_assets.contains(asset_id.as_str()) {
                diagnostics.push(Diagnostic::error(
                    "VAL-006",
                    format!("audio layer {} references unknown asset {}", layer.layer_id, asset_id),
                ));
            }
        }

        for section_id in &layer.entry_rules.section_refs {
            if !known_sections.contains(section_id.as_str()) {
                diagnostics.push(Diagnostic::error(
                    "VAL-007",
                    format!(
                        "audio layer {} references unknown section {}",
                        layer.layer_id, section_id
                    ),
                ));
            }
        }
    }

    for section in &plan.set_plan.sections {
        if let Some(visual_intent) = &section.visual_intent
            && !plan.visual_dsl.scenes.iter().any(|scene| &scene.scene_id == visual_intent)
        {
            diagnostics.push(Diagnostic::error(
                "VAL-008",
                format!(
                    "section {} references unknown visual scene {}",
                    section.section_id, visual_intent
                ),
            ));
        }
    }

    if plan.constraint_set.allowed_patch_scopes.is_empty() {
        diagnostics.push(Diagnostic::warning(
            "VAL-009",
            "constraint_set should declare at least one allowed patch scope",
        ));
    }

    for required_tag in &plan.constraint_set.required_tags {
        for asset in &plan.asset_records {
            if !asset.tags.iter().any(|tag| tag == required_tag) {
                diagnostics.push(Diagnostic::error(
                    "VAL-010",
                    format!("asset {} is missing required tag {}", asset.asset_id, required_tag),
                ));
            }
        }
    }

    diagnostics
}

#[cfg(test)]
mod tests {
    use super::validate_plan;
    use vidodo_ir::PlanBundle;

    #[test]
    fn rejects_unknown_audio_asset() {
        let mut plan = PlanBundle::minimal("show-phase0");
        plan.audio_dsl.layers[0].asset_candidates = vec![String::from("missing-asset")];

        let diagnostics = validate_plan(&plan);

        assert!(diagnostics.iter().any(|diagnostic| diagnostic.code == "VAL-006"));
    }

    #[test]
    fn rejects_empty_show_id() {
        let plan = PlanBundle::minimal("");
        let diagnostics = validate_plan(&plan);
        assert!(diagnostics.iter().any(|d| d.code == "VAL-001"));
    }

    #[test]
    fn rejects_mismatched_show_ids() {
        let mut plan = PlanBundle::minimal("show-a");
        plan.audio_dsl.show_id = String::from("show-b");
        let diagnostics = validate_plan(&plan);
        assert!(diagnostics.iter().any(|d| d.code == "VAL-002"));
    }

    #[test]
    fn rejects_empty_sections() {
        let mut plan = PlanBundle::minimal("show-phase0");
        plan.set_plan.sections.clear();
        let diagnostics = validate_plan(&plan);
        assert!(diagnostics.iter().any(|d| d.code == "VAL-003"));
    }

    #[test]
    fn rejects_audio_layer_exceeding_max() {
        let mut plan = PlanBundle::minimal("show-phase0");
        plan.constraint_set.max_audio_layers = 0;
        let diagnostics = validate_plan(&plan);
        assert!(diagnostics.iter().any(|d| d.code == "VAL-004"));
    }

    #[test]
    fn warns_on_empty_patch_scopes() {
        let mut plan = PlanBundle::minimal("show-phase0");
        plan.constraint_set.allowed_patch_scopes.clear();
        let diagnostics = validate_plan(&plan);
        assert!(diagnostics.iter().any(|d| d.code == "VAL-009" && d.severity == "warning"));
    }

    #[test]
    fn rejects_audio_layer_referencing_unknown_section() {
        let mut plan = PlanBundle::minimal("show-phase0");
        plan.audio_dsl.layers[0].entry_rules.section_refs =
            vec![String::from("nonexistent-section")];
        let diagnostics = validate_plan(&plan);
        assert!(diagnostics.iter().any(|d| d.code == "VAL-007"));
    }

    #[test]
    fn accepts_valid_minimal_plan() {
        let plan = PlanBundle::minimal("show-phase0");
        let diagnostics = validate_plan(&plan);
        let errors: Vec<_> = diagnostics.iter().filter(|d| d.severity == "error").collect();
        assert!(errors.is_empty(), "expected no errors but got: {:?}", errors);
    }
}
