use vidodo_ir::{
    CompiledRevision, Diagnostic, LivePatchProposal, PatchDecision, PatchScope, TimelineEntry,
    TimelineScheduler,
};

pub fn check_patch(revision: &CompiledRevision, proposal: &LivePatchProposal) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    if proposal.base_revision != revision.revision {
        diagnostics.push(Diagnostic::error(
            "PAT-001",
            format!(
                "patch base_revision {} does not match active revision {}",
                proposal.base_revision, revision.revision
            ),
        ));
    }

    if proposal.patch_class != "local_content" {
        diagnostics
            .push(Diagnostic::error("PAT-002", "Phase 0 only supports local_content patches"));
    }

    if !revision
        .constraint_set
        .allowed_patch_scopes
        .iter()
        .any(|scope| scope == &proposal.scope.window)
    {
        diagnostics.push(Diagnostic::error(
            "PAT-003",
            format!(
                "patch window {} is not allowed by the active constraint set",
                proposal.scope.window
            ),
        ));
    }

    if proposal.scope.from_bar > proposal.scope.to_bar
        || proposal.scope.to_bar > revision.final_bar()
    {
        diagnostics.push(Diagnostic::error(
            "PAT-004",
            "patch scope is outside the active revision timeline",
        ));
    }

    for change in &proposal.changes {
        if change.op != "replace_asset" {
            diagnostics.push(Diagnostic::error(
                "PAT-005",
                format!("unsupported patch operation {}", change.op),
            ));
        }

        let layer_exists =
            revision.audio_dsl.layers.iter().any(|layer| layer.layer_id == change.target);
        if !layer_exists {
            diagnostics.push(Diagnostic::error(
                "PAT-006",
                format!("patch target {} does not match any audio layer", change.target),
            ));
        }

        let replacement_asset =
            revision.asset_records.iter().find(|asset| asset.asset_id == change.to);
        if replacement_asset.is_none() {
            diagnostics.push(Diagnostic::error(
                "PAT-007",
                format!(
                    "replacement asset {} does not exist in the active asset registry",
                    change.to
                ),
            ));
        } else if let Some(asset) = replacement_asset {
            let ready =
                matches!(asset.readiness.as_deref(), Some("live_candidate") | Some("warmed"));
            let warm = matches!(asset.warm_status.as_deref(), Some("warmed"));
            if !(ready && warm) {
                diagnostics.push(Diagnostic::error(
                    "PAT-008",
                    format!("replacement asset {} is not warmed and live-ready", change.to),
                ));
            }
        }

        let matching_action_exists =
            revision.performance_ir.performance_actions.iter().any(|action| {
                action.layer_id == change.target
                    && action.target_asset_id.as_deref() == Some(change.from.as_str())
                    && action.musical_time.bar >= proposal.scope.from_bar
                    && action.musical_time.bar <= proposal.scope.to_bar
            });
        if !matching_action_exists {
            diagnostics.push(Diagnostic::error(
                "PAT-009",
                format!(
                    "patch change {} -> {} does not match any performance action inside the requested scope",
                    change.from, change.to
                ),
            ));
        }
    }

    diagnostics
}

pub fn apply_patch(
    revision: &CompiledRevision,
    proposal: &LivePatchProposal,
) -> Result<CompiledRevision, Vec<Diagnostic>> {
    let diagnostics = check_patch(revision, proposal);
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let mut patched = revision.clone();
    patched.base_revision = revision.revision;
    patched.revision = revision.revision + 1;
    patched.compile_run_id = format!("patch-{}-rev-{}", proposal.patch_id, patched.revision);

    for change in &proposal.changes {
        for action in &mut patched.performance_ir.performance_actions {
            if action.layer_id == change.target
                && action.target_asset_id.as_deref() == Some(change.from.as_str())
                && action.musical_time.bar >= proposal.scope.from_bar
                && action.musical_time.bar <= proposal.scope.to_bar
            {
                action.target_asset_id = Some(change.to.clone());
            }
        }

        if let Some(layer) =
            patched.audio_dsl.layers.iter_mut().find(|layer| layer.layer_id == change.target)
            && let Some(first_candidate) = layer.asset_candidates.first_mut()
        {
            *first_candidate = change.to.clone();
        }
    }

    let decision = PatchDecision {
        patch_id: proposal.patch_id.clone(),
        base_revision: revision.revision,
        candidate_revision: patched.revision,
        decision: String::from("applied"),
        window: proposal.scope.window.clone(),
        scope: PatchScope {
            from_bar: proposal.scope.from_bar,
            to_bar: proposal.scope.to_bar,
            window: proposal.scope.window.clone(),
        },
        fallback_revision: proposal.fallback_revision,
        reasons: vec![String::from("local content patch accepted")],
    };

    patched.patch_history.push(decision.clone());
    patched.timeline.push(TimelineEntry {
        r#type: String::from("timeline_entry"),
        id: format!("timeline-patch-{}", proposal.patch_id),
        show_id: patched.show_id.clone(),
        revision: patched.revision,
        channel: String::from("patch"),
        target_ref: proposal.patch_id.clone(),
        effective_window: vidodo_ir::EffectiveWindow {
            from_bar: proposal.scope.from_bar,
            to_bar: proposal.scope.to_bar,
        },
        scheduler: TimelineScheduler {
            lookahead_ms: 100,
            priority: 100,
            conflict_group: format!("patch-{}", proposal.patch_id),
        },
        guards: std::collections::BTreeMap::new(),
    });
    patched.timeline.sort_by(|left, right| {
        left.effective_window
            .from_bar
            .cmp(&right.effective_window.from_bar)
            .then(left.scheduler.priority.cmp(&right.scheduler.priority))
            .then(left.id.cmp(&right.id))
    });

    Ok(patched)
}

