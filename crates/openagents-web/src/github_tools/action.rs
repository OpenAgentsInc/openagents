//! The chat's GitHub tools as typed actions (#11167): what a tool form
//! asks for, what the confirm card says it will change, and nothing else.
//!
//! A form names its tool with one bounded value ([`Tool`]); every other
//! field is an exact value (a repository, a number, a branch, a status
//! name) checked here before anything reaches GitHub. Nothing reads a
//! message's words to pick a tool.

use serde::{Deserialize, Serialize};

/// The longest title an issue or pull request may have here.
pub(crate) const MAX_TITLE: usize = 256;
/// The longest body or comment, in characters.
pub(crate) const MAX_BODY: usize = 8_000;
/// The longest board status name.
pub(crate) const MAX_STATUS: usize = 64;
/// The longest branch name.
pub(crate) const MAX_BRANCH: usize = 255;

/// Which tool a form is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Tool {
    CreateIssue,
    Comment,
    CloseIssue,
    MoveOnBoard,
    OpenPullRequest,
}

impl Tool {
    /// Every tool, in the order the tools page lists them.
    pub(crate) const ALL: [Tool; 5] = [
        Tool::CreateIssue,
        Tool::Comment,
        Tool::CloseIssue,
        Tool::MoveOnBoard,
        Tool::OpenPullRequest,
    ];

    /// The form's `tool` value.
    pub(crate) fn key(self) -> &'static str {
        match self {
            Tool::CreateIssue => "create_issue",
            Tool::Comment => "comment",
            Tool::CloseIssue => "close_issue",
            Tool::MoveOnBoard => "move_on_board",
            Tool::OpenPullRequest => "open_pull_request",
        }
    }

    /// A form's `tool` value back, exactly.
    pub(crate) fn from_key(value: &str) -> Option<Tool> {
        Tool::ALL.into_iter().find(|tool| tool.key() == value)
    }

    /// The tool's name, as its heading and its confirm card say it.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Tool::CreateIssue => "Open an issue",
            Tool::Comment => "Comment on an issue or pull request",
            Tool::CloseIssue => "Close an issue",
            Tool::MoveOnBoard => "Move an issue on a board",
            Tool::OpenPullRequest => "Open a pull request",
        }
    }
}

/// A GitHub Projects board and the status to set there. The board belongs
/// to the repository's owner (an organization or a person), as
/// `scripts/dev/issue-board.sh` reads it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Board {
    pub number: u32,
    pub status: String,
}

/// One change to make on GitHub as the signed-in person.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "tool", rename_all = "snake_case")]
pub(crate) enum Action {
    /// Open an issue, and put it on a board when one is named.
    CreateIssue {
        repository: String,
        title: String,
        body: String,
        #[serde(default)]
        board: Option<Board>,
    },
    /// Comment on an issue or a pull request.
    Comment {
        repository: String,
        number: u64,
        body: String,
    },
    /// Close an issue as completed, commenting first when there is a
    /// comment.
    CloseIssue {
        repository: String,
        number: u64,
        #[serde(default)]
        comment: Option<String>,
    },
    /// Set an issue's (or pull request's) status on a board, adding it to
    /// the board first when it isn't there.
    MoveOnBoard {
        repository: String,
        number: u64,
        board: Board,
    },
    /// Open a pull request from `head` into `base` (the repository's
    /// default branch when `base` is `None`).
    OpenPullRequest {
        repository: String,
        head: String,
        #[serde(default)]
        base: Option<String>,
        title: String,
        body: String,
        #[serde(default)]
        draft: bool,
    },
}

/// What a tool form sends: the tool, and the fields that tool reads.
#[derive(Clone, Debug, Default, Deserialize)]
pub(crate) struct ToolForm {
    #[serde(default)]
    pub tool: String,
    #[serde(default)]
    pub csrf: String,
    #[serde(default)]
    pub repository: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub number: String,
    #[serde(default)]
    pub board: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub head: String,
    #[serde(default)]
    pub base: String,
    #[serde(default)]
    pub comment: String,
    #[serde(default)]
    pub draft: Option<String>,
}

/// What the confirm card shows before anything changes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Card {
    pub heading: &'static str,
    pub repository: String,
    /// One sentence per change, in the order they happen.
    pub changes: Vec<String>,
    /// The text that will be posted (a body or a comment), when there is
    /// one.
    pub text: Option<String>,
}

