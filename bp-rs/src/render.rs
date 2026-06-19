use chrono::DateTime;
use chrono::Utc;

use crate::domain::Task;

/// Renders a task as agent-consumable markdown (for `read current` and `read <id>`).
///
/// Empty sections are omitted. Timestamps appear inline with the metadata
/// block so the agent sees the full lifecycle state at a glance.
pub fn render_task_markdown(task: &Task) -> String {
    let mut out = String::new();
    out.push_str(&format!("# Task {}: {}\n\n", task.id, task.title));
    out.push_str(&format!("Status: {}\n", task.status));
    out.push_str(&format!("Created: {}\n", fmt_ts(task.created_at)));
    if let Some(started_at) = task.started_at {
        out.push_str(&format!("Started: {}\n", fmt_ts(started_at)));
    }
    if let Some(completed_at) = task.completed_at {
        out.push_str(&format!("Completed: {}\n", fmt_ts(completed_at)));
    }
    push_md_section(&mut out, "Description", &task.description_md);
    push_md_section(&mut out, "Context", &task.context_md);
    push_md_section(&mut out, "Acceptance criteria", &task.acceptance_md);
    push_md_section(&mut out, "Completion notes", &task.completion_notes_md);
    out
}

/// Renders a task as human-readable key-value detail (for `show <id>`).
///
/// All content sections always appear, showing "(none)" when empty.
/// Runtime metrics are appended at the end only when present.
pub fn render_task_detail(task: &Task) -> String {
    let mut out = String::new();
    out.push_str(&format!("ID:      {}\n", task.id));
    out.push_str(&format!("Title:   {}\n", task.title));
    out.push_str(&format!("Status:  {}\n", task.status));
    out.push_str(&format!("Created: {}\n", fmt_ts(task.created_at)));

    for (label, content) in [
        ("Description", task.description_md.as_str()),
        ("Context", task.context_md.as_str()),
        ("Acceptance criteria", task.acceptance_md.as_str()),
        ("Completion notes", task.completion_notes_md.as_str()),
    ] {
        out.push('\n');
        out.push_str(label);
        out.push_str(":\n");
        if content.is_empty() {
            out.push_str("(none)\n");
        } else {
            out.push_str(content);
            if !content.ends_with('\n') {
                out.push('\n');
            }
        }
    }

    let has_metrics = task.started_at.is_some()
        || task.completed_at.is_some()
        || task.duration_seconds.is_some()
        || task.model.is_some()
        || task.input_tokens.is_some()
        || task.output_tokens.is_some()
        || task.commit_sha.is_some();

    if has_metrics {
        out.push('\n');
        if let Some(started_at) = task.started_at {
            out.push_str(&format!("Started:   {}\n", fmt_ts(started_at)));
        }
        if let Some(completed_at) = task.completed_at {
            out.push_str(&format!("Completed: {}\n", fmt_ts(completed_at)));
        }
        if let Some(duration) = task.duration_seconds {
            out.push_str(&format!("Duration:  {}s\n", duration));
        }
        if let Some(model) = &task.model {
            out.push_str(&format!("Model:     {}\n", model));
        }
        if task.input_tokens.is_some() || task.output_tokens.is_some() {
            out.push_str(&format!(
                "Tokens in/out: {} / {}\n",
                task.input_tokens.unwrap_or(0),
                task.output_tokens.unwrap_or(0)
            ));
        }
        if let Some(sha) = &task.commit_sha {
            out.push_str(&format!("Commit:    {}\n", sha));
        }
    }

    out
}

fn push_md_section(out: &mut String, heading: &str, content: &str) {
    if content.is_empty() {
        return;
    }
    out.push_str(&format!("\n## {heading}\n"));
    out.push_str(content);
    if !content.ends_with('\n') {
        out.push('\n');
    }
}

