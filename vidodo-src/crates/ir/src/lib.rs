use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanBundle {
    pub show_id: String,
    pub goal: String,
    pub base_revision: u64,
}

impl PlanBundle {
    pub fn minimal(show_id: impl Into<String>) -> Self {
        Self {
            show_id: show_id.into(),
            goal: String::from("phase0-minimal-loop"),
            base_revision: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimelineEntry {
    pub bar: u32,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledPlan {
    pub show_id: String,
    pub revision: u64,
    pub timeline: Vec<TimelineEntry>,
}
