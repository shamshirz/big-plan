//! Sequential `bp run` orchestration: prompt layering and agent subprocess boundary.

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};

use chrono::Utc;

use crate::agent_stream::AgentUsageMetrics;
use crate::domain::{
    check_no_running_task, merge_completion_metrics, next_pending, transition_complete,
    transition_fail, transition_start, CompletionData, DomainError, Task, TaskStatus,
};
use crate::render::render_task_markdown;
use crate::repository::{LoopError, TaskRepository};
use crate::run_lock;

/// Outcome of an agent subprocess invocation.
pub struct AgentOutcome {
    pub status: std::process::ExitStatus,
    pub usage: AgentUsageMetrics,
}

/// Options for `bp run` (CLI flags with env fallbacks applied in `commands::run`).
#[derive(Debug, Clone, Default)]
pub struct RunConfig {
    pub agent_model: Option<String>,
}

struct RunLockGuard<'a> {
    project_root: &'a Path,
}

impl Drop for RunLockGuard<'_> {
    fn drop(&mut self) {
        run_lock::clear_run_lock(self.project_root);
    }
}

/// Universal prompt slice (stable, installable without repo-local `AGENT.md`).
const DEFAULT_UNIVERSAL_GUIDANCE: &str = "# Universal guidance\n\n\
- Execute exactly one planning task unless told otherwise.\n\
- Keep edits minimal and focused; avoid unrelated refactors.\n\
- Record outcomes with `bp complete [--notes \"...\"]` when finished.\n\
- Use `bp read plan`, `bp read current`, or `bp read <id>` for canonical task text.\n\
\n";

/// Runs pending tasks until none remain, or until an agent failure / stall.
pub fn execute_run(repo: &dyn TaskRepository, project_root: &Path, config: &RunConfig) -> i32 {
    loop {
        let tasks = match repo.list_tasks() {
            Ok(t) => t,
            Err(LoopError::NotInitialized) => {
                eprintln!("error: bp not initialized — run `bp init` first");
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
                eprintln!("error: bp not initialized — run `bp init` first");
                return 1;
            }
            Err(e) => {
                eprintln!("error: {e}");
                return 2;
            }
        };

        if let Err(e) = repo.update_task(started) {
            if matches!(e, LoopError::NotInitialized) {
                eprintln!("error: bp not initialized — run `bp init` first");
                return 1;
            }
            eprintln!("error: {e}");
            return 2;
        }

        println!("Running task {}: {}", task_id, task_title);
        if let Err(e) = run_lock::write_run_lock(project_root, &task_id) {
            eprintln!("error: could not write run lock — {e}");
            return 2;
        }
        let _run_guard = RunLockGuard { project_root };

        if run_skip_agent_enabled() {
            let running_task = match load_running_task(repo, &task_id) {
                Ok(t) => t,
                Err(code) => return code,
            };
            let data = CompletionData {
                notes: "auto-completed via BP_RUN_SKIP_AGENT=1 (integration / CI; no agent subprocess)"
                    .to_owned(),
                completed_at: Utc::now(),
                input_tokens: None,
                output_tokens: None,
                model: Some("skip-agent".to_owned()),
                commit_sha: None,
            };
            let completed = match transition_complete(running_task, data) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("error: {e}");
                    return 2;
                }
            };
            if let Err(e) = repo.update_task(completed) {
                eprintln!("error: {e}");
                return 2;
            }
            println!("Task {task_id} complete.");
            continue;
        }

        let git_head_before = git_head_short(project_root);
        let outcome = match invoke_agent(&prompt, project_root, config) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error: could not run agent subprocess — {e}");
                return 2;
            }
        };

        let exit_detail = format_exit_detail(&outcome.status);

        if !outcome.status.success() {
            return fail_task_and_stop(repo, &task_id, &exit_detail);
        }

        let commit_sha = git_commit_after_task(project_root, git_head_before.as_deref());

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
                if let Some(task) = task_state {
                    if let Err(code) = patch_agent_metrics(
                        repo,
                        task,
                        &outcome.usage,
                        config.agent_model.as_deref(),
                        commit_sha,
                    ) {
                        return code;
                    }
                }
                println!("Task {task_id} complete.");
            }
            Some(TaskStatus::Running) => {
                eprintln!(
                    "error: task {task_id} exited successfully ({exit_detail}) but did not complete — run `bp complete` during the agent session or `bp reset {task_id}` to retry"
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

fn load_running_task(repo: &dyn TaskRepository, task_id: &str) -> Result<Task, i32> {
    let tasks = match repo.list_tasks() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: {e}");
            return Err(2);
        }
    };
    match tasks.iter().find(|t| t.id.as_str() == task_id) {
        Some(t) if t.status == TaskStatus::Running => Ok(t.clone()),
        _ => {
            eprintln!("error: internal: running task {task_id} not found after start");
            Err(2)
        }
    }
}