fn fmt_ts(dt: DateTime<Utc>) -> String {
    dt.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// Wall-clock display for `bp summary` (UTC).
pub fn fmt_ts_summary(dt: DateTime<Utc>) -> String {
    dt.format("%Y-%m-%d %H:%M:%S").to_string()
}

/// Human-readable duration: `5m 46s`, `39m 44s`, `1h 2m 3s`.
pub fn fmt_duration_human(seconds: i64) -> String {
    if seconds < 0 {
        return format!("{seconds}s");
    }
    let h = seconds / 3600;
    let m = (seconds % 3600) / 60;
    let s = seconds % 60;
    if h > 0 {
        format!("{h}h {m}m {s}s")
    } else if m > 0 {
        format!("{m}m {s}s")
    } else {
        format!("{s}s")
    }
}

/// Compact token count: `1.2M`, `340k`, `999`.
pub fn fmt_tokens_compact(n: i64) -> String {
    if n >= 1_000_000 {
        let v = n as f64 / 1_000_000.0;
        if (v - v.round()).abs() < 0.05 {
            format!("{}M", v.round() as i64)
        } else {
            format!("{v:.1}M")
        }
    } else if n >= 1_000 {
        let v = n as f64 / 1_000.0;
        if (v - v.round()).abs() < 0.05 {
            format!("{}k", v.round() as i64)
        } else {
            format!("{v:.1}k")
        }
    } else {
        n.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Task, TaskId, TaskStatus};
    use chrono::TimeZone;

    fn ts(y: i32, mo: u32, d: u32, h: u32, min: u32, s: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, mo, d, h, min, s).unwrap()
    }

    fn minimal_task() -> Task {
        Task::new(1, "My task".to_owned(), ts(2026, 5, 7, 10, 0, 0))
    }

    fn full_task() -> Task {
        Task {
            id: TaskId::from_seq(2),
            seq: 2,
            title: "Full task".to_owned(),
            status: TaskStatus::Complete,
            depends_on: vec![],
            description_md: "Do the thing.".to_owned(),
            context_md: "Some context here.".to_owned(),
            acceptance_md: "- Works.".to_owned(),
            completion_notes_md: "All done.".to_owned(),
            created_at: ts(2026, 5, 7, 10, 0, 0),
            started_at: Some(ts(2026, 5, 7, 10, 5, 0)),
            completed_at: Some(ts(2026, 5, 7, 10, 10, 0)),
            duration_seconds: Some(300),
            input_tokens: Some(100),
            output_tokens: Some(50),
            model: Some("claude-sonnet-4-6".to_owned()),
            commit_sha: Some("abc1234".to_owned()),
        }
    }

    // --- render_task_markdown ---

    #[test]
    fn markdown_minimal_pending_task() {
        let out = render_task_markdown(&minimal_task());
        assert_eq!(
            out,
            "# Task 001: My task\n\nStatus: pending\nCreated: 2026-05-07T10:00:00Z\n"
        );
    }

    #[test]
    fn markdown_full_complete_task() {
        let out = render_task_markdown(&full_task());
        assert_eq!(
            out,
            "# Task 002: Full task\n\n\
             Status: complete\n\
             Created: 2026-05-07T10:00:00Z\n\
             Started: 2026-05-07T10:05:00Z\n\
             Completed: 2026-05-07T10:10:00Z\n\
             \n## Description\n\
             Do the thing.\n\
             \n## Context\n\
             Some context here.\n\
             \n## Acceptance criteria\n\
             - Works.\n\
             \n## Completion notes\n\
             All done.\n"
        );
    }

    #[test]
    fn markdown_omits_empty_sections() {
        let mut task = minimal_task();
        task.description_md = "Has description.".to_owned();
        let out = render_task_markdown(&task);
        assert!(out.contains("## Description"));
        assert!(!out.contains("## Context"));
        assert!(!out.contains("## Acceptance criteria"));
        assert!(!out.contains("## Completion notes"));
    }

    #[test]
    fn markdown_includes_started_when_running() {
        let mut task = minimal_task();
        task.status = TaskStatus::Running;
        task.started_at = Some(ts(2026, 5, 7, 10, 5, 0));
        let out = render_task_markdown(&task);
        assert!(out.contains("Started: 2026-05-07T10:05:00Z\n"));
        assert!(!out.contains("Completed:"));
    }

    #[test]
    fn markdown_content_with_trailing_newline_does_not_double_newline() {
        let mut task = minimal_task();
        task.description_md = "Line one.\n".to_owned();
        let out = render_task_markdown(&task);
        assert!(!out.contains("Line one.\n\n\n"));
        assert!(out.contains("## Description\nLine one.\n"));
    }

    #[test]
    fn markdown_is_deterministic() {
        let task = full_task();
        assert_eq!(render_task_markdown(&task), render_task_markdown(&task));
    }

    // --- render_task_detail ---

    #[test]
    fn detail_minimal_pending_task_shows_all_sections_as_none() {
        let out = render_task_detail(&minimal_task());
        assert_eq!(
            out,
            "ID:      001\n\
             Title:   My task\n\
             Status:  pending\n\
             Created: 2026-05-07T10:00:00Z\n\
             \nDescription:\n(none)\n\
             \nContext:\n(none)\n\
             \nAcceptance criteria:\n(none)\n\
             \nCompletion notes:\n(none)\n"
        );
    }

    #[test]
    fn detail_full_complete_task() {
        let out = render_task_detail(&full_task());
        assert_eq!(
            out,
            "ID:      002\n\
             Title:   Full task\n\
             Status:  complete\n\
             Created: 2026-05-07T10:00:00Z\n\
             \nDescription:\nDo the thing.\n\
             \nContext:\nSome context here.\n\
             \nAcceptance criteria:\n- Works.\n\
             \nCompletion notes:\nAll done.\n\
             \nStarted:   2026-05-07T10:05:00Z\n\
             Completed: 2026-05-07T10:10:00Z\n\
             Duration:  300s\n\
             Model:     claude-sonnet-4-6\n\
             Tokens in/out: 100 / 50\n\
             Commit:    abc1234\n"
        );
    }

    #[test]
    fn detail_omits_metrics_section_when_not_present() {
        let out = render_task_detail(&minimal_task());
        assert!(!out.contains("Started:"));
        assert!(!out.contains("Completed:"));
        assert!(!out.contains("Duration:"));
        assert!(!out.contains("Tokens"));
    }

    #[test]
    fn detail_content_with_trailing_newline_does_not_double_newline() {
        let mut task = minimal_task();
        task.description_md = "Line one.\n".to_owned();
        let out = render_task_detail(&task);
        assert!(!out.contains("Line one.\n\n\n"));
        assert!(out.contains("Line one.\n\nContext:"));
    }

    #[test]
    fn detail_is_deterministic() {
        let task = full_task();
        assert_eq!(render_task_detail(&task), render_task_detail(&task));
    }

    #[test]
    fn detail_partial_tokens_shows_zeros_for_missing_side() {
        let mut task = minimal_task();
        task.started_at = Some(ts(2026, 5, 7, 10, 5, 0));
        task.input_tokens = Some(42);
        let out = render_task_detail(&task);
        assert!(out.contains("Tokens in/out: 42 / 0\n"));
    }

    #[test]
    fn fmt_duration_human_examples() {
        assert_eq!(fmt_duration_human(46), "46s");
        assert_eq!(fmt_duration_human(346), "5m 46s");
        assert_eq!(fmt_duration_human(2384), "39m 44s");
    }

    #[test]
    fn fmt_tokens_compact_examples() {
        assert_eq!(fmt_tokens_compact(999), "999");
        assert_eq!(fmt_tokens_compact(340_000), "340k");
        assert_eq!(fmt_tokens_compact(1_200_000), "1.2M");
    }
}
