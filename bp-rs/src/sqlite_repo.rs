use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};

use crate::domain::{validate_title, EventType, Task, TaskId, TaskStatus};
use crate::repository::{LoopError, TaskRepository};

const LATEST_VERSION: i32 = 1;

const MIGRATIONS: &[(i32, &str)] = &[(
    1,
    "CREATE TABLE tasks (
        id                  TEXT    NOT NULL PRIMARY KEY,
        seq                 INTEGER NOT NULL UNIQUE,
        title               TEXT    NOT NULL,
        status              TEXT    NOT NULL DEFAULT 'pending'
                                CHECK(status IN ('pending','running','complete','failed')),
        depends_on          TEXT    NOT NULL DEFAULT '[]',
        description_md      TEXT    NOT NULL DEFAULT '',
        context_md          TEXT    NOT NULL DEFAULT '',
        acceptance_md       TEXT    NOT NULL DEFAULT '',
        completion_notes_md TEXT    NOT NULL DEFAULT '',
        created_at          TEXT    NOT NULL,
        started_at          TEXT,
        completed_at        TEXT,
        duration_seconds    INTEGER,
        input_tokens        INTEGER,
        output_tokens       INTEGER,
        model               TEXT,
        commit_sha          TEXT
    );
    CREATE TABLE events (
        id            INTEGER PRIMARY KEY AUTOINCREMENT,
        task_id       TEXT    NOT NULL REFERENCES tasks(id),
        event_type    TEXT    NOT NULL
                          CHECK(event_type IN ('created','started','completed','failed','reset')),
        timestamp     TEXT    NOT NULL,
        metadata_json TEXT    NOT NULL DEFAULT '{}'
    );
    CREATE TABLE config (
        key   TEXT NOT NULL PRIMARY KEY,
        value TEXT NOT NULL DEFAULT ''
    );
    CREATE INDEX idx_tasks_status ON tasks(status);
    CREATE INDEX idx_events_task_id ON events(task_id);",
)];

pub struct SqliteRepository {
    loop_dir: PathBuf,
    db_path: PathBuf,
}

impl SqliteRepository {
    pub fn new(project_root: &Path) -> Self {
        let loop_dir = project_root.join(".loop");
        let db_path = loop_dir.join("loop.db");
        Self { loop_dir, db_path }
    }

    fn open(&self) -> Result<Connection, LoopError> {
        if !self.db_path.exists() {
            return Err(LoopError::NotInitialized);
        }
        Connection::open(&self.db_path).map_err(|e| LoopError::Io(e.to_string()))
    }

    // Returns true if any migrations were applied, false if already up to date.
    pub(crate) fn apply_migrations(conn: &mut Connection) -> Result<bool, LoopError> {
        let current: i32 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(|e| LoopError::Io(e.to_string()))?;

        if current >= LATEST_VERSION {
            return Ok(false);
        }

        for &(version, sql) in MIGRATIONS {
            if version > current {
                let tx = conn
                    .transaction()
                    .map_err(|e| LoopError::Io(e.to_string()))?;
                tx.execute_batch(sql)
                    .map_err(|e| LoopError::Io(format!("migration {version} failed: {e}")))?;
                tx.commit().map_err(|e| LoopError::Io(e.to_string()))?;
                conn.pragma_update(None, "user_version", &version)
                    .map_err(|e| LoopError::Io(e.to_string()))?;
            }
        }
        Ok(true)
    }
}

impl TaskRepository for SqliteRepository {
    fn initialize(&self) -> Result<(), LoopError> {
        std::fs::create_dir_all(&self.loop_dir).map_err(|e| {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                LoopError::PermissionDenied(self.loop_dir.display().to_string())
            } else {
                LoopError::Io(e.to_string())
            }
        })?;

        let already_existed = self.db_path.exists();
        let mut conn = Connection::open(&self.db_path).map_err(|e| LoopError::Io(e.to_string()))?;

        let applied = Self::apply_migrations(&mut conn)?;

        if already_existed && !applied {
            return Err(LoopError::AlreadyInitialized);
        }

