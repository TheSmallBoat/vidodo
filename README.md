# vidodo

Vidodo is a design-first repository for an externally planned audiovisual system.

The project defines a deterministic system that accepts structured plans and live patches from humans or external agents, validates and compiles them into shared IR and timelines, and drives audio and visual runtimes for live execution or offline export.

## Repository Status

- Current stage: design and architecture drafting
- Main contents: product documents, technical design documents, and MCP tool schema
- Implementation status: `vidodo-src/` is reserved for future code and is currently empty

## What This Repository Covers

- Product positioning for an externally planned audiovisual system
- Shared concepts for DSL, IR, timeline, trace, and patch control
- CLI and MCP capability model design
- Runtime architecture for audio and visual execution
- MVP architecture, Phase 0 implementation plan, and engineering workflow guidance
- Initial JSON Schema artifacts for MCP tool definitions

## Core Idea

Vidodo is not positioned as an embedded LLM product, an open-ended AI music generator, or a DAW replacement.

It is positioned as:

> A deterministic audiovisual composition, compilation, and execution system driven by external planners.

That means:

- planners stay outside the system
- the system focuses on validation, compilation, scheduling, execution, trace, and rollback
- offline and live execution share the same time semantics and artifact model
- audio and visual runtimes are coordinated through a common protocol rather than loose signal following

## Repository Structure

```text
.
├── vidodo-docs/
│   ├── 00-26 design and planning documents
│   └── schemas/
│       └── mcp-tools/
│           └── av-tool-registry.v0.json
└── vidodo-src/
    └── reserved for implementation code
```

## Recommended Reading Order

If you want the quickest path to understanding the repository, start here:

1. `vidodo-docs/00-视听系统产品包总览.md`
2. `vidodo-docs/02-视听系统产品定位与规划方案书.md`
3. `vidodo-docs/09-CLI与MCP能力模型与命令语义.md`
4. `vidodo-docs/14-MVP技术架构图与Phase0实施清单.md`
5. `vidodo-docs/25-开发工作流与分支策略.md`

## Key Design Directions

- One product, two runtimes: Audio Runtime and Visual Runtime
- External planning surface via CLI, files, API, or MCP
- Shared capability model between CLI and MCP
- Shared IR and timeline for both live and offline paths
- Structured trace, replay, diagnostics, and bounded patch rollback
- Design-first monorepo intended to evolve into a Rust + Python implementation

## Current Assets

- 27 planning and design documents in Chinese
- 1 MCP tool registry schema draft
- Phase 0 implementation blueprint
- Development workflow and branching strategy draft

## Suggested GitHub About

Externally planned audiovisual system for composition, compilation, scheduling, runtime execution, and live patch control.

## Suggested Topics

`audiovisual`
`creative-coding`
`live-performance`
`music-tech`
`visual-runtime`
`rust`
`python`
`mcp`
`json-schema`
`system-design`

## Roadmap

1. Finalize schemas for planning, runtime, trace, and patch artifacts.
2. Establish the monorepo implementation skeleton in `vidodo-src/`.
3. Build the first CLI and capability-layer prototypes.
4. Implement the artifact store and validation pipeline.
5. Prove the Phase 0 end-to-end loop.

## License

This repository is licensed under the Apache License 2.0. See the `LICENSE` file for details.