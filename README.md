# vidodo

Vidodo is a design-first repository for an externally planned audiovisual system.

The project defines a deterministic system that accepts structured plans and live patches from humans or external agents, validates and compiles them into shared IR and timelines, and drives audio and visual runtimes for live execution or offline export.

## Repository Status

- Current stage: design-first repository with an initial Rust workspace scaffold
- Main contents: product documents, technical design documents, GitHub workflow templates, Copilot harness files, and a minimal Rust implementation skeleton
- Implementation status: `vidodo-src/` now contains a compilable Rust workspace with placeholder apps, core crates, benchmarks, and workspace automation

## Core Idea

Vidodo is not positioned as an embedded LLM product, an open-ended AI music generator, or a DAW replacement.

It is positioned as:

> A deterministic audiovisual composition, compilation, and execution system driven by external planners.

That means:

- planners stay outside the system
- the system focuses on validation, compilation, scheduling, execution, trace, and rollback
- offline and live execution share the same time semantics and artifact model
- audio and visual runtimes are coordinated through a common protocol rather than loose signal following


## Key Design Directions

- One product, two runtimes: Audio Runtime and Visual Runtime
- External planning surface via CLI, files, API, or MCP
- Shared capability model between CLI and MCP
- Shared IR and timeline for both live and offline paths
- Structured trace, replay, diagnostics, and bounded patch rollback
- Design-first monorepo intended to evolve into a Rust + Python implementation

## Current Code Status

The codebase is still early-stage, but it is no longer empty.

- `vidodo-src/apps/avctl` provides a minimal smoke CLI with `doctor` and `plan validate` commands
- `vidodo-src/crates/ir` defines the first shared serializable types for a minimal plan bundle and compiled plan
- `vidodo-src/crates/validator` and `vidodo-src/crates/compiler` provide a deterministic minimal validation and compile loop
- `vidodo-src/crates/scheduler`, `trace`, `storage`, and `patch-manager` provide placeholder domain modules with basic tests
- `vidodo-src/xtask` centralizes the workspace quality gate and command workflow

This scaffold is intended to support the first real task-card implementations rather than represent finished runtime behavior.

## Default Rust Workflow

Run from `vidodo-src/`:

- `cargo fmt --all`
- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --all-targets`
- `cargo audit`
- `cargo bench --workspace`
- `cargo xtask ci`

## Roadmap

1. Finalize schemas for planning, runtime, trace, and patch artifacts.
2. Expand the initial Rust scaffold into real task-card implementations.
3. Build the first schema, validator, and compiler deliverables from Workstream A and C.
4. Implement the artifact store and validation pipeline.
5. Prove the Phase 0 end-to-end loop.

## License

This repository is licensed under the Apache License 2.0. See the `LICENSE` file for details.