# AGENT.md

## Why This Exists

`loop` is designed to decompose project work into narrow, high-signal tasks that can be executed by agents with bounded context.

## Task Decomposition Heuristics

- One major concern per task.
- Prefer serializable units that can be reviewed independently.
- Split design-heavy and implementation-heavy work.
- Separate data model design from DB integration code.
- Separate persistence integration from CLI UX shaping.

## Context Is A Feature

Rebuilding context per task is beneficial when tasks are compartmentalized.
Use this to reduce prompt bloat and improve focus.

Examples of good separation:
- CLI public API contract task
- SQLite schema and migrations task
- Rust domain model/state transition task
- Agent orchestration shell adapter task

## Completion Standard Per Task

- Update task acceptance criteria.
- Document completion notes with concrete changes and validation.
- Avoid unrelated refactors.
