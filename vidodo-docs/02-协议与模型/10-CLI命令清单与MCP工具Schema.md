# CLI 命令清单与 MCP Tool Schema

日期：2026-04-01
状态：Draft
前置阅读：05-DSL与IR字段定义草案.md，06-资产接入与分析缓存设计.md，07-追踪回放与评估数据模型.md，09-CLI与MCP能力模型与命令语义.md

## 1. 文档目的

本文把 09 文档中的能力模型进一步细化为：

1. 可直接实现的 CLI 命令清单
2. 可直接暴露给外部 Agent 的 MCP tool registry
3. 每类能力的输入输出 schema 约定

本文不改变 09 的原则，只把它落到更具体的接口面。

## 2. 总体设计

### 2.1 统一能力，不统一表现形式

系统的能力对象只有一套，但存在两种调用表现：

- 人类使用 CLI
- 外部 Agent 使用 MCP tools

因此，一个能力总是同时有：

- capability id
- CLI mapping
- MCP tool name
- input schema
- output schema

### 2.2 推荐返回 envelope

所有 CLI JSON 输出与 MCP tool 响应都建议包裹在统一 envelope 中：

```json
{
  "status": "ok",
  "capability": "asset.ingest",
  "request_id": "req-018",
  "data": {},
  "diagnostics": [],
  "artifacts": [],
  "next_actions": []
}
```

字段说明：

- status：ok、accepted、warning、error
- capability：能力唯一名
- request_id：请求级唯一标识
- data：主体返回值
- diagnostics：校验、预算、兼容性诊断
- artifacts：相关工件引用
- next_actions：推荐下一步动作

## 3. CLI 命令清单

### 3.1 asset 命令族

| 命令 | 作用 | 类型 | 幂等性 | 主要返回 |
| --- | --- | --- | --- | --- |
| avctl asset ingest | 导入素材并创建 ingestion run | mutating | conditional | ingestion_run_id、asset_ids |
| avctl asset list | 查询素材索引 | query | idempotent | assets[] |
| avctl asset inspect | 查看单素材的 record、analysis、capabilities | query | idempotent | asset_record |
| avctl asset analyze | 显式触发或补跑分析任务 | mutating | conditional | analysis_job_id |
| avctl asset warm | 预热 live 需要的资源 | mutating | conditional | warm_ticket |
| avctl asset pools | 查看素材池与标签过滤结果 | query | idempotent | asset_pool |

#### avctl asset ingest

```text
avctl asset ingest --from imports/session_a --kind audio_loop --profile live-safe
```

输入：

- --from：导入路径
- --kind：声明类型，可选
- --profile：导入配置，如 live-safe、offline-full
- --tags：附加标签，可重复

输出示例：

```json
{
  "status": "accepted",
  "capability": "asset.ingest",
  "data": {
    "ingestion_run_id": "ing-20260401-01",
    "published": 4,
    "reused": 1,
    "failed": 0,
    "asset_ids": [
      "asset-loop-kick-a",
      "asset-stem-bass-a"
    ]
  },
  "artifacts": [
    "registry/ingestion-runs.jsonl#ing-20260401-01"
  ]
}
```

#### avctl asset inspect

```text
avctl asset inspect --asset asset-loop-kick-a --include analysis,capabilities
```

#### avctl asset warm

```text
avctl asset warm --asset asset-loop-kick-a --target live --deadline next_phrase
```

### 3.2 plan 命令族

| 命令 | 作用 | 类型 | 幂等性 | 主要返回 |
| --- | --- | --- | --- | --- |
| avctl plan validate | 预检 SetPlan / AudioDsl / VisualDsl / ConstraintSet | validate | idempotent | diagnostics |
| avctl plan submit | 提交规划工件 | mutating | conditional | submission_id |
| avctl plan show | 查询当前 show 的 planning 工件 | query | idempotent | plan bundle |
| avctl plan lock | 锁定 section 或 revision | mutating | conditional | lock record |

