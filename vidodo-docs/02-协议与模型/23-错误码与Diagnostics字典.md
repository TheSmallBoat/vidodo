# 错误码与 Diagnostics 字典

日期：2026-04-01
状态：Draft
前置阅读：09-CLI与MCP能力模型与命令语义.md，16-Capability-Layer与命令执行模型详细设计.md，21-测试与验证顶层设计方案.md

## 1. 文档目的

本文为系统所有模块统一定义错误码编码规范、Diagnostic 对象结构和标准错误目录。

目标是让 CLI、MCP、Trace、Evaluation 输出的诊断信息可被人类和外部 Agent 等价消费。

## 2. 设计原则

### 2.1 错误码必须机器可解析

错误码不能只是自由文本。每个错误都应具有稳定的 code，可被程序分支、过滤和统计。

### 2.2 严重级别必须统一

所有模块使用同一套严重级别枚举，不允许自定义级别。

### 2.3 错误码按命名空间组织

错误码前缀对应模块命名空间，避免跨模块冲突。

## 3. Diagnostic 对象结构

### 3.1 标准 Diagnostic

```json
{
  "code": "PATCH_WINDOW_MISSED",
  "namespace": "patch",
  "severity": "error",
  "message": "Patch window 'next_phrase_boundary' has already passed for bar 129.",
  "target": {
    "object_type": "live_patch_proposal",
    "object_id": "patch-018",
    "field": "scope.window"
  },
  "retryable": false,
  "details": {},
  "suggestion": "Wait for next available patch window or use 'next_section_boundary'."
}
```

### 3.2 字段说明

| 字段 | 类型 | 必须 | 说明 |
| --- | --- | --- | --- |
| code | string | 是 | 稳定错误码，大写蛇形命名 |
| namespace | string | 是 | 归属模块命名空间 |
| severity | enum | 是 | error / warning / info / hint |
| message | string | 是 | 人类可读描述 |
| target | object | 否 | 问题定位 |
| retryable | bool | 是 | 是否可重试 |
| details | object | 否 | 补充机器数据 |
| suggestion | string | 否 | 建议修正方向 |

### 3.3 严重级别定义

| 级别 | 含义 | 系统行为 |
| --- | --- | --- |
| error | 阻断操作 | 请求失败，不产生状态变更 |
| warning | 有风险但不阻断 | 操作继续，标记风险 |
| info | 辅助信息 | 不影响操作 |
| hint | 建议优化 | 人类参考，不进入自动决策 |

## 4. 命名空间与前缀

| 命名空间 | 前缀 | 归属模块 |
| --- | --- | --- |
| schema | SCHEMA_ | Validator |
| asset | ASSET_ | Asset Ingestion / Registry |
| analysis | ANALYSIS_ | Analysis Pipeline |
| plan | PLAN_ | Plan 提交与校验 |
| compile | COMPILE_ | Compiler |
| revision | REVISION_ | Revision Manager |
| run | RUN_ | Scheduler / Runtime |
| patch | PATCH_ | Patch Manager |
| trace | TRACE_ | Trace Writer / Query |
| eval | EVAL_ | Evaluation Engine |
| system | SYSTEM_ | 系统级 |
| auth | AUTH_ | 授权 |

## 5. 标准错误码目录

### 5.1 Schema 校验

| Code | Severity | 含义 | 可重试 |
| --- | --- | --- | --- |
| SCHEMA_INVALID_TYPE | error | 对象 type 字段不合法 | 否 |
| SCHEMA_MISSING_REQUIRED | error | 缺少必填字段 | 否 |
| SCHEMA_VERSION_UNSUPPORTED | error | Schema 版本不受支持 | 否 |
| SCHEMA_FIELD_OUT_OF_RANGE | error | 字段值超出允许范围 | 否 |
| SCHEMA_UNKNOWN_ENUM | error | 枚举值不在允许列表中 | 否 |

### 5.2 Asset 模块

