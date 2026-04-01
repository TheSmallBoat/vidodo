# JSON Schema 完整清单与版本策略

日期：2026-04-01
状态：Draft
前置阅读：05-DSL与IR字段定义草案.md，04-音频与视觉运行时事件协议与数据结构.md，07-追踪回放与评估数据模型.md，10-CLI命令清单与MCP工具Schema.md，23-错误码与Diagnostics字典.md，../04-测试与工程执行/24-工作任务卡与开发里程碑.md

## 1. 文档目的

本文盘点系统中所有需要正式 JSON Schema 定义的对象，规划 schema 文件的命名、版本策略与实施优先级。

本文是 Workstream A 的执行蓝图，对应任务卡 WSA-01 至 WSA-14。

## 2. Schema 管理原则

### 2.1 Schema 是工件面一等公民

Schema 不是代码的附属文档，而是系统工件边界的形式化定义。
所有模块的输入输出都必须可被 schema 校验。

### 2.2 命名规范

```text
schemas/{category}/{object-name}.v{major}.json
```

示例：

- `schemas/planning/set-plan.v0.json`
- `schemas/ir/structure-ir.v0.json`
- `schemas/runtime/audio-event.v0.json`

### 2.3 版本策略

- 初始版本一律使用 `v0`，表示 MVP 草案阶段
- 非兼容字段变更必须升主版本号（v0 → v1）
- 兼容性扩展（新增可选字段）不升版本，但更新 schema 文件内的 description
- 旧版本 schema 文件保留在仓库中，不删除
- Rust 类型中的 `schema` 字段值必须与 schema 文件 `$id` 一致

### 2.4 引用与复用

- 通用结构通过 `$ref` 引用 `schemas/common/` 下的共享定义
- 不在每个 schema 文件中重复定义 musical_time、diagnostic 等通用结构
- 用 `$defs` 放置当前文件私有定义，用 `$ref` 引用 common 定义

### 2.5 Schema 元信息

