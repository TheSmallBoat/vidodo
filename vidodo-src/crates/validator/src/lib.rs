use std::collections::BTreeSet;

use vidodo_ir::{
    DeploymentProfile, Diagnostic, DistributedNodeDescriptor, PlanBundle, TransportContract,
};

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

// ---------------------------------------------------------------------------
// Deployment validation (WSL-03: DEP-001 ~ DEP-003)
// ---------------------------------------------------------------------------

/// Validate a deployment profile against its constituent nodes and transports.
///
/// Returns diagnostics for:
/// - DEP-001: orphan nodes (referenced but have no transport contracts)
/// - DEP-002: undefined transport contracts
/// - DEP-003: duplicate or undefined node IDs
pub fn validate_deployment(
    profile: &DeploymentProfile,
    nodes: &[DistributedNodeDescriptor],
    transports: &[TransportContract],
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    let known_node_ids: BTreeSet<&str> = nodes.iter().map(|n| n.node_id.as_str()).collect();
    let known_transport_ids: BTreeSet<&str> =
        transports.iter().map(|t| t.transport_id.as_str()).collect();

    // DEP-003: check for duplicate node IDs and undefined node refs
    let mut seen_nodes = BTreeSet::new();
    for node in nodes {
        if !seen_nodes.insert(node.node_id.as_str()) {
            diagnostics.push(Diagnostic::error(
                "DEP-003",
                format!("duplicate node ID '{}'", node.node_id),
            ));
        }
    }
    for node_ref in &profile.node_refs {
        if !known_node_ids.contains(node_ref.as_str()) {
            diagnostics.push(Diagnostic::error(
                "DEP-003",
                format!("node_ref '{}' not found in provided nodes", node_ref),
            ));
        }
    }

    // DEP-002: check that all transport_refs in the profile exist
    for transport_ref in &profile.transport_refs {
        if !known_transport_ids.contains(transport_ref.as_str()) {
            diagnostics.push(Diagnostic::error(
                "DEP-002",
                format!("transport contract '{}' referenced but not defined", transport_ref),
            ));
        }
    }

    // DEP-001: orphan node detection — nodes referenced in profile with no transport_refs
    let profile_node_set: BTreeSet<&str> = profile.node_refs.iter().map(|r| r.as_str()).collect();
    for node in nodes {
        if profile_node_set.contains(node.node_id.as_str()) && node.transport_refs.is_empty() {
            diagnostics.push(Diagnostic::error(
                "DEP-001",
                format!(
                    "node '{}' has no transport contracts; cannot participate in deployment",
                    node.node_id,
                ),
            ));
        }
    }

    diagnostics
}

#[cfg(test)]
mod tests {
    use super::{validate_deployment, validate_plan};
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

    // -----------------------------------------------------------------------
    // Deployment validation tests
    // -----------------------------------------------------------------------

    use vidodo_ir::{DeploymentProfile, DistributedNodeDescriptor, TransportContract};

    fn sample_node(id: &str, transports: &[&str]) -> DistributedNodeDescriptor {
        DistributedNodeDescriptor {
            node_id: id.to_string(),
            node_role: String::from("audio_renderer"),
            host_ref: None,
            plugin_refs: Vec::new(),
            assigned_topologies: Vec::new(),
            resource_hub_mounts: Vec::new(),
            transport_refs: transports.iter().map(|s| s.to_string()).collect(),
            health_endpoint: None,
            status: None,
        }
    }

    fn sample_transport(id: &str) -> TransportContract {
        TransportContract {
            transport_id: id.to_string(),
            bus_kind: String::from("control"),
            protocol: String::from("nats"),
            topology: None,
            ordering: None,
            delivery_guarantee: None,
            latency_budget_ms: None,
            reconnect_policy: None,
            security_profile: None,
        }
    }

    fn sample_profile(nodes: &[&str], transports: &[&str]) -> DeploymentProfile {
        DeploymentProfile {
            deployment_id: String::from("dep-1"),
            mode: String::from("multi_node"),
            node_refs: nodes.iter().map(|s| s.to_string()).collect(),
            transport_refs: transports.iter().map(|s| s.to_string()).collect(),
            time_authority: None,
            resource_prewarm_policy: None,
            rollout_strategy: None,
            failure_policy: None,
            trace_policy: None,
        }
    }

    #[test]
    fn dep001_detects_orphan_node_without_transport() {
        let nodes = vec![
            sample_node("n1", &["t1"]),
            sample_node("n2", &[]), // orphan — no transports
        ];
        let transports = vec![sample_transport("t1")];
        let profile = sample_profile(&["n1", "n2"], &["t1"]);

        let diagnostics = validate_deployment(&profile, &nodes, &transports);
        assert!(
            diagnostics.iter().any(|d| d.code == "DEP-001"),
            "expected DEP-001: {:?}",
            diagnostics
        );
    }

    #[test]
    fn dep002_detects_undefined_transport() {
        let nodes = vec![sample_node("n1", &["t1"])];
        let transports = vec![sample_transport("t1")];
        let profile = sample_profile(&["n1"], &["t1", "t_missing"]);

        let diagnostics = validate_deployment(&profile, &nodes, &transports);
        assert!(
            diagnostics.iter().any(|d| d.code == "DEP-002"),
            "expected DEP-002: {:?}",
            diagnostics
        );
    }

    #[test]
    fn dep003_detects_duplicate_node_id() {
        let nodes = vec![
            sample_node("n1", &["t1"]),
            sample_node("n1", &["t1"]), // duplicate
        ];
        let transports = vec![sample_transport("t1")];
        let profile = sample_profile(&["n1"], &["t1"]);

        let diagnostics = validate_deployment(&profile, &nodes, &transports);
        assert!(
            diagnostics.iter().any(|d| d.code == "DEP-003"),
            "expected DEP-003: {:?}",
            diagnostics
        );
    }

    #[test]
    fn dep003_detects_undefined_node_ref() {
        let nodes = vec![sample_node("n1", &["t1"])];
        let transports = vec![sample_transport("t1")];
        let profile = sample_profile(&["n1", "n_missing"], &["t1"]);

        let diagnostics = validate_deployment(&profile, &nodes, &transports);
        assert!(
            diagnostics.iter().any(|d| d.code == "DEP-003"),
            "expected DEP-003: {:?}",
            diagnostics
        );
    }

    #[test]
    fn valid_deployment_produces_no_errors() {
        let nodes = vec![sample_node("n1", &["t1"]), sample_node("n2", &["t1"])];
        let transports = vec![sample_transport("t1")];
        let profile = sample_profile(&["n1", "n2"], &["t1"]);

        let diagnostics = validate_deployment(&profile, &nodes, &transports);
        let errors: Vec<_> = diagnostics.iter().filter(|d| d.severity == "error").collect();
        assert!(errors.is_empty(), "expected no errors but got: {:?}", errors);
    }
}
