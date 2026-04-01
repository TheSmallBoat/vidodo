# vidodo-src

This directory contains the Rust workspace for the current Vidodo Phase 0 mainline.

## Layout

- `apps/avctl`: current single control surface for asset ingest/list/show, validate, compile, run, patch, trace, and doctor
- `apps/core-service`: deferred placeholder until the local closed loop no longer fits inside `avctl`
- `apps/visual-runtime`: deferred placeholder while the fake visual backend remains sufficient for Phase 0
- `apps/mcp-adapter`: deferred placeholder until the CLI/file surface is saturated
- `crates/ir`: shared P0 schema-aligned artifact and runtime types
- `crates/validator`: semantic validation for fixture-backed plan bundles
- `crates/compiler`: deterministic compile path from planning objects to IR and timeline artifacts
- `crates/scheduler`: musical clock, fake audio/visual backend dispatch, and run status generation
- `crates/patch-manager`: local-content patch checking, submit, and rollback decision flow
- `crates/trace`: trace manifest and event log writing/loading
- `crates/storage`: repo-root discovery plus asset registry, local WAV/PCM probe, analysis cache, and artifact-store helpers
- `xtask`: workspace automation for fmt, clippy, audit, test, and bench

## Default Commands

Run from `vidodo-src`:

- `cargo fmt --all`
- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --all-targets`
- `cargo audit`
- `cargo bench --workspace`
- `cargo xtask ci`
- `cargo run -p avctl -- asset ingest --source-dir ../tests/fixtures/imports/minimal-audio-pack --declared-kind audio_loop --tags fixture,smoke`
- `cargo run -p avctl -- asset ingest --source-dir ../tests/fixtures/imports/manifest-audio-pack --declared-kind audio_loop --tags fixture,manifest`
- `cargo run -p avctl -- asset ingest --source-dir ../tests/fixtures/imports/minimal-audio-pack --declared-kind audio_loop --asset-namespace session-a`
- `cargo run -p avctl -- doctor`

Run from the repository root:

- `./scripts/schema-validate.sh`
- `./scripts/init-artifact-store.sh`
- `./tests/e2e/asset_ingest_smoke.sh`
- `./tests/e2e/phase0_smoke.sh`

## Phase 0 Mainline

1. Validate a controlled plan fixture through `avctl plan validate`.
2. Ingest controlled WAV/PCM source files through `avctl asset ingest`, run the local audio probe, and inspect the registry with `avctl asset list/show`.
3. Compile the plan into revision artifacts through `avctl compile run`, which now writes a filtered compile snapshot from published `compile_ready` or `warmed` registry assets when ingest has populated the artifact store.
4. Submit a bounded local-content patch through `avctl patch submit`.
5. Execute the patched revision through `avctl run start`.
6. Persist trace artifacts and inspect them with `avctl trace show/events`.
7. Generate a rollback decision through `avctl patch rollback`.

This is the only supported implementation path in the current repository. `core-service`, MCP, lighting, and distributed execution remain intentionally outside the active closure loop.

`asset ingest` uses a minimal human-readable naming policy: `asset_id = declared_kind + source basename`. Inside one `asset_kind`, basename collisions are rejected with explicit diagnostics unless the operator provides a batch namespace or a per-file asset-id override map.

`--asset-namespace <namespace>` inserts one readable segment between the declared kind prefix and the basename, for example `audio.loop.bundle-a.kick`.

`asset ingest` also auto-loads `vidodo-asset-pack.json` from the source dir when present. The manifest can carry `asset_namespace` and `asset_id_overrides`, where override keys are source-dir-relative paths such as `crate-a/kick.wav` and override values are explicit final asset IDs such as `audio.loop.bundle-a.kick`.

When both are present, explicit override entries take precedence over `asset_namespace`. A CLI `--asset-namespace` still overrides the manifest namespace for one-off operator runs.