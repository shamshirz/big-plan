---
id: 007
title: Implement task markdown rendering and read/show projections
status: complete
depends_on: []
created_at: 2026-05-07T18:32:47Z
started_at: 2026-05-07T19:00:07Z
completed_at: null
duration_seconds: 256
input_tokens: 21
output_tokens: 17451
model: null
commit_sha: f2026c9ee9e856d2895adb10079d21254ab2a372
---

# Task 007: Implement task markdown rendering and read/show projections

## Description
Implement markdown/text projection logic for `read` and `show` outputs from canonical persisted task state.

## Context
- Focus on output rendering/projection.
- Avoid introducing SQL schema changes in this task.
- Inputs: domain model definitions and CLI output requirements.

## Acceptance criteria
- [ ] `read plan|current|<id>` and `show <id>` return consistent canonical text.
- [ ] Task metadata and completion notes render deterministically.
- [ ] Renderer/projection logic is unit tested with fixture-like examples.
- [ ] Output remains stable across repeated runs.

---

## Completion notes
Created `loop-rs/src/render.rs` with two pure projection functions:
- `render_task_markdown(task)` — agent-consumable markdown for `read current` / `read <id>`; omits empty sections, includes lifecycle timestamps inline with metadata block
- `render_task_detail(task)` — human-readable key-value format for `show <id>`; always shows all content sections (with "(none)" fallback), appends runtime metrics block only when present

Both functions are deterministic (same input → same output, verified by test assertions on exact strings). Trailing-newline edge cases are handled via `push_md_section` helper to avoid double-blank-lines.

Removed the private `format_task_markdown` from `commands.rs`; updated `show`, `read_current`, and `read_task` handlers to use the new renderers. Added 11 unit tests in `render.rs` covering: minimal/full fixtures, section omission, timestamp inclusion, determinism, and trailing-newline idempotency.

All 79 tests pass (including 11 new render tests and all pre-existing cli/domain/sqlite_repo tests).
