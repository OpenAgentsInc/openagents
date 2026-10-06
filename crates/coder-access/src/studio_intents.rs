//! What a person asks the studio to do from a view, as the
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
//! studio's intents. [`complete`] offers what Tab completes a line to:
//! seats, commands, decision identities, tasks, and repositories.

use crate::review::TaskReview;
use crate::studio::{Decision, DecisionKind, MergeDecision, Task, Verdict, View};
use crate::{Operation, Right};
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
    /// Plan a failed or cancelled task again, under a new identity.
    Retry(String),
    /// Move a planned task ahead of its goal's other planned tasks.
    Prioritize(String),
    /// Cancel a planned or running task.
    Cancel(String),
    /// Give a planned task to another seat.
    Reassign {
        task: String,
        seat: String,
    },
    /// Answer the open decision `decision` at `based_on`.
    Answer {
        decision: String,
        based_on: u64,
        text: String,
    },
    /// Approve the open approval `decision` at `based_on` and ask the
    /// host to keep the standing rule `rule`, the exact text it offered:
    /// **Always allow for this seat**.
    AllowAlways {
        decision: String,
        based_on: u64,
        rule: String,
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
            Self::Retry(task) => Operation::RetryTask { task },
            Self::Prioritize(task) => Operation::PrioritizeTask { task },
            Self::Cancel(task) => Operation::CancelStudioTask { task },
            Self::Reassign { task, seat } => Operation::ReassignTask { task, seat },
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
            Self::AllowAlways {
                decision,
                based_on,
                rule,
            } => Operation::AllowAlways {
                decision,
                based_on,
                rule,
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

    /// What sending this does, in a few words, for the acknowledgment the
    /// composer shows in place of the sent text.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::SubmitGoal { workspace, .. } => format!("new goal on {workspace}"),
            Self::Message {
                seat: Some(seat), ..
            } => format!("message to {seat}"),
            Self::Message { seat: None, .. } => "message to every seat".into(),
            Self::Pause(seat) => format!("pause {seat}"),
            Self::Resume(seat) => format!("resume {seat}"),
            Self::Stop(seat) => format!("stop {seat}"),
            Self::Retry(task) => format!("retry {}", short(task)),
            Self::Prioritize(task) => format!("prioritize {}", short(task)),
            Self::Cancel(task) => format!("cancel {}", short(task)),
            Self::Reassign { task, seat } => format!("reassign {} to {seat}", short(task)),
            Self::Answer { decision, .. } => format!("answer {}", short(decision)),
            Self::AllowAlways { decision, .. } => {
                format!("always allow {} for its seat", short(decision))
            }
            Self::Decide { verdict, .. } => match verdict {
                Verdict::Merge => "merge".into(),
                Verdict::RequestChanges => "request changes".into(),
                Verdict::Reject => "reject".into(),
            },
            Self::ListWorkspaces => "list the repositories".into(),
        }
    }
}

/// At most the first 16 characters of an identity, for display.
fn short(id: &str) -> &str {
    id.char_indices().nth(16).map_or(id, |(end, _)| &id[..end])
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

/// How soon a decision of `kind` comes up: a seat blocked on an approval
/// first, then a seat's question, then a goal's plan decision.
fn urgency(kind: DecisionKind) -> u8 {
    match kind {
        DecisionKind::Approval => 0,
        DecisionKind::Question => 1,
        DecisionKind::InvalidPlan
        | DecisionKind::NoPlan
        | DecisionKind::LeadFailed
        | DecisionKind::DependencyFailed => 2,
    }
}

/// The open decisions in the order the podium takes them: approvals, then
/// questions, then goal decisions, and within each, oldest first by when
/// their goal was submitted, then by the asking task's place on the
/// goal's board. The podium answers the first unless the person moves to
/// another.
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
        urgency(a.kind)
            .cmp(&urgency(b.kind))
            .then(submitted(&a.goal).cmp(&submitted(&b.goal)))
            .then(position(a.task.as_deref()).cmp(&position(b.task.as_deref())))
    });
    open
}

