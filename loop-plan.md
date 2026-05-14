> **Shipped CLI:** Use **`bp`** (Rust + SQLite). This file is the original markdown-on-disk PoC spec; wherever it says **`loop`** as a command, invoke **`bp`** instead (see root **`README.md`**).

# Historical spec: markdown-first task orchestration

## What we're building

A CLI that orchestrates Claude Code sessions through a checked-in task list. State lives in flat markdown files **outside** the agent's context window. A deterministic outer loop picks the next pending task, spawns a fresh `claude -p` session for it, waits for the session to mark the task complete, commits the work, and recurses.

The point: each task gets a fresh context window. Big multi-step plans stop degrading as token counts grow. The plan is the source of truth, not the agent's memory.

---

## Design principles (do not violate)

1. **State lives in files, not in agents.** Each task is a markdown file with frontmatter. The orchestrator and the agent both read/write these files. That's the only shared state.
2. **Mutating ops go through the CLI; reads are direct file I/O.** Agents are good at reading files — don't wrap that. Only wrap state changes so the contract is stable and observable.
3. **Orchestrator owns git, not the agent.** Agents edit files. The outer loop commits after each task with a structured message. Simpler agent contract, cleaner history.
4. **Sequential first.** v1 is a linked list of tasks. The schema accommodates `depends_on:` so we can add parallel later without redesigning anything.
5. **Resist gold-plating.** See "Out of scope" below.

---

## Storage layout

```
<repo>/
  .loop/
    plan.md                  # the overarching plan (written by the planning agent)
    CURRENT                  # symlink → tasks/NNN-*.md (set by orchestrator before spawning)
    tasks/
      001-set-up-schema.md
      002-add-auth.md
      003-add-tests.md
    log.jsonl                # one line per orchestrator event (optional, append-only)
```

`.loop/` is checked in. The whole point of this tool is the plan + history is a first-class artifact of the work, like `beads`.

---

## Task file format

Each task file is markdown with YAML frontmatter:

```markdown
---
id: 003
title: Add user authentication
status: pending          # pending | running | complete | failed
depends_on: []           # list of task IDs; v1 leaves empty (sequential)
created_at: 2026-05-07T10:30:00Z
started_at: null
completed_at: null
duration_seconds: null
input_tokens: null
output_tokens: null
model: null
commit_sha: null
---

# Task 003: Add user authentication

## Description
[Written by the planning agent. What this task should accomplish.]

## Context
[Optional. References to prior tasks, plan sections, files to look at.]

## Acceptance criteria
- [ ] [Optional checklist]

---

## Completion notes
[Filled in by the executing agent before it runs `bp complete`.
Should be detailed enough that downstream tasks can rely on it without
re-deriving anything. If the agent had to make a non-obvious decision,
record it here.]
```

The body sections above the `---` divider are inputs (written by the planner). Below it is the agent's output. Easy to eyeball.

---

## CLI surface

Four commands. Three more are nice-to-haves.

### Required

#### `bp init`

