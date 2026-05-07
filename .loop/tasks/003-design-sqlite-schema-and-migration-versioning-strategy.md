---
id: 003
title: Design SQLite schema and migration/versioning strategy
status: pending
depends_on: []
created_at: 2026-05-07T18:32:46Z
started_at: null
completed_at: null
duration_seconds: null
input_tokens: null
output_tokens: null
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
[Fill this section before running `loop complete`.]
