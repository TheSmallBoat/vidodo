use vidodo_ir::{CompiledPlan, Diagnostic, PlanBundle, TimelineEntry};
use vidodo_validator::validate_plan;

pub fn compile_plan(plan: &PlanBundle) -> Result<CompiledPlan, Vec<Diagnostic>> {
    let diagnostics = validate_plan(plan);
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    Ok(CompiledPlan {
        show_id: plan.show_id.clone(),
        revision: plan.base_revision + 1,
        timeline: vec![
            TimelineEntry { bar: 1, label: String::from("bootstrap-intro") },
            TimelineEntry { bar: 9, label: String::from("steady-section") },
        ],
    })
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

        assert_eq!(first, second);
        assert_eq!(first.revision, 1);
        assert_eq!(first.timeline.len(), 2);
    }
}
