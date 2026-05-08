# loop

`loop` is a project-local CLI that orchestrates task-focused agent sessions backed by SQLite.
The Rust implementation lives in `loop-rs/` (crate **`loop-cli`**, binary **`loop`**).

## Install

- **Prerequisites:** a stable Rust toolchain (`rustup` recommended).
- **From this repository (development / local PoC):**
  ```bash
  cargo install --path loop-rs
  ```
  This builds the `loop` binary and installs it into `~/.cargo/bin` (ensure that directory is on your `PATH`).
- **From crates.io (after publish):** the crate name is `loop-cli`, not `loop`:
  ```bash
  cargo install loop-cli
  ```
  The installed executable is still named `loop`.

## Quick start

```bash
cd your-project
loop init                        # bootstrap .loop/ state in current directory
loop add "Scaffold Rust crate"   # add a pending task (quote titles with spaces)
loop status                      # list tasks
loop run                         # reserved for sequential agent sessions (see PoC checklist)
```

Workflow when a task is in **`running`** status (for example after an external runner sets it, or in tests):

```bash
loop read plan                   # print plan context (.loop/plan.md)
loop read current                # print the running task as markdown
loop read <id>                   # print a specific task by id
loop complete --notes "Done."    # mark the running task complete
loop show <id>                   # inspect full task detail
```

## Command reference

| Command | Description |
|---------|-------------|
| `loop init` | Initialize `.loop/` state and SQLite DB in the current directory |
| `loop add <title>` | Add a new pending task (use quotes if the title contains spaces) |
| `loop status` | List all tasks with ID, status, and title |
| `loop show <id>` | Print full task detail |
| `loop read plan`, `loop read current`, or `loop read <id>` | Print planning or task markdown for agent use |
| `loop run` | Advance the pending queue; full agent subprocess wiring is not implemented yet (PoC) |
| `loop complete [--notes "..."]` | Mark the **current running** task complete (optional notes) |
| `loop reset <id>` | Return a task to **pending** and clear completion metrics |

`loop`, `loop -h`, and `loop --help` print the same concise usage and exit 0.

## How it works

1. `loop init` creates `.loop/loop.db` (SQLite) and seed files such as `.loop/plan.md` and `.loop/agent-project.md`.
2. Add tasks with `loop add`; each gets a stable id and `pending` status until started.
3. **Target behavior:** `loop run` should execute pending tasks one at a time, composing prompts from universal guidance + `.loop/agent-project.md` + the task record, then driving an agent session.
4. **PoC today:** `loop run` selects the next pending task and prints intent, but **does not** spawn an agent subprocess yet; use external orchestration or tests that put tasks into `running` while that path is finished.
5. Inside a session, the agent reads context with `loop read`, performs work, then runs `loop complete [--notes ...]` so SQLite records completion notes, timestamps, and status.

## PoC release checklist

Use this list before calling the PoC “done” for an external audience.

### Validation

- [ ] `cargo test` passes in `loop-rs/` (unit + integration tests against the real binary).
- [ ] Smoke the CLI in a clean directory: `loop init`, `loop add`, `loop status`, `loop show`, `loop read plan`.
- [ ] Confirm persistence: `.loop/loop.db` exists; task rows survive a new shell / cwd session in the same project.

### Known limitations (PoC)

- **`loop run`:** agent subprocess invocation is not implemented; the command exits with an error after announcing the task (see source: `commands.rs`). External runners may transition tasks to `running` and invoke agents until this is wired.
- **Parallelism / remotes:** single serial queue, project-local SQLite only (no sync service).
- **Publishing:** crates.io name is `loop-cli`; installing the binary `loop` may require `PATH` to include `~/.cargo/bin`.

### Next steps

- Implement agent subprocess and prompt assembly inside `loop run` (or document a supported external adapter contract).
- Publish `loop-cli` to crates.io and semver the schema/migrations story.
- Expand integration coverage around failure paths and dependency ordering once `depends_on` is exercised in the runner.

## Project context customization

- **`.loop/agent-project.md`** — stable project guidance (architecture, test commands, coding standards). This is the intended project slice when prompts are composed for each task.
- **`.loop/plan.md`** — evolving release plan, readable via `loop read plan`.
- **`.loop/cli-contract.md`** — authoritative CLI behavior contract for implementors.

## Reference docs

- `.loop/plan.md` — release/spec plan
- `.loop/cli-contract.md` — explicit CLI behavior contract (outputs, errors, exit codes)
- `CLAUDE.md` / `AGENT.md` — shared guidance for humans and coding agents using loop
