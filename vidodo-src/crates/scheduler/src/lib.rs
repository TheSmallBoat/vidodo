use serde::Serialize;
use vidodo_ir::CompiledPlan;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RunSummary {
    pub show_id: String,
    pub starting_bar: u32,
    pub event_count: usize,
}

pub fn prepare_run_summary(compiled: &CompiledPlan) -> RunSummary {
    RunSummary {
        show_id: compiled.show_id.clone(),
        starting_bar: compiled.timeline.first().map_or(1, |entry| entry.bar),
        event_count: compiled.timeline.len(),
    }
}
