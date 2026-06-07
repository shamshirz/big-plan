//! Tracks an in-flight `bp run` session so `bp status` can distinguish live vs stale runs.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunLock {
    pub pid: u32,
    pub task_id: String,
}

pub fn loop_dir(project_root: &Path) -> PathBuf {
    project_root.join(".loop")
}

pub fn lock_path(project_root: &Path) -> PathBuf {
    loop_dir(project_root).join("run.lock")
}

pub fn write_run_lock(project_root: &Path, task_id: &str) -> std::io::Result<()> {
    let path = lock_path(project_root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let content = format!("{}\n{}\n", std::process::id(), task_id);
    fs::write(path, content)
}

pub fn clear_run_lock(project_root: &Path) {
    let _ = fs::remove_file(lock_path(project_root));
}

pub fn read_run_lock(project_root: &Path) -> Option<RunLock> {
    let content = fs::read_to_string(lock_path(project_root)).ok()?;
    let mut lines = content.lines();
    let pid: u32 = lines.next()?.trim().parse().ok()?;
    let task_id = lines.next()?.trim().to_owned();
    if task_id.is_empty() {
        return None;
    }
    Some(RunLock { pid, task_id })
}

pub fn process_alive(pid: u32) -> bool {
    Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Human-readable summary for `bp status` when a run lock and/or running task exist.
pub fn format_run_activity(project_root: &Path, running_task_id: Option<&str>) -> Option<String> {
    let lock = read_run_lock(project_root);
    match (&lock, running_task_id) {
        (Some(l), _) if process_alive(l.pid) => Some(format!(
            "Active bp run: task {} (pid {})",
            l.task_id, l.pid
        )),
        (Some(l), Some(task_id)) if l.task_id == task_id => Some(format!(
            "Task {task_id} is marked running but bp run (pid {}) is not active — likely interrupted (sleep/crash). Run `bp reset {task_id}` to retry.",
            l.pid
        )),
        (None, Some(task_id)) => Some(format!(
            "Task {task_id} is marked running but no active bp run — likely interrupted (sleep/crash). Run `bp reset {task_id}` to retry."
        )),
        (Some(l), _) => Some(format!(
            "Stale run lock for task {} (pid {} not running). Run `bp reset {}` if the task is stuck.",
            l.task_id, l.pid, l.task_id
        )),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn write_read_clear_run_lock() {
        let tmp = TempDir::new().unwrap();
        write_run_lock(tmp.path(), "008").unwrap();
        let lock = read_run_lock(tmp.path()).unwrap();
        assert_eq!(lock.task_id, "008");
        assert!(lock.pid > 0);
        clear_run_lock(tmp.path());
        assert!(read_run_lock(tmp.path()).is_none());
    }

    #[test]
    fn process_alive_current_pid() {
        assert!(process_alive(std::process::id()));
    }

    #[test]
    fn process_alive_dead_pid() {
        assert!(!process_alive(999_999_999));
    }
}
