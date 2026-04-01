# schemas

本目录存放 Vidodo 的正式 JSON Schema 文件。

当前已落地的第一批骨架文件包括：

- `common/`
  - `object-header.v0.json`
  - `source-info.v0.json`
  - `provenance.v0.json`
  - `musical-time.v0.json`
  - `diagnostic.v0.json`
  - `response-envelope.v0.json`
- `planning/`
  - `set-plan.v0.json`
  - `audio-dsl.v0.json`
  - `visual-dsl.v0.json`
  - `constraint-set.v0.json`
- `asset/`
  - `asset-record.v0.json`
- `ir/`
  - `asset-ir.v0.json`
  - `structure-ir.v0.json`
  - `performance-ir.v0.json`
  - `visual-ir.v0.json`
  - `timeline-entry.v0.json`
- `patch/`
  - `live-patch-proposal.v0.json`
- `trace/`
  - `trace-manifest.v0.json`
- `runtime/`
  - `event-header.v0.json`
- `mcp-tools/`
  - `av-tool-registry.v0.json`

这批文件的目标是先把命名、目录、`$id`、`$ref` 和 required 字段框架固定下来。

当前已补到第二批骨架，新增包括：

- `runtime/`
  - `transport-event.v0.json`
  - `timing-event.v0.json`
  - `audio-event.v0.json`
  - `visual-event.v0.json`
  - `semantic-event.v0.json`
  - `patch-event.v0.json`
- `ir/`
  - `display-topology.v0.json`
  - `view-group.v0.json`
  - `speaker-matrix-topology.v0.json`
  - `route-set.v0.json`

下一批建议按 `26-JSON-Schema完整清单与版本策略.md` 继续补：

1. `ir/show-state-snapshot.v0.json`、`ir/compile-record.v0.json`、`ir/revision-record.v0.json`
2. `trace/` 的 event / resource / replay / evaluation 系列 schema
3. `asset/` 的 `ingestion-*`、`analysis-*` 系列 schema
4. `audio/`、`link/`、`capability/` 分类

当前所有 `.json` 文件已经通过基础 JSON 语法校验。