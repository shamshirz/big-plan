use chrono::{DateTime, Utc};
use std::fmt;

// --- TaskId ---

/// Zero-padded task identifier (e.g. "001", "042").
/// Immutable after creation; seq value equals the numeric interpretation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TaskId(String);

impl TaskId {
    pub fn from_seq(seq: u32) -> Self {
        TaskId(format!("{:03}", seq))
    }

    pub fn parse(s: &str) -> Result<Self, DomainError> {
        if s.is_empty() || !s.chars().all(|c| c.is_ascii_digit()) {
            return Err(DomainError::InvalidTaskId(s.to_owned()));
        }
        Ok(TaskId(s.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn seq(&self) -> u32 {
        self.0.parse().expect("TaskId always contains digits")
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

// --- TaskStatus ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Pending,
    Running,
    Complete,
    Failed,
}

impl TaskStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            TaskStatus::Pending => "pending",
            TaskStatus::Running => "running",
            TaskStatus::Complete => "complete",
            TaskStatus::Failed => "failed",
        }
    }

    pub fn parse(s: &str) -> Result<Self, DomainError> {
        match s {
            "pending" => Ok(TaskStatus::Pending),
            "running" => Ok(TaskStatus::Running),
            "complete" => Ok(TaskStatus::Complete),
            "failed" => Ok(TaskStatus::Failed),
            _ => Err(DomainError::InvalidStatus(s.to_owned())),
        }
    }
}

impl fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// --- Task ---

/// Core domain entity representing a unit of work.
#[derive(Debug, Clone)]
pub struct Task {
    pub id: TaskId,
    pub seq: u32,
    pub title: String,
    pub status: TaskStatus,
    pub depends_on: Vec<TaskId>,
    pub description_md: String,
    pub context_md: String,
    pub acceptance_md: String,
    pub completion_notes_md: String,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub duration_seconds: Option<i64>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub model: Option<String>,
    pub commit_sha: Option<String>,
}

impl Task {
    pub fn new(seq: u32, title: String, created_at: DateTime<Utc>) -> Self {
        Task {
            id: TaskId::from_seq(seq),
            seq,
            title,
            status: TaskStatus::Pending,
            depends_on: vec![],
            description_md: String::new(),
            context_md: String::new(),
            acceptance_md: String::new(),
            completion_notes_md: String::new(),
            created_at,
            started_at: None,
            completed_at: None,
            duration_seconds: None,
            input_tokens: None,
            output_tokens: None,
            model: None,
            commit_sha: None,
        }
    }
}

// --- Completion payload ---

/// Data supplied by the agent when marking a task complete.
#[derive(Debug, Clone)]
pub struct CompletionData {
    pub notes: String,
    pub completed_at: DateTime<Utc>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub model: Option<String>,
    pub commit_sha: Option<String>,
}

// --- EventType ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventType {
    Created,
    Started,
    Completed,
    Failed,
    Reset,
}

impl EventType {
    pub fn as_str(self) -> &'static str {
        match self {
            EventType::Created => "created",
            EventType::Started => "started",
            EventType::Completed => "completed",
            EventType::Failed => "failed",
            EventType::Reset => "reset",
        }
    }

    pub fn parse(s: &str) -> Result<Self, DomainError> {
        match s {
            "created" => Ok(EventType::Created),
            "started" => Ok(EventType::Started),
            "completed" => Ok(EventType::Completed),
            "failed" => Ok(EventType::Failed),
            "reset" => Ok(EventType::Reset),
            _ => Err(DomainError::InvalidEventType(s.to_owned())),
        }
    }
}

impl fmt::Display for EventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// --- EventMetadata ---

/// Typed payload carried by each lifecycle event.
#[derive(Debug, Clone)]
pub enum EventMetadata {
    Empty,
    Completed {
        model: Option<String>,
        input_tokens: Option<i64>,
        output_tokens: Option<i64>,
        commit_sha: Option<String>,
    },
    Failed {
        exit_code: i32,
        message: String,
    },
}

// --- Event ---

/// Append-only audit record for a task lifecycle transition.
#[derive(Debug, Clone)]
pub struct Event {
    /// None until persisted to the DB.
    pub id: Option<i64>,
    pub task_id: TaskId,
    pub event_type: EventType,
    pub timestamp: DateTime<Utc>,
    pub metadata: EventMetadata,
}

impl Event {
    pub fn created(task_id: TaskId, timestamp: DateTime<Utc>) -> Self {
        Event {
            id: None,
            task_id,
            event_type: EventType::Created,
            timestamp,
            metadata: EventMetadata::Empty,
        }
    }

