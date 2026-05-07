---
id: 004
title: Define Rust domain model and state transition rules
status: complete
depends_on: []
created_at: 2026-05-07T18:32:47Z
started_at: 2026-05-07T18:38:36Z
completed_at: 2026-05-07T18:42:31Z
duration_seconds: 242
input_tokens: 1319
output_tokens: 17503
model: null
commit_sha: null
---

# Task 004: Define Rust domain model and state transition rules

## Description
Define Rust domain entities and pure state transition rules independent from storage/CLI adapters.

## Context
- Focus on functional core design.
- Avoid direct SQLite crate or subprocess concerns in this task.
- Inputs: task lifecycle requirements from `.loop/plan.md` and CLI contract from task 002.

## Acceptance criteria
- [ ] Define domain types for task, status, event, and run outcomes.
- [ ] Define valid transitions (`pending -> running -> complete|failed`, reset behavior).
- [ ] Define validation rules and domain errors.
- [ ] Ensure the model is adapter-agnostic and testable in isolation.

---

## Completion notes
[Fill this section before running `loop complete`.]

Created loop-rs/ Rust crate with domain module at loop-rs/src/domain.rs. Defined TaskId, TaskStatus, Task, CompletionData, EventType, EventMetadata, Event, DomainError. Implemented pure transition functions (transition_start, transition_complete, transition_fail, transition_reset) and collection helpers (check_no_running_task, next_pending, current_running). All 26 unit tests pass via cargo test. No SQLite or subprocess dependencies — adapter-agnostic.
