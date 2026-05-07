---
id: 006
title: Implement SQLite repository adapter with rusqlite
status: pending
depends_on: []
created_at: 2026-05-07T18:32:47Z
started_at: null
completed_at: null
duration_seconds: null
input_tokens: null
output_tokens: null
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
[Fill this section before running `loop complete`.]