#### avctl plan validate

```text
avctl plan validate \
  --set-plan plans/set-plan-a.json \
  --audio dsl/audio-a.json \
  --visual dsl/visual-a.json \
  --constraints constraints/live-safe.json
```

输出重点：

- schema 是否通过
- 资源预算是否可行
- 所需资产是否已发布
- 是否存在锁定区间冲突

#### avctl plan submit

```text
avctl plan submit --bundle bundles/plan-bundle-a.json
```

### 3.3 compile 命令族

| 命令 | 作用 | 类型 | 幂等性 | 主要返回 |
| --- | --- | --- | --- | --- |
| avctl compile run | 生成新 revision 并执行校验 | mutating | conditional | compiler_run_id、candidate_revision |
| avctl compile inspect | 查看指定 revision 的 IR 工件 | query | idempotent | artifact ref |
| avctl compile diff | 比较两个 revision 的结构差异 | validate | idempotent | revision_diff |
| avctl compile publish | 把 candidate_revision 标记为可运行 | mutating | conditional | published revision |

#### avctl compile run

```text
avctl compile run --submission subm-018 --base-revision 11
```

输出示例：

```json
{
  "status": "accepted",
  "capability": "compile.run",
  "data": {
    "compiler_run_id": "compile-032",
    "base_revision": 11,
    "candidate_revision": 12,
    "validation_status": "passed"
  },
  "artifacts": [
    "compile/assets.json",
    "compile/structure.json",
    "compile/timeline.json"
  ]
}
```

### 3.4 run 命令族

| 命令 | 作用 | 类型 | 幂等性 | 主要返回 |
| --- | --- | --- | --- | --- |
| avctl run start | 启动 live 或 offline run | mutating | conditional | run_id |
| avctl run status | 查看 show state、revision、资源与 patch window | query | idempotent | runtime status |
| avctl run pause | 暂停支持 pause 的 mode | mutating | conditional | status update |
| avctl run stop | 停止执行 | mutating | conditional | final status |
| avctl run checkpoints | 查询 checkpoint | query | idempotent | checkpoint list |

#### avctl run status

```text
avctl run status --show-id show-2026-04-01-a --include show_state,resource,patch
```

返回重点：

- current_revision
- current_section
- next_patch_window
- locked_sections
- cpu/gpu headroom

### 3.5 patch 命令族

| 命令 | 作用 | 类型 | 幂等性 | 主要返回 |
| --- | --- | --- | --- | --- |
| avctl patch check | 预检 patch proposal | validate | idempotent | diagnostics、authorization preview |
| avctl patch submit | 提交 patch proposal | mutating | conditional | patch_ticket |
| avctl patch approve | 批准高风险 patch | mutating | conditional | authorization result |
| avctl patch reject | 拒绝 patch | mutating | conditional | decision record |
| avctl patch status | 查询 patch 生命周期状态 | query | idempotent | patch state |
| avctl patch rollback | 触发回退 | mutating | conditional | rollback record |
| avctl patch degrade | 触发降级模式 | mutating | conditional | degrade record |

#### avctl patch check

```text
avctl patch check --file patches/patch-018.json --against-run run-03
```

输出示例：

```json
{
  "status": "warning",
  "capability": "patch.check",
  "data": {
    "patch_id": "patch-018",
    "base_revision": 12,
    "window": "next_phrase_boundary",
    "authorization": {
      "allowed": true,
      "requires_human_confirmation": false
    },
    "resource_delta": {
      "cpu": 0.03,
      "gpu": 0.07
    }
  },
  "diagnostics": [
    {
      "code": "gpu_headroom_low",
      "severity": "warning"
    }
  ]
}
```

#### avctl patch rollback

```text
avctl patch rollback --patch-id patch-018 --mode immediate
```

### 3.6 trace / replay / eval 命令族