/// The name a task goes by at the console: its plan entry when no other
/// task in the view shares it, else its identity.
#[must_use]
pub fn task_name<'a>(view: &View, task: &'a Task) -> &'a str {
    let shared = view
        .tasks
        .iter()
        .filter(|other| other.entry == task.entry)
        .count();
    if shared == 1 { &task.entry } else { &task.task }
}

/// The task `word` names: its identity, its plan entry when only one task
/// has it, or the start of exactly one identity.
///
/// # Errors
/// Why `word` names no task, or more than one.
pub fn find_task<'a>(view: &'a View, word: &str) -> Result<&'a Task, String> {
    if word.is_empty() {
        return Err("Name a task after /task.".into());
    }
    if let Some(task) = view.tasks.iter().find(|task| task.task == word) {
        return Ok(task);
    }
    let by_entry: Vec<&Task> = view.tasks.iter().filter(|t| t.entry == word).collect();
    let found = if by_entry.is_empty() {
        view.tasks
            .iter()
            .filter(|task| task.task.starts_with(word))
            .collect()
    } else {
        by_entry
    };
    match found.as_slice() {
        [one] => Ok(one),
        [] => Err(format!("No task is named {word}.")),
        many => Err(format!(
            "{word} names {} tasks; Tab completes their identities.",
            many.len()
        )),
    }
}

/// What a line typed at the console asks for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Console {
    /// Send this intent.
    Act(Action),
    /// `/spawn @seat [text]`: resume the seat, and message it `text`.
    Spawn { seat: String, text: Option<String> },
    /// Plain text with several repositories and none picked: the person
    /// picks the goal's repository first.
    PickRepository(String),
    /// `/diff`: open the diff review.
    Review,
    /// `/decide`: open the decisions at the podium.
    Decide,
    /// `/status`: say what the studio is doing.
    Status,
    /// `/help`: say what the console reads.
    Help,
    /// `/clear`: forget what the console said.
    Clear,
    /// `/sound on` or `/sound off`.
    Sound(bool),
    /// `/repo LABEL`: start later goals on this repository.
    Repository(String),
    /// The line asks for nothing the console can do; the text says why.
    Refused(String),
}

/// The console's help text.
pub const HELP: &str = "Plain text starts a goal; `@seat text` messages a seat, and \
     `@everyone text` every seat; `/answer [decision] text` answers a decision, the \
     first waiting without one; `/pause`, `/resume`, `/stop`, and `/spawn @seat [text]` \
     take a seat; `/task TASK cancel|retry|prioritize|reassign @seat` changes a task; \
     `/decide` opens the decisions and `/diff` the review; `/status` sums up; `/repos` \
     lists the repositories and `/repo LABEL` picks one; `/clear` and `/sound on|off`. \
     Tab completes, Up and Down walk the history, and Shift+Enter adds a line.";

/// The console's commands, for completion.
pub const COMMANDS: [&str; 14] = [
    "answer", "clear", "decide", "diff", "help", "pause", "repo", "repos", "resume", "sound",
    "spawn", "status", "stop", "task",
];