    pub fn started(task_id: TaskId, timestamp: DateTime<Utc>) -> Self {
        Event {
            id: None,
            task_id,
            event_type: EventType::Started,
            timestamp,
            metadata: EventMetadata::Empty,
        }
    }

    pub fn completed(task_id: TaskId, timestamp: DateTime<Utc>, data: &CompletionData) -> Self {
        Event {
            id: None,
            task_id,
            event_type: EventType::Completed,
            timestamp,
            metadata: EventMetadata::Completed {
                model: data.model.clone(),
                input_tokens: data.input_tokens,
                output_tokens: data.output_tokens,
                commit_sha: data.commit_sha.clone(),
            },
        }
    }

    pub fn failed(task_id: TaskId, timestamp: DateTime<Utc>, exit_code: i32, message: String) -> Self {
        Event {
            id: None,
            task_id,
            event_type: EventType::Failed,
            timestamp,
            metadata: EventMetadata::Failed { exit_code, message },
        }
    }

    pub fn reset(task_id: TaskId, timestamp: DateTime<Utc>) -> Self {
        Event {
            id: None,
            task_id,
            event_type: EventType::Reset,
            timestamp,
            metadata: EventMetadata::Empty,
        }
    }
}

// --- DomainError ---

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainError {
    InvalidTaskId(String),
    InvalidStatus(String),
    InvalidEventType(String),
    InvalidTransition { from: TaskStatus, to: TaskStatus },
    TitleEmpty,
    AlreadyRunning(String),
    NoRunningTask,
    TaskNotFound(String),
    NotInitialized,
}

impl fmt::Display for DomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DomainError::InvalidTaskId(id) => write!(f, "invalid task id: '{id}'"),
            DomainError::InvalidStatus(s) => write!(f, "invalid status: '{s}'"),
            DomainError::InvalidEventType(s) => write!(f, "invalid event type: '{s}'"),
            DomainError::InvalidTransition { from, to } => {
                write!(f, "invalid transition: {from} -> {to}")
            }
            DomainError::TitleEmpty => write!(f, "title must not be empty"),
            DomainError::AlreadyRunning(id) => write!(
                f,
                "task {id} is already running — complete or reset it before running again"
            ),
            DomainError::NoRunningTask => write!(f, "no task is currently running"),
            DomainError::TaskNotFound(id) => write!(f, "task '{id}' not found"),
            DomainError::NotInitialized => {
                write!(f, "loop not initialized — run `loop init` first")
            }
        }
    }
}

impl std::error::Error for DomainError {}

// --- Validation ---

/// Reject blank or whitespace-only titles.
pub fn validate_title(title: &str) -> Result<(), DomainError> {
    if title.trim().is_empty() {
        Err(DomainError::TitleEmpty)
    } else {
        Ok(())
    }
}

// --- State transitions ---
//
// Each transition is a pure function: it takes ownership of a Task and returns
// either an updated Task or a DomainError. No I/O occurs here.

/// pending → running
pub fn transition_start(task: Task, started_at: DateTime<Utc>) -> Result<Task, DomainError> {
    match task.status {
        TaskStatus::Pending => Ok(Task {
            status: TaskStatus::Running,
            started_at: Some(started_at),
            ..task
        }),
        other => Err(DomainError::InvalidTransition {
            from: other,
            to: TaskStatus::Running,
        }),
    }
}

/// running → complete
///
/// Notes are appended to any existing completion_notes_md, separated by a
/// newline when both sides are non-empty.
pub fn transition_complete(task: Task, data: CompletionData) -> Result<Task, DomainError> {
    match task.status {
        TaskStatus::Running => {
            let duration_seconds = task
                .started_at
                .map(|s| (data.completed_at - s).num_seconds());
            let completion_notes_md = append_notes(&task.completion_notes_md, &data.notes);
            Ok(Task {
                status: TaskStatus::Complete,
                completion_notes_md,
                completed_at: Some(data.completed_at),
                duration_seconds,
                input_tokens: data.input_tokens,
                output_tokens: data.output_tokens,
                model: data.model,
                commit_sha: data.commit_sha,
                ..task
            })
        }
        other => Err(DomainError::InvalidTransition {
            from: other,
            to: TaskStatus::Complete,
        }),
    }
}

/// running → failed
pub fn transition_fail(task: Task, failed_at: DateTime<Utc>) -> Result<Task, DomainError> {
    match task.status {
        TaskStatus::Running => {
            let duration_seconds = task.started_at.map(|s| (failed_at - s).num_seconds());
            Ok(Task {
                status: TaskStatus::Failed,
                completed_at: Some(failed_at),
                duration_seconds,
                ..task
            })
        }
        other => Err(DomainError::InvalidTransition {
            from: other,
            to: TaskStatus::Failed,
        }),
    }
}

