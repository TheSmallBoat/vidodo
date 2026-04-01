use serde::Serialize;
use vidodo_ir::CompiledPlan;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TraceBundleManifest {
    pub run_id: String,
    pub revision: u64,
    pub event_count: usize,
}

pub fn manifest_from_plan(
    run_id: impl Into<String>,
    compiled: &CompiledPlan,
) -> TraceBundleManifest {
    TraceBundleManifest {
        run_id: run_id.into(),
        revision: compiled.revision,
        event_count: compiled.timeline.len(),
    }
}
