# Loop Rust+SQLite Project Context

## Mission

Build and release a PoC Rust implementation of `loop` that is installable with `cargo install`, uses project-local SQLite persistence, and supports focused task-by-task execution through agent sessions.

## Architectural Guidance

- Prefer functional core + imperative shell.
- Keep domain logic pure where possible (state transitions, validation, rendering).
- Isolate side effects (filesystem, sqlite I/O, subprocess/agent invocation, git operations).
- Keep public CLI behavior stable and explicit.

## Context Strategy

Treat small, focused context windows as an optimization:
- Design tasks so each one needs only a narrow knowledge slice.
- Avoid mixing deep SQLite concerns with advanced Rust ownership concerns in one task.
- Separate API design from persistence implementation where possible.

## Baseline Workflow Expectations

- `loop init` bootstraps hidden project state in current working directory.
- `loop run` executes pending tasks one at a time.
- Task agents update completion notes and mark completion.
- Orchestrator records usage/metrics and handles commit boundaries.

## Quality Bar

- deterministic CLI outputs and errors
- repeatable tests
- clear migration/version strategy for SQLite schema
- concise documentation that explains operations, not implementation trivia

## Files To Keep In Sync

- `.loop/plan.md`: evolving release/spec plan
- `.loop/tasks/*.md`: execution units with acceptance criteria
- `README.md`: user-facing quick start and command reference
- `CLAUDE.md` and `AGENT.md`: shared agent guidance
