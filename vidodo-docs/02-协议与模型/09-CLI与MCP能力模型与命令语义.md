# CLI / MCP 能力模型与命令语义

日期：2026-04-01
状态：Draft
前置阅读：03-外部规划与介入机制顶层设计书.md，05-DSL与IR字段定义草案.md，08-实时补丁授权与回退策略.md

## 1. 文档目的

本文定义 CLI 与 MCP 的共享能力模型、命令分类、输入输出约定与语义边界。

目标是让两类调用方都成立：

- 人类操作员通过 CLI 提交、查询与控制
- 外部 Agent 通过 MCP 发现能力并提交相同结构化请求

系统必须保持一个原则：

```text
CLI 与 MCP 只是两种入口，同一能力只能有一套底层语义。
```

## 2. 顶层设计结论

### 2.1 先定义能力模型，再定义命令文本

如果先设计 CLI 子命令，再补 MCP 工具，最终一定会出现：

- 能力不对齐
- 输出结构不一致
- 权限模型分裂
- 自动化脚本与 Agent 行为不等价

因此，应先定义统一能力对象，再分别映射到 CLI 和 MCP。

### 2.2 命令语义必须偏事务，而不是偏聊天

每个命令都应清楚回答：

- 它读取什么
- 它修改什么
- 它返回什么工件或状态
- 它是否幂等
- 它是否异步

## 3. 能力模型

### 3.1 推荐能力命名空间

| 命名空间 | 作用 |
| --- | --- |
| asset | 素材接入、查询、分析预热 |
| plan | 规划提交、校验、查看 |
| compile | IR 生成、验证、diff |
| run | 启动、暂停、停止、状态查询 |
| patch | patch 预检、提交、审批、回退 |
| trace | 查询 trace、checkpoint、事件 |
| replay | 发起重放与比较 |
| eval | 发起评价、查询报告 |
| system | 能力发现、健康检查、策略查询 |

### 3.2 能力元数据

每个能力都应声明以下元信息：

```json
{
  "capability": "patch.submit",
  "version": "0.1",
  "input_schema": "av.patch.submit.v0",
  "output_schema": "av.patch.ticket.v0",
  "side_effect": "mutating",
  "idempotency": "conditional",
  "async": true,
  "authorization": "operator_or_policy",
  "artifacts": ["patch_ticket", "audit_record"]
}
```

## 4. CLI 设计草案

### 4.1 CLI 名称与总体形式

本文用 avctl 作为占位 CLI 名称。

总体形式：

```text
avctl <namespace> <verb> [args] [flags]
```

设计原则：

- namespace 固定能力域
- verb 表达动作
- 默认输出结构化 JSON
- 人类可通过 --format table 获取可读视图

### 4.2 核心命令族

#### asset

```text
avctl asset ingest --from imports/session_a --kind audio_loop
avctl asset list --tag kick --json
avctl asset analyze --asset asset-loop-kick-a --profile live-min
avctl asset warm --asset asset-loop-kick-a
```

#### plan

```text
avctl plan validate --set-plan plans/set-plan-a.json --audio dsl/audio-a.json --visual dsl/visual-a.json
avctl plan submit --bundle plan-bundle.json
avctl plan show --show-id show-2026-04-01-a
```

#### compile

```text
avctl compile run --submission subm-018
avctl compile diff --from-revision 11 --to-revision 12
avctl compile inspect --revision 12 --artifact timeline
```

#### run

```text
avctl run start --revision 12 --mode live
avctl run status --show-id show-2026-04-01-a
avctl run stop --show-id show-2026-04-01-a --policy graceful
```

#### patch

```text
avctl patch check --file patches/patch-018.json
avctl patch submit --file patches/patch-018.json
avctl patch approve --patch-id patch-018
avctl patch rollback --patch-id patch-018 --mode deferred
```

#### trace / replay / eval

```text
avctl trace show --run-id run-03
avctl trace events --run-id run-03 --from-bar 129 --to-bar 145
avctl replay start --trace-bundle trace-show-a-rev12-run03 --mode deterministic
avctl eval run --trace-bundle trace-show-a-rev12-run03
```

### 4.3 输入输出约定

默认约定如下：

- 输入优先接受文件路径或显式 JSON
- 输出默认 JSON 到 stdout
- 大工件返回 artifact refs，不强行把大对象直接打印到终端
- 非 0 exit code 只用于命令失败，不用于业务警告

## 5. MCP 能力模型草案

### 5.1 MCP 工具不应复制 CLI 字符串，而应暴露结构化能力

例如，CLI 命令：

```text
avctl patch submit --file patches/patch-018.json
```

在 MCP 中应表达为：

```json
{
  "tool": "patch.submit",
  "arguments": {
    "patch_ref": "patches/patch-018.json"
  }
}
```

