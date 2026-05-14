use std::env;

#[derive(Debug, PartialEq)]
pub enum Command {
    Help,
    Init,
    Add { title: String },
    Status,
    Show { id: String },
    Read { target: ReadTarget },
    Run,
    Complete { notes: Option<String> },
    Reset { id: String },
}

#[derive(Debug, PartialEq)]
pub enum ReadTarget {
    Plan,
    Current,
    Task(String),
}

#[derive(Debug, PartialEq)]
pub enum ParseError {
    UnknownCommand(String),
    MissingTitle,
    EmptyTitle,
    MissingId { cmd: &'static str },
    MissingReadTarget,
    MissingNotesValue,
}

pub fn parse() -> Result<Command, ParseError> {
    let args: Vec<String> = env::args().collect();
    parse_from(&args[1..])
}

pub fn parse_from(args: &[String]) -> Result<Command, ParseError> {
    match args.first().map(String::as_str) {
        None | Some("-h") | Some("--help") => Ok(Command::Help),
        Some("init") => Ok(Command::Init),
        Some("add") => parse_add(&args[1..]),
        Some("status") => Ok(Command::Status),
        Some("show") => parse_show(&args[1..]),
        Some("read") => parse_read(&args[1..]),
        Some("run") => Ok(Command::Run),
        Some("complete") => parse_complete(&args[1..]),
        Some("reset") => parse_reset(&args[1..]),
        Some(unknown) => Err(ParseError::UnknownCommand(unknown.to_owned())),
    }
}

fn parse_add(args: &[String]) -> Result<Command, ParseError> {
    match args.first() {
        None => Err(ParseError::MissingTitle),
        Some(t) if t.trim().is_empty() => Err(ParseError::EmptyTitle),
        Some(t) => Ok(Command::Add { title: t.clone() }),
    }
}

fn parse_show(args: &[String]) -> Result<Command, ParseError> {
    match args.first() {
        None => Err(ParseError::MissingId { cmd: "show" }),
        Some(id) => Ok(Command::Show { id: id.clone() }),
    }
}

fn parse_read(args: &[String]) -> Result<Command, ParseError> {
    match args.first().map(String::as_str) {
        None => Err(ParseError::MissingReadTarget),
        Some("plan") => Ok(Command::Read { target: ReadTarget::Plan }),
        Some("current") => Ok(Command::Read { target: ReadTarget::Current }),
        Some(id) => Ok(Command::Read { target: ReadTarget::Task(id.to_owned()) }),
    }
}

fn parse_complete(args: &[String]) -> Result<Command, ParseError> {
    let notes = extract_notes_flag(args)?;
    Ok(Command::Complete { notes })
}

fn parse_reset(args: &[String]) -> Result<Command, ParseError> {
    match args.first() {
        None => Err(ParseError::MissingId { cmd: "reset" }),
        Some(id) => Ok(Command::Reset { id: id.clone() }),
    }
}

fn extract_notes_flag(args: &[String]) -> Result<Option<String>, ParseError> {
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--notes" {
            return match args.get(i + 1) {
                None => Err(ParseError::MissingNotesValue),
                Some(v) => Ok(Some(v.clone())),
            };
        }
        i += 1;
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &str) -> Vec<String> {
        s.split_whitespace().map(|s| s.to_owned()).collect()
    }

    #[test]
    fn no_args_gives_help() {
        assert_eq!(parse_from(&[]), Ok(Command::Help));
    }

    #[test]
    fn dash_h_gives_help() {
        assert_eq!(parse_from(&args("-h")), Ok(Command::Help));
    }

    #[test]
    fn double_dash_help_gives_help() {
        assert_eq!(parse_from(&args("--help")), Ok(Command::Help));
    }

    #[test]
    fn parse_init() {
        assert_eq!(parse_from(&args("init")), Ok(Command::Init));
    }

    #[test]
    fn parse_add_with_title() {
        let a = ["add".to_owned(), "My task".to_owned()];
        assert_eq!(parse_from(&a), Ok(Command::Add { title: "My task".to_owned() }));
    }

    #[test]
    fn parse_add_no_title_is_missing() {
        assert_eq!(parse_from(&args("add")), Err(ParseError::MissingTitle));
    }

    #[test]
    fn parse_add_empty_title_is_empty() {
        let a = ["add".to_owned(), String::new()];
        assert_eq!(parse_from(&a), Err(ParseError::EmptyTitle));
    }

    #[test]
    fn parse_add_whitespace_title_is_empty() {
        let a = ["add".to_owned(), "   ".to_owned()];
        assert_eq!(parse_from(&a), Err(ParseError::EmptyTitle));
    }

    #[test]
    fn parse_status() {
        assert_eq!(parse_from(&args("status")), Ok(Command::Status));
    }

    #[test]
    fn parse_show_with_id() {
        assert_eq!(
            parse_from(&args("show 003")),
            Ok(Command::Show { id: "003".to_owned() })
        );
    }

    #[test]
    fn parse_show_no_id_is_missing() {
        assert_eq!(parse_from(&args("show")), Err(ParseError::MissingId { cmd: "show" }));
    }

    #[test]
    fn parse_read_plan() {
        assert_eq!(
            parse_from(&args("read plan")),
            Ok(Command::Read { target: ReadTarget::Plan })
        );
    }

    #[test]
    fn parse_read_current() {
        assert_eq!(
            parse_from(&args("read current")),
            Ok(Command::Read { target: ReadTarget::Current })
        );
    }

    #[test]
    fn parse_read_task_id() {
        assert_eq!(
            parse_from(&args("read 005")),
            Ok(Command::Read { target: ReadTarget::Task("005".to_owned()) })
        );
    }

    #[test]
    fn parse_read_no_target_is_missing() {
        assert_eq!(parse_from(&args("read")), Err(ParseError::MissingReadTarget));
    }

    #[test]
    fn parse_run() {
        assert_eq!(parse_from(&args("run")), Ok(Command::Run));
    }

    #[test]
    fn parse_complete_no_notes() {
        assert_eq!(parse_from(&args("complete")), Ok(Command::Complete { notes: None }));
    }

    #[test]
    fn parse_complete_with_notes() {
        let a = ["complete".to_owned(), "--notes".to_owned(), "done".to_owned()];
        assert_eq!(parse_from(&a), Ok(Command::Complete { notes: Some("done".to_owned()) }));
    }

    #[test]
    fn parse_complete_notes_flag_missing_value() {
        let a = ["complete".to_owned(), "--notes".to_owned()];
        assert_eq!(parse_from(&a), Err(ParseError::MissingNotesValue));
    }

    #[test]
    fn parse_reset_with_id() {
        assert_eq!(
            parse_from(&args("reset 003")),
            Ok(Command::Reset { id: "003".to_owned() })
        );
    }

    #[test]
    fn parse_reset_no_id_is_missing() {
        assert_eq!(parse_from(&args("reset")), Err(ParseError::MissingId { cmd: "reset" }));
    }

    #[test]
    fn parse_unknown_command() {
        assert_eq!(
            parse_from(&args("foo")),
            Err(ParseError::UnknownCommand("foo".to_owned()))
        );
    }
}
