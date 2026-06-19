# Skill: Using `bp` in your project

Use this when an agent should plan or execute work through **big-plan** (`bp`).

## Core idea

- A **goal** is one bounded run sequence: decompose a plan into tasks, then execute them.
- Each **task** gets a fresh agent context window — split work so tasks benefit from isolation.
- State lives in SQLite under `.loop/` (runtime only; gitignored).

## Starting a new goal

```bash
bp init                              # once per repo
bp run path/to/plan.md               # new goal + planning agent creates tasks
bp run                               # execute pending tasks in the active goal
bp summary                           # report when the goal finishes
```

To start a fresh goal without a plan file:

```bash
bp goal new
bp add "First task"
bp run
```

## Agent commands (every session)

```bash
bp read plan          # active goal plan text
bp read current       # current running task (markdown)
bp read <id>          # specific task
bp complete --notes "what changed and how you verified"
```

Full skill reference: `.loop/SKILL.md` (this file, copied on `bp init`).

## Planning agent (first task in a goal)

When decomposing a plan into tasks:

1. Read the plan and project context.
2. Create **one task per context window** — narrow, independently reviewable units.
3. Use `bp add "<title>"` for each executable task (do not edit SQLite or `.loop/` files directly).
4. Split design from implementation, schema from integration, API from persistence.
5. Run `bp complete --notes "..."` when the task queue is ready.

## Executor agent (each subsequent task)

1. `bp read current` and `bp read plan`.
2. Implement **only** the current task scope.
3. `bp complete --notes "..."` with concrete changes and verification commands.

## Human commands

```bash
bp status             # live goal dashboard (see below)
bp show <id>          # human-readable task detail
bp reset <id>         # retry a stuck task
bp goal list          # all goals in this project
```

## Checking in with `bp status`

Run `bp status` to see where the active goal stands — progress bar, digest, and per-task metrics in one glance (no TUI, no browser).

Typical output:

```
Goal 1 (active): My feature

Progress: 2/4 complete · 1 running · 1 pending          [███████░░░░░░░] 50%

Digest: Run in progress · started 12m ago · agent time 9m · now on 003 "Wire API" (3m) · Last finished: 002 at 2026-06-19 14:32 UTC · Next: 004 "Docs"

  ID    STATUS     TIME     COMMIT      TITLE
  ✓ 001 complete 4m       abc1234     Scaffold module
  ✓ 002 complete 5m       def5678     Add persistence
  ▶ 003 running  3m       —           Wire API
  · 004 pending  —        —           Docs

Active bp run: task 003 (pid 12345)
```

- **Progress** — counts plus a bar (`complete / total`)
- **Digest** — run state, wall-clock and agent time, current task, last finished (with commit), next pending, slowest completed
- **Task table** — `✓` complete, `▶` running, `·` pending, `✗` failed; per-task elapsed time and commit SHA

Use `bp summary` for the full retrospective (tokens, `--last-run`, `--since`).

## Task quality checklist

- One major concern per task.
- Clear acceptance checks in completion notes.
- No unrelated refactors.
- Serializable: another agent could pick it up cold.
