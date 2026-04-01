# Vidodo Task Closure Checklist

- Confirm the target task card and direct dependencies.
- Confirm the task still fits documented product boundaries.
- Add or update a deterministic test, fixture, or smoke command.
- Implement the minimum code needed for a real closed loop.
- Run focused verification for the changed area.
- Run `cargo fmt --all --check`.
- Run `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
- Run `cargo test --workspace --all-targets`.
- Run `cargo audit`.
- Run `cargo bench --workspace` when the change affects performance or before milestone closure.
- Update any affected docs, fixtures, and task notes.
- Mark the task as `review` or `done` only after the checks pass.
