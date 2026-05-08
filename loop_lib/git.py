"""Imperative git adapter for the loop orchestrator.

Commit policy (after a successful task, when the worktree has changes):

- **Stage** only if `git status --porcelain` is non-empty (tracked or untracked
  files that are not ignored). This skips a no-op `git add` when there is nothing
  relevant to commit.
- **Commit** only if the index is non-empty after staging (`git diff --cached`).
- **Subject line** (`git commit -m`): ``task {id}: {title}`` with the title
  normalized to a single line (whitespace collapsed, newlines removed).
- **Body** (optional second ``-m``): first 500 characters of the task markdown
  body under ``## Completion notes`` — see `loop_lib.storage.completion_notes_excerpt`.
  If that excerpt is empty, the commit has only the subject line.
- **Recorded metadata**: on success, `git rev-parse HEAD` is stored in the task
  frontmatter as `commit_sha`. If there was nothing to commit, `commit_sha` stays
  null.

All subprocess failures from mutating git commands are raised as
`GitOperationError` with the command's stderr when available.
"""

from __future__ import annotations

from datetime import datetime, timezone
import subprocess


class GitOperationError(RuntimeError):
    """Git command exited non-zero (typically commit or add)."""


def _run(args: list[str], check: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(["git", *args], text=True, capture_output=True, check=check)


def create_loop_branch() -> str:
    timestamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    name = f"loop/{timestamp}"
    _run(["checkout", "-b", name])
    return name


def stage_all() -> None:
    try:
        _run(["add", "-A"])
    except subprocess.CalledProcessError as e:
        raise GitOperationError(
            (e.stderr or e.stdout or "").strip() or "git add failed"
        ) from e


def has_worktree_changes() -> bool:
    """True if there is anything `git add -A` could stage (non-ignored paths)."""
    proc = _run(["status", "--porcelain"], check=True)
    return bool(proc.stdout.strip())


def has_staged_changes() -> bool:
    proc = _run(["diff", "--cached", "--quiet"], check=False)
    return proc.returncode != 0


def task_commit_subject(task_id: str, title: str) -> str:
    clean = " ".join((title or "task").replace("\r", " ").split())
    return f"task {task_id}: {clean}"


def commit(message: str, body: str | None = None) -> str:
    args = ["commit", "-m", message]
    if body:
        args.extend(["-m", body])
    try:
        _run(args)
    except subprocess.CalledProcessError as e:
        detail = (e.stderr or e.stdout or "").strip() or "git commit failed"
        raise GitOperationError(detail) from e
    try:
        return _run(["rev-parse", "HEAD"]).stdout.strip()
    except subprocess.CalledProcessError as e:
        detail = (e.stderr or e.stdout or "").strip() or "git rev-parse failed"
        raise GitOperationError(detail) from e
