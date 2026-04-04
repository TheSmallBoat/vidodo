# vidodo

Vidodo is a deterministic audiovisual composition, compilation, and execution system driven by external planners.

The system accepts structured plans and live patches from humans or external agents, validates and compiles them into shared IR and timelines, coordinates audio / visual / lighting runtimes, and produces traceable evidence of every decision.

## Repository Status

**Phase 4 complete** — 112 task cards done, 24 milestones closed (M0-M23).

| Metric | Value |
|--------|-------|
| Capabilities | 39 (CLI + HTTP + MCP) |
| Rust tests | 256 |
| Schema fixtures | 101 |
| Crates | 12 |
| Apps | 5 (avctl, core-service, mcp-adapter, visual-runtime, lighting-runtime) |
| Showcase examples | 4 |
| E2E regression suites | 10/10 |

**Important**: The control plane (compile → schedule → patch → trace → evaluate) is fully real. Audio, visual, and lighting backends are currently deterministic simulations — no SuperCollider, no GPU/wgpu, no DMX hardware. See [实现状态矩阵](vidodo-docs/04-测试与工程执行/27-实现状态矩阵.md) for the full Real vs Mock assessment.

## Core Idea

Vidodo is not an LLM product, an AI music generator, or a DAW replacement.

> A deterministic audiovisual composition, compilation, and execution system driven by external planners.

- Planners stay outside the system
- The system validates, compiles, schedules, executes, traces, and rolls back
- Offline and live execution share the same time semantics and artifact model
- Audio and visual runtimes are coordinated through a unified adapter protocol

## Quick Start

```bash
# Clone and build
cd vidodo-src
cargo build --workspace

# Run quality gate
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets
```

## Operation Guide

All commands below run from `vidodo-src/`.

### System Health

```bash
cargo run -p avctl -- doctor                    # Full system diagnostic
cargo run -p avctl -- system capabilities       # List all 39 capabilities
cargo run -p avctl -- system health             # Backend health status
```

### Asset Management

```bash
# Ingest a sample audio pack
cargo run -p avctl -- asset ingest \
  --source-dir ../tests/fixtures/imports/minimal-audio-pack \
  --declared-kind audio_loop --tags fixture,smoke

cargo run -p avctl -- asset list                # List ingested assets
cargo run -p avctl -- asset show <asset_id>     # Show asset details
```

### Plan → Compile → Run → Trace

```bash
# Validate a plan
cargo run -p avctl -- plan validate --plan-file ../tests/fixtures/plans/show-phase0-minimal.json

# Compile plan to IR
cargo run -p avctl -- compile run --plan-file ../tests/fixtures/plans/show-phase0-minimal.json

# Run the show (deterministic offline simulation)
cargo run -p avctl -- run start --show-id show-phase0-minimal --bars 8

# Check run status
cargo run -p avctl -- run status --show-id show-phase0-minimal

# View trace
cargo run -p avctl -- trace show --show-id show-phase0-minimal
cargo run -p avctl -- trace events --show-id show-phase0-minimal --bar 1
```

### Live Patches

```bash
# Check a patch proposal
cargo run -p avctl -- patch check --patch-file ../tests/fixtures/patches/patch-insert-pad.json

# Submit (apply) a patch
cargo run -p avctl -- patch submit --patch-file ../tests/fixtures/patches/patch-insert-pad.json

# Rollback last patch
cargo run -p avctl -- patch rollback --show-id show-phase0-minimal
```

### Revision Management

```bash
cargo run -p avctl -- revision list --show-id show-phase0-minimal
cargo run -p avctl -- revision publish --show-id show-phase0-minimal --rev 1
cargo run -p avctl -- revision archive --show-id show-phase0-minimal --rev 1
```

### Export

```bash
cargo run -p avctl -- export audio --show-id show-phase0-minimal --format wav
```

### Showcase Demos

```bash
# List available built-in examples
cargo run -p avctl -- demo list

# Run a demo (zero-intervention, end-to-end)
cargo run -p avctl -- demo run minimal-beat-show
cargo run -p avctl -- demo run ambient-drift
cargo run -p avctl -- demo run live-patch-demo
```

### Templates & Scenes

```bash
cargo run -p avctl -- template list
cargo run -p avctl -- template load --template-id <id>
cargo run -p avctl -- scene list
cargo run -p avctl -- scene activate --scene-id <id>
```

### External Control

```bash
cargo run -p avctl -- control status
cargo run -p avctl -- control send --event-file <path>
```

### Adapter & Hub Management

```bash
cargo run -p avctl -- adapter load --plugin-path <path>
cargo run -p avctl -- adapter status
cargo run -p avctl -- adapter shutdown --adapter-id <id>
cargo run -p avctl -- hub register --descriptor-file <path>
cargo run -p avctl -- hub resolve --resource-uri <uri>
cargo run -p avctl -- hub status
```

### HTTP API (core-service)

```bash
# Start the service
cargo run -p core-service

# Query capabilities
curl http://localhost:3000/api/capabilities
curl http://localhost:3000/api/capabilities/plan.validate
```

### Schema Validation (outside Rust)

```bash
cd ..  # repository root
./scripts/schema-validate.sh        # Validate all 101 fixtures
./scripts/init-artifact-store.sh    # Initialize artifact directories
```

### E2E Regression

```bash
cd tests/e2e
./regression_suite.sh               # Run all 10 E2E suites
```

## Repository Structure

| Directory | Purpose |
|-----------|---------|
| `vidodo-docs/` | Source of truth — product specs, architecture, task cards, test strategy |
| `schemas/` | Canonical JSON Schema definitions (asset, IR, runtime events, patch, trace, etc.) |
| `scripts/` | Repository-level validation and artifact store initialization |
| `tests/` | Schema fixtures (101), E2E scripts (10 suites), controlled inputs |
| `vidodo-src/` | Rust workspace — 12 crates, 5 apps, 256 tests |
| `artifacts/` | Generated artifacts (traces, exports, revisions, analysis cache) |
| `examples/` | Showcase examples (minimal-beat-show, ambient-drift, live-patch-demo, full-showcase-comprehensive) |

## Phase 5 Roadmap (Real Runtime Implementation)

Phase 5 replaces deterministic simulations with real backends. 34 task cards across 6 workstreams and 6 milestones (M24-M29). See [task cards](vidodo-docs/04-测试与工程执行/24-工作任务卡与开发里程碑.md) for full details.

| Milestone | Workstream | Cards | Goal |
|-----------|------------|-------|------|
| M24 | Y: SuperCollider bridge | WSY-01~05 | OSC client + scsynth process management + BackendAdapter |
| M25 | Z: wgpu visual rendering | WSZ-01~08 | GLSL→SPIR-V + render pipeline + particle shader + multi-viewport |
| M26 | AA: Python analysis | WSAA-01~05 | librosa/essentia/music21 + Rust subprocess bridge |
| M27 | AB: DMX/ArtNet lighting | WSAB-01~05 | DMX frames + ArtNet UDP + fixture topology + BackendAdapter |
| M28 | AC: Real-time scheduler | WSAC-01~05 | Wall-clock MusicalClock + transport + realtime dispatch |
| M29 | AD: Process separation | WSAD-01~06 | IPC messages + thread channels + causation tracing + resilience |

## License

This repository is licensed under the Apache License 2.0. See the `LICENSE` file for details.