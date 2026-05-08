//! Sequential `loop run` orchestration: prompt layering and agent subprocess boundary.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use chrono::Utc;

use crate::domain::{
    check_no_running_task, next_pending, transition_fail, transition_start, DomainError, Task,
    TaskStatus,
};
use crate::render::render_task_markdown;
use crate::repository::{LoopError, TaskRepository};

/// Universal prompt slice (stable, installable without repo-local `AGENT.md`).
const DEFAULT_UNIVERSAL_GUIDANCE: &str = "# Universal guidance\n\n\
- Execute exactly one planning task unless told otherwise.\n\
- Keep edits minimal and focused; avoid unrelated refactors.\n\
- Record outcomes with `loop complete [--notes \"...\"]` when finished.\n\
- Use `loop read plan`, `loop read current`, or `loop read <id>` for canonical task text.\n\
\n";

/// Runs pending tasks until none remain, or until an agent failure / stall.
pub fn execute_run(repo: &dyn TaskRepository, project_root: &Path) -> i32 {
    loop {
        let tasks = match repo.list_tasks() {
            Ok(t) => t,
            Err(LoopError::NotInitialized) => {
                eprintln!("error: loop not initialized — run `loop init` first");
                return 1;
            }
            Err(e) => {
                eprintln!("error: {e}");
                return 1;
            }
        };

        if let Err(DomainError::AlreadyRunning(id)) = check_no_running_task(&tasks) {
            eprintln!(
                "error: task {id} is already running — complete or reset it before running again"
            );
            return 1;
        }

        let pending = match next_pending(&tasks) {
            None => {
                println!("No pending tasks.");
                return 0;
            }
            Some(t) => t.clone(),
        };
        let task_title = pending.title.clone();

        let started = match transition_start(pending, Utc::now()) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("error: {e}");
                return 2;
            }
        };

        let task_id = started.id.to_string();
        let prompt = match assemble_layered_prompt(repo, &started) {
            Ok(p) => p,
            Err(LoopError::NotInitialized) => {
                eprintln!("error: loop not initialized — run `loop init` first");
                return 1;
            }
            Err(e) => {
                eprintln!("error: {e}");
                return 2;
            }
        };

        if let Err(e) = repo.update_task(started) {
            if matches!(e, LoopError::NotInitialized) {
                eprintln!("error: loop not initialized — run `loop init` first");
                return 1;
            }
            eprintln!("error: {e}");
            return 2;
        }

        println!("Running task {}: {}", task_id, task_title);

        let status = match invoke_agent(&prompt, project_root) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error: could not run agent subprocess — {e}");
                return 2;
            }
        };

        let exit_detail = format_exit_detail(&status);

        if !status.success() {
            return fail_task_and_stop(repo, &task_id, &exit_detail);
        }

        let tasks_after = match repo.list_tasks() {
            Ok(t) => t,
            Err(e) => {
                eprintln!("error: {e}");
                return 2;
            }
        };

        let task_state = tasks_after
            .iter()
            .find(|t| t.id.as_str() == task_id.as_str());
        match task_state.map(|t| t.status) {
            Some(TaskStatus::Complete) => {
                println!("Task {task_id} complete.");
            }
            Some(TaskStatus::Running) => {
                eprintln!(
                    "error: task {task_id} exited successfully ({exit_detail}) but did not complete — run `loop complete` during the agent session or `loop reset {task_id}` to retry"
                );
                return 1;
            }
            Some(TaskStatus::Failed) | Some(TaskStatus::Pending) | None => {
                eprintln!("error: task {task_id} finished in an unexpected state after agent exit");
                return 2;
            }
        }
    }
}

fn assemble_layered_prompt(repo: &dyn TaskRepository, task: &Task) -> Result<String, LoopError> {
    let project_md = repo.read_agent_project()?.trim().to_owned();
    let project_slice = if project_md.is_empty() {
        "(no project context)\n".to_owned()
    } else {
        format!("{project_md}\n")
    };

    Ok(format!(
        "## Universal Guidance\n\n{DEFAULT_UNIVERSAL_GUIDANCE}\
         ---\n\n## Project-Specific Context\n\n{project_slice}\
         ---\n\n## Task-Specific Context\n\n{}",
        render_task_markdown(task)
    ))
}

