# Skill: Planner-to-bp execution

Use this when you want one agent to plan and task work, and executor agents to run each task in isolated sessions.

## Always include this context for agents

```text
Project context:
- Read README.md for workflow overview.
- Read loop-plan.md for design constraints.
- Use `bp` commands (not raw .loop file browsing) for plan/task reads:
  - `bp read plan`
  - `bp read current`
  - `bp read <task-id>`
```

## Planning agent responsibilities

1. Build or refine the project plan.
2. Ensure tasks are concrete and independently executable.
3. Add/adjust tasks so each has:
   - clear objective,
   - acceptance checks,
   - relevant file pointers.
4. Provide a brief handoff note for executors.

## Executor agent responsibilities

1. Read task context:
   - `bp read current`
   - `bp read plan`
2. Implement only the current task scope.
3. Add useful completion notes to the task file.
4. Mark done with:

```bash
bp complete
```

Do not commit manually during agent work unless the task says otherwise; the Rust **`bp`** PoC does not auto-commit after each task (see `README.md`).

## Human/operator commands

```bash
# start or continue queued execution (pin Cursor model when needed)
bp run --model composer-2.5

# inspect queue; shows active bp run or stale-running warning
bp status
```

## Recommended handoff template

```text
Task: <id + title>
Context: <important repo state>
Do now: <single focused objective>
Verify: <exact command(s)>
Notes: <constraints/decisions for downstream tasks>
```
