# AGENT.md — modifying the big-plan repo

This file is for agents working **in this repository** to evolve the `bp` tool itself. For using `bp` in other projects, see **`SKILL.md`** (also copied to `.loop/SKILL.md` on `bp init`).

`CLAUDE.md` is a symlink to this file (Cursor/Claude Code reads that path by convention).

## Repository intent

- Rust crate in `bp-rs/` produces binary **`bp`** (crate name `big-plan`).
- Tasks and goals live in SQLite under `.loop/` — no per-task markdown files.
- Sequential execution with clear status and completion metadata.

## Prompt layering

Each `bp run` task agent receives, in order:

1. Universal guidance (abbreviated `bp` usage + `.loop/SKILL.md` path)
2. Project-specific context (`.loop/agent-project.md`)
3. Task-specific context (SQLite task rendered as markdown)

Planning tasks (`bp run plan.md`) use plan-decomposition guidance instead of normal task markdown.

## How to work here

1. Run `bp init` once in this repo root (if not already).
2. Start a goal: `bp run simplification-plan.md` or `bp goal new` + `bp add`.
3. Execute: `bp run [--model <id>]` (Cursor default).
4. Inspect: `bp status`, `bp show <id>`, `bp summary`.

For deterministic tests without spawning an agent: `BP_RUN_SKIP_AGENT=1 bp run`.

## Task decomposition (when editing this codebase)

- One major concern per task.
- Split domain logic from SQLite adapter from CLI from orchestrator.
- Keep functional core pure; shell/agent I/O at the edges.
- Preserve user-visible CLI behavior unless the task explicitly changes it.

## Completion standard

- Minimal, focused diffs.
- `cargo test --manifest-path bp-rs/Cargo.toml` passes.
- Document non-obvious behavior in README or code comments sparingly.
- Mark done: `bp complete --notes "..."` with what changed and how you verified.

## Public CLI surface

```
bp init
bp goal new | list
bp run [plan.md] [--model <id>] [--backend cursor|claude]
bp add "<title>"
bp status | show <id> | read plan|current|<id>
bp complete [--notes "..."] [--if-running]
bp reset <id>
bp summary [--json]
```

User-facing config is **flags only** on `bp run`. CI uses `BP_RUN_SKIP_AGENT=1` internally.
