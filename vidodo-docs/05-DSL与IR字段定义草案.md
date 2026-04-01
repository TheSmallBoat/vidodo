# DSL / IR 字段定义草案

日期：2026-04-01
状态：Draft
前置阅读：02-视听系统产品定位与规划方案书.md，03-外部规划与介入机制顶层设计书.md，04-音频与视觉运行时事件协议与数据结构.md

## 1. 文档目的

本文定义视听系统中 DSL 与 IR 的字段层级、对象边界与最小约束。

目标不是立即锁死最终语法，而是先回答三件事：

1. 外部规划者到底提交哪些对象。
2. 编译链路内部到底保留哪些稳定字段。
3. Audio Runtime 与 Visual Runtime 到底消费哪一层可执行对象。

本文延续已有文档中的总原则：

- 规划者位于系统外部
- 系统只接受结构化工件
- 离线与实时共享同一套时间语义
- Runtime 只消费可验证、可版本化、可回放的对象

## 2. 分层结论

### 2.1 推荐采用四层对象体系

```text
Planning DSL
  -> Normalized IR
  -> Executable IR
  -> Runtime Event Payload
```

各层职责如下：

| 层级 | 作用 | 主要使用者 |
| --- | --- | --- |
| Planning DSL | 面向人类或外部 Agent 的高层规划表达 | 外部规划者 |
| Normalized IR | 编译器统一后的规范化对象层 | Compiler / Validators |
| Executable IR | 调度器直接消费的可执行时间线与状态图 | Scheduler / Patch Manager |
| Runtime Event Payload | Runtime 最终接收的事件载荷 | Audio Runtime / Visual Runtime |

### 2.2 不建议把所有语义压进单层 JSON

如果把结构规划、资源提示、patch 意图和 runtime 事件全部塞进同一种对象，会导致：

- 验证粒度混乱
- patch diff 难以解释
- Runtime 消费过多高层语义
- 离线与实时难以保持稳定兼容

因此应显式区分：

- 规划层对象
- 规范化对象
- 可执行对象
- 运行时事件对象

## 3. 通用字段规范

### 3.1 所有一等对象都应带有统一头

```json
{
  "id": "section-drop-a",
  "type": "section",
  "version": "0.1",
  "revision": 12,
  "schema": "av.section.v0",
  "source": {
    "kind": "external_client",
    "submitted_by": "copilot",
    "submission_id": "subm-018"
  },
  "provenance": {
    "plan_id": "plan-live-a",
    "compiler_run_id": "compile-032",
    "parent_ids": ["setplan-main"]
  },
  "labels": ["live_safe", "drop"],
  "annotations": {
    "note": "optional human readable note"
  }
}
```

最小字段说明：

- id：对象稳定标识，要求在同一 show 内唯一
- type：对象类型名，不依赖文件后缀推断
- version：对象自身格式版本
- revision：所属 show revision
- schema：校验 schema 名称
- source：对象来源
- provenance：编译与父对象来源链
- labels：策略门禁或搜索标签
- annotations：非关键元信息，不进入确定性比较

### 3.2 ID 与引用规则

- 所有对象 ID 必须稳定，不应在无语义变化时重新随机生成
- 引用使用显式字段，如 asset_id、section_id、scene_id
- 跨对象引用只允许引用已发布对象或同一编译单元中的对象
- Runtime Event 不直接引用 Planning DSL 节点，必须引用 Executable IR 节点

### 3.3 时间字段规则

统一使用以下三类时间字段：

| 字段 | 含义 | 允许出现的层级 |
| --- | --- | --- |
| musical_time | 音乐结构时间 | Normalized IR 及以后 |
| scheduler_time | 调度器提交时间 | Executable IR 及以后 |
| wallclock_hint | 真实时间提示 | Runtime Event |

时间相关字段命名要求：

- beat、bar、phrase、section 用于结构定位
- duration_beats、lookahead_ms、latency_ms 用于执行属性
- from_bar、to_bar 用于 patch 或选择范围

### 3.4 数值与枚举规则

- 所有归一化连续值使用 0.0 到 1.0，除非领域量纲明确
- dB、Hz、BPM、ms 保留原始量纲，不做归一化伪装
- 枚举值尽量短且稳定，例如 live、offline_render、safe、deferred
- 布尔值只表达真伪，不混入三态语义；三态请用枚举

### 3.5 Patch 相关字段规则

每个可 patch 对象都应声明以下元信息：

```json
{
  "patchability": {
    "allowed": true,
    "policy": "phrase_boundary_only",
    "locked": false,
    "fallback_ref": "rev-12"
  }
}
```

这保证 patch 管理器能在不猜测业务语义的情况下做初步决策。

