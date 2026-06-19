//! Run-level retrospective aggregation for `bp summary`.

use chrono::{DateTime, Utc};

use crate::domain::{Event, Task, TaskId, TaskStatus};

/// Scope filters for which tasks appear in a summary.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SummaryFilter {
    /// Include tasks with `seq >=` this task's seq.
    pub since_seq: Option<u32>,
    /// Include only tasks started after the last queue-idle boundary (see `last_run_boundary`).
    pub last_run: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StatusCounts {
    pub complete: usize,
    pub failed: usize,
    pub pending: usize,
    pub running: usize,
}

#[derive(Debug, Clone)]
pub struct RunSummary {
    pub tasks: Vec<Task>,
    pub counts: StatusCounts,
    pub wall_start: Option<DateTime<Utc>>,
    pub wall_end: Option<DateTime<Utc>>,
    pub wall_seconds: Option<i64>,
    pub agent_seconds: i64,
    pub overhead_seconds: Option<i64>,
    pub total_input_tokens: Option<i64>,
    pub total_output_tokens: Option<i64>,
    pub any_tokens_recorded: bool,
    pub model_label: Option<String>,
}

pub fn build_summary(all_tasks: &[Task], filter: &SummaryFilter, events: &[Event]) -> RunSummary {
    let tasks = filter_tasks(all_tasks, filter, events);
    let counts = count_statuses(&tasks);

    let wall_start = tasks.iter().filter_map(|t| t.started_at).min();
    let wall_end = tasks.iter().filter_map(|t| t.completed_at).max();
    let wall_seconds = match (wall_start, wall_end) {
        (Some(s), Some(e)) if e >= s => Some((e - s).num_seconds()),
        _ => None,
    };

    let agent_seconds: i64 = tasks.iter().filter_map(|t| t.duration_seconds).sum();

    let overhead_seconds = wall_seconds.map(|w| w - agent_seconds);

    let mut total_in: i64 = 0;
    let mut total_out: i64 = 0;
    let mut any_in = false;
    let mut any_out = false;
    for t in &tasks {
        if let Some(n) = t.input_tokens {
            total_in += n;
            any_in = true;
        }
        if let Some(n) = t.output_tokens {
            total_out += n;
            any_out = true;
        }
    }
    let any_tokens_recorded = any_in || any_out;
    let total_input_tokens = if any_in { Some(total_in) } else { None };
    let total_output_tokens = if any_out { Some(total_out) } else { None };

    let model_label = derive_model_label(&tasks);

    RunSummary {
        tasks,
        counts,
        wall_start,
        wall_end,
        wall_seconds,
        agent_seconds,
        overhead_seconds,
        total_input_tokens,
        total_output_tokens,
        any_tokens_recorded,
        model_label,
    }
}

pub fn filter_tasks(all_tasks: &[Task], filter: &SummaryFilter, events: &[Event]) -> Vec<Task> {
    let mut tasks: Vec<Task> = all_tasks.to_vec();
    if filter.since_seq.is_some() || filter.last_run {
        tasks.retain(|t| task_in_scope(t, filter, events, all_tasks));
    }
    tasks
}

fn task_in_scope(task: &Task, filter: &SummaryFilter, events: &[Event], all_tasks: &[Task]) -> bool {
    if let Some(since) = filter.since_seq {
        if task.seq < since {
            return false;
        }
    }
    if filter.last_run {
        let boundary = match last_run_start_boundary(events, all_tasks) {
            None => return false,
            Some(None) => return task.started_at.is_some(),
            Some(Some(b)) => b,
        };
        return task.started_at.map(|s| s > boundary).unwrap_or(false);
    }
    true
}