pub fn rollback_patch(
    revision: &CompiledRevision,
    patch_id: &str,
) -> Result<PatchDecision, Box<Diagnostic>> {
    rollback_patch_with_reason(revision, patch_id, "manual rollback requested")
}

/// Trigger a deferred rollback due to an anomaly detected during a run.
///
/// The deferred rollback produces a `PatchDecision` with
/// `decision = "deferred_rollback"` and the supplied anomaly reason.
/// The caller is responsible for recording the decision in the trace bundle.
pub fn deferred_rollback(
    revision: &CompiledRevision,
    patch_id: &str,
    anomaly: &str,
) -> Result<PatchDecision, Box<Diagnostic>> {
    let Some(existing) =
        revision.patch_history.iter().find(|decision| decision.patch_id == patch_id)
    else {
        return Err(Box::new(Diagnostic::error(
            "PAT-010",
            format!("patch {} was not found in revision {}", patch_id, revision.revision),
        )));
    };

    Ok(PatchDecision {
        patch_id: existing.patch_id.clone(),
        base_revision: revision.revision,
        candidate_revision: existing.fallback_revision,
        decision: String::from("deferred_rollback"),
        window: existing.window.clone(),
        scope: existing.scope.clone(),
        fallback_revision: existing.fallback_revision,
        reasons: vec![format!("deferred rollback: {anomaly}")],
    })
}

fn rollback_patch_with_reason(
    revision: &CompiledRevision,
    patch_id: &str,
    reason: &str,
) -> Result<PatchDecision, Box<Diagnostic>> {
    let Some(existing) =
        revision.patch_history.iter().find(|decision| decision.patch_id == patch_id)
    else {
        return Err(Box::new(Diagnostic::error(
            "PAT-010",
            format!("patch {} was not found in revision {}", patch_id, revision.revision),
        )));
    };

    Ok(PatchDecision {
        patch_id: existing.patch_id.clone(),
        base_revision: revision.revision,
        candidate_revision: existing.fallback_revision,
        decision: String::from("rolled_back"),
        window: existing.window.clone(),
        scope: existing.scope.clone(),
        fallback_revision: existing.fallback_revision,
        reasons: vec![String::from(reason)],
    })
}

#[cfg(test)]
mod tests {
    use super::{apply_patch, check_patch, deferred_rollback, rollback_patch};
    use vidodo_compiler::compile_plan;
    use vidodo_ir::{LivePatchProposal, PatchChange, PatchScope, PlanBundle};

    fn minimal_proposal(patch_id: &str, base_revision: u64) -> LivePatchProposal {
        LivePatchProposal {
            patch_id: String::from(patch_id),
            submitted_by: Some(String::from("tests")),
            patch_class: String::from("local_content"),
            base_revision,
            scope: PatchScope {
                from_bar: 9,
                to_bar: 16,
                window: String::from("next_phrase_boundary"),
            },
            intent: std::collections::BTreeMap::new(),
            changes: vec![PatchChange {
                op: String::from("replace_asset"),
                target: String::from("texture-bed"),
                from: String::from("audio.loop.pad-a"),
                to: String::from("audio.loop.pad-b"),
            }],
            fallback_revision: 1,
        }
    }

