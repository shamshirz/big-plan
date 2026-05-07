# PoC Release Plan: Rust + SQLite Loop CLI

## Purpose

Ship a production-credible PoC of `loop` as a Rust CLI that orchestrates task-focused agent sessions for software projects. The tool enables users to define work as independent tasks, execute tasks sequentially, and preserve deterministic project-local state for resumability and auditing.

The PoC must prioritize:
- project-local isolation (each repo has its own hidden state)
- clear public CLI API
- reliable, inspectable persistence via SQLite
- task decomposition that rewards narrow, focused context windows

## User Outcomes

Users should be able to:
- install with `cargo install` and run `loop` from any project directory
- initialize loop state in the current directory without affecting other projects
- create, inspect, and run tasks with explicit status and completion notes
- rely on the orchestrator for sequencing, logging, and commit boundaries
- evolve plans through iterative planning tasks that can generate additional tasks

## Non-Goals (PoC)

- No distributed execution or parallel task execution.
- No remote backend service requirement.
- No hard dependency on a specific UI or IDE plugin.
- No assumption that current Python internals are the target architecture.

## Product Constraints

- Invocation context is the current working directory.
- Hidden state directory is project-local (for example `.loop/`).
- SQLite DB is stored in the hidden directory of the invoked project.
- Agent prompt composition must support:
  1) universal guidance
  2) project-specific context
  3) task-specific context
- Functional core + imperative shell should be the dominant implementation shape.

## Public CLI API (Target)

Required commands:
- `loop init`
  - initializes project-local hidden directory and SQLite schema
  - creates seed context artifacts (plan/project context templates)
- `loop add "<title>"`
  - inserts a pending task
- `loop status`
  - lists tasks with ID, status, and title
- `loop show <task-id>`
  - prints full task details
- `loop read plan|current|<task-id>`
  - retrieves canonical planning/task text for agent consumption
- `loop run`
  - executes pending tasks sequentially via agent sessions
- `loop complete [--notes "..."]`
  - marks current agent task complete and appends notes
- `loop reset <task-id>`
  - returns task to pending and clears runtime metrics

Behavioral expectations:
- `loop` and `loop -h` both print concise help.
- command errors are deterministic and actionable.
- project state is never cross-contaminated between directories.

## Data Model Requirements

Persistent entities (SQLite):
- `tasks`
  - `id` (stable task ID; ordered)
  - `title`
  - `status` (`pending|running|complete|failed`)
  - `depends_on` (normalized relation or encoded list)
  - `description_md`
  - `context_md`
  - `acceptance_md`
  - `completion_notes_md`
  - `created_at`
  - `started_at`
  - `completed_at`
  - `duration_seconds`
  - `input_tokens`
  - `output_tokens`
  - `model`
  - `commit_sha`
- `events` (append-only audit log)
  - task lifecycle transitions, agent exit code, commit metadata, timestamps
- optional config/context table
  - project-level prompt context, schema version, and settings

Derived/canonical projections:
- current task pointer
- pending queue ordering
- per-task markdown render for `show/read`

## Task Design Principles

- Single focus domain per task (CLI UX, SQLite schema, Rust ownership boundaries, orchestration engine, etc.).
- Maximize reusability by minimizing irrelevant context in each task.
- Treat context rebuilding as a feature: each task should require only the minimal context needed to execute safely.
- Encourage planning tasks to split future work into smaller, independent tasks.

## Milestones

1. Spec refinement and executable task graph creation.
2. Rust crate scaffolding and CLI surface.
3. SQLite persistence layer and migrations.
4. Domain logic (functional core) for tasks/events/state transitions.
5. Imperative shell orchestration for agent execution + git integration.
6. Validation, docs, and release readiness for PoC.
