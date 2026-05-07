# Loop CLI Contract

This document is the authoritative user-facing behavior contract for the Rust PoC.
Implementation tasks must conform to these specs without re-litigating them.

---

## Help Behavior

```
loop
loop -h
loop --help
```

All three print the same concise help block and exit 0:

```
loop — project-local task orchestration

Usage:
  loop <command> [args]

Commands:
  init                  Initialize loop state in the current directory
  add "<title>"         Add a new pending task
  status                List all tasks with ID, status, and title
  show <id>             Print full task detail
  read plan|current|<id>  Print planning or task text for agent use
  run                   Execute pending tasks sequentially via agent sessions
  complete [--notes ""] Mark the current task complete
  reset <id>            Return a task to pending and clear metrics

Run `loop <command> -h` for command-specific help.
```

Unknown commands exit 1 with:
```
error: unknown command '<cmd>'
Run `loop -h` for usage.
```

---

## loop init

### Normal case (first run in a directory)

Creates `.loop/` hidden directory structure and SQLite database.

```
$ loop init
Initialized loop state in .loop/
```

Side effects:
- Creates `.loop/` directory
- Creates `.loop/loop.db` (SQLite, schema version 1)
- Creates `.loop/plan.md` (empty template with placeholder headings)
- Creates `.loop/agent-project.md` (empty template with placeholder headings)

Exit code: 0

### Already initialized

```
$ loop init
Loop already initialized in .loop/
```

Does not overwrite existing data. Exit code: 0

### Error cases

- Cannot create `.loop/` (permission denied):
  ```
  error: cannot create .loop/ — permission denied
  ```
  Exit code: 1

---

## loop add "<title>"

### Normal case

```
$ loop add "Scaffold Rust crate"
Added task 003: Scaffold Rust crate
```

Side effects:
- Inserts a task row with status `pending`, `created_at = now()`, next sequential ID.

### Missing title

```
$ loop add
error: title is required
Usage: loop add "<title>"
```
Exit code: 1

### Empty title

```
$ loop add ""
error: title must not be empty
```
Exit code: 1

### Not initialized

All commands that require `.loop/loop.db` emit:
```
error: loop not initialized — run `loop init` first
```
Exit code: 1

---

## loop status

### Normal case (tasks exist)

Tabular output, one task per line:

```
ID    STATUS    TITLE
001   complete  Define CLI contract
002   running   Scaffold Rust crate
003   pending   Implement SQLite schema
```

Column widths are fixed: ID (5), STATUS (10), TITLE (rest of line, truncated at 60 chars).

### No tasks

```
No tasks. Run `loop add "<title>"` to create one.
```

Exit code: 0

### Not initialized → standard error (see above)

---

## loop show <id>

### Normal case

Prints all fields of the task in human-readable markdown-ish format:

```
ID:      003
Title:   Implement SQLite schema
Status:  pending
Created: 2026-05-07T18:32:46Z

Description:
<description_md contents, or "(none)")>

Context:
<context_md contents, or "(none)">

Acceptance criteria:
<acceptance_md contents, or "(none)">

Completion notes:
<completion_notes_md contents, or "(none)">
```

Runtime metrics (started_at, completed_at, duration_seconds, model, input_tokens,
output_tokens, commit_sha) are appended only when non-null:

```
Started:   2026-05-07T19:00:00Z
Completed: 2026-05-07T19:12:34Z
Duration:  754s
Model:     claude-sonnet-4-6
Tokens in/out: 12430 / 3201
Commit:    83e1245
```

### Task not found

```
error: task '999' not found
```
Exit code: 1

### Missing argument

```
error: task id is required
Usage: loop show <id>
```
Exit code: 1

---

## loop read plan|current|<id>

Outputs raw markdown text suitable for piping into an agent prompt.
No decorators, headers, or ANSI codes in the output.

### `loop read plan`

Prints contents of `.loop/plan.md`.

If `.loop/plan.md` is empty or does not exist:
```
(no plan content)
```
Exit code: 0

### `loop read current`

Prints the full markdown of the task currently in `running` status.

If no task is running:
```
error: no task is currently running
```
Exit code: 1

### `loop read <id>`

Prints the full markdown of the specified task (same content as `loop show <id>`
but as raw markdown, not formatted output).

Task not found:
```
error: task '<id>' not found
```
Exit code: 1

---

## loop run

Starts executing pending tasks one at a time. For each task:

1. Sets task status to `running`, records `started_at`.
2. Assembles agent prompt (universal + project context + task context).
3. Spawns agent subprocess.
4. On agent exit 0: records `completed_at`, `duration_seconds`, metrics; transitions
   task to `complete` (if the agent ran `loop complete`) or leaves as `running` for
   the orchestrator to flag as stalled.
5. On agent exit non-0: transitions task to `failed`; prints error and stops.

### Normal session start

```
$ loop run
Running task 003: Implement SQLite schema
[agent session output...]
Task 003 complete.
Running task 004: Add CLI scaffold
...
```

### No pending tasks

```
No pending tasks.
```
Exit code: 0

### A task is already running

```
error: task 002 is already running — complete or reset it before running again
```
Exit code: 1

### Agent subprocess fails (non-zero exit)

```
error: task 003 failed (exit code 1) — check agent output above
Loop stopped. Fix the task or run `loop reset 003` to retry.
```
Exit code: 1

---

## loop complete [--notes "<text>"]

Marks the currently running task as `complete` and appends optional notes.
Intended to be called by the agent subprocess during a task session.

### Normal case

```
$ loop complete --notes "Implemented schema v1, verified with sqlite3."
Task 002 marked complete.
```

Side effects:
- Sets task status to `complete`.
- Sets `completed_at = now()`.
- Appends `--notes` text to `completion_notes_md`.
- Computes and stores `duration_seconds`.

### No running task

```
error: no task is currently running
```
Exit code: 1

### Notes flag without value

```
error: --notes requires a value
Usage: loop complete [--notes "<text>"]
```
Exit code: 1

---

## loop reset <id>

Returns a task to `pending` and clears all runtime metrics.

### Normal case

```
$ loop reset 003
Task 003 reset to pending.
```

Side effects:
- Sets `status = pending`.
- Clears `started_at`, `completed_at`, `duration_seconds`, `input_tokens`,
  `output_tokens`, `model`, `commit_sha`.
- Does NOT clear `completion_notes_md` (preserved for reference).

### Task not found

```
error: task '999' not found
```
Exit code: 1

### Missing argument

```
error: task id is required
Usage: loop reset <id>
```
Exit code: 1

---

## Exit Code Summary

| Condition                      | Exit code |
|-------------------------------|-----------|
| Success                        | 0         |
| Invalid usage / bad args       | 1         |
| Not initialized                | 1         |
| Entity not found               | 1         |
| Agent subprocess failure       | 1         |
| Unexpected internal error      | 2         |

---

## Output Rules

- All user-facing output goes to **stdout**.
- All `error:` lines go to **stderr**.
- No ANSI color codes unless `--color` flag is explicitly added later.
- `loop read *` outputs raw markdown with no wrapper or decoration.
- Timestamps are always ISO 8601 UTC (`2026-05-07T18:32:46Z`).
- IDs are zero-padded to 3 digits (e.g., `001`, `042`).

---

## Invariants

- Only one task may have status `running` at a time.
- `loop run` will not start a new task if one is already running.
- Task IDs are immutable after creation.
- `loop reset` is the only operation that transitions a task backward.
- `loop complete` transitions only `running → complete`.
