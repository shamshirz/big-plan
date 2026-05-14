pub mod cli;
pub mod commands;
pub mod domain;
pub mod orchestrator;
pub mod render;
pub mod repository;
pub mod sqlite_repo;

use cli::{Command, ParseError, ReadTarget};
use repository::TaskRepository;
use sqlite_repo::SqliteRepository;

fn main() {
    let cmd = match cli::parse() {
        Ok(c) => c,
        Err(ParseError::UnknownCommand(cmd)) => {
            eprintln!("error: unknown command '{cmd}'");
            eprintln!("Run `bp -h` for usage.");
            std::process::exit(1);
        }
        Err(ParseError::MissingTitle) => {
            eprintln!("error: title is required");
            eprintln!("Usage: bp add \"<title>\"");
            std::process::exit(1);
        }
        Err(ParseError::EmptyTitle) => {
            eprintln!("error: title must not be empty");
            std::process::exit(1);
        }
        Err(ParseError::MissingId { cmd }) => {
            eprintln!("error: task id is required");
            eprintln!("Usage: bp {cmd} <id>");
            std::process::exit(1);
        }
        Err(ParseError::MissingReadTarget) => {
            eprintln!("error: target is required");
            eprintln!("Usage: bp read plan|current|<id>");
            std::process::exit(1);
        }
        Err(ParseError::MissingNotesValue) => {
            eprintln!("error: --notes requires a value");
            eprintln!("Usage: bp complete [--notes \"<text>\"]");
            std::process::exit(1);
        }
    };

    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let repo = SqliteRepository::new(&cwd);
    let exit_code = dispatch(cmd, &repo, &cwd);
    std::process::exit(exit_code);
}

fn dispatch(cmd: Command, repo: &dyn TaskRepository, cwd: &std::path::Path) -> i32 {
    match cmd {
        Command::Help => commands::help(),
        Command::Init => commands::init(repo),
        Command::Add { title } => commands::add(repo, &title),
        Command::Status => commands::status(repo),
        Command::Show { id } => commands::show(repo, &id),
        Command::Read {
            target: ReadTarget::Plan,
        } => commands::read_plan(repo),
        Command::Read {
            target: ReadTarget::Current,
        } => commands::read_current(repo),
        Command::Read {
            target: ReadTarget::Task(id),
        } => commands::read_task(repo, &id),
        Command::Run => commands::run(repo, cwd),
        Command::Complete { notes } => commands::complete(repo, notes.as_deref()),
        Command::Reset { id } => commands::reset(repo, &id),
    }
}
