# Capability Layer 与命令执行模型详细设计

日期：2026-04-01
状态：Draft
前置阅读：10-CLI命令清单与MCP工具Schema.md，15-功能模块设计方案总览.md

## 1. 文档目的

本文细化系统的 Capability Layer 与命令执行模型。

目标是明确：

1. 外部请求如何进入系统。
2. 命令如何被解析、校验、路由、执行与回传。
3. ticket、operation、diagnostics、artifact refs 如何统一组织。

## 2. 设计原则

### 2.1 能力优先于入口

CLI、文件接口、MCP 都只是入口形式。

系统内部只认 capability，不认入口特权。

### 2.2 请求与执行解耦

Capability Layer 不承担具体业务执行，而承担：

- 请求标准化
- 身份与权限上下文附着
- 执行计划路由
- 响应 envelope 生成

### 2.3 同一能力只应有一套语义

例如：

- `patch.submit`
- `compile.run`
- `trace.query`

这些能力无论来自 CLI 还是 MCP，都必须具有相同的输入语义、状态变化和输出结构。

## 3. 模块边界

### 3.1 Capability Layer 负责什么

- 入口适配
- 输入对象解析
- schema 预校验
- capability 路由
- request_id / operation_id / ticket_id 分配
- 响应 envelope 生成
- 执行期错误封装

### 3.2 Capability Layer 不负责什么

- 不做编译逻辑
- 不做 patch 决策终裁
- 不直接改 Runtime 内部状态
- 不直接写 Timeline 或 Revision 元数据

## 4. 内部子模块

### 4.1 Request Adapter

职责：

- 将 CLI 参数、文件输入、MCP arguments 归一化为统一请求对象

输入：

- argv
- 文件路径
- JSON payload
- MCP tool arguments

输出：

- CapabilityRequest

### 4.2 Request Normalizer

职责：

- 补齐默认值
- 解析文件引用
- 规范化 schema 版本与对象头

输出对象示例：

```json
{
  "request_id": "req-018",
  "capability": "plan.submit",
  "actor": {
    "actor_id": "operator-a",
    "role": "planner"
  },
  "payload": {},
  "metadata": {
    "source": "cli",
    "received_at": 1711942100
  }
}
```

### 4.3 Authorization Context Resolver

职责：

- 解析当前调用身份
- 注入 role、policy profile、allowed capability set

### 4.4 Capability Router

职责：

- 将能力映射到对应执行器

典型路由：

- `asset.*` -> Asset Service
- `compile.*` -> Compiler Service
- `patch.*` -> Patch Service
- `trace.*` -> Trace Query Service

### 4.5 Operation Coordinator

职责：

- 负责同步调用与异步任务的统一编排
- 生成 operation_id
- 跟踪 operation state

### 4.6 Response Builder

职责：

- 生成统一 envelope
- 填充 diagnostics、artifacts、next_actions

## 5. 核心对象模型

### 5.1 CapabilityRequest

```json
{
  "request_id": "req-018",
  "capability": "compile.run",
  "actor": {
    "actor_id": "planner-a",
    "role": "planner"
  },
  "payload": {
    "submission_id": "subm-018",
    "base_revision": 11
  },
  "metadata": {
    "source": "cli",
    "trace_parent": "req-017"
  }
}
```

### 5.2 CapabilityResponse

```json
{
  "status": "accepted",
  "capability": "compile.run",
  "request_id": "req-018",
  "data": {
    "compiler_run_id": "compile-032",
    "candidate_revision": 12
  },
  "diagnostics": [],
  "artifacts": ["compile/timeline.json"],
  "next_actions": ["run compile.inspect"]
}
```

### 5.3 OperationTicket

```json
{
  "operation_id": "op-104",
  "request_id": "req-018",
  "capability": "compile.run",
  "state": "running",
  "started_at": 1711942101,
  "updated_at": 1711942103
}
```

### 5.4 CapabilityDiagnostic

字段建议：

- code
- severity
- message
- target
- retryable
- details

## 6. 命令执行模型

### 6.1 同步命令

典型能力：

- `asset.list`
- `run.status`
- `trace.query`

执行方式：

- 请求进入后立即路由
- 无需 operation ticket
- 直接返回 CapabilityResponse

### 6.2 异步命令

典型能力：

- `asset.ingest`
- `compile.run`
- `replay.start`
- `eval.run`

执行方式：

- 请求进入后创建 operation_id
- 返回 accepted 响应
- 调用方后续查询状态或等待 artifact 生成

### 6.3 条件幂等命令

典型能力：

- `plan.submit`
- `patch.submit`
- `run.start`

要求：

- 如果请求主体与 submission_id 相同，可返回已有结果
- 如果上下文状态变化，则生成新 ticket 或新决策

## 7. 能力注册模型

### 7.1 Capability Descriptor

建议字段：

- capability
- input_schema
- output_schema
- execution_mode
- idempotency
- authorization
- target_service

### 7.2 注册表示例

```json
{
  "capability": "patch.check",
  "execution_mode": "sync",
  "idempotency": "idempotent",
  "authorization": ["planner", "operator"],
  "target_service": "patch_service"
}
```

## 8. 状态机

### 8.1 Request State

```text
Received
  -> Normalized
  -> Authorized
  -> Routed
  -> Executing
  -> Completed
```

失败分支：

```text
Received -> Rejected
Executing -> Failed
Executing -> TimedOut
```

### 8.2 Operation State

```text
Pending
  -> Running
  -> Succeeded
  -> Failed
  -> Cancelled
```

## 9. 错误与诊断模型

### 9.1 错误层级

- request_parse_error
- schema_invalid
- unauthorized
- unsupported_capability
- downstream_failure
- internal_error

### 9.2 返回原则

- 输入错误：直接 error 响应
- 下游业务拒绝：返回 warning 或 error，并带 diagnostics
- 异步任务失败：operation status 标记 failed，保留 failure diagnostics

## 10. 与 CLI / MCP 的映射

### 10.1 CLI

CLI 仅负责：

- 参数采集
- 本地文件读取
- 请求发起
- envelope 输出格式化

### 10.2 MCP

MCP adapter 仅负责：

- tool arguments 与 CapabilityRequest 的转换
- 输出 envelope 到 tool result

## 11. MVP 范围

MVP 阶段建议优先实现以下能力：

- `asset.ingest`
- `asset.list`
- `plan.validate`
- `plan.submit`
- `compile.run`
- `run.start`
- `run.status`
- `patch.check`
- `patch.submit`
- `patch.rollback`
- `trace.query`
- `eval.run`

## 12. 验收标准

Capability Layer 可视为达标，当且仅当：

1. CLI 与 MCP 可走同一 capability surface。
2. 所有响应都可输出统一 envelope。
3. 异步能力都有 operation 状态可查。
4. 所有失败都有结构化 diagnostics。

## 13. 结论

Capability Layer 的价值不在“多一个网关”，而在于把整个系统的外部调用面变成稳定、可追踪、可演进的能力接口。
