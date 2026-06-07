//! Black-box integration tests: invoke the `bp` binary with isolated temp directories.

use std::path::Path;
use std::process::Command;

use rusqlite::params;
use tempfile::TempDir;

fn bp_bin() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_bp"))
}

fn run_in(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new(bp_bin())
        .current_dir(dir)
        .args(args)
        .output()
        .expect("spawn bp")
}

/// Clear env flags inherited from the developer/CI host so `bp run` behaves like a fresh shell.
fn clear_run_env(cmd: &mut Command) {
    cmd
        .env_remove("BP_RUN_SKIP_AGENT")
        .env_remove("LOOP_RUN_SKIP_AGENT")
        .env_remove("BP_RUN_AGENT_SCRIPT")
        .env_remove("LOOP_RUN_AGENT_SCRIPT");
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
        assert!(text.contains("bp (big-plan)"));
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
    assert!(stderr.contains("bp -h"));
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
    assert!(stderr.contains("bp init"));
}

#[test]
fn run_stalls_when_agent_does_not_complete() {
    let tmp = TempDir::new().expect("tempdir");
    init_project(tmp.path());
    let add = run_in(tmp.path(), &["add", "do the thing"]);
    assert!(add.status.success());
    let mut cmd = Command::new(bp_bin());
    cmd.current_dir(tmp.path()).arg("run");
    clear_run_env(&mut cmd);
    cmd.env("BP_RUN_AGENT_SCRIPT", "true");
    let out = cmd.output().expect("spawn run");
    assert!(!out.status.success());
    let (stdout, stderr) = output_utf8(&out);
    assert!(stdout.contains("Running task"));
    assert!(stdout.contains("do the thing"));
    assert!(stderr.contains("did not complete"));
    assert!(stderr.contains("bp complete"));
}

#[test]
fn run_completes_when_agent_shell_invokes_bp_complete() {
    let tmp = TempDir::new().expect("tempdir");
    init_project(tmp.path());
    assert!(run_in(tmp.path(), &["add", "solo"]).status.success());
    let exe = bp_bin().display().to_string();
    let script = format!("exec '{exe}' complete --notes 'via run'");
    let mut cmd = Command::new(bp_bin());
    cmd.current_dir(tmp.path()).arg("run");
    clear_run_env(&mut cmd);
    cmd.env("BP_RUN_AGENT_SCRIPT", &script);
    let out = cmd.output().expect("spawn run");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let (stdout, _) = output_utf8(&out);
    assert!(stdout.contains("Running task"));
    assert!(stdout.contains("Task 001 complete"));
    let status = run_in(tmp.path(), &["status"]);
    assert!(status.status.success());
    let (listing, _) = output_utf8(&status);
    assert!(listing.contains("001"));
    assert!(listing.contains("complete"));
}

#[test]
fn run_completes_when_legacy_loop_env_script_set() {
    let tmp = TempDir::new().expect("tempdir");
    init_project(tmp.path());
    assert!(run_in(tmp.path(), &["add", "legacy env"]).status.success());
    let exe = bp_bin().display().to_string();
    let script = format!("exec '{exe}' complete --notes 'legacy'");
    let mut cmd = Command::new(bp_bin());
    cmd.current_dir(tmp.path()).arg("run");
    clear_run_env(&mut cmd);
    cmd.env("LOOP_RUN_AGENT_SCRIPT", &script);
    let out = cmd.output().expect("spawn run");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let (stdout, _) = output_utf8(&out);
    assert!(stdout.contains("Task 001 complete"));
}

#[test]
fn run_skip_agent_completes_all_pending_without_subprocess() {
    let tmp = TempDir::new().expect("tempdir");
    init_project(tmp.path());
    assert!(run_in(tmp.path(), &["add", "a"]).status.success());
    assert!(run_in(tmp.path(), &["add", "b"]).status.success());
    let out = Command::new(bp_bin())
        .current_dir(tmp.path())
        .arg("run")
        .env("BP_RUN_SKIP_AGENT", "1")
        .output()
        .expect("spawn run");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let (stdout, _) = output_utf8(&out);
    assert!(stdout.contains("Task 001 complete"));
    assert!(stdout.contains("Task 002 complete"));
    let status = run_in(tmp.path(), &["status"]);
    let (listing, _) = output_utf8(&status);
    assert!(listing.lines().filter(|l| l.contains("complete")).count() >= 2);
}

#[test]
fn run_marks_failed_on_nonzero_agent_exit() {
    let tmp = TempDir::new().expect("tempdir");
    init_project(tmp.path());
    assert!(run_in(tmp.path(), &["add", "flaky"]).status.success());
    let mut cmd = Command::new(bp_bin());
    cmd.current_dir(tmp.path()).arg("run");
    clear_run_env(&mut cmd);
    cmd.env("BP_RUN_AGENT_SCRIPT", "exit 2");
    let out = cmd.output().expect("spawn run");
    assert!(!out.status.success());
    let (_, stderr) = output_utf8(&out);
    assert!(stderr.contains("failed (exit code 2)"));
    assert!(stderr.contains("bp run stopped"));
    let show = run_in(tmp.path(), &["show", "001"]);
    assert!(show.status.success());
    let (detail, _) = output_utf8(&show);
    assert!(detail.contains("Status:  failed"));
}

#[test]
fn run_processes_multiple_pending_tasks_in_order() {
    let tmp = TempDir::new().expect("tempdir");
    init_project(tmp.path());
    assert!(run_in(tmp.path(), &["add", "first"]).status.success());
    assert!(run_in(tmp.path(), &["add", "second"]).status.success());
    let exe = bp_bin().display().to_string();
    let script = format!("exec '{exe}' complete --notes 'ok'");
    let mut cmd = Command::new(bp_bin());
    cmd.current_dir(tmp.path()).arg("run");
    clear_run_env(&mut cmd);
    cmd.env("BP_RUN_AGENT_SCRIPT", &script);
    let out = cmd.output().expect("spawn run");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let (stdout, _) = output_utf8(&out);
    assert!(stdout.contains("Task 001 complete"));
    assert!(stdout.contains("Task 002 complete"));
    let status = run_in(tmp.path(), &["status"]);
    let (listing, _) = output_utf8(&status);
    assert!(listing.lines().filter(|l| l.contains("complete")).count() >= 2);
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
fn complete_if_running_is_silent_without_running_task() {
    let tmp = TempDir::new().expect("tempdir");
    init_project(tmp.path());
    let out = run_in(tmp.path(), &["complete", "--if-running"]);
    assert!(out.status.success());
    let (_, stderr) = output_utf8(&out);
    assert!(!stderr.contains("no task is currently running"));
}

#[test]
fn status_warns_when_task_running_without_active_bp_run() {
    let tmp = TempDir::new().expect("tempdir");
    init_project(tmp.path());
    assert!(run_in(tmp.path(), &["add", "stuck"]).status.success());
    force_task_running(tmp.path(), "001");
    let out = run_in(tmp.path(), &["status"]);
    assert!(out.status.success());
    let (stdout, _) = output_utf8(&out);
    assert!(stdout.contains("running"));
    assert!(stdout.contains("no active bp run"));
    assert!(stdout.contains("bp reset 001"));
}

#[test]
fn parse_error_ux_add_without_title() {
    let tmp = TempDir::new().expect("tempdir");
    let out = run_in(tmp.path(), &["add"]);
    assert!(!out.status.success());
    let (_, stderr) = output_utf8(&out);
    assert!(stderr.contains("title is required"));
    assert!(stderr.contains("bp add"));
}