fn run_skip_agent_enabled() -> bool {
    env_truthy_two_keys("BP_RUN_SKIP_AGENT", "LOOP_RUN_SKIP_AGENT")
}

fn env_truthy_two_keys(primary: &str, legacy: &str) -> bool {
    std::env::var(primary)
        .or_else(|_| std::env::var(legacy))
        .ok()
        .map(|v| {
            let v = v.trim().to_ascii_lowercase();
            matches!(v.as_str(), "1" | "true" | "yes" | "on")
        })
        .unwrap_or(false)
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
/// Environment (prefer `BP_*`; `LOOP_*` is still accepted):
/// - `BP_RUN_AGENT_SHELL` / `LOOP_RUN_AGENT_SHELL`: shell executable (default: `sh`).
/// - `BP_RUN_AGENT_SCRIPT` / `LOOP_RUN_AGENT_SCRIPT`: argument to `shell -c` (highest priority).
/// - `BP_AGENT_BACKEND` / `LOOP_AGENT_BACKEND`: `cursor` (default) or `claude`
///   when no explicit script is provided.
///
/// Backend defaults automatically read prompt text from stdin and invoke
/// `bp complete` on successful exit, so users do not need to handcraft scripts.
pub fn invoke_agent(
    prompt: &str,
    project_root: &Path,
    config: &RunConfig,
) -> std::io::Result<AgentOutcome> {
    let bp_cmd = std::env::current_exe()
        .ok()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "bp".to_string());
    let (shell, script) = agent_shell_and_script(&bp_cmd, config);

    let mut child = Command::new(&shell)
        .current_dir(project_root)
        .arg("-c")
        .arg(&script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(prompt.as_bytes())?;
    }

    let mut usage = AgentUsageMetrics::default();
    if let Some(stdout) = child.stdout.take() {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            let line = line?;
            println!("{line}");
            usage.absorb_line(&line);
        }
    }

    let status = child.wait()?;
    Ok(AgentOutcome { status, usage })
}

fn agent_shell_and_script(bp_cmd: &str, config: &RunConfig) -> (String, String) {
    let shell = std::env::var("BP_RUN_AGENT_SHELL")
        .or_else(|_| std::env::var("LOOP_RUN_AGENT_SHELL"))
        .unwrap_or_else(|_| "sh".to_string());
    let script = match std::env::var("BP_RUN_AGENT_SCRIPT")
        .or_else(|_| std::env::var("LOOP_RUN_AGENT_SCRIPT"))
    {
        Ok(s) => s,
        Err(_) => {
            let backend = std::env::var("BP_AGENT_BACKEND")
                .or_else(|_| std::env::var("LOOP_AGENT_BACKEND"))
                .unwrap_or_else(|_| "cursor".to_string())
                .trim()
                .to_ascii_lowercase();
            match backend.as_str() {
                "claude" => default_claude_script(bp_cmd, config.agent_model.as_deref()),
                _ => default_cursor_script(bp_cmd, config.agent_model.as_deref()),
            }
        }
    };
    (shell, script)
}

fn default_cursor_script(bp_cmd: &str, agent_model: Option<&str>) -> String {
    let model_flag = agent_model
        .map(|m| format!(" --model \"{m}\""))
        .unwrap_or_default();
    let model_env = agent_model
        .map(|m| format!("BP_COMPLETE_MODEL=\"{m}\" "))
        .unwrap_or_default();
    format!(
        "prompt=\"$(cat)\"; cursor agent \"$prompt\"{model_flag} --print --force --trust --output-format stream-json; \
code=$?; if [ $code -eq 0 ]; then {model_env}'{bp_cmd}' complete --if-running --notes \"completed via cursor backend\" || true; fi; exit $code"
    )
}

