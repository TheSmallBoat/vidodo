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
    #[serde(default)]
    pub lighting_topology: Option<LightingTopology>,
    #[serde(default)]
    pub cue_sets: Vec<CueSet>,
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

        let lighting_topology = Some(LightingTopology {
            topology_id: String::from("topo-phase0-minimal"),
            backend: String::from("fake_lighting_backend"),
            calibration_profile: None,
            fixture_endpoints: vec![
                LightingFixture {
                    fixture_id: String::from("fx-front-wash"),
                    role: String::from("wash"),
                    device_ref: String::from("node-a"),
                    universe: Some(1),
                    address: Some(1),
                    position: Some([0.0, 3.0, 0.0]),
                    orientation: None,
                    capabilities: vec![String::from("dimmer"), String::from("rgb")],
                    status: Some(String::from("available")),
                },
                LightingFixture {
                    fixture_id: String::from("fx-back-spot"),
                    role: String::from("spot"),
                    device_ref: String::from("node-b"),
                    universe: Some(1),
                    address: Some(10),
                    position: Some([0.0, 4.0, -2.0]),
                    orientation: None,
                    capabilities: vec![
                        String::from("dimmer"),
                        String::from("rgb"),
                        String::from("gobo"),
                    ],
                    status: Some(String::from("available")),
                },
            ],
        });

        let cue_sets = vec![CueSet {
            cue_set_id: String::from("cue-phase0-main"),
            topology_ref: String::from("topo-phase0-minimal"),
            entries: vec![LightingCue {
                source_ref: String::from("intro"),
                fixture_group: vec![String::from("fx-front-wash")],
                intensity: Some(0.8),
                color: Some([0.2, 0.4, 1.0]),
                fade_beats: Some(4.0),
                motion_preset: None,
                policy: Some(String::from("crossfade")),
            }],
        }];

        Self {
            show_id,
            base_revision: 0,
            set_plan,
            audio_dsl,
            visual_dsl,
            constraint_set,
            asset_records,
            lighting_topology,
            cue_sets,
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
    /// Beat positions (seconds) from audio analysis, if available.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub beat_map: Vec<f64>,
    /// Detected key from harmony analysis, if available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detected_key: Option<String>,
    /// Section boundaries from analysis, if available.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub section_boundaries: Vec<AnalysisSectionBoundary>,
}

/// A section boundary detected by audio analysis.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalysisSectionBoundary {
    pub start_sec: f64,
    pub end_sec: f64,
    #[serde(default)]
    pub label: String,
    pub confidence: f64,
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

/// Runtime event emitted when a backend degradation is detected.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DegradeEvent {
    pub degrade_id: String,
    pub mode: String,
    pub reason: String,
    #[serde(default)]
    pub affected_backends: Vec<String>,
    #[serde(default)]
    pub fallback_action: Option<String>,
}

// ---------------------------------------------------------------------------
// External control events (MIDI / OSC)
// ---------------------------------------------------------------------------

/// A MIDI Control Change message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MidiCC {
    pub channel: u8,
    pub cc: u8,
    pub value: u8,
}

/// A MIDI Note On / Off message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MidiNote {
    pub channel: u8,
    pub note: u8,
    pub velocity: u8,
    pub on: bool,
}

/// An Open Sound Control message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OscMessage {
    pub address: String,
    pub args: Vec<serde_json::Value>,
}

/// External control event from hardware or virtual controller.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "control_type", rename_all = "snake_case")]
pub enum ExternalControlEvent {
    MidiCc { source_id: String, midi_cc: MidiCC },
    MidiNote { source_id: String, midi_note: MidiNote },
    OscMessage { source_id: String, osc_message: OscMessage },
}

/// Binding record for an external control source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlBinding {
    pub source_id: String,
    pub protocol: String,
}

/// Trait for adapters that bridge external MIDI/OSC controllers into the runtime.
///
/// Implementors manage source bindings, poll for new events each tick, and
/// expose the list of currently active bindings.
pub trait ExternalControlAdapter {
    /// Register a control source.
    fn bind_source(&mut self, source_id: &str, protocol: &str) -> Result<(), String>;
    /// Unregister a control source.
    fn unbind_source(&mut self, source_id: &str) -> Result<(), String>;
    /// Poll for pending control events since the last call.
    fn poll_events(&mut self) -> Vec<ExternalControlEvent>;
    /// List currently active bindings.
    fn list_bindings(&self) -> Vec<ControlBinding>;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "payload_type", content = "payload", rename_all = "snake_case")]
pub enum RuntimePayload {
    Timing(TimingEvent),
    Audio(AudioEvent),
    Visual(VisualEvent),
    Patch(PatchEvent),
    Lighting(LightingEvent),
    Degrade(DegradeEvent),
    ExternalControl(ExternalControlEvent),
}

