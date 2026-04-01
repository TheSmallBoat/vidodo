---
name: task-closure-loop
description: 'Fixed task-card closure workflow for Vidodo. Use for WSA/WSB/WSC tasks, checklist execution, TDD loops, acceptance checks, and minimal closed-loop delivery in the Rust workspace.'
argument-hint: 'Task card id or closure target, for example: WSA-02 or compiler minimal loop'
user-invocable: true
---

# Task Closure Loop

Use this skill when working on Vidodo task cards, milestone checklist items, or any implementation that must be closed with a deterministic minimal loop.

## When to Use

- A task card such as `WSA-01`, `WSB-05`, or `WSC-02` is being implemented.
- A checklist item must be carried from `todo` to `review` or `done`.
- A Rust change needs a strict TDD loop and a fixed quality gate before closure.
- A Copilot-driven implementation needs to stay inside documented product boundaries.

## Procedure

1. Read the task card and direct dependency cards in `vidodo-docs/24-工作任务卡与开发里程碑.md`.
2. Read the matching boundary docs before coding.
3. Convert acceptance criteria into one of three closure mechanisms:
   - a unit or integration test
   - a fixture or golden output check
   - a smoke command with deterministic output
4. Implement the smallest useful end-to-end loop.
5. Run the closure checklist in [task-closure-checklist.md](./assets/task-closure-checklist.md).
6. Run the standard command sequence from [quality-gates.md](./references/quality-gates.md).
7. Close the task only if acceptance checks, tests, and document sync are all complete.

## Closure Standard

- `done`: acceptance criteria met, tests green, docs synced, no open blocker.
- `review`: implementation complete but waiting on human review or external confirmation.
- `blocked`: dependency, boundary conflict, or missing prerequisite prevents safe closure.
