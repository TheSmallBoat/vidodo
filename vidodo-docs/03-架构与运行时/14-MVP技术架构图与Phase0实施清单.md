# MVP 技术架构图与 Phase 0 实施清单

日期：2026-04-01
状态：Draft
前置阅读：02-视听系统产品定位与规划方案书.md，11-从素材接入到实时补丁回退的端到端示例.md，13-产品技术栈决策依据建议书.md

## 1. 文档目的

本文把前面各文档中的产品边界、技术选型和最小闭环目标，收敛为一份可执行的 MVP 实施蓝图。

本文回答两件事：

1. MVP 在工程上应该长什么样。
2. Phase 0 第一轮实现应该按什么顺序做，以及做到什么程度算完成。

## 2. MVP 目标边界

### 2.1 MVP 要证明的事情

MVP 不是要证明“完整演出系统已经完成”，而是要证明下面这条最小闭环成立：

```text
外部规划者提交结构化工件
  -> 系统完成校验与编译
  -> 音频 Runtime 与视觉 Runtime 依据统一时间语义执行
  -> 系统产出 trace 与导出结果
  -> 必要时可进行受限 patch 与安全回退
```

### 2.2 MVP 明确不做的事情

- 不做完整 GUI 产品
- 不做开放式自然语言创作系统
- 不做完整 DAW 替代
- 不自研完整音频 DSP 图
- 不把在线模型推理接入实时热路径

## 3. MVP 技术架构图

### 3.1 总体架构

```text
+--------------------------------------------------------------+
| External Planning Surface                                    |
|                                                              |
|  [CLI avctl]    [Plan / DSL / Patch Files]    [MCP Adapter]  |
+----------------------------+---------------------------------+
               |
               v
+--------------------------------------------------------------+
| Core Control Service - Rust                                  |
|                                                              |
|  [Capability Layer]                                          |
|      |                                                       |
|      +--> [Validators]                                       |
|      +--> [Compiler] -----------+                            |
|      +--> [Patch Manager] -----+ |                           |
|      +--> [Trace Writer] <-----|-+-----------+               |
|                                |             |               |
|                                v             v               |
|                           [Scheduler] --> [Audio Runtime] ---+----> [Offline Export]
|                                |                              |             |
|                                +----------> [Visual Runtime] -+             v
+--------------------------------------------------------------+        [Asset / Export Files]

+--------------------------------------------------------------+
| Analysis Toolchain - Python                                  |
|                                                              |
|  [Asset Ingestion] --> [Audio / Structure Analysis]          |
|          |                         |                         |
|          |                         v                         |
|          +--------------------> [Asset Registry] ------------+---> [Compiler]
|                                               |              +---> [Patch Manager]
+-----------------------------------------------+--------------+
                        |
                        v
+--------------------------------------------------------------+
| Artifact Store                                                |
|                                                              |
|  [JSON / JSONL Artifacts]   [SQLite Index]   [Asset Files]   |
+----------------------+----------------------+-----------------+
             |                      |
             |                      +<---- [Offline Export]
             |
             +----> [Evaluation]
             +----> [Replay]

Trace / artifact write path:
  Compiler ---------> JSON / JSONL Artifacts
  Patch Manager ----> JSON / JSONL Artifacts
  Trace Writer -----> JSON / JSONL Artifacts
  Capability Layer -> SQLite Index
  Asset Registry ---> SQLite Index
  Asset Ingestion --> Asset Files
```

### 3.2 架构图解读

这张图里最重要的工程判断有四个：

1. 外部入口只进入 capability layer，不直接碰 Runtime。
2. 分析链路与实时链路分离，Python 只在非实时面运行。
3. Scheduler 是唯一进入双 Runtime 的统一下发点。
4. Trace、artifact、SQLite 共同构成 MVP 的可追踪基础。

## 4. MVP 组件划分

### 4.1 必需组件

| 组件 | 语言 / 运行环境 | MVP 角色 |
| --- | --- | --- |
| CLI avctl | Rust | 提交命令、查看状态、触发 patch / replay / eval |
| Core Control Service | Rust | capability surface、revision、ticket、调度控制 |
| Validators | Rust | schema、资源、锁定区间、patch 准入检查 |
| Compiler | Rust | Planning DSL -> IR -> Timeline |
| Scheduler | Rust | 统一时钟、lookahead、事件下发 |
| Patch Manager | Rust | patch 生命周期、授权、回退 |
| Asset Ingestion | Python + Rust glue | 素材接入、分析任务调度、资产发布 |
| Analysis Workers | Python | beat、section、key、特征分析 |
| Audio Runtime | 成熟音频引擎 | 音频回放、自动化、导出 |
| Visual Runtime | Rust + wgpu | scene、uniform、camera、render、多视角 view set |
| Artifact Store | JSON / JSONL / SQLite / 文件目录 | 工件、索引、trace、导出 |

