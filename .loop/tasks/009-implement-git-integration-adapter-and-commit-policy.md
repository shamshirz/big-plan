---
id: 009
title: Implement git integration adapter and commit policy
status: complete
depends_on: []
created_at: 2026-05-07T18:32:47Z
started_at: 2026-05-08T00:25:39Z
completed_at: 2026-05-08T00:27:26Z
duration_seconds: 112
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
- [x] Staging/commit operations are applied only when relevant changes exist.
- [x] Commit message/body policy is documented and implemented.
- [x] Commit SHA is persisted back to task records.
- [x] Failure modes (no changes, commit failures) are handled predictably.

---

## Completion notes
Extended `loop_lib/git.py` as the imperative git adapter: module docstring defines commit subject/body policy; `has_worktree_changes()` gates `git add -A`; commit runs only when the index is non-empty after staging; `task_commit_subject()` normalizes titles to a single line; `commit()` maps subprocess failures to `GitOperationError`. Runner stages/commits in that order, always persists `commit_sha` (or null when no commit), and on git failure marks the task `failed`, logs `git_operation_failed`, prints a clear message, and exits the loop with code 1.

Verified: `python3 -m compileall -q loop_lib`; `LOOP_AGENT_BACKEND=claude python3 tests/integration_loop_run.py` (integration harness requires explicit backend when the environment defaults to cursor).

Also verified: LOOP_AGENT_BACKEND=claude for tests/integration_loop_run.py when env defaults to cursor.
