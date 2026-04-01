# Vidodo Copilot Instructions

## Scope

- This repository is Copilot-driven and task-card-driven.
- Treat `vidodo-docs/` as the source of truth for product boundary, architecture, task cards, and test strategy.
- If a request mentions `video-src`, treat it as `vidodo-src` unless the user explicitly creates a separate directory.

## Required Build Flow

- Work from a single task card or a tightly related pair of task cards.
- Read the relevant task card, checklist, and boundary documents before editing code.
- Build the smallest end-to-end loop that satisfies the task card acceptance criteria.
- Prefer test-first or fixture-first changes for schema, validation, compiler, scheduler, trace, and patch logic.
- Run the narrowest useful test loop while iterating, then run the workspace quality gate before closing the task.

## Task Closure Rules

- Do not mark a task closed until acceptance criteria are satisfied.
- Do not skip validation because a feature is still partial; instead, narrow the scope until a real closed loop exists.
- Keep implementation aligned with document constraints: deterministic artifacts, external planners, shared IR/timeline, traceability, and bounded patch rollback.
- When schema or artifact shape changes, update tests and linked design docs in the same change.

## Rust Workspace Defaults

- Use the commands in `vidodo-src`: `cargo fmt --all`, `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-targets`, `cargo audit`, and `cargo bench --workspace`.
- Prefer small composable crates over monolithic modules.
- Keep binaries thin; move logic into library crates.
- Avoid introducing async runtime, networking, or heavy external dependencies until the relevant task cards require them.

## Agent Behavior

- Advance in phases: clarify scope, add or tighten tests, implement the minimum useful change, run quality gates, then update closure status.
- Surface blockers early if a requested change conflicts with documented boundaries.
- Do not overwrite user edits in unrelated files.
