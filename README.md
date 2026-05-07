# loop

`loop` is a project-local CLI that orchestrates task-focused agent sessions backed by SQLite.
Install with `cargo install loop` and run from any project directory.

## Quick start

```bash
loop init                        # bootstrap .loop/ state in current directory
loop add "Scaffold Rust crate"   # add a pending task
loop status                      # list tasks
loop run                         # execute pending tasks via agent sessions
```

Inside an agent session:

```bash
loop read plan                   # print plan context
loop read current                # print the running task
loop complete --notes "Done."    # mark task complete
```

## Command reference

| Command | Description |
|---------|-------------|
| `loop init` | Initialize `.loop/` state and SQLite DB in the current directory |
| `loop add "<title>"` | Add a new pending task |
| `loop status` | List all tasks with ID, status, and title |
| `loop show <id>` | Print full task detail |
| `loop read plan\|current\|<id>` | Print raw planning or task text for agent use |
| `loop run` | Execute pending tasks sequentially via agent sessions |
| `loop complete [--notes "..."]` | Mark the current task complete |
| `loop reset <id>` | Return a task to pending and clear metrics |

`loop -h` prints concise help. `loop <command> -h` prints command-specific help.

## How it works

1. `loop init` creates `.loop/loop.db` (SQLite) and context template files.
2. Add tasks with `loop add`; each gets a sequential ID and `pending` status.
3. `loop run` executes pending tasks one at a time. For each task it assembles an agent prompt from universal guidance + `.loop/agent-project.md` + task-specific context, then spawns an agent session.
4. The agent reads context via `loop read`, does the work, then calls `loop complete --notes "..."`.
5. The orchestrator records metrics and commits after each successful task.

## Project context customization

- **`.loop/agent-project.md`** — stable project guidance (architecture, test commands, coding standards). Included in every agent prompt.
- **`.loop/plan.md`** — evolving release plan, readable via `loop read plan`.
- **`.loop/cli-contract.md`** — authoritative CLI behavior contract for implementors.

## Reference docs

- `.loop/plan.md` — release/spec plan
- `.loop/cli-contract.md` — explicit CLI behavior contract (outputs, errors, exit codes)
