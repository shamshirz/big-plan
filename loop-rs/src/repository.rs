use crate::domain::Task;
use std::fmt;

#[derive(Debug)]
pub enum LoopError {
    NotInitialized,
    AlreadyInitialized,
    TaskNotFound(String),
    AlreadyRunning(String),
    NoRunningTask,
    PermissionDenied(String),
    Io(String),
}

impl fmt::Display for LoopError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoopError::NotInitialized => {
                write!(f, "loop not initialized — run `loop init` first")
            }
            LoopError::AlreadyInitialized => write!(f, "Loop already initialized in .loop/"),
            LoopError::TaskNotFound(id) => write!(f, "task '{id}' not found"),
            LoopError::AlreadyRunning(id) => write!(
                f,
                "task {id} is already running — complete or reset it before running again"
            ),
            LoopError::NoRunningTask => write!(f, "no task is currently running"),
            LoopError::PermissionDenied(path) => {
                write!(f, "cannot create {path} — permission denied")
            }
            LoopError::Io(msg) => write!(f, "{msg}"),
        }
    }
}

pub trait TaskRepository {
    fn initialize(&self) -> Result<(), LoopError>;
    fn add_task(&self, title: &str) -> Result<Task, LoopError>;
    fn list_tasks(&self) -> Result<Vec<Task>, LoopError>;
    /// Look up a task by its raw id string (e.g. "003"). Returns TaskNotFound for
    /// both unknown IDs and malformed ID strings.
    fn get_task(&self, id: &str) -> Result<Task, LoopError>;
    fn update_task(&self, task: Task) -> Result<Task, LoopError>;
    fn read_plan(&self) -> Result<String, LoopError>;
}