// ---------------------------------------------------------------------------
// Show templates and scene packs
// ---------------------------------------------------------------------------

/// A section within a show template.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateSectionRef {
    pub section_id: String,
    pub order: u32,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_bars: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scene_pack_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub energy_target: Option<f64>,
}

/// Default parameters for a show template.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateDefaultParams {
    pub tempo_bpm: f64,
    pub time_signature: [u32; 2],
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub style_tags: Vec<String>,
}

/// Reusable show template describing section layout and default parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShowTemplate {
    #[serde(rename = "type")]
    pub template_type: String,
    pub template_id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub mode: String,
    pub sections: Vec<TemplateSectionRef>,
    pub default_params: TemplateDefaultParams,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scene_pack_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

/// Transition strategy for scenes within a pack.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransitionStrategy {
    pub default_mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crossfade_beats: Option<f64>,
}

/// A single scene descriptor within a scene pack.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SceneDescriptor {
    pub scene_id: String,
    pub label: String,
    pub asset_refs: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visual_program_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub energy_range: Option<[f64; 2]>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

/// Bundled scene descriptors with asset references and transition strategy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScenePack {
    #[serde(rename = "type")]
    pub pack_type: String,
    pub pack_id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub scenes: Vec<SceneDescriptor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transition_strategy: Option<TransitionStrategy>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
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

impl ShowState {
    /// Build a minimal ShowState from a compiled revision for testing.
    pub fn default_for_test(compiled: &CompiledRevision) -> Self {
        Self {
            show_id: compiled.show_id.clone(),
            revision: compiled.revision,
            mode: compiled.set_plan.mode.clone(),
            time: MusicalTime::at_bar(1, 1, String::from("intro"), 128.0),
            semantic: ShowSemantic {
                energy: 0.5,
                density: 0.5,
                tension: 0.5,
                brightness: 0.5,
                motion: 0.5,
                intent: String::from("test"),
            },
            transition: ShowTransition {
                state: String::from("steady"),
                from_scene: String::from("scene_intro"),
                to_scene: String::from("scene_intro"),
                window_open: true,
            },
            visual_output: OutputBinding {
                backend_id: String::from("fake_visual"),
                topology_ref: String::from("flat-display-a"),
                calibration_profile: String::from("default"),
                active_group: String::from("scene_intro"),
            },
            audio_output: OutputBinding {
                backend_id: String::from("fake_audio"),
                topology_ref: String::from("stereo-main"),
                calibration_profile: String::from("default"),
                active_group: String::from("stereo-main"),
            },
            patch: ShowPatchState {
                allowed: true,
                scope: String::from("next_phrase_boundary"),
                locked_sections: Vec::new(),
            },
            adapter_plugins: BTreeMap::new(),
            resource_hubs: BTreeMap::new(),
            active_audio_layers: Vec::new(),
            active_visual_scene: String::from("scene_intro"),
        }
    }
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
    #[serde(default)]
    pub lighting_topology: Option<LightingTopology>,
    #[serde(default)]
    pub cue_sets: Vec<CueSet>,
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
// Phase 2 — Adapter Plugin Manifest
// ---------------------------------------------------------------------------

/// Health reporting contract for an adapter plugin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HealthContract {
    #[serde(default)]
    pub reports_ack: bool,
    #[serde(default)]
    pub reports_status: bool,
    #[serde(default)]
    pub supports_degrade_mode: bool,
}

/// Manifest for a hardware adapter plugin — maps to adapter-plugin-manifest.v0.json.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdapterPluginManifest {
    pub plugin_id: String,
    pub plugin_kind: String,
    pub backend_kind: String,
    pub version: String,
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub target_topology_types: Vec<String>,
    #[serde(default)]
    pub health_contract: Option<HealthContract>,
    #[serde(default)]
    pub status: Option<String>,
}

// ---------------------------------------------------------------------------
// Phase 2 — Resource Hub Descriptor
// ---------------------------------------------------------------------------

/// Compatibility constraints for a resource hub.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HubCompatibility {
    #[serde(default)]
    pub runtime: Vec<String>,
    #[serde(default)]
    pub backends: Vec<String>,
    #[serde(default)]
    pub schema_versions: Vec<String>,
}

/// Descriptor for an external resource hub — maps to resource-hub-descriptor.v0.json.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourceHubDescriptor {
    pub hub_id: String,
    pub resource_kind: String,
    pub version: String,
    pub locator: String,
    pub provides: Vec<String>,
    #[serde(default)]
    pub compatibility: Option<HubCompatibility>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

// ---------------------------------------------------------------------------
// Phase 2 — Deployment Objects
// ---------------------------------------------------------------------------