/// any → pending  (clears runtime fields; preserves completion_notes_md)
pub fn transition_reset(task: Task) -> Task {
    Task {
        status: TaskStatus::Pending,
        started_at: None,
        completed_at: None,
        duration_seconds: None,
        input_tokens: None,
        output_tokens: None,
        model: None,
        commit_sha: None,
        ..task
    }
}

// --- Collection-level queries and invariants ---

/// Enforces the single-running-task invariant.
/// Returns an error naming the conflicting task if one is already running.
pub fn check_no_running_task(tasks: &[Task]) -> Result<(), DomainError> {
    match tasks.iter().find(|t| t.status == TaskStatus::Running) {
        Some(t) => Err(DomainError::AlreadyRunning(t.id.to_string())),
        None => Ok(()),
    }
}

/// Returns the next pending task in seq order, or None if none exist.
pub fn next_pending(tasks: &[Task]) -> Option<&Task> {
    tasks
        .iter()
        .filter(|t| t.status == TaskStatus::Pending)
        .min_by_key(|t| t.seq)
}

/// Returns the currently running task, or an error if none is running.
pub fn current_running(tasks: &[Task]) -> Result<&Task, DomainError> {
    tasks
        .iter()
        .find(|t| t.status == TaskStatus::Running)
        .ok_or(DomainError::NoRunningTask)
}

// --- Helpers ---

fn append_notes(existing: &str, new_notes: &str) -> String {
    match (existing.is_empty(), new_notes.is_empty()) {
        (_, true) => existing.to_owned(),
        (true, false) => new_notes.to_owned(),
        (false, false) => format!("{existing}\n{new_notes}"),
    }
}

