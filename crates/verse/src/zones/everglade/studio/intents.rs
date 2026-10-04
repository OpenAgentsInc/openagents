//! What a person asks the studio to do from Everglade's panels, as the
//! NIP-HOST studio intents the host checks (`docs/verse/agent-studio.md`,
//! "The client is a view").
//!
//! An [`Action`] is one intent before it is sent: [`Action::operation`]
//! builds its operation, minting the command identity an answer or a merge
//! decision carries, and [`Action::right`] names the one right the host
//! requires for it. A panel offers an action only when the source's rights
//! hold that right; the host checks the device's grant again either way.
//!
//! [`console`] reads a line typed at the console: plain text starts a
//! goal, `@seat text` messages a seat, and the slash commands mirror the
//! studio's intents.

use coder_access::review::TaskReview;
use coder_access::studio::{Decision, MergeDecision, Verdict, View};
use coder_access::{Operation, Right};
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicU64, Ordering};

/// One studio intent before it is sent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// Start a goal on the admitted repository `workspace`.
    SubmitGoal {
        text: String,
        workspace: String,
    },
    /// Message one seat, or every seat when `seat` is `None`.
    Message {
        seat: Option<String>,
        text: String,
    },
    Pause(String),
    Resume(String),
    Stop(String),
    /// Answer the open decision `decision` at `based_on`.
    Answer {
        decision: String,
        based_on: u64,
        text: String,
    },
    /// **Merge**, **Request changes**, or **Reject** the change `review`
    /// read, at its revisions.
    Decide {
        review: Box<TaskReview>,
        verdict: Verdict,
        text: String,
    },
    /// List the repositories a goal may name.
    ListWorkspaces,
}

impl Action {
    /// The operation that carries this action, issued at `now` (Unix
    /// seconds). An answer and a merge decision get a fresh command
    /// identity, which a retry of the same request keeps.
    #[must_use]
    pub fn operation(&self, now: u64) -> Operation {
        match self.clone() {
            Self::SubmitGoal { text, workspace } => Operation::SubmitGoal {
                text,
                workspace,
                lead: None,
            },
            Self::Message { seat, text } => Operation::MessageSeat { seat, text },
            Self::Pause(seat) => Operation::PauseSeat { seat },
            Self::Resume(seat) => Operation::ResumeSeat { seat },
            Self::Stop(seat) => Operation::StopSeat { seat },
            Self::Answer {
                decision,
                based_on,
                text,
            } => Operation::AnswerDecision {
                decision,
                based_on,
                text,
                command: mint(),
                issued_at: now,
            },
            Self::Decide {
                review,
                verdict,
                text,
            } => Operation::DecideMerge {
                decision: Box::new(MergeDecision {
                    task: review.task,
                    base: review.base,
                    head_commit: review.head_commit,
                    head: review.head,
                    verdict,
                    text,
                    command: mint(),
                    issued_at: now,
                }),
            },
            Self::ListWorkspaces => Operation::ListWorkspaces {},
        }
    }

    /// The one right the host requires for this action.
    #[must_use]
    pub fn right(&self) -> Right {
        match self {
            Self::Decide { .. } => Right::Review,
            _ => Right::Operate,
        }
    }
}

/// Whether `rights` hold `right`.
#[must_use]
pub fn allows(rights: &[Right], right: Right) -> bool {
    rights.contains(&right)
}

/// A fresh 64-hex identity: a NIP-HOST request ID, or a command ID an
/// answer or a merge decision carries. Unique within this process and,
/// through the time and process ID it digests, across processes.
#[must_use]
pub fn mint() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    let mut hash = Sha256::new();
    hash.update(b"verse-studio-identity");
    hash.update(nanos.to_be_bytes());
    hash.update(std::process::id().to_be_bytes());
    hash.update(NEXT.fetch_add(1, Ordering::Relaxed).to_be_bytes());
    hash.finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Seconds since the epoch, for an intent's `issued_at`.
#[must_use]
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

/// The open decisions, oldest first: by when their goal was submitted,
/// then by the asking task's place on the goal's board. The podium answers
/// the first.
#[must_use]
pub fn decisions(view: &View) -> Vec<&Decision> {
    let submitted = |goal: &str| {
        view.goals
            .iter()
            .find(|g| g.goal == goal)
            .map_or(0, |g| g.submitted_at)
    };
    let position = |task: Option<&str>| {
        task.and_then(|id| view.tasks.iter().find(|t| t.task == id))
            .map_or(0, |t| t.position)
    };
    let mut open: Vec<&Decision> = view.decisions.iter().collect();
    open.sort_by(|a, b| {
        submitted(&a.goal)
            .cmp(&submitted(&b.goal))
            .then(position(a.task.as_deref()).cmp(&position(b.task.as_deref())))
    });
    open
}

