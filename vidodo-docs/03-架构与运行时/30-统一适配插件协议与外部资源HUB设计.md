# 统一适配插件协议与外部资源 HUB 设计

日期：2026-04-02
状态：Draft
前置阅读：02-视听系统产品定位与规划方案书.md，03-外部规划与介入机制顶层设计书.md，15-功能模块设计方案总览.md，19-调度运行时与补丁详细设计.md，27-视觉后端抽象与多视角空间显示装置设计.md，28-音频输出后端抽象与空间扬声器矩阵装置设计.md，29-灯光输出后端抽象与离散式分布式灯光装置设计.md

## 1. 文档目的

27、28、29 号文档已经分别把视觉、音频、灯光后端抽象清晰化，但当前文档体系还缺少一条更上层的统一设计原则：

1. 硬件装置实现可以不同，但系统必须维持统一抽象与统一协议边界。
2. 离散式视觉 / 声场 / 灯光装置应被理解为支持统一协议设定的适配型插件。
3. 音频素材、GLSL 代码、3D 场景模型等应被视为可替换的外部资源，并以 HUB 形式组织。
4. 系统核心应保持精简，把设备差异与资源多样性推到边界层处理。

本文的目标是把这条设计意图提升为全系统级约束。

## 2. 顶层设计结论

### 2.1 核心必须小，边界必须强

Vidodo 的核心不应随着装置类型和资源形态不断膨胀。

核心只负责：

- 规划工件校验
- 编译为统一 DSL / IR / Timeline
- 维护统一 Show State 与调度语义
- 驱动执行后端
- 记录 trace、回放与回退证据链

核心不应负责：

- 把具体硬件协议写死进 Runtime Core
- 把某类素材格式写死进编译器与调度器
- 为每种设备和资源类型单独发明一套控制语义

### 2.2 硬件差异通过适配型插件承接

视觉、音频、灯光三类装置都应遵循同一原则：

```text
Unified Control Contract
  -> Adapter Plugin
  -> Concrete Device / Device Topology
```

其中：

- `Unified Control Contract` 定义核心对后端的统一语义边界
- `Adapter Plugin` 负责协议翻译、能力声明、端点发现、状态回报
- `Concrete Device / Device Topology` 是具体屏幕、扬声器、灯具或桥接系统

### 2.3 资源差异通过外部 HUB 承接

音频素材、GLSL 代码、3D 场景模型、贴图、灯光 cue 模板等，不应被当作核心代码的一部分长期内置。

更合理的结构是：

```text
External Resource Hubs
  -> Resource Manifest / Metadata / Analysis Result
  -> Resolver / Registry / Cache
  -> Compiler / Runtime Reference
```

核心只依赖稳定引用和能力描述，不依赖某个具体资源包必须内嵌在仓库中。

### 2.4 系统一致性来自“统一协议”，不是“统一实现”

对 Vidodo 而言，真正需要保持统一的是：

- Show State
- 时间模型
- DSL / IR / Timeline
- patch / rollback 边界
- diagnostics / trace / replay 证据链

而不需要统一的是：

- 屏幕控制驱动实现
- 音频设备接入实现
- 灯光协议与桥接实现
- 素材包、Shader 包、场景包的来源和组织方式

## 3. 统一抽象设计

### 3.1 两类边界对象

整个系统应明确区分两类可替换边界：

1. `Adapter Plugin`：面向设备与执行端点
2. `Resource Hub`：面向内容与资源集合

两者都可替换，但职责完全不同。

### 3.2 Adapter Plugin 的职责

Adapter Plugin 负责：

- 声明自身 backend kind 与 capability
- 接收核心发出的统一控制对象
- 映射到具体设备协议、拓扑或驱动调用
- 回传 ack、status、resource sample、degrade reason

它不负责：

- 改写系统时间语义
- 绕过 Compiler / Scheduler 直接控制核心状态
- 自定义一套无法被 trace 和 replay 的私有执行语义

### 3.3 Resource Hub 的职责

Resource Hub 负责：

- 发布外部资源集合
- 提供稳定 resource id、版本、标签与能力描述
- 挂接 probe、analysis、compatibility 与 warm status
- 让 Compiler / Runtime 通过引用访问资源，而不是硬编码路径

它不负责：

- 持续控制 Runtime
- 直接决定 patch 是否生效
- 修改核心调度流程

## 4. 统一插件协议方向

### 4.1 统一语义接口

