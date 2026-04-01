# vidodo-src

This directory contains the default Rust workspace skeleton for the Vidodo MVP build.

## Layout

- `apps/avctl`: CLI entrypoint for minimal operator workflows
- `apps/core-service`: placeholder core control service binary
- `apps/visual-runtime`: placeholder visual runtime binary
- `apps/mcp-adapter`: placeholder MCP adapter binary
- `crates/ir`: shared deterministic artifact and IR types
- `crates/validator`: validation entrypoints and diagnostics
- `crates/compiler`: minimal compile path for a plan bundle
- `crates/scheduler`: minimal runtime preparation logic
- `crates/patch-manager`: bounded patch gate helpers
- `crates/trace`: trace manifest helpers
- `crates/storage`: artifact-store path helpers
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

## Minimal Closed Loop

1. Start from a single task card.
2. Add a failing test or fixture.
3. Implement the smallest crate-level change.
4. Run crate-local tests.
5. Run `cargo xtask ci`.
6. Run `cargo xtask bench` when performance matters or before milestone closure.