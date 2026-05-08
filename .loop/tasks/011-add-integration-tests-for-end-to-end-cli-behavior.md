---
id: 011
title: Add integration tests for end-to-end CLI behavior
status: complete
depends_on: []
created_at: 2026-05-07T18:32:47Z
started_at: 2026-05-08T00:28:37Z
completed_at: 2026-05-08T00:29:16Z
duration_seconds: 41
input_tokens: null
output_tokens: null
model: null
commit_sha: null
---

# Task 011: Add integration tests for end-to-end CLI behavior

## Description
Add end-to-end integration tests validating CLI flows from init through run/complete/reset behavior.

## Context
- Focus on black-box CLI behavior.
- Avoid reworking internals unless required to make behavior testable.
- Inputs: finalized CLI contract and implemented adapters.

## Acceptance criteria
- [x] Integration tests cover happy path and at least one failure path for `run`.
- [x] Tests verify project-local isolation (separate directories have separate state).
- [x] Tests verify help behavior and core command UX.
- [x] Test harness is repeatable in CI/local environments.

---

## Completion notes
- Added `loop-rs/tests/cli_integration.rs`: spawns built `loop` via `CARGO_BIN_EXE_loop`, uses `tempfile::TempDir` per test for cwd isolation (repeatable CI/local).
- `run`: happy path asserts `No pending tasks.` after init with no tasks; failure paths cover not initialized, pending task (agent stub exits 1), and already-running guard (via targeted SQLite update on `.loop/loop.db` as fixture-only setup).
- Isolation test: two temp projects; task added only in project A appears only in A’s `status`.
- Help/UX: `-h`, `--help`, no-args show usage banner; unknown command and `add` without title verify stderr messaging.
- E2E: init → add → force running → `complete --notes` → `show` then `reset` → pending.
- Verified: `cd loop-rs && cargo test` (88 unit + 9 integration tests, all passing).

Added loop-rs/tests/cli_integration.rs (tempfile dirs, subprocess loop binary). Covers run happy/failure, isolation, help/UX, init→complete→reset. Dev-deps tempfile+rusqlite. Verified cargo test.
