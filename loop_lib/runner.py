from __future__ import annotations

from datetime import datetime, timezone
import json
import os
from pathlib import Path
import subprocess
import sys
from typing import Any

from . import git
from .storage import (
    AGENT_PROJECT_FILE,
    PLAN_FILE,
    append_log,
    completion_notes_excerpt,
    next_pending_task,
    parse_task_file,
    set_current_symlink,
    TaskFile,
    utc_now_iso,
    write_task,
)


DEFAULT_UNIVERSAL_GUIDANCE = """You are working on exactly one task in a multi-task plan.

Execution contract:
- Stay focused on this task only.
- Make concrete code changes, then run the relevant validation commands.
- Keep edits minimal and safe; avoid unrelated refactors.
- Update the task's "Completion notes" with what changed and how you verified it.
- Mark the task complete when done.

Operational rules:
- Do not manage branches.
- Do not create commits; the orchestrator commits after you exit.
- Prefer deterministic validation steps over broad ad-hoc exploration.
"""


def _duration_seconds(started_at_iso: str) -> int:
    started = datetime.fromisoformat(started_at_iso.replace("Z", "+00:00"))
    now = datetime.now(timezone.utc)
    return int((now - started).total_seconds())


def _read_project_context() -> str:
    if AGENT_PROJECT_FILE.exists():
        return AGENT_PROJECT_FILE.read_text(encoding="utf-8").strip()
    return PLAN_FILE.read_text(encoding="utf-8").strip()


def _build_agent_prompt(task: TaskFile, loop_bin_path: str) -> str:
    task_text = task.path.read_text(encoding="utf-8").strip()
    project_context = _read_project_context()
    return f"""## Universal Guidance
{DEFAULT_UNIVERSAL_GUIDANCE.strip()}

## Project-Specific Context
{project_context}

## Task-Specific Context
{task_text}

## Required Commands
Use these commands (from repo root):
- `python3 "{loop_bin_path}" read current`
- `python3 "{loop_bin_path}" read plan`
- `python3 "{loop_bin_path}" read <id>`
- `python3 "{loop_bin_path}" complete --notes "<short summary>"`
"""


def _extract_result_usage(result_event: dict[str, Any] | None) -> tuple[Any, Any]:
    if not result_event:
        return None, None
    usage = result_event.get("usage")
    if isinstance(usage, dict):
        return usage.get("input_tokens"), usage.get("output_tokens")
    return None, None


def _select_backend() -> str:
    backend = os.environ.get("LOOP_AGENT_BACKEND", "claude").strip().lower()
    if backend not in {"claude", "cursor"}:
        raise RuntimeError(
            f"Unsupported LOOP_AGENT_BACKEND={backend!r}. Use 'claude' or 'cursor'."
        )
    return backend


def _spawn_claude_agent(task: TaskFile, loop_bin_path: str) -> tuple[int, dict[str, Any] | None]:
    env = os.environ.copy()
    env["LOOP_TASK_ID"] = str(task.frontmatter["id"])
    cmd = [
        "claude",
        "-p",
        _build_agent_prompt(task, loop_bin_path),
        "--verbose",
        "--output-format",
        "stream-json",
        "--dangerously-skip-permissions",
    ]
    return _spawn_streaming_agent(cmd=cmd, env=env)


def _spawn_cursor_agent(task: TaskFile, loop_bin_path: str) -> tuple[int, dict[str, Any] | None]:
    env = os.environ.copy()
    env["LOOP_TASK_ID"] = str(task.frontmatter["id"])
    cmd = [
        "cursor",
        "agent",
        "-p",
        _build_agent_prompt(task, loop_bin_path),
        "--output-format",
        "stream-json",
        "--stream-partial-output",
        "--force",
        "--trust",
    ]
    return _spawn_streaming_agent(cmd=cmd, env=env)


def _spawn_streaming_agent(cmd: list[str], env: dict[str, str]) -> tuple[int, dict[str, Any] | None]:
    proc = subprocess.Popen(
        cmd,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        env=env,
    )
    last_result: dict[str, Any] | None = None
    last_model: str | None = None
    assert proc.stdout is not None
    for line in proc.stdout:
        sys.stdout.write(line)
        sys.stdout.flush()
        try:
            parsed = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(parsed, dict):
            message = parsed.get("message")
            if isinstance(message, dict):
                model = message.get("model")
                if isinstance(model, str):
                    last_model = model
        if isinstance(parsed, dict) and parsed.get("type") == "result":
            last_result = parsed
    if last_result is not None and last_model and not last_result.get("model"):
        last_result["model"] = last_model
    code = proc.wait()
    return code, last_result


def _spawn_agent(task: TaskFile, loop_bin_path: str) -> tuple[int, dict[str, Any] | None]:
    backend = _select_backend()
    if backend == "cursor":
        return _spawn_cursor_agent(task, loop_bin_path)
    return _spawn_claude_agent(task, loop_bin_path)


def run_loop() -> int:
    loop_bin_path = str(Path(sys.argv[0]).resolve())
    backend = _select_backend()
    print(f"Using agent backend: {backend}")
    while True:
        task = next_pending_task()
        if task is None:
            print("No pending tasks. loop run is complete.")
            return 0

        task_id = str(task.frontmatter["id"])
        task.frontmatter["status"] = "running"
        task.frontmatter["started_at"] = utc_now_iso()
        write_task(task)
        set_current_symlink(task)

        exit_code, result_event = _spawn_agent(task, loop_bin_path)

        refreshed = parse_task_file(task.path)
        if refreshed.frontmatter.get("status") != "complete" or exit_code != 0:
            refreshed.frontmatter["status"] = "failed"
            write_task(refreshed)
            append_log(
                {
                    "event": "task_failed",
                    "task_id": task_id,
                    "timestamp": utc_now_iso(),
                    "agent_exit_code": exit_code,
                }
            )
            print(f"Task {task_id} failed; stopping loop.")
            return 1

        refreshed.frontmatter["duration_seconds"] = _duration_seconds(
            str(refreshed.frontmatter.get("started_at"))
        )

        input_tokens, output_tokens = _extract_result_usage(result_event)
        if input_tokens is not None:
            refreshed.frontmatter["input_tokens"] = input_tokens
        if output_tokens is not None:
            refreshed.frontmatter["output_tokens"] = output_tokens
        if result_event:
            refreshed.frontmatter["model"] = result_event.get("model")
        write_task(refreshed)

        sha: str | None = None
        try:
            if git.has_worktree_changes():
                git.stage_all()
                if git.has_staged_changes():
                    message = git.task_commit_subject(
                        task_id, str(refreshed.frontmatter.get("title", "task"))
                    )
                    body = completion_notes_excerpt(refreshed)
                    sha = git.commit(message, body if body else None)
        except git.GitOperationError as exc:
            refreshed.frontmatter["status"] = "failed"
            refreshed.frontmatter["commit_sha"] = None
            write_task(refreshed)
            append_log(
                {
                    "event": "git_operation_failed",
                    "task_id": task_id,
                    "timestamp": utc_now_iso(),
                    "error": str(exc),
                }
            )
            print(f"Task {task_id} failed during git operations: {exc}")
            return 1

        refreshed.frontmatter["commit_sha"] = sha
        write_task(refreshed)

        append_log(
            {
                "event": "task_complete",
                "task_id": task_id,
                "timestamp": utc_now_iso(),
                "commit_sha": sha,
                "input_tokens": refreshed.frontmatter.get("input_tokens"),
                "output_tokens": refreshed.frontmatter.get("output_tokens"),
                "duration_seconds": refreshed.frontmatter.get("duration_seconds"),
            }
        )
