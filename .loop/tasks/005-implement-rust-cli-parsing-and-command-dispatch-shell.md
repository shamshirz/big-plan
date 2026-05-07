---
id: 005
title: Implement Rust CLI parsing and command dispatch shell
status: complete
depends_on: []
created_at: 2026-05-07T18:32:47Z
started_at: 2026-05-07T18:42:38Z
completed_at: 2026-05-07T18:48:48Z
duration_seconds: 376
input_tokens: 16
output_tokens: 27306
model: null
commit_sha: 38b477832a642e2d3a8987893d0e427aaf62a5fc
---

# Task 005: Implement Rust CLI parsing and command dispatch shell

## Description
Implement Rust command parsing and top-level command dispatch according to the finalized CLI contract.

## Context
- Focus on imperative shell entrypoints and argument handling.
- Consume domain/storage interfaces; do not implement DB internals here.
- Inputs: task 002 output and project context docs.

## Acceptance criteria
- [ ] CLI commands and options parse correctly.
- [ ] `loop` and `loop -h` produce concise equivalent help output.
- [ ] Command dispatch delegates to domain/repository interfaces cleanly.
- [ ] Error exits/messages align with the CLI contract.

---

## Completion notes
[Fill this section before running `loop complete`.]

Created loop-rs/src/cli.rs (manual arg parsing → Command enum, 22 tests), loop-rs/src/repository.rs (TaskRepository trait + LoopError), loop-rs/src/commands.rs (all 8 commands dispatch against the trait), and updated loop-rs/src/main.rs with dispatch wiring and StubRepository placeholder. All 48 tests pass (cargo test). Binary behavior verified: loop/-h/--help produce identical help at exit 0; unknown commands, missing args, and flag errors emit correct error messages to stderr at exit 1 per CLI contract. run command stubs subprocess invocation pending the orchestrator task.
