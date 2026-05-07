---
id: 002
title: Finalize public CLI API and user-facing behavior contract
status: complete
depends_on: []
created_at: 2026-05-07T18:32:46Z
started_at: 2026-05-07T18:35:52Z
completed_at: 2026-05-07T18:37:03Z
duration_seconds: 76
input_tokens: 8
output_tokens: 4544
model: null
commit_sha: 030286f972d63a95407df83adc0d249cc5d19319
---

# Task 002: Finalize public CLI API and user-facing behavior contract

## Description
Define the user-facing CLI contract for the Rust PoC so implementation tasks can execute without re-litigating command behavior.

## Context
- Focus only on CLI behavior and user experience.
- Avoid SQLite schema and low-level Rust ownership concerns in this task.
- Use `.loop/plan.md`, `README.md`, and `.loop/agent-project.md` as source context.

## Acceptance criteria
- [ ] Produce an explicit command contract for `init`, `add`, `status`, `show`, `read`, `run`, `complete`, and `reset`.
- [ ] Specify expected outputs and error messages for normal and edge cases.
- [ ] Define help behavior (`loop` and `loop -h` equivalence).
- [ ] Capture command semantics in a form directly usable by implementation tasks.

---

## Completion notes
[Fill this section before running `loop complete`.]

Produced .loop/cli-contract.md — full command contract for all 8 commands (init, add, status, show, read, run, complete, reset) covering normal outputs, edge-case error messages, exit codes, output rules, and task state invariants. Updated README.md to reflect the Rust PoC target CLI surface with a command reference table and links to new contract doc.
