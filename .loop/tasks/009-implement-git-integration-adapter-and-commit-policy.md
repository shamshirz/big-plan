---
id: 009
title: Implement git integration adapter and commit policy
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

# Task 009: Implement git integration adapter and commit policy

## Description
Implement git-side effects as an imperative adapter, including staging, commit creation, and metadata capture policy.

## Context
- Focus on git integration boundaries only.
- Inputs: orchestrator flow from task 008 and domain/repository APIs.
- Avoid changing CLI command contracts in this task.

## Acceptance criteria
- [ ] Staging/commit operations are applied only when relevant changes exist.
- [ ] Commit message/body policy is documented and implemented.
- [ ] Commit SHA is persisted back to task records.
- [ ] Failure modes (no changes, commit failures) are handled predictably.

---

## Completion notes
[Fill this section before running `loop complete`.]
