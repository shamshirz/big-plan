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

## Released loop workflow (Rust PoC)

When this repository (or an installed `loop` binary from `loop-cli`) drives work:

1. **Project bootstrap:** humans run `loop init` once per repo; context lives under `.loop/`.
2. **Task intake:** `loop add "<title>"` appends a pending task; `loop status` / `loop show <id>` inspect the queue.
3. **Agent session:** the runner ensures exactly one task is **`running`** (today this may be an external orchestrator while `loop run` agent spawning is still landing). Agents pull canonical text with `loop read plan`, `loop read current`, or `loop read <id>`.
4. **Wrap-up:** agents run `loop complete --notes "..."` to persist notes and mark **complete**; use `loop reset <id>` to return a task to **pending** if work must be redone.

Behavior details and error messages are specified in `.loop/cli-contract.md`; user-facing overview is in `README.md`.