| 命令 | 作用 | 类型 | 幂等性 | 主要返回 |
| --- | --- | --- | --- | --- |
| avctl trace show | 查看 trace bundle manifest | query | idempotent | trace manifest |
| avctl trace events | 查询事件区间 | query | idempotent | event records |
| avctl trace patches | 查询 patch decision 记录 | query | idempotent | patch decisions |
| avctl replay start | 发起 replay 任务 | mutating | conditional | replay_session_id |
| avctl replay status | 查询 replay 进度 | query | idempotent | replay status |
| avctl eval run | 发起评价 | mutating | conditional | evaluation_job_id |
| avctl eval show | 查看评价报告 | query | idempotent | evaluation report |

#### avctl trace events

```text
avctl trace events --run-id run-03 --from-bar 129 --to-bar 145 --kind visual.scene.update
```

#### avctl eval run

```text
avctl eval run --trace-bundle trace-show-a-rev12-run03 --compare-with 11
```

### 3.7 system 命令族

| 命令 | 作用 | 类型 |
| --- | --- | --- |
| avctl system capabilities | 输出 capability registry | query |
| avctl system health | 输出服务与 runtime 健康摘要 | query |
| avctl system policies | 输出当前 patch / resource / authorization policy | query |
| avctl system schemas | 列出支持的 schema 版本 | query |

## 4. MCP Tool Registry 设计

### 4.1 Tool 命名规则

统一采用：

```text
<namespace>.<verb>
```

例如：

- asset.ingest
- compile.run
- patch.check
- trace.query

### 4.2 Tool descriptor 通用结构

```json
{
  "name": "patch.submit",
  "title": "Submit Live Patch",
  "description": "Submit a bounded live patch proposal against a running show.",
  "inputSchema": {
    "type": "object",
    "required": ["patch"],
    "properties": {
      "patch": {
        "$ref": "#/defs/livePatchProposal"
      },
      "dry_run": {
        "type": "boolean",
        "default": false
      }
    }
  },
  "outputSchema": {
    "$ref": "#/defs/patchTicket"
  },
  "annotations": {
    "readOnlyHint": false,
    "idempotency": "conditional",
    "async": true,
    "authorization": "operator_or_policy"
  }
}
```

### 4.3 推荐 tool 列表

#### asset namespace

- asset.ingest
- asset.list
- asset.inspect
- asset.analyze
- asset.warm
- asset.pools

#### plan namespace

- plan.validate
- plan.submit
- plan.show
- plan.lock

#### compile namespace

- compile.run
- compile.inspect
- compile.diff
- compile.publish

#### run namespace

- run.start
- run.status
- run.pause
- run.stop
- run.checkpoints

#### patch namespace

- patch.check
- patch.submit
- patch.approve
- patch.reject
- patch.status
- patch.rollback
- patch.degrade

#### trace / replay / eval namespace

- trace.show
- trace.query
- trace.patches
- replay.start
- replay.status
- eval.run
- eval.show

#### system namespace

- system.describe_capabilities
- system.health
- system.policies
- system.schemas

## 5. 关键 MCP Tool Schema

### 5.1 asset.ingest

输入：

```json
{
  "type": "object",
  "required": ["source"],
  "properties": {
    "source": {
      "type": "string"
    },
    "declared_kind": {
      "type": "string",
      "enum": [
        "audio_stem",
        "audio_loop",
        "audio_oneshot",
        "midi_clip",
        "glsl_program",
        "texture_2d",
        "buffer_graph"
      ]
    },
    "profile": {
      "type": "string",
      "enum": ["live-safe", "offline-full", "fast-stage"]
    },
    "tags": {
      "type": "array",
      "items": {
        "type": "string"
      }
    }
  }
}
```

输出：

