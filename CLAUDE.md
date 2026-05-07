# CLAUDE.md

## Repository Intent

This repository defines and evolves `loop`, a task-loop orchestration CLI. The active initiative is a Rust + SQLite PoC release with strong project-local isolation and an explicit public CLI API.

## Agent Working Norms

- Favor focused, independent tasks over broad mixed-scope changes.
- Build only the context needed for the current task.
- Keep functional core logic pure; isolate side effects in shell adapters.
- Preserve user-visible behavior unless the task explicitly changes it.

## PoC Product Requirements

- `cargo install` friendly executable.
- Project-local hidden state in the cwd where CLI is invoked.
- SQLite-backed task/event persistence within that hidden directory.
- Sequential task execution with clear status and completion metadata.

## Public CLI Surface (Target)

- `loop init`
- `loop add "<title>"`
- `loop status`
- `loop show <task-id>`
- `loop read plan|current|<task-id>`
- `loop run`
- `loop complete [--notes "..."]`
- `loop reset <task-id>`

## Prompt Layering Contract

Each loop-run task agent should receive:
1. universal guidance
2. project-specific context
3. task-specific context

This order is intentional and should be preserved.
