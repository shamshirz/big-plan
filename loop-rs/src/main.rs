pub mod cli;
pub mod commands;
pub mod domain;
pub mod repository;

use cli::{Command, ParseError, ReadTarget};
use repository::{LoopError, TaskRepository};

struct StubRepository;

impl TaskRepository for StubRepository {
    fn initialize(&self) -> Result<(), LoopError> {
        Err(LoopError::Io(
            "SQLite repository not yet implemented — see task 006".to_owned(),
        ))
    }
    fn add_task(&self, _title: &str) -> Result<domain::Task, LoopError> {
        Err(LoopError::NotInitialized)
    }
    fn list_tasks(&self) -> Result<Vec<domain::Task>, LoopError> {
        Err(LoopError::NotInitialized)
    }
    fn get_task(&self, id: &str) -> Result<domain::Task, LoopError> {
        Err(LoopError::TaskNotFound(id.to_owned()))
    }
    fn update_task(&self, _task: domain::Task) -> Result<domain::Task, LoopError> {
        Err(LoopError::NotInitialized)
    }
    fn read_plan(&self) -> Result<String, LoopError> {
        Err(LoopError::NotInitialized)
    }
}

fn main() {
    let cmd = match cli::parse() {
        Ok(c) => c,
        Err(ParseError::UnknownCommand(cmd)) => {
            eprintln!("error: unknown command '{cmd}'");
            eprintln!("Run `loop -h` for usage.");
            std::process::exit(1);
        }
        Err(ParseError::MissingTitle) => {
            eprintln!("error: title is required");
            eprintln!("Usage: loop add \"<title>\"");
            std::process::exit(1);
        }
        Err(ParseError::EmptyTitle) => {
            eprintln!("error: title must not be empty");
            std::process::exit(1);
        }
        Err(ParseError::MissingId { cmd }) => {
            eprintln!("error: task id is required");
            eprintln!("Usage: loop {cmd} <id>");
            std::process::exit(1);
        }
        Err(ParseError::MissingReadTarget) => {
            eprintln!("error: target is required");
            eprintln!("Usage: loop read plan|current|<id>");
            std::process::exit(1);
        }
        Err(ParseError::MissingNotesValue) => {
            eprintln!("error: --notes requires a value");
            eprintln!("Usage: loop complete [--notes \"<text>\"]");
            std::process::exit(1);
        }
    };

    let repo = StubRepository;
    let exit_code = dispatch(cmd, &repo);
    std::process::exit(exit_code);
}

fn dispatch(cmd: Command, repo: &dyn TaskRepository) -> i32 {
    match cmd {
        Command::Help => commands::help(),
        Command::Init => commands::init(repo),
        Command::Add { title } => commands::add(repo, &title),
        Command::Status => commands::status(repo),
        Command::Show { id } => commands::show(repo, &id),
        Command::Read { target: ReadTarget::Plan } => commands::read_plan(repo),
        Command::Read { target: ReadTarget::Current } => commands::read_current(repo),
        Command::Read { target: ReadTarget::Task(id) } => commands::read_task(repo, &id),
        Command::Run => commands::run(repo),
        Command::Complete { notes } => commands::complete(repo, notes.as_deref()),
        Command::Reset { id } => commands::reset(repo, &id),
    }
}