/// Describes a node participating in distributed device orchestration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DistributedNodeDescriptor {
    pub node_id: String,
    pub node_role: String,
    #[serde(default)]
    pub host_ref: Option<String>,
    #[serde(default)]
    pub plugin_refs: Vec<String>,
    #[serde(default)]
    pub assigned_topologies: Vec<String>,
    #[serde(default)]
    pub resource_hub_mounts: Vec<String>,
    #[serde(default)]
    pub transport_refs: Vec<String>,
    #[serde(default)]
    pub health_endpoint: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
}

/// Describes a concrete device deployment — single-node or multi-node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeploymentProfile {
    pub deployment_id: String,
    pub mode: String,
    pub node_refs: Vec<String>,
    #[serde(default)]
    pub transport_refs: Vec<String>,
    #[serde(default)]
    pub time_authority: Option<String>,
    #[serde(default)]
    pub resource_prewarm_policy: Option<String>,
    #[serde(default)]
    pub rollout_strategy: Option<String>,
    #[serde(default)]
    pub failure_policy: Option<String>,
    #[serde(default)]
    pub trace_policy: Option<String>,
}

/// Semantic transport specification between distributed nodes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransportContract {
    pub transport_id: String,
    pub bus_kind: String,
    pub protocol: String,
    #[serde(default)]
    pub topology: Option<String>,
    #[serde(default)]
    pub ordering: Option<String>,
    #[serde(default)]
    pub delivery_guarantee: Option<String>,
    #[serde(default)]
    pub latency_budget_ms: Option<u64>,
    #[serde(default)]
    pub reconnect_policy: Option<String>,
    #[serde(default)]
    pub security_profile: Option<String>,
}

// ---------------------------------------------------------------------------
// Phase 2 — Health & Degradation
// ---------------------------------------------------------------------------

/// A snapshot of a backend's health status.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BackendHealthSnapshot {
    pub backend_ref: String,
    pub plugin_ref: String,
    pub status: String,
    pub timestamp: String,
    #[serde(default)]
    pub latency_ms: Option<f64>,
    #[serde(default)]
    pub error_count: Option<u64>,
    #[serde(default)]
    pub last_ack_lag_ms: Option<f64>,
    #[serde(default)]
    pub degrade_reason: Option<String>,
}

/// A degradation mode that can be applied to a backend.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DegradeMode {
    pub mode: String,
    pub reason: String,
    #[serde(default)]
    pub affected_backends: Vec<String>,
    #[serde(default)]
    pub fallback_action: Option<String>,
}

// ---------------------------------------------------------------------------
// Phase 3 — BackendAdapter Trait & Executable Types (WSO-01, WSO-02)
// ---------------------------------------------------------------------------

/// Description returned by [`BackendAdapter::describe_backend`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BackendDescription {
    pub plugin_id: String,
    pub backend_kind: String,
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub topology_types: Vec<String>,
    #[serde(default)]
    pub status: String,
}

/// Topology reference passed to [`BackendAdapter::prepare_backend`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "topology_kind", rename_all = "snake_case")]
pub enum BackendTopology {
    /// Visual display topology — flat or spatial multi-view.
    Visual {
        topology_ref: String,
        #[serde(default)]
        calibration_profile: Option<String>,
        #[serde(default)]
        display_endpoints: Vec<String>,
    },
    /// Audio output topology — system or spatial speaker matrix.
    Audio {
        topology_ref: String,
        #[serde(default)]
        calibration_profile: Option<String>,
        #[serde(default)]
        speaker_endpoints: Vec<String>,
    },
    /// Lighting fixture topology — fixture bus or spatial lighting matrix.
    Lighting {
        topology_ref: String,
        #[serde(default)]
        calibration_profile: Option<String>,
        #[serde(default)]
        fixture_endpoints: Vec<String>,
    },
}

/// Channel-specific execution payload consumed by [`BackendAdapter::execute_payload`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "payload_kind", rename_all = "snake_case")]
pub enum ExecutablePayload {
    /// Audio execution command — play, stop, crossfade, etc.
    Audio {
        layer_id: String,
        op: String,
        #[serde(default)]
        target_asset_id: Option<String>,
        #[serde(default)]
        gain_db: Option<f64>,
        #[serde(default)]
        duration_beats: Option<u32>,
        #[serde(default)]
        route_set_ref: Option<String>,
        #[serde(default)]
        speaker_group: Vec<String>,
    },
    /// Visual execution command — scene switch, uniform update, etc.
    Visual {
        scene_id: String,
        shader_program: String,
        #[serde(default)]
        uniforms: BTreeMap<String, String>,
        #[serde(default)]
        duration_beats: Option<u32>,
        #[serde(default)]
        blend: Option<String>,
        #[serde(default)]
        view_group: Option<String>,
    },
    /// Lighting execution command — cue apply, fade, etc.
    Lighting {
        cue_set_id: String,
        source_ref: String,
        #[serde(default)]
        fixture_group: Vec<String>,
        #[serde(default)]
        intensity: Option<f64>,
        #[serde(default)]
        color: Option<[f64; 3]>,
        #[serde(default)]
        fade_beats: Option<f64>,
    },
}