/// Reads `line`, typed at the console, against the studio `view`. A goal
/// starts on `workspace` when one is picked, else on the view's only
/// repository; with several, the person picks one first.
#[must_use]
pub fn console(line: &str, view: &View, workspace: Option<&str>) -> Console {
    let line = line.trim();
    if line.is_empty() {
        return Console::Refused("Type a goal, `@seat text`, or a command.".into());
    }
    let seat = |name: &str| -> Result<String, Console> {
        let name = name.trim_start_matches('@');
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
            "answer" => answer(argument, view),
            "pause" => seat_action(Action::Pause),
            "resume" => seat_action(Action::Resume),
            "stop" => seat_action(Action::Stop),
            "spawn" => {
                let (name, text) = argument
                    .split_once(char::is_whitespace)
                    .unwrap_or((argument, ""));
                match seat(name) {
                    Ok(seat) => Console::Spawn {
                        seat,
                        text: Some(text.trim().to_owned()).filter(|text| !text.is_empty()),
                    },
                    Err(refused) => refused,
                }
            }
            "task" => task(argument, view),
            "diff" => Console::Review,
            "decide" => Console::Decide,
            "status" => Console::Status,
            "help" => Console::Help,
            "clear" => Console::Clear,
            "sound" => match argument {
                "on" => Console::Sound(true),
                "off" => Console::Sound(false),
                _ => Console::Refused("Write /sound on or /sound off.".into()),
            },
            "repos" => Console::Act(Action::ListWorkspaces),
            "repo" if !argument.is_empty() => Console::Repository(argument.into()),
            "repo" => Console::Refused("Name the repository after /repo.".into()),
            other => Console::Refused(format!("No command is named /{other}.")),
        };
    }
    let workspace = workspace
        .map(str::to_owned)
        .or_else(|| match view.repositories.as_slice() {
            [only] => Some(only.workspace.clone()),
            _ => None,
        });
    match workspace {
        Some(workspace) => Console::Act(Action::SubmitGoal {
            text: line.into(),
            workspace,
        }),
        None if view.repositories.len() > 1 => Console::PickRepository(line.into()),
        None => Console::Refused(
            "Pick a repository first: `/repos` lists them and `/repo LABEL` picks one.".into(),
        ),
    }
}

/// `/answer [decision] text`: the named open decision, or the first the
/// podium takes.
fn answer(argument: &str, view: &View) -> Console {
    let (first, rest) = argument
        .split_once(char::is_whitespace)
        .unwrap_or((argument, ""));
    let named = view.decisions.iter().find(|open| open.decision == first);
    let (open, text) = match named {
        Some(open) => (Some(open), rest.trim()),
        None => (decisions(view).into_iter().next(), argument),
    };
    let Some(open) = open else {
        return Console::Refused("No decision waits on you.".into());
    };
    if text.is_empty() {
        return Console::Refused("Write the answer after /answer.".into());
    }
    Console::Act(Action::Answer {
        decision: open.decision.clone(),
        based_on: open.based_on,
        text: text.into(),
    })
}

/// `/task TASK cancel|retry|prioritize|reassign @seat`.
fn task(argument: &str, view: &View) -> Console {
    let mut words = argument.split_whitespace();
    let Some(name) = words.next() else {
        return Console::Refused(
            "Write /task, a task, and cancel, retry, prioritize, or reassign @seat.".into(),
        );
    };
    let task = match find_task(view, name) {
        Ok(task) => task.task.clone(),
        Err(why) => return Console::Refused(why),
    };
    match words.next() {
        Some("cancel") => Console::Act(Action::Cancel(task)),
        Some("retry") => Console::Act(Action::Retry(task)),
        Some("prioritize") => Console::Act(Action::Prioritize(task)),
        Some("reassign") => match words.next().map(|seat| seat.trim_start_matches('@')) {
            Some(seat) if view.seats.iter().any(|s| s.seat == seat) => {
                Console::Act(Action::Reassign {
                    task,
                    seat: seat.to_owned(),
                })
            }
            Some(seat) => Console::Refused(format!("No seat is named {seat}.")),
            None => Console::Refused("Name the seat after reassign.".into()),
        },
        Some(other) => Console::Refused(format!(
            "/task does not {other}: write cancel, retry, prioritize, or reassign @seat."
        )),
        None => Console::Refused("Write cancel, retry, prioritize, or reassign @seat.".into()),
    }
}

