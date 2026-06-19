//! Rich `bp status` rendering: progress, digest narrative, and task table.

use chrono::{DateTime, Utc};

use crate::domain::{Goal, Task, TaskStatus};
use crate::render::{fmt_duration_human, fmt_ts_summary, render_progress_bar, truncate};
use crate::summary::RunSummary;

pub struct StatusContext<'a> {
    pub goal: &'a Goal,
    pub tasks: &'a [Task],
    pub summary: &'a RunSummary,
    pub activity: Option<String>,
    /// Override clock for live elapsed (tests). Defaults to `Utc::now()`.
    pub now: Option<DateTime<Utc>>,
}

pub fn render_status(ctx: &StatusContext) -> String {
    let mut out = String::new();
    out.push_str(&render_goal_header(ctx.goal));
    if !ctx.tasks.is_empty() {
        out.push('\n');
        out.push_str(&render_progress_line(ctx.summary));
        out.push_str("\n\n");
        out.push_str(&render_digest(ctx.summary, ctx.tasks, effective_now(ctx)));
        out.push_str("\n\n");
        out.push_str(&render_task_table(ctx.tasks, effective_now(ctx)));
    }
    if let Some(ref activity) = ctx.activity {
        out.push_str(&render_activity_footer(activity));
    } else if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

fn effective_now(ctx: &StatusContext) -> DateTime<Utc> {
    ctx.now.unwrap_or_else(Utc::now)
}

pub fn render_goal_header(goal: &Goal) -> String {
    format!("Goal {} ({}): {}", goal.id, goal.status, goal.title)
}

pub fn render_progress_line(summary: &RunSummary) -> String {
    let c = &summary.counts;
    let total = c.complete + c.failed + c.pending + c.running;
    let mut parts = vec![format!("{}/{} complete", c.complete, total)];
    if c.failed > 0 {
        parts.push(format!("{} failed", c.failed));
    }
    if c.running > 0 {
        parts.push(format!("{} running", c.running));
    }
    if c.pending > 0 {
        parts.push(format!("{} pending", c.pending));
    }
    let counts = parts.join(" · ");
    let bar = render_progress_bar(c.complete, total, 14);
    if bar.is_empty() {
        format!("Progress: {counts}")
    } else {
        let pct = (c.complete * 100) / total;
        format!("Progress: {counts}          {bar} {pct}%")
    }
}

pub fn render_digest(summary: &RunSummary, tasks: &[Task], now: DateTime<Utc>) -> String {
    let in_progress = summary.counts.running > 0 || summary.counts.pending > 0;
    let mut clauses = vec![run_state_clause(summary)];

    if let Some(c) = wall_clock_clause(summary, in_progress, now) {
        clauses.push(c);
    }
    if summary.agent_seconds > 0 {
        clauses.push(format!(
            "agent time {}",
            fmt_duration_human(summary.agent_seconds)
        ));
    }
    if let Some(c) = now_clause(tasks, now) {
        clauses.push(c);
    }
    if let Some(c) = last_finished_clause(tasks) {
        clauses.push(c);
    }
    if let Some(c) = next_clause(tasks) {
        clauses.push(c);
    }
    if let Some(c) = slowest_clause(tasks) {
        clauses.push(c);
    }

    format!("Digest: {}", clauses.join(" · "))
}

pub fn render_task_table(tasks: &[Task], now: DateTime<Utc>) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "  {:<5} {:<10} {:<8} {:<11} {}\n",
        "ID", "STATUS", "TIME", "COMMIT", "TITLE"
    ));
    for task in tasks {
        let time = task_time_column(task, now);
        let commit = crate::summary::task_commit_line(task);
        out.push_str(&format!(
            "  {} {:<3} {:<10} {:<8} {:<11} {}\n",
            status_marker(task.status),
            task.id,
            task.status,
            time,
            commit,
            truncate(&task.title, 48),
        ));
    }
    out
}

pub fn render_activity_footer(activity: &str) -> String {
    format!("\n{activity}\n")
}

fn run_state_clause(summary: &RunSummary) -> String {
    let c = &summary.counts;
    if c.running > 0 || c.pending > 0 {
        "Run in progress".to_owned()
    } else if c.failed > 0 {
        "Run finished with failures".to_owned()
    } else {
        "Run complete".to_owned()
    }
}