/// Result of [`BackendAdapter::collect_backend_status`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BackendStatus {
    pub plugin_id: String,
    pub status: String,
    #[serde(default)]
    pub latency_ms: Option<f64>,
    #[serde(default)]
    pub error_count: Option<u64>,
    #[serde(default)]
    pub last_ack_lag_ms: Option<f64>,
    #[serde(default)]
    pub detail: Option<String>,
}

/// Structured result from an analysis adapter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalysisResult {
    pub analyzer_id: String,
    pub asset_id: String,
    pub status: String,
    pub metrics: BTreeMap<String, serde_json::Value>,
}

/// Trait for third-party analysis adapters (audio, visual, etc.).
///
/// Analysis adapters inspect assets and return structured results that can
/// be stored in the analysis cache.
pub trait AnalysisAdapter {
    /// Return the unique identifier for this analyzer.
    fn analyzer_id(&self) -> &str;
    /// Check whether the analyzer is ready.
    fn ready(&self) -> bool;
    /// Run analysis on an asset identified by `asset_id` at the given `path`.
    fn analyze(&self, asset_id: &str, path: &std::path::Path) -> Result<AnalysisResult, String>;
}

/// Unified backend adapter trait — Doc 30 §4.2 seven-method protocol.
///
/// All three backend types (audio, visual, lighting) implement this trait
/// to maintain a single protocol boundary with the core scheduling system.
pub trait BackendAdapter {
    /// Return a description of this backend: kind, capabilities, supported topologies.
    fn describe_backend(&self) -> BackendDescription;

    /// Prepare the backend for execution with a given topology and calibration.
    fn prepare_backend(&mut self, topology: &BackendTopology) -> Result<(), String>;

    /// Push the current show state to this backend.
    fn apply_show_state(&mut self, show_state: &ShowState) -> Result<(), String>;

    /// Execute a channel-specific payload (audio/visual/lighting command).
    fn execute_payload(&mut self, payload: &ExecutablePayload) -> Result<BackendAck, String>;

    /// Collect the current health/status of this backend.
    fn collect_backend_status(&self) -> BackendStatus;

    /// Apply a degradation mode (e.g., reduced resolution, mute, blackout).
    fn apply_degrade_mode(&mut self, mode: &DegradeMode) -> Result<(), String>;

    /// Gracefully shut down this backend, releasing resources.
    fn shutdown_backend(&mut self) -> Result<(), String>;
}

// ---------------------------------------------------------------------------
// Phase 3 — Authorization Policy Types (WSQ-01)
// ---------------------------------------------------------------------------

/// A single policy rule mapping an actor role to allowed/denied capabilities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyRule {
    pub role: String,
    pub effect: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub patch_classes: Vec<String>,
    #[serde(default)]
    pub conditions: BTreeMap<String, String>,
}

/// Authorization policy — a set of rules evaluated in order.
///
/// Covers Doc 08 §4 three actor roles:
/// - `human_operator`: full access
/// - `external_agent`: restricted (no emergency patch)
/// - `auto_recovery`: only degrade/rollback actions
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorizationPolicy {
    pub policy_id: String,
    pub version: String,
    pub default_effect: String,
    pub rules: Vec<PolicyRule>,
}

// ---------------------------------------------------------------------------
// Phase 4 — Contract Hardening: Rollback Checkpoint
// ---------------------------------------------------------------------------

/// State of a resource handle during patch rollback lifecycle.
///
/// Per Doc 08 §4, resource handles must be explicitly tracked through
/// rollback to ensure old revision resources are released and new/warming
/// resources are managed deterministically.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceHandleState {
    /// Resource is actively used by the current revision.
    Active,
    /// Resource has been released after rollback.
    Released,
    /// Resource is pre-warming for a pending revision.
    Warming,
}

/// A snapshot of resource handle states at rollback time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourceHandleSnapshot {
    pub resource_id: String,
    pub state: ResourceHandleState,
    #[serde(default)]
    pub backend_kind: Option<String>,
}

