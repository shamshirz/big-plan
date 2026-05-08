---
id: 008
title: Implement orchestrator run loop and agent process adapter
status: failed
depends_on: []
created_at: 2026-05-07T18:32:47Z
started_at: 2026-05-08T00:25:23Z
completed_at: null
duration_seconds: null
input_tokens: null
output_tokens: null
model: null
commit_sha: null
---

# Task 008: Implement orchestrator run loop and agent process adapter

## Description
Implement the sequential `run` orchestrator and agent subprocess adapter that executes pending tasks and captures outcomes.

## Context
- Focus on run-loop control flow and process boundary behavior.
- Use repository/domain interfaces; avoid deep DB schema edits here.
- Inputs: domain transition rules and CLI contract.

## Acceptance criteria
- [ ] `loop run` processes pending tasks one-by-one until completion or failure.
- [ ] Agent invocation receives layered prompt context (universal -> project -> task).
- [ ] Exit codes, failure handling, and task state transitions are deterministic.
- [ ] Runtime metadata (model/tokens/duration) is captured when available.

---

## Completion notes
[Fill this section before running `loop complete`.]
