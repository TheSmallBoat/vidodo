# Phase 1 验收报告

> 日期：2026-04-03  
> 里程碑：M10 — Phase 1 验收  
> 前置里程碑：M7（能力层抽象建立）、M8（服务层可远程调用）、M9（三通道运行时可执行）

---

## 1. 验收目标

| 验收标准 | 结果 |
|----------|------|
| CLI、HTTP、MCP 三入口执行同一能力语义并产出一致工件 | ✅ 34 项检查全部通过 |
| audio + visual + lighting 三运行时可统一调度 | ✅ 同一 revision 产出 events.jsonl + visual-acks.json + lighting-acks.json |
| 交付物：Phase 1 验收报告与演示脚本 | ✅ 本文档 + `tests/e2e/phase1_acceptance.sh` |

---

## 2. 任务卡完成状态

Phase 1 共计 15 张任务卡，覆盖 Workstream H（能力层）、I（MCP 工具面）、J（灯光运行时），全部 `done`。

### Workstream H — Capability Layer

| 卡号 | 标题 | 状态 |
|------|------|------|
| WSH-01 | 定义 Capability 类型与 Schema | done |
| WSH-02 | 实现 capability crate 与能力注册表 | done |
| WSH-03 | 迁移 avctl 到 capability 路由 | done |
| WSH-04 | 实现 Operation Tracker | done |
| WSH-05 | 实现 core-service HTTP 骨架 | done |
| WSH-06 | 包装 asset/plan/compile 能力面 | done |
| WSH-07 | 包装 run/patch/trace/eval/export 能力面 | done |

### Workstream I — MCP 工具面

| 卡号 | 标题 | 状态 |
|------|------|------|
| WSI-01 | 定义 MCP tool 与 capability 映射规则 | done |
| WSI-02 | 实现 mcp-adapter 工具主机骨架 | done |
| WSI-03 | 实现 system.describe_capabilities 工具 | done |
| WSI-04 | MCP 端到端集成测试 | done |

### Workstream J — 灯光运行时骨架

| 卡号 | 标题 | 状态 |
|------|------|------|
| WSJ-01 | 扩展灯光 IR 类型 | done |
| WSJ-02 | 实现 lighting-runtime 事件消费者 | done |
| WSJ-03 | 灯光 patch 检查路径 | done |
| WSJ-04 | 灯光端到端集成测试 | done |

---

## 3. 里程碑达标确认

### M7：能力层抽象建立 ✅

- `crates/capability/` 含 21 个能力描述符、路由器（21 条 `RouteTarget` 路由）、操作跟踪器（真实时间戳 + `start_if_async` 门控）、MCP 工具映射（21 条 + `resolve_mcp_tool`）
- `avctl system capabilities` 输出 21 个能力描述符 JSON
- 所有 Phase 0 E2E 测试不降级

### M8：服务层可远程调用 ✅

- `core-service` axum HTTP 服务器（`GET /health`、`GET /capabilities`、`POST /capability/{id}`），全部 21 个能力可 HTTP 调用
- `mcp-adapter` stdio JSON-RPC 工具主机（`initialize`、`tools/list`、`tools/call`），全部 21 个 MCP tool 可调用
- MCP Agent 可发现（`tools/list` 返回 21 个 tool 含 `inputSchema`）并提交 `plan.validate` → `compile.run` 完整工作流
- `mcp_e2e.sh`（17 项检查）+ `phase1_acceptance.sh` MCP 段（6 项检查）全部通过

### M9：三通道运行时可执行 ✅

- `visual-runtime` 消费 Visual + Timing 事件，输出 `visual-acks.json`（含 `rendered` + `synced`）
- `lighting-runtime` 消费 Lighting + Timing 事件，输出 `lighting-acks.json`（含 `cue_executed` + `synced`）
- 三个运行时（audio 在 `run start` 内联、visual + lighting 独立进程）均可处理同一 revision 时间线

---

## 4. 三入口语义等价验证

`tests/e2e/phase1_acceptance.sh` 对同一 `plan.validate` → `compile.run` 工作流进行 CLI / HTTP / MCP 三入口对比：

| 验证维度 | CLI | HTTP | MCP |
|----------|-----|------|-----|
| `show_id` | show-phase0-minimal | show-phase0-minimal | show-phase0-minimal |
| `timeline_entries` | 一致 | 一致 | 一致 |
| `capability count` | 21 | 21 | 21 |
| `plan.validate` status | ok | ok | ok |
| `compile.run` status | ok | ok | ok |

---

## 5. 测试覆盖

| 指标 | 数量 |
|------|------|
| Rust 单元 / 集成测试 | 94 |
| Schema fixture（valid + invalid） | 75 |
| E2E phase0_smoke | passing |
| E2E asset_ingest_smoke | passing |
| E2E mcp_e2e | 17 项 passing |
| E2E negative_paths | 24 项 passing |
| E2E phase1_acceptance | 34 项 passing |

---

## 6. 工件拓扑

```
vidodo-src/
├── crates/
│   ├── ir/           50+ 公共类型（含灯光、能力、补丁）
│   ├── validator/    11 校验码
│   ├── compiler/     编译 + revision 生命周期
│   ├── scheduler/    clock + lookahead + show_state
│   ├── patch-manager/ 4 阶段（含灯光 fixture 校验）
│   ├── storage/      SQLite + dual analyzer ingest
│   ├── trace/        JSONL + patch-decisions + resource-samples + export + bar filter
│   ├── capability/   21 描述符 + 路由器 + tracker + MCP 映射
│   └── evaluation/   4D 评分
├── apps/
│   ├── avctl/            CLI 主入口
│   ├── core-service/     HTTP 能力服务（axum 0.8）
│   ├── mcp-adapter/      MCP stdio JSON-RPC 工具主机
│   ├── visual-runtime/   视觉事件消费者
│   └── lighting-runtime/ 灯光事件消费者
```

---

## 7. 已知限制（明确延后）

| 限制 | 延后至 |
|------|--------|
| 真实硬件后端集成 | Phase 2 |
| 分布式多节点运行时 | Phase 2 |
| GUI 产品 | Phase 2+ |
| 完整权限体系 | Phase 2 |
| revision.publish / revision.archive HTTP handler（当前返回 stub） | Phase 2 |

---

## 8. 演示脚本

```bash
# 完整三入口 + 三运行时验收
bash tests/e2e/phase1_acceptance.sh

# 已有的独立验证
bash tests/e2e/phase0_smoke.sh
bash tests/e2e/mcp_e2e.sh
bash tests/e2e/asset_ingest_smoke.sh
bash tests/e2e/negative_paths.sh
```

---

## 9. 结论

Phase 1 全部 15 张任务卡 `done`，M7/M8/M9 三个里程碑均达标。CLI、HTTP、MCP 三入口产出语义等价工件，audio + visual + lighting 三运行时可在同一 revision 上统一调度。Phase 1 验收通过。