/// Checkpoint captured during patch rollback, per Doc 08 §4 rollback strategy.
///
/// Preserves enough state to verify that resources were released, backends
/// were re-aligned, and show state was restored to a consistent baseline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RollbackCheckpoint {
    pub patch_id: String,
    pub base_revision: u64,
    pub rollback_target_revision: u64,
    pub resource_handles: Vec<ResourceHandleSnapshot>,
    pub backend_snapshots: Vec<BackendHealthSnapshot>,
    pub show_state_snapshot: ShowState,
    pub reason: String,
    pub timestamp: String,
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

    // --- Phase 2 serde round-trip tests ---

    #[test]
    fn adapter_plugin_manifest_serde_round_trip() {
        let manifest = AdapterPluginManifest {
            plugin_id: String::from("visual-led-wall"),
            plugin_kind: String::from("visual_output"),
            backend_kind: String::from("led_matrix"),
            version: String::from("0.1.0"),
            capabilities: vec![String::from("scene_switch"), String::from("fade")],
            target_topology_types: vec![String::from("display_topology")],
            health_contract: Some(HealthContract {
                reports_ack: true,
                reports_status: true,
                supports_degrade_mode: true,
            }),
            status: Some(String::from("ready")),
        };
        let json = serde_json::to_string(&manifest).unwrap();
        let back: AdapterPluginManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(manifest, back);
    }

    #[test]
    fn resource_hub_descriptor_serde_round_trip() {
        let hub = ResourceHubDescriptor {
            hub_id: String::from("audio-pack-standard"),
            resource_kind: String::from("audio_asset_hub"),
            version: String::from("1.0.0"),
            locator: String::from("file:///hubs/audio-standard"),
            provides: vec![String::from("wav"), String::from("aif")],
            compatibility: Some(HubCompatibility {
                runtime: vec![String::from("vidodo-runtime-0.1")],
                backends: vec![],
                schema_versions: vec![String::from("v0")],
            }),
            status: Some(String::from("available")),
            tags: vec![String::from("production")],
        };
        let json = serde_json::to_string(&hub).unwrap();
        let back: ResourceHubDescriptor = serde_json::from_str(&json).unwrap();
        assert_eq!(hub, back);
    }

    #[test]
    fn distributed_node_descriptor_serde_round_trip() {
        let node = DistributedNodeDescriptor {
            node_id: String::from("node-visual-a"),
            node_role: String::from("visual_renderer"),
            host_ref: Some(String::from("192.168.1.10")),
            plugin_refs: vec![String::from("visual-led-wall")],
            assigned_topologies: vec![String::from("display-wall-left")],
            resource_hub_mounts: vec![String::from("glsl-hub-v2")],
            transport_refs: vec![String::from("control-bus-ws")],
            health_endpoint: Some(String::from("http://192.168.1.10:7401/health")),
            status: Some(String::from("online")),
        };
        let json = serde_json::to_string(&node).unwrap();
        let back: DistributedNodeDescriptor = serde_json::from_str(&json).unwrap();
        assert_eq!(node, back);
    }

    #[test]
    fn deployment_profile_serde_round_trip() {
        let profile = DeploymentProfile {
            deployment_id: String::from("show-2026-wall"),
            mode: String::from("multi_node"),
            node_refs: vec![String::from("node-visual-a"), String::from("node-audio-b")],
            transport_refs: vec![String::from("control-bus-ws")],
            time_authority: Some(String::from("node-audio-b")),
            resource_prewarm_policy: Some(String::from("eager")),
            rollout_strategy: Some(String::from("rolling")),
            failure_policy: Some(String::from("isolate_and_degrade")),
            trace_policy: Some(String::from("full")),
        };
        let json = serde_json::to_string(&profile).unwrap();
        let back: DeploymentProfile = serde_json::from_str(&json).unwrap();
        assert_eq!(profile, back);
    }

    #[test]
    fn transport_contract_serde_round_trip() {
        let contract = TransportContract {
            transport_id: String::from("control-bus-ws"),
            bus_kind: String::from("control"),
            protocol: String::from("websocket"),
            topology: Some(String::from("star")),
            ordering: Some(String::from("ordered")),
            delivery_guarantee: Some(String::from("at_least_once")),
            latency_budget_ms: Some(50),
            reconnect_policy: Some(String::from("exponential_backoff")),
            security_profile: Some(String::from("tls")),
        };
        let json = serde_json::to_string(&contract).unwrap();
        let back: TransportContract = serde_json::from_str(&json).unwrap();
        assert_eq!(contract, back);
    }

    #[test]
    fn backend_health_snapshot_serde_round_trip() {
        let snap = BackendHealthSnapshot {
            backend_ref: String::from("display-wall-left"),
            plugin_ref: String::from("visual-led-wall"),
            status: String::from("degraded"),
            timestamp: String::from("2026-04-03T12:00:00Z"),
            latency_ms: Some(12.5),
            error_count: Some(3),
            last_ack_lag_ms: Some(8.2),
            degrade_reason: Some(String::from("gpu_thermal_throttle")),
        };
        let json = serde_json::to_string(&snap).unwrap();
        let back: BackendHealthSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(snap, back);
    }

    #[test]
    fn degrade_mode_serde_round_trip() {
        let mode = DegradeMode {
            mode: String::from("reduced_resolution"),
            reason: String::from("gpu_thermal_throttle"),
            affected_backends: vec![String::from("display-wall-left")],
            fallback_action: Some(String::from("lower_fps")),
        };
        let json = serde_json::to_string(&mode).unwrap();
        let back: DegradeMode = serde_json::from_str(&json).unwrap();
        assert_eq!(mode, back);
    }

    // --- Phase 3 serde round-trip tests ---

    #[test]
    fn backend_topology_visual_serde_round_trip() {
        let topo = BackendTopology::Visual {
            topology_ref: String::from("display-wall-left"),
            calibration_profile: Some(String::from("hdr-calibrated")),
            display_endpoints: vec![String::from("screen-1"), String::from("screen-2")],
        };
        let json = serde_json::to_string(&topo).unwrap();
        let back: BackendTopology = serde_json::from_str(&json).unwrap();
        assert_eq!(topo, back);
    }

    #[test]
    fn backend_topology_audio_serde_round_trip() {
        let topo = BackendTopology::Audio {
            topology_ref: String::from("speaker-matrix-a"),
            calibration_profile: None,
            speaker_endpoints: vec![String::from("spk-front-l"), String::from("spk-front-r")],
        };
        let json = serde_json::to_string(&topo).unwrap();
        let back: BackendTopology = serde_json::from_str(&json).unwrap();
        assert_eq!(topo, back);
    }

    #[test]
    fn backend_topology_lighting_serde_round_trip() {
        let topo = BackendTopology::Lighting {
            topology_ref: String::from("lighting-grid-a"),
            calibration_profile: Some(String::from("venue-preset")),
            fixture_endpoints: vec![String::from("fx-01"), String::from("fx-02")],
        };
        let json = serde_json::to_string(&topo).unwrap();
        let back: BackendTopology = serde_json::from_str(&json).unwrap();
        assert_eq!(topo, back);
    }

    #[test]
    fn executable_payload_audio_serde_round_trip() {
        let payload = ExecutablePayload::Audio {
            layer_id: String::from("layer-drums"),
            op: String::from("play"),
            target_asset_id: Some(String::from("asset-kick-01")),
            gain_db: Some(-6.0),
            duration_beats: Some(4),
            route_set_ref: None,
            speaker_group: vec![String::from("front")],
        };
        let json = serde_json::to_string(&payload).unwrap();
        let back: ExecutablePayload = serde_json::from_str(&json).unwrap();
        assert_eq!(payload, back);
    }

    #[test]
    fn executable_payload_visual_serde_round_trip() {
        let payload = ExecutablePayload::Visual {
            scene_id: String::from("scene-intro"),
            shader_program: String::from("glsl/wave.frag"),
            uniforms: BTreeMap::from([(String::from("u_time"), String::from("0.0"))]),
            duration_beats: Some(8),
            blend: Some(String::from("crossfade")),
            view_group: None,
        };
        let json = serde_json::to_string(&payload).unwrap();
        let back: ExecutablePayload = serde_json::from_str(&json).unwrap();
        assert_eq!(payload, back);
    }

    #[test]
    fn executable_payload_lighting_serde_round_trip() {
        let payload = ExecutablePayload::Lighting {
            cue_set_id: String::from("cue-drop-a"),
            source_ref: String::from("scene/drop"),
            fixture_group: vec![String::from("fx-01")],
            intensity: Some(0.9),
            color: Some([1.0, 0.0, 0.5]),
            fade_beats: Some(2.0),
        };
        let json = serde_json::to_string(&payload).unwrap();
        let back: ExecutablePayload = serde_json::from_str(&json).unwrap();
        assert_eq!(payload, back);
    }

    #[test]
    fn backend_description_serde_round_trip() {
        let desc = BackendDescription {
            plugin_id: String::from("visual-led-wall"),
            backend_kind: String::from("visual"),
            capabilities: vec![String::from("scene_switch")],
            topology_types: vec![String::from("display_topology")],
            status: String::from("ready"),
        };
        let json = serde_json::to_string(&desc).unwrap();
        let back: BackendDescription = serde_json::from_str(&json).unwrap();
        assert_eq!(desc, back);
    }

    #[test]
    fn backend_status_serde_round_trip() {
        let status = BackendStatus {
            plugin_id: String::from("audio-system"),
            status: String::from("healthy"),
            latency_ms: Some(5.2),
            error_count: Some(0),
            last_ack_lag_ms: None,
            detail: None,
        };
        let json = serde_json::to_string(&status).unwrap();
        let back: BackendStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(status, back);
    }

    #[test]
    fn authorization_policy_serde_round_trip() {
        let policy = AuthorizationPolicy {
            policy_id: String::from("default-policy"),
            version: String::from("1.0"),
            default_effect: String::from("deny"),
            rules: vec![
                PolicyRule {
                    role: String::from("human_operator"),
                    effect: String::from("allow"),
                    capabilities: vec![String::from("*")],
                    patch_classes: vec![],
                    conditions: BTreeMap::new(),
                },
                PolicyRule {
                    role: String::from("external_agent"),
                    effect: String::from("allow"),
                    capabilities: vec![
                        String::from("plan.*"),
                        String::from("compile.*"),
                        String::from("asset.*"),
                    ],
                    patch_classes: vec![String::from("param"), String::from("local_content")],
                    conditions: BTreeMap::new(),
                },
                PolicyRule {
                    role: String::from("auto_recovery"),
                    effect: String::from("allow"),
                    capabilities: vec![
                        String::from("patch.rollback"),
                        String::from("system.degrade"),
                    ],
                    patch_classes: vec![String::from("emergency")],
                    conditions: BTreeMap::from([(
                        String::from("trigger"),
                        String::from("health_monitor"),
                    )]),
                },
            ],
        };
        let json = serde_json::to_string(&policy).unwrap();
        let back: AuthorizationPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(policy, back);
    }

    #[test]
    fn policy_rule_serde_round_trip() {
        let rule = PolicyRule {
            role: String::from("external_agent"),
            effect: String::from("deny"),
            capabilities: vec![String::from("patch.submit")],
            patch_classes: vec![String::from("emergency"), String::from("structural")],
            conditions: BTreeMap::from([(String::from("require_approval"), String::from("true"))]),
        };
        let json = serde_json::to_string(&rule).unwrap();
        let back: PolicyRule = serde_json::from_str(&json).unwrap();
        assert_eq!(rule, back);
    }

    #[test]
    fn resource_handle_state_serde_round_trip() {
        for state in [
            ResourceHandleState::Active,
            ResourceHandleState::Released,
            ResourceHandleState::Warming,
        ] {
            let json = serde_json::to_string(&state).unwrap();
            let back: ResourceHandleState = serde_json::from_str(&json).unwrap();
            assert_eq!(state, back);
        }
    }

    #[test]
    fn rollback_checkpoint_serde_round_trip() {
        let checkpoint = RollbackCheckpoint {
            patch_id: String::from("patch-001"),
            base_revision: 2,
            rollback_target_revision: 1,
            resource_handles: vec![
                ResourceHandleSnapshot {
                    resource_id: String::from("audio-stem-a"),
                    state: ResourceHandleState::Released,
                    backend_kind: Some(String::from("audio_output")),
                },
                ResourceHandleSnapshot {
                    resource_id: String::from("visual-scene-b"),
                    state: ResourceHandleState::Active,
                    backend_kind: Some(String::from("visual_output")),
                },
            ],
            backend_snapshots: vec![BackendHealthSnapshot {
                backend_ref: String::from("audio-ref"),
                plugin_ref: String::from("audio-plugin-01"),
                status: String::from("healthy"),
                timestamp: String::from("2026-04-06T12:00:00Z"),
                latency_ms: Some(2.5),
                error_count: Some(0),
                last_ack_lag_ms: None,
                degrade_reason: None,
            }],
            show_state_snapshot: ShowState {
                show_id: String::from("show-001"),
                revision: 1,
                mode: String::from("live"),
                time: MusicalTime {
                    beat: 1.0,
                    bar: 1,
                    beat_in_bar: 1.0,
                    phrase: 1,
                    section: String::from("intro"),
                    tempo: 120.0,
                    time_signature: [4, 4],
                },
                semantic: ShowSemantic {
                    energy: 0.5,
                    density: 0.3,
                    tension: 0.2,
                    brightness: 0.7,
                    motion: 0.1,
                    intent: String::from("ambient"),
                },
                transition: ShowTransition {
                    state: String::from("idle"),
                    from_scene: String::new(),
                    to_scene: String::new(),
                    window_open: false,
                },
                visual_output: OutputBinding {
                    backend_id: String::from("vis-ref"),
                    topology_ref: String::from("topo-v"),
                    calibration_profile: String::new(),
                    active_group: String::from("main"),
                },
                audio_output: OutputBinding {
                    backend_id: String::from("aud-ref"),
                    topology_ref: String::from("topo-a"),
                    calibration_profile: String::new(),
                    active_group: String::from("main"),
                },
                patch: ShowPatchState {
                    allowed: true,
                    scope: String::from("global"),
                    locked_sections: vec![],
                },
                adapter_plugins: BTreeMap::new(),
                resource_hubs: BTreeMap::new(),
                active_audio_layers: vec![],
                active_visual_scene: String::from("intro"),
            },
            reason: String::from("anomaly: xrun threshold exceeded"),
            timestamp: String::from("2026-04-06T12:00:00Z"),
        };
        let json = serde_json::to_string(&checkpoint).unwrap();
        let back: RollbackCheckpoint = serde_json::from_str(&json).unwrap();
        assert_eq!(checkpoint, back);
    }

    #[test]
    fn external_control_midi_cc_serde_round_trip() {
        let event = ExternalControlEvent::MidiCc {
            source_id: String::from("nanokontrol-1"),
            midi_cc: MidiCC { channel: 0, cc: 74, value: 100 },
        };
        let json = serde_json::to_string(&event).unwrap();
        let back: ExternalControlEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, back);

        let payload = RuntimePayload::ExternalControl(event.clone());
        let json2 = serde_json::to_string(&payload).unwrap();
        let back2: RuntimePayload = serde_json::from_str(&json2).unwrap();
        assert_eq!(payload, back2);
    }

    #[test]
    fn external_control_osc_serde_round_trip() {
        let event = ExternalControlEvent::OscMessage {
            source_id: String::from("touchosc-ipad"),
            osc_message: OscMessage {
                address: String::from("/scene/tempo"),
                args: vec![
                    serde_json::Value::from(128.0),
                    serde_json::Value::from("half-time"),
                    serde_json::Value::from(true),
                ],
            },
        };
        let json = serde_json::to_string(&event).unwrap();
        let back: ExternalControlEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, back);
    }

    #[test]
    fn show_template_serde_round_trip() {
        let template = ShowTemplate {
            template_type: String::from("show_template"),
            template_id: String::from("tpl-edm-01"),
            name: String::from("EDM Festival Set"),
            description: Some(String::from("Four-section energy arc")),
            mode: String::from("live"),
            sections: vec![
                TemplateSectionRef {
                    section_id: String::from("sec-intro"),
                    order: 0,
                    label: String::from("Intro"),
                    duration_bars: Some(16),
                    scene_pack_ref: Some(String::from("pack-ambient-01")),
                    energy_target: Some(0.3),
                },
                TemplateSectionRef {
                    section_id: String::from("sec-drop"),
                    order: 1,
                    label: String::from("Drop"),
                    duration_bars: Some(32),
                    scene_pack_ref: None,
                    energy_target: Some(1.0),
                },
            ],
            default_params: TemplateDefaultParams {
                tempo_bpm: 128.0,
                time_signature: [4, 4],
                style_tags: vec![String::from("edm")],
            },
            scene_pack_refs: vec![String::from("pack-ambient-01")],
            tags: vec![String::from("festival")],
        };
        let json = serde_json::to_string(&template).unwrap();
        let back: ShowTemplate = serde_json::from_str(&json).unwrap();
        assert_eq!(template, back);
    }

    #[test]
    fn scene_pack_serde_round_trip() {
        let pack = ScenePack {
            pack_type: String::from("scene_pack"),
            pack_id: String::from("pack-ambient-01"),
            name: String::from("Ambient Visuals"),
            description: Some(String::from("Low-energy scenes")),
            scenes: vec![
                SceneDescriptor {
                    scene_id: String::from("scene-fog"),
                    label: String::from("Fog Drift"),
                    asset_refs: vec![String::from("visual.program.fog-drift")],
                    visual_program_ref: Some(String::from("shader-fog-drift-v1")),
                    energy_range: Some([0.0, 0.4]),
                    tags: vec![String::from("ambient")],
                },
                SceneDescriptor {
                    scene_id: String::from("scene-star"),
                    label: String::from("Starfield"),
                    asset_refs: vec![String::from("visual.program.starfield")],
                    visual_program_ref: None,
                    energy_range: None,
                    tags: vec![],
                },
            ],
            transition_strategy: Some(TransitionStrategy {
                default_mode: String::from("crossfade"),
                crossfade_beats: Some(4.0),
            }),
            tags: vec![String::from("ambient")],
        };
        let json = serde_json::to_string(&pack).unwrap();
        let back: ScenePack = serde_json::from_str(&json).unwrap();
        assert_eq!(pack, back);
    }

    #[test]
    fn scene_descriptor_serde_round_trip() {
        let scene = SceneDescriptor {
            scene_id: String::from("scene-laser"),
            label: String::from("Laser Grid"),
            asset_refs: vec![
                String::from("visual.program.laser-grid"),
                String::from("audio.loop.bass-a"),
            ],
            visual_program_ref: Some(String::from("shader-laser-v2")),
            energy_range: Some([0.7, 1.0]),
            tags: vec![String::from("high-energy"), String::from("laser")],
        };
        let json = serde_json::to_string(&scene).unwrap();
        let back: SceneDescriptor = serde_json::from_str(&json).unwrap();
        assert_eq!(scene, back);
    }
}
