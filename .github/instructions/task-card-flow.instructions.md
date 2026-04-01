---
description: "Use when implementing a task card, checklist item, milestone deliverable, or phase closure step. Covers fixed task write-off flow, TDD loop, and minimal end-to-end closure for Vidodo."
name: "Task Card Flow"
---
# Task Card Flow

Use this fixed flow for WSA/WSB/WSC and related task-card work:

1. Read the target task card and its dependency cards.
2. Restate the acceptance criteria as executable checks.
3. Create or tighten a failing test, fixture, or smoke command.
4. Implement the smallest change that satisfies the check.
5. Run focused verification first, then the workspace quality gate.
6. Update affected docs, fixtures, and task closure notes in the same change.
7. Only then mark the task ready for review or done.

Keep the closure loop minimal:

- schema work: fixture first, validator second, Rust types third
- compiler work: parser or IR test first, compile path second, snapshot or golden check third
- runtime work: fake backend or smoke test first, then integration path
- patch work: authorization and rollback checks must exist before task closure

Do not batch unrelated task cards into one implementation pass.
