from __future__ import annotations

import argparse
import os
import sys

from . import git
from .runner import run_loop
from .storage import (
    PLAN_FILE,
    append_completion_notes,
    create_task,
    ensure_loop_absent,
    ensure_loop_exists,
    find_task_by_id,
    init_layout,
    list_tasks,
    read_current_task,
    utc_now_iso,
    write_task,
)


def cmd_init(_: argparse.Namespace) -> int:
    ensure_loop_absent()
    init_layout()
    branch = git.create_loop_branch()
    print(f"Initialized .loop and created branch: {branch}")
    return 0


def cmd_add(args: argparse.Namespace) -> int:
    ensure_loop_exists()
    path = create_task(args.title)
    print(path.as_posix())
    return 0


def cmd_status(_: argparse.Namespace) -> int:
    ensure_loop_exists()
    tasks = list_tasks()
    if not tasks:
        print("No tasks found.")
        return 0
    for task in tasks:
        status = task.frontmatter.get("status", "unknown")
        task_id = task.frontmatter.get("id", "???")
        title = task.frontmatter.get("title", "")
        print(f"{task_id} [{status}] {title}")
    return 0


def cmd_complete(args: argparse.Namespace) -> int:
    ensure_loop_exists()
    task_id = os.environ.get("LOOP_TASK_ID")
    if not task_id:
        print("LOOP_TASK_ID is not set; run this from `loop run` agent session.", file=sys.stderr)
        return 1
    task = find_task_by_id(task_id)
    task.frontmatter["status"] = "complete"
    task.frontmatter["completed_at"] = utc_now_iso()
    if args.notes:
        append_completion_notes(task, args.notes)
    write_task(task)
    print(f"Task {task_id} marked complete.")
    return 0


def cmd_read(args: argparse.Namespace) -> int:
    ensure_loop_exists()
    target = args.target.strip().lower()
    if target == "plan":
        print(PLAN_FILE.read_text(encoding="utf-8"), end="")
        return 0
    if target == "current":
        task = read_current_task()
        print(task.path.read_text(encoding="utf-8"), end="")
        return 0
    task = find_task_by_id(target)
    print(task.path.read_text(encoding="utf-8"), end="")
    return 0


def cmd_show(args: argparse.Namespace) -> int:
    ensure_loop_exists()
    task = find_task_by_id(args.task_id)
    print(task.path.read_text(encoding="utf-8"), end="")
    return 0


def cmd_reset(args: argparse.Namespace) -> int:
    ensure_loop_exists()
    task = find_task_by_id(args.task_id)
    task.frontmatter["status"] = "pending"
    task.frontmatter["started_at"] = None
    task.frontmatter["completed_at"] = None
    task.frontmatter["duration_seconds"] = None
    task.frontmatter["input_tokens"] = None
    task.frontmatter["output_tokens"] = None
    task.frontmatter["model"] = None
    task.frontmatter["commit_sha"] = None
    write_task(task)
    print(f"Task {task.frontmatter.get('id')} reset to pending.")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="loop", description="Simple task loop CLI.")
    sub = parser.add_subparsers(dest="command", required=True)

    p_init = sub.add_parser("init", help="Initialize loop storage and branch")
    p_init.set_defaults(func=cmd_init)

    p_add = sub.add_parser("add", help="Create a new task")
    p_add.add_argument("title")
    p_add.set_defaults(func=cmd_add)

    p_run = sub.add_parser("run", help="Run pending tasks in sequence")
    p_run.set_defaults(func=lambda _: run_loop())

    p_complete = sub.add_parser("complete", help="Mark current task complete")
    p_complete.add_argument("--notes")
    p_complete.set_defaults(func=cmd_complete)

    p_status = sub.add_parser("status", help="List tasks and status")
    p_status.set_defaults(func=cmd_status)

    p_show = sub.add_parser("show", help="Print a task by ID")
    p_show.add_argument("task_id")
    p_show.set_defaults(func=cmd_show)

    p_reset = sub.add_parser("reset", help="Reset a task to pending")
    p_reset.add_argument("task_id")
    p_reset.set_defaults(func=cmd_reset)

    p_read = sub.add_parser("read", help="Read plan/current/task content")
    p_read.add_argument("target", help="plan | current | <task-id>")
    p_read.set_defaults(func=cmd_read)

    return parser


def main(argv: list[str] | None = None) -> int:
    if argv is None:
        argv = sys.argv[1:]
    if not argv:
        argv = ["-h"]

    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        return int(args.func(args))
    except RuntimeError as exc:
        print(str(exc), file=sys.stderr)
        return 1