每个 schema 文件应包含：

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "av.{category}.{object-name}.v0",
  "title": "...",
  "description": "...",
  "type": "object",
  "required": [...],
  "properties": {...}
}
```

## 3. 完整 Schema 清单

### 3.1 common —— 通用共享定义

| 文件名 | $id | 内容 | 来源文档 | 任务卡 |
| --- | --- | --- | --- | --- |
| object-header.v0.json | av.common.object-header.v0 | 所有一等对象的统一头：id、type、version、revision、schema、source、provenance、labels、annotations | 05 §3.1 | WSA-01 |
| musical-time.v0.json | av.common.musical-time.v0 | beat、bar、beat_in_bar、phrase、section、tempo、time_signature | 04 §4.2 | WSA-01 |
| response-envelope.v0.json | av.common.response-envelope.v0 | status、capability、request_id、data、diagnostics、artifacts、next_actions | 10 §2.2 | WSA-12 |
| diagnostic.v0.json | av.common.diagnostic.v0 | code、namespace、severity、message、target、retryable、details、suggestion | 23 §3.1 | WSA-12 |
| source-info.v0.json | av.common.source-info.v0 | kind、submitted_by、submission_id | 05 §3.1 | WSA-01 |
| provenance.v0.json | av.common.provenance.v0 | plan_id、compiler_run_id、parent_ids | 05 §3.1 | WSA-01 |

### 3.2 planning —— 外部规划者提交的对象

| 文件名 | $id | 内容 | 来源文档 | 任务卡 |
| --- | --- | --- | --- | --- |
| set-plan.v0.json | av.planning.set-plan.v0 | show_id、mode、goal、asset_pool_refs、sections[]、constraints_ref、delivery | 05 §4.1 | WSA-02 |
| audio-dsl.v0.json | av.planning.audio-dsl.v0 | show_id、layers[]（layer_id、role、source_strategy、asset_candidates、entry_rules、automation） | 05 §4.2 | WSA-03 |
| visual-dsl.v0.json | av.planning.visual-dsl.v0 | show_id、scenes[]（scene_id、program_ref、output_backend、view_group_ref、display_topology_ref、inputs、semantic_binding、uniform_defaults） | 05 §4.3 | WSA-04 |
| constraint-set.v0.json | av.planning.constraint-set.v0 | locked_sections、max_audio_layers、max_gpu_peak、allow_hard_cut、allowed_patch_scopes、banned_assets、required_tags、delivery_requirements | 05 §4.4 | WSA-05 |
| plan-submission.v0.json | av.planning.plan-submission.v0 | type、version、submitted_by、show_id、mode、inputs（set_plan、audio_dsl、visual_dsl、constraints） | 04 §9.1 | WSA-02 |

### 3.3 asset —— 资产与分析对象

| 文件名 | $id | 内容 | 来源文档 | 任务卡 |
| --- | --- | --- | --- | --- |
| asset-record.v0.json | av.asset.asset-record.v0 | asset_id、asset_kind、content_hash、raw_locator、normalized_locator、status、analysis_refs、derived_refs、tags、warm_status、readiness | 06 §5.1, 17 §6.3 | WSA-06 |
| ingestion-candidate.v0.json | av.asset.ingestion-candidate.v0 | candidate_id、path、declared_kind、size_bytes、modified_at | 06 §4.2 | WSA-06 |
| ingestion-run.v0.json | av.asset.ingestion-run.v0 | ingestion_run_id、source、mode、started_at、completed_at、discovered、published、reused、failed | 06 §5.2 | WSA-06 |
| analysis-job.v0.json | av.asset.analysis-job.v0 | analysis_job_id、asset_id、analyzer、analyzer_version、params_hash、status、cache_key、result_ref | 06 §5.3 | WSA-06 |
| analysis-cache-entry.v0.json | av.asset.analysis-cache-entry.v0 | cache_key、asset_id、analyzer、analyzer_version、input_fingerprint、dependency_fingerprint、created_at、status、payload_ref | 06 §5.4 | WSA-06 |

### 3.4 ir —— Normalized IR 与 Executable IR

| 文件名 | $id | 内容 | 来源文档 | 任务卡 |
| --- | --- | --- | --- | --- |
| asset-ir.v0.json | av.ir.asset-ir.v0 | asset_kind、locator、format、analysis、capabilities | 05 §5.1 | WSA-06 |
| structure-ir.v0.json | av.ir.structure-ir.v0 | sections[]（section_id、order、span、targets、locks）、transitions[] | 05 §5.2 | WSA-07 |
| performance-ir.v0.json | av.ir.performance-ir.v0 | performance_action[]（action_id、layer_id、op、target_asset_id、musical_time、duration_beats、quantize、priority、rollback_token、resource_hint） | 05 §5.3 | WSA-07 |
| visual-ir.v0.json | av.ir.visual-ir.v0 | visual_action[]（visual_action_id、scene_id、program_ref、uniform_set、camera_state、output_backend、view_group_ref、display_topology_ref、duration_beats、blend_mode、gpu_cost_hint、fallback_scene_id） | 05 §5.4 | WSA-07 |
| display-topology.v0.json | av.ir.display-topology.v0 | backend、display_endpoints[]、display roles、resolution、window placement | 05 §5.5 | WSA-07 |
| view-group.v0.json | av.ir.view-group.v0 | scene_ref、display_topology_ref、views[]（view_id、camera_id、display_id） | 05 §5.5 | WSA-07 |
| speaker-matrix-topology.v0.json | av.ir.speaker-matrix-topology.v0 | backend、speaker_endpoints[]、roles、device/channel mapping | 05 §5.5 | WSA-07 |
| route-set.v0.json | av.ir.route-set.v0 | topology_ref、routes[]（source_ref、route_mode、speaker_group） | 05 §5.5 | WSA-07 |
| timeline-entry.v0.json | av.ir.timeline-entry.v0 | id、show_id、revision、channel、target_ref、effective_window、scheduler（lookahead_ms、priority、conflict_group）、guards | 05 §6.1 | WSA-08 |
| show-state-snapshot.v0.json | av.ir.show-state-snapshot.v0 | show_id、revision、mode、time、semantic、transition、patch、active_audio_layers、active_visual_scene、active_view_group、active_route_group、visual_output、audio_output、resource_budget、health | 05 §6.2, 04 §5.2 | WSA-08 |
| compile-record.v0.json | av.ir.compile-record.v0 | compiler_run_id、base_submission_id、input_revision、output_revision、status、warnings、artifacts | 07 §4.2, 18 §4.5 | WSA-07 |
| revision-record.v0.json | av.ir.revision-record.v0 | revision、show_id、base_revision、status、artifact_refs、created_by | 18 §5.3 | WSA-07 |

### 3.5 patch —— 补丁相关对象

| 文件名 | $id | 内容 | 来源文档 | 任务卡 |
| --- | --- | --- | --- | --- |
| live-patch-proposal.v0.json | av.patch.live-patch-proposal.v0 | patch_id、submitted_by、patch_class、base_revision、scope（from_bar、to_bar、window）、intent、changes[]、fallback_revision | 08 §6 | WSA-09 |
| patch-ticket.v0.json | av.patch.patch-ticket.v0 | ticket_id、patch_id、state、effective_revision、fallback_revision、audit_ref | MCP schema $defs | WSA-09 |
| patch-decision.v0.json | av.patch.patch-decision.v0 | patch_id、base_revision、candidate_revision、decision、window、scope、authorization_ref、fallback_revision、reasons | 07 §4.4 | WSA-09 |

### 3.6 runtime —— 运行时事件

| 文件名 | $id | 内容 | 来源文档 | 任务卡 |
| --- | --- | --- | --- | --- |
| event-header.v0.json | av.runtime.event-header.v0 | event_id、show_id、revision、kind、source、musical_time、scheduler_time_ms、wallclock_hint_ms、priority、causation_id、replay_token | 04 §6.2, 05 §7.1 | WSA-11 |
| transport-event.v0.json | av.runtime.transport-event.v0 | op（play、pause、seek、stop） | 04 §6.1 | WSA-11 |
| timing-event.v0.json | av.runtime.timing-event.v0 | phrase、section、tempo、downbeat | 04 §7.1 | WSA-11 |
| audio-event.v0.json | av.runtime.audio-event.v0 | layer_id、op、output_backend、route_mode、route_set_ref、speaker_group、gain_db、duration_beats、filter | 04 §7.2 | WSA-11 |
| visual-event.v0.json | av.runtime.visual-event.v0 | scene_id、shader_program、output_backend、view_group、display_topology、calibration_profile、uniforms、views[]、duration_beats、blend | 04 §7.3 | WSA-11 |
| semantic-event.v0.json | av.runtime.semantic-event.v0 | energy、density、tension、intent | 04 §7.4 | WSA-11 |
| patch-event.v0.json | av.runtime.patch-event.v0 | patch_id、scope、effective_revision、fallback_revision | 04 §7.5 | WSA-11 |
| audio-analysis-summary.v0.json | av.runtime.audio-analysis-summary.v0 | window_ms、rms、crest、spectral_centroid、low/mid/high_band、transient_density、onset | 04 §8.2 | WSA-11 |

### 3.7 trace —— 追踪与评估

| 文件名 | $id | 内容 | 来源文档 | 任务卡 |
| --- | --- | --- | --- | --- |
| trace-manifest.v0.json | av.trace.trace-manifest.v0 | trace_bundle_id、show_id、revision、run_id、mode、started_at、completed_at、status、input_refs、event_log_ref、metrics_ref、evaluation_ref | 07 §3.2 | WSA-10 |
| event-record.v0.json | av.trace.event-record.v0 | record_type、event_id、show_id、revision、kind、phase、source、musical_time、scheduler_time_ms、wallclock_time_ms、causation_id、payload_ref、ack | 07 §4.3 | WSA-10 |
| resource-sample.v0.json | av.trace.resource-sample.v0 | sample_time_ms、show_id、revision、cpu、gpu、memory_mb、audio_xruns、video_dropped_frames、active_scene | 07 §4.5 | WSA-10 |
| metrics-summary.v0.json | av.trace.metrics-summary.v0 | show_id、revision、status、timing、quality（sync_score、transition_score、visual_semantic_score）、resource、issues[] | 04 §9.2 | WSA-10 |
| evaluation-request.v0.json | av.trace.evaluation-request.v0 | target（trace_bundle_id、revision）、dimensions[]、compare_with_revision | 07 §6.1 | WSA-10 |
| evaluation-report.v0.json | av.trace.evaluation-report.v0 | report_id、show_id、revision、scores、issues[]、summary | 07 §6.2 | WSA-10 |
| evaluation-fact.v0.json | av.trace.evaluation-fact.v0 | fact_id、dimension、metric、value、threshold、status、evidence_refs | 07 §6.3 | WSA-10 |
| replay-request.v0.json | av.trace.replay-request.v0 | trace_bundle_id、mode、from_bar、to_bar、with_audio_analysis、with_visual_frames | 07 §5.1 | WSA-10 |
| replay-session.v0.json | av.trace.replay-session.v0 | replay_session_id、source_trace_bundle_id、mode、status、inputs_restored、revision_restored、artifacts | 07 §5.2 | WSA-10 |
| checkpoint-snapshot.v0.json | av.trace.checkpoint-snapshot.v0 | checkpoint_id、show_id、revision、bar、show_state_ref、active_assets、rollback_ready | 07 §5.3 | WSA-10 |
| revision-diff.v0.json | av.trace.revision-diff.v0 | from_revision、to_revision、changes、impact | 07 §7.1 | WSA-10 |

### 3.8 link —— 视听联动

| 文件名 | $id | 内容 | 来源文档 | 任务卡 |
| --- | --- | --- | --- | --- |
| link-binding.v0.json | av.link.link-binding.v0 | link_id、mode、source、target、mapping、activation、policy | 12 §3.3 | WSA-11 |
| link-state.v0.json | av.link.link-state.v0 | show_id、revision、link_state（mode、active_scene、structural_anchor、semantic_bindings、signal_bindings_enabled、safety_mode） | 12 §8.1 | WSA-11 |

### 3.9 audio —— 音频输出与乐器绑定

| 文件名 | $id | 内容 | 来源文档 | 任务卡 |
| --- | --- | --- | --- | --- |
| audio-output-binding.v0.json | av.audio.audio-output-binding.v0 | binding_id、binding_type、target_backend、target_ref、latency_profile、fallback_ref | 20 §7.1 | WSA-11 |
| instrument-binding.v0.json | av.audio.instrument-binding.v0 | binding_id、binding_type、target_backend、preset_ref、midi_channel、latency_profile | 20 §7.2 | WSA-11 |
| backend-capability.v0.json | av.audio.backend-capability.v0 | backend_id、backend_kind、status、supports[]、latency_class | 20 §7.3 | WSA-11 |

### 3.10 capability —— 能力层对象

| 文件名 | $id | 内容 | 来源文档 | 任务卡 |
| --- | --- | --- | --- | --- |
| capability-request.v0.json | av.capability.capability-request.v0 | request_id、capability、actor、payload、metadata | 16 §5.1 | WSA-12 |
| capability-response.v0.json | av.capability.capability-response.v0 | （使用 response-envelope.v0.json） | 16 §5.2 | WSA-12 |
| operation-ticket.v0.json | av.capability.operation-ticket.v0 | operation_id、request_id、capability、state、started_at、updated_at | 16 §5.3 | WSA-12 |
| capability-descriptor.v0.json | av.capability.capability-descriptor.v0 | capability、input_schema、output_schema、execution_mode、idempotency、authorization、target_service | 16 §7.1 | WSA-12 |

---

## 4. 实施优先级

### 4.1 Phase 0 必须完成（M0 里程碑）

以下 schema 是编码启动的前置条件：

| 优先级 | Schema | 理由 |
| --- | --- | --- |
| P0 | common/* | 所有对象的基础引用 |
| P0 | planning/set-plan | 编译器输入 |
| P0 | planning/audio-dsl | 编译器输入 |
| P0 | planning/visual-dsl | 编译器输入 |
| P0 | planning/constraint-set | Validator 输入 |
| P0 | asset/asset-record | Registry 核心对象 |
| P0 | ir/structure-ir | 编译器输出 |
| P0 | ir/performance-ir | 编译器输出 |
| P0 | ir/visual-ir | 编译器输出 |
| P0 | ir/timeline-entry | Scheduler 输入 |
| P0 | patch/live-patch-proposal | Patch Manager 输入 |
| P0 | trace/trace-manifest | Trace Writer 输出 |
| P0 | runtime/event-header | 所有 Runtime 事件基础 |

### 4.2 Phase 0 过程中完成（M1-M5 期间）

| 优先级 | Schema | 理由 |
| --- | --- | --- |
| P1 | asset/ingestion-*, analysis-* | WSB 素材接入任务依赖 |
| P1 | ir/show-state-snapshot | WSD Scheduler 依赖 |
| P1 | ir/compile-record, revision-record | WSC Revision Manager 依赖 |
| P1 | runtime/timing-event, audio-event, visual-event | WSD 事件发布依赖 |
| P1 | patch/patch-ticket, patch-decision | WSF Patch Manager 依赖 |
| P1 | trace/event-record, resource-sample | WSE Trace Writer 依赖 |

### 4.3 Phase 0 末尾完成（M6 前）

| 优先级 | Schema | 理由 |
| --- | --- | --- |
| P2 | runtime/semantic-event, patch-event | 联动与 patch 集成测试 |
| P2 | trace/evaluation-*, replay-*, checkpoint-* | Evaluation 占位与 Replay 预研 |
| P2 | link/* | 视听联动集成测试 |
| P2 | audio/* | 音频后端绑定集成测试 |
| P2 | capability/* | MCP adapter 集成 |

---

## 5. Fixture 策略

### 5.1 每个 Schema 至少配套

- 1 个最小正例 fixture（只含 required 字段）
- 1 个完整正例 fixture（含所有 optional 字段）
- 2 个反例 fixture（缺少 required 字段 / 字段类型错误）

### 5.2 Fixture 目录

```text
tests/schema/
├── common/
│   ├── object-header.valid.min.json
│   ├── object-header.valid.full.json
│   ├── object-header.invalid.missing-id.json
│   └── object-header.invalid.wrong-type.json
├── planning/
│   ├── set-plan.valid.min.json
│   ├── set-plan.valid.full.json
│   ├── set-plan.invalid.missing-show-id.json
│   └── set-plan.invalid.bad-mode.json
└── ...
```

### 5.3 Golden Output

编译器输出的 IR 与 Timeline 应有 golden output fixture：

```text
tests/fixtures/golden/
├── compile-output-rev12/
│   ├── structure-ir.json
│   ├── performance-ir.json
│   ├── visual-ir.json
│   └── timeline.json
```

Golden output 用于回归测试，变更需显式审批。

---

## 6. Schema 总量统计

| 分类 | 文件数量 |
| --- | --- |
| common | 6 |
| planning | 5 |
| asset | 5 |
| ir | 12 |
| patch | 3 |
| runtime | 8 |
| trace | 11 |
| link | 2 |
| audio | 3 |
| capability | 4 |
| **合计** | **59** |

加上已有的 `schemas/mcp-tools/av-tool-registry.v0.json`，总计 **60** 个 schema 文件。

## 7. 结论

本清单覆盖了设计文档 04-20、27、28 中定义的主要结构化对象。59 个 schema 文件按三个优先级分批交付：

1. **P0**（M0 里程碑）：13 个核心 schema，编码启动前置。
2. **P1**（M1-M5 期间）：随各 Workstream 按需交付。
3. **P2**（M6 前）：集成测试与进阶功能所需。

Schema 是系统工件面的形式化边界，任何 schema 变更都必须触发配套 fixture 更新与 CI 校验。
