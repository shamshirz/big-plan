# AGENT.md

## Why This Exists

**big-plan** (`bp`) decomposes project work into narrow, high-signal tasks that can be executed by agents with bounded context.

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

## Released `bp` workflow (Rust)

When this repository (or an installed **`bp`** binary from crate **`big-plan`**) drives work:

1. **Project bootstrap:** run `bp init` once per repo; SQLite and templates live under `.loop/`.
2. **Task intake:** `bp add "<title>"` creates a pending task; `bp status` / `bp show <id>` inspect the queue.
3. **Agent session:** `bp run [--model <id>]` marks the next pending task **running** and spawns your agent hook (`BP_RUN_AGENT_SCRIPT`, etc.; default Cursor backend accepts `--model`, e.g. `composer-2.5`). Agents read canonical text with `bp read plan`, `bp read current`, or `bp read <id>`.
4. **Wrap-up:** agents run `bp complete --notes "..."` to persist notes and mark **complete**; `bp reset <id>` returns a task to **pending** if work must be redone. `bp status` shows active runs (pid + task) or warns when a task is stale **running** after interrupt/sleep.

For CI and deterministic integration tests, `BP_RUN_SKIP_AGENT=1` completes tasks without spawning an agent.

Behavior details and error messages are specified in `.loop/cli-contract.md`; user-facing overview is in `README.md`.