- Creates `.loop/` if not present
- Writes a `plan.md` template
- Creates a git branch `loop/<UTC-timestamp>` from current HEAD
- Idempotent: if `.loop/` exists, error out (don't clobber)

#### `bp add "<title>"`

- Allocates the next ID (zero-padded, 3 digits)
- Slugifies the title for the filename: `NNN-<slug>.md`
- Writes the file with frontmatter populated and template body
- Prints the path so the planning agent can immediately edit it
- The planning agent fills in Description / Context / Acceptance criteria via its normal file-edit tools

#### `bp run`

The outer loop. See pseudocode below.

#### `bp complete [--notes "..."]`

- Run from inside an executing agent's session
- Reads `LOOP_TASK_ID` from env (set by `bp run` before spawning) — **Python only**; **`bp`** binds the running task from SQLite.
- Sets that task's `status: complete`, `completed_at: now`
- If `--notes` given, appends to the Completion notes section (agent can also just edit the file directly — both work)
- Exits 0; agent's session should exit immediately after

### Nice-to-have (add only if trivial)

- `bp status` — prints all tasks with status, for humans
- `bp show <id>` — prints a task file (`cat`, basically)
- `bp reset <id>` — sets a task back to pending; for re-running

---

## The outer loop (`bp run`)

Pseudocode:

```
while True:
    task = find_next_pending_task()
    if task is None:
        print summary; exit 0

    set task.status = "running"
    set task.started_at = now()
    write task

    point .loop/CURRENT symlink to task file
    set env LOOP_TASK_ID = task.id

    start_time = now()
    proc = spawn(
        "claude", "-p", AGENT_PROMPT,
        "--output-format", "stream-json",
        "--dangerously-skip-permissions",   # if appropriate
        env={..., LOOP_TASK_ID: task.id}
    )

    last_result = None
    for line in proc.stdout:
        relay to terminal
        try parse JSON
        if event.type == "result": last_result = event

    proc.wait()
    duration = now() - start_time

    re-read task file from disk
    if task.status != "complete":
        set task.status = "failed"
        write task
        print error; exit 1

    record token usage from last_result into task frontmatter
    record duration
    write task

    git add -A
    if there are staged changes:
        git commit -m "task {id}: {title}" -m "{first 500 chars of completion notes}"
        record commit_sha into task
    else:
        record commit_sha = null (no-op task)

    write task
    append event to log.jsonl
```

Key behaviors:

- The agent is responsible for setting `status: complete` (via `bp complete` or by editing frontmatter directly). If it exits without doing so, that task is marked `failed` and the loop **stops** — don't try to recover, just bail and let the human inspect.
- Failure halts the loop. v1 doesn't retry.
- Don't capture-and-replay `claude` output; stream it to the terminal so the human can watch.

---

## The agent prompt (constant across tasks)

```
You are working on a single task in a multi-task plan.

1. Read your current task: `.loop/CURRENT` (symlink to your task file).
2. Read the overall plan: `.loop/plan.md`.
3. If your task references other task IDs, read those files in `.loop/tasks/`.
   Earlier tasks' "Completion notes" tell you what's already done.
4. Do the work. Use your normal tools to read code, edit files, run tests.
5. Before finishing, edit the "Completion notes" section of your task file.
   Be specific: what files you touched, what decisions you made, what
   the next task can rely on.
6. When done, run `bp complete`. Then exit.

You do not need to commit. The orchestrator commits after you exit.
You do not need to manage branches.
Do not edit other tasks' files unless your task explicitly says to.
```

This prompt is static. Task-specific content reaches the agent via the task file, not via the prompt — keeps the prompt small and the contract observable.

---

## Git workflow

- `bp init` creates `loop/<timestamp>` branch
- After each successful task, orchestrator commits everything in the working tree on that branch
- Commit message format: `task {id}: {title}` with the first ~500 chars of completion notes as the body
- Commit SHA written back into the task frontmatter (`commit_sha:`)
- That gives you: `git log` shows task progression, each task file has its commit SHA, each session can be linked to a specific commit

The branch name plus task IDs makes correlating "which agent session produced which commit" trivial later.

---

## Implementation guidance

**Language:** Rust (crate **`big-plan`**, binary **`bp`**) in `bp-rs/`, SQLite via `rusqlite`. The historical Python stdlib-only prototype has been removed from this repo.

**Layout:**

```
bp-rs/
  Cargo.toml
  src/
    main.rs, cli.rs, commands.rs, domain.rs, repository.rs, sqlite_repo.rs,
    orchestrator.rs, render.rs, …
```

Earlier design notes in this document that reference Python symlinks or `loop_lib/` describe the **retired** implementation; **`bp`** behavior is authoritative.

**Stream-json parsing:** the last JSON object emitted by `claude -p --output-format stream-json` has `type: "result"` and includes `usage.input_tokens`, `usage.output_tokens`, `total_cost_usd`, `duration_ms`. Capture that line, ignore the rest for stats purposes (still relay everything to the terminal).

**Symlinks on Windows:** don't worry about it. PoC is Unix-only. Document this.

---

## Build order

Implement in this order. Don't move on until each step works end-to-end.

1. **Scaffold + `init` + `add`.** Just file creation. Verify by hand that `.loop/` and task files look right.
2. **`bp run` skeleton.** Find next pending task, mark running, mark complete, advance — but instead of spawning Claude, just `print("would run task", id)` and immediately set status=complete. Confirm the loop terminates on an empty queue.
3. **Wire in `claude -p`.** Spawn it with the prompt, stream output, wait. No JSON parsing yet. Verify a real task runs end-to-end with the agent calling `bp complete` itself.
4. **Token/time capture.** Parse `--output-format stream-json`, extract usage from the final `result` event, write into frontmatter.
5. **Git integration.** Branch creation in `init`, auto-commit after each task, SHA back into frontmatter.
6. **Polish.** `bp status`, error messages, the `log.jsonl` append.

Stop after step 6. Don't add features not on this list.

---

## Out of scope (do NOT build)

- Parallel execution / DAG scheduling. Schema supports it, runtime doesn't.
- Resume after orchestrator crash. (Tasks left in `running` state on next `run` are the human's problem to inspect.)
- Retry on failure. Failure halts the loop, full stop.
- TUI / web UI / dashboards.
- Notifications, Slack integration, anything cross-process.
- Multi-repo support.
- Locking. Single-user, single-machine, single-process.
- Configuring the model, tool budget, or agent permissions per task. v1 uses the same `claude -p` invocation for every task.

---

## Acceptance test (PoC)

1. `cd` into a clean git repo.
2. `bp init`. See `.loop/` and a new branch.
3. Manually write a `plan.md` with a 3-task plan ("create a hello.py, add a test, run the test").
4. `bp add "create hello.py"` × 3, edit each task's Description.
5. `bp run`.
6. Watch three Claude sessions execute in series. Each commits one (or more) commit on the branch.
7. After it exits, every task file has `status: complete`, populated `completion_notes`, `input_tokens`, `output_tokens`, `duration_seconds`, `commit_sha`.
8. `git log` shows three commits with task IDs in messages.

If all eight pass, the PoC is done.