fn wall_clock_clause(
    summary: &RunSummary,
    in_progress: bool,
    now: DateTime<Utc>,
) -> Option<String> {
    let start = summary.wall_start?;
    let seconds = if in_progress {
        (now - start).num_seconds()
    } else {
        summary.wall_seconds?
    };
    if seconds < 0 {
        return None;
    }
    Some(format!("started {} ago", fmt_duration_human(seconds)))
}

fn now_clause(tasks: &[Task], now: DateTime<Utc>) -> Option<String> {
    let task = tasks.iter().find(|t| t.status == TaskStatus::Running)?;
    let started = task.started_at?;
    let elapsed = (now - started).num_seconds();
    Some(format!(
        "now on {} \"{}\" ({})",
        task.id,
        truncate(&task.title, 48),
        fmt_duration_human(elapsed)
    ))
}

fn last_finished_clause(tasks: &[Task]) -> Option<String> {
    let task = tasks
        .iter()
        .filter(|t| matches!(t.status, TaskStatus::Complete | TaskStatus::Failed))
        .max_by_key(|t| t.completed_at)?;
    let completed = task.completed_at?;
    let commit_part = task
        .commit_sha
        .as_deref()
        .map(|sha| format!(" (commit {})", &sha[..sha.len().min(7)]))
        .unwrap_or_default();
    Some(format!(
        "Last finished: {} at {} UTC{}",
        task.id,
        fmt_ts_summary(completed),
        commit_part
    ))
}

fn next_clause(tasks: &[Task]) -> Option<String> {
    let task = tasks.iter().find(|t| t.status == TaskStatus::Pending)?;
    Some(format!(
        "Next: {} \"{}\"",
        task.id,
        truncate(&task.title, 48)
    ))
}

fn slowest_clause(tasks: &[Task]) -> Option<String> {
    let completed: Vec<&Task> = tasks
        .iter()
        .filter(|t| t.status == TaskStatus::Complete && t.duration_seconds.is_some())
        .collect();
    if completed.len() < 2 {
        return None;
    }
    let task = completed
        .iter()
        .max_by_key(|t| t.duration_seconds.unwrap())?;
    Some(format!(
        "Slowest: {} ({})",
        task.id,
        fmt_duration_human(task.duration_seconds.unwrap())
    ))
}

pub fn status_marker(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Complete => "✓",
        TaskStatus::Running => "▶",
        TaskStatus::Pending => "·",
        TaskStatus::Failed => "✗",
    }
}

