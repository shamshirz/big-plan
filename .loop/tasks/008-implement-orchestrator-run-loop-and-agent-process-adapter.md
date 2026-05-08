---
id: 008
title: Implement orchestrator run loop and agent process adapter
status: complete
depends_on: []
created_at: 2026-05-07T18:32:47Z
started_at: 2026-05-08T00:39:07Z
completed_at: 2026-05-08T00:41:44Z
duration_seconds: 179
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
- [x] `loop run` processes pending tasks one-by-one until completion or failure.
- [x] Agent invocation receives layered prompt context (universal -> project -> task).
- [x] Exit codes, failure handling, and task state transitions are deterministic.
- [x] Runtime metadata (model/tokens/duration) is captured when available.

---

## Completion notes

Implemented sequential `execute_run` in `loop-rs/src/orchestrator.rs`: transition pending→running (`transition_start`), persist, print `Running task`, spawn `LOOP_RUN_AGENT_SHELL` / `LOOP_RUN_AGENT_SCRIPT` (default `sh` + `true`) with layered stdin prompt (built-in universal block + `.loop/agent-project.md` via `read_agent_project` + `render_task_markdown`). Child stdout/stderr inherit; stdin carries the assembled prompt.

On nonzero agent exit the running task transitions to failed (`transition_fail`) with duration and emits contract-shaped stderr; exit 1. On exit 0 reloads SQLite: complete continues the loop until no pending tasks; running without completion emits stall error referencing `loop complete` / reset; unexpected status yields exit 2.

`commands::complete` reads optional `LOOP_COMPLETE_MODEL`, `LOOP_COMPLETE_INPUT_TOKENS`, `LOOP_COMPLETE_OUTPUT_TOKENS`, `LOOP_COMPLETE_COMMIT_SHA` when the agent persists completion.

Verification: `cd loop-rs && cargo test` (90 unit tests + 12 CLI integration tests, including sequential run, stall, failure persistence, shell-driven `complete`).
