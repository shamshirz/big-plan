//! Black-box integration tests: invoke the `loop` binary with isolated temp directories.

use std::path::Path;
use std::process::Command;

use rusqlite::params;
use tempfile::TempDir;

fn loop_bin() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_loop"))
}

fn run_in(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new(loop_bin())
        .current_dir(dir)
        .args(args)
        .output()
        .expect("spawn loop")
}

fn output_utf8(out: &std::process::Output) -> (String, String) {
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    (stdout, stderr)
}

fn init_project(dir: &Path) {
    let out = run_in(dir, &["init"]);
    assert!(
        out.status.success(),
        "init failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Mark task `id` as running so `complete` / `run` behavior can be exercised without an agent.
fn force_task_running(dir: &Path, id: &str) {
    let db = dir.join(".loop").join("loop.db");
    let conn = rusqlite::Connection::open(&db).expect("open loop.db");
    conn.execute(
        "UPDATE tasks SET status = 'running', started_at = ?1 WHERE id = ?2",
        params!["2026-05-07T12:00:00Z", id],
    )
    .expect("set running");
}

#[test]
fn help_and_no_args_print_usage() {
    for args in [&["--help"][..], &["-h"][..], &[][..]] {
        let tmp = TempDir::new().expect("tempdir");
        let out = run_in(tmp.path(), args);
        assert!(out.status.success());
        let (stdout, stderr) = output_utf8(&out);
        let text = format!("{stdout}{stderr}");
        assert!(text.contains("loop — project-local task orchestration"));
        assert!(text.contains("Usage:"));
        assert!(text.contains("Commands:"));
        assert!(text.contains("init"));
        assert!(text.contains("run"));
    }
}

#[test]
fn unknown_command_exits_with_usage_hint() {
    let tmp = TempDir::new().expect("tempdir");
    let out = run_in(tmp.path(), &["nope"]);
    assert!(!out.status.success());
    let (_, stderr) = output_utf8(&out);
    assert!(stderr.contains("unknown command"));
    assert!(stderr.contains("loop -h"));
}

#[test]
fn run_happy_path_no_pending_tasks() {
    let tmp = TempDir::new().expect("tempdir");
    init_project(tmp.path());
    let out = run_in(tmp.path(), &["run"]);
    assert!(out.status.success());
    let (stdout, _) = output_utf8(&out);
    assert_eq!(stdout.trim(), "No pending tasks.");
}

#[test]
fn run_fails_when_not_initialized() {
    let tmp = TempDir::new().expect("tempdir");
    let out = run_in(tmp.path(), &["run"]);
    assert!(!out.status.success());
    let (_, stderr) = output_utf8(&out);
    assert!(stderr.contains("not initialized"));
    assert!(stderr.contains("loop init"));
}

#[test]
fn run_fails_when_pending_exists_agent_stub() {
    let tmp = TempDir::new().expect("tempdir");
    init_project(tmp.path());
    let add = run_in(tmp.path(), &["add", "do the thing"]);
    assert!(add.status.success());
    let out = run_in(tmp.path(), &["run"]);
    assert!(!out.status.success());
    let (stdout, stderr) = output_utf8(&out);
    assert!(stdout.contains("Running task"));
    assert!(stdout.contains("do the thing"));
    assert!(stderr.contains("agent subprocess invocation not yet implemented"));
}

#[test]
fn run_fails_when_task_already_running() {
    let tmp = TempDir::new().expect("tempdir");
    init_project(tmp.path());
    assert!(run_in(tmp.path(), &["add", "blocked"]).status.success());
    force_task_running(tmp.path(), "001");
    let out = run_in(tmp.path(), &["run"]);
    assert!(!out.status.success());
    let (_, stderr) = output_utf8(&out);
    assert!(stderr.contains("already running"));
    assert!(stderr.contains("001"));
}

#[test]
fn project_local_isolation_separate_directories() {
    let a = TempDir::new().expect("tempdir a");
    let b = TempDir::new().expect("tempdir b");
    init_project(a.path());
    init_project(b.path());
    assert!(run_in(a.path(), &["add", "only in A"]).status.success());
    let status_a = run_in(a.path(), &["status"]);
    let status_b = run_in(b.path(), &["status"]);
    assert!(status_a.status.success());
    assert!(status_b.status.success());
    let (out_a, _) = output_utf8(&status_a);
    let (out_b, _) = output_utf8(&status_b);
    assert!(out_a.contains("only in A"));
    assert!(out_b.contains("No tasks."));
    assert!(!out_b.contains("only in A"));
}

#[test]
fn init_add_complete_reset_end_to_end() {
    let tmp = TempDir::new().expect("tempdir");
    init_project(tmp.path());
    assert!(run_in(tmp.path(), &["add", "e2e task"]).status.success());
    force_task_running(tmp.path(), "001");
    let done = run_in(tmp.path(), &["complete", "--notes", "verified"]);
    assert!(done.status.success());
    let (stdout, _) = output_utf8(&done);
    assert!(stdout.contains("marked complete"));
    let show = run_in(tmp.path(), &["show", "001"]);
    assert!(show.status.success());
    let (detail, _) = output_utf8(&show);
    assert!(detail.contains("Status:  complete"));
    assert!(detail.contains("verified"));
    let reset = run_in(tmp.path(), &["reset", "001"]);
    assert!(reset.status.success());
    let show2 = run_in(tmp.path(), &["show", "001"]);
    assert!(show2.status.success());
    let (after, _) = output_utf8(&show2);
    assert!(after.contains("Status:  pending"));
}

#[test]
fn parse_error_ux_add_without_title() {
    let tmp = TempDir::new().expect("tempdir");
    let out = run_in(tmp.path(), &["add"]);
    assert!(!out.status.success());
    let (_, stderr) = output_utf8(&out);
    assert!(stderr.contains("title is required"));
    assert!(stderr.contains("loop add"));
}
