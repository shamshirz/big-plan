---
id: 004
title: Define Rust domain model and state transition rules
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
