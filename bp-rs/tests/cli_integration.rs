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
    cmd.env_remove("BP_RUN_SKIP_AGENT")
        .env_remove("BP_RUN_AGENT_SCRIPT");
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
    assert!(listing.contains("Progress: 1/1 complete"));
    assert!(listing.contains("✓ 001 complete"));
    assert!(listing.contains("solo"));
}

#[test]
fn run_from_plan_creates_goal_and_planning_task() {
    let tmp = TempDir::new().expect("tempdir");
    init_project(tmp.path());
    std::fs::write(
        tmp.path().join("plan.md"),
        "# Simplification\n\nDo the thing.\n",
    )
    .unwrap();
    let mut cmd = Command::new(bp_bin());
    cmd.current_dir(tmp.path())
        .args(["run", "plan.md"])
        .env("BP_RUN_SKIP_AGENT", "1");
    let out = cmd.output().expect("spawn run");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let (stdout, _) = output_utf8(&out);
    assert!(stdout.contains("Started goal"));
    assert!(stdout.contains("planning task"));
    let status = run_in(tmp.path(), &["status"]);
    let (listing, _) = output_utf8(&status);
    assert!(listing.contains("(active): plan"));
    assert!(listing.contains("Progress:"));
    assert!(listing.contains("Digest:"));
    assert!(listing.contains("Plan:"));
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
    assert!(out_b.contains("Goal 1 (active): Initial"));
    assert!(!out_b.contains("Progress:"));
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
    assert!(stdout.contains("Progress: 0/1 complete · 1 running"));
    assert!(stdout.contains("Digest: Run in progress"));
    assert!(stdout.contains("▶ 001 running"));
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

fn seed_completed_task(
    dir: &Path,
    id: &str,
    seq: i64,
    started: &str,
    completed: &str,
    duration: i64,
    notes: &str,
    commit_sha: Option<&str>,
    input_tokens: Option<i64>,
    output_tokens: Option<i64>,
) {
    let db = dir.join(".loop").join("loop.db");
    let conn = rusqlite::Connection::open(&db).expect("open loop.db");
    conn.execute(
        "INSERT INTO tasks (id, seq, goal_id, kind, title, status, depends_on, created_at, started_at, \
         completed_at, duration_seconds, completion_notes_md, input_tokens, output_tokens, \
         model, commit_sha) \
         VALUES (?1, ?2, 1, 'execute', ?3, 'complete', '[]', ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            id,
            seq,
            format!("task {id}"),
            started,
            started,
            completed,
            duration,
            notes,
            input_tokens,
            output_tokens,
            "composer-2.5",
            commit_sha,
        ],
    )
    .expect("seed task");
}

#[test]
fn status_shows_progress_and_digest() {
    let tmp = TempDir::new().expect("tempdir");
    init_project(tmp.path());
    seed_completed_task(
        tmp.path(),
        "001",
        1,
        "2026-06-17T02:21:29Z",
        "2026-06-17T02:23:11Z",
        102,
        "done",
        None,
        None,
        None,
    );
    assert!(run_in(tmp.path(), &["add", "active"]).status.success());
    assert!(run_in(tmp.path(), &["add", "waiting"]).status.success());
    force_task_running(tmp.path(), "002");
    let out = run_in(tmp.path(), &["status"]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let (stdout, _) = output_utf8(&out);
    assert!(stdout.contains("Progress: 1/3 complete · 1 running · 1 pending"));
    assert!(stdout.contains("Digest: Run in progress"));
    assert!(stdout.contains("now on 002"));
    assert!(stdout.contains("Next: 003"));
    assert!(stdout.contains("Last finished: 001"));
    assert!(stdout.contains("ID    STATUS"));
}

#[test]
fn status_shows_duration_and_commit() {
    let tmp = TempDir::new().expect("tempdir");
    init_project(tmp.path());
    seed_completed_task(
        tmp.path(),
        "001",
        1,
        "2026-06-17T02:21:29Z",
        "2026-06-17T02:23:11Z",
        102,
        "Commit: abc1234 decompose build plan into bp queue",
        Some("abc1234"),
        None,
        None,
    );
    let out = run_in(tmp.path(), &["status"]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let (stdout, _) = output_utf8(&out);
    assert!(stdout.contains("✓ 001 complete"));
    assert!(stdout.contains("1m 42s"));
    assert!(stdout.contains("abc1234 decompose"));
}

#[test]
fn status_shows_unicode_markers() {
    let tmp = TempDir::new().expect("tempdir");
    init_project(tmp.path());
    seed_completed_task(
        tmp.path(),
        "001",
        1,
        "2026-06-17T02:00:00Z",
        "2026-06-17T02:01:00Z",
        60,
        "ok",
        None,
        None,
        None,
    );
    assert!(run_in(tmp.path(), &["add", "active"]).status.success());
    assert!(run_in(tmp.path(), &["add", "waiting"]).status.success());
    force_task_running(tmp.path(), "002");
    let out = run_in(tmp.path(), &["status"]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let (stdout, _) = output_utf8(&out);
    assert!(stdout.contains("✓ 001"));
    assert!(stdout.contains("▶ 002"));
    assert!(stdout.contains("· 003"));
}

#[test]
fn summary_reports_wall_clock_and_commits() {
    let tmp = TempDir::new().expect("tempdir");
    init_project(tmp.path());
    seed_completed_task(
        tmp.path(),
        "001",
        1,
        "2026-06-17T02:21:29Z",
        "2026-06-17T02:23:11Z",
        102,
        "Commit: abc1234 decompose build plan into bp queue",
        Some("abc1234"),
        None,
        None,
    );
    seed_completed_task(
        tmp.path(),
        "002",
        2,
        "2026-06-17T02:25:00Z",
        "2026-06-17T03:01:13Z",
        346,
        "Commit: f6d751c scaffold Phoenix app with SQLite at repo root",
        Some("f6d751c"),
        Some(1200),
        Some(340),
    );

    let out = run_in(tmp.path(), &["summary"]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let (stdout, _) = output_utf8(&out);
    assert!(stdout.contains("Run summary (2 tasks complete)"));
    assert!(stdout.contains("2026-06-17 02:21:29 → 2026-06-17 03:01:13 UTC"));
    assert!(stdout.contains("Agent time:"));
    assert!(stdout.contains("1.2k/340"));
    assert!(stdout.contains("abc1234 decompose build plan"));
    assert!(stdout.contains("f6d751c scaffold Phoenix"));
}

#[test]
fn run_captures_stream_json_tokens_from_agent_stdout() {
    let tmp = TempDir::new().expect("tempdir");
    init_project(tmp.path());
    assert!(run_in(tmp.path(), &["add", "metrics"]).status.success());
    let exe = bp_bin().display().to_string();
    let script = format!(
        r#"printf '%s\n' '{{"type":"result","usage":{{"input_tokens":42,"output_tokens":7}}}}'; exec '{exe}' complete --if-running --notes 'with tokens'"#
    );
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
    let show = run_in(tmp.path(), &["show", "001"]);
    assert!(show.status.success());
    let (detail, _) = output_utf8(&show);
    assert!(detail.contains("Tokens in/out: 42 / 7"));
}
