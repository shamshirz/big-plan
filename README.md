# big-plan

**big-plan** is a project-local CLI (**`bp`**) for orchestrating task-focused agent sessions, with state in **SQLite** under `.loop/` in whatever directory you run it from.

- **Crate name (Cargo):** `big-plan`
- **Binary name:** `bp` (e.g. `bp init`, `bp run`, `bp add "…"`)

## Install

Rust 1.74+ (edition 2021) recommended.

```bash
# From this repo (development)
cargo install --path bp-rs
```

The installed executable is **`bp`**. Ensure `~/.cargo/bin` is on your `PATH`.

After publishing to crates.io: `cargo install big-plan` (binary still **`bp`**).

## Quick start

```bash
cd your-project
bp init
bp add "First concrete task"
bp status
bp run                    # see “Agent / run” below
```

Help: `bp`, `bp -h`, and `bp --help` are equivalent.

## Commands

| Command | Purpose |
|--------|---------|
| `bp init` | Create `.loop/`, `loop.db`, seed `plan.md` and `agent-project.md` |
| `bp add "<title>"` | Add a pending task |
| `bp status` | List tasks (id, status, title) |
| `bp show <id>` | Full task detail |
| `bp read plan` / `current` / `<id>` | Markdown for agents |
| `bp run [--model <id>]` | Run pending tasks one-by-one (agent hook + layered prompt) |
| `bp complete [--notes "..."] [--if-running]` | Mark the **running** task complete (`--if-running` no-ops silently) |
| `bp reset <id>` | Put task back to `pending`; clear run metrics |

## Agent / `bp run`

1. Next **pending** task becomes **running** and receives a prompt: **universal → `.loop/agent-project.md` → task markdown**.
2. `bp run` runs **`$BP_RUN_AGENT_SHELL -c "$BP_RUN_AGENT_SCRIPT"`** with that prompt on the subprocess **stdin** (same pattern as `sh -c '… $(cat) …'`).
3. When the child exits **0**, `bp` checks that the task was marked **complete** (typically the agent ran **`bp complete`** in the same project directory).
4. On failure exit code, the task is marked **failed** and `bp run` stops.

### Environment variables

| Variable | Meaning |
|----------|---------|
| **`BP_RUN_SKIP_AGENT=1`** | **CI / integration default:** do not spawn an agent; auto-complete each task with a synthetic note (no subprocess). |
| **`BP_AGENT_BACKEND`** | Backend adapter when no explicit script is set: `cursor` (default) or `claude`. |
| **`BP_AGENT_MODEL`** | Cursor model id for `bp run` when `--model` is not passed (e.g. `composer-2.5`). |
| **`BP_RUN_AGENT_SHELL`** | Shell (default `sh`). `LOOP_RUN_AGENT_SHELL` still works. |
| **`BP_RUN_AGENT_SCRIPT`** | Full override script passed to `shell -c` (advanced). `LOOP_RUN_AGENT_SCRIPT` still works. |
| **`BP_COMPLETE_*`** | Optional metrics when completing (see `commands.rs`): `INPUT_TOKENS`, `OUTPUT_TOKENS`, `MODEL`, `COMMIT_SHA`. `LOOP_COMPLETE_*` still accepted. |

### Example: Cursor Agent

Authenticate once (`cursor agent login`). Then:

```bash
export BP_AGENT_BACKEND=cursor   # default, can omit
bp run --model composer-2.5      # pin model (or: export BP_AGENT_MODEL=composer-2.5)
```

`bp status` reports an **active** `bp run` (pid + task) while a queue is executing, or warns when a task is **stale running** after sleep/crash — run `bp reset <id>` and `bp run` again.

### Example: Claude Code

```bash
export BP_AGENT_BACKEND=claude
bp run
```

`bp` already has built-in backend adapters, so you usually do **not** need `BP_RUN_AGENT_SCRIPT`.
Use `BP_RUN_AGENT_SCRIPT` only if you want a custom launch command.

## Tests

From **`bp-rs/`** (crate package **`big-plan`**, targets include binary **`bp`**):

```bash
cd bp-rs
cargo test
```

From the **repository root** (same invocation CI uses):

```bash
cargo test --manifest-path bp-rs/Cargo.toml
```

Package-scoped variants use **`-p big-plan`** (helpful when a workspace grows; harmless in this single-package tree).

- **Unit tests:** `domain`, `cli`, `render`, `sqlite_repo`, `orchestrator` modules.
- **Integration tests:** `tests/cli_integration.rs` spawns the real **`bp`** binary in temp dirs. The main end-to-end path uses **`BP_RUN_SKIP_AGENT=1`** for deterministic full queue runs; other cases use **`BP_RUN_AGENT_SCRIPT`** to invoke `bp complete` without a real LLM.
- **CI:** `.github/workflows/ci.yml` runs **`cargo test --manifest-path bp-rs/Cargo.toml`** from the repo root.
- **Real agent (opt-in):** `tests/real_agent.rs` is **`#[ignore]`**. Run manually when authenticated and willing to spend tokens:

```bash
# from bp-rs/
BP_REAL_AGENT_BACKEND=cursor cargo test -p big-plan real_agent_smoke -- --ignored --nocapture

# same from repo root
BP_REAL_AGENT_BACKEND=cursor cargo test --manifest-path bp-rs/Cargo.toml -p big-plan real_agent_smoke -- --ignored --nocapture
# or: BP_REAL_AGENT_BACKEND=claude …
```

## Layout

- `bp-rs/` — Rust crate `big-plan`, binary **`bp`**
- `.loop/` — per-project state (when initialized): `loop.db`, `plan.md`, `agent-project.md`, etc.

## Next steps (this repo)

Use **`bp`** in this repository to track follow-ups (e.g. Python removal, publishing `big-plan` to crates.io). Run **`bp status`** after **`bp init`** if you use SQLite-backed tasks here.
