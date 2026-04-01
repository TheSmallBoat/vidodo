use vidodo_ir::{Diagnostic, PlanBundle};

pub fn validate_plan(plan: &PlanBundle) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    if plan.show_id.trim().is_empty() {
        diagnostics.push(Diagnostic {
            code: String::from("VAL-001"),
            message: String::from("show_id must not be empty"),
        });
    }

    if plan.goal.trim().is_empty() {
        diagnostics.push(Diagnostic {
            code: String::from("VAL-002"),
            message: String::from("goal must not be empty"),
        });
    }

    diagnostics
}

#[cfg(test)]
mod tests {
    use super::validate_plan;
    use vidodo_ir::PlanBundle;

    #[test]
    fn rejects_blank_show_id() {
        let plan = PlanBundle {
            show_id: String::from("  "),
            goal: String::from("smoke"),
            base_revision: 0,
        };

        let diagnostics = validate_plan(&plan);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "VAL-001");
    }
}
