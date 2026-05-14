//! Opt-in integration tests that spawn **real** Cursor Agent or Claude Code.
//!
//! These are **ignored** by default (cost, latency). Run manually, for example:
//!
//! ```text
//! BP_REAL_AGENT_BACKEND=cursor cargo test -p big-plan real_agent_smoke -- --ignored --nocapture
//! BP_REAL_AGENT_BACKEND=claude cargo test -p big-plan real_agent_smoke -- --ignored --nocapture
//! ```
//!
//! The agent receives the composed prompt on **stdin** of `sh -c` via `$(cat)` inside
//! `BP_RUN_AGENT_SCRIPT`. If the model does not run `bp complete`, the test script may still
//! succeed by chaining `exec … bp complete` (optional tail) — adjust for your setup.

use std::path::Path;
use std::process::Command;

use tempfile::TempDir;

fn bp_bin() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_bp"))
}

#[test]
#[ignore = "manual: real agent; set BP_REAL_AGENT_BACKEND=cursor|claude and authenticate the CLI"]
fn real_agent_smoke() {
    let backend = std::env::var("BP_REAL_AGENT_BACKEND").expect(
        "set BP_REAL_AGENT_BACKEND=cursor or claude when running with --ignored",
    );
    assert!(
        backend == "cursor" || backend == "claude",
        "BP_REAL_AGENT_BACKEND must be 'cursor' or 'claude', got {backend:?}"
    );

    let tmp = TempDir::new().expect("tempdir");
    let dir = tmp.path();

    assert!(
        Command::new(bp_bin())
            .current_dir(dir)
            .arg("init")
            .status()
            .expect("init")
            .success()
    );
    assert!(
        Command::new(bp_bin())
            .current_dir(dir)
            .args([
                "add",
                "Smoke: follow tool instructions and run `bp complete --notes done` when finished",
            ])
            .status()
            .expect("add")
            .success()
    );

    let exe = bp_bin().display().to_string();
    let agent_line = match backend.as_str() {
        "cursor" => format!(
            r#"{} agent -p "$(cat)" --print --force --trust --output-format text"#,
            find_in_path("cursor").expect("cursor not on PATH")
        ),
        "claude" => format!(
            r#"{} -p "$(cat)" --verbose --output-format stream-json --dangerously-skip-permissions"#,
            find_in_path("claude").expect("claude not on PATH")
        ),
        _ => unreachable!(),
    };
    let script =
        format!("{agent_line}; exec '{exe}' complete --notes 'real-agent-smoke-fallback-complete'");

    let status = Command::new(bp_bin())
        .current_dir(dir)
        .arg("run")
        .env("BP_RUN_AGENT_SCRIPT", &script)
        .status()
        .expect("bp run");

    assert!(
        status.success(),
        "bp run failed — check agent stderr; remove fallback `exec … complete` if you want to require the model to call `bp complete`."
    );

    let out = Command::new(bp_bin())
        .current_dir(dir)
        .arg("status")
        .output()
        .expect("status");
    let listing = String::from_utf8_lossy(&out.stdout);
    assert!(
        listing.contains("complete"),
        "expected task complete in status output:\n{listing}"
    );
}

fn find_in_path(cmd: &str) -> Option<String> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(cmd);
        if candidate.is_file() {
            return candidate.to_str().map(|s| s.to_owned());
        }
    }
    None
}