尽管三类后端不同，但都应能回答同一组问题：

- 你是谁，你属于哪类 backend
- 你暴露哪些 endpoint / topology
- 你支持哪些 capability 与降级模式
- 你当前是否 ready / degraded / offline
- 你如何接收 show state 和 executable payload
- 你如何回传 ack、health 和 failure

### 4.2 最小协议形态

推荐抽象为以下统一语义：

- `describe_backend()`
- `prepare_backend(topology_ref, calibration_profile)`
- `apply_show_state(show_state)`
- `execute_payload(executable_payload)`
- `collect_backend_status()`
- `apply_degrade_mode(mode)`
- `shutdown_backend()`

函数名不是重点，重点是三类插件都应落在同一协议骨架内。

### 4.3 三类插件的落点

- 视觉：`Visual Output Backend Adapter Plugin`
- 音频：`Audio Output Backend Adapter Plugin`
- 灯光：`Lighting Output Backend Adapter Plugin`

它们共享同一插件约束，但各自接收不同的 topology 与 executable payload。

## 5. 外部资源 HUB 设计

### 5.1 资源类型

首批建议支持以下 HUB 类型：

- `audio_asset_hub`
- `glsl_scene_hub`
- `scene_model_hub`
- `lighting_preset_hub`

必要时还可扩展：

- `texture_hub`
- `instrument_preset_hub`
- `analysis_profile_hub`

### 5.2 资源对象最小字段

无论资源来自哪个 HUB，至少应具备：

- `resource_id`
- `resource_kind`
- `version`
- `source_locator`
- `content_hash`
- `capabilities`
- `tags`
- `compatibility`
- `warm_status`

### 5.3 HUB 与核心的关系

更合理的关系是：

```text
External Hub
  -> Registry / Resolver
  -> Asset / Resource Record
  -> Compiler / Runtime Reference
```

其中：

- Hub 可以是目录、包仓库、对象存储、远端索引或本地缓存镜像
- 核心只消费解析后的 resource record 与 capability 事实
- 真正进入 Runtime 的是已校验的引用，不是 Hub 自身

## 6. 对系统规划的影响

### 6.1 对产品定位的影响

产品不再被理解为“带一堆内置资源与内置设备支持的重型引擎”，而是：

```text
一个保持核心精简、围绕统一协议组织、通过插件接硬件、通过 HUB 接外部资源的视听编排与执行系统。
```

### 6.2 对架构的影响

系统架构应显式分成四层：

1. Planning / Capability Surface
2. Core Control And Scheduling
3. Adapter Plugin Layer
4. External Resource Hubs

### 6.3 对 MVP 的影响

MVP 不必一次做完整插件生态和完整 HUB 市场，但必须先固定：

- 插件协议边界
- backend capability 描述方式
- 资源记录与引用方式
- 核心与边界的职责分割

否则后续无论接新装置还是新资源，都会反向污染核心。

## 7. 与 27 / 28 / 29 文档的关系

27、28、29 号文档分别回答三类装置“如何抽象后端”。

本文进一步统一它们的共同原则：

- 视觉、音频、灯光后端都应被理解为 adapter plugin
- 它们都应遵循统一控制协议与状态回报边界
- 它们消费的是统一的 show state 与 executable payload
- 它们各自依赖的素材、算法、预设和模型应来自外部 HUB

因此：

- 27 更偏视觉后端与 GLSL 场景问题
- 28 更偏音频输出后端与声场路由问题
- 29 更偏灯光后端与空间光场问题
- 30 负责把三者提升为系统级统一设计原则

## 8. 建议的后续对象化方向

如果继续往 DSL / IR / Schema 层推进，建议后续显式引入：

- `backend_descriptor`
- `adapter_plugin_manifest`
- `resource_hub_descriptor`
- `resource_record`
- `lighting_topology`
- `cue_set`

这些对象不要求立刻全部实现，但应成为后续 schema 和 runtime 能力建模的稳定方向。

## 9. 结论

本轮修订后，Vidodo 的统一设计意图应明确为：

1. 核心只保留编排、校验、编译、调度、trace 和回退等高价值公共能力。
2. 离散式视觉 / 声场 / 灯光装置通过统一协议约束下的 adapter plugin 接入。
3. 音频素材、GLSL 代码、3D 场景模型等作为外部资源，由 HUB 组织与发布。
4. 系统一致性来自统一协议、统一时间和统一证据链，而不是来自把所有实现和资源都塞进核心。