# big-plan

**big-plan** is a project-local CLI (**`bp`**) for orchestrating task-focused agent sessions. State lives in SQLite under `.loop/` (runtime only — gitignored, like [Simon Willison's `llm` CLI](https://github.com/simonw/llm) history DB).

> **Disclaimer:** The Rust in `bp-rs/` is **entirely agent-written** and **not human-reviewed**. Treat it as experimental — read the code, run tests, and use at your own risk.

- **Crate name (Cargo):** `big-plan`
- **Binary name:** `bp`

## Install

Rust 1.74+ (edition 2021).

```bash
# From this repo
cargo install --path bp-rs --force
```

Ensure `~/.cargo/bin` is on your `PATH`. Clone this repo on each machine and reinstall — no crates.io publish required.

## Concepts

| Term | Meaning |
|------|---------|
| **Goal** | A bounded run sequence: one plan decomposed into tasks, then executed one-by-one. |
| **Task** | A single unit of work in SQLite — each gets a fresh agent context window. |
| **`.loop/`** | Runtime directory (`loop.db`, `SKILL.md`, optional `agent-project.md`). Not committed. |

## Quick start

```bash
cd your-project
bp init
bp run docs/my-plan.md          # new goal + planning agent creates tasks
bp run                          # execute pending tasks in the active goal
bp summary                      # report when done
```

Start a fresh goal without a plan file:

```bash
bp goal new
bp add "First task"
bp run
```

## Commands

| Command | Purpose |
|--------|---------|
| `bp init` | Create `.loop/`, SQLite schema, seed `SKILL.md` |
| `bp goal new` | Start a new active goal (archives the previous one) |
| `bp goal list` | List goals (`*` = active) |
| `bp run [plan.md] [--model <id>] [--backend cursor\|claude]` | Run active goal; optional plan file starts a new goal first |
| `bp add "<title>"` | Add a pending task to the active goal |
| `bp status` | Live dashboard: progress bar, digest, and task table with time/commits |
| `bp show <id>` | Human-readable task detail |
| `bp read plan\|current\|<id>` | Markdown for agents |
| `bp complete [--notes "..."] [--if-running]` | Mark the running task complete |
| `bp reset <id>` | Return a task to pending |
| `bp summary [--json]` | Post-run report (wall clock, tokens, commits) |

## `bp status` example

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

Unicode markers: `✓` complete, `▶` running, `·` pending, `✗` failed. Use `bp summary` for token totals and `--last-run` / `--since` filters.

## Agent workflow

1. **Planning** — `bp run plan.md` runs one planning task. The agent reads the plan and creates tasks with `bp add`, then `bp complete`.
2. **Execution** — `bp run` runs each pending task sequentially. Agents use `bp read current`, implement scope, `bp complete`.

Every agent prompt includes a short `bp` usage guide plus a pointer to `.loop/SKILL.md`. Optional project notes: edit `.loop/agent-project.md`.

Default backend is Cursor (`cursor agent …`). Use `--backend claude` for Claude Code.

## Tests

```bash
cargo test --manifest-path bp-rs/Cargo.toml
```

Integration tests use `BP_RUN_SKIP_AGENT=1` for deterministic runs. Opt-in real-agent smoke: see `bp-rs/tests/real_agent.rs`.

## Repo layout

- `bp-rs/` — Rust crate `big-plan`, binary `bp`
- `SKILL.md` — canonical skill doc (copied to `.loop/SKILL.md` on init)
- `AGENT.md` — guidance for agents modifying **this** repo