impl Action {
    /// Read a tool form into an action, or say what to fix.
    pub(crate) fn from_form(form: &ToolForm) -> Result<Action, &'static str> {
        let tool = Tool::from_key(form.tool.trim()).ok_or("Pick a tool, then try again.")?;
        let repository = repository(&form.repository)?;
        let action = match tool {
            Tool::CreateIssue => Action::CreateIssue {
                repository,
                title: title(&form.title)?,
                body: body(&form.body, false)?,
                board: optional_board(&form.board, &form.status)?,
            },
            Tool::Comment => Action::Comment {
                repository,
                number: number(&form.number)?,
                body: body(&form.body, true)?,
            },
            Tool::CloseIssue => Action::CloseIssue {
                repository,
                number: number(&form.number)?,
                comment: Some(body(&form.comment, false)?).filter(|c| !c.is_empty()),
            },
            Tool::MoveOnBoard => Action::MoveOnBoard {
                repository,
                number: number(&form.number)?,
                board: optional_board(&form.board, &form.status)?
                    .ok_or("Enter the board's number, such as 22.")?,
            },
            Tool::OpenPullRequest => Action::OpenPullRequest {
                repository,
                head: branch(&form.head, true)?.ok_or("Enter the branch with your changes.")?,
                base: branch(&form.base, false)?,
                title: title(&form.title)?,
                body: body(&form.body, false)?,
                draft: form.draft.as_deref().is_some_and(|v| !v.is_empty()),
            },
        };
        Ok(action)
    }

    /// Which tool this is.
    pub(crate) fn tool(&self) -> Tool {
        match self {
            Action::CreateIssue { .. } => Tool::CreateIssue,
            Action::Comment { .. } => Tool::Comment,
            Action::CloseIssue { .. } => Tool::CloseIssue,
            Action::MoveOnBoard { .. } => Tool::MoveOnBoard,
            Action::OpenPullRequest { .. } => Tool::OpenPullRequest,
        }
    }

    /// The repository it changes (`owner/name`).
    pub(crate) fn repository(&self) -> &str {
        match self {
            Action::CreateIssue { repository, .. }
            | Action::Comment { repository, .. }
            | Action::CloseIssue { repository, .. }
            | Action::MoveOnBoard { repository, .. }
            | Action::OpenPullRequest { repository, .. } => repository,
        }
    }

    /// Whether it changes a board, which needs GitHub's `project` access.
    pub(crate) fn needs_board(&self) -> bool {
        matches!(
            self,
            Action::CreateIssue { board: Some(_), .. } | Action::MoveOnBoard { .. }
        )
    }

    /// Whether the action, read back from a sealed card, still holds every
    /// bound a form holds (a card is signed, but bounds are cheap).
    pub(crate) fn valid(&self) -> bool {
        let board_ok = |board: &Board| board.number > 0 && status_ok(&board.status);
        oa_auth::repos::full_name(self.repository())
            && match self {
                Action::CreateIssue {
                    title, body, board, ..
                } => title_ok(title) && body_ok(body) && board.as_ref().is_none_or(board_ok),
                Action::Comment { number, body, .. } => {
                    *number > 0 && body_ok(body) && !body.trim().is_empty()
                }
                Action::CloseIssue {
                    number, comment, ..
                } => *number > 0 && comment.as_deref().is_none_or(body_ok),
                Action::MoveOnBoard { number, board, .. } => *number > 0 && board_ok(board),
                Action::OpenPullRequest {
                    head,
                    base,
                    title,
                    body,
                    ..
                } => {
                    branch_ok(head)
                        && base.as_deref().is_none_or(branch_ok)
                        && title_ok(title)
                        && body_ok(body)
                }
            }
    }

    /// What the confirm card says will change.
    pub(crate) fn card(&self) -> Card {
        let mut changes = Vec::new();
        let text = match self {
            Action::CreateIssue {
                title, body, board, ..
            } => {
                changes.push(format!("Opens a new issue titled \u{201c}{title}\u{201d}."));
                if let Some(board) = board {
                    changes.push(format!(
                        "Puts it on board {} with the status {}.",
                        board.number, board.status
                    ));
                }
                Some(body.clone()).filter(|b| !b.is_empty())
            }
            Action::Comment { number, body, .. } => {
                changes.push(format!("Adds a comment to #{number}."));
                Some(body.clone())
            }
            Action::CloseIssue {
                number, comment, ..
            } => {
                if comment.is_some() {
                    changes.push(format!("Adds a comment to #{number}."));
                }
                changes.push(format!("Closes #{number} as completed."));
                comment.clone()
            }
            Action::MoveOnBoard { number, board, .. } => {
                changes.push(format!(
                    "Sets #{number} to {} on board {}, adding it to the board if it isn't there.",
                    board.status, board.number
                ));
                None
            }
            Action::OpenPullRequest {
                head,
                base,
                title,
                body,
                draft,
                ..
            } => {
                let into = base
                    .as_deref()
                    .map_or_else(|| "the default branch".to_string(), str::to_string);
                changes.push(format!(
                    "Opens a {}pull request titled \u{201c}{title}\u{201d} from {head} into {into}.",
                    if *draft { "draft " } else { "" }
                ));
                Some(body.clone()).filter(|b| !b.is_empty())
            }
        };
        Card {
            heading: self.tool().name(),
            repository: self.repository().to_string(),
            changes,
            text,
        }
    }
}