/// The lines Tab completes `line` to, in order: the last word completed as
/// a seat after `@` or a seat command, a command after `/`, a task after
/// `/task`, a task change after the task, a decision after `/answer`, or
/// a repository after `/repo`. Empty when nothing completes it.
#[must_use]
pub fn complete(line: &str, view: &View) -> Vec<String> {
    let (head, word) = match line.rfind(char::is_whitespace) {
        Some(at) => line.split_at(at + 1),
        None => ("", line),
    };
    let words: Vec<&str> = head.split_whitespace().collect();
    let seats = || view.seats.iter().map(|seat| seat.seat.clone());
    let options: Vec<String> = match words.as_slice() {
        [] if word.starts_with('@') => seats()
            .chain(std::iter::once("everyone".to_owned()))
            .map(|name| format!("@{name}"))
            .collect(),
        [] if word.starts_with('/') => COMMANDS.iter().map(|c| format!("/{c}")).collect(),
        ["/pause" | "/resume" | "/stop" | "/spawn"] => {
            seats().map(|name| format!("@{name}")).collect()
        }
        ["/task"] => view
            .tasks
            .iter()
            .map(|task| task_name(view, task).to_owned())
            .collect(),
        ["/task", _] => ["cancel", "prioritize", "reassign", "retry"]
            .iter()
            .map(|verb| (*verb).to_owned())
            .collect(),
        ["/task", _, "reassign"] => seats().map(|name| format!("@{name}")).collect(),
        ["/answer"] => decisions(view)
            .into_iter()
            .map(|open| open.decision.clone())
            .collect(),
        ["/repo"] => view
            .repositories
            .iter()
            .map(|repository| repository.workspace.clone())
            .collect(),
        ["/sound"] => vec!["off".into(), "on".into()],
        _ => Vec::new(),
    };
    let mut lines: Vec<String> = options
        .into_iter()
        .filter(|option| option.starts_with(word))
        .map(|option| format!("{head}{option}"))
        .collect();
    lines.dedup();
    lines
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
    use crate::studio::{Activity, Goal, GoalStatus, Repository, Role, Seat, Station};

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
            approval: None,
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
            completeness: crate::review::Completeness::Complete,
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

    fn task(id: &str, entry: &str, status: crate::studio::TaskStatus) -> Task {
        Task {
            spend: Default::default(),
            task: id.into(),
            goal: "g-old".into(),
            entry: entry.into(),
            position: 1,
            title: format!("Do {entry}"),
            seat: "ada".into(),
            depends_on: Vec::new(),
            status,
        }
    }

    fn busy() -> View {
        use crate::studio::TaskStatus;
        let mut view = view();
        view.tasks = vec![
            task("studio-g-old-a", "a", TaskStatus::Failed),
            task("studio-g-old-lead", "lead", TaskStatus::Done),
            task("studio-g-new-lead", "lead", TaskStatus::Running),
        ];
        view
    }

    #[test]
    fn approvals_come_before_questions_and_goal_decisions() {
        let mut view = view();
        let asked = |id: &str, kind: DecisionKind| Decision {
            decision: id.into(),
            goal: "g-new".into(),
            task: Some(id.into()),
            seat: Some("ada".into()),
            kind,
            text: "May I?".into(),
            based_on: 1,
            approval: None,
        };
        view.decisions.push(asked("q-new", DecisionKind::Question));
        view.decisions.push(asked("p-new", DecisionKind::Approval));
        let order: Vec<&str> = decisions(&view)
            .into_iter()
            .map(|open| open.decision.as_str())
            .collect();
        assert_eq!(order, ["p-new", "q-new", "g-old", "g-new"]);
        // `/answer` takes the first, or the one it names.
        assert!(matches!(
            console("/answer yes", &view, None),
            Console::Act(Action::Answer { decision, .. }) if decision == "p-new"
        ));
        assert!(matches!(
            console("/answer g-new Use this plan", &view, None),
            Console::Act(Action::Answer { decision, text, .. })
                if decision == "g-new" && text == "Use this plan"
        ));
    }

    #[test]
    fn the_console_changes_tasks_and_spawns_seats() {
        let view = busy();
        assert_eq!(
            console("/task a retry", &view, None),
            Console::Act(Action::Retry("studio-g-old-a".into()))
        );
        assert_eq!(
            console("/task studio-g-new cancel", &view, None),
            Console::Act(Action::Cancel("studio-g-new-lead".into()))
        );
        assert_eq!(
            console("/task a reassign @lead", &view, None),
            Console::Act(Action::Reassign {
                task: "studio-g-old-a".into(),
                seat: "lead".into()
            })
        );
        assert_eq!(
            console("/task a prioritize", &view, None),
            Console::Act(Action::Prioritize("studio-g-old-a".into()))
        );
        // Two leads share the entry, so it names neither.
        assert!(matches!(
            console("/task lead cancel", &view, None),
            Console::Refused(_)
        ));
        assert!(matches!(
            console("/task a reassign @grace", &view, None),
            Console::Refused(_)
        ));
        assert!(matches!(
            console("/task a", &view, None),
            Console::Refused(_)
        ));
        assert_eq!(
            console("/spawn @ada Pick up the docs", &view, None),
            Console::Spawn {
                seat: "ada".into(),
                text: Some("Pick up the docs".into())
            }
        );
        assert_eq!(
            console("/spawn ada", &view, None),
            Console::Spawn {
                seat: "ada".into(),
                text: None
            }
        );
        assert_eq!(
            console("/pause @ada", &view, None),
            Console::Act(Action::Pause("ada".into()))
        );
        assert_eq!(console("/decide", &view, None), Console::Decide);
        assert_eq!(console("/clear", &view, None), Console::Clear);
        assert_eq!(console("/help", &view, None), Console::Help);
        assert_eq!(console("/sound off", &view, None), Console::Sound(false));
        assert!(matches!(
            console("/sound loud", &view, None),
            Console::Refused(_)
        ));
    }

    #[test]
    fn a_goal_with_several_repositories_asks_which_first() {
        let mut view = view();
        view.repositories.push(Repository {
            workspace: "site".into(),
            goals: 0,
            open_tasks: 0,
        });
        assert_eq!(
            console("Add a flag", &view, None),
            Console::PickRepository("Add a flag".into())
        );
        assert_eq!(
            console("Add a flag", &view, Some("site")),
            Console::Act(Action::SubmitGoal {
                text: "Add a flag".into(),
                workspace: "site".into()
            })
        );
    }

    #[test]
    fn tab_completes_seats_commands_tasks_and_decisions() {
        let view = busy();
        assert_eq!(complete("@a", &view), ["@ada"]);
        assert_eq!(complete("@", &view), ["@ada", "@lead", "@everyone"]);
        assert_eq!(complete("/st", &view), ["/status", "/stop"]);
        assert_eq!(complete("/pause @l", &view), ["/pause @lead"]);
        assert_eq!(
            complete("/task ", &view),
            [
                "/task a",
                "/task studio-g-old-lead",
                "/task studio-g-new-lead"
            ]
        );
        assert_eq!(
            complete("/task a re", &view),
            ["/task a reassign", "/task a retry"]
        );
        assert_eq!(
            complete("/task a reassign @", &view),
            ["/task a reassign @ada", "/task a reassign @lead"]
        );
        assert_eq!(complete("/answer g-o", &view), ["/answer g-old"]);
        assert_eq!(complete("/repo a", &view), ["/repo app"]);
        assert!(complete("plain words", &view).is_empty());
        assert!(complete("", &view).is_empty());
    }

    #[test]
    fn task_changes_name_their_operations() {
        for action in [
            Action::Retry("studio-g-old-a".into()),
            Action::Prioritize("studio-g-old-a".into()),
            Action::Cancel("studio-g-old-a".into()),
            Action::Reassign {
                task: "studio-g-old-a".into(),
                seat: "lead".into(),
            },
        ] {
            assert_eq!(action.right(), Right::Operate);
            let operation = action.operation(1_790_000_000);
            operation.validate().unwrap();
            assert_eq!(operation.required(), Some(Right::Operate));
            assert!(!action.describe().is_empty());
        }
    }
}