/// Replay lifecycle events; returns the timestamp after which the most recent run's tasks started.
///
/// Uses the second-to-last "queue idle" moment (all tasks complete/failed). If only one idle
/// moment exists (first run), returns `None` meaning no lower bound.
pub fn last_run_start_boundary(events: &[Event], tasks: &[Task]) -> Option<Option<DateTime<Utc>>> {
    if tasks.is_empty() {
        return None;
    }

    let mut status: std::collections::HashMap<String, TaskStatus> = std::collections::HashMap::new();

    let mut sorted: Vec<&Event> = events.iter().collect();
    sorted.sort_by_key(|e| e.timestamp);

    let mut idle_times: Vec<DateTime<Utc>> = Vec::new();

    for event in sorted {
        apply_event_to_status(&mut status, event);
        if queue_is_idle(&status) {
            idle_times.push(event.timestamp);
        }
    }

    if idle_times.is_empty() {
        return Some(None);
    }
    if idle_times.len() == 1 {
        return Some(None);
    }
    Some(Some(idle_times[idle_times.len() - 2]))
}

fn apply_event_to_status(status: &mut std::collections::HashMap<String, TaskStatus>, event: &Event) {
    let id = event.task_id.as_str().to_owned();
    match event.event_type {
        crate::domain::EventType::Created => {
            status.entry(id).or_insert(TaskStatus::Pending);
        }
        crate::domain::EventType::Started => {
            status.insert(id, TaskStatus::Running);
        }
        crate::domain::EventType::Completed => {
            status.insert(id, TaskStatus::Complete);
        }
        crate::domain::EventType::Failed => {
            status.insert(id, TaskStatus::Failed);
        }
        crate::domain::EventType::Reset => {
            status.insert(id, TaskStatus::Pending);
        }
    }
}

fn queue_is_idle(status: &std::collections::HashMap<String, TaskStatus>) -> bool {
    !status.is_empty()
        && status
            .values()
            .all(|s| matches!(s, TaskStatus::Complete | TaskStatus::Failed))
}

fn count_statuses(tasks: &[Task]) -> StatusCounts {
    let mut counts = StatusCounts::default();
    for t in tasks {
        match t.status {
            TaskStatus::Complete => counts.complete += 1,
            TaskStatus::Failed => counts.failed += 1,
            TaskStatus::Pending => counts.pending += 1,
            TaskStatus::Running => counts.running += 1,
        }
    }
    counts
}

fn derive_model_label(tasks: &[Task]) -> Option<String> {
    let models: Vec<&str> = tasks
        .iter()
        .filter_map(|t| t.model.as_deref())
        .filter(|m| !m.is_empty())
        .collect();
    if models.is_empty() {
        return None;
    }
    let first = models[0];
    if models.iter().all(|m| *m == first) {
        Some(first.to_owned())
    } else {
        Some("mixed".to_owned())
    }
}

/// One-line commit for summary table rows.
pub fn task_commit_line(task: &Task) -> String {
    if let Some(sha) = &task.commit_sha {
        if let Some(subject) = commit_subject_from_notes(&task.completion_notes_md, sha) {
            return format!("{sha} {subject}");
        }
        return sha.clone();
    }
    parse_commit_from_notes(&task.completion_notes_md)
}

fn commit_subject_from_notes(notes: &str, sha: &str) -> Option<String> {
    for line in notes.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("Commit:") {
            let rest = rest.trim();
            if rest.is_empty() || rest == "—" {
                continue;
            }
            if let Some(after_sha) = rest.strip_prefix(sha) {
                let subject = after_sha.trim();
                if !subject.is_empty() {
                    return Some(subject.to_owned());
                }
            }
            if !rest.starts_with(sha) {
                return Some(rest.to_owned());
            }
        }
    }
    None
}

fn parse_commit_from_notes(notes: &str) -> String {
    for line in notes.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("Commit:") {
            let rest = rest.trim();
            if !rest.is_empty() && rest != "—" {
                return rest.to_owned();
            }
        }
    }
    "—".to_owned()
}

pub fn since_seq_from_id(id: &str) -> Result<u32, String> {
    TaskId::parse(id)
        .map(|tid| tid.seq())
        .map_err(|e| e.to_string())
}

