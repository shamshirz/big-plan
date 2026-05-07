---
id: 006
title: Implement SQLite repository adapter with rusqlite
status: complete
depends_on: []
created_at: 2026-05-07T18:32:47Z
started_at: 2026-05-07T18:48:54Z
completed_at: 2026-05-07T19:00:00Z
duration_seconds: 672
input_tokens: 29
output_tokens: 48230
model: null
commit_sha: null
---

# Task 006: Implement SQLite repository adapter with rusqlite

## Description
Implement the SQLite adapter in Rust using `rusqlite`, mapping repository operations to the schema/migration plan.

## Context
- Focus on persistence adapter only.
- Use domain model interfaces from task 004 and schema outputs from task 003.
- Avoid CLI text formatting concerns in this task.

## Acceptance criteria
- [ ] DB initialization and migrations execute safely/idempotently.
- [ ] CRUD/query operations for tasks/events are implemented with transactions where needed.
- [ ] Adapter maps SQL rows <-> domain types with robust error handling.
- [ ] Repository behavior is covered by focused persistence tests.

---

## Completion notes
Implemented `SqliteRepository` in `loop-rs/src/sqlite_repo.rs` and wired it into `main.rs` (replaced `StubRepository`).

- Added `rusqlite = { version = "0.31", features = ["bundled"] }` to `Cargo.toml`.
- Migration bootstrap: reads `PRAGMA user_version`, applies v1 schema (tasks/events/config tables + indexes) in a transaction, updates version after commit. Idempotent: returns `AlreadyInitialized` if already at latest version.
- `initialize()` creates `.loop/`, runs migrations, seeds config rows with `INSERT OR IGNORE`, and writes template `plan.md` / `agent-project.md` files (skips if already present).
- `add_task()`: computes next seq with `SELECT MAX(seq)`, inserts task + `created` event in a single transaction.
- `list_tasks()`: uses `stmt.query([])` + while-let over `Rows` to avoid `query_map` temporary lifetime issue.
- `get_task()`: uses `query_row` with `QueryReturnedNoRows` mapped to `TaskNotFound`.
- `update_task()`: reads old status, UPDATEs task row, inserts lifecycle event (started/completed/failed/reset) inferred from the status transition — all in one transaction.
- `row_to_task()`: maps all 17 columns; domain parse errors mapped to `rusqlite::Error::InvalidColumnName`.
- `parse_depends_on()` handles the JSON array of ID strings manually.
- 17 persistence tests cover: init/idempotency, add/list/get/update lifecycle, not-initialized errors, reset clears runtime fields, plan read. All 67 tests pass (`cargo test`).

Implemented SqliteRepository in sqlite_repo.rs with migrations, CRUD ops, event tracking, and 17 persistence tests. All 67 tests pass.