补充约束：

- Visual Runtime 应在 Core 内部支持可替换的 output backend 抽象。
- MVP 至少明确 `flat_display_backend` 与 `spatial_multiview_backend` 的接口边界。
- Audio Runtime 也应为常规系统输出与空间扬声器矩阵输出保留后端抽象接口。

### 4.2 可后置组件

- Operator Console
- 服务化 API
- 复杂权限系统
- 第三方插件机制
- 在线协作与远程多机调度

## 5. MVP 推荐目录布局

```text
av-system/
  apps/
    avctl/
    core-service/
    visual-runtime/
    mcp-adapter/
  crates/
    ir/
    validator/
    compiler/
    scheduler/
    patch-manager/
    trace/
    storage/
  python/
    ingestion/
    analyzers/
  schemas/
    planning/
    runtime/
    trace/
  artifacts/
    assets/
    analysis/
    traces/
    exports/
    registry.db
```

## 6. Phase 0 实施策略

### 6.1 Phase 0 的总目标

Phase 0 只做一件事：

```text
证明最小闭环可跑通，并且每一步都有结构化工件和证据链。
```

### 6.2 Phase 0 交付口径

Phase 0 完成时，团队应能演示：

1. 导入一小组音频与视觉素材。
2. 提交一份 SetPlan + AudioDsl + VisualDsl。
3. 编译出 Structure IR、Performance IR、Visual IR 与 Unified Timeline。
4. 启动一次 live 或 offline run。
5. 输出 trace bundle 与导出产物。
6. 提交一次受限 patch，并在异常情况下回退。

## 7. Phase 0 实施清单

### 7.1 Workstream A：Schema 与工件基础

目标：先把工件面固定住，防止后面一边写代码一边漂移对象定义。

清单：

- 定义 SetPlan 最小 schema
- 定义 AudioDsl 最小 schema
- 定义 VisualDsl 最小 schema
- 定义 AssetRecord / Asset IR 最小 schema
- 定义 TimelineEntry 最小 schema
- 定义 LivePatchProposal 最小 schema
- 定义 Trace Bundle manifest 最小 schema

完成标准：

- 所有示例文件都能通过 schema 校验
- CLI 能输出一致的 validation diagnostics

### 7.2 Workstream B：资产接入与分析

目标：把“文件”变成“可引用的资产对象”。

清单：

- 实现单目录素材发现
- 生成 content hash 与基础 probe 信息
- 生成 normalized locator
- 建立 SQLite 资产索引表
- 打通 1 到 2 个音频分析任务，例如 beat_track、section_segmentation
- 为视觉素材建立最小 registry 记录

完成标准：

- CLI 可列出已发布资产
- 编译器只能引用已发布资产
- patch 检查可识别 asset_not_warmed / asset_missing

### 7.3 Workstream C：Compiler 与 Timeline

目标：把规划工件转成最小可执行时间线。

清单：

- 实现 Planning DSL 读取
- 实现基础 validator
- 生成 Structure IR
- 生成 Performance IR
- 生成 Visual IR
- 生成 Unified Timeline
- 输出 candidate revision 与 artifact refs

完成标准：

- 同一输入重复编译结果稳定
- timeline entry 含 revision、musical_time、priority、conflict_group

### 7.4 Workstream D：Scheduler 与双 Runtime 骨架

目标：让统一时间语义真正进入执行层。

清单：

- 实现最小 musical clock
- 实现 lookahead 发布机制
- 实现 Audio Runtime 控制桥
- 实现 Visual Runtime 控制桥
- 定义并打通 timing、audio、visual 三类最小事件
- 维护最小 Show State
- 为多视角 visual output backend 预留 display topology 与 calibration profile
- 为音频输出后端预留 speaker matrix topology 与 calibration profile

完成标准：

- Audio 与 Visual 都能消费同一 revision 的时间线
- section / phrase 事件能同时到达两个 Runtime
- Visual Runtime 能以单 scene 输出至少一个可切换 view group
- Runtime 工件中能表达 display topology / speaker topology 的引用

### 7.5 Workstream E：Trace 与导出