| Code | Severity | 含义 | 可重试 |
| --- | --- | --- | --- |
| ASSET_NOT_FOUND | error | 引用的资产不存在 | 否 |
| ASSET_NOT_PUBLISHED | error | 资产存在但尚未发布 | 是 |
| ASSET_NOT_WARMED | warning | 资产未预热，live 模式下有风险 | 是 |
| ASSET_STALE | warning | 资产分析结果已过期 | 是 |
| ASSET_FORMAT_UNSUPPORTED | error | 素材格式不支持 | 否 |
| ASSET_PROBE_FAILED | error | 素材探测失败 | 是 |
| ASSET_NORMALIZE_FAILED | error | 规范化失败 | 是 |
| ASSET_DUPLICATE | info | 内容哈希重复，跳过导入 | 否 |
| ASSET_MISSING_ANALYSIS | warning | 缺少指定分析结果 | 是 |

### 5.3 Analysis 模块

| Code | Severity | 含义 | 可重试 |
| --- | --- | --- | --- |
| ANALYSIS_JOB_FAILED | error | 分析任务执行失败 | 是 |
| ANALYSIS_CACHE_MISS | info | 缓存未命中，需重新分析 | 是 |
| ANALYSIS_CACHE_STALE | warning | 缓存因版本变更失效 | 是 |
| ANALYSIS_TIMEOUT | error | 分析超时 | 是 |
| ANALYSIS_PARTIAL | warning | 分析部分完成 | 是 |

### 5.4 Plan 模块

| Code | Severity | 含义 | 可重试 |
| --- | --- | --- | --- |
| PLAN_ASSET_POOL_EMPTY | error | 素材池为空 | 否 |
| PLAN_SECTION_OVERLAP | error | Section 时间范围重叠 | 否 |
| PLAN_CONSTRAINT_CONFLICT | error | 约束集内部矛盾 | 否 |
| PLAN_DURATION_EXCEEDS_LIMIT | warning | 计划时长超出目标 | 否 |
| PLAN_MISSING_VISUAL_INTENT | warning | Section 缺少视觉意图声明 | 否 |

### 5.5 Compile 模块

| Code | Severity | 含义 | 可重试 |
| --- | --- | --- | --- |
| COMPILE_ASSET_UNRESOLVED | error | 编译时无法解析资产引用 | 否 |
| COMPILE_TIMELINE_CONFLICT | error | 时间线条目冲突 | 否 |
| COMPILE_GPU_BUDGET_EXCEEDED | warning | 视觉 GPU 预算超限 | 否 |
| COMPILE_CPU_BUDGET_EXCEEDED | warning | 音频 CPU 预算超限 | 否 |
| COMPILE_DETERMINISM_VIOLATION | error | 编译结果不稳定 | 否 |

### 5.6 Revision 模块

| Code | Severity | 含义 | 可重试 |
| --- | --- | --- | --- |
| REVISION_NOT_FOUND | error | 指定 revision 不存在 | 否 |
| REVISION_NOT_PUBLISHED | error | Revision 尚未发布 | 是 |
| REVISION_ALREADY_ARCHIVED | error | Revision 已归档不可再修改 | 否 |
| REVISION_BASE_MISMATCH | error | 基础 revision 与预期不符 | 否 |

### 5.7 Run / Scheduler 模块

| Code | Severity | 含义 | 可重试 |
| --- | --- | --- | --- |
| RUN_ALREADY_ACTIVE | error | Show 已有活跃 run | 否 |
| RUN_NOT_ACTIVE | error | 无活跃 run | 否 |
| RUN_BACKEND_UNAVAILABLE | error | 音频/视觉后端不可用 | 是 |
| RUN_XRUN_DETECTED | warning | 检测到 audio xrun | 否 |
| RUN_GPU_OVERLOAD | warning | GPU 峰值超阈值 | 否 |
| RUN_SYNC_DRIFT | warning | 视听同步偏差超容差 | 否 |

### 5.8 Patch 模块

