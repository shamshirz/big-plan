from __future__ import annotations

from datetime import datetime, timezone
import subprocess


def _run(args: list[str], check: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(["git", *args], text=True, capture_output=True, check=check)


def create_loop_branch() -> str:
    timestamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    name = f"loop/{timestamp}"
    _run(["checkout", "-b", name])
    return name


def stage_all() -> None:
    _run(["add", "-A"])


def has_staged_changes() -> bool:
    proc = _run(["diff", "--cached", "--quiet"], check=False)
    return proc.returncode != 0


def commit(message: str, body: str | None = None) -> str:
    args = ["commit", "-m", message]
    if body:
        args.extend(["-m", body])
    _run(args)
    return _run(["rev-parse", "HEAD"]).stdout.strip()