目标：让 MVP 不是“跑出来了”，而是“跑完能复盘”。

清单：

- 记录 run manifest
- 记录 event_record JSONL
- 记录 patch decision JSONL
- 记录最小 resource sample
- 输出一次离线导出结果
- 生成 trace bundle 目录结构

完成标准：

- 能按 run_id 找到完整 trace bundle
- 能按 bar 区间查询事件记录

### 7.6 Workstream F：Patch 与回退最小闭环

目标：证明 live patch 不是空概念。

清单：

- 实现 patch.check
- 实现 patch.submit
- 实现 patch window 检查
- 实现 fallback_revision 检查
- 实现 deferred rollback
- 把 patch decision 写入 trace

完成标准：

- 能成功演示一次 local_content patch
- 能因预设问题触发一次 rollback

## 8. 推荐实施顺序

### 8.1 顺序建议

按下面顺序推进最稳妥：

1. Schema 与 artifact store
2. Asset ingestion 与 registry
3. Compiler 与 revision
4. Scheduler 与最小 Show State
5. Audio / Visual Runtime bridge
6. Trace bundle
7. Patch / rollback
8. Evaluation 占位输出

### 8.2 为什么这样排序

因为这条顺序尽量先固定“事实层”，再进入“执行层”。

如果一开始就直奔 Runtime，后续最容易返工的是：

- schema 漂移
- revision 语义不清
- trace 记录缺失
- patch 无法补上

## 9. Phase 0 角色分工建议

### 9.1 最小团队分工

| 角色 | 主要负责 |
| --- | --- |
| Core Engineer | schema、compiler、scheduler、patch manager |
| Runtime Engineer | audio bridge、visual runtime、event ingress |
| Tools Engineer | CLI、artifact store、trace、SQLite、脚本化集成 |
| Analysis Engineer | ingestion、音频分析、资产发布 |

如果人手更少，可以先由 2 人承担：

- 一人负责 Core + CLI + Trace
- 一人负责 Runtime + Ingestion + Analysis

## 10. Phase 0 演示脚本建议

### 10.1 推荐演示流程

```text
1. 导入 4 个音频素材和 2 个视觉素材
2. 展示 asset list
3. 提交并校验一份 plan bundle
4. 编译得到 revision 12
5. 启动 run-03
6. 展示 show state 与 patch window
7. 提交 patch-018
8. 展示 patch activated
9. 触发 deferred rollback
10. 展示 trace bundle 与导出结果
```

### 10.2 演示成功标准

- 每一步都有可展示的 artifact 或状态输出
- 没有任何步骤依赖手工改 Runtime 内存状态
- patch 与 rollback 能在 trace 中被查询到

## 11. Phase 0 风险清单

### 11.1 高风险点

- 过早陷入音频底层实现细节
- 视觉 Runtime 先做太重，拖慢闭环
- 没有先固定 schema 导致编译器频繁重写
- patch 只做表层接口，没有真实回退能力

### 11.2 控制策略

- 音频优先复用成熟引擎
- 视觉先做最小 scene / uniform / camera 三件套
- trace 与 revision 必须从第一周开始存在
- patch 只支持 local_content 类，暂不扩展 structural 类

## 12. Phase 0 验收标准

Phase 0 可以视为完成，当且仅当以下条件同时满足：

1. 一套有限素材可被稳定 ingest、分析并发布为资产。
2. 一份规划工件可被编译为稳定 revision 和 unified timeline。
3. Audio Runtime 与 Visual Runtime 可在同一时间语义下执行。
4. 系统可导出 trace bundle，并支持最小查询。
5. 一次 live patch 与一次 rollback 可被成功演示和追踪。

## 13. Phase 1 前的自然延伸

当 Phase 0 完成后，最自然的 Phase 1 入口是：

- 扩展共享 Show State
- 强化视听联动协议执行
- 引入更完整的 evaluation
- 加入 MCP tool surface 的真正对外接入

## 14. 结论

MVP 阶段最重要的不是把所有功能都做出来，而是把最小闭环做正确。

关键判断如下：

1. MVP 架构必须从一开始就保留双 Runtime、统一 revision、统一 trace 这三条主骨架。
2. Phase 0 的第一优先级是“结构化工件闭环”，不是“界面可见度”或“效果复杂度”。
3. 只要 Phase 0 能稳定演示 ingest -> compile -> run -> patch -> rollback -> trace，这条路线就是成立的。

这份文档可直接作为 MVP 启动会或第一轮工程拆解的基础版本。
