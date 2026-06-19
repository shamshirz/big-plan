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
bp status             # active goal + task queue
bp show <id>          # human-readable task detail
bp reset <id>         # retry a stuck task
bp goal list          # all goals in this project
```

## Task quality checklist

- One major concern per task.
- Clear acceptance checks in completion notes.
- No unrelated refactors.
- Serializable: another agent could pick it up cold.