fn default_claude_script(bp_cmd: &str, agent_model: Option<&str>) -> String {
    let model_env = agent_model
        .map(|m| format!("BP_COMPLETE_MODEL=\"{m}\" "))
        .unwrap_or_default();
    format!(
        "prompt=\"$(cat)\"; claude -p \"$prompt\" --verbose --output-format stream-json --dangerously-skip-permissions; \
code=$?; if [ $code -eq 0 ]; then {model_env}'{bp_cmd}' complete --if-running --notes \"completed via claude backend\" || true; fi; exit $code"
    )
}

fn format_exit_detail(status: &std::process::ExitStatus) -> String {
    if let Some(code) = status.code() {
        format!("exit code {code}")
    } else {
        "terminated without exit code".to_owned()
    }
}

fn git_head_short(project_root: &Path) -> Option<String> {
    Command::new("git")
        .current_dir(project_root)
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .filter(|s| !s.is_empty())
}

fn git_commit_after_task(project_root: &Path, head_before: Option<&str>) -> Option<String> {
    let head_after = git_head_short(project_root)?;
    match head_before {
        Some(before) if before == head_after.as_str() => None,
        _ => Some(head_after),
    }
}

fn patch_agent_metrics(
    repo: &dyn TaskRepository,
    task: &Task,
    usage: &AgentUsageMetrics,
    model_override: Option<&str>,
    commit_sha: Option<String>,
) -> Result<(), i32> {
    let model = usage
        .model
        .clone()
        .or_else(|| model_override.map(str::to_owned));
    let patch = CompletionData {
        notes: String::new(),
        completed_at: task.completed_at.unwrap_or_else(Utc::now),
        input_tokens: usage.input_tokens,
        output_tokens: usage.output_tokens,
        model,
        commit_sha,
    };

    let needs_patch = task.input_tokens.is_none()
        || task.output_tokens.is_none()
        || task.model.is_none()
        || task.commit_sha.is_none();
    if !needs_patch && patch.input_tokens.is_none() && patch.output_tokens.is_none() {
        return Ok(());
    }

    let merged = merge_completion_metrics(task.clone(), &patch);
    if merged.input_tokens == task.input_tokens
        && merged.output_tokens == task.output_tokens
        && merged.model == task.model
        && merged.commit_sha == task.commit_sha
    {
        return Ok(());
    }

    match repo.update_task(merged) {
        Ok(_) => Ok(()),
        Err(e) => {
            eprintln!("error: could not persist agent metrics — {e}");
            Err(2)
        }
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
            eprintln!("bp run stopped. Fix the task or run `bp reset {task_id}` to retry.");
            return 1;
        }
    };

    let failed_at = Utc::now();
    let failed = match transition_fail(running, failed_at) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: task '{task_id}' failed ({exit_detail}) — check agent output above");
            eprintln!("bp run stopped. Fix the task or run `bp reset {task_id}` to retry.");
            eprintln!("error: could not persist failed state — {e}");
            return 2;
        }
    };

    if let Err(e) = repo.update_task(failed) {
        eprintln!("error: task '{task_id}' failed ({exit_detail}) — check agent output above");
        eprintln!("bp run stopped. Fix the task or run `bp reset {task_id}` to retry.");
        eprintln!("error: could not persist failed state — {e}");
        return 2;
    }

    eprintln!("error: task {task_id} failed ({exit_detail}) — check agent output above");
    eprintln!("bp run stopped. Fix the task or run `bp reset {task_id}` to retry.");
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
        fn list_events(&self) -> Result<Vec<crate::domain::Event>, LoopError> {
            Ok(vec![])
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

    #[test]
    fn universal_guidance_mentions_bp_not_loop() {
        assert!(DEFAULT_UNIVERSAL_GUIDANCE.contains("bp complete"));
        assert!(DEFAULT_UNIVERSAL_GUIDANCE.contains("bp read"));
    }

    #[test]
    fn default_cursor_script_includes_model_flag() {
        let script = default_cursor_script("/usr/bin/bp", Some("composer-2.5"));
        assert!(script.contains("--model \"composer-2.5\""));
        assert!(script.contains("complete --if-running"));
    }

    #[test]
    fn default_cursor_script_uses_stream_json() {
        let script = default_cursor_script("/usr/bin/bp", Some("composer-2.5"));
        assert!(script.contains("--output-format stream-json"));
        assert!(script.contains("BP_COMPLETE_MODEL=\"composer-2.5\""));
    }

    #[test]
    fn default_cursor_script_omits_model_when_unset() {
        let script = default_cursor_script("/usr/bin/bp", None);
        assert!(!script.contains("--model"));
        assert!(!script.contains("BP_COMPLETE_MODEL"));
    }
}
