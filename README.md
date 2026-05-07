# big-plan

`big-plan` is a lightweight CLI loop for running coding tasks as separate agent sessions, with state tracked in files and commits.

## Core idea

- Keep shared state in `.loop/` files (plan + tasks), not in a long-lived chat context.
- Let a planner define tasks, then let `loop run` execute them one by one.
- Require each task agent to write completion notes and mark itself complete.
- Let the orchestrator commit after each successful task.

## Quick start

```bash
# from repo root
./loop init
./loop add "First task title"
./loop status
./loop run
```

Inside an executing agent session:

```bash
./loop read current
./loop read plan
./loop complete
```

## Planner -> task loop workflow

1. A planning agent writes/updates `.loop/plan.md` and creates tasks.
2. Tasks are represented by IDs and markdown files in `.loop/tasks/`.
3. Run `./loop run` to execute pending tasks sequentially.
4. Each task agent reads context with `./loop read plan` and `./loop read current`, implements the task, updates completion notes, then runs `./loop complete`.
5. The orchestrator records usage/time metadata and commits task results.

## Reference docs

- `loop-plan.md` — implementation/design spec
- `SKILL.md` — reusable prompt and execution pattern for planner/executor agents
