use std::path::Path;

use chrono::Utc;

use crate::domain::{
    current_running, transition_complete, transition_reset, CompletionData, DomainError,
};
use crate::render::{render_task_detail, render_task_markdown};
use crate::repository::{LoopError, TaskRepository};

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
           read plan|current|<id>  Print planning or task text for agent use\n\
           run                   Execute pending tasks sequentially via agent sessions\n\
           complete [--notes \"\"] Mark the current task complete\n\
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
    match repo.list_tasks() {
        Ok(tasks) if tasks.is_empty() => {
            println!("No tasks. Run `bp add \"<title>\"` to create one.");
            0
        }
        Ok(tasks) => {
            println!("{:<5} {:<10} {}", "ID", "STATUS", "TITLE");
            for task in &tasks {
                println!(
                    "{:<5} {:<10} {}",
                    task.id,
                    task.status,
                    truncate(&task.title, 60)
                );
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

pub fn run(repo: &dyn TaskRepository, project_root: &Path) -> i32 {
    crate::orchestrator::execute_run(repo, project_root)
}

pub fn complete(repo: &dyn TaskRepository, notes: Option<&str>) -> i32 {
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