| Code | Severity | 含义 | 可重试 |
| --- | --- | --- | --- |
| PATCH_WINDOW_MISSED | error | 目标 patch 窗口已过 | 否 |
| PATCH_LOCKED_SECTION | error | 目标 section 已锁定 | 否 |
| PATCH_UNAUTHORIZED | error | 无权限执行此类 patch | 否 |
| PATCH_ASSET_NOT_WARMED | error | Patch 引用资产未预热 | 是 |
| PATCH_GPU_BUDGET_EXCEEDED | warning | Patch 将超出 GPU 预算 | 否 |
| PATCH_CPU_BUDGET_EXCEEDED | warning | Patch 将超出 CPU 预算 | 否 |
| PATCH_MISSING_FALLBACK | error | 未指定有效 fallback_revision | 否 |
| PATCH_COMPILE_FAILED | error | Patch 局部编译失败 | 否 |
| PATCH_ROLLBACK_TRIGGERED | info | 回退已触发 | 否 |
| PATCH_DEGRADED | info | 已进入降级模式 | 否 |
| PATCH_EXPIRED | warning | Patch 超时未生效已过期 | 否 |

### 5.9 Trace 模块

| Code | Severity | 含义 | 可重试 |
| --- | --- | --- | --- |
| TRACE_BUNDLE_NOT_FOUND | error | Trace bundle 不存在 | 否 |
| TRACE_RUN_NOT_FOUND | error | 指定 run 不存在 | 否 |
| TRACE_EVENT_RANGE_EMPTY | info | 查询范围内无事件 | 否 |

### 5.10 Eval 模块

| Code | Severity | 含义 | 可重试 |
| --- | --- | --- | --- |
| EVAL_TRACE_INCOMPLETE | warning | Trace 数据不完整影响评分 | 否 |
| EVAL_DIMENSION_UNSUPPORTED | error | 请求了不支持的评价维度 | 否 |

### 5.11 System 模块

| Code | Severity | 含义 | 可重试 |
| --- | --- | --- | --- |
| SYSTEM_CAPABILITY_NOT_FOUND | error | 请求的能力不存在 | 否 |
| SYSTEM_HEALTH_DEGRADED | warning | 系统组件健康状态降级 | 是 |
| SYSTEM_STORAGE_FULL | error | 存储空间不足 | 否 |

### 5.12 Auth 模块

| Code | Severity | 含义 | 可重试 |
| --- | --- | --- | --- |
| AUTH_ROLE_INSUFFICIENT | error | 当前角色权限不足 | 否 |
| AUTH_TOKEN_EXPIRED | error | 授权令牌已过期 | 是 |
| AUTH_POLICY_DENIED | error | 策略拒绝本次操作 | 否 |

## 6. 使用规范

### 6.1 CLI 输出

错误码应出现在 JSON envelope 的 diagnostics 数组中：

```json
{
  "status": "error",
  "capability": "patch.submit",
  "diagnostics": [
    {
      "code": "PATCH_LOCKED_SECTION",
      "namespace": "patch",
      "severity": "error",
      "message": "Section 'intro' is locked and cannot be patched.",
      "retryable": false
    }
  ]
}
```

### 6.2 MCP 输出

MCP tool 响应中，diagnostics 使用相同结构。

### 6.3 Trace 写入

Patch Decision、Compile Record 等 trace 对象中的 diagnostics / warnings 字段应引用同一套错误码。

### 6.4 测试断言

21 号文档要求每个错误至少验证 code、severity、retryable 和 target。测试中应直接引用本文档定义的错误码。

## 7. 扩展规范

- 新增错误码必须先确定命名空间前缀
- 错误码命名使用大写蛇形：`NAMESPACE_ACTION_REASON`
- 不允许在 code 中携带动态参数，动态信息放在 details 中
- 废弃错误码不删除，标记为 deprecated 并注明替代码

## 8. 结论

本字典覆盖 Phase 0 最小必需的错误码集合。随着系统扩展，新模块应按照本文规范扩展错误码，保持全系统诊断信息的一致性与可消费性。
