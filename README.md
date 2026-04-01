# vidodo

Vidodo is a design-first repository for an externally planned audiovisual system.

The project defines a deterministic system that accepts structured plans and live patches from humans or external agents, validates and compiles them into shared IR and timelines, and drives audio and visual runtimes for live execution or offline export.

## Repository Status

- Current stage: design-first repository with a normalized Phase 0 execution baseline
- Main contents: `vidodo-docs/` design documents, root-level `schemas/`, root-level `scripts/`, controlled `tests/fixtures/`, GitHub workflow templates, and a Rust workspace under `vidodo-src/`
- Implementation status: `vidodo-src/` now carries the single Phase 0 mainline plus a minimal asset ingestion and analysis-cache loop via `avctl`

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

The codebase is still early-stage, but the repository now has a real closed-loop baseline.

- `schemas/` is the canonical schema root for Phase 0 artifacts
- `scripts/schema-validate.sh` validates root schemas against controlled fixtures in `tests/schema/`
- `scripts/init-artifact-store.sh` initializes the root artifact store, including raw/normalized asset layers and analysis cache directories
- `tests/fixtures/` contains the controlled plan, asset, patch, and import fixtures used by the current loops
- `vidodo-src/apps/avctl` now exposes the operator flow for `asset ingest/list/show`, `plan validate`, `compile run`, `run start/status`, `patch check/submit/rollback`, `trace show/events`, and `doctor`
- `asset ingest` now runs a local minimal WAV/PCM audio probe and persists probe-backed beat-track results into the analysis cache
- `vidodo-src/crates/ir`, `validator`, `compiler`, `scheduler`, `patch-manager`, `trace`, and `storage` implement the deterministic compile -> patch -> fake runtime -> trace loop
- `vidodo-src/apps/core-service`, `mcp-adapter`, and `visual-runtime` remain deliberately deferred placeholders until this single mainline is exhausted

## Canonical Roots

- `vidodo-docs/`: source of truth for product boundary, architecture, task cards, and test strategy
- `schemas/`: canonical JSON Schema root
- `scripts/`: repository-level validation and artifact-store scripts
- `tests/`: schema fixtures, controlled inputs, and end-to-end smoke scripts
- `vidodo-src/`: Rust workspace implementing the current Phase 0 mainline

## Default Workflow

Run from the repository root:

- `./scripts/schema-validate.sh`
- `./scripts/init-artifact-store.sh`
- `./tests/e2e/asset_ingest_smoke.sh`

Run from `vidodo-src/`:

- `cargo fmt --all`
- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --all-targets`
- `cargo audit`
- `cargo bench --workspace`
- `cargo xtask ci`
- `cargo run -p avctl -- asset ingest --source-dir ../tests/fixtures/imports/minimal-audio-pack --declared-kind audio_loop --tags fixture,smoke`
- `cargo run -p avctl -- doctor`

## Roadmap

1. Expand fixture and negative coverage from the current P0 schema set to the remaining schema catalog.
2. Expand the minimal asset ingestion path from the current deterministic file-copy + cache-probe baseline to richer normalizers and analyzers.
3. Harden scheduler resync, degraded mode, and trace detail around longer-running scenarios.
4. Decide whether `core-service` should remain a library-driven local loop or split into a dedicated service.
5. Only then widen MCP, lighting, and distributed deployment scope.

## License

This repository is licensed under the Apache License 2.0. See the `LICENSE` file for details.