## 4. Planning DSL 字段草案

### 4.1 SetPlan

SetPlan 是外部规划者提交的最高层规划对象，用于描述作品或演出的宏观结构。

```json
{
  "type": "set_plan",
  "id": "set-main-a",
  "show_id": "show-2026-04-01-a",
  "mode": "live",
  "goal": {
    "intent": "club_build_release",
    "duration_target_sec": 900,
    "style_tags": ["percussive", "industrial", "narrow_palette"]
  },
  "asset_pool_refs": ["pool-audio-main", "pool-visual-main"],
  "sections": [
    {
      "section_id": "intro",
      "length_bars": 32,
      "energy_target": 0.28,
      "density_target": 0.24,
      "visual_intent": "low_glow_corridor"
    }
  ],
  "constraints_ref": "constraint-live-safe-v1",
  "delivery": {
    "render_bundle": true,
    "trace_bundle": true,
    "evaluation": true
  }
}
```

建议字段：

- show_id：所属 show
- mode：live、offline_render、rehearsal
- goal：风格、时长、强约束
- asset_pool_refs：可用素材池
- sections：宏观段落草案
- constraints_ref：门禁规则集
- delivery：输出要求

### 4.2 AudioDsl

AudioDsl 描述音频层意图，不直接暴露底层 DSP 图细节。

```json
{
  "type": "audio_dsl",
  "id": "audio-main-a",
  "show_id": "show-2026-04-01-a",
  "layers": [
    {
      "layer_id": "deck_a",
      "role": "rhythm",
      "source_strategy": "pool_select",
      "asset_candidates": ["loop/kick-a", "loop/kick-b"],
      "entry_rules": {
        "section_refs": ["build_a", "drop_a"],
        "quantize": "bar",
        "max_simultaneous": 1
      },
      "automation": [
        {
          "param": "gain_db",
          "curve": "linear",
          "from": -12.0,
          "to": -3.0,
          "duration_beats": 16
        }
      ]
    }
  ]
}
```

### 4.3 VisualDsl

VisualDsl 描述场景、材质输入与参数轨迹。

```json
{
  "type": "visual_dsl",
  "id": "visual-main-a",
  "show_id": "show-2026-04-01-a",
  "scenes": [
    {
      "scene_id": "corridor_low",
      "program_ref": "glsl/corridor_main",
      "inputs": {
        "texture_refs": ["tex/fog-01"],
        "buffer_graph": "graph/corridor-a"
      },
      "semantic_binding": {
        "intent": "compress",
        "section_refs": ["intro", "build_a"]
      },
      "uniform_defaults": {
        "u_motion_gain": 0.32,
        "u_noise_scale": 0.18
      }
    }
  ]
}
```

### 4.4 ConstraintSet

ConstraintSet 不属于实现细节，而是规划输入的一部分。

建议字段：

- locked_sections
- max_audio_layers
- max_gpu_peak
- allow_hard_cut
- allowed_patch_scopes
- banned_assets
- required_tags
- delivery_requirements

## 5. Normalized IR 字段草案

### 5.1 Asset IR

Asset IR 用于稳定描述声音、视觉及其衍生分析结果。

```json
{
  "type": "asset_ir",
  "id": "asset-loop-kick-a",
  "asset_kind": "audio_loop",
  "locator": {
    "uri": "asset://audio/loop/kick-a.wav",
    "content_hash": "sha256:...",
    "storage_tier": "normalized"
  },
  "format": {
    "codec": "wav",
    "sample_rate": 48000,
    "channels": 2,
    "duration_ms": 7421
  },
  "analysis": {
    "tempo": 128.0,
    "key": "E minor",
    "downbeat_confidence": 0.92,
    "energy_profile_ref": "analysis-013",
    "section_map_ref": "analysis-014"
  },
  "capabilities": {
    "time_stretch_safe": true,
    "pitch_shift_safe": false,
    "loop_aligned": true
  }
}
```

### 5.2 Structure IR

Structure IR 统一表示 section、phrase、transition、cue 与目标语义状态。

```json
{
  "type": "structure_ir",
  "id": "structure-main-a",
  "show_id": "show-2026-04-01-a",
  "sections": [
    {
      "section_id": "build_a",
      "order": 2,
      "span": {
        "from_bar": 33,
        "to_bar": 64
      },
      "targets": {
        "energy": 0.74,
        "density": 0.62,
        "tension": 0.81,
        "visual_intent": "compress_then_expand"
      },
      "locks": {
        "structure_locked": false,
        "tempo_locked": true
      }
    }
  ],
  "transitions": [
    {
      "transition_id": "build_to_drop_a",
      "from_section": "build_a",
      "to_section": "drop_a",
      "window": "phrase_boundary",
      "policy": "safe_crossfade_only"
    }
  ]
}
```

### 5.3 Performance IR

Performance IR 是调度器直接消费的音频执行对象。

核心字段：

- action_id
- layer_id
- op
- target_asset_id
- musical_time
- duration_beats
- quantize
- priority
- rollback_token
- resource_hint
- patch_origin

示例：

```json
{
  "type": "performance_action",
  "id": "act-128",
  "layer_id": "deck_b",
  "op": "launch_asset",
  "target_asset_id": "asset-loop-kick-a",
  "musical_time": {
    "bar": 65,
    "beat": 257.0,
    "phrase": 17,
    "section": "drop_a"
  },
  "duration_beats": 16,
  "quantize": "bar",
  "resource_hint": {
    "cpu_cost": 0.12,
    "io_streams": 1
  },
  "rollback_token": "rev12-act128"
}
```

### 5.4 Visual IR

Visual IR 是视觉侧的可执行对象，结构与 Performance IR 对齐，但保持视觉语义独立。

建议字段：

- visual_action_id
- scene_id
- program_ref
- uniform_set
- camera_state
- duration_beats
- blend_mode
- semantic_dependency
- gpu_cost_hint
- fallback_scene_id

## 6. Executable IR 字段草案

### 6.1 Unified Timeline Entry

Executable IR 最核心的对象是统一时间线条目。

```json
{
  "type": "timeline_entry",
  "id": "tle-00128",
  "show_id": "show-2026-04-01-a",
  "revision": 12,
  "channel": "audio",
  "target_ref": "act-128",
  "effective_window": {
    "from_bar": 65,
    "to_bar": 69,
    "activation": "bar_boundary"
  },
  "scheduler": {
    "lookahead_ms": 120,
    "priority": 80,
    "conflict_group": "deck_b"
  },
  "guards": {
    "requires_assets": ["asset-loop-kick-a"],
    "requires_revision": 12,
    "abort_if_locked": true
  }
}
```

### 6.2 Show State Snapshot

Show State Snapshot 是执行期共享上下文，不应由外部直接伪造。

建议字段：

- show_id
- revision
- mode
- time
- semantic
- transition
- patch
- active_audio_layers
- active_visual_scene
- resource_budget
- health

### 6.3 Patch Plan

Patch Plan 是 Executable IR 的特化对象，用于承接已经通过验证的 patch。

建议字段：

- patch_id
- base_revision
- effective_revision
- scope
- intent
- changed_entries
- authorization_ref
- fallback_revision
- expires_at_bar

## 7. Runtime Event Payload 字段草案

### 7.1 统一事件头

此层延续 04 文档中的事件头约束，再补充两个字段：

- causation_id：本事件由哪个上游动作触发
- replay_token：重放系统用于关联确认结果的稳定令牌

```json
{
  "event_id": "evt-000128",
  "kind": "visual.scene.update",
  "show_id": "show-2026-04-01-a",
  "revision": 12,
  "causation_id": "tle-00129",
  "replay_token": "rev12-evt128"
}
```

### 7.2 Audio Runtime Payload

建议最小字段：

- layer_id
- op
- asset_id
- gain_db
- duration_beats
- filter
- quantize_anchor
- fallback_op

### 7.3 Visual Runtime Payload

建议最小字段：

- scene_id
- program_ref
- uniform_updates
- camera_state
- blend
- duration_beats
- semantic_state
- fallback_scene_id

## 8. 字段兼容性与演进策略

### 8.1 允许扩展的字段区域

以下区域可以通过 namespaced 扩展字段演进：

- annotations
- labels
- analysis.extra
- resource_hint.extra
- runtime_extensions

### 8.2 不应轻易变动的字段

以下字段应视为稳定协议面：

- id
- type
- show_id
- revision
- musical_time
- scope
- patchability
- scheduler.priority
- fallback_revision

### 8.3 演进规则

- 新字段只能向后兼容追加
- 语义变化必须升级 schema 名称或主版本
- 删除稳定字段时必须提供迁移器和 diff 解释

## 9. 最小文件组织建议

```text
plans/
  set-plan-a.json
dsl/
  audio-a.json
  visual-a.json
ir/
  assets.json
  structure.json
  timeline.json
patches/
  patch-018.json
constraints/
  live-safe.json
```

## 10. 结论

本草案的关键判断是：

1. 规划表达、规范化对象、可执行对象和运行时事件必须分层。
2. 统一头、统一时间字段和统一 patch 元数据必须尽早固定。
3. Runtime 只应消费 Executable IR 与 Runtime Event，不应反向理解高层 DSL。

这使系统既能被人类编写，也能被外部 Agent 生成，同时仍保持可验证与可回放。