pub fn summary_headline(summary: &RunSummary) -> String {
    let n = summary.tasks.len();
    let c = &summary.counts;
    if n == 0 {
        return "Run summary (no tasks in scope)".to_owned();
    }
    if c.running > 0 || c.pending > 0 {
        format!(
            "Run summary ({n} tasks: {} complete, {} failed, {} pending, {} running)",
            c.complete, c.failed, c.pending, c.running
        )
    } else if c.failed > 0 {
        format!(
            "Run summary ({n} tasks: {} complete, {} failed)",
            c.complete, c.failed
        )
    } else {
        format!("Run summary ({n} tasks complete)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn ts(y: i32, mo: u32, d: u32, h: u32, min: u32, s: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, mo, d, h, min, s).unwrap()
    }

    fn task(
        seq: u32,
        status: TaskStatus,
        started: Option<DateTime<Utc>>,
        completed: Option<DateTime<Utc>>,
        duration: Option<i64>,
    ) -> Task {
        let mut t = Task::new(seq, format!("task {seq}"), ts(2026, 6, 1, 0, 0, 0));
        t.status = status;
        t.started_at = started;
        t.completed_at = completed;
        t.duration_seconds = duration;
        t
    }

    #[test]
    fn empty_queue_summary() {
        let s = build_summary(&[], &SummaryFilter::default(), &[]);
        assert_eq!(s.tasks.len(), 0);
        assert_eq!(s.agent_seconds, 0);
        assert!(!s.any_tokens_recorded);
    }

    #[test]
    fn aggregates_wall_clock_and_agent_time() {
        let t1 = task(
            1,
            TaskStatus::Complete,
            Some(ts(2026, 6, 17, 2, 21, 29)),
            Some(ts(2026, 6, 17, 2, 30, 0)),
            Some(100),
        );
        let t2 = task(
            2,
            TaskStatus::Complete,
            Some(ts(2026, 6, 17, 2, 25, 0)),
            Some(ts(2026, 6, 17, 3, 1, 13)),
            Some(200),
        );
        let s = build_summary(&[t1, t2], &SummaryFilter::default(), &[]);
        assert_eq!(s.wall_start, Some(ts(2026, 6, 17, 2, 21, 29)));
        assert_eq!(s.wall_end, Some(ts(2026, 6, 17, 3, 1, 13)));
        assert_eq!(s.agent_seconds, 300);
        assert_eq!(s.overhead_seconds, Some(s.wall_seconds.unwrap() - 300));
    }

    #[test]
    fn partial_tokens_only_sum_recorded() {
        let mut t1 = task(1, TaskStatus::Complete, None, None, Some(10));
        t1.input_tokens = Some(100);
        let t2 = task(2, TaskStatus::Complete, None, None, Some(20));
        let s = build_summary(&[t1, t2], &SummaryFilter::default(), &[]);
        assert!(s.any_tokens_recorded);
        assert_eq!(s.total_input_tokens, Some(100));
        assert_eq!(s.total_output_tokens, None);
    }

    #[test]
    fn since_filter_by_seq() {
        let tasks = vec![
            task(1, TaskStatus::Complete, None, None, None),
            task(2, TaskStatus::Complete, None, None, None),
            task(3, TaskStatus::Pending, None, None, None),
        ];
        let filter = SummaryFilter {
            since_seq: Some(2),
            last_run: false,
        };
        let s = build_summary(&tasks, &filter, &[]);
        assert_eq!(s.tasks.len(), 2);
        assert_eq!(s.tasks[0].seq, 2);
    }

    #[test]
    fn commit_line_from_sha_and_notes() {
        let mut t = Task::new(1, "x".to_owned(), ts(2026, 1, 1, 0, 0, 0));
        t.commit_sha = Some("abc1234".to_owned());
        t.completion_notes_md = "Commit: abc1234 decompose build plan into bp queue".to_owned();
        assert_eq!(
            task_commit_line(&t),
            "abc1234 decompose build plan into bp queue"
        );
    }

    #[test]
    fn commit_line_parsed_from_notes_only() {
        let mut t = Task::new(1, "x".to_owned(), ts(2026, 1, 1, 0, 0, 0));
        t.completion_notes_md = "Commit: f6d751c scaffold Phoenix app".to_owned();
        assert_eq!(task_commit_line(&t), "f6d751c scaffold Phoenix app");
    }

    #[test]
    fn commit_line_dash_when_missing() {
        let t = Task::new(1, "x".to_owned(), ts(2026, 1, 1, 0, 0, 0));
        assert_eq!(task_commit_line(&t), "—");
    }

    #[test]
    fn last_run_includes_first_run_tasks() {
        let t1 = task(
            1,
            TaskStatus::Complete,
            Some(ts(2026, 6, 1, 10, 0, 0)),
            Some(ts(2026, 6, 1, 10, 5, 0)),
            Some(300),
        );
        let t2 = task(
            2,
            TaskStatus::Complete,
            Some(ts(2026, 6, 1, 10, 6, 0)),
            Some(ts(2026, 6, 1, 11, 5, 0)),
            Some(300),
        );
        let events = vec![
            crate::domain::Event::created(TaskId::from_seq(1), ts(2026, 6, 1, 9, 0, 0)),
            crate::domain::Event::created(TaskId::from_seq(2), ts(2026, 6, 1, 9, 0, 0)),
            crate::domain::Event::started(TaskId::from_seq(1), ts(2026, 6, 1, 10, 0, 0)),
            crate::domain::Event::completed(
                TaskId::from_seq(1),
                ts(2026, 6, 1, 10, 5, 0),
                &crate::domain::CompletionData {
                    notes: String::new(),
                    completed_at: ts(2026, 6, 1, 10, 5, 0),
                    input_tokens: None,
                    output_tokens: None,
                    model: None,
                    commit_sha: None,
                },
            ),
            crate::domain::Event::started(TaskId::from_seq(2), ts(2026, 6, 1, 10, 6, 0)),
            crate::domain::Event::completed(
                TaskId::from_seq(2),
                ts(2026, 6, 1, 11, 5, 0),
                &crate::domain::CompletionData {
                    notes: String::new(),
                    completed_at: ts(2026, 6, 1, 11, 5, 0),
                    input_tokens: None,
                    output_tokens: None,
                    model: None,
                    commit_sha: None,
                },
            ),
        ];
        let filter = SummaryFilter {
            since_seq: None,
            last_run: true,
        };
        let s = build_summary(&[t1.clone(), t2.clone()], &filter, &events);
        assert_eq!(s.tasks.len(), 2);

        let t3 = task(
            3,
            TaskStatus::Complete,
            Some(ts(2026, 6, 1, 12, 0, 0)),
            Some(ts(2026, 6, 1, 12, 10, 0)),
            Some(600),
        );
        let mut events2 = events;
        events2.push(crate::domain::Event::created(
            TaskId::from_seq(3),
            ts(2026, 6, 1, 11, 30, 0),
        ));
        events2.push(crate::domain::Event::started(
            TaskId::from_seq(3),
            ts(2026, 6, 1, 12, 0, 0),
        ));
        events2.push(crate::domain::Event::completed(
            TaskId::from_seq(3),
            ts(2026, 6, 1, 12, 10, 0),
            &crate::domain::CompletionData {
                notes: String::new(),
                completed_at: ts(2026, 6, 1, 12, 10, 0),
                input_tokens: None,
                output_tokens: None,
                model: None,
                commit_sha: None,
            },
        ));
        let s2 = build_summary(&[t1, t2, t3], &filter, &events2);
        assert_eq!(s2.tasks.len(), 1);
        assert_eq!(s2.tasks[0].seq, 3);
    }
}