```json
{
  "type": "object",
  "required": ["ingestion_run_id", "published", "reused", "failed"],
  "properties": {
    "ingestion_run_id": {"type": "string"},
    "published": {"type": "integer"},
    "reused": {"type": "integer"},
    "failed": {"type": "integer"},
    "asset_ids": {
      "type": "array",
      "items": {"type": "string"}
    }
  }
}
```

### 5.2 plan.submit

输入：

```json
{
  "type": "object",
  "required": ["show_id", "set_plan", "audio_dsl", "visual_dsl", "constraints"],
  "properties": {
    "show_id": {"type": "string"},
    "set_plan": {"type": "object"},
    "audio_dsl": {"type": "object"},
    "visual_dsl": {"type": "object"},
    "constraints": {"type": "object"},
    "submission_id": {"type": "string"}
  }
}
```

输出：

```json
{
  "type": "object",
  "required": ["submission_id", "show_id", "status"],
  "properties": {
    "submission_id": {"type": "string"},
    "show_id": {"type": "string"},
    "status": {"type": "string", "enum": ["planned", "rejected"]},
    "diagnostics": {
      "type": "array",
      "items": {"type": "object"}
    }
  }
}
```

### 5.3 compile.run

输入：

```json
{
  "type": "object",
  "required": ["submission_id"],
  "properties": {
    "submission_id": {"type": "string"},
    "base_revision": {"type": "integer"},
    "publish_on_success": {"type": "boolean", "default": false}
  }
}
```

输出：

```json
{
  "type": "object",
  "required": ["compiler_run_id", "candidate_revision", "validation_status"],
  "properties": {
    "compiler_run_id": {"type": "string"},
    "base_revision": {"type": "integer"},
    "candidate_revision": {"type": "integer"},
    "validation_status": {"type": "string", "enum": ["passed", "failed"]},
    "artifact_refs": {
      "type": "array",
      "items": {"type": "string"}
    }
  }
}
```

### 5.4 run.status

输入：

```json
{
  "type": "object",
  "required": ["show_id"],
  "properties": {
    "show_id": {"type": "string"},
    "include": {
      "type": "array",
      "items": {
        "type": "string",
        "enum": ["show_state", "resource", "patch", "checkpoints"]
      }
    }
  }
}
```

输出：

```json
{
  "type": "object",
  "required": ["show_id", "run_id", "revision", "mode"],
  "properties": {
    "show_id": {"type": "string"},
    "run_id": {"type": "string"},
    "revision": {"type": "integer"},
    "mode": {"type": "string"},
    "show_state": {"type": "object"},
    "resource": {"type": "object"},
    "patch": {"type": "object"}
  }
}
```

### 5.5 patch.check

输入：

```json
{
  "type": "object",
  "required": ["patch"],
  "properties": {
    "patch": {
      "$ref": "#/defs/livePatchProposal"
    },
    "run_id": {"type": "string"}
  }
}
```

输出：

```json
{
  "type": "object",
  "required": ["patch_id", "allowed", "diagnostics"],
  "properties": {
    "patch_id": {"type": "string"},
    "allowed": {"type": "boolean"},
    "requires_human_confirmation": {"type": "boolean"},
    "window": {"type": "string"},
    "resource_delta": {"type": "object"},
    "diagnostics": {
      "type": "array",
      "items": {"type": "object"}
    }
  }
}
```

### 5.6 patch.submit

输入：

```json
{
  "type": "object",
  "required": ["patch"],
  "properties": {
    "patch": {
      "$ref": "#/defs/livePatchProposal"
    },
    "auto_approve_if_allowed": {
      "type": "boolean",
      "default": true
    }
  }
}
```

输出：

```json
{
  "type": "object",
  "required": ["patch_id", "ticket_id", "state"],
  "properties": {
    "patch_id": {"type": "string"},
    "ticket_id": {"type": "string"},
    "state": {
      "type": "string",
      "enum": ["proposed", "authorized", "validated", "staged", "activated", "rejected"]
    },
    "effective_revision": {"type": "integer"},
    "fallback_revision": {"type": "integer"}
  }
}
```

