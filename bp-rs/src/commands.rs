use std::path::Path;

use chrono::Utc;

use crate::domain::{
    current_running, transition_complete, transition_reset, CompletionData, DomainError, TaskStatus,
};
use crate::orchestrator::RunConfig;
use crate::render::{
    fmt_duration_human, fmt_tokens_compact, fmt_ts_summary, render_task_detail, render_task_markdown,
};
use crate::repository::{LoopError, TaskRepository};
use crate::run_lock;
use crate::summary::{
    build_summary, summary_headline, task_commit_line, since_seq_from_id, SummaryFilter,
};

pub fn help() -> i32 {
    println!(
        "bp (big-plan) — project-local task orchestration\n\
         \n\
         Usage:\n\
           bp <command> [args]\n\
         \n\
         Commands:\n\
           init                  Initialize .loop/ state in the current directory\n\
           add \"<title>\"         Add a new pending task\n\
           status                List all tasks with ID, status, and title\n\
           show <id>             Print full task detail\n\
           summary [--json] [--since <id>] [--last-run]  Run completion report\n\
           read plan|current|<id>  Print planning or task text for agent use\n\
           run [--model <id>]    Execute pending tasks sequentially via agent sessions\n\
           complete [--notes \"\"] [--if-running] Mark the current task complete\n\
           reset <id>            Return a task to pending and clear metrics\n\
         \n\
         Run `bp <command> -h` for command-specific help."
    );
    0
}

pub fn init(repo: &dyn TaskRepository) -> i32 {
    match repo.initialize() {
        Ok(()) => {
            println!("Initialized big-plan state in .loop/");
            0
        }
        Err(LoopError::AlreadyInitialized) => {
            println!("big-plan already initialized in .loop/");
            0
        }
        Err(LoopError::PermissionDenied(path)) => {
            eprintln!("error: cannot create {path} — permission denied");
            1
        }
        Err(e) => {
            eprintln!("error: {e}");
            1
        }
    }
}

pub fn add(repo: &dyn TaskRepository, title: &str) -> i32 {
    match repo.add_task(title) {
        Ok(task) => {
            println!("Added task {}: {}", task.id, task.title);
            0
        }
        Err(LoopError::NotInitialized) => {
            eprintln!("error: bp not initialized — run `bp init` first");
            1
        }
        Err(e) => {
            eprintln!("error: {e}");
            1
        }
    }
}

pub fn status(repo: &dyn TaskRepository) -> i32 {
    let project_root = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());
    match repo.list_tasks() {
        Ok(tasks) if tasks.is_empty() => {
            println!("No tasks. Run `bp add \"<title>\"` to create one.");
            0
        }
        Ok(tasks) => {
            println!("{:<5} {:<10} {}", "ID", "STATUS", "TITLE");
            for task in &tasks {
                let mut line = format!(
                    "{:<5} {:<10} {}",
                    task.id,
                    task.status,
                    truncate(&task.title, 60)
                );
                if task.status == TaskStatus::Running {
                    if let Some(started_at) = task.started_at {
                        let elapsed = Utc::now().signed_duration_since(started_at);
                        let mins = elapsed.num_minutes();
                        if mins > 0 {
                            line.push_str(&format!("  ({mins}m running)"));
                        } else {
                            line.push_str("  (<1m running)");
                        }
                    }
                }
                println!("{line}");
            }
            let running_task_id = tasks
                .iter()
                .find(|t| t.status == TaskStatus::Running)
                .map(|t| t.id.as_str());
            if let Some(activity) = run_lock::format_run_activity(&project_root, running_task_id) {
                println!();
                println!("{activity}");
            }
            0
        }
        Err(LoopError::NotInitialized) => {
            eprintln!("error: bp not initialized — run `bp init` first");
            1
        }
        Err(e) => {
            eprintln!("error: {e}");
            1
        }
    }
}

pub fn show(repo: &dyn TaskRepository, id: &str) -> i32 {
    match repo.get_task(id) {
        Ok(task) => {
            print!("{}", render_task_detail(&task));
            0
        }
        Err(LoopError::NotInitialized) => {
            eprintln!("error: bp not initialized — run `bp init` first");
            1
        }
        Err(LoopError::TaskNotFound(id)) => {
            eprintln!("error: task '{id}' not found");
            1
        }
        Err(e) => {
            eprintln!("error: {e}");
            1
        }
    }
}

pub fn read_plan(repo: &dyn TaskRepository) -> i32 {
    match repo.read_plan() {
        Ok(content) => {
            if content.trim().is_empty() {
                println!("(no plan content)");
            } else {
                print!("{}", content);
            }
            0
        }
        Err(LoopError::NotInitialized) => {
            eprintln!("error: bp not initialized — run `bp init` first");
            1
        }
        Err(e) => {
            eprintln!("error: {e}");
            1
        }
    }
}

pub fn read_current(repo: &dyn TaskRepository) -> i32 {
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
    match current_running(&tasks) {
        Ok(task) => {
            print!("{}", render_task_markdown(task));
            0
        }
        Err(DomainError::NoRunningTask) => {
            eprintln!("error: no task is currently running");
            1
        }
        Err(e) => {
            eprintln!("error: {e}");
            1
        }
    }
}

pub fn read_task(repo: &dyn TaskRepository, id: &str) -> i32 {
    match repo.get_task(id) {
        Ok(task) => {
            print!("{}", render_task_markdown(&task));
            0
        }
        Err(LoopError::NotInitialized) => {
            eprintln!("error: bp not initialized — run `bp init` first");
            1
        }
        Err(LoopError::TaskNotFound(id)) => {
            eprintln!("error: task '{id}' not found");
            1
        }
        Err(e) => {
            eprintln!("error: {e}");
            1
        }
    }
}

pub fn run(repo: &dyn TaskRepository, project_root: &Path, agent_model: Option<&str>) -> i32 {
    let model = agent_model
        .map(str::to_owned)
        .or_else(|| std::env::var("BP_AGENT_MODEL").ok().filter(|s| !s.trim().is_empty()));
    let config = RunConfig { agent_model: model };
    crate::orchestrator::execute_run(repo, project_root, &config)
}

pub fn complete(repo: &dyn TaskRepository, notes: Option<&str>, if_running: bool) -> i32 {
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

    let running = match current_running(&tasks) {
        Ok(t) => t.clone(),
        Err(DomainError::NoRunningTask) => {
            if if_running {
                return 0;
            }
            eprintln!("error: no task is currently running");
            return 1;
        }
        Err(e) => {
            eprintln!("error: {e}");
            return 1;
        }
    };

    let task_id = running.id.clone();
    let (input_tokens, output_tokens, model, commit_sha) = completion_metrics_from_env();
    let data = CompletionData {
        notes: notes.unwrap_or("").to_owned(),
        completed_at: Utc::now(),
        input_tokens,
        output_tokens,
        model,
        commit_sha,
    };

    let updated = match transition_complete(running, data) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: {e}");
            return 1;
        }
    };

    match repo.update_task(updated) {
        Ok(_) => {
            println!("Task {} marked complete.", task_id);
            0
        }
        Err(e) => {
            eprintln!("error: {e}");
            1
        }
    }
}

pub fn summary(
    repo: &dyn TaskRepository,
    json: bool,
    since: Option<&str>,
    last_run: bool,
) -> i32 {
    let since_seq = match since {
        None => None,
        Some(id) => match since_seq_from_id(id) {
            Ok(seq) => Some(seq),
            Err(e) => {
                eprintln!("error: invalid --since task id: {e}");
                return 1;
            }
        },
    };

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

    let events = match repo.list_events() {
        Ok(e) => e,
        Err(LoopError::NotInitialized) => {
            eprintln!("error: bp not initialized — run `bp init` first");
            return 1;
        }
        Err(e) => {
            eprintln!("error: {e}");
            return 1;
        }
    };

    let filter = SummaryFilter {
        since_seq,
        last_run,
    };
    let summary = build_summary(&tasks, &filter, &events);

    if json {
        print!("{}", render_summary_json(&summary));
    } else {
        print!("{}", render_summary_text(&summary));
    }
    0
}

pub fn reset(repo: &dyn TaskRepository, id: &str) -> i32 {
    let task = match repo.get_task(id) {
        Ok(t) => t,
        Err(LoopError::NotInitialized) => {
            eprintln!("error: bp not initialized — run `bp init` first");
            return 1;
        }
        Err(LoopError::TaskNotFound(id)) => {
            eprintln!("error: task '{id}' not found");
            return 1;
        }
        Err(e) => {
            eprintln!("error: {e}");
            return 1;
        }
    };

    let task_id = task.id.clone();
    let reset_task = transition_reset(task);

    match repo.update_task(reset_task) {
        Ok(_) => {
            println!("Task {} reset to pending.", task_id);
            0
        }
        Err(e) => {
            eprintln!("error: {e}");
            1
        }
    }
}

/// Runtime metadata optionally supplied via environment (typically by the agent session).
/// Reads `BP_COMPLETE_*` first, then `LOOP_COMPLETE_*` for backward compatibility.
fn completion_metrics_from_env() -> (Option<i64>, Option<i64>, Option<String>, Option<String>) {
    let input_tokens = env_opt_i64("BP_COMPLETE_INPUT_TOKENS", "LOOP_COMPLETE_INPUT_TOKENS");
    let output_tokens = env_opt_i64("BP_COMPLETE_OUTPUT_TOKENS", "LOOP_COMPLETE_OUTPUT_TOKENS");
    let model = env_opt_string("BP_COMPLETE_MODEL", "LOOP_COMPLETE_MODEL");
    let commit_sha = env_opt_string("BP_COMPLETE_COMMIT_SHA", "LOOP_COMPLETE_COMMIT_SHA");
    (input_tokens, output_tokens, model, commit_sha)
}

