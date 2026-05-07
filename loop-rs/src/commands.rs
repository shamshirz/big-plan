use chrono::Utc;

use crate::domain::{
    check_no_running_task, current_running, next_pending, transition_complete, transition_reset,
    CompletionData, DomainError,
};
use crate::repository::{LoopError, TaskRepository};

pub fn help() -> i32 {
    println!(
        "loop — project-local task orchestration\n\
         \n\
         Usage:\n\
           loop <command> [args]\n\
         \n\
         Commands:\n\
           init                  Initialize loop state in the current directory\n\
           add \"<title>\"         Add a new pending task\n\
           status                List all tasks with ID, status, and title\n\
           show <id>             Print full task detail\n\
           read plan|current|<id>  Print planning or task text for agent use\n\
           run                   Execute pending tasks sequentially via agent sessions\n\
           complete [--notes \"\"] Mark the current task complete\n\
           reset <id>            Return a task to pending and clear metrics\n\
         \n\
         Run `loop <command> -h` for command-specific help."
    );
    0
}

pub fn init(repo: &dyn TaskRepository) -> i32 {
    match repo.initialize() {
        Ok(()) => {
            println!("Initialized loop state in .loop/");
            0
        }
        Err(LoopError::AlreadyInitialized) => {
            println!("Loop already initialized in .loop/");
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
            eprintln!("error: loop not initialized — run `loop init` first");
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
            println!("No tasks. Run `loop add \"<title>\"` to create one.");
            0
        }
        Ok(tasks) => {
            println!("{:<5} {:<10} {}", "ID", "STATUS", "TITLE");
            for task in &tasks {
                println!("{:<5} {:<10} {}", task.id, task.status, truncate(&task.title, 60));
            }
            0
        }
        Err(LoopError::NotInitialized) => {
            eprintln!("error: loop not initialized — run `loop init` first");
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
            println!("ID:      {}", task.id);
            println!("Title:   {}", task.title);
            println!("Status:  {}", task.status);
            println!("Created: {}", task.created_at.format("%Y-%m-%dT%H:%M:%SZ"));
            println!();
            println!("Description:");
            println!(
                "{}",
                if task.description_md.is_empty() { "(none)" } else { &task.description_md }
            );
            println!();
            println!("Context:");
            println!(
                "{}",
                if task.context_md.is_empty() { "(none)" } else { &task.context_md }
            );
            println!();
            println!("Acceptance criteria:");
            println!(
                "{}",
                if task.acceptance_md.is_empty() { "(none)" } else { &task.acceptance_md }
            );
            println!();
            println!("Completion notes:");
            println!(
                "{}",
                if task.completion_notes_md.is_empty() {
                    "(none)"
                } else {
                    &task.completion_notes_md
                }
            );
            // Runtime metrics appended only when present
            if let Some(started_at) = task.started_at {
                println!("Started:   {}", started_at.format("%Y-%m-%dT%H:%M:%SZ"));
            }
            if let Some(completed_at) = task.completed_at {
                println!("Completed: {}", completed_at.format("%Y-%m-%dT%H:%M:%SZ"));
            }
            if let Some(duration) = task.duration_seconds {
                println!("Duration:  {}s", duration);
            }
            if let Some(model) = &task.model {
                println!("Model:     {}", model);
            }
            if task.input_tokens.is_some() || task.output_tokens.is_some() {
                println!(
                    "Tokens in/out: {} / {}",
                    task.input_tokens.unwrap_or(0),
                    task.output_tokens.unwrap_or(0)
                );
            }
            if let Some(sha) = &task.commit_sha {
                println!("Commit:    {}", sha);
            }
            0
        }
        Err(LoopError::NotInitialized) => {
            eprintln!("error: loop not initialized — run `loop init` first");
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
            eprintln!("error: loop not initialized — run `loop init` first");
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
            eprintln!("error: loop not initialized — run `loop init` first");
            return 1;
        }
        Err(e) => {
            eprintln!("error: {e}");
            return 1;
        }
    };
    match current_running(&tasks) {
        Ok(task) => {
            print!("{}", format_task_markdown(task));
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
            print!("{}", format_task_markdown(&task));
            0
        }
        Err(LoopError::NotInitialized) => {
            eprintln!("error: loop not initialized — run `loop init` first");
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

pub fn run(repo: &dyn TaskRepository) -> i32 {
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

    match next_pending(&tasks) {
        None => {
            println!("No pending tasks.");
            0
        }
        Some(task) => {
            println!("Running task {}: {}", task.id, task.title);
            // Agent subprocess invocation is handled by the run orchestrator (future task).
            eprintln!("error: agent subprocess invocation not yet implemented");
            1
        }
    }
}

pub fn complete(repo: &dyn TaskRepository, notes: Option<&str>) -> i32 {
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
    let data = CompletionData {
        notes: notes.unwrap_or("").to_owned(),
        completed_at: Utc::now(),
        input_tokens: None,
        output_tokens: None,
        model: None,
        commit_sha: None,
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
            eprintln!("error: loop not initialized — run `loop init` first");
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

fn format_task_markdown(task: &crate::domain::Task) -> String {
    let mut out = String::new();
    out.push_str(&format!("# Task {}: {}\n\n", task.id, task.title));
    out.push_str(&format!("Status: {}\n", task.status));
    out.push_str(&format!(
        "Created: {}\n",
        task.created_at.format("%Y-%m-%dT%H:%M:%SZ")
    ));
    if !task.description_md.is_empty() {
        out.push_str("\n## Description\n");
        out.push_str(&task.description_md);
        out.push('\n');
    }
    if !task.context_md.is_empty() {
        out.push_str("\n## Context\n");
        out.push_str(&task.context_md);
        out.push('\n');
    }
    if !task.acceptance_md.is_empty() {
        out.push_str("\n## Acceptance criteria\n");
        out.push_str(&task.acceptance_md);
        out.push('\n');
    }
    if !task.completion_notes_md.is_empty() {
        out.push_str("\n## Completion notes\n");
        out.push_str(&task.completion_notes_md);
        out.push('\n');
    }
    out
}

fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_owned()
    } else {
        s.chars().take(max_chars).collect()
    }
}
