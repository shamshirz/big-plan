---
id: 005
title: Implement Rust CLI parsing and command dispatch shell
status: pending
depends_on: []
created_at: 2026-05-07T18:32:47Z
started_at: null
completed_at: null
duration_seconds: null
input_tokens: null
output_tokens: null
model: null
commit_sha: null
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
