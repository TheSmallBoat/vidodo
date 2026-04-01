# vidodo-src

This directory contains the Rust workspace for the current Vidodo Phase 0 mainline.

## Layout

- `apps/avctl`: current single control surface for validate, compile, run, patch, trace, and doctor
- `apps/core-service`: deferred placeholder until the local closed loop no longer fits inside `avctl`
- `apps/visual-runtime`: deferred placeholder while the fake visual backend remains sufficient for Phase 0
- `apps/mcp-adapter`: deferred placeholder until the CLI/file surface is saturated
- `crates/ir`: shared P0 schema-aligned artifact and runtime types
- `crates/validator`: semantic validation for fixture-backed plan bundles
- `crates/compiler`: deterministic compile path from planning objects to IR and timeline artifacts
- `crates/scheduler`: musical clock, fake audio/visual backend dispatch, and run status generation
- `crates/patch-manager`: local-content patch checking, submit, and rollback decision flow
- `crates/trace`: trace manifest and event log writing/loading
- `crates/storage`: repo-root discovery and root artifact-store helpers
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
- `cargo run -p avctl -- doctor`

Run from the repository root:

- `./scripts/schema-validate.sh`
- `./scripts/init-artifact-store.sh`
- `./tests/e2e/phase0_smoke.sh`

## Phase 0 Mainline

1. Validate a controlled plan fixture through `avctl plan validate`.
2. Compile the plan into revision artifacts through `avctl compile run`.
3. Submit a bounded local-content patch through `avctl patch submit`.
4. Execute the patched revision through `avctl run start`.
5. Persist trace artifacts and inspect them with `avctl trace show/events`.
6. Generate a rollback decision through `avctl patch rollback`.

This is the only supported implementation path in the current repository. `core-service`, MCP, lighting, and distributed execution remain intentionally outside the active closure loop.