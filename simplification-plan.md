# big-plan simplification goal

Finish the Rust-only PoC: goals, plan-driven runs, clear docs, minimal CLI contract.

## Phase 1 — Python removal cleanup

- Remove stale Python references from `.gitignore`, README, SKILL.md, and docs.
- Archive or remove `loop-plan.md` (historical markdown-on-disk spec; superseded by SQLite).
- Align `CLAUDE.md` public command list with shipped CLI (include `summary`, `goal`, `run plan.md`).

## Phase 2 — Goals and plan-driven runs

- SQLite `goals` table; tasks scoped to active goal.
- `bp run plan.md` — new goal, planning agent decomposes plan into tasks via `bp add`.
- `bp goal new` / `bp goal list` — start fresh goal without a plan file.
- `bp read plan` reads active goal plan from SQLite (not flat files).
- Copy `SKILL.md` to `.loop/` on init; reference in universal agent prompt.

## Phase 3 — CLI contract simplification

- One user-facing configuration path: CLI flags on `bp run` (`--model`, `--backend cursor|claude`).
- Remove `LOOP_*` legacy env aliases.
- Keep `BP_RUN_SKIP_AGENT` and `BP_RUN_AGENT_SCRIPT` as undocumented CI/test hooks only.
- Fix help text (remove broken per-command `-h` promise or implement it).

## Phase 4 — Documentation

- **SKILL.md** — using `bp` in any project (also copied to `.loop/SKILL.md`).
- **AGENT.md** — using an agent to modify the big-plan repo itself.
- **README.md** — install (`cargo install --path bp-rs`), goals workflow, `.loop/` gitignored runtime.
- Document how to start a new goal vs continue the active one.

## Phase 5 — Verification

- `cargo test --manifest-path bp-rs/Cargo.toml` passes.
- Dogfood: `bp run simplification-plan.md` then `bp run` on this repo.
