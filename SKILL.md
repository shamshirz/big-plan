# Skill: Planner-to-Loop Execution

Use this when you want one agent to plan and task work, and loop workers to execute each task in isolated sessions.

## Always include this context for agents

```text
Project context:
- Read README.md for workflow overview.
- Read loop-plan.md for design constraints.
- Use loop commands (not raw .loop file browsing) for plan/task reads:
  - ./loop read plan
  - ./loop read current
  - ./loop read <task-id>
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
   - `./loop read current`
   - `./loop read plan`
2. Implement only the current task scope.
3. Add useful completion notes to the task file.
4. Mark done with:

```bash
./loop complete
```

Do not commit manually; the loop orchestrator commits after each successful task.

## Human/operator commands

```bash
# start or continue queued execution
./loop run

# inspect queue
./loop status
```

## Recommended handoff template

```text
Task: <id + title>
Context: <important repo state>
Do now: <single focused objective>
Verify: <exact command(s)>
Notes: <constraints/decisions for downstream tasks>
```
