# SQLite Schema and Migration Strategy

This document is the authoritative schema design for `loop.db`.
The Rust adapter (task 006) must implement exactly this schema.

---

## Database File

Path: `<project-root>/.loop/loop.db`

Created by `loop init`. Never created automatically by other commands.

---

## Schema Version Tracking

Use SQLite's built-in `PRAGMA user_version` to track the applied migration version.
No extra table needed. Bootstrap sequence:

1. Open (or create) `loop.db`.
2. Read `PRAGMA user_version` — returns `0` for a brand-new database.
3. Apply each migration N where N > current `user_version`, in order.
4. After each migration succeeds, `PRAGMA user_version = N`.

Each migration runs inside a transaction. If the transaction fails, the database is
left at the previous version and the error is surfaced to the user.

Current latest version: **1**

---

## Migration 1 — Initial Schema

```sql
-- tasks: one row per planned unit of work
CREATE TABLE tasks (
    id                  TEXT    NOT NULL PRIMARY KEY,
    seq                 INTEGER NOT NULL UNIQUE,
    title               TEXT    NOT NULL,
    status              TEXT    NOT NULL DEFAULT 'pending'
                            CHECK(status IN ('pending','running','complete','failed')),
    depends_on          TEXT    NOT NULL DEFAULT '[]',   -- JSON array of task id strings
    description_md      TEXT    NOT NULL DEFAULT '',
    context_md          TEXT    NOT NULL DEFAULT '',
    acceptance_md       TEXT    NOT NULL DEFAULT '',
    completion_notes_md TEXT    NOT NULL DEFAULT '',
    created_at          TEXT    NOT NULL,                -- ISO 8601 UTC
    started_at          TEXT,
    completed_at        TEXT,
    duration_seconds    INTEGER,
    input_tokens        INTEGER,
    output_tokens       INTEGER,
    model               TEXT,
    commit_sha          TEXT
);

-- events: append-only lifecycle audit log
CREATE TABLE events (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id       TEXT    NOT NULL REFERENCES tasks(id),
    event_type    TEXT    NOT NULL
                      CHECK(event_type IN ('created','started','completed','failed','reset')),
    timestamp     TEXT    NOT NULL,                      -- ISO 8601 UTC
    metadata_json TEXT    NOT NULL DEFAULT '{}'          -- freeform JSON for extras
);

-- config: project-level key-value store (schema version is in PRAGMA, not here)
CREATE TABLE config (
    key   TEXT NOT NULL PRIMARY KEY,
    value TEXT NOT NULL DEFAULT ''
);

-- indexes for the hot read paths
CREATE INDEX idx_tasks_status ON tasks(status);
CREATE INDEX idx_events_task_id ON events(task_id);
```

---

## Field Notes

### `tasks.id`

Zero-padded three-digit string: `"001"`, `"042"`, `"100"`.
Assigned at insert time as `printf('%03d', max(seq) + 1)` or `1` when the table is empty.
`id` and `seq` are always equal in value — `seq` is the integer form used for ordering queries.
IDs are immutable after creation.

### `tasks.depends_on`

JSON-encoded array of `id` strings, e.g. `'["001","002"]'` or `'[]'`.
Dependency resolution is application-level; the DB does not enforce referential integrity
on this field. Chosen over a normalized join table to keep the PoC adapter simple.

### `tasks.status` transitions (enforced by application logic)

```
pending ──run──> running ──complete──> complete
                         └─failure──> failed
any state ──reset──> pending  (clears runtime fields)
```

Invariant: at most one task may have `status = 'running'` at any time.
Enforced in application code (see task 004 domain model), not by a DB constraint,
because SQLite partial unique indexes require SQLite ≥ 3.8.9 and the constraint
is more clearly expressed in domain logic.

### Timestamp format

All timestamp columns store ISO 8601 UTC strings: `"2026-05-07T18:32:46Z"`.
SQLite has no native DATETIME type; TEXT is the canonical choice for portability.

### `events.metadata_json`

Free-form JSON object. Typical contents by event type:

| event_type  | typical keys                                      |
|-------------|---------------------------------------------------|
| `created`   | `{}`                                              |
| `started`   | `{}`                                              |
| `completed` | `{"model": "...", "input_tokens": N, "output_tokens": N, "commit_sha": "..."}` |
| `failed`    | `{"exit_code": N, "message": "..."}`              |
| `reset`     | `{}`                                              |

### `config` pre-seeded keys

`loop init` inserts these rows after running migrations:

| key                  | initial value            |
|----------------------|--------------------------|
| `schema_version_tag` | `"v1"`                   |
| `project_name`       | `""`  (user fills in)    |

---

## `loop init` Bootstrap Sequence

1. Check for `.loop/` directory. If absent, create it.
2. Check for `.loop/loop.db`. If absent, create it (SQLite opens on first access).
3. Run migration bootstrap:
   - Read `PRAGMA user_version`.
   - Apply all pending migrations in version order.
4. Seed `config` rows (skip if key already exists — use `INSERT OR IGNORE`).
5. Create `.loop/plan.md` and `.loop/agent-project.md` with placeholder templates
   (skip if already present — do not overwrite).
6. Print `Initialized loop state in .loop/` and exit 0.

If `.loop/loop.db` already exists and `user_version` matches the latest, print
`Loop already initialized in .loop/` and exit 0 without modifying data.

---

## `loop add` Insert Sequence

```sql
BEGIN;
INSERT INTO tasks (id, seq, title, status, created_at)
VALUES (
    printf('%03d', COALESCE((SELECT MAX(seq) FROM tasks), 0) + 1),
    COALESCE((SELECT MAX(seq) FROM tasks), 0) + 1,
    ?,         -- title
    'pending',
    ?          -- created_at ISO8601
);
INSERT INTO events (task_id, event_type, timestamp)
VALUES (last_insert_rowid()... -- use the id just inserted
COMMIT;
```

(Exact Rust implementation deferred to task 006.)

---

## Query Patterns

### `loop status` — list all tasks ordered by seq

```sql
SELECT id, status, title FROM tasks ORDER BY seq ASC;
```

### `loop show <id>` / `loop read <id>` — fetch one task

```sql
SELECT * FROM tasks WHERE id = ?;
```

### `loop read current` — find the running task

```sql
SELECT * FROM tasks WHERE status = 'running' LIMIT 1;
```

### `loop run` — find next pending task

```sql
SELECT * FROM tasks WHERE status = 'pending' ORDER BY seq ASC LIMIT 1;
```

### `loop reset <id>` — clear runtime fields

```sql
UPDATE tasks
SET status = 'pending',
    started_at = NULL,
    completed_at = NULL,
    duration_seconds = NULL,
    input_tokens = NULL,
    output_tokens = NULL,
    model = NULL,
    commit_sha = NULL
WHERE id = ?;
```

---

## Future Migration Guidance

When schema changes are needed:

1. Write a new migration SQL block numbered N+1.
2. Add it to the migration dispatch in the Rust adapter (an ordered array of `(version, sql)` pairs).
3. The migration bootstrap applies it automatically on next `loop init` or DB open.
4. Never modify an already-applied migration — write a new one instead.
5. Document breaking changes in `CHANGELOG.md` (not required for PoC).

Migrations that add nullable columns or new tables are safe on existing DBs.
Migrations that rename or drop columns require a SQLite table-rebuild pattern
(copy → drop → rename) since SQLite does not support `DROP COLUMN` before version 3.35.