### 5.2 推荐 MCP 工具集合

- system.describe_capabilities
- asset.ingest
- asset.list
- asset.inspect
- asset.warm
- plan.validate
- plan.submit
- compile.run
- compile.diff
- run.status
- patch.check
- patch.submit
- patch.approve
- patch.rollback
- trace.query
- replay.start
- eval.run

### 5.3 MCP 工具响应约定

每个 MCP 工具响应建议包含：

- status
- revision 或 ticket_id
- artifact_refs
- diagnostics
- next_actions

示例：

```json
{
  "status": "accepted",
  "ticket_id": "patch-ticket-018",
  "artifact_refs": ["patches/ticket-018.json"],
  "diagnostics": [],
  "next_actions": ["await_patch_window"]
}
```

## 6. 命令语义分类

### 6.1 Query 命令

只读，不改变系统状态。

例如：

- asset list
- plan show
- run status
- trace show
- trace events

要求：

- 幂等
- 可缓存
- 默认可被低权限角色调用

### 6.2 Validate 命令

进行检查但不提交状态变更。

例如：

- plan validate
- patch check
- compile diff

要求：

- 返回结构化 diagnostics
- 不产生新 revision
- 可用于人类或 Agent 预检

### 6.3 Mutating 命令

改变系统状态或生成新工件。

例如：

- asset ingest
- plan submit
- compile run
- patch submit
- patch approve
- run start

要求：

- 返回 ticket、revision 或 run_id
- 明确 side effect
- 记录审计条目

### 6.4 Long-running 命令

异步完成，需要 ticket 轮询或事件订阅。

例如：

- asset analyze
- compile run
- replay start
- eval run

要求：

- 返回 operation_id
- 提供查询状态的对应能力
- 不要求调用方持续保持会话

## 7. 错误模型

### 7.1 统一错误对象

建议 CLI 与 MCP 共享同一种错误结构：

```json
{
  "error": {
    "code": "patch_window_missed",
    "message": "Patch window next_phrase_boundary has already passed.",
    "retryable": true,
    "details": {
      "next_window": "bar 145"
    }
  }
}
```

### 7.2 错误分层

| 错误层 | 示例 |
| --- | --- |
| Input Error | schema_invalid、missing_required_field |
| Policy Error | unauthorized_patch_class、locked_section |
| Resource Error | gpu_budget_exceeded、asset_not_warmed |
| Runtime Error | runtime_unreachable、activation_timeout |
| Internal Error | compiler_crash、storage_write_failed |

## 8. 幂等性与版本语义

### 8.1 幂等规则

- Query 命令必须幂等
- Validate 命令必须幂等
- Mutating 命令应显式声明是否幂等
- 相同 submission_id 的重复提交可选择返回已有结果

### 8.2 Revision 语义

CLI / MCP 的所有 mutating 能力都应明确：

- 基于哪个 base_revision
- 生成哪个 candidate_revision
- 最终生效 revision 是什么

## 9. 权限与授权语义

### 9.1 权限不是入口私有逻辑

CLI 和 MCP 必须共用同一套权限决策。

例如：

- human operator 通过 CLI 批准高风险 patch
- 外部 Agent 通过 MCP 提交同一 patch proposal
- 最终都进入同一 Patch Manager 授权链路

### 9.2 建议权限级别

- read_only
- planner
- operator
- recovery_controller
- admin

## 10. 能力发现与自描述

### 10.1 system.describe_capabilities

系统应提供自描述能力，至少返回：

- 支持的 namespaces
- 每个能力的 schema 版本
- 当前 mode 是否支持该能力
- 权限要求
- 是否异步

### 10.2 为什么自描述很关键

没有能力发现，外部 Agent 就只能硬编码命令假设，容易在版本演进时失效。

## 11. 最小命令映射表

| 能力 | CLI | MCP |
| --- | --- | --- |
| 提交规划 | avctl plan submit | plan.submit |
| 编译 revision | avctl compile run | compile.run |
| 查询运行状态 | avctl run status | run.status |
| 提交 patch | avctl patch submit | patch.submit |
| 批准 patch | avctl patch approve | patch.approve |
| 回退 patch | avctl patch rollback | patch.rollback |
| 查询 trace | avctl trace show | trace.query |
| 发起 replay | avctl replay start | replay.start |
| 发起 evaluation | avctl eval run | eval.run |

## 12. 结论

CLI / MCP 的关键不是做两个接口，而是做一套共享能力语义的双入口系统。

关键判断如下：

1. 先定义 capability，再定义 CLI 子命令和 MCP 工具。
2. Query、Validate、Mutating、Long-running 四类命令必须分清。
3. 错误模型、权限模型、revision 语义和 artifact 返回格式必须统一。

这样，人类与外部 Agent 才能真正通过同一套系统边界稳定协作。
