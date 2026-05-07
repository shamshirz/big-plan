---
id: 011
title: Add integration tests for end-to-end CLI behavior
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

# Task 011: Add integration tests for end-to-end CLI behavior

## Description
Add end-to-end integration tests validating CLI flows from init through run/complete/reset behavior.

## Context
- Focus on black-box CLI behavior.
- Avoid reworking internals unless required to make behavior testable.
- Inputs: finalized CLI contract and implemented adapters.

## Acceptance criteria
- [ ] Integration tests cover happy path and at least one failure path for `run`.
- [ ] Tests verify project-local isolation (separate directories have separate state).
- [ ] Tests verify help behavior and core command UX.
- [ ] Test harness is repeatable in CI/local environments.

---

## Completion notes
[Fill this section before running `loop complete`.]
