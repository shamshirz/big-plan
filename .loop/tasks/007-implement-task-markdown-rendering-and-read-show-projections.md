---
id: 007
title: Implement task markdown rendering and read/show projections
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
[Fill this section before running `loop complete`.]
