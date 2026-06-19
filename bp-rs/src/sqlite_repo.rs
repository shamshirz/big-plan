use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};

use crate::domain::{
    validate_title, Event, EventMetadata, EventType, Goal, GoalStatus, Task, TaskId, TaskKind,
    TaskStatus,
};
use crate::repository::{LoopError, TaskRepository};

const SKILL_TEMPLATE: &str = include_str!("../templates/SKILL.md");

const LATEST_VERSION: i32 = 2;

const MIGRATIONS: &[(i32, &str)] = &[
    (
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
    ),
    (
        2,
        "CREATE TABLE goals (
        id          INTEGER PRIMARY KEY AUTOINCREMENT,
        title       TEXT    NOT NULL,
        plan_md     TEXT    NOT NULL DEFAULT '',
        created_at  TEXT    NOT NULL,
        status      TEXT    NOT NULL DEFAULT 'active'
                        CHECK(status IN ('active','complete','archived'))
    );
    INSERT INTO goals (id, title, plan_md, created_at, status)
        VALUES (1, 'Initial', '', datetime('now'), 'active');
    ALTER TABLE tasks ADD COLUMN goal_id INTEGER NOT NULL DEFAULT 1;
    ALTER TABLE tasks ADD COLUMN kind TEXT NOT NULL DEFAULT 'execute'
        CHECK(kind IN ('plan','execute'));
    INSERT OR REPLACE INTO config (key, value) VALUES ('active_goal_id', '1');
    CREATE INDEX idx_tasks_goal_id ON tasks(goal_id);",
    ),
];

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
        let mut conn = Connection::open(&self.db_path).map_err(|e| LoopError::Io(e.to_string()))?;
        Self::apply_migrations(&mut conn)?;
        self.ensure_runtime_files()?;
        Ok(conn)
    }

    fn ensure_runtime_files(&self) -> Result<(), LoopError> {
        let skill_path = self.loop_dir.join("SKILL.md");
        if !skill_path.exists() {
            std::fs::write(skill_path, SKILL_TEMPLATE).map_err(|e| LoopError::Io(e.to_string()))?;
        }
        Ok(())
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

    fn active_goal_id(&self, conn: &Connection) -> Result<u64, LoopError> {
        let value: String = conn
            .query_row(
                "SELECT value FROM config WHERE key = 'active_goal_id'",
                [],
                |row| row.get(0),
            )
            .unwrap_or_else(|_| "1".to_owned());
        Ok(value.parse::<u64>().unwrap_or(1))
    }

    fn insert_task(
        &self,
        conn: &mut Connection,
        goal_id: u64,
        kind: TaskKind,
        title: &str,
        description_md: &str,
        acceptance_md: &str,
    ) -> Result<Task, LoopError> {
        validate_title(title).map_err(|e| LoopError::Io(e.to_string()))?;
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
            "INSERT INTO tasks (id, seq, goal_id, kind, title, status, depends_on, \
             description_md, acceptance_md, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, 'pending', '[]', ?6, ?7, ?8)",
            params![
                id.as_str(),
                next_seq as i64,
                goal_id as i64,
                kind.as_str(),
                title,
                description_md,
                acceptance_md,
                &now_str,
            ],
        )
        .map_err(|e| LoopError::Io(e.to_string()))?;

        tx.execute(
            "INSERT INTO events (task_id, event_type, timestamp, metadata_json) \
             VALUES (?1, 'created', ?2, ?3)",
            params![id.as_str(), &now_str, "{}"],
        )
        .map_err(|e| LoopError::Io(e.to_string()))?;

        tx.commit().map_err(|e| LoopError::Io(e.to_string()))?;

        Ok(Task {
            id,
            seq: next_seq,
            goal_id,
            kind,
            title: title.to_owned(),
            status: TaskStatus::Pending,
            depends_on: vec![],
            description_md: description_md.to_owned(),
            context_md: String::new(),
            acceptance_md: acceptance_md.to_owned(),
            completion_notes_md: String::new(),
            created_at: now,
            started_at: None,
            completed_at: None,
            duration_seconds: None,
            input_tokens: None,
            output_tokens: None,
            model: None,
            commit_sha: None,
        })
    }

    fn query_tasks(&self, conn: &Connection, goal_id: Option<i64>) -> Result<Vec<Task>, LoopError> {
        let (sql, param): (&str, Option<i64>) = match goal_id {
            Some(id) => (
                "SELECT id, seq, goal_id, kind, title, status, depends_on, description_md, \
                 context_md, acceptance_md, completion_notes_md, created_at, started_at, \
                 completed_at, duration_seconds, input_tokens, output_tokens, model, commit_sha \
                 FROM tasks WHERE goal_id = ?1 ORDER BY seq ASC",
                Some(id),
            ),
            None => (
                "SELECT id, seq, goal_id, kind, title, status, depends_on, description_md, \
                 context_md, acceptance_md, completion_notes_md, created_at, started_at, \
                 completed_at, duration_seconds, input_tokens, output_tokens, model, commit_sha \
                 FROM tasks ORDER BY seq ASC",
                None,
            ),
        };

        let mut stmt = conn.prepare(sql).map_err(|e| LoopError::Io(e.to_string()))?;
        let mut rows = match param {
            Some(id) => stmt.query(params![id]),
            None => stmt.query([]),
        }
        .map_err(|e| LoopError::Io(e.to_string()))?;

        let mut tasks = Vec::new();
        while let Some(row) = rows.next().map_err(|e| LoopError::Io(e.to_string()))? {
            tasks.push(row_to_task(row).map_err(|e| LoopError::Io(e.to_string()))?);
        }
        Ok(tasks)
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
            "INSERT OR IGNORE INTO config (key, value) VALUES ('schema_version_tag', 'v2');
             INSERT OR IGNORE INTO config (key, value) VALUES ('project_name', '');",
        )
        .map_err(|e| LoopError::Io(e.to_string()))?;

        let agent_path = self.loop_dir.join("agent-project.md");
        if !agent_path.exists() {
            std::fs::write(
                &agent_path,
                "# Project Context\n\n<!-- Optional: project-specific notes for agents -->\n",
            )
            .map_err(|e| LoopError::Io(e.to_string()))?;
        }

        let skill_path = self.loop_dir.join("SKILL.md");
        if !skill_path.exists() {
            std::fs::write(skill_path, SKILL_TEMPLATE).map_err(|e| LoopError::Io(e.to_string()))?;
        }

        Ok(())
    }

    fn add_task(&self, title: &str) -> Result<Task, LoopError> {
        let mut conn = self.open()?;
        let goal_id = self.active_goal_id(&conn)?;
        self.insert_task(
            &mut conn,
            goal_id,
            TaskKind::Execute,
            title,
            "",
            "",
        )
    }

    fn add_planning_task(&self, title: &str, plan_md: &str) -> Result<Task, LoopError> {
        let mut conn = self.open()?;
        let goal_id = self.active_goal_id(&conn)?;
        let acceptance = "Use `bp add \"<title>\"` for each executable task. \
                          Split work for fresh context windows. \
                          Run `bp complete` when the queue is ready.";
        self.insert_task(
            &mut conn,
            goal_id,
            TaskKind::Plan,
            title,
            plan_md,
            acceptance,
        )
    }

    fn list_active_goal_tasks(&self) -> Result<Vec<Task>, LoopError> {
        let conn = self.open()?;
        let goal_id = self.active_goal_id(&conn)? as i64;
        self.query_tasks(&conn, Some(goal_id))
    }

    fn create_goal(&self, title: &str, plan_md: &str) -> Result<Goal, LoopError> {
        let mut conn = self.open()?;
        let now = Utc::now();
        let now_str = format_ts(now);

        let tx = conn
            .transaction()
            .map_err(|e| LoopError::Io(e.to_string()))?;

        tx.execute(
            "UPDATE goals SET status = 'archived' WHERE status = 'active'",
            [],
        )
        .map_err(|e| LoopError::Io(e.to_string()))?;

        tx.execute(
            "INSERT INTO goals (title, plan_md, created_at, status) VALUES (?1, ?2, ?3, 'active')",
            params![title, plan_md, &now_str],
        )
        .map_err(|e| LoopError::Io(e.to_string()))?;

        let goal_id: i64 = tx.last_insert_rowid();
        tx.execute(
            "INSERT OR REPLACE INTO config (key, value) VALUES ('active_goal_id', ?1)",
            params![goal_id.to_string()],
        )
        .map_err(|e| LoopError::Io(e.to_string()))?;

        tx.commit().map_err(|e| LoopError::Io(e.to_string()))?;

        Ok(Goal {
            id: goal_id as u64,
            title: title.to_owned(),
            plan_md: plan_md.to_owned(),
            created_at: now,
            status: GoalStatus::Active,
        })
    }

    fn list_goals(&self) -> Result<Vec<Goal>, LoopError> {
        let conn = self.open()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, title, plan_md, created_at, status FROM goals ORDER BY id ASC",
            )
            .map_err(|e| LoopError::Io(e.to_string()))?;
        let mut rows = stmt.query([]).map_err(|e| LoopError::Io(e.to_string()))?;
        let mut goals = Vec::new();
        while let Some(row) = rows.next().map_err(|e| LoopError::Io(e.to_string()))? {
            goals.push(row_to_goal(row).map_err(|e| LoopError::Io(e.to_string()))?);
        }
        Ok(goals)
    }

    fn get_active_goal(&self) -> Result<Goal, LoopError> {
        let conn = self.open()?;
        let goal_id = self.active_goal_id(&conn)? as i64;
        conn.query_row(
            "SELECT id, title, plan_md, created_at, status FROM goals WHERE id = ?1",
            params![goal_id],
            row_to_goal,
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => LoopError::Io("no active goal".to_owned()),
            e => LoopError::Io(e.to_string()),
        })
    }

    fn read_skill(&self) -> Result<String, LoopError> {
        if !self.db_path.exists() {
            return Err(LoopError::NotInitialized);
        }
        let path = self.loop_dir.join("SKILL.md");
        std::fs::read_to_string(&path).map_err(|e| LoopError::Io(e.to_string()))
    }

    fn skill_path(&self) -> String {
        self.loop_dir.join("SKILL.md").display().to_string()
    }

    fn list_tasks(&self) -> Result<Vec<Task>, LoopError> {
        let conn = self.open()?;
        self.query_tasks(&conn, None)
    }

    fn get_task(&self, id: &str) -> Result<Task, LoopError> {
        let task_id = TaskId::parse(id).map_err(|_| LoopError::TaskNotFound(id.to_owned()))?;
        let conn = self.open()?;
        conn.query_row(
            "SELECT id, seq, goal_id, kind, title, status, depends_on, description_md, context_md, \
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
        self.get_active_goal().map(|g| g.plan_md)
    }

    fn read_agent_project(&self) -> Result<String, LoopError> {
        if !self.db_path.exists() {
            return Err(LoopError::NotInitialized);
        }
        let path = self.loop_dir.join("agent-project.md");
        std::fs::read_to_string(&path).map_err(|e| LoopError::Io(e.to_string()))
    }

    fn list_events(&self) -> Result<Vec<crate::domain::Event>, LoopError> {
        let conn = self.open()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, task_id, event_type, timestamp, metadata_json \
                 FROM events ORDER BY timestamp ASC, id ASC",
            )
            .map_err(|e| LoopError::Io(e.to_string()))?;

        let mut rows = stmt.query([]).map_err(|e| LoopError::Io(e.to_string()))?;
        let mut events = Vec::new();
        while let Some(row) = rows.next().map_err(|e| LoopError::Io(e.to_string()))? {
            events.push(row_to_event(row).map_err(|e| LoopError::Io(e.to_string()))?);
        }
        Ok(events)
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

fn row_to_event(row: &rusqlite::Row<'_>) -> rusqlite::Result<Event> {
    let id: i64 = row.get("id")?;
    let task_id_str: String = row.get("task_id")?;
    let event_type_str: String = row.get("event_type")?;
    let timestamp_str: String = row.get("timestamp")?;
    let _metadata_json: String = row.get("metadata_json")?;

    let task_id = TaskId::parse(&task_id_str)
        .map_err(|e| rusqlite::Error::InvalidColumnName(e.to_string()))?;
    let event_type = EventType::parse(&event_type_str)
        .map_err(|e| rusqlite::Error::InvalidColumnName(e.to_string()))?;

    Ok(Event {
        id: Some(id),
        task_id,
        event_type,
        timestamp: parse_ts(&timestamp_str),
        metadata: EventMetadata::Empty,
    })
}

fn row_to_goal(row: &rusqlite::Row<'_>) -> rusqlite::Result<Goal> {
    let id: i64 = row.get("id")?;
    let title: String = row.get("title")?;
    let plan_md: String = row.get("plan_md")?;
    let created_at_str: String = row.get("created_at")?;
    let status_str: String = row.get("status")?;
    let status = GoalStatus::parse(&status_str)
        .map_err(|e| rusqlite::Error::InvalidColumnName(e.to_string()))?;
    Ok(Goal {
        id: id as u64,
        title,
        plan_md,
        created_at: parse_ts(&created_at_str),
        status,
    })
}

fn row_to_task(row: &rusqlite::Row<'_>) -> rusqlite::Result<Task> {
    let id_str: String = row.get("id")?;
    let seq: i64 = row.get("seq")?;
    let goal_id: i64 = row.get("goal_id")?;
    let kind_str: String = row.get("kind")?;
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
    let kind = TaskKind::parse(&kind_str)
        .map_err(|e| rusqlite::Error::InvalidColumnName(e.to_string()))?;
    let status = TaskStatus::parse(&status_str)
        .map_err(|e| rusqlite::Error::InvalidColumnName(e.to_string()))?;
    let depends_on = parse_depends_on(&depends_on_json);
    let created_at = parse_ts(&created_at_str);
    let started_at = started_at_str.as_deref().map(parse_ts);
    let completed_at = completed_at_str.as_deref().map(parse_ts);

    Ok(Task {
        id,
        seq: seq as u32,
        goal_id: goal_id as u64,
        kind,
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
        assert!(root.join(".loop").join("SKILL.md").exists());
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
    fn initialize_does_not_overwrite_existing_skill() {
        let (repo, root) = temp_repo();
        repo.initialize().unwrap();
        let skill_path = root.join(".loop").join("SKILL.md");
        std::fs::write(&skill_path, "# Custom skill\n").unwrap();
        let repo2 = SqliteRepository::new(&root);
        assert!(matches!(
            repo2.initialize(),
            Err(LoopError::AlreadyInitialized)
        ));
        assert_eq!(
            std::fs::read_to_string(&skill_path).unwrap(),
            "# Custom skill\n"
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
        let task = Task::new(99, 1, TaskKind::Execute, "ghost".to_owned(), Utc::now());
        assert!(matches!(
            repo.update_task(task),
            Err(LoopError::TaskNotFound(_))
        ));
    }

    #[test]
    fn read_plan_returns_active_goal_plan() {
        let (repo, _root) = init_repo();
        repo.create_goal("Test plan", "# My Plan\n\nDo great things.\n")
            .unwrap();
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
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('tasks','events','config','goals')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(table_count, 4);

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