fn repository(value: &str) -> Result<String, &'static str> {
    let value = value.trim().trim_end_matches(".git");
    let value = value
        .strip_prefix("https://github.com/")
        .unwrap_or(value)
        .trim_end_matches('/');
    if oa_auth::repos::full_name(value) {
        Ok(value.to_string())
    } else {
        Err("Enter the repository as owner/name, such as OpenAgentsInc/openagents.")
    }
}

fn title_ok(title: &str) -> bool {
    !title.trim().is_empty()
        && title.chars().count() <= MAX_TITLE
        && !title.chars().any(char::is_control)
}

fn title(value: &str) -> Result<String, &'static str> {
    let value = value.trim();
    if title_ok(value) {
        Ok(value.to_string())
    } else {
        Err("Enter a title of at most 256 characters, on one line.")
    }
}

fn body_ok(body: &str) -> bool {
    body.chars().count() <= MAX_BODY
}

fn body(value: &str, required: bool) -> Result<String, &'static str> {
    let value = value.trim().replace("\r\n", "\n");
    if required && value.is_empty() {
        return Err("Enter the comment.");
    }
    if body_ok(&value) {
        Ok(value)
    } else {
        Err("Keep the text to at most 8,000 characters.")
    }
}

fn number(value: &str) -> Result<u64, &'static str> {
    value
        .trim()
        .trim_start_matches('#')
        .parse::<u64>()
        .ok()
        .filter(|n| *n > 0)
        .ok_or("Enter the issue or pull request number, such as 11167.")
}

fn status_ok(status: &str) -> bool {
    !status.trim().is_empty()
        && status.chars().count() <= MAX_STATUS
        && !status.chars().any(char::is_control)
}

fn optional_board(board: &str, status: &str) -> Result<Option<Board>, &'static str> {
    let board = board.trim().trim_start_matches('#');
    if board.is_empty() {
        return Ok(None);
    }
    let number = board
        .parse::<u32>()
        .ok()
        .filter(|n| *n > 0)
        .ok_or("Enter the board's number, such as 22.")?;
    let status = status.trim();
    if !status_ok(status) {
        return Err("Enter the status to set, such as Todo.");
    }
    Ok(Some(Board {
        number,
        status: status.to_string(),
    }))
}

/// A branch name Git and GitHub accept, plus `owner:branch` for a head on
/// a fork.
fn branch_ok(value: &str) -> bool {
    let name = value.split_once(':').map_or(
        value,
        |(owner, name)| {
            if owner.is_empty() { "" } else { name }
        },
    );
    !name.is_empty()
        && value.len() <= MAX_BRANCH
        && !name.starts_with('-')
        && !name.starts_with('/')
        && !name.ends_with('/')
        && !name.ends_with(".lock")
        && !name.ends_with('.')
        && !name.contains("..")
        && !name.contains("@{")
        && !name.contains("//")
        && !name
            .chars()
            .any(|c| c.is_control() || c.is_whitespace() || "~^:?*[\\".contains(c))
}

fn branch(value: &str, required: bool) -> Result<Option<String>, &'static str> {
    let value = value.trim();
    if value.is_empty() {
        return if required {
            Err("Enter the branch with your changes.")
        } else {
            Ok(None)
        };
    }
    if branch_ok(value) {
        Ok(Some(value.to_string()))
    } else {
        Err("Enter a branch name as Git writes it, such as fix-login.")
    }
}
