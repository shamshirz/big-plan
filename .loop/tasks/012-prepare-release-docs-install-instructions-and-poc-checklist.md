---
id: 012
title: Prepare release docs, install instructions, and PoC checklist
status: complete
depends_on: []
created_at: 2026-05-07T18:32:47Z
started_at: 2026-05-08T00:29:19Z
completed_at: 2026-05-08T00:30:03Z
duration_seconds: 55
input_tokens: null
output_tokens: null
model: null
commit_sha: null
---

# Task 012: Prepare release docs, install instructions, and PoC checklist

## Description
Prepare PoC release documentation and readiness checklist for publishing/installing the Rust CLI.

## Context
- Focus on documentation and release readiness.
- Avoid large implementation changes in this task.
- Inputs: implemented CLI behavior, test commands, and architecture decisions.

## Acceptance criteria
- [x] README documents install (`cargo install`) and core usage workflow.
- [x] Public CLI command reference is concise and accurate.
- [x] PoC readiness checklist includes validation, known limitations, and next steps.
- [x] Agent/project context docs reflect the released workflow.

---

## Completion notes
Updated `README.md` with install paths (`cargo install --path loop-rs`, publish name `loop-cli`), accurate command table (including `read` variants and `complete` semantics), “How it works” split between target vs current PoC (`loop run` subprocess not implemented), and a PoC release checklist (validation, limitations, next steps). Synced `.loop/plan.md` (pointer to README checklist), `CLAUDE.md` (crate/binary naming + PoC CLI surface wording), and `AGENT.md` (released workflow for bootstrap → read → complete/reset). Verified with `cargo test` in `loop-rs/`.