/// Spawns the agent subprocess with the composed prompt on stdin.
///
/// Environment:
/// - `LOOP_RUN_AGENT_SHELL`: shell executable (default: `sh`).
/// - `LOOP_RUN_AGENT_SCRIPT`: argument to `shell -c` (default: `true`, consumes no meaningful work).
pub fn invoke_agent(
    prompt: &str,
    project_root: &Path,
) -> std::io::Result<std::process::ExitStatus> {
    let shell = std::env::var("LOOP_RUN_AGENT_SHELL").unwrap_or_else(|_| "sh".to_string());
    let script = std::env::var("LOOP_RUN_AGENT_SCRIPT").unwrap_or_else(|_| "true".to_string());

    let mut child = Command::new(&shell)
        .current_dir(project_root)
        .arg("-c")
        .arg(&script)
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(prompt.as_bytes())?;
    }

    child.wait()
}

fn format_exit_detail(status: &std::process::ExitStatus) -> String {
    if let Some(code) = status.code() {
        format!("exit code {code}")
    } else {
        "terminated without exit code".to_owned()
    }
}

fn fail_task_and_stop(repo: &dyn TaskRepository, task_id: &str, exit_detail: &str) -> i32 {
    let tasks = match repo.list_tasks() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };

    let running = match tasks.iter().find(|t| t.id.as_str() == task_id) {
        Some(t) if t.status == TaskStatus::Running => t.clone(),
        _ => {
            eprintln!("error: task '{task_id}' failed ({exit_detail}) — check agent output above");
            eprintln!("Loop stopped. Fix the task or run `loop reset {task_id}` to retry.");
            return 1;
        }
    };

    let failed_at = Utc::now();
    let failed = match transition_fail(running, failed_at) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: task '{task_id}' failed ({exit_detail}) — check agent output above");
            eprintln!("Loop stopped. Fix the task or run `loop reset {task_id}` to retry.");
            eprintln!("error: could not persist failed state — {e}");
            return 2;
        }
    };

    if let Err(e) = repo.update_task(failed) {
        eprintln!("error: task '{task_id}' failed ({exit_detail}) — check agent output above");
        eprintln!("Loop stopped. Fix the task or run `loop reset {task_id}` to retry.");
        eprintln!("error: could not persist failed state — {e}");
        return 2;
    }

    eprintln!("error: task {task_id} failed ({exit_detail}) — check agent output above");
    eprintln!("Loop stopped. Fix the task or run `loop reset {task_id}` to retry.");
    1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Task;
    use chrono::TimeZone;

    struct FakeRepo {
        project: String,
    }

    impl TaskRepository for FakeRepo {
        fn initialize(&self) -> Result<(), LoopError> {
            unimplemented!()
        }
        fn add_task(&self, _title: &str) -> Result<Task, LoopError> {
            unimplemented!()
        }
        fn list_tasks(&self) -> Result<Vec<Task>, LoopError> {
            unimplemented!()
        }
        fn get_task(&self, _id: &str) -> Result<Task, LoopError> {
            unimplemented!()
        }
        fn update_task(&self, _task: Task) -> Result<Task, LoopError> {
            unimplemented!()
        }
        fn read_plan(&self) -> Result<String, LoopError> {
            unimplemented!()
        }
        fn read_agent_project(&self) -> Result<String, LoopError> {
            Ok(self.project.clone())
        }
    }

    #[test]
    fn layered_prompt_ordering() {
        let ts = Utc.with_ymd_and_hms(2026, 5, 7, 12, 0, 0).unwrap();
        let task = Task::new(3, "Do the work".to_owned(), ts);
        let repo = FakeRepo {
            project: "Project slice here.".to_owned(),
        };
        let md = assemble_layered_prompt(&repo, &task).unwrap();
        let u = md.find("## Universal Guidance").unwrap();
        let p = md.find("## Project-Specific Context").unwrap();
        let t = md.find("## Task-Specific Context").unwrap();
        assert!(u < p && p < t);
        assert!(md.contains("Project slice here."));
        assert!(md.contains("# Task 003:"));
    }
}