### 5.7 patch.rollback

输入：

```json
{
  "type": "object",
  "required": ["patch_id", "mode"],
  "properties": {
    "patch_id": {"type": "string"},
    "mode": {
      "type": "string",
      "enum": ["deferred", "immediate", "partial"]
    },
    "reason": {"type": "string"}
  }
}
```

输出：

```json
{
  "type": "object",
  "required": ["patch_id", "rollback_id", "status"],
  "properties": {
    "patch_id": {"type": "string"},
    "rollback_id": {"type": "string"},
    "status": {"type": "string", "enum": ["accepted", "completed"]},
    "restored_revision": {"type": "integer"},
    "checkpoint_ref": {"type": "string"}
  }
}
```

### 5.8 trace.query

输入：

```json
{
  "type": "object",
  "required": ["run_id"],
  "properties": {
    "run_id": {"type": "string"},
    "from_bar": {"type": "integer"},
    "to_bar": {"type": "integer"},
    "kind": {"type": "string"}
  }
}
```

### 5.9 eval.run

输入：

```json
{
  "type": "object",
  "required": ["trace_bundle_id"],
  "properties": {
    "trace_bundle_id": {"type": "string"},
    "dimensions": {
      "type": "array",
      "items": {
        "type": "string",
        "enum": [
          "sync",
          "transition",
          "visual_semantic_alignment",
          "resource_stability"
        ]
      }
    },
    "compare_with_revision": {"type": "integer"}
  }
}
```

## 6. 共享 defs 建议

### 6.1 livePatchProposal

```json
{
  "type": "object",
  "required": ["patch_id", "patch_class", "base_revision", "scope", "changes", "fallback_revision"],
  "properties": {
    "patch_id": {"type": "string"},
    "patch_class": {
      "type": "string",
      "enum": ["param", "local_content", "structural", "emergency"]
    },
    "base_revision": {"type": "integer"},
    "scope": {
      "type": "object",
      "required": ["from_bar", "to_bar", "window"],
      "properties": {
        "from_bar": {"type": "integer"},
        "to_bar": {"type": "integer"},
        "window": {
          "type": "string",
          "enum": [
            "next_beat",
            "next_bar",
            "next_phrase_boundary",
            "next_section_boundary",
            "emergency_slot"
          ]
        }
      }
    },
    "intent": {"type": "object"},
    "changes": {
      "type": "array",
      "items": {"type": "object"}
    },
    "fallback_revision": {"type": "integer"}
  }
}
```

### 6.2 patchTicket

```json
{
  "type": "object",
  "required": ["ticket_id", "patch_id", "state"],
  "properties": {
    "ticket_id": {"type": "string"},
    "patch_id": {"type": "string"},
    "state": {"type": "string"},
    "effective_revision": {"type": "integer"},
    "fallback_revision": {"type": "integer"},
    "audit_ref": {"type": "string"}
  }
}
```

## 7. 实现顺序建议

优先实现顺序建议如下：

1. asset.ingest / asset.inspect / asset.warm
2. plan.validate / plan.submit
3. compile.run / compile.diff
4. run.start / run.status
5. patch.check / patch.submit / patch.rollback
6. trace.query / eval.run / replay.start

原因是这条路径刚好覆盖从素材到 live patch 回退的最小闭环。

## 8. 结论

CLI 命令清单和 MCP tool schema 的关键不在“命令多不多”，而在是否共享同一套能力语义与对象 schema。

关键判断如下：

1. CLI 与 MCP 必须使用统一 capability id、统一输入输出 envelope。
2. 高价值能力至少应明确输入字段、输出字段、幂等性、异步性和授权要求。
3. patch、trace、evaluation 相关能力必须天生结构化，不能退化成文本接口。

这样，后续实现时才不会把人类操作链路和外部 Agent 链路做成两套系统。