fn task_time_column(task: &Task, now: DateTime<Utc>) -> String {
    match task.status {
        TaskStatus::Running => task
            .started_at
            .map(|s| fmt_duration_human((now - s).num_seconds()))
            .unwrap_or_else(|| "—".to_owned()),
        TaskStatus::Complete | TaskStatus::Failed => task
            .duration_seconds
            .map(fmt_duration_human)
            .unwrap_or_else(|| "—".to_owned()),
        TaskStatus::Pending => "—".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Goal, GoalStatus, TaskKind};
    use chrono::TimeZone;

    fn ts(y: i32, mo: u32, d: u32, h: u32, min: u32, s: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, mo, d, h, min, s).unwrap()
    }

    fn test_goal() -> Goal {
        Goal {
            id: 1,
            title: "Test goal".to_owned(),
            plan_md: String::new(),
            created_at: ts(2026, 6, 1, 0, 0, 0),
            status: GoalStatus::Active,
        }
    }

    fn task(
        seq: u32,
        status: TaskStatus,
        title: &str,
        started: Option<DateTime<Utc>>,
        completed: Option<DateTime<Utc>>,
        duration: Option<i64>,
    ) -> Task {
        let mut t = Task::new(
            seq,
            1,
            TaskKind::Execute,
            title.to_owned(),
            ts(2026, 6, 1, 0, 0, 0),
        );
        t.status = status;
        t.started_at = started;
        t.completed_at = completed;
        t.duration_seconds = duration;
        t
    }

    use crate::summary::{build_summary, SummaryFilter};

    fn ctx<'a>(
        goal: &'a Goal,
        tasks: &'a [Task],
        summary: &'a RunSummary,
        now: Option<DateTime<Utc>>,
    ) -> StatusContext<'a> {
        StatusContext {
            goal,
            tasks,
            summary,
            activity: None,
            now,
        }
    }

    #[test]
    fn empty_queue_shows_goal_only() {
        let goal = test_goal();
        let summary = build_summary(&[], &SummaryFilter::default(), &[]);
        let out = render_status(&StatusContext {
            goal: &goal,
            tasks: &[],
            summary: &summary,
            activity: None,
            now: None,
        });
        assert_eq!(out, "Goal 1 (active): Test goal\n");
    }

    #[test]
    fn all_pending_shows_progress_and_markers() {
        let goal = test_goal();
        let tasks = vec![
            task(1, TaskStatus::Pending, "First", None, None, None),
            task(2, TaskStatus::Pending, "Second", None, None, None),
        ];
        let summary = build_summary(&tasks, &SummaryFilter::default(), &[]);
        let out = render_status(&ctx(&goal, &tasks, &summary, None));
        assert!(out.contains("Progress: 0/2 complete · 2 pending"));
        assert!(out.contains("Digest: Run in progress"));
        assert!(out.contains("Next: 001 \"First\""));
        assert!(out.contains("· 001 pending"));
        assert!(out.contains("· 002 pending"));
    }

    #[test]
    fn mixed_complete_running_pending() {
        let goal = test_goal();
        let now = ts(2026, 6, 17, 10, 4, 0);
        let tasks = vec![
            task(
                1,
                TaskStatus::Complete,
                "Done task",
                Some(ts(2026, 6, 17, 9, 0, 0)),
                Some(ts(2026, 6, 17, 9, 12, 0)),
                Some(720),
            ),
            task(
                2,
                TaskStatus::Running,
                "Active task",
                Some(ts(2026, 6, 17, 10, 0, 0)),
                None,
                None,
            ),
            task(3, TaskStatus::Pending, "Waiting", None, None, None),
        ];
        let mut t1 = tasks[0].clone();
        t1.commit_sha = Some("abc1234".to_owned());
        let tasks = vec![t1, tasks[1].clone(), tasks[2].clone()];
        let summary = build_summary(&tasks, &SummaryFilter::default(), &[]);

        let out = render_status(&ctx(&goal, &tasks, &summary, Some(now)));
        assert!(out.contains("Progress: 1/3 complete · 1 running · 1 pending"));
        assert!(out.contains("[████░░░░░░░░░░] 33%"));
        assert!(out.contains("Digest: Run in progress"));
        assert!(out.contains("now on 002 \"Active task\" (4m 0s)"));
        assert!(out.contains("Last finished: 001"));
        assert!(out.contains("Next: 003 \"Waiting\""));
        assert!(out.contains("✓ 001 complete"));
        assert!(out.contains("▶ 002 running"));
        assert!(out.contains("· 003 pending"));
    }

    #[test]
    fn all_complete_digest() {
        let goal = test_goal();
        let tasks = vec![task(
            1,
            TaskStatus::Complete,
            "Only task",
            Some(ts(2026, 6, 17, 9, 0, 0)),
            Some(ts(2026, 6, 17, 9, 5, 0)),
            Some(300),
        )];
        let summary = build_summary(&tasks, &SummaryFilter::default(), &[]);
        let out = render_status(&ctx(
            &goal,
            &tasks,
            &summary,
            Some(ts(2026, 6, 17, 10, 0, 0)),
        ));
        assert!(out.contains("Progress: 1/1 complete"));
        assert!(out.contains("[██████████████] 100%"));
        assert!(out.contains("Digest: Run complete"));
        assert!(out.contains("started 5m 0s ago"));
        assert!(!out.contains("now on"));
        assert!(!out.contains("Next:"));
    }

    #[test]
    fn failed_tasks_in_progress_line() {
        let goal = test_goal();
        let tasks = vec![
            task(
                1,
                TaskStatus::Complete,
                "ok",
                None,
                Some(ts(2026, 6, 17, 9, 0, 0)),
                Some(60),
            ),
            task(
                2,
                TaskStatus::Failed,
                "bad",
                None,
                Some(ts(2026, 6, 17, 9, 5, 0)),
                Some(30),
            ),
            task(3, TaskStatus::Pending, "retry", None, None, None),
        ];
        let summary = build_summary(&tasks, &SummaryFilter::default(), &[]);
        let out = render_status(&ctx(&goal, &tasks, &summary, None));
        assert!(out.contains("Progress: 1/3 complete · 1 failed · 1 pending"));
        assert!(out.contains("Digest: Run in progress"));
        assert!(out.contains("✗ 002 failed"));
    }

    #[test]
    fn run_finished_with_failures() {
        let goal = test_goal();
        let tasks = vec![
            task(
                1,
                TaskStatus::Complete,
                "ok",
                Some(ts(2026, 6, 17, 9, 0, 0)),
                Some(ts(2026, 6, 17, 9, 5, 0)),
                Some(300),
            ),
            task(
                2,
                TaskStatus::Failed,
                "bad",
                Some(ts(2026, 6, 17, 9, 5, 0)),
                Some(ts(2026, 6, 17, 9, 10, 0)),
                Some(300),
            ),
        ];
        let summary = build_summary(&tasks, &SummaryFilter::default(), &[]);
        let out = render_status(&ctx(
            &goal,
            &tasks,
            &summary,
            Some(ts(2026, 6, 17, 10, 0, 0)),
        ));
        assert!(out.contains("Digest: Run finished with failures"));
    }

    #[test]
    fn progress_bar_edge_cases() {
        assert_eq!(
            render_progress_line(&build_summary(&[], &SummaryFilter::default(), &[])),
            "Progress: 0/0 complete"
        );

        let goal = test_goal();
        let tasks = vec![task(1, TaskStatus::Pending, "solo", None, None, None)];
        let summary = build_summary(&tasks, &SummaryFilter::default(), &[]);
        let out = render_status(&ctx(&goal, &tasks, &summary, None));
        assert!(out.contains("[░░░░░░░░░░░░░░] 0%"));

        let tasks = vec![task(
            1,
            TaskStatus::Complete,
            "solo",
            None,
            None,
            Some(10),
        )];
        let summary = build_summary(&tasks, &SummaryFilter::default(), &[]);
        let out = render_status(&ctx(&goal, &tasks, &summary, None));
        assert!(out.contains("[██████████████] 100%"));
    }

    #[test]
    fn unicode_markers_per_status() {
        assert_eq!(status_marker(TaskStatus::Complete), "✓");
        assert_eq!(status_marker(TaskStatus::Running), "▶");
        assert_eq!(status_marker(TaskStatus::Pending), "·");
        assert_eq!(status_marker(TaskStatus::Failed), "✗");
    }

    #[test]
    fn slowest_shown_when_two_or_more_complete() {
        let goal = test_goal();
        let tasks = vec![
            task(
                1,
                TaskStatus::Complete,
                "fast",
                None,
                Some(ts(2026, 6, 17, 9, 0, 0)),
                Some(60),
            ),
            task(
                2,
                TaskStatus::Complete,
                "slow",
                None,
                Some(ts(2026, 6, 17, 9, 5, 0)),
                Some(600),
            ),
            task(3, TaskStatus::Pending, "next", None, None, None),
        ];
        let summary = build_summary(&tasks, &SummaryFilter::default(), &[]);
        let out = render_status(&ctx(&goal, &tasks, &summary, None));
        assert!(out.contains("Slowest: 002 (10m 0s)"));
    }

    #[test]
    fn activity_footer_appended() {
        let goal = test_goal();
        let tasks = vec![task(1, TaskStatus::Pending, "solo", None, None, None)];
        let summary = build_summary(&tasks, &SummaryFilter::default(), &[]);
        let out = render_status(&StatusContext {
            goal: &goal,
            tasks: &tasks,
            summary: &summary,
            activity: Some("Active bp run: task 001 (pid 42)".to_owned()),
            now: None,
        });
        assert!(out.ends_with("Active bp run: task 001 (pid 42)\n"));
    }

    #[test]
    fn agent_time_in_digest_when_positive() {
        let goal = test_goal();
        let tasks = vec![task(
            1,
            TaskStatus::Complete,
            "done",
            Some(ts(2026, 6, 17, 9, 0, 0)),
            Some(ts(2026, 6, 17, 9, 10, 0)),
            Some(346),
        )];
        let summary = build_summary(&tasks, &SummaryFilter::default(), &[]);
        let out = render_status(&ctx(
            &goal,
            &tasks,
            &summary,
            Some(ts(2026, 6, 17, 10, 0, 0)),
        ));
        assert!(out.contains("agent time 5m 46s"));
    }
}