fn env_opt_i64(primary: &str, legacy: &str) -> Option<i64> {
    std::env::var(primary)
        .or_else(|_| std::env::var(legacy))
        .ok()
        .and_then(|s| s.trim().parse::<i64>().ok())
}

fn env_opt_string(primary: &str, legacy: &str) -> Option<String> {
    std::env::var(primary)
        .or_else(|_| std::env::var(legacy))
        .ok()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
}

fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_owned()
    } else {
        s.chars().take(max_chars).collect()
    }
}

fn render_summary_text(summary: &crate::summary::RunSummary) -> String {
    let mut out = String::new();
    out.push_str(&summary_headline(summary));
    out.push('\n');

    if let (Some(start), Some(end)) = (summary.wall_start, summary.wall_end) {
        let wall = summary
            .wall_seconds
            .map(fmt_duration_human)
            .unwrap_or_else(|| "—".to_owned());
        out.push_str(&format!(
            "Wall clock:  {} → {} UTC ({wall})\n",
            fmt_ts_summary(start),
            fmt_ts_summary(end)
        ));
    } else {
        out.push_str("Wall clock:  —\n");
    }

    let agent = fmt_duration_human(summary.agent_seconds);
    let overhead = summary
        .overhead_seconds
        .map(|o| format!(" · overhead {o}s"))
        .unwrap_or_default();
    out.push_str(&format!(
        "Agent time:  {agent} ({}s){overhead}\n",
        summary.agent_seconds
    ));

    if summary.any_tokens_recorded {
        let tin = summary
            .total_input_tokens
            .map(fmt_tokens_compact)
            .unwrap_or_else(|| "0".to_owned());
        let tout = summary
            .total_output_tokens
            .map(fmt_tokens_compact)
            .unwrap_or_else(|| "0".to_owned());
        out.push_str(&format!("Tokens:      {tin} in / {tout} out\n"));
    } else {
        out.push_str("Tokens:      — (not recorded)\n");
    }

    if let Some(model) = &summary.model_label {
        out.push_str(&format!("Model:       {model}\n"));
    }

    out.push('\n');
    out.push_str(&format!(
        "{:<5} {:<10} {:<8} {:<11} {}\n",
        "ID", "STATUS", "TIME", "TOKENS", "COMMIT"
    ));

    for task in &summary.tasks {
        let time = task
            .duration_seconds
            .map(fmt_duration_human)
            .unwrap_or_else(|| "—".to_owned());
        let tokens = match (task.input_tokens, task.output_tokens) {
            (Some(i), Some(o)) => format!("{}/{}", fmt_tokens_compact(i), fmt_tokens_compact(o)),
            (Some(i), None) => format!("{}/—", fmt_tokens_compact(i)),
            (None, Some(o)) => format!("—/{}", fmt_tokens_compact(o)),
            (None, None) => "—".to_owned(),
        };
        let commit = task_commit_line(task);
        out.push_str(&format!(
            "{:<5} {:<10} {:<8} {:<11} {}\n",
            task.id, task.status, time, tokens, commit
        ));
    }

    out
}

fn render_summary_json(summary: &crate::summary::RunSummary) -> String {
    use serde_json::{json, Value};

    let wall = match (summary.wall_start, summary.wall_end) {
        (Some(s), Some(e)) => json!({
            "start": fmt_ts_summary(s),
            "end": fmt_ts_summary(e),
            "seconds": summary.wall_seconds,
        }),
        _ => Value::Null,
    };

    let tokens = if summary.any_tokens_recorded {
        json!({
            "input": summary.total_input_tokens,
            "output": summary.total_output_tokens,
        })
    } else {
        Value::Null
    };

    let tasks: Vec<Value> = summary
        .tasks
        .iter()
        .map(|t| {
            json!({
                "id": t.id.to_string(),
                "status": t.status.as_str(),
                "duration_seconds": t.duration_seconds,
                "duration_human": t.duration_seconds.map(fmt_duration_human),
                "input_tokens": t.input_tokens,
                "output_tokens": t.output_tokens,
                "model": t.model,
                "commit": task_commit_line(t),
            })
        })
        .collect();

    let doc = json!({
        "headline": summary_headline(summary),
        "wall_clock": wall,
        "agent_time_seconds": summary.agent_seconds,
        "overhead_seconds": summary.overhead_seconds,
        "tokens": tokens,
        "model": summary.model_label,
        "counts": {
            "complete": summary.counts.complete,
            "failed": summary.counts.failed,
            "pending": summary.counts.pending,
            "running": summary.counts.running,
        },
        "tasks": tasks,
    });

    format!("{doc}\n")
}
