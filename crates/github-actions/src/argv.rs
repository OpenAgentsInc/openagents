//! An [`Action`] as the `openagents` command words that do the same thing
//! (#11166's `issue` and `project` verbs), and back.
//!
//! The chat router proposes a command by descending the command tree with
//! Jev and filling its free text with the model (`coder::cli_route`); the
//! web chat reads such a proposal into an [`Action`] here and shows it on
//! a signed confirm card (#11167). This is exact parsing of an already
//! chosen command's bounded fields, never routing: an argv that isn't one
//! of [`COMMANDS`] is [`ArgvError::NotGithub`], and every value goes
//! through the same checks a form's does.
//!
//! The words are the program's own, without `openagents`, group first:
//!
//! - `issue create --title TEXT [--body TEXT] [--label NAME]... [--project N --status NAME] [--repo OWNER/NAME]`
//! - `issue comment N --body TEXT [--repo OWNER/NAME]`
//! - `issue close N [--reason completed|not_planned] [--comment TEXT] [--repo OWNER/NAME]`
//! - `project move N --status NAME --project N [--repo OWNER/NAME]`
//! - `project add N --project N --status NAME [--repo OWNER/NAME]`
//! - `pr open --head BRANCH [--base BRANCH] --title TEXT [--body TEXT] [--draft] [--repo OWNER/NAME]`
//!
//! Words that name a file (`--body-file`, `--comment-file`) are refused:
//! a caller that reads files builds the action from their text itself.

use crate::action::{self, Action, Board, CloseReason, MAX_LABELS};

/// The commands an [`Action`] has a form for, as their command-tree paths.
pub const COMMANDS: [&str; 6] = [
    "issue create",
    "issue comment",
    "issue close",
    "project move",
    "project add",
    "pr open",
];

/// Why an argv is no [`Action`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArgvError {
    /// Not one of [`COMMANDS`]: someone else's command.
    NotGithub,
    /// One of them, without `--repo` and with no repository to default
    /// to.
    NeedsRepository,
    /// One of them, with a word or value it doesn't take; says which.
    Invalid(String),
}

impl std::fmt::Display for ArgvError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArgvError::NotGithub => f.write_str("not a GitHub command"),
            ArgvError::NeedsRepository => f.write_str("name the repository with --repo OWNER/NAME"),
            ArgvError::Invalid(why) => f.write_str(why),
        }
    }
}

/// Whether `argv` (without `openagents`) names one of [`COMMANDS`].
#[must_use]
pub fn is_github(argv: &[String]) -> bool {
    path(argv).is_some()
}

fn path(argv: &[String]) -> Option<&'static str> {
    let (group, verb) = (argv.first()?, argv.get(1)?);
    COMMANDS
        .into_iter()
        .find(|command| command.split_once(' ') == Some((group.as_str(), verb.as_str())))
}

/// The command's words after its two path words: positionals, and each
/// option with its value (`--name value` or `--name=value`).
struct Words {
    positionals: Vec<String>,
    options: Vec<(String, Option<String>)>,
}

/// Options that take no value.
const FLAGS: [&str; 1] = ["draft"];

fn words(rest: &[String]) -> Result<Words, ArgvError> {
    let mut positionals = Vec::new();
    let mut options = Vec::new();
    let mut at = 0;
    while at < rest.len() {
        let word = &rest[at];
        at += 1;
        let Some(option) = word.strip_prefix("--") else {
            positionals.push(word.clone());
            continue;
        };
        if let Some((name, value)) = option.split_once('=') {
            options.push((name.to_string(), Some(value.to_string())));
        } else if FLAGS.contains(&option) {
            options.push((option.to_string(), None));
        } else {
            let value = rest
                .get(at)
                .ok_or_else(|| ArgvError::Invalid(format!("--{option} needs a value")))?;
            at += 1;
            options.push((option.to_string(), Some(value.clone())));
        }
    }
    Ok(Words {
        positionals,
        options,
    })
}

impl Words {
    /// Refuse any option outside `allowed`, and a repeat of any but
    /// `repeats`.
    fn only(&self, allowed: &[&str], repeats: &[&str]) -> Result<(), ArgvError> {
        for (at, (name, _)) in self.options.iter().enumerate() {
            if !allowed.contains(&name.as_str()) {
                return Err(ArgvError::Invalid(format!("--{name} isn't taken here")));
            }
            if !repeats.contains(&name.as_str())
                && self.options[..at]
                    .iter()
                    .any(|(earlier, _)| earlier == name)
            {
                return Err(ArgvError::Invalid(format!("--{name} is given twice")));
            }
        }
        Ok(())
    }

    fn one(&self, name: &str) -> Option<&str> {
        self.options
            .iter()
            .find(|(option, _)| option == name)
            .and_then(|(_, value)| value.as_deref())
    }

    fn all(&self, name: &str) -> impl Iterator<Item = &str> {
        self.options
            .iter()
            .filter(move |(option, _)| option == name)
            .filter_map(|(_, value)| value.as_deref())
    }

    fn flag(&self, name: &str) -> bool {
        self.options.iter().any(|(option, _)| option == name)
    }

    /// The single positional `N`.
    fn number(&self) -> Result<u64, ArgvError> {
        match self.positionals.as_slice() {
            [n] => action::number(n).map_err(invalid),
            _ => Err(ArgvError::Invalid("name one issue number".into())),
        }
    }

    fn no_positionals(&self) -> Result<(), ArgvError> {
        if self.positionals.is_empty() {
            Ok(())
        } else {
            Err(ArgvError::Invalid(format!(
                "{} isn't taken here",
                self.positionals[0]
            )))
        }
    }
}

fn invalid(why: &str) -> ArgvError {
    ArgvError::Invalid(why.to_string())
}

fn required<'a>(value: Option<&'a str>, name: &str) -> Result<&'a str, ArgvError> {
    value.ok_or_else(|| ArgvError::Invalid(format!("--{name} is required")))
}

impl Action {
    /// Read `argv` (without `openagents`) into an action. `repository` is
    /// the repository to use when the words name none (the chat's
    /// project, or the checkout's).
    ///
    /// # Errors
    ///
    /// [`ArgvError`]: not a GitHub command, no repository, or a word or
    /// value the command doesn't take.
    pub fn from_argv(argv: &[String], repository: Option<&str>) -> Result<Action, ArgvError> {
        let command = path(argv).ok_or(ArgvError::NotGithub)?;
        let words = words(&argv[2..])?;
        let repository = match words.one("repo").or(repository) {
            Some(value) => action::repository(value).map_err(invalid)?,
            None => return Err(ArgvError::NeedsRepository),
        };
        let action = match command {
            "issue create" => {
                words.only(
                    &["title", "body", "label", "project", "status", "repo"],
                    &["label"],
                )?;
                words.no_positionals()?;
                let labels: Vec<String> = words
                    .all("label")
                    .flat_map(|value| value.split(','))
                    .map(str::trim)
                    .filter(|label| !label.is_empty())
                    .map(str::to_string)
                    .collect();
                if labels.len() > MAX_LABELS || !labels.iter().all(|l| action::label_ok(l)) {
                    return Err(invalid(
                        "labels are names of at most 50 characters, ten at most",
                    ));
                }
                let board = match (words.one("project"), words.one("status")) {
                    (None, None) => None,
                    (Some(board), Some(status)) => {
                        action::optional_board(board, status).map_err(invalid)?
                    }
                    (Some(_), None) => return Err(invalid("--project needs --status here")),
                    (None, Some(_)) => return Err(invalid("--status needs --project here")),
                };
                Action::CreateIssue {
                    repository,
                    title: action::title(required(words.one("title"), "title")?)
                        .map_err(invalid)?,
                    body: action::body(words.one("body").unwrap_or_default(), false)
                        .map_err(invalid)?,
                    labels,
                    board,
                }
            }
            "issue comment" => {
                words.only(&["body", "repo"], &[])?;
                Action::Comment {
                    repository,
                    number: words.number()?,
                    body: action::body(required(words.one("body"), "body")?, true)
                        .map_err(invalid)?,
                }
            }
            "issue close" => {
                words.only(&["reason", "comment", "repo"], &[])?;
                let reason = match words.one("reason") {
                    None => CloseReason::Completed,
                    Some(word) => CloseReason::parse(word)
                        .ok_or_else(|| invalid("--reason is completed or not_planned"))?,
                };
                let comment = action::body(words.one("comment").unwrap_or_default(), false)
                    .map_err(invalid)?;
                Action::CloseIssue {
                    repository,
                    number: words.number()?,
                    comment: Some(comment).filter(|c| !c.is_empty()),
                    reason,
                }
            }
            "project move" | "project add" => {
                words.only(&["project", "status", "repo"], &[])?;
                let board = action::optional_board(
                    required(words.one("project"), "project")?,
                    required(words.one("status"), "status")?,
                )
                .map_err(invalid)?
                .ok_or_else(|| invalid("--project is a board number"))?;
                Action::MoveOnBoard {
                    repository,
                    number: words.number()?,
                    board,
                }
            }
            "pr open" => {
                words.only(&["head", "base", "title", "body", "draft", "repo"], &[])?;
                words.no_positionals()?;
                Action::OpenPullRequest {
                    repository,
                    head: action::branch(required(words.one("head"), "head")?, true)
                        .map_err(invalid)?
                        .ok_or_else(|| invalid("--head is required"))?,
                    base: action::branch(words.one("base").unwrap_or_default(), false)
                        .map_err(invalid)?,
                    title: action::title(required(words.one("title"), "title")?)
                        .map_err(invalid)?,
                    body: action::body(words.one("body").unwrap_or_default(), false)
                        .map_err(invalid)?,
                    draft: words.flag("draft"),
                }
            }
            _ => return Err(ArgvError::NotGithub),
        };
        if action.valid() {
            Ok(action)
        } else {
            Err(invalid("a value is out of bounds"))
        }
    }

    /// The command words (without `openagents`) that do this, always with
    /// `--repo`: [`Action::from_argv`] reads them back to this action.
    #[must_use]
    pub fn argv(&self) -> Vec<String> {
        let mut words: Vec<String> = Vec::new();
        let head: Vec<String> = match self {
            Action::CreateIssue {
                title,
                body,
                labels,
                board,
                ..
            } => {
                push(&mut words, "title", title);
                if !body.is_empty() {
                    push(&mut words, "body", body);
                }
                for label in labels {
                    push(&mut words, "label", label);
                }
                if let Some(board) = board {
                    push(&mut words, "project", &board.number.to_string());
                    push(&mut words, "status", &board.status);
                }
                vec!["issue".into(), "create".into()]
            }
            Action::Comment { number, body, .. } => {
                push(&mut words, "body", body);
                vec!["issue".into(), "comment".into(), number.to_string()]
            }
            Action::CloseIssue {
                number,
                comment,
                reason,
                ..
            } => {
                if *reason != CloseReason::Completed {
                    push(&mut words, "reason", reason.word());
                }
                if let Some(comment) = comment {
                    push(&mut words, "comment", comment);
                }
                vec!["issue".into(), "close".into(), number.to_string()]
            }
            Action::MoveOnBoard {
                number,
                board: Board { number: b, status },
                ..
            } => {
                push(&mut words, "status", status);
                push(&mut words, "project", &b.to_string());
                vec!["project".into(), "move".into(), number.to_string()]
            }
            Action::OpenPullRequest {
                head,
                base,
                title,
                body,
                draft,
                ..
            } => {
                push(&mut words, "head", head);
                if let Some(base) = base {
                    push(&mut words, "base", base);
                }
                push(&mut words, "title", title);
                if !body.is_empty() {
                    push(&mut words, "body", body);
                }
                if *draft {
                    words.push("--draft".into());
                }
                vec!["pr".into(), "open".into()]
            }
        };
        let mut argv = head;
        argv.append(&mut words);
        argv.push("--repo".into());
        argv.push(self.repository().to_string());
        argv
    }
}

/// `--name value`, onto `words`.
fn push(words: &mut Vec<String>, name: &str, value: &str) {
    words.push(format!("--{name}"));
    words.push(value.to_string());
}
