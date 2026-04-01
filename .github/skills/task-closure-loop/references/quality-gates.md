# Vidodo Rust Quality Gates

Run commands from `vidodo-src`.

## Inner Loop

- `cargo test -p <crate>` for the crate being changed
- `cargo fmt --all --check`

## Closure Gate

- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --all-targets`
- `cargo audit`

## Milestone Or Performance Gate

- `cargo bench --workspace`

Bench is intentionally separated from the shortest closure gate so agents can keep iteration tight while still having a defined performance checkpoint.