        conn.execute_batch(
            "INSERT OR IGNORE INTO config (key, value) VALUES ('schema_version_tag', 'v1');
             INSERT OR IGNORE INTO config (key, value) VALUES ('project_name', '');",
        )
        .map_err(|e| LoopError::Io(e.to_string()))?;

        let plan_path = self.loop_dir.join("plan.md");
        if !plan_path.exists() {
            std::fs::write(&plan_path, "# Plan\n\n<!-- Add your plan here -->\n")
                .map_err(|e| LoopError::Io(e.to_string()))?;
        }

        let agent_path = self.loop_dir.join("agent-project.md");
        if !agent_path.exists() {
            std::fs::write(
                &agent_path,
                "# Project Context\n\n<!-- Add project context for agents -->\n",
            )
            .map_err(|e| LoopError::Io(e.to_string()))?;
        }

        Ok(())
    }

    fn add_task(&self, title: &str) -> Result<Task, LoopError> {
        validate_title(title).map_err(|e| LoopError::Io(e.to_string()))?;

        let mut conn = self.open()?;
        let now = Utc::now();
        let now_str = format_ts(now);

        let tx = conn
            .transaction()
            .map_err(|e| LoopError::Io(e.to_string()))?;

        let max_seq: i64 = tx
            .query_row("SELECT COALESCE(MAX(seq), 0) FROM tasks", [], |row| {
                row.get(0)
            })
            .map_err(|e| LoopError::Io(e.to_string()))?;

        let next_seq = (max_seq + 1) as u32;
        let id = TaskId::from_seq(next_seq);

        tx.execute(
            "INSERT INTO tasks (id, seq, title, status, depends_on, created_at) \
             VALUES (?1, ?2, ?3, 'pending', '[]', ?4)",
            params![id.as_str(), next_seq as i64, title, &now_str],
        )
        .map_err(|e| LoopError::Io(e.to_string()))?;

        tx.execute(
            "INSERT INTO events (task_id, event_type, timestamp, metadata_json) \
             VALUES (?1, 'created', ?2, ?3)",
            params![id.as_str(), &now_str, "{}"],
        )
        .map_err(|e| LoopError::Io(e.to_string()))?;

        tx.commit().map_err(|e| LoopError::Io(e.to_string()))?;

        Ok(Task::new(next_seq, title.to_owned(), now))
    }

    fn list_tasks(&self) -> Result<Vec<Task>, LoopError> {
        let conn = self.open()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, seq, title, status, depends_on, description_md, context_md, \
                 acceptance_md, completion_notes_md, created_at, started_at, completed_at, \
                 duration_seconds, input_tokens, output_tokens, model, commit_sha \
                 FROM tasks ORDER BY seq ASC",
            )
            .map_err(|e| LoopError::Io(e.to_string()))?;

        let mut rows = stmt.query([]).map_err(|e| LoopError::Io(e.to_string()))?;
        let mut tasks = Vec::new();
        while let Some(row) = rows.next().map_err(|e| LoopError::Io(e.to_string()))? {
            tasks.push(row_to_task(row).map_err(|e| LoopError::Io(e.to_string()))?);
        }
        Ok(tasks)
    }

    fn get_task(&self, id: &str) -> Result<Task, LoopError> {
        let task_id = TaskId::parse(id).map_err(|_| LoopError::TaskNotFound(id.to_owned()))?;
        let conn = self.open()?;
        conn.query_row(
            "SELECT id, seq, title, status, depends_on, description_md, context_md, \
             acceptance_md, completion_notes_md, created_at, started_at, completed_at, \
             duration_seconds, input_tokens, output_tokens, model, commit_sha \
             FROM tasks WHERE id = ?1",
            params![task_id.as_str()],
            row_to_task,
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => LoopError::TaskNotFound(id.to_owned()),
            e => LoopError::Io(e.to_string()),
        })
    }

    fn update_task(&self, task: Task) -> Result<Task, LoopError> {
        let mut conn = self.open()?;
        let now = Utc::now();

        let tx = conn
            .transaction()
            .map_err(|e| LoopError::Io(e.to_string()))?;

        let old_status_str: String = tx
            .query_row(
                "SELECT status FROM tasks WHERE id = ?1",
                params![task.id.as_str()],
                |row| row.get(0),
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    LoopError::TaskNotFound(task.id.to_string())
                }
                e => LoopError::Io(e.to_string()),
            })?;

        let old_status =
            TaskStatus::parse(&old_status_str).map_err(|e| LoopError::Io(e.to_string()))?;
        let event_type = infer_event_type(old_status, task.status);

        tx.execute(
            "UPDATE tasks SET status=?1, started_at=?2, completed_at=?3, \
             duration_seconds=?4, input_tokens=?5, output_tokens=?6, \
             model=?7, commit_sha=?8, completion_notes_md=?9 \
             WHERE id=?10",
            params![
                task.status.as_str(),
                task.started_at.map(format_ts),
                task.completed_at.map(format_ts),
                task.duration_seconds,
                task.input_tokens,
                task.output_tokens,
                task.model.as_deref(),
                task.commit_sha.as_deref(),
                task.completion_notes_md.as_str(),
                task.id.as_str(),
            ],
        )
        .map_err(|e| LoopError::Io(e.to_string()))?;

        if let Some(ev_type) = event_type {
            tx.execute(
                "INSERT INTO events (task_id, event_type, timestamp, metadata_json) \
                 VALUES (?1, ?2, ?3, ?4)",
                params![task.id.as_str(), ev_type.as_str(), format_ts(now), "{}"],
            )
            .map_err(|e| LoopError::Io(e.to_string()))?;
        }

        tx.commit().map_err(|e| LoopError::Io(e.to_string()))?;
        Ok(task)
    }

    fn read_plan(&self) -> Result<String, LoopError> {
        if !self.db_path.exists() {
            return Err(LoopError::NotInitialized);
        }
        let plan_path = self.loop_dir.join("plan.md");
        std::fs::read_to_string(&plan_path).map_err(|e| LoopError::Io(e.to_string()))
    }

    fn read_agent_project(&self) -> Result<String, LoopError> {
        if !self.db_path.exists() {
            return Err(LoopError::NotInitialized);
        }
        let path = self.loop_dir.join("agent-project.md");
        std::fs::read_to_string(&path).map_err(|e| LoopError::Io(e.to_string()))
    }
}

