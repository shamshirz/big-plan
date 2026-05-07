from __future__ import annotations

from datetime import datetime, timezone
import json
import os
from pathlib import Path
import subprocess
import sys

from . import git
from .storage import (
    append_log,
    completion_notes_excerpt,
    next_pending_task,
    parse_task_file,
    set_current_symlink,
    utc_now_iso,
    write_task,
)


AGENT_PROMPT = """You are working on a single task in a multi-task plan.

1. Read your current task with: `loop read current`.
2. Read the overall plan with: `loop read plan`.
3. If your task references other task IDs, read them with: `loop read <id>`.
4. Do the work. Use your normal tools to read code, edit files, run tests.
5. Before finishing, edit the "Completion notes" section of your task file.
6. When done, run `loop complete`. Then exit.

You do not need to commit. The orchestrator commits after you exit.
You do not need to manage branches.
Do not inspect the .loop storage directory directly; use `loop read`.
"""


def _duration_seconds(started_at_iso: str) -> int:
    started = datetime.fromisoformat(started_at_iso.replace("Z", "+00:00"))
    now = datetime.now(timezone.utc)
    return int((now - started).total_seconds())


def _spawn_agent(task_id: str) -> tuple[int, dict | None]:
    env = os.environ.copy()
    env["LOOP_TASK_ID"] = task_id
    cmd = [
        "claude",
        "-p",
        AGENT_PROMPT,
        "--output-format",
        "stream-json",
        "--dangerously-skip-permissions",
    ]
    proc = subprocess.Popen(
        cmd,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        env=env,
    )
    last_result = None
    assert proc.stdout is not None
    for line in proc.stdout:
        sys.stdout.write(line)
        sys.stdout.flush()
        try:
            parsed = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(parsed, dict) and parsed.get("type") == "result":
            last_result = parsed
    code = proc.wait()
    return code, last_result


def run_loop() -> int:
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

        exit_code, result_event = _spawn_agent(task_id)

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

        if result_event and isinstance(result_event.get("usage"), dict):
            usage = result_event["usage"]
            refreshed.frontmatter["input_tokens"] = usage.get("input_tokens")
            refreshed.frontmatter["output_tokens"] = usage.get("output_tokens")
        if result_event:
            refreshed.frontmatter["model"] = result_event.get("model")
        write_task(refreshed)

        git.stage_all()
        sha = None
        if git.has_staged_changes():
            title = str(refreshed.frontmatter.get("title", "task"))
            message = f"task {task_id}: {title}"
            body = completion_notes_excerpt(refreshed)
            sha = git.commit(message, body if body else None)
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