/// What a line typed at the console asks for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Console {
    /// Send this intent.
    Act(Action),
    /// `/diff`: open the diff review.
    Review,
    /// `/status`: say what the studio is doing.
    Status,
    /// `/repo LABEL`: start later goals on this repository.
    Repository(String),
    /// The line asks for nothing the console can do; the text says why.
    Refused(String),
}

/// The console's help text.
pub const HELP: &str = "Plain text starts a goal; `@seat text` messages a seat, and \
     `@everyone text` every seat; `/answer text` answers the oldest decision; \
     `/pause`, `/resume`, and `/stop` take a seat; `/diff` opens the review; \
     `/status` sums up; `/repos` lists the repositories and `/repo LABEL` picks one.";

/// Reads `line`, typed at the console, against the studio `view`. A goal
/// starts on `workspace` when one is picked, else on the view's first
/// repository.
#[must_use]
pub fn console(line: &str, view: &View, workspace: Option<&str>) -> Console {
    let line = line.trim();
    if line.is_empty() {
        return Console::Refused("Type a goal, `@seat text`, or a command.".into());
    }
    let seat = |name: &str| -> Result<String, Console> {
        if view.seats.iter().any(|s| s.seat == name) {
            Ok(name.to_owned())
        } else if name.is_empty() {
            Err(Console::Refused("Name a seat.".into()))
        } else {
            Err(Console::Refused(format!("No seat is named {name}.")))
        }
    };
    if let Some(rest) = line.strip_prefix('@') {
        let (name, text) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
        let text = text.trim();
        if text.is_empty() {
            return Console::Refused(format!("Write a message after @{name}."));
        }
        let to = if matches!(name, "everyone" | "all") {
            None
        } else {
            match seat(name) {
                Ok(name) => Some(name),
                Err(refused) => return refused,
            }
        };
        return Console::Act(Action::Message {
            seat: to,
            text: text.into(),
        });
    }
    if let Some(rest) = line.strip_prefix('/') {
        let (word, argument) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
        let argument = argument.trim();
        let seat_action = |make: fn(String) -> Action| match seat(argument) {
            Ok(name) => Console::Act(make(name)),
            Err(refused) => refused,
        };
        return match word {
            "answer" => {
                let Some(oldest) = decisions(view).into_iter().next() else {
                    return Console::Refused("No decision waits on you.".into());
                };
                if argument.is_empty() {
                    return Console::Refused("Write the answer after /answer.".into());
                }
                Console::Act(Action::Answer {
                    decision: oldest.decision.clone(),
                    based_on: oldest.based_on,
                    text: argument.into(),
                })
            }
            "pause" => seat_action(Action::Pause),
            "resume" => seat_action(Action::Resume),
            "stop" => seat_action(Action::Stop),
            "diff" => Console::Review,
            "status" => Console::Status,
            "repos" => Console::Act(Action::ListWorkspaces),
            "repo" if !argument.is_empty() => Console::Repository(argument.into()),
            "repo" => Console::Refused("Name the repository after /repo.".into()),
            other => Console::Refused(format!("No command is named /{other}.")),
        };
    }
    let workspace = workspace.map(str::to_owned).or_else(|| {
        view.repositories
            .first()
            .map(|repository| repository.workspace.clone())
    });
    match workspace {
        Some(workspace) => Console::Act(Action::SubmitGoal {
            text: line.into(),
            workspace,
        }),
        None => Console::Refused(
            "Pick a repository first: `/repos` lists them and `/repo LABEL` picks one.".into(),
        ),
    }
}

