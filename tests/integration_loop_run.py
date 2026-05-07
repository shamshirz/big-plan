#!/usr/bin/env python3
"""Integration test for `loop run` using a mock claude binary.

This test validates that:
1) three agent sessions are invoked,
2) each session mutates repository files,
3) the loop marks all tasks complete,
4) commits are created by the orchestrator,
5) final output summarizes the result.
"""

from __future__ import annotations

from dataclasses import dataclass
import argparse
from pathlib import Path
import json
import os
import shutil
import subprocess
import tempfile


REPO_ROOT = Path(__file__).resolve().parents[1]
LOOP_BIN = REPO_ROOT / "loop"
LOOP_LIB = REPO_ROOT / "loop_lib"


def run(
    args: list[str],
    cwd: Path,
    env: dict[str, str] | None = None,
    check: bool = True,
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(args, cwd=cwd, env=env, text=True, capture_output=True, check=check)


def git(args: list[str], cwd: Path, check: bool = True) -> subprocess.CompletedProcess[str]:
    return run(["git", *args], cwd=cwd, check=check)


def parse_frontmatter(path: Path) -> dict[str, str]:
    raw = path.read_text(encoding="utf-8")
    if not raw.startswith("---\n"):
        raise AssertionError(f"Missing frontmatter in {path}")
    header = raw.split("\n---\n", 1)[0][4:]
    data: dict[str, str] = {}
    for line in header.splitlines():
        if ":" not in line:
            continue
        key, value = line.split(":", 1)
        data[key.strip()] = value.strip()
    return data


def write_mock_claude(bin_dir: Path, loop_path: Path) -> Path:
    claude_path = bin_dir / "claude"
    script = f"""#!/usr/bin/env python3
import json
import os
import subprocess
from pathlib import Path

task_id = os.environ["LOOP_TASK_ID"]
repo = Path.cwd()
repo.joinpath("AGENT_RUNS.log").open("a", encoding="utf-8").write(task_id + "\\n")

target = repo / "agent_changes.txt"
with target.open("a", encoding="utf-8") as handle:
    handle.write(f"task {{task_id}} changed files\\n")

subprocess.run(
    ["python3", "{loop_path}", "complete", "--notes", f"Completed by mock claude for task {{task_id}}."],
    check=True,
)

print(json.dumps({{"type": "event", "message": f"task {{task_id}} running"}}))
print(json.dumps({{
    "type": "result",
    "model": "mock-claude",
    "usage": {{"input_tokens": 10, "output_tokens": 20}},
    "duration_ms": 50
}}))
"""
    claude_path.write_text(script, encoding="utf-8")
    claude_path.chmod(0o755)
    return claude_path


@dataclass
class FinalResult:
    workdir: Path
    agent_runs: int
    complete_tasks: int
    commit_count: int
    changed_files_last_three_commits: list[str]
    status_stdout: str
    session_summaries: list[str]


def run_integration() -> FinalResult:
    with tempfile.TemporaryDirectory(prefix="loop-int-") as tmp:
        workdir = Path(tmp)
        git(["init"], cwd=workdir)
        git(["config", "user.name", "Loop Test"], cwd=workdir)
        git(["config", "user.email", "loop-test@example.com"], cwd=workdir)
        (workdir / "README.md").write_text("integration harness\n", encoding="utf-8")
        git(["add", "-A"], cwd=workdir)
        git(["commit", "-m", "init"], cwd=workdir)

        shutil.copy2(LOOP_BIN, workdir / "loop")
        shutil.copytree(LOOP_LIB, workdir / "loop_lib")

        # Mock claude first on PATH so we can prove agents mutate files.
        bin_dir = workdir / ".bin"
        bin_dir.mkdir(parents=True, exist_ok=True)
        write_mock_claude(bin_dir, workdir / "loop")
        env = os.environ.copy()
        env["PATH"] = f"{bin_dir}:{env['PATH']}"

        run(["python3", "loop", "init"], cwd=workdir, env=env)
        run(["python3", "loop", "add", "task one"], cwd=workdir, env=env)
        run(["python3", "loop", "add", "task two"], cwd=workdir, env=env)
        run(["python3", "loop", "add", "task three"], cwd=workdir, env=env)

        run_proc = run(["python3", "loop", "run"], cwd=workdir, env=env)
        status_proc = run(["python3", "loop", "status"], cwd=workdir, env=env)

        tasks = sorted((workdir / ".loop" / "tasks").glob("*.md"))
        complete_tasks = 0
        session_summaries: list[str] = []
        for task in tasks:
            fm = parse_frontmatter(task)
            if fm.get("status") == "complete":
                complete_tasks += 1
            title = fm.get("title", "unknown task")
            input_tokens = fm.get("input_tokens", "null")
            output_tokens = fm.get("output_tokens", "null")
            task_id = fm.get("id", "???")
            session_summaries.append(
                f"{task_id} | {title} | input_tokens={input_tokens} output_tokens={output_tokens}"
            )

        runs_file = workdir / "AGENT_RUNS.log"
        agent_runs = 0
        if runs_file.exists():
            agent_runs = len([line for line in runs_file.read_text(encoding="utf-8").splitlines() if line.strip()])

        commit_count = int(git(["rev-list", "--count", "HEAD"], cwd=workdir).stdout.strip())
        changed_files = (
            git(["diff", "--name-only", "HEAD~3..HEAD"], cwd=workdir).stdout.strip().splitlines()
            if commit_count >= 4
            else []
        )

        assert run_proc.returncode == 0, run_proc.stdout + "\n" + run_proc.stderr
        assert agent_runs == 3, f"Expected 3 agent runs, got {agent_runs}"
        assert complete_tasks == 3, f"Expected 3 complete tasks, got {complete_tasks}"
        assert commit_count >= 4, f"Expected at least 4 commits including init, got {commit_count}"
        assert "agent_changes.txt" in changed_files, (
            "Expected agent-mutated file in the last 3 commits; "
            "this may indicate the agent only printed output without edits."
        )

        return FinalResult(
            workdir=workdir,
            agent_runs=agent_runs,
            complete_tasks=complete_tasks,
            commit_count=commit_count,
            changed_files_last_three_commits=changed_files,
            status_stdout=status_proc.stdout.strip(),
            session_summaries=session_summaries,
        )


def main() -> int:
    parser = argparse.ArgumentParser(description="Integration harness for loop run")
    parser.add_argument(
        "--probe-real-claude",
        action="store_true",
        help="Additionally test whether local `claude -p` can edit files in a temp repo",
    )
    args = parser.parse_args()

    result = run_integration()
    print("Integration test passed.")
    print(f"agent_runs={result.agent_runs}")
    print(f"complete_tasks={result.complete_tasks}")
    print(f"commit_count={result.commit_count}")
    print("status:")
    print(result.status_stdout)
    print("changed_files_last_three_commits:")
    for item in result.changed_files_last_three_commits:
        print(f"- {item}")
    if args.probe_real_claude:
        print("")
        print("Running real claude -p edit probe...")
        probe_real_claude_edit()
        print("session_token_summary:")
        for line in result.session_summaries:
            print(line)
    return 0


def probe_real_claude_edit() -> None:
    with tempfile.TemporaryDirectory(prefix="claude-probe-") as tmp:
        workdir = Path(tmp)
        git(["init"], cwd=workdir)
        git(["config", "user.name", "Loop Test"], cwd=workdir)
        git(["config", "user.email", "loop-test@example.com"], cwd=workdir)
        prompt = (
            "Create a file named probe_edit.txt in the current directory with exactly one line: "
            "claude can edit files. Then exit."
        )
        proc = run(
            [
                "claude",
                "-p",
                prompt,
                "--dangerously-skip-permissions",
            ],
            cwd=workdir,
            check=False,
        )
        target = workdir / "probe_edit.txt"
        if proc.returncode != 0:
            raise AssertionError(
                "Real claude -p probe failed.\n"
                f"stdout:\n{proc.stdout}\n\nstderr:\n{proc.stderr}"
            )
        if not target.exists():
            raise AssertionError(
                "Real claude -p completed but did not create probe_edit.txt. "
                "This suggests it may have responded in text without writing files."
            )
        print("real_claude_probe=passed")


if __name__ == "__main__":
    raise SystemExit(main())
