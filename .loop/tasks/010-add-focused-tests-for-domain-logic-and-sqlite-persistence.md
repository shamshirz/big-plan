---
id: 010
title: Add focused tests for domain logic and sqlite persistence
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

# Task 010: Add focused tests for domain logic and sqlite persistence

## Description
Add focused automated tests for pure domain logic and SQLite persistence behavior.

## Context
- Keep scope to domain + repository layers.
- Avoid full process orchestration coverage in this task.
- Inputs: tasks 003, 004, and 006 outputs.

## Acceptance criteria
- [ ] Domain transition rules have focused unit tests.
- [ ] SQLite adapter/migration paths have deterministic tests.
- [ ] Edge cases (invalid transitions, missing task IDs, migration idempotence) are covered.
- [ ] Test suite runs locally with clear pass/fail output.

---

## Completion notes
[Fill this section before running `loop complete`.]