/// One line that sums up the studio, for `/status`.
#[must_use]
pub fn status(view: &View) -> String {
    let working = view
        .seats
        .iter()
        .filter(|seat| seat.task.is_some() && !seat.paused)
        .count();
    let open = view
        .tasks
        .iter()
        .filter(|task| !task.status.is_final())
        .count();
    format!(
        "{} goals · {} of {} seats working · {} open tasks · {} decisions waiting",
        view.goals.len(),
        working,
        view.seats.len(),
        open,
        view.decisions.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use coder_access::studio::{
        Activity, DecisionKind, Goal, GoalStatus, Repository, Role, Seat, Station,
    };

    fn view() -> View {
        let seat = |name: &str| Seat {
            seat: name.into(),
            role: Role::Worker,
            route: "codex:gpt-6".into(),
            look: "default".into(),
            desk: 0,
            activity: Activity::Idle,
            station: Station::Desk,
            task: None,
            paused: false,
            spend: Default::default(),
        };
        let goal = |id: &str, at: u64| Goal {
            goal: id.into(),
            text: "Add a flag".into(),
            workspace: "app".into(),
            lead: "lead".into(),
            status: GoalStatus::Decision,
            final_tasks: 0,
            total_tasks: 0,
            submitted_at: at,
            spend: Default::default(),
        };
        let decision = |goal: &str, based_on: u64| Decision {
            decision: goal.into(),
            goal: goal.into(),
            task: None,
            seat: None,
            kind: DecisionKind::NoPlan,
            text: "Answer with a plan.".into(),
            based_on,
        };
        View {
            goals: vec![goal("g-new", 20), goal("g-old", 10)],
            seats: vec![seat("ada"), seat("lead")],
            decisions: vec![decision("g-new", 2), decision("g-old", 1)],
            repositories: vec![Repository {
                workspace: "app".into(),
                goals: 2,
                open_tasks: 0,
            }],
            ..View::default()
        }
    }

    #[test]
    fn the_console_reads_goals_messages_and_commands() {
        let view = view();
        assert_eq!(
            console("Add a --verbose flag", &view, None),
            Console::Act(Action::SubmitGoal {
                text: "Add a --verbose flag".into(),
                workspace: "app".into()
            })
        );
        assert_eq!(
            console("Ship it", &view, Some("site")),
            Console::Act(Action::SubmitGoal {
                text: "Ship it".into(),
                workspace: "site".into()
            })
        );
        assert_eq!(
            console("@ada  keep it short", &view, None),
            Console::Act(Action::Message {
                seat: Some("ada".into()),
                text: "keep it short".into()
            })
        );
        assert_eq!(
            console("@everyone stop and read", &view, None),
            Console::Act(Action::Message {
                seat: None,
                text: "stop and read".into()
            })
        );
        assert!(matches!(
            console("@grace hi", &view, None),
            Console::Refused(_)
        ));
        assert_eq!(
            console("/pause ada", &view, None),
            Console::Act(Action::Pause("ada".into()))
        );
        assert_eq!(console("/diff", &view, None), Console::Review);
        assert_eq!(
            console("/repo site", &view, None),
            Console::Repository("site".into())
        );
        assert_eq!(
            console("/repos", &view, None),
            Console::Act(Action::ListWorkspaces)
        );
        assert!(matches!(
            console("/dance", &view, None),
            Console::Refused(_)
        ));
        // `/answer` answers the oldest goal's decision at its own point.
        assert_eq!(
            console("/answer Use the plan below", &view, None),
            Console::Act(Action::Answer {
                decision: "g-old".into(),
                based_on: 1,
                text: "Use the plan below".into()
            })
        );
        // Without a repository, a goal says how to pick one.
        let empty = View::default();
        assert!(matches!(
            console("A goal", &empty, None),
            Console::Refused(_)
        ));
    }

    #[test]
    fn each_action_names_its_operation_and_right() {
        let review = TaskReview {
            task: "studio-g1-a".into(),
            base: "a".repeat(40),
            head_commit: "a".repeat(40),
            head: "b".repeat(40),
            files: Vec::new(),
            files_total: 0,
            added: 0,
            removed: 0,
            uncounted: 0,
            diff: String::new(),
            completeness: coder_access::review::Completeness::Complete,
            publication: None,
        };
        let decide = Action::Decide {
            review: Box::new(review),
            verdict: Verdict::RequestChanges,
            text: "Name the flag.".into(),
        };
        assert_eq!(decide.right(), Right::Review);
        let operation = decide.operation(1_790_000_000);
        operation.validate().unwrap();
        assert_eq!(operation.required(), Some(Right::Review));
        let answer = Action::Answer {
            decision: "g-old".into(),
            based_on: 1,
            text: "Yes".into(),
        };
        assert_eq!(answer.right(), Right::Operate);
        let operation = answer.operation(1_790_000_000);
        operation.validate().unwrap();
        assert_eq!(operation.required(), Some(Right::Operate));
        // Two answers never share a command identity.
        assert_ne!(answer.operation(1), answer.operation(1));
        assert!(allows(&[Right::Observe, Right::Operate], Right::Operate));
        assert!(!allows(&[Right::Observe], Right::Review));
    }
}