// --- Tests ---

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn ts(y: i32, mo: u32, d: u32, h: u32, min: u32, s: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, mo, d, h, min, s).unwrap()
    }

    fn pending_task() -> Task {
        Task::new(1, "Do something".to_owned(), ts(2026, 5, 7, 10, 0, 0))
    }

    // --- TaskId ---

    #[test]
    fn task_id_from_seq_pads_to_three_digits() {
        assert_eq!(TaskId::from_seq(1).as_str(), "001");
        assert_eq!(TaskId::from_seq(42).as_str(), "042");
        assert_eq!(TaskId::from_seq(100).as_str(), "100");
        assert_eq!(TaskId::from_seq(1000).as_str(), "1000");
    }

    #[test]
    fn task_id_parse_accepts_digit_strings() {
        assert!(TaskId::parse("001").is_ok());
        assert!(TaskId::parse("999").is_ok());
        assert!(TaskId::parse("1").is_ok());
    }

    #[test]
    fn task_id_parse_rejects_non_digits() {
        assert_eq!(TaskId::parse("abc"), Err(DomainError::InvalidTaskId("abc".to_owned())));
        assert_eq!(TaskId::parse(""), Err(DomainError::InvalidTaskId("".to_owned())));
        assert_eq!(TaskId::parse("01a"), Err(DomainError::InvalidTaskId("01a".to_owned())));
    }

    // --- TaskStatus ---

    #[test]
    fn task_status_round_trips() {
        for s in ["pending", "running", "complete", "failed"] {
            let status = TaskStatus::parse(s).unwrap();
            assert_eq!(status.as_str(), s);
        }
    }

    #[test]
    fn task_status_rejects_unknown() {
        assert!(matches!(
            TaskStatus::parse("done"),
            Err(DomainError::InvalidStatus(_))
        ));
    }

    // --- validate_title ---

    #[test]
    fn validate_title_accepts_non_empty() {
        assert!(validate_title("Do something").is_ok());
        assert!(validate_title("  x  ").is_ok());
    }

    #[test]
    fn validate_title_rejects_empty_and_whitespace() {
        assert_eq!(validate_title(""), Err(DomainError::TitleEmpty));
        assert_eq!(validate_title("   "), Err(DomainError::TitleEmpty));
    }

    // --- transition_start ---

    #[test]
    fn start_pending_becomes_running() {
        let task = pending_task();
        let started = ts(2026, 5, 7, 10, 5, 0);
        let task = transition_start(task, started).unwrap();
        assert_eq!(task.status, TaskStatus::Running);
        assert_eq!(task.started_at, Some(started));
    }

    #[test]
    fn start_already_running_fails() {
        let task = pending_task();
        let task = transition_start(task, ts(2026, 5, 7, 10, 5, 0)).unwrap();
        let err = transition_start(task, ts(2026, 5, 7, 10, 6, 0)).unwrap_err();
        assert!(matches!(err, DomainError::InvalidTransition { from: TaskStatus::Running, to: TaskStatus::Running }));
    }

    #[test]
    fn start_complete_fails() {
        let task = pending_task();
        let task = transition_start(task, ts(2026, 5, 7, 10, 0, 0)).unwrap();
        let data = CompletionData {
            notes: String::new(),
            completed_at: ts(2026, 5, 7, 10, 1, 0),
            input_tokens: None,
            output_tokens: None,
            model: None,
            commit_sha: None,
        };
        let task = transition_complete(task, data).unwrap();
        let err = transition_start(task, ts(2026, 5, 7, 10, 2, 0)).unwrap_err();
        assert!(matches!(err, DomainError::InvalidTransition { from: TaskStatus::Complete, to: TaskStatus::Running }));
    }

    // --- transition_complete ---

    #[test]
    fn complete_running_records_duration_and_notes() {
        let task = pending_task();
        let started = ts(2026, 5, 7, 10, 0, 0);
        let task = transition_start(task, started).unwrap();
        let completed = ts(2026, 5, 7, 10, 2, 0); // 120 seconds later
        let data = CompletionData {
            notes: "All done.".to_owned(),
            completed_at: completed,
            input_tokens: Some(100),
            output_tokens: Some(50),
            model: Some("claude-sonnet-4-6".to_owned()),
            commit_sha: Some("abc1234".to_owned()),
        };
        let task = transition_complete(task, data).unwrap();
        assert_eq!(task.status, TaskStatus::Complete);
        assert_eq!(task.duration_seconds, Some(120));
        assert_eq!(task.completion_notes_md, "All done.");
        assert_eq!(task.input_tokens, Some(100));
        assert_eq!(task.model.as_deref(), Some("claude-sonnet-4-6"));
    }

    #[test]
    fn complete_appends_to_existing_notes() {
        let mut task = pending_task();
        task.completion_notes_md = "Prior notes.".to_owned();
        let task = transition_start(task, ts(2026, 5, 7, 10, 0, 0)).unwrap();
        let data = CompletionData {
            notes: "New notes.".to_owned(),
            completed_at: ts(2026, 5, 7, 10, 1, 0),
            input_tokens: None,
            output_tokens: None,
            model: None,
            commit_sha: None,
        };
        let task = transition_complete(task, data).unwrap();
        assert_eq!(task.completion_notes_md, "Prior notes.\nNew notes.");
    }

    #[test]
    fn complete_pending_fails() {
        let task = pending_task();
        let data = CompletionData {
            notes: String::new(),
            completed_at: ts(2026, 5, 7, 10, 1, 0),
            input_tokens: None,
            output_tokens: None,
            model: None,
            commit_sha: None,
        };
        assert!(matches!(
            transition_complete(task, data),
            Err(DomainError::InvalidTransition { from: TaskStatus::Pending, to: TaskStatus::Complete })
        ));
    }

    // --- transition_fail ---

    #[test]
    fn fail_running_records_duration() {
        let task = pending_task();
        let started = ts(2026, 5, 7, 10, 0, 0);
        let task = transition_start(task, started).unwrap();
        let failed_at = ts(2026, 5, 7, 10, 1, 30); // 90 seconds
        let task = transition_fail(task, failed_at).unwrap();
        assert_eq!(task.status, TaskStatus::Failed);
        assert_eq!(task.duration_seconds, Some(90));
    }

    #[test]
    fn fail_pending_fails() {
        let task = pending_task();
        assert!(matches!(
            transition_fail(task, ts(2026, 5, 7, 10, 0, 0)),
            Err(DomainError::InvalidTransition { from: TaskStatus::Pending, to: TaskStatus::Failed })
        ));
    }

    // --- transition_reset ---

    #[test]
    fn reset_clears_runtime_fields_preserves_notes() {
        let task = pending_task();
        let task = transition_start(task, ts(2026, 5, 7, 10, 0, 0)).unwrap();
        let data = CompletionData {
            notes: "Done.".to_owned(),
            completed_at: ts(2026, 5, 7, 10, 1, 0),
            input_tokens: Some(42),
            output_tokens: Some(10),
            model: Some("claude".to_owned()),
            commit_sha: Some("deadbeef".to_owned()),
        };
        let task = transition_complete(task, data).unwrap();
        let task = transition_reset(task);
        assert_eq!(task.status, TaskStatus::Pending);
        assert!(task.started_at.is_none());
        assert!(task.completed_at.is_none());
        assert!(task.duration_seconds.is_none());
        assert!(task.input_tokens.is_none());
        assert!(task.model.is_none());
        assert!(task.commit_sha.is_none());
        assert_eq!(task.completion_notes_md, "Done."); // preserved
    }

    #[test]
    fn reset_valid_from_any_status() {
        for build in [
            pending_task(),
            {
                let t = pending_task();
                transition_start(t, ts(2026, 5, 7, 10, 0, 0)).unwrap()
            },
            {
                let t = pending_task();
                let t = transition_start(t, ts(2026, 5, 7, 10, 0, 0)).unwrap();
                let data = CompletionData {
                    notes: String::new(),
                    completed_at: ts(2026, 5, 7, 10, 1, 0),
                    input_tokens: None,
                    output_tokens: None,
                    model: None,
                    commit_sha: None,
                };
                transition_complete(t, data).unwrap()
            },
            {
                let t = pending_task();
                let t = transition_start(t, ts(2026, 5, 7, 10, 0, 0)).unwrap();
                transition_fail(t, ts(2026, 5, 7, 10, 1, 0)).unwrap()
            },
        ] {
            let reset = transition_reset(build);
            assert_eq!(reset.status, TaskStatus::Pending);
        }
    }

    // --- collection-level helpers ---

    #[test]
    fn check_no_running_task_ok_when_none_running() {
        let tasks = vec![pending_task()];
        assert!(check_no_running_task(&tasks).is_ok());
    }

    #[test]
    fn check_no_running_task_errors_when_one_running() {
        let task = transition_start(pending_task(), ts(2026, 5, 7, 10, 0, 0)).unwrap();
        let err = check_no_running_task(&[task]).unwrap_err();
        assert!(matches!(err, DomainError::AlreadyRunning(_)));
    }

    #[test]
    fn next_pending_returns_lowest_seq() {
        let t1 = Task::new(1, "first".to_owned(), ts(2026, 5, 7, 10, 0, 0));
        let t2 = Task::new(2, "second".to_owned(), ts(2026, 5, 7, 10, 0, 0));
        let tasks = vec![t2, t1]; // out of order
        let next = next_pending(&tasks).unwrap();
        assert_eq!(next.seq, 1);
    }

    #[test]
    fn next_pending_skips_non_pending() {
        let t1 = transition_start(
            Task::new(1, "first".to_owned(), ts(2026, 5, 7, 10, 0, 0)),
            ts(2026, 5, 7, 10, 1, 0),
        )
        .unwrap();
        let t2 = Task::new(2, "second".to_owned(), ts(2026, 5, 7, 10, 0, 0));
        let tasks = [t1, t2];
        let next = next_pending(&tasks).unwrap();
        assert_eq!(next.seq, 2);
    }

    #[test]
    fn next_pending_none_when_all_done() {
        let t = transition_start(
            Task::new(1, "only".to_owned(), ts(2026, 5, 7, 10, 0, 0)),
            ts(2026, 5, 7, 10, 1, 0),
        )
        .unwrap();
        assert!(next_pending(&[t]).is_none());
    }

    #[test]
    fn current_running_finds_running_task() {
        let running = transition_start(pending_task(), ts(2026, 5, 7, 10, 0, 0)).unwrap();
        let tasks = vec![running];
        assert_eq!(current_running(&tasks).unwrap().status, TaskStatus::Running);
    }

    #[test]
    fn current_running_errors_when_none_running() {
        let tasks = vec![pending_task()];
        assert!(matches!(
            current_running(&tasks),
            Err(DomainError::NoRunningTask)
        ));
    }

    // --- EventType ---

    #[test]
    fn event_type_round_trips() {
        for s in ["created", "started", "completed", "failed", "reset"] {
            assert_eq!(EventType::parse(s).unwrap().as_str(), s);
        }
    }

    // --- Event constructors ---

    #[test]
    fn event_constructors_produce_correct_types() {
        let id = TaskId::from_seq(1);
        let now = ts(2026, 5, 7, 10, 0, 0);
        assert_eq!(Event::created(id.clone(), now).event_type, EventType::Created);
        assert_eq!(Event::started(id.clone(), now).event_type, EventType::Started);
        assert_eq!(Event::reset(id.clone(), now).event_type, EventType::Reset);
        let data = CompletionData {
            notes: String::new(),
            completed_at: now,
            input_tokens: None,
            output_tokens: None,
            model: None,
            commit_sha: None,
        };
        assert_eq!(Event::completed(id.clone(), now, &data).event_type, EventType::Completed);
        assert_eq!(
            Event::failed(id, now, 1, "err".to_owned()).event_type,
            EventType::Failed
        );
    }
}
