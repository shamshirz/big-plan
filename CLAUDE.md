# CLAUDE.md

## Repository Intent

This repository defines **big-plan**: a task-orchestration CLI invoked as **`bp`**, backed by Rust + SQLite with project-local isolation. The supported implementation lives in **`bp-rs/`** (see `README.md`).

## Agent Working Norms

- Favor focused, independent tasks over broad mixed-scope changes.
- Build only the context needed for the current task.
- Keep functional core logic pure; isolate side effects in shell adapters.
- Preserve user-visible behavior unless the task explicitly changes it.

## PoC Product Requirements

- `cargo install` friendly executable: Rust crate **`big-plan`** produces binary **`bp`** (`cargo install --path bp-rs` from this repo, or `cargo install big-plan` once published; see `README.md`).
- Project-local hidden state in the cwd where CLI is invoked.
- SQLite-backed task/event persistence within that hidden directory.
- Sequential task execution with clear status and completion metadata.

## Public CLI Surface (PoC release)

These commands are the supported user-facing API of the Rust PoC; keep outputs and errors aligned with `.loop/cli-contract.md` (update that doc to say `bp` when migrating off Python).

- `bp init`
- `bp add "<title>"`
- `bp status`
- `bp show <task-id>`
- `bp read plan|current|<task-id>`
- `bp run [--model <cursor-model-id>]`
- `bp complete [--notes "..."] [--if-running]`
- `bp reset <task-id>`

## Prompt Layering Contract

Each `bp run` task agent should receive:
1. universal guidance
2. project-specific context
3. task-specific context

This order is intentional and should be preserved.
