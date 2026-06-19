# CLAUDE.md

## Repository Intent

This repository defines **big-plan**: a task-orchestration CLI invoked as **`bp`**, backed by Rust + SQLite with project-local isolation. The supported implementation lives in **`bp-rs/`** (see `README.md`).

## Agent Working Norms

- Favor focused, independent tasks over broad mixed-scope changes.
- Build only the context needed for the current task.
- Keep functional core logic pure; isolate side effects in shell adapters.
- Preserve user-visible behavior unless the task explicitly changes it.

## PoC Product Requirements

- `cargo install --path bp-rs` friendly executable: crate **`big-plan`** produces binary **`bp`**.
- Project-local hidden state in `.loop/` (gitignored runtime; not source of truth in git).
- SQLite-backed tasks, goals, and events.
- Sequential task execution with clear status and completion metadata.

## Public CLI Surface (PoC release)

- `bp init`
- `bp goal new` / `bp goal list`
- `bp run [plan.md] [--model <cursor-model-id>] [--backend cursor|claude]`
- `bp add "<title>"`
- `bp status`
- `bp show <task-id>`
- `bp read plan|current|<task-id>`
- `bp complete [--notes "..."] [--if-running]`
- `bp reset <task-id>`
- `bp summary [--json]`

## Prompt Layering Contract

Each `bp run` task agent should receive:
1. universal guidance (abbreviated `bp` usage + `.loop/SKILL.md` path)
2. project-specific context (`.loop/agent-project.md`)
3. task-specific context (SQLite task rendered as markdown)

Planning tasks (`bp run plan.md`) use plan-decomposition guidance instead of normal task markdown.

## Docs split

- **`SKILL.md`** — using `bp` in any project (copied to `.loop/SKILL.md` on init).
- **`AGENT.md`** — agents modifying this repo.