// --- Helpers ---

fn format_ts(dt: DateTime<Utc>) -> String {
    dt.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

fn parse_ts(s: &str) -> DateTime<Utc> {
    s.parse::<DateTime<Utc>>().unwrap_or_else(|_| Utc::now())
}

fn parse_depends_on(json: &str) -> Vec<TaskId> {
    let trimmed = json.trim();
    if trimmed == "[]" {
        return vec![];
    }
    trimmed
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .filter_map(|s| {
            let s = s.trim().trim_matches('"');
            if s.is_empty() {
                None
            } else {
                TaskId::parse(s).ok()
            }
        })
        .collect()
}

fn infer_event_type(from: TaskStatus, to: TaskStatus) -> Option<EventType> {
    match (from, to) {
        (TaskStatus::Pending, TaskStatus::Running) => Some(EventType::Started),
        (TaskStatus::Running, TaskStatus::Complete) => Some(EventType::Completed),
        (TaskStatus::Running, TaskStatus::Failed) => Some(EventType::Failed),
        (_, TaskStatus::Pending) => Some(EventType::Reset),
        _ => None,
    }
}

fn row_to_task(row: &rusqlite::Row<'_>) -> rusqlite::Result<Task> {
    let id_str: String = row.get("id")?;
    let seq: i64 = row.get("seq")?;
    let title: String = row.get("title")?;
    let status_str: String = row.get("status")?;
    let depends_on_json: String = row.get("depends_on")?;
    let description_md: String = row.get("description_md")?;
    let context_md: String = row.get("context_md")?;
    let acceptance_md: String = row.get("acceptance_md")?;
    let completion_notes_md: String = row.get("completion_notes_md")?;
    let created_at_str: String = row.get("created_at")?;
    let started_at_str: Option<String> = row.get("started_at")?;
    let completed_at_str: Option<String> = row.get("completed_at")?;
    let duration_seconds: Option<i64> = row.get("duration_seconds")?;
    let input_tokens: Option<i64> = row.get("input_tokens")?;
    let output_tokens: Option<i64> = row.get("output_tokens")?;
    let model: Option<String> = row.get("model")?;
    let commit_sha: Option<String> = row.get("commit_sha")?;

    let id =
        TaskId::parse(&id_str).map_err(|e| rusqlite::Error::InvalidColumnName(e.to_string()))?;
    let status = TaskStatus::parse(&status_str)
        .map_err(|e| rusqlite::Error::InvalidColumnName(e.to_string()))?;
    let depends_on = parse_depends_on(&depends_on_json);
    let created_at = parse_ts(&created_at_str);
    let started_at = started_at_str.as_deref().map(parse_ts);
    let completed_at = completed_at_str.as_deref().map(parse_ts);

    Ok(Task {
        id,
        seq: seq as u32,
        title,
        status,
        depends_on,
        description_md,
        context_md,
        acceptance_md,
        completion_notes_md,
        created_at,
        started_at,
        completed_at,
        duration_seconds,
        input_tokens,
        output_tokens,
        model,
        commit_sha,
    })
}

// --- Tests ---

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{transition_complete, transition_reset, transition_start, CompletionData};
    use rusqlite::Connection;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_repo() -> (SqliteRepository, PathBuf) {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let pid = std::process::id();
        let root = std::env::temp_dir().join(format!("loop_sqlite_test_{pid}_{ns}_{n}"));
        std::fs::create_dir_all(&root).unwrap();
        let repo = SqliteRepository::new(&root);
        (repo, root)
    }

    fn init_repo() -> (SqliteRepository, PathBuf) {
        let (repo, root) = temp_repo();
        repo.initialize().unwrap();
        (repo, root)
    }

    #[test]
    fn initialize_creates_db_and_template_files() {
        let (repo, root) = temp_repo();
        repo.initialize().unwrap();
        assert!(root.join(".loop").join("loop.db").exists());
        assert!(root.join(".loop").join("plan.md").exists());
        assert!(root.join(".loop").join("agent-project.md").exists());
    }

    #[test]
    fn initialize_returns_already_initialized_on_second_call() {
        let (repo, _root) = temp_repo();
        repo.initialize().unwrap();
        let result = repo.initialize();
        assert!(matches!(result, Err(LoopError::AlreadyInitialized)));
    }

    #[test]
    fn initialize_does_not_overwrite_existing_plan() {
        let (repo, root) = temp_repo();
        repo.initialize().unwrap();
        let plan_path = root.join(".loop").join("plan.md");
        std::fs::write(&plan_path, "# My custom plan\n").unwrap();
        // Re-initializing from a fresh repo object on the same dir should give AlreadyInitialized
        let repo2 = SqliteRepository::new(&root);
        assert!(matches!(
            repo2.initialize(),
            Err(LoopError::AlreadyInitialized)
        ));
        assert_eq!(
            std::fs::read_to_string(&plan_path).unwrap(),
            "# My custom plan\n"
        );
    }

    #[test]
    fn add_task_assigns_sequential_ids() {
        let (repo, _root) = init_repo();
        let t1 = repo.add_task("First task").unwrap();
        let t2 = repo.add_task("Second task").unwrap();
        assert_eq!(t1.seq, 1);
        assert_eq!(t2.seq, 2);
        assert_eq!(t1.id.as_str(), "001");
        assert_eq!(t2.id.as_str(), "002");
        assert_eq!(t1.status, TaskStatus::Pending);
    }

    #[test]
    fn add_task_empty_title_fails() {
        let (repo, _root) = init_repo();
        assert!(repo.add_task("").is_err());
        assert!(repo.add_task("   ").is_err());
    }

    #[test]
    fn add_task_not_initialized_fails() {
        let (repo, _root) = temp_repo();
        assert!(matches!(repo.add_task("x"), Err(LoopError::NotInitialized)));
    }

    #[test]
    fn list_tasks_returns_in_seq_order() {
        let (repo, _root) = init_repo();
        repo.add_task("First").unwrap();
        repo.add_task("Second").unwrap();
        repo.add_task("Third").unwrap();
        let tasks = repo.list_tasks().unwrap();
        assert_eq!(tasks.len(), 3);
        assert_eq!(tasks[0].seq, 1);
        assert_eq!(tasks[1].seq, 2);
        assert_eq!(tasks[2].seq, 3);
    }

    #[test]
    fn list_tasks_empty_when_no_tasks() {
        let (repo, _root) = init_repo();
        let tasks = repo.list_tasks().unwrap();
        assert!(tasks.is_empty());
    }

    #[test]
    fn list_tasks_not_initialized_fails() {
        let (repo, _root) = temp_repo();
        assert!(matches!(repo.list_tasks(), Err(LoopError::NotInitialized)));
    }

    #[test]
    fn get_task_by_id() {
        let (repo, _root) = init_repo();
        let added = repo.add_task("My task").unwrap();
        let fetched = repo.get_task(added.id.as_str()).unwrap();
        assert_eq!(fetched.id, added.id);
        assert_eq!(fetched.title, "My task");
        assert_eq!(fetched.status, TaskStatus::Pending);
    }

    #[test]
    fn get_task_not_found() {
        let (repo, _root) = init_repo();
        assert!(matches!(
            repo.get_task("999"),
            Err(LoopError::TaskNotFound(_))
        ));
    }

    #[test]
    fn get_task_invalid_id_format() {
        let (repo, _root) = init_repo();
        assert!(matches!(
            repo.get_task("abc"),
            Err(LoopError::TaskNotFound(_))
        ));
    }

    #[test]
    fn update_task_start_persists() {
        let (repo, _root) = init_repo();
        let task = repo.add_task("To be started").unwrap();
        let started = transition_start(task, Utc::now()).unwrap();
        let _ = repo.update_task(started).unwrap();
        let fetched = repo.get_task("001").unwrap();
        assert_eq!(fetched.status, TaskStatus::Running);
        assert!(fetched.started_at.is_some());
    }

    #[test]
    fn update_task_complete_persists_metrics() {
        let (repo, _root) = init_repo();
        let task = repo.add_task("To be completed").unwrap();
        let started = transition_start(task, Utc::now()).unwrap();
        let _ = repo.update_task(started.clone()).unwrap();
        let data = CompletionData {
            notes: "Done!".to_owned(),
            completed_at: Utc::now(),
            input_tokens: Some(100),
            output_tokens: Some(50),
            model: Some("claude-sonnet-4-6".to_owned()),
            commit_sha: Some("abc1234".to_owned()),
        };
        let completed = transition_complete(started, data).unwrap();
        let _ = repo.update_task(completed).unwrap();
        let fetched = repo.get_task("001").unwrap();
        assert_eq!(fetched.status, TaskStatus::Complete);
        assert_eq!(fetched.completion_notes_md, "Done!");
        assert_eq!(fetched.input_tokens, Some(100));
        assert_eq!(fetched.output_tokens, Some(50));
        assert_eq!(fetched.model.as_deref(), Some("claude-sonnet-4-6"));
        assert_eq!(fetched.commit_sha.as_deref(), Some("abc1234"));
        assert!(fetched.completed_at.is_some());
        assert!(fetched.duration_seconds.is_some());
    }

    #[test]
    fn update_task_reset_clears_runtime_fields_preserves_notes() {
        let (repo, _root) = init_repo();
        let task = repo.add_task("To be reset").unwrap();
        let started = transition_start(task, Utc::now()).unwrap();
        let _ = repo.update_task(started.clone()).unwrap();
        let data = CompletionData {
            notes: "Preserved note".to_owned(),
            completed_at: Utc::now(),
            input_tokens: Some(10),
            output_tokens: Some(5),
            model: None,
            commit_sha: None,
        };
        let completed = transition_complete(started, data).unwrap();
        let _ = repo.update_task(completed).unwrap();
        let fetched = repo.get_task("001").unwrap();
        let reset = transition_reset(fetched);
        let _ = repo.update_task(reset).unwrap();
        let final_task = repo.get_task("001").unwrap();
        assert_eq!(final_task.status, TaskStatus::Pending);
        assert!(final_task.started_at.is_none());
        assert!(final_task.completed_at.is_none());
        assert!(final_task.duration_seconds.is_none());
        assert!(final_task.input_tokens.is_none());
        assert_eq!(final_task.completion_notes_md, "Preserved note");
    }

    #[test]
    fn update_task_not_found_fails() {
        let (repo, _root) = init_repo();
        let task = Task::new(99, "ghost".to_owned(), Utc::now());
        assert!(matches!(
            repo.update_task(task),
            Err(LoopError::TaskNotFound(_))
        ));
    }

    #[test]
    fn read_plan_returns_content() {
        let (repo, root) = init_repo();
        let plan_path = root.join(".loop").join("plan.md");
        std::fs::write(&plan_path, "# My Plan\n\nDo great things.\n").unwrap();
        let content = repo.read_plan().unwrap();
        assert!(content.contains("My Plan"));
        assert!(content.contains("Do great things."));
    }

    #[test]
    fn read_agent_project_returns_content() {
        let (repo, root) = init_repo();
        let path = root.join(".loop").join("agent-project.md");
        std::fs::write(&path, "# Project-only\nCTX\n").unwrap();
        let content = repo.read_agent_project().unwrap();
        assert!(content.contains("Project-only"));
        assert!(content.contains("CTX"));
    }

    #[test]
    fn read_plan_not_initialized_fails() {
        let (repo, _root) = temp_repo();
        assert!(matches!(repo.read_plan(), Err(LoopError::NotInitialized)));
    }

    #[test]
    fn depends_on_round_trips() {
        assert_eq!(parse_depends_on("[]"), vec![]);
        let ids = parse_depends_on(r#"["001","002"]"#);
        assert_eq!(ids.len(), 2);
        assert_eq!(ids[0].as_str(), "001");
        assert_eq!(ids[1].as_str(), "002");
    }

    #[test]
    fn infer_event_type_maps_domain_transitions() {
        assert_eq!(
            infer_event_type(TaskStatus::Pending, TaskStatus::Running),
            Some(EventType::Started)
        );
        assert_eq!(
            infer_event_type(TaskStatus::Running, TaskStatus::Complete),
            Some(EventType::Completed)
        );
        assert_eq!(
            infer_event_type(TaskStatus::Running, TaskStatus::Failed),
            Some(EventType::Failed)
        );
        assert_eq!(
            infer_event_type(TaskStatus::Complete, TaskStatus::Pending),
            Some(EventType::Reset)
        );
        assert_eq!(
            infer_event_type(TaskStatus::Failed, TaskStatus::Pending),
            Some(EventType::Reset)
        );
        assert_eq!(
            infer_event_type(TaskStatus::Pending, TaskStatus::Complete),
            None
        );
        assert_eq!(
            infer_event_type(TaskStatus::Running, TaskStatus::Running),
            None
        );
    }

    #[test]
    fn apply_migrations_fresh_then_idempotent_second_call() {
        let (_repo, root) = temp_repo();
        let loop_dir = root.join(".loop");
        std::fs::create_dir_all(&loop_dir).unwrap();
        let db_path = loop_dir.join("loop.db");

        let mut conn = Connection::open(&db_path).unwrap();
        let v_before: i32 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(v_before, 0);

        assert!(SqliteRepository::apply_migrations(&mut conn).unwrap());

        let v_after: i32 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(v_after, super::LATEST_VERSION);

        let table_count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('tasks','events','config')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(table_count, 3);

        assert!(!SqliteRepository::apply_migrations(&mut conn).unwrap());
        drop(conn);

        let repo = SqliteRepository::new(&root);
        repo.add_task("after migrate").unwrap();
        let tasks = repo.list_tasks().unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title, "after migrate");
    }

    #[test]
    fn apply_migrations_idempotent_when_schema_already_current() {
        let (_repo, root) = init_repo();
        let db_path = root.join(".loop").join("loop.db");
        let mut conn = Connection::open(&db_path).unwrap();
        assert!(!SqliteRepository::apply_migrations(&mut conn).unwrap());
        assert!(!SqliteRepository::apply_migrations(&mut conn).unwrap());
    }

    #[test]
    fn get_task_missing_digit_id_is_task_not_found() {
        let (repo, _root) = init_repo();
        repo.add_task("exists").unwrap();
        assert!(matches!(
            repo.get_task("002"),
            Err(LoopError::TaskNotFound(_))
        ));
    }
}
