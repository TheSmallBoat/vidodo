---
description: "Use when editing Rust workspace files, Cargo manifests, benches, or source under vidodo-src. Covers workspace layout, crate boundaries, and required quality gates."
name: "Rust Workspace Rules"
applyTo:
  - "vidodo-src/**/*.rs"
  - "vidodo-src/**/Cargo.toml"
  - "vidodo-src/.cargo/config.toml"
  - "vidodo-src/rust-toolchain.toml"
  - "vidodo-src/rustfmt.toml"
---
# Rust Workspace Rules

- Keep business logic in `vidodo-src/crates/`; keep `vidodo-src/apps/` binaries thin.
- Match crate purpose to the architecture docs: `ir`, `validator`, `compiler`, `scheduler`, `patch-manager`, `trace`, `storage`.
- Prefer deterministic pure functions and serializable artifact types.
- Add or update tests with each behavioral change.
- Before considering a Rust task done, run the smallest relevant loop during development and the full workspace gate before closure:
  - `cargo fmt --all --check`
  - `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  - `cargo test --workspace --all-targets`
  - `cargo audit`
- Run `cargo bench --workspace` for performance-sensitive changes or before milestone closure.
