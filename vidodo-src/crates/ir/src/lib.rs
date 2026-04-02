use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceInfo {
    pub kind: String,
    pub submitted_by: String,
    pub submission_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    pub plan_id: String,
    pub compiler_run_id: String,
    pub parent_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectHeader {
    pub id: String,
    pub r#type: String,
    pub version: String,
    pub revision: u64,
    pub schema: String,
    pub source: SourceInfo,
    pub provenance: Provenance,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub annotations: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticTarget {
    pub object_type: String,
    pub object_id: String,
    pub field: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub code: String,
    pub namespace: String,
    pub severity: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<DiagnosticTarget>,
    pub retryable: bool,
    #[serde(default)]
    pub details: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

impl Diagnostic {
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            namespace: String::from("validator"),
            severity: String::from("error"),
            message: message.into(),
            target: None,
            retryable: false,
            details: BTreeMap::new(),
            suggestion: None,
        }
    }

    pub fn warning(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            namespace: String::from("validator"),
            severity: String::from("warning"),
            message: message.into(),
            target: None,
            retryable: false,
            details: BTreeMap::new(),
            suggestion: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResponseEnvelope<T> {
    pub status: String,
    pub capability: String,
    pub request_id: String,
    pub data: T,
    pub diagnostics: Vec<Diagnostic>,
    pub artifacts: Vec<String>,
    pub next_actions: Vec<String>,
}

impl<T> ResponseEnvelope<T> {
    pub fn new(
        status: impl Into<String>,
        capability: impl Into<String>,
        request_id: impl Into<String>,
        data: T,
        diagnostics: Vec<Diagnostic>,
        artifacts: Vec<String>,
        next_actions: Vec<String>,
    ) -> Self {
        Self {
            status: status.into(),
            capability: capability.into(),
            request_id: request_id.into(),
            data,
            diagnostics,
            artifacts,
            next_actions,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MusicalTime {
    pub beat: f64,
    pub bar: u32,
    pub beat_in_bar: f64,
    pub phrase: u32,
    pub section: String,
    pub tempo: f64,
    pub time_signature: [u32; 2],
}

impl MusicalTime {
    pub fn at_bar(bar: u32, phrase: u32, section: impl Into<String>, tempo: f64) -> Self {
        let beats_per_bar = 4.0;
        Self {
            beat: ((bar.saturating_sub(1)) as f64 * beats_per_bar) + 1.0,
            bar,
            beat_in_bar: 1.0,
            phrase,
            section: section.into(),
            tempo,
            time_signature: [4, 4],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetPlanGoal {
    pub intent: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_target_sec: Option<u32>,
    #[serde(default)]
    pub style_tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanSection {
    pub section_id: String,
    pub length_bars: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub energy_target: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub density_target: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visual_intent: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetPlan {
    pub r#type: String,
    pub id: String,
    pub show_id: String,
    pub mode: String,
    pub goal: SetPlanGoal,
    pub asset_pool_refs: Vec<String>,
    pub sections: Vec<PlanSection>,
    pub constraints_ref: String,
    pub delivery: BTreeMap<String, bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntryRules {
    #[serde(default)]
    pub section_refs: Vec<String>,
    pub quantize: String,
    pub max_simultaneous: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioAutomation {
    pub param: String,
    pub curve: String,
    #[serde(rename = "from")]
    pub from_value: f64,
    #[serde(rename = "to")]
    pub to_value: f64,
    pub duration_beats: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioLayer {
    pub layer_id: String,
    pub role: String,
    pub source_strategy: String,
    #[serde(default)]
    pub asset_candidates: Vec<String>,
    pub entry_rules: EntryRules,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_backend_hint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route_group_ref: Option<String>,
    #[serde(default)]
    pub automation: Vec<AudioAutomation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioDsl {
    pub r#type: String,
    pub id: String,
    pub show_id: String,
    pub layers: Vec<AudioLayer>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisualScene {
    pub scene_id: String,
    pub program_ref: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_backend: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub view_group_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_topology_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub calibration_profile_ref: Option<String>,
    #[serde(default)]
    pub inputs: BTreeMap<String, String>,
    #[serde(default)]
    pub semantic_binding: BTreeMap<String, String>,
    #[serde(default)]
    pub uniform_defaults: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisualDsl {
    pub r#type: String,
    pub id: String,
    pub show_id: String,
    pub scenes: Vec<VisualScene>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConstraintSet {
    pub locked_sections: Vec<String>,
    pub max_audio_layers: u32,
    pub max_gpu_peak: f64,
    #[serde(default)]
    pub allow_hard_cut: bool,
    pub allowed_patch_scopes: Vec<String>,
    #[serde(default)]
    pub banned_assets: Vec<String>,
    #[serde(default)]
    pub required_tags: Vec<String>,
    #[serde(default)]
    pub delivery_requirements: BTreeMap<String, bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetRecord {
    pub asset_id: String,
    pub asset_kind: String,
    pub content_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_locator: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normalized_locator: Option<String>,
    pub status: String,
    #[serde(default)]
    pub analysis_refs: Vec<String>,
    #[serde(default)]
    pub derived_refs: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warm_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub readiness: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IngestionCandidate {
    pub candidate_id: String,
    pub path: String,
    pub declared_kind: String,
    pub size_bytes: u64,
    pub modified_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IngestionRun {
    pub ingestion_run_id: String,
    pub source: String,
    pub mode: String,
    pub status: String,
    pub started_at: u64,
    pub completed_at: u64,
    pub discovered: u32,
    pub published: u32,
    pub reused: u32,
    pub failed: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisJob {
    pub analysis_job_id: String,
    pub asset_id: String,
    pub analyzer: String,
    pub analyzer_version: String,
    pub params_hash: String,
    pub status: String,
    pub cache_key: String,
    pub result_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisCacheEntry {
    pub cache_key: String,
    pub asset_id: String,
    pub analyzer: String,
    pub analyzer_version: String,
    pub input_fingerprint: String,
    pub dependency_fingerprint: String,
    pub created_at: u64,
    pub status: String,
    pub payload_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioProbeSummary {
    pub container: String,
    pub codec: String,
    pub sample_rate_hz: u32,
    pub channel_count: u16,
    pub bits_per_sample: u16,
    pub frame_count: u64,
    pub duration_ms: u64,
    pub peak_level_dbfs_tenths: i32,
    pub rms_level_dbfs_tenths: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BeatTrackAnalysis {
    pub asset_id: String,
    pub analyzer: String,
    pub analyzer_version: String,
    pub probe: AudioProbeSummary,
    pub estimated_tempo_bpm: u32,
    pub downbeat_bar: u32,
    pub estimated_bars: u32,
    pub transient_count: u32,
    pub source_size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SectionBoundary {
    pub start_bar: u32,
    pub end_bar: u32,
    pub label: String,
    pub confidence: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SectionSegmentationAnalysis {
    pub asset_id: String,
    pub analyzer: String,
    pub analyzer_version: String,
    pub probe: AudioProbeSummary,
    pub sections: Vec<SectionBoundary>,
    pub source_size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetIngestReport {
    pub run: IngestionRun,
    #[serde(default)]
    pub candidates: Vec<IngestionCandidate>,
    #[serde(default)]
    pub assets: Vec<AssetRecord>,
    #[serde(default)]
    pub analysis_jobs: Vec<AnalysisJob>,
    #[serde(default)]
    pub analysis_entries: Vec<AnalysisCacheEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanBundle {
    pub show_id: String,
    pub base_revision: u64,
    pub set_plan: SetPlan,
    pub audio_dsl: AudioDsl,
    pub visual_dsl: VisualDsl,
    pub constraint_set: ConstraintSet,
    pub asset_records: Vec<AssetRecord>,
}

impl PlanBundle {
    pub fn minimal(show_id: impl Into<String>) -> Self {
        let show_id = show_id.into();
        let set_plan = SetPlan {
            r#type: String::from("set_plan"),
            id: String::from("set-phase0-main"),
            show_id: show_id.clone(),
            mode: String::from("live"),
            goal: SetPlanGoal {
                intent: String::from("phase0_unique_mainline"),
                duration_target_sec: Some(64),
                style_tags: vec![String::from("deterministic"), String::from("fixture-driven")],
            },
            asset_pool_refs: vec![String::from("pool-minimal")],
            sections: vec![
                PlanSection {
                    section_id: String::from("intro"),
                    length_bars: 8,
                    energy_target: Some(0.2),
                    density_target: Some(0.2),
                    visual_intent: Some(String::from("scene_intro")),
                },
                PlanSection {
                    section_id: String::from("drop"),
                    length_bars: 8,
                    energy_target: Some(0.7),
                    density_target: Some(0.6),
                    visual_intent: Some(String::from("scene_drop")),
                },
            ],
            constraints_ref: String::from("constraint-phase0-main"),
            delivery: BTreeMap::from([
                (String::from("render_bundle"), false),
                (String::from("trace_bundle"), true),
                (String::from("evaluation"), false),
            ]),
        };

        let audio_dsl = AudioDsl {
            r#type: String::from("audio_dsl"),
            id: String::from("audio-phase0-main"),
            show_id: show_id.clone(),
            layers: vec![
                AudioLayer {
                    layer_id: String::from("rhythm-main"),
                    role: String::from("rhythm"),
                    source_strategy: String::from("fixed_asset"),
                    asset_candidates: vec![String::from("audio.loop.kick-a")],
                    entry_rules: EntryRules {
                        section_refs: vec![String::from("intro"), String::from("drop")],
                        quantize: String::from("bar"),
                        max_simultaneous: 1,
                    },
                    output_backend_hint: Some(String::from("fake_audio_backend")),
                    route_group_ref: Some(String::from("stereo-main")),
                    automation: vec![AudioAutomation {
                        param: String::from("gain_db"),
                        curve: String::from("linear"),
                        from_value: -6.0,
                        to_value: -3.0,
                        duration_beats: 8,
                    }],
                },
                AudioLayer {
                    layer_id: String::from("texture-bed"),
                    role: String::from("texture"),
                    source_strategy: String::from("fixed_asset"),
                    asset_candidates: vec![String::from("audio.loop.pad-a")],
                    entry_rules: EntryRules {
                        section_refs: vec![String::from("drop")],
                        quantize: String::from("bar"),
                        max_simultaneous: 1,
                    },
                    output_backend_hint: Some(String::from("fake_audio_backend")),
                    route_group_ref: Some(String::from("stereo-main")),
                    automation: Vec::new(),
                },
            ],
        };

        let visual_dsl = VisualDsl {
            r#type: String::from("visual_dsl"),
            id: String::from("visual-phase0-main"),
            show_id: show_id.clone(),
            scenes: vec![
                VisualScene {
                    scene_id: String::from("scene_intro"),
                    program_ref: String::from("glsl/scene-intro"),
                    output_backend: Some(String::from("fake_visual_backend")),
                    view_group_ref: None,
                    display_topology_ref: Some(String::from("flat-display-a")),
                    calibration_profile_ref: None,
                    inputs: BTreeMap::new(),
                    semantic_binding: BTreeMap::new(),
                    uniform_defaults: BTreeMap::from([(
                        String::from("tint"),
                        String::from("blue"),
                    )]),
                },
                VisualScene {
                    scene_id: String::from("scene_drop"),
                    program_ref: String::from("glsl/scene-drop"),
                    output_backend: Some(String::from("fake_visual_backend")),
                    view_group_ref: None,
                    display_topology_ref: Some(String::from("flat-display-a")),
                    calibration_profile_ref: None,
                    inputs: BTreeMap::new(),
                    semantic_binding: BTreeMap::new(),
                    uniform_defaults: BTreeMap::from([(
                        String::from("tint"),
                        String::from("amber"),
                    )]),
                },
            ],
        };

        let constraint_set = ConstraintSet {
            locked_sections: vec![String::from("intro")],
            max_audio_layers: 4,
            max_gpu_peak: 0.5,
            allow_hard_cut: false,
            allowed_patch_scopes: vec![String::from("next_phrase_boundary")],
            banned_assets: Vec::new(),
            required_tags: vec![String::from("fixture")],
            delivery_requirements: BTreeMap::from([(String::from("trace_bundle"), true)]),
        };

        let asset_records = vec![
            AssetRecord {
                asset_id: String::from("audio.loop.kick-a"),
                asset_kind: String::from("audio_loop"),
                content_hash: String::from("sha256:asset-kick-a"),
                raw_locator: None,
                normalized_locator: Some(String::from("fixture://audio/loop-kick-a.wav")),
                status: String::from("published"),
                analysis_refs: Vec::new(),
                derived_refs: Vec::new(),
                tags: vec![String::from("fixture"), String::from("rhythm")],
                warm_status: Some(String::from("warmed")),
                readiness: Some(String::from("live_candidate")),
            },
            AssetRecord {
                asset_id: String::from("audio.loop.pad-a"),
                asset_kind: String::from("audio_loop"),
                content_hash: String::from("sha256:asset-pad-a"),
                raw_locator: None,
                normalized_locator: Some(String::from("fixture://audio/loop-pad-a.wav")),
                status: String::from("published"),
                analysis_refs: Vec::new(),
                derived_refs: Vec::new(),
                tags: vec![String::from("fixture"), String::from("texture")],
                warm_status: Some(String::from("warmed")),
                readiness: Some(String::from("live_candidate")),
            },
            AssetRecord {
                asset_id: String::from("audio.loop.pad-b"),
                asset_kind: String::from("audio_loop"),
                content_hash: String::from("sha256:asset-pad-b"),
                raw_locator: None,
                normalized_locator: Some(String::from("fixture://audio/loop-pad-b.wav")),
                status: String::from("published"),
                analysis_refs: Vec::new(),
                derived_refs: Vec::new(),
                tags: vec![
                    String::from("fixture"),
                    String::from("texture"),
                    String::from("patch-ready"),
                ],
                warm_status: Some(String::from("warmed")),
                readiness: Some(String::from("live_candidate")),
            },
        ];

        Self {
            show_id,
            base_revision: 0,
            set_plan,
            audio_dsl,
            visual_dsl,
            constraint_set,
            asset_records,
        }
    }

    pub fn show_id(&self) -> &str {
        &self.show_id
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StructureSpan {
    pub start_bar: u32,
    pub end_bar: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StructureSection {
    pub section_id: String,
    pub order: usize,
    pub span: StructureSpan,
    #[serde(default)]
    pub targets: BTreeMap<String, String>,
    #[serde(default)]
    pub locks: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StructureIr {
    pub r#type: String,
    pub id: String,
    pub show_id: String,
    pub sections: Vec<StructureSection>,
    #[serde(default)]
    pub transitions: Vec<BTreeMap<String, String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PerformanceAction {
    pub action_id: String,
    pub layer_id: String,
    pub op: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_asset_id: Option<String>,
    pub musical_time: MusicalTime,
    pub duration_beats: u32,
    pub quantize: String,
    pub priority: i32,
    pub rollback_token: String,
    #[serde(default)]
    pub resource_hint: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_backend_hint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route_set_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PerformanceIr {
    pub performance_actions: Vec<PerformanceAction>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisualView {
    pub view_id: String,
    pub camera_id: String,
    pub display_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisualAction {
    pub visual_action_id: String,
    pub scene_id: String,
    pub program_ref: String,
    #[serde(default)]
    pub uniform_set: BTreeMap<String, String>,
    #[serde(default)]
    pub camera_state: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_backend: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub view_group_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_topology_ref: Option<String>,
    pub duration_beats: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blend_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu_cost_hint: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_scene_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisualIr {
    pub visual_actions: Vec<VisualAction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectiveWindow {
    pub from_bar: u32,
    pub to_bar: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimelineScheduler {
    pub lookahead_ms: u32,
    pub priority: i32,
    pub conflict_group: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimelineEntry {
    pub r#type: String,
    pub id: String,
    pub show_id: String,
    pub revision: u64,
    pub channel: String,
    pub target_ref: String,
    pub effective_window: EffectiveWindow,
    pub scheduler: TimelineScheduler,
    #[serde(default)]
    pub guards: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchScope {
    pub from_bar: u32,
    pub to_bar: u32,
    pub window: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchChange {
    pub op: String,
    pub target: String,
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LivePatchProposal {
    pub patch_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub submitted_by: Option<String>,
    pub patch_class: String,
    pub base_revision: u64,
    pub scope: PatchScope,
    #[serde(default)]
    pub intent: BTreeMap<String, String>,
    pub changes: Vec<PatchChange>,
    pub fallback_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchDecision {
    pub patch_id: String,
    pub base_revision: u64,
    pub candidate_revision: u64,
    pub decision: String,
    pub window: String,
    pub scope: PatchScope,
    pub fallback_revision: u64,
    #[serde(default)]
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventHeader {
    pub event_id: String,
    pub show_id: String,
    pub revision: u64,
    pub kind: String,
    pub source: String,
    pub musical_time: MusicalTime,
    pub scheduler_time_ms: u64,
    pub wallclock_hint_ms: u64,
    pub priority: i32,
    pub causation_id: String,
    pub replay_token: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimingEvent {
    pub phrase: u32,
    pub section: String,
    pub tempo: f64,
    pub downbeat: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bar: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub beat: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_signature: Option<[u32; 2]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transition_window_open: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioFilter {
    pub kind: String,
    pub cutoff_hz: f64,
    pub resonance: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioEvent {
    pub layer_id: String,
    pub op: String,
    pub output_backend: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route_set_ref: Option<String>,
    #[serde(default)]
    pub speaker_group: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gain_db: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_beats: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<AudioFilter>,
    #[serde(default)]
    pub automation: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_asset_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Viewport {
    pub width: u32,
    pub height: u32,
    pub x: u32,
    pub y: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisualViewport {
    pub view_id: String,
    pub camera_id: String,
    pub display_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub viewport: Option<Viewport>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisualEvent {
    pub scene_id: String,
    pub shader_program: String,
    pub output_backend: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub view_group: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_topology: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub calibration_profile: Option<String>,
    #[serde(default)]
    pub uniforms: BTreeMap<String, String>,
    #[serde(default)]
    pub views: Vec<VisualViewport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_beats: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blend: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchEvent {
    pub patch_id: String,
    pub scope: PatchScope,
    pub effective_revision: u64,
    pub fallback_revision: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LightingEvent {
    pub cue_set_id: String,
    pub source_ref: String,
    pub fixture_group: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intensity: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<[f64; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fade_beats: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "payload_type", content = "payload", rename_all = "snake_case")]
pub enum RuntimePayload {
    Timing(TimingEvent),
    Audio(AudioEvent),
    Visual(VisualEvent),
    Patch(PatchEvent),
    Lighting(LightingEvent),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendAck {
    pub backend: String,
    pub target: String,
    pub status: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventRecord {
    pub event_id: String,
    pub show_id: String,
    pub revision: u64,
    pub kind: String,
    pub phase: String,
    pub source: String,
    pub musical_time: MusicalTime,
    pub scheduler_time_ms: u64,
    pub wallclock_time_ms: u64,
    pub causation_id: String,
    pub payload: RuntimePayload,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ack: Option<BackendAck>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceManifest {
    pub trace_bundle_id: String,
    pub show_id: String,
    pub revision: u64,
    pub run_id: String,
    pub mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    pub status: String,
    pub input_refs: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_log_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evaluation_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patch_decisions_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_samples_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub export_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShowSemantic {
    pub energy: f64,
    pub density: f64,
    pub tension: f64,
    pub brightness: f64,
    pub motion: f64,
    pub intent: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShowTransition {
    pub state: String,
    pub from_scene: String,
    pub to_scene: String,
    pub window_open: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputBinding {
    pub backend_id: String,
    pub topology_ref: String,
    pub calibration_profile: String,
    pub active_group: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShowPatchState {
    pub allowed: bool,
    pub scope: String,
    pub locked_sections: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShowState {
    pub show_id: String,
    pub revision: u64,
    pub mode: String,
    pub time: MusicalTime,
    pub semantic: ShowSemantic,
    pub transition: ShowTransition,
    pub visual_output: OutputBinding,
    pub audio_output: OutputBinding,
    pub patch: ShowPatchState,
    #[serde(default)]
    pub adapter_plugins: BTreeMap<String, String>,
    #[serde(default)]
    pub resource_hubs: BTreeMap<String, String>,
    #[serde(default)]
    pub active_audio_layers: Vec<String>,
    pub active_visual_scene: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourceSample {
    pub sample_time_ms: u64,
    pub show_id: String,
    pub revision: u64,
    pub bar: u32,
    pub section: String,
    pub cpu: f64,
    pub gpu: f64,
    pub memory_mb: u32,
    pub audio_xruns: u32,
    pub video_dropped_frames: u32,
    pub active_scene: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportArtifactRecord {
    pub artifact_id: String,
    pub artifact_type: String,
    pub locator: String,
    pub content_hash: String,
    pub derived_from_run_id: String,
    pub revision: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_sec: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunSummary {
    pub show_id: String,
    pub revision: u64,
    pub starting_bar: u32,
    pub final_bar: u32,
    pub event_count: usize,
    pub final_section: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompiledRevision {
    pub show_id: String,
    pub revision: u64,
    pub base_revision: u64,
    pub compile_run_id: String,
    pub set_plan: SetPlan,
    pub audio_dsl: AudioDsl,
    pub visual_dsl: VisualDsl,
    pub constraint_set: ConstraintSet,
    pub asset_records: Vec<AssetRecord>,
    pub structure_ir: StructureIr,
    pub performance_ir: PerformanceIr,
    pub visual_ir: VisualIr,
    pub timeline: Vec<TimelineEntry>,
    #[serde(default)]
    pub patch_history: Vec<PatchDecision>,
}

impl CompiledRevision {
    pub fn final_bar(&self) -> u32 {
        self.structure_ir.sections.last().map_or(1, |section| section.span.end_bar)
    }

    pub fn section_for_bar(&self, bar: u32) -> Option<&StructureSection> {
        self.structure_ir
            .sections
            .iter()
            .find(|section| bar >= section.span.start_bar && bar <= section.span.end_bar)
    }
}

// ---------------------------------------------------------------------------
// Capability Layer types (Phase 1)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityDescriptor {
    pub capability: String,
    pub version: String,
    pub execution_mode: String,
    pub idempotency: String,
    pub authorization: Vec<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub input_schema: String,
    #[serde(default)]
    pub output_schema: String,
    #[serde(default)]
    pub target_service: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityRequest {
    pub request_id: String,
    pub capability: String,
    pub payload: serde_json::Value,
    #[serde(default)]
    pub actor: Option<ActorContext>,
    #[serde(default)]
    pub metadata: Option<RequestMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActorContext {
    pub actor_id: String,
    pub role: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestMetadata {
    pub source: String,
    #[serde(default)]
    pub trace_parent: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationTicket {
    pub operation_id: String,
    pub request_id: String,
    pub capability: String,
    pub state: String,
    pub started_at: u64,
    #[serde(default)]
    pub updated_at: Option<u64>,
    #[serde(default)]
    pub artifact_refs: Vec<String>,
}

// ---------------------------------------------------------------------------
// Lighting types (Phase 1 — WSJ-01)
// ---------------------------------------------------------------------------

/// A single lighting fixture endpoint, matching lighting-topology.v0.json fixture_endpoints items.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LightingFixture {
    pub fixture_id: String,
    pub role: String,
    pub device_ref: String,
    #[serde(default)]
    pub universe: Option<u32>,
    #[serde(default)]
    pub address: Option<u32>,
    #[serde(default)]
    pub position: Option<[f64; 3]>,
    #[serde(default)]
    pub orientation: Option<[f64; 3]>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub status: Option<String>,
}

/// A lighting topology — maps to lighting-topology.v0.json root.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LightingTopology {
    pub topology_id: String,
    pub backend: String,
    #[serde(default)]
    pub calibration_profile: Option<String>,
    pub fixture_endpoints: Vec<LightingFixture>,
}

/// A single cue entry within a cue set, matching cue-set.v0.json entries items.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LightingCue {
    pub source_ref: String,
    pub fixture_group: Vec<String>,
    #[serde(default)]
    pub intensity: Option<f64>,
    #[serde(default)]
    pub color: Option<[f64; 3]>,
    #[serde(default)]
    pub fade_beats: Option<f64>,
    #[serde(default)]
    pub motion_preset: Option<String>,
    #[serde(default)]
    pub policy: Option<String>,
}

/// A complete cue set bound to a topology — maps to cue-set.v0.json root.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CueSet {
    pub cue_set_id: String,
    pub topology_ref: String,
    pub entries: Vec<LightingCue>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lighting_fixture_serde_round_trip() {
        let fixture = LightingFixture {
            fixture_id: String::from("fx-01"),
            role: String::from("fill"),
            device_ref: String::from("node-a"),
            universe: Some(1),
            address: Some(42),
            position: Some([1.0, 2.0, 3.0]),
            orientation: Some([0.0, 90.0, 0.0]),
            capabilities: vec![String::from("dimmer"), String::from("rgb")],
            status: Some(String::from("available")),
        };
        let json = serde_json::to_string(&fixture).unwrap();
        let back: LightingFixture = serde_json::from_str(&json).unwrap();
        assert_eq!(fixture, back);
    }

    #[test]
    fn lighting_topology_from_schema_fixture() {
        let json = r#"{
            "topology_id": "lighting-grid-a",
            "backend": "spatial_lighting_matrix_backend",
            "fixture_endpoints": [{
                "fixture_id": "fx-up-left-03",
                "role": "upper_left_fill",
                "device_ref": "lighting-node-a"
            }]
        }"#;
        let topo: LightingTopology = serde_json::from_str(json).unwrap();
        assert_eq!(topo.topology_id, "lighting-grid-a");
        assert_eq!(topo.fixture_endpoints.len(), 1);
        assert_eq!(topo.fixture_endpoints[0].fixture_id, "fx-up-left-03");
        // round-trip
        let back: LightingTopology =
            serde_json::from_str(&serde_json::to_string(&topo).unwrap()).unwrap();
        assert_eq!(topo, back);
    }

    #[test]
    fn cue_set_from_schema_fixture() {
        let json = r#"{
            "cue_set_id": "cue-drop-a-v3",
            "topology_ref": "lighting-grid-a",
            "entries": [{
                "source_ref": "scene/drop_a",
                "fixture_group": ["fx-up-left-03"],
                "intensity": 0.8,
                "color": [1.0, 0.0, 0.5],
                "fade_beats": 2.0
            }]
        }"#;
        let cs: CueSet = serde_json::from_str(json).unwrap();
        assert_eq!(cs.cue_set_id, "cue-drop-a-v3");
        assert_eq!(cs.entries.len(), 1);
        assert_eq!(cs.entries[0].intensity, Some(0.8));
        assert_eq!(cs.entries[0].color, Some([1.0, 0.0, 0.5]));
        // round-trip
        let back: CueSet = serde_json::from_str(&serde_json::to_string(&cs).unwrap()).unwrap();
        assert_eq!(cs, back);
    }
}
