from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
import json
import re
from typing import Any


LOOP_DIR = Path(".loop")
TASKS_DIR = LOOP_DIR / "tasks"
PLAN_FILE = LOOP_DIR / "plan.md"
AGENT_PROJECT_FILE = LOOP_DIR / "agent-project.md"
CURRENT_FILE = LOOP_DIR / "CURRENT"
LOG_FILE = LOOP_DIR / "log.jsonl"

TASK_HEADER_ORDER = [
    "id",
    "title",
    "status",
    "depends_on",
    "created_at",
    "started_at",
    "completed_at",
    "duration_seconds",
    "input_tokens",
    "output_tokens",
    "model",
    "commit_sha",
]


@dataclass
class TaskFile:
    path: Path
    frontmatter: dict[str, Any]
    body: str


def utc_now_iso() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def ensure_loop_absent() -> None:
    if LOOP_DIR.exists():
        raise RuntimeError(".loop already exists; refusing to clobber")


def ensure_loop_exists() -> None:
    if not LOOP_DIR.exists():
        raise RuntimeError(".loop does not exist; run `loop init` first")


def init_layout() -> None:
    LOOP_DIR.mkdir(parents=True, exist_ok=False)
    TASKS_DIR.mkdir(parents=True, exist_ok=False)
    PLAN_FILE.write_text(
        "# Plan\n\n"
        "Describe the overall objective and strategy here.\n\n"
        "## Milestones\n\n"
        "- [ ] Add tasks with `loop add \"...\"`\n",
        encoding="utf-8",
    )
    AGENT_PROJECT_FILE.write_text(
        "# Project Context For Loop Agents\n\n"
        "Add stable, repo-specific guidance every task agent should see.\n\n"
        "Examples:\n"
        "- architecture constraints\n"
        "- coding standards\n"
        "- required test commands\n"
        "- deployment or safety rules\n",
        encoding="utf-8",
    )
    LOG_FILE.touch()


def parse_scalar(raw: str, *, key: str | None = None) -> Any:
    val = raw.strip()
    if val == "null":
        return None
    if val in ("[]", ""):
        return []
    if val in ("true", "false"):
        return val == "true"
    numeric_fields = {"duration_seconds", "input_tokens", "output_tokens"}
    if key in numeric_fields and re.fullmatch(r"-?\d+", val):
        return int(val)
    if val.startswith("[") and val.endswith("]"):
        inner = val[1:-1].strip()
        if not inner:
            return []
        return [item.strip().strip("\"'") for item in inner.split(",")]
    return val


def to_scalar(value: Any) -> str:
    if value is None:
        return "null"
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, list):
        if not value:
            return "[]"
        quoted = ", ".join(json.dumps(str(item)) for item in value)
        return f"[{quoted}]"
    return str(value)


def parse_task_file(path: Path) -> TaskFile:
    raw = path.read_text(encoding="utf-8")
    if not raw.startswith("---\n"):
        raise RuntimeError(f"Task file missing frontmatter: {path}")
    parts = raw.split("\n---\n", 1)
    header_block = parts[0][4:]
    body = parts[1] if len(parts) > 1 else ""
    frontmatter: dict[str, Any] = {}
    for line in header_block.splitlines():
        if not line.strip():
            continue
        if ":" not in line:
            continue
        key, value = line.split(":", 1)
        parsed_key = key.strip()
        frontmatter[parsed_key] = parse_scalar(value, key=parsed_key)
    return TaskFile(path=path, frontmatter=frontmatter, body=body)


def write_task(task: TaskFile) -> None:
    lines = ["---"]
    for key in TASK_HEADER_ORDER:
        if key in task.frontmatter:
            lines.append(f"{key}: {to_scalar(task.frontmatter[key])}")
    extra_keys = [k for k in task.frontmatter.keys() if k not in TASK_HEADER_ORDER]
    for key in sorted(extra_keys):
        lines.append(f"{key}: {to_scalar(task.frontmatter[key])}")
    lines.append("---")
    content = "\n".join(lines) + "\n" + task.body
    task.path.write_text(content, encoding="utf-8")


def next_task_id() -> str:
    ensure_loop_exists()
    max_id = 0
    for path in TASKS_DIR.glob("*.md"):
        name = path.name.split("-", 1)[0]
        if name.isdigit():
            max_id = max(max_id, int(name))
    return f"{max_id + 1:03d}"


def slugify(text: str) -> str:
    slug = re.sub(r"[^a-z0-9]+", "-", text.lower()).strip("-")
    return slug or "task"


def build_task_template(task_id: str, title: str) -> str:
    header_lines = [
        "---",
        f"id: {task_id}",
        f"title: {title}",
        "status: pending",
        "depends_on: []",
        f"created_at: {utc_now_iso()}",
        "started_at: null",
        "completed_at: null",
        "duration_seconds: null",
        "input_tokens: null",
        "output_tokens: null",
        "model: null",
        "commit_sha: null",
        "---",
        "",
        f"# Task {task_id}: {title}",
        "",
        "## Description",
        "[Describe what this task should accomplish.]",
        "",
        "## Context",
        "[Optional references, constraints, and related files.]",
        "",
        "## Acceptance criteria",
        "- [ ] Define success criteria",
        "",
        "---",
        "",
        "## Completion notes",
        "[Fill this section before running `loop complete`.]",
        "",
    ]
    return "\n".join(header_lines)


def create_task(title: str) -> Path:
    task_id = next_task_id()
    slug = slugify(title)
    path = TASKS_DIR / f"{task_id}-{slug}.md"
    path.write_text(build_task_template(task_id, title), encoding="utf-8")
    return path


def task_paths() -> list[Path]:
    ensure_loop_exists()
    return sorted(TASKS_DIR.glob("*.md"))


def list_tasks() -> list[TaskFile]:
    return [parse_task_file(path) for path in task_paths()]


def find_task_by_id(task_id: str) -> TaskFile:
    normalized = f"{int(task_id):03d}" if task_id.isdigit() else task_id
    matches = list(TASKS_DIR.glob(f"{normalized}-*.md"))
    if not matches:
        raise RuntimeError(f"Task {task_id} not found")
    return parse_task_file(matches[0])


def next_pending_task() -> TaskFile | None:
    for task in list_tasks():
        if task.frontmatter.get("status") == "pending":
            return task
    return None


def set_current_symlink(task: TaskFile) -> None:
    target = task.path
    if CURRENT_FILE.exists() or CURRENT_FILE.is_symlink():
        CURRENT_FILE.unlink()
    CURRENT_FILE.symlink_to(target.relative_to(LOOP_DIR))


def read_current_task() -> TaskFile:
    if not CURRENT_FILE.exists() and not CURRENT_FILE.is_symlink():
        raise RuntimeError(".loop/CURRENT does not exist")
    resolved = CURRENT_FILE.resolve()
    return parse_task_file(resolved)


def append_completion_notes(task: TaskFile, notes: str) -> None:
    marker = "## Completion notes"
    if marker not in task.body:
        task.body += f"\n{marker}\n"
    task.body = task.body.rstrip() + "\n\n" + notes.strip() + "\n"


def completion_notes_excerpt(task: TaskFile, max_chars: int = 500) -> str:
    marker = "## Completion notes"
    if marker not in task.body:
        return ""
    notes = task.body.split(marker, 1)[1].strip()
    return notes[:max_chars]


def append_log(event: dict[str, Any]) -> None:
    LOG_FILE.parent.mkdir(parents=True, exist_ok=True)
    with LOG_FILE.open("a", encoding="utf-8") as handle:
        handle.write(json.dumps(event, sort_keys=True) + "\n")
