---
id: 003
title: Design SQLite schema and migration/versioning strategy
status: complete
depends_on: []
created_at: 2026-05-07T18:32:46Z
started_at: 2026-05-07T18:37:09Z
completed_at: 2026-05-07T18:38:31Z
duration_seconds: 87
input_tokens: 9
output_tokens: 4917
model: null
commit_sha: null
---

# Task 003: Design SQLite schema and migration/versioning strategy

## Description
Design the SQLite persistence model and migration/versioning strategy for project-local loop state.

## Context
- Focus only on data model and migration lifecycle.
- Do not implement Rust command handlers here.
- Inputs: `.loop/plan.md`, `.loop/agent-project.md`, and task metadata currently used by loop.

## Acceptance criteria
- [ ] Define tables/relations for tasks, events, and config/version state.
- [ ] Document canonical fields for task lifecycle and metrics.
- [ ] Specify migration/version bootstrap rules for `loop init` and future upgrades.
- [ ] Clarify indexing and integrity constraints needed for status/read/run performance.

---

## Completion notes
Schema design documented in `.loop/sqlite-schema.md`.

- **Tables defined**: `tasks`, `events`, `config` — all with CHECK constraints and NOT NULL defaults.
- **Version tracking**: uses `PRAGMA user_version` (SQLite-native, no extra table).
- **Migration bootstrap**: ordered array of `(version, sql)` pairs; applied transactionally on `loop init` or DB open; idempotent via version check.
- **`depends_on`**: stored as JSON TEXT array in `tasks` row (PoC simplicity over normalized join table).
- **Indexes**: `idx_tasks_status` and `idx_events_task_id` cover the hot read paths for `run`, `status`, and `read current`.
- **Invariant enforcement**: single-running-task rule is application-level (domain model, task 004), not a DB partial unique index.
- **`loop init` sequence**: creates dir → opens DB → runs migrations → seeds config → writes template files → exits 0.
- **Query patterns** for all eight CLI commands are documented with exact SQL.

Schema design documented in .loop/sqlite-schema.md. Tables: tasks, events, config. Version tracking via PRAGMA user_version. Migration bootstrap: ordered (version, sql) pairs applied transactionally. depends_on as JSON TEXT. Indexes on tasks.status and events.task_id. Single-running-task invariant is application-level. Full query patterns for all 8 CLI commands documented.