    #[test]
    fn applies_a_local_content_patch() {
        let compiled =
            compile_plan(&PlanBundle::minimal("show-phase0")).expect("plan should compile");
        let proposal = minimal_proposal("patch-phase0-pad-swap", 1);

        assert!(check_patch(&compiled, &proposal).is_empty());

        let patched = apply_patch(&compiled, &proposal).expect("patch should apply");
        assert_eq!(patched.revision, 2);
        assert!(
            patched
                .performance_ir
                .performance_actions
                .iter()
                .any(|action| action.target_asset_id.as_deref() == Some("audio.loop.pad-b"))
        );
    }

    #[test]
    fn rejects_base_revision_mismatch() {
        let compiled =
            compile_plan(&PlanBundle::minimal("show-phase0")).expect("plan should compile");
        let proposal = minimal_proposal("patch-wrong-rev", 999);
        let diagnostics = check_patch(&compiled, &proposal);
        assert!(diagnostics.iter().any(|d| d.code == "PAT-001"));
    }

    #[test]
    fn rejects_non_local_content_patch_class() {
        let compiled =
            compile_plan(&PlanBundle::minimal("show-phase0")).expect("plan should compile");
        let mut proposal = minimal_proposal("patch-structural", 1);
        proposal.patch_class = String::from("structural");
        let diagnostics = check_patch(&compiled, &proposal);
        assert!(diagnostics.iter().any(|d| d.code == "PAT-002"));
    }

    #[test]
    fn rejects_scope_outside_timeline() {
        let compiled =
            compile_plan(&PlanBundle::minimal("show-phase0")).expect("plan should compile");
        let mut proposal = minimal_proposal("patch-out-of-range", 1);
        proposal.scope =
            PatchScope { from_bar: 200, to_bar: 300, window: String::from("next_phrase_boundary") };
        let diagnostics = check_patch(&compiled, &proposal);
        assert!(diagnostics.iter().any(|d| d.code == "PAT-004"));
    }

    #[test]
    fn rejects_unknown_replacement_asset() {
        let compiled =
            compile_plan(&PlanBundle::minimal("show-phase0")).expect("plan should compile");
        let mut proposal = minimal_proposal("patch-unknown-asset", 1);
        proposal.changes = vec![PatchChange {
            op: String::from("replace_asset"),
            target: String::from("texture-bed"),
            from: String::from("audio.loop.pad-a"),
            to: String::from("audio.loop.nonexistent"),
        }];
        let diagnostics = check_patch(&compiled, &proposal);
        assert!(diagnostics.iter().any(|d| d.code == "PAT-007"));
    }

    #[test]
    fn rollback_fails_for_unknown_patch_id() {
        let compiled =
            compile_plan(&PlanBundle::minimal("show-phase0")).expect("plan should compile");
        let result = rollback_patch(&compiled, "nonexistent-patch");
        assert!(result.is_err());
        let diagnostic = result.unwrap_err();
        assert_eq!(diagnostic.code, "PAT-010");
    }

    #[test]
    fn rollback_restores_fallback_revision() {
        let compiled =
            compile_plan(&PlanBundle::minimal("show-phase0")).expect("plan should compile");
        let proposal = minimal_proposal("patch-phase0-pad-swap", 1);
        let patched = apply_patch(&compiled, &proposal).expect("patch should apply");
        let rollback =
            rollback_patch(&patched, "patch-phase0-pad-swap").expect("rollback should succeed");
        assert_eq!(rollback.decision, "rolled_back");
        assert_eq!(rollback.fallback_revision, 1);
    }

    #[test]
    fn deferred_rollback_on_anomaly() {
        let compiled =
            compile_plan(&PlanBundle::minimal("show-phase0")).expect("plan should compile");
        let proposal = minimal_proposal("patch-phase0-pad-swap", 1);
        let patched = apply_patch(&compiled, &proposal).expect("patch should apply");

        let decision =
            deferred_rollback(&patched, "patch-phase0-pad-swap", "resource_overload: GPU > 90%")
                .expect("deferred rollback should succeed");
        assert_eq!(decision.decision, "deferred_rollback");
        assert_eq!(decision.candidate_revision, 1); // restored to fallback
        assert!(decision.reasons[0].contains("resource_overload"));
    }

    #[test]
    fn deferred_rollback_unknown_patch_fails() {
        let compiled =
            compile_plan(&PlanBundle::minimal("show-phase0")).expect("plan should compile");
        let result = deferred_rollback(&compiled, "no-such-patch", "anomaly");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, "PAT-010");
    }
}
