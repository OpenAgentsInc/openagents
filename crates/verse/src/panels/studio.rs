//! The Agent Studio's panels over Everglade (`docs/verse/agent-studio.md`,
//! "The studio in Verse"): the console at the notice board, a seat's agent
//! card at its desk, the decisions at the podium, the diff review at the
//! merge station, a task's details on its Task Wall card, and shared
//! memory at the library.
//!
//! Each panel is a [`Panel`] of transcript rows in the chat palette, built
//! from the studio snapshot Everglade draws. The decisions read each
//! question or approval with the desktop app's question flow
//! (`openagents_chat_app::decision`), and the roster orders seats by the
//! app's attention value (`openagents_chat_app::attention`). The review is
//! the task's real diff in the changes tab.
//!
//! A [`Controller`] turns a panel's controls and keys into studio intents
//! ([`intents`]):
//!
//! - The console's composer starts goals and messages seats. Tab completes
//!   seats, commands, decisions, and tasks; Up and Down walk the history;
//!   Shift+Enter adds a line; and what was sent, or why the host refused
//!   it, shows in place of the draft. A refused draft comes back. With
//!   several repositories, a goal asks which one first.
//! - A seat's agent card shows its state, task, the decisions it owns or
//!   filed, and its recent log; it pauses, resumes, stops, spawns, and
//!   messages its seat.
//! - The podium takes approvals, then questions, then goal decisions,
//!   oldest first. Number keys pick options and Tab moves between
//!   decisions. Enter alone never grants, and option keys wait out a short
//!   lock after a decision comes up.
//! - The merge station decides **Merge**, **Request changes** (with line
//!   comments from `openagents_chat_app::review_comments`), or **Reject**,
//!   which asks for a second press. Keys move between files and hunks and
//!   copy a file's path.
//! - A task's details retry, prioritize, reassign, and cancel it; cancel
//!   asks for a second press.
//!
//! A panel offers a control only when the source's rights hold its right,
//! and the host checks it again. The host's newest answer, or its refusal
//! code, shows in every studio panel.

use super::{Intent, Key, Panel, Tab, message, tool};
use crate::zones::everglade::studio::intents::{self, Action, Console};
use crate::zones::everglade::studio::{Answer, PanelKind, word};
use coder_access::review::TaskReview;
use coder_access::studio::{
    Activity, Approval, Decision, DecisionKind, Memory, MemoryKind, Risk, Seat, Spend, Task,
    TaskStatus, Verdict, View,
};
use coder_access::{Code, Outcome, Right};
use openagents_chat_app::{attention, changes, decision, review_comments};
use rust_native::{MessageRole, Node, ToolState};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

/// How long option keys and Enter wait after a decision comes up at the
/// podium, so a key meant for the last one never answers the next.
pub const INPUT_LOCK: Duration = Duration::from_millis(350);
/// How long **Reject** or **Cancel** waits for its confirming press.
pub const CONFIRM: Duration = Duration::from_secs(5);
/// The most lines the console's history keeps.
pub const MAX_HISTORY: usize = 50;
/// The most decisions an agent card offers to answer.
const CARD_DECISIONS: usize = 2;

/// The attention a seat doing `activity` shows in the roster: the desktop
/// app's indicator over the seat's activity. A seat whose task is over is
/// shown as seen, so only a waiting, failed, or working seat stands out.
#[must_use]
pub fn indicator(activity: Activity) -> attention::Indicator {
    let source = match activity {
        Activity::Waiting => attention::Activity::AwaitingInput,
        Activity::Failed | Activity::Blocked => attention::Activity::Failed,
        Activity::Reading
        | Activity::Editing
        | Activity::Running
        | Activity::Testing
        | Activity::Judging
        | Activity::Thinking => attention::Activity::Working,
        Activity::Done => attention::Activity::Completed,
        Activity::Idle | Activity::Paused => attention::Activity::Idle,
    };
    attention::indicator(source, std::time::Duration::ZERO, false)
}

/// The seat a panel of `kind` shows, if it shows one.
#[must_use]
pub fn seat<'a>(kind: &PanelKind, view: &'a View) -> Option<&'a Seat> {
    match kind {
        PanelKind::Seat(name) => view.seats.iter().find(|s| s.seat == *name),
        PanelKind::Desk(desk) => view.seats.iter().find(|s| s.desk == *desk),
        _ => None,
    }
}

/// The task a panel of `kind` shows, if it shows one.
#[must_use]
pub fn task<'a>(kind: &PanelKind, view: &'a View) -> Option<&'a Task> {
    match kind {
        PanelKind::Task(id) => view.tasks.iter().find(|t| t.task == *id),
        _ => None,
    }
}

/// The panel's title.
#[must_use]
pub fn title(kind: &PanelKind, view: Option<&View>) -> String {
    match kind {
        PanelKind::Console => "Console".into(),
        PanelKind::Seat(_) | PanelKind::Desk(_) => match view.and_then(|v| seat(kind, v)) {
            Some(seat) => format!("{} · {}", seat.seat, seat.route),
            None => match kind {
                PanelKind::Desk(desk) => format!("Desk {}", desk + 1),
                _ => "Seat".into(),
            },
        },
        PanelKind::Decisions => "Decisions".into(),
        PanelKind::Review => "Diff review".into(),
        PanelKind::Task(_) => match view.and_then(|v| task(kind, v)) {
            Some(task) => format!("Task · {}", task.title),
            None => "Task".into(),
        },
        PanelKind::Library => "Library".into(),
    }
}

/// When the goal of `task` was submitted, for ordering.
fn submitted(view: &View, goal: &str) -> u64 {
    view.goals
        .iter()
        .find(|g| g.goal == goal)
        .map_or(0, |g| g.submitted_at)
}

/// The tasks the merge station can review, newest first: plan tasks that
/// are done.
#[must_use]
pub fn reviewable(view: &View) -> Vec<&Task> {
    let mut tasks: Vec<&Task> = view
        .tasks
        .iter()
        .filter(|t| t.status == TaskStatus::Done && t.entry != "lead")
        .collect();
    tasks.sort_by(|a, b| {
        submitted(view, &b.goal)
            .cmp(&submitted(view, &a.goal))
            .then(b.position.cmp(&a.position))
    });
    tasks
}

/// The word a task's status is shown with.
#[must_use]
pub fn status_word(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Held => "planned",
        TaskStatus::Blocked => "blocked",
        TaskStatus::Queued => "queued",
        TaskStatus::Running => "running",
        TaskStatus::Waiting => "waiting on you",
        TaskStatus::Done => "done",
        TaskStatus::Failed => "failed",
        TaskStatus::Cancelled => "cancelled",
        TaskStatus::Missing => "missing",
    }
}

fn note(key: &str, text: &str) -> Node<()> {
    message(key, MessageRole::System, text)
}

/// What the studio's goals in `view` spent together.
#[must_use]
pub fn spent(view: &View) -> Spend {
    view.goals
        .iter()
        .fold(Spend::default(), |sum, goal| sum.plus(goal.spend))
}

fn console(view: &View, rights: &[Right]) -> Vec<Node<()>> {
    let mut rows = Vec::new();
    if !view.goals.is_empty() {
        let goals = match view.goals.len() {
            1 => "1 goal".to_owned(),
            n => format!("{n} goals"),
        };
        rows.push(note(
            "spend",
            &format!("Spent {} across {goals}.", spent(view).label()),
        ));
    }
    let mut goals: Vec<_> = view.goals.iter().collect();
    goals.sort_by(|a, b| b.submitted_at.cmp(&a.submitted_at));
    for goal in goals {
        let status = match goal.status {
            coder_access::studio::GoalStatus::Planning => "planning",
            coder_access::studio::GoalStatus::Decision => "waiting on a decision",
            coder_access::studio::GoalStatus::Running => "running",
            coder_access::studio::GoalStatus::Done => "done",
        };
        rows.push(message(
            &format!("goal-{}", goal.goal),
            MessageRole::User,
            &format!(
                "**Goal** · {status}\n\n{}\n\n{} of {} tasks over · {} spent · lead {} · {}",
                goal.text,
                goal.final_tasks,
                goal.total_tasks,
                goal.spend.label(),
                goal.lead,
                goal.workspace
            ),
        ));
    }
    if view.goals.is_empty() {
        rows.push(note("no-goals", "No goal yet."));
    }
    let mut seats: Vec<&Seat> = view.seats.iter().collect();
    seats.sort_by_key(|s| indicator(s.activity).rank());
    for seat in seats {
        let state = match indicator(seat.activity) {
            attention::Indicator::Working | attention::Indicator::AwaitingInput => {
                ToolState::Running
            }
            attention::Indicator::Errored => ToolState::Failed,
            _ => ToolState::Done,
        };
        let label = indicator(seat.activity).label().unwrap_or("Idle");
        rows.push(tool(
            &format!("roster-{}", seat.seat),
            &seat.seat,
            &format!("{label} · {} · {}", word(seat.activity), seat.route),
            "",
            state,
        ));
    }
    let open = view.decisions.len();
    if open > 0 {
        rows.push(note(
            "decisions-open",
            &match open {
                1 => "1 decision waits at the podium. `/decide` opens it.".to_owned(),
                n => format!("{n} decisions wait at the podium. `/decide` opens them."),
            },
        ));
    }
    let help = if intents::allows(rights, Right::Operate) {
        intents::HELP.to_owned()
    } else {
        format!(
            "{} This view observes: sending needs a host connection with the `operate` \
             right.",
            intents::HELP
        )
    };
    rows.push(note("console-help", &help));
    rows
}

/// The state a log line shows in: failed for a failure, running for the
/// last line of a seat that is `working`, else done.
fn line_state(activity: Activity, working: bool, last: bool) -> ToolState {
    if matches!(activity, Activity::Failed | Activity::Blocked) {
        ToolState::Failed
    } else if working && last {
        ToolState::Running
    } else {
        ToolState::Done
    }
}

/// The agent card: the seat's state and task, the decisions it owns (its
/// own question or approval) or filed (its goal's plan decision as lead),
/// and its recent log.
fn seat_rows(kind: &PanelKind, view: &View) -> Vec<Node<()>> {
    let Some(seat) = seat(kind, view) else {
        return vec![note("no-seat", "No seat sits at this desk.")];
    };
    let mut rows = Vec::new();
    let role = match seat.role {
        coder_access::studio::Role::Lead => "lead",
        coder_access::studio::Role::Worker => "worker",
    };
    let task = seat
        .task
        .as_deref()
        .and_then(|id| view.tasks.iter().find(|t| t.task == id));
    let state = if seat.paused && seat.task.is_none() {
        "Stopped".to_owned()
    } else {
        indicator(seat.activity)
            .label()
            .unwrap_or("Idle")
            .to_owned()
    };
    let mut body = format!(
        "**{}** · {role} · {}\n\n{state} · {}{} · {} spent",
        seat.seat,
        seat.route,
        word(seat.activity),
        if seat.paused { " · paused" } else { "" },
        seat.spend.label()
    );
    match task {
        Some(task) => body.push_str(&format!(
            "\n\nTask: {} · {} · {} spent",
            task.title,
            status_word(task.status),
            task.spend.label()
        )),
        None => body.push_str("\n\nNo task."),
    }
    rows.push(message("seat", MessageRole::Assistant, &body));
    for open in intents::decisions(view)
        .into_iter()
        .filter(|open| open.seat.as_deref() == Some(seat.seat.as_str()))
    {
        let whose = if open.task.is_some() {
            "Waiting on you"
        } else {
            "Filed for its goal"
        };
        rows.push(note(
            &format!("owned-{}", open.decision),
            &format!(
                "**{whose}** · {}\n\n{}",
                heading(open),
                coder_access::studio::first_line(&open.text, 240)
            ),
        ));
    }
    let lines = view
        .logs
        .iter()
        .find(|log| log.seat == seat.seat)
        .map_or(&[][..], |log| log.lines.as_slice());
    let working = indicator(seat.activity) == attention::Indicator::Working;
    for (i, line) in lines.iter().enumerate() {
        rows.push(tool(
            &format!("log-{i}"),
            word(line.activity),
            &line.text,
            "",
            line_state(line.activity, working, i + 1 == lines.len()),
        ));
    }
    if lines.is_empty() {
        rows.push(note("no-log", "Nothing in this seat's log yet."));
    }
    // Each message to the seat with its delivery: an accepted steer is
    // not a consumed one (NIP-SESS).
    for sent in view.messages.iter().filter(|sent| sent.seat == seat.seat) {
        rows.push(message(
            &format!("message-{}", sent.message),
            MessageRole::User,
            &format!(
                "**{}**: {}\n\n*{}*",
                sent.sender(),
                sent.text,
                sent.delivery()
            ),
        ));
    }
    rows
}

/// A task's details: its place, seat, goal, dependencies, the decision it
/// waits on, and its newest log line.
fn task_rows(kind: &PanelKind, view: &View) -> Vec<Node<()>> {
    let Some(task) = task(kind, view) else {
        return vec![note("no-task", "This task is no longer on the board.")];
    };
    let mut rows = Vec::new();
    let place = if task.entry == "lead" {
        "the lead's plan".to_owned()
    } else {
        format!("step {} of its goal", task.position)
    };
    let mut body = format!(
        "**{}** · {}\n\n{place} · seat {} · `{}`",
        task.title,
        status_word(task.status),
        task.seat,
        intents::task_name(view, task)
    );
    if let Some(goal) = view.goals.iter().find(|goal| goal.goal == task.goal) {
        body.push_str(&format!("\n\nGoal: {} · on {}", goal.text, goal.workspace));
    }
    rows.push(message("task", MessageRole::Assistant, &body));
    if !task.depends_on.is_empty() {
        let mut after = "**Depends on**".to_owned();
        for entry in &task.depends_on {
            let found = view
                .tasks
                .iter()
                .find(|other| other.goal == task.goal && other.entry == *entry);
            match found {
                Some(other) => after.push_str(&format!(
                    "\n- {entry}: {} · {}",
                    other.title,
                    status_word(other.status)
                )),
                None => after.push_str(&format!("\n- {entry}: not on the board")),
            }
        }
        rows.push(note("depends", &after));
    }
    if let Some(open) = view
        .decisions
        .iter()
        .find(|open| open.task.as_deref() == Some(task.task.as_str()))
    {
        rows.push(note("asks", &format!("{}\n\n{}", heading(open), open.text)));
    }
    let line = view
        .logs
        .iter()
        .find(|log| log.task.as_deref() == Some(task.task.as_str()))
        .and_then(|log| log.lines.last());
    match line {
        Some(line) => rows.push(tool(
            "live",
            word(line.activity),
            &line.text,
            "",
            line_state(line.activity, task.status == TaskStatus::Running, true),
        )),
        None => rows.push(note("no-live", "No log line from this task yet.")),
    }
    rows
}

/// The word a memory entry's kind is shown with.
fn memory_word(kind: MemoryKind) -> &'static str {
    match kind {
        MemoryKind::Plan => "Plan",
        MemoryKind::Decision => "Decision",
        MemoryKind::Convention => "Convention",
        MemoryKind::Note => "Note",
    }
}

/// `plan`'s text with each listed plan entry followed by its task's
/// status, so the pinned plan reads as a progress report.
fn annotated(plan: &Memory, view: &View) -> String {
    plan.text
        .lines()
        .map(|line| {
            let entry = line
                .strip_prefix("- ")
                .and_then(|rest| rest.split_once(':'))
                .map(|(entry, _)| entry.trim());
            let found = entry.and_then(|entry| {
                view.tasks.iter().find(|task| {
                    task.entry == entry && plan.goal.as_deref().is_none_or(|g| task.goal == g)
                })
            });
            match found {
                Some(task) => format!("{line} · {}", status_word(task.status)),
                None => line.to_owned(),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Shared memory as the library shows it: the pinned plan first, then the
/// other entries, newest first.
fn library_rows(view: &View) -> Vec<Node<()>> {
    let mut rows = Vec::new();
    let goal = |entry: &Memory| {
        entry
            .goal
            .as_deref()
            .map_or(String::new(), |goal| format!(" · goal {goal}"))
    };
    if let Some(plan) = view.plan() {
        rows.push(message(
            "plan",
            MessageRole::Assistant,
            &format!(
                "**Plan** · pinned · from {}{}\n\n{}",
                plan.author,
                goal(plan),
                annotated(plan, view)
            ),
        ));
    }
    for entry in view.memory.iter().rev().filter(|entry| !entry.pinned) {
        rows.push(message(
            &format!("memory-{}", entry.entry),
            MessageRole::User,
            &format!(
                "**{}** · from {}{}\n\n{}",
                memory_word(entry.kind),
                entry.author,
                goal(entry),
                entry.text
            ),
        ));
    }
    if rows.is_empty() {
        rows.push(note(
            "no-memory",
            "No shared memory yet. The lead's plan is pinned here once a goal is planned.",
        ));
    }
    rows
}

/// The question flow a decision is answered with: an approval's options,
/// with its named step when the host named one, or the question's pages.
/// A goal's plan decision is answered with a plan as free text.
#[must_use]
pub fn flow(open: &Decision) -> decision::Flow {
    match (open.kind, &open.approval) {
        (DecisionKind::Approval, Some(step)) => {
            decision::Flow::approval_step(&open.text, prompt(step))
        }
        (DecisionKind::Approval, None) => decision::Flow::approval(&open.text),
        _ => decision::Flow::question(&open.text),
    }
}

/// The decision panel's prompt for the step an approval names.
#[must_use]
pub fn prompt(step: &Approval) -> decision::Prompt {
    decision::Prompt {
        tool: step.tool.clone(),
        command: step.command.clone(),
        cwd: step.cwd.clone(),
        reason: step.reason.clone(),
        risk: match step.risk {
            Risk::Low => decision::Risk::Low,
            Risk::Medium => decision::Risk::Medium,
            Risk::High => decision::Risk::High,
        },
        always: step.always.clone(),
    }
}

fn heading(open: &Decision) -> String {
    let who = open.seat.as_deref().unwrap_or("The studio");
    match open.kind {
        DecisionKind::Approval => format!("**{who}** asks to go ahead"),
        DecisionKind::Question => format!("**{who}** asks"),
        DecisionKind::InvalidPlan => {
            "**The plan failed validation**: answer with a corrected plan".to_owned()
        }
        DecisionKind::NoPlan => {
            "**The lead finished without a plan**: answer with a plan".to_owned()
        }
        DecisionKind::LeadFailed => {
            "**The lead failed**: answer with a plan, or retry the lead".to_owned()
        }
        DecisionKind::DependencyFailed => {
            "**A dependency did not finish**: retry or cancel the task".to_owned()
        }
    }
}

/// A decision's text at the podium: its heading, each page's prompt and
/// options, and an approval's named step.
fn decision_body(open: &Decision) -> String {
    let flow = flow(open);
    let pages = flow.pages();
    let mut body = heading(open);
    for (n, page) in pages.iter().enumerate() {
        if pages.len() > 1 {
            body.push_str(&format!("\n\n*{} of {}*", n + 1, pages.len()));
        }
        if !page.prompt.is_empty() {
            body.push_str("\n\n");
            body.push_str(&page.prompt);
        }
        if let Some(step) = flow.prompt() {
            body.push_str("\n\n");
            body.push_str(&step.markdown());
        }
        for (k, option) in page.options.iter().enumerate() {
            body.push_str(&format!("\n{}. {option}", k + 1));
        }
    }
    body
}

fn decision_rows(view: &View, answering: Option<&str>) -> Vec<Node<()>> {
    let mut rows = Vec::new();
    let open = intents::decisions(view);
    let answering = answering.or_else(|| open.first().map(|d| d.decision.as_str()));
    for (i, open) in open.into_iter().enumerate() {
        let mut body = decision_body(open);
        if answering == Some(open.decision.as_str()) {
            // The marker follows the heading, the body's first line.
            let end = body.find('\n').unwrap_or(body.len());
            body.insert_str(end, " · *answering now*");
        }
        rows.push(message(
            &format!("decision-{i}-{}", open.decision),
            MessageRole::Assistant,
            &body,
        ));
    }
    let merges = reviewable(view).len();
    if merges > 0 {
        rows.push(note(
            "merges",
            &match merges {
                1 => "1 change waits at the merge station.".to_owned(),
                n => format!("{n} changes wait at the merge station."),
            },
        ));
    }
    if view.decisions.is_empty() {
        rows.push(note("no-decisions", "No decision waits on you."));
    }
    rows
}

fn review_rows(view: &View, review: Option<&TaskReview>, rights: &[Right]) -> Vec<Node<()>> {
    let Some(review) = review else {
        return vec![note(
            "no-review",
            "No review is open. A done task's review loads here when its seat finishes.",
        )];
    };
    let task = view.tasks.iter().find(|t| t.task == review.task);
    let title = task.map_or(review.task.as_str(), |t| t.title.as_str());
    let mut body = format!(
        "**What changed** · {title}\n\n{} {} · +{} −{}",
        review.files_total,
        if review.files_total == 1 {
            "file"
        } else {
            "files"
        },
        review.added,
        review.removed
    );
    for file in &review.files {
        body.push_str(&format!(
            "\n- `{}` +{} −{}",
            file.path,
            file.added.unwrap_or(0),
            file.removed.unwrap_or(0)
        ));
    }
    let mut rows = vec![message("review", MessageRole::Assistant, &body)];
    // The worker's summary: the newest line its log holds for the task.
    let summary = view
        .logs
        .iter()
        .find(|log| log.task.as_deref() == Some(review.task.as_str()))
        .and_then(|log| log.lines.last().map(|line| (log.seat.as_str(), line)));
    if let Some((seat, line)) = summary {
        rows.push(note(
            "review-summary",
            &format!("**{seat}'s summary** · {}", line.text),
        ));
    }
    // The reviewers' notes: decisions shared memory holds on the task.
    let notes: Vec<&Memory> = view
        .memory
        .iter()
        .filter(|entry| entry.kind == MemoryKind::Decision && entry.text.contains(&review.task))
        .collect();
    if !notes.is_empty() {
        let mut text = "**Reviewers' notes**".to_owned();
        for entry in notes {
            text.push_str(&format!("\n- {} · {}", entry.author, entry.text));
        }
        rows.push(note("review-notes", &text));
    }
    let help = if intents::allows(rights, Right::Review) {
        "Pick a line in **What changed** and type to comment on it; with no line picked, \
         what you type is your note. **Request changes** sends the note and the comments. \
         In **What changed**, `n` and `p` move between files, `]` and `[` between hunks, \
         `c` copies the file's path, and Enter starts a note."
    } else {
        "**Merge**, **Request changes**, and **Reject** need a host connection with the \
         `review` right. In **What changed**, `n` and `p` move between files, `]` and `[` \
         between hunks, and `c` copies the file's path."
    };
    rows.push(note("review-help", help));
    rows
}

/// The rows a panel of `kind` shows for `view`, and for the review panel,
/// `review`, to a view that holds no rights.
#[must_use]
pub fn rows(kind: &PanelKind, view: Option<&View>, review: Option<&TaskReview>) -> Vec<Node<()>> {
    rows_for(kind, view, review, &[])
}

/// The rows a panel of `kind` shows for `view` and `review` to a view
/// whose source holds `rights`.
#[must_use]
pub fn rows_for(
    kind: &PanelKind,
    view: Option<&View>,
    review: Option<&TaskReview>,
    rights: &[Right],
) -> Vec<Node<()>> {
    let Some(view) = view else {
        return vec![note(
            "not-loaded",
            "The studio has not loaded. It loads while you are in Everglade with a studio \
             source.",
        )];
    };
    match kind {
        PanelKind::Console => console(view, rights),
        PanelKind::Seat(_) | PanelKind::Desk(_) => seat_rows(kind, view),
        PanelKind::Decisions => decision_rows(view, None),
        PanelKind::Review => review_rows(view, review, rights),
        PanelKind::Task(_) => task_rows(kind, view),
        PanelKind::Library => library_rows(view),
    }
}

/// The word a refusal code is spelled with on the wire, such as `stale`.
#[must_use]
pub fn code_word(code: Code) -> String {
    serde_json::to_value(code)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| format!("{code:?}"))
}

/// What the host's answer to `answer` says, for the status row: what it
/// did, or its refusal code and message.
#[must_use]
pub fn status_text(answer: &Answer) -> String {
    let operation = answer.operation;
    match &answer.result {
        Ok(Outcome::Dispatched { receipt }) => {
            format!("**Sent** · `{operation}` · {}", receipt.reference)
        }
        Ok(Outcome::Merged { merged }) => match (&merged.verdict, &merged.publication) {
            (_, Some(publication)) => format!("**Merge** · {}", publication.note),
            (Verdict::RequestChanges, None) => {
                "**Changes requested** · the seat takes them as its next turn".to_owned()
            }
            (_, None) => "**Rejected** · the task's worktree stays until it is archived".to_owned(),
        },
        Ok(Outcome::Workspaces { workspaces }) if workspaces.is_empty() => {
            "**Repositories** · the host admits none".to_owned()
        }
        Ok(Outcome::Workspaces { workspaces }) => format!(
            "**Repositories** · {} · `/repo LABEL` picks one",
            workspaces.join(", ")
        ),
        Ok(_) => format!("**Done** · `{operation}`"),
        Err(error) => {
            let code = code_word(error.code);
            let mut text = format!("**Refused** · `{operation}` · `{code}`: {}", error.message);
            if error.code == Code::Stale && operation == "studio.merge.decide" {
                text.push_str(
                    " The change moved after you read it, so nothing landed; the review \
                     reloads. Read it again before you decide.",
                );
            }
            text
        }
    }
}

/// What a studio panel's control or key asks the app to do.
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    /// Send this intent to the host ([`crate::zones::everglade::studio::Studio::send`]),
    /// then tell the controller what became of it ([`Controller::sent`]).
    Send(Action),
    /// Show this panel instead.
    Open(PanelKind),
    /// Show the podium at this decision.
    Answer(String),
    /// Put this text on the clipboard.
    Copy(String),
    /// Turn the studio's sounds on or off.
    Sound(bool),
}

fn review_identity(review: Option<&TaskReview>) -> Option<(String, String)> {
    review.map(|review| (review.task.clone(), review.head.clone()))
}

/// What one of a studio panel's action buttons does.
#[derive(Clone, Debug, PartialEq)]
enum Control {
    Act(Action),
    /// Resume a stopped seat.
    Spawn(String),
    /// Pick an option on the decision's current page.
    Pick {
        decision: String,
        option: usize,
    },
    /// Go back a page in the decision's flow.
    Back(String),
    /// Move to the next (1) or previous (-1) open decision.
    Cycle(isize),
    /// Decide the open review.
    Decide(Verdict),
    /// Give the task to the seat the composer names.
    Reassign(String),
    /// Start the waiting goal on this repository.
    Repository(String),
    /// Show another panel.
    Open(PanelKind),
    /// Show the podium at this decision.
    AnswerAt(String),
}

impl Control {
    /// Whether the control asks for a second press: **Reject** and
    /// **Cancel** cannot be undone.
    fn confirms(&self) -> bool {
        matches!(
            self,
            Self::Decide(Verdict::Reject) | Self::Act(Action::Cancel(_))
        )
    }
}

/// What the console keeps while its panel is closed: the history, the
/// unsent draft, and the picked repository.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Recall {
    history: Vec<String>,
    draft: String,
    workspace: Option<String>,
}

/// Something sent from a panel: the text to put back if it is refused,
/// what it does in a few words, and the host's ticket once it left.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Sending {
    text: String,
    describe: String,
    ticket: Option<u64>,
}

/// The state behind one open studio panel: which panel it is, the studio
/// revision it shows, its controls, the decisions' question flows, the
/// review's line comments and note, and the console's history.
pub struct Controller {
    kind: PanelKind,
    shown: Option<u64>,
    /// The task and head of the review the panel shows.
    reviewed: Option<(String, String)>,
    controls: Vec<Control>,
    flows: BTreeMap<String, decision::Flow>,
    comments: review_comments::Comments,
    /// The reviewer's note for **Request changes** or **Reject**.
    note: String,
    /// The repository later goals start on, once the person picks one.
    workspace: Option<String>,
    /// What the panel said back to the last thing typed or pressed.
    said: Option<String>,
    /// Whether the source holds the `operate` right, as of the last fill.
    operate: bool,
    /// The console's sent lines, oldest first, where Up walked to, and
    /// the draft Up walked away from.
    history: Vec<String>,
    recall: Option<usize>,
    stash: String,
    /// The lines Tab offered and the one the draft shows.
    completion: Option<(Vec<String>, usize)>,
    /// A goal waiting for the person to pick its repository.
    picking: Option<String>,
    /// What the composer shows in place of the last thing sent.
    ack: Option<String>,
    sending: Option<Sending>,
    /// The decision the person moved to at the podium.
    selected: Option<String>,
    /// The decision the podium shows and when it came up.
    front: Option<(String, Instant)>,
    /// A **Reject** or **Cancel** waiting for its second press, and until
    /// when.
    confirm: Option<(Control, Instant)>,
    /// The review: whether keys type the note rather than move through the
    /// diff, and the diff line the last move went to.
    typing: bool,
    cursor: Option<usize>,
}

impl Controller {
    #[must_use]
    pub fn new(kind: PanelKind) -> Self {
        Self {
            kind,
            shown: None,
            reviewed: None,
            controls: Vec::new(),
            flows: BTreeMap::new(),
            comments: review_comments::Comments::new(),
            note: String::new(),
            workspace: None,
            said: None,
            operate: false,
            history: Vec::new(),
            recall: None,
            stash: String::new(),
            completion: None,
            picking: None,
            ack: None,
            sending: None,
            selected: None,
            front: None,
            confirm: None,
            typing: false,
            cursor: None,
        }
    }

    /// The panel this controls.
    #[must_use]
    pub fn kind(&self) -> &PanelKind {
        &self.kind
    }

    /// Whether the panel shows an older studio than `revision`, or another
    /// review than `review`, which a host source reads after the panel
    /// opens.
    #[must_use]
    pub fn stale(&self, revision: u64, review: Option<&TaskReview>) -> bool {
        self.shown != Some(revision) || self.reviewed != review_identity(review)
    }

    /// What the console keeps while closed, with `panel`'s unsent draft.
    #[must_use]
    pub fn recall(&self, panel: &Panel) -> Recall {
        Recall {
            history: self.history.clone(),
            draft: panel.draft().to_owned(),
            workspace: self.workspace.clone(),
        }
    }

    /// Takes back what the console kept while closed. Call after
    /// [`Controller::fill`], so the composer holds the draft.
    pub fn restore(&mut self, recall: Recall, panel: &mut Panel) {
        self.history = recall.history;
        if recall.workspace.is_some() {
            self.workspace = recall.workspace;
        }
        if panel.draft().is_empty() {
            panel.set_draft(&recall.draft);
        }
    }

    /// Moves the podium to the open decision `decision`.
    pub fn select(&mut self, decision: &str) {
        self.selected = Some(decision.to_owned());
    }

    /// The open decision the podium answers now: the one the person moved
    /// to, else the first in the podium's order.
    fn answering<'a>(&self, view: Option<&'a View>) -> Option<&'a Decision> {
        let open = intents::decisions(view?);
        self.selected
            .as_deref()
            .and_then(|id| open.iter().copied().find(|d| d.decision == id))
            .or_else(|| open.first().copied())
    }

    /// The review the merge station may decide: loaded, and of a task the
    /// studio still holds. The host decides whether the decision lands.
    fn decidable<'a>(
        view: Option<&View>,
        review: Option<&'a TaskReview>,
    ) -> Option<&'a TaskReview> {
        let view = view?;
        review.filter(|review| view.tasks.iter().any(|task| task.task == review.task))
    }

    /// Whether a key at `now` comes too soon after the podium's decision
    /// came up. A decision that came up since the last fill starts the
    /// lock now.
    fn locked(&mut self, open: &Decision, now: Instant) -> bool {
        match &self.front {
            Some((id, since)) if *id == open.decision => now < *since + INPUT_LOCK,
            _ => {
                self.front = Some((open.decision.clone(), now));
                true
            }
        }
    }

    /// Whether a press of `control` at `now` is its confirming press; the
    /// first press waits [`CONFIRM`] for it.
    fn confirmed(&mut self, control: &Control, now: Instant) -> bool {
        match &self.confirm {
            Some((waiting, until)) if waiting == control && now <= *until => {
                self.confirm = None;
                true
            }
            _ => {
                self.confirm = Some((control.clone(), now + CONFIRM));
                false
            }
        }
    }

    /// Whether `control` waits for its confirming press.
    fn arming(&self, control: &Control) -> bool {
        self.confirm
            .as_ref()
            .is_some_and(|(waiting, until)| waiting == control && Instant::now() <= *until)
    }

    /// Shows the studio at `revision` in `panel`: its title, rows, diff,
    /// controls, and composer, offering only what `rights` allow, and the
    /// host's newest answer, `status`.
    pub fn fill(
        &mut self,
        panel: &mut Panel,
        revision: u64,
        view: Option<&View>,
        review: Option<&TaskReview>,
        rights: &[Right],
        status: Option<&Answer>,
    ) {
        self.shown = Some(revision);
        let identity = review_identity(review);
        if self.reviewed != identity {
            self.typing = false;
            self.cursor = None;
        }
        self.reviewed = identity;
        if let Some(view) = view {
            self.flows
                .retain(|id, _| view.decisions.iter().any(|open| open.decision == *id));
        }
        // The host's answer to what this panel sent: acknowledged in place
        // of the draft, and a refused draft comes back.
        let mut restore = None;
        if let Some(status) = status
            && let Some(sending) = &self.sending
            && sending.ticket == Some(status.ticket)
        {
            let sending = self.sending.take().expect("matched above");
            match &status.result {
                Ok(_) => self.ack = Some(format!("Sent: {}", sending.describe)),
                Err(error) => {
                    self.ack = Some(format!(
                        "Refused: {} (`{}`)",
                        sending.describe,
                        code_word(error.code)
                    ));
                    if !sending.text.is_empty() {
                        self.said =
                            Some("The host refused it; your text is back in the composer.".into());
                        restore = Some(sending.text);
                    }
                }
            }
        }
        panel.set_title(&title(&self.kind, view));
        if self.kind == PanelKind::Review {
            let diff = review.map_or("", |r| r.diff.as_str());
            if panel.diff_source() != Some(diff) {
                panel.set_diff(diff);
            }
        }
        let mut rows = rows_for(&self.kind, view, review, rights);
        let operate = intents::allows(rights, Right::Operate);
        self.operate = operate;
        let mut controls = Vec::new();
        let mut composer = None;
        match &self.kind {
            PanelKind::Console => {
                if operate && view.is_some() {
                    composer = Some("Type a goal, @seat and a message, or a /command".to_owned());
                }
                if let (Some(text), Some(view)) = (&self.picking, view) {
                    rows.push(note(
                        "console-pick",
                        &format!(
                            "Pick the repository for **{text}**: press its number or its \
                             button. Esc keeps the goal as a draft."
                        ),
                    ));
                    for repository in view.repositories.iter().take(9) {
                        controls.push(Control::Repository(repository.workspace.clone()));
                    }
                } else if let Some(workspace) = &self.workspace {
                    rows.push(note(
                        "console-repository",
                        &format!("Goals start on **{workspace}**."),
                    ));
                }
                if let Some((lines, shown)) = &self.completion
                    && lines.len() > 1
                {
                    let listed: Vec<String> = lines
                        .iter()
                        .enumerate()
                        .map(|(i, line)| {
                            if i == *shown {
                                format!("**{line}**")
                            } else {
                                line.clone()
                            }
                        })
                        .collect();
                    rows.push(note(
                        "console-completion",
                        &format!("Tab: {}", listed.join(" · ")),
                    ));
                }
            }
            PanelKind::Seat(_) | PanelKind::Desk(_) => {
                if let Some(view) = view
                    && let Some(seat) = seat(&self.kind, view)
                    && operate
                {
                    let name = seat.seat.clone();
                    if seat.paused && seat.task.is_none() {
                        controls.push(Control::Spawn(name.clone()));
                    } else {
                        controls.push(if seat.paused {
                            Control::Act(Action::Resume(name.clone()))
                        } else {
                            Control::Act(Action::Pause(name.clone()))
                        });
                        controls.push(Control::Act(Action::Stop(name.clone())));
                    }
                    for open in intents::decisions(view)
                        .into_iter()
                        .filter(|open| open.seat.as_deref() == Some(name.as_str()))
                        .take(CARD_DECISIONS)
                    {
                        controls.push(Control::AnswerAt(open.decision.clone()));
                    }
                    composer = Some(format!("Message {name}"));
                }
            }
            PanelKind::Decisions => {
                if let Some(open) = self.answering(view) {
                    let id = open.decision.clone();
                    if self.front.as_ref().is_none_or(|(front, _)| *front != id) {
                        self.front = Some((id.clone(), Instant::now()));
                    }
                    if let Some(view) = view {
                        rows = decision_rows(view, Some(&id));
                    }
                    let count = view.map_or(0, |view| view.decisions.len());
                    if operate {
                        let flow = self.flows.entry(id.clone()).or_insert_with(|| flow(open));
                        let page = flow.current();
                        let place = intents::decisions(view.expect("a decision is open"))
                            .iter()
                            .position(|other| other.decision == id)
                            .map_or(1, |i| i + 1);
                        rows.push(message(
                            "answering",
                            MessageRole::System,
                            &format!(
                                "Answering {place} of {count}: {} · {}{}",
                                heading(open),
                                flow.counter(),
                                if page.options.is_empty() {
                                    " · type the answer".to_owned()
                                } else {
                                    " · press an option's number, or type the answer".to_owned()
                                }
                            ),
                        ));
                        for option in 0..page.options.len() {
                            controls.push(Control::Pick {
                                decision: id.clone(),
                                option,
                            });
                        }
                        if flow.page() > 0 {
                            controls.push(Control::Back(id.clone()));
                        }
                        composer = Some("Type the answer".to_owned());
                    }
                    if count > 1 {
                        controls.push(Control::Cycle(-1));
                        controls.push(Control::Cycle(1));
                    }
                }
                if view.is_some_and(|view| !reviewable(view).is_empty()) {
                    controls.push(Control::Open(PanelKind::Review));
                }
            }
            PanelKind::Review => {
                if let Some(review) = Self::decidable(view, review)
                    && intents::allows(rights, Right::Review)
                {
                    for verdict in [Verdict::Merge, Verdict::RequestChanges, Verdict::Reject] {
                        controls.push(Control::Decide(verdict));
                    }
                    composer = Some(if panel.tab() == Tab::Changes && !self.typing {
                        "Enter writes a note · n p file · ] [ hunk · c copies the path".to_owned()
                    } else {
                        "Type a note, or a comment on the picked line".to_owned()
                    });
                    if let Some(target) = panel
                        .selected_line()
                        .zip(panel.diff_document())
                        .and_then(|(line, doc)| review_comments::target(doc, line))
                    {
                        rows.push(note(
                            "review-line",
                            &format!("Picked line: `{}`", target.location()),
                        ));
                    }
                    for comment in self.comments.all() {
                        let outdated = if comment.is_current(review) {
                            ""
                        } else {
                            " · outdated, not sent"
                        };
                        rows.push(note(
                            &format!("comment-{}", comment.id),
                            &format!(
                                "**{}** ({}){outdated}: {}",
                                comment.target.location(),
                                comment.target.side.tag(),
                                comment.body
                            ),
                        ));
                    }
                    if !self.note.is_empty() {
                        rows.push(note("review-note", &format!("Your note: {}", self.note)));
                    }
                }
            }
            PanelKind::Task(id) => {
                if let Some(task) = view.and_then(|view| task(&self.kind, view))
                    && operate
                {
                    match task.status {
                        TaskStatus::Failed | TaskStatus::Cancelled | TaskStatus::Blocked => {
                            controls.push(Control::Act(Action::Retry(id.clone())));
                        }
                        TaskStatus::Held | TaskStatus::Queued => {
                            controls.push(Control::Act(Action::Prioritize(id.clone())));
                            controls.push(Control::Reassign(id.clone()));
                            composer = Some("Type @seat, then Reassign or Enter".to_owned());
                        }
                        _ => {}
                    }
                    if !task.status.is_final() {
                        controls.push(Control::Act(Action::Cancel(id.clone())));
                    }
                }
            }
            PanelKind::Library => {}
        }
        if let Some((waiting, _)) = &self.confirm
            && self.arming(waiting)
        {
            rows.push(note(
                "confirm",
                match waiting {
                    Control::Decide(_) => {
                        "Press **Confirm reject** to reject: the task closes, and its worktree \
                         stays for inspection."
                    }
                    _ => "Press **Confirm cancel** to cancel the task: it stops the work on it.",
                },
            ));
        }
        if let Some(said) = &self.said {
            rows.push(note("said", said));
        }
        if let Some(status) = status {
            rows.push(note("status", &status_text(status)));
        }
        let labels: Vec<(String, bool)> = controls
            .iter()
            .enumerate()
            .map(|(index, control)| (self.label(control, index), true))
            .collect();
        self.controls = controls;
        panel.set_rows(rows);
        panel.set_actions(labels);
        // What was sent shows in place of the draft until the next one.
        let composer = composer.map(|placeholder| self.ack.clone().unwrap_or(placeholder));
        panel.set_composer(composer.as_deref());
        if let Some(text) = restore
            && panel.draft().is_empty()
        {
            panel.set_draft(&text);
        }
    }

    /// The label of `control`, the `index`th button. A repository's
    /// button carries the number key that picks it.
    fn label(&self, control: &Control, index: usize) -> String {
        match control {
            Control::Act(Action::Pause(_)) => "Pause".to_owned(),
            Control::Act(Action::Resume(_)) => "Resume".to_owned(),
            Control::Act(Action::Stop(_)) => "Stop".to_owned(),
            Control::Act(Action::Retry(_)) => "Retry".to_owned(),
            Control::Act(Action::Prioritize(_)) => "Prioritize".to_owned(),
            Control::Act(Action::Cancel(_)) if self.arming(control) => "Confirm cancel".to_owned(),
            Control::Act(Action::Cancel(_)) => "Cancel".to_owned(),
            Control::Act(_) => "Send".to_owned(),
            Control::Spawn(_) => "Spawn".to_owned(),
            Control::Pick { decision, option } => self
                .flows
                .get(decision)
                .and_then(|flow| flow.current().options.get(*option))
                .map_or_else(
                    || format!("Option {}", option + 1),
                    |label| format!("{}. {label}", option + 1),
                ),
            Control::Back(_) => "Back".to_owned(),
            Control::Cycle(step) if *step < 0 => "Previous decision".to_owned(),
            Control::Cycle(_) => "Next decision".to_owned(),
            Control::Decide(Verdict::Merge) => "Merge".to_owned(),
            Control::Decide(Verdict::RequestChanges) => "Request changes".to_owned(),
            Control::Decide(Verdict::Reject) if self.arming(control) => "Confirm reject".to_owned(),
            Control::Decide(Verdict::Reject) => "Reject".to_owned(),
            Control::Reassign(_) => "Reassign".to_owned(),
            Control::Repository(label) => format!("{}. {label}", index + 1),
            Control::Open(PanelKind::Review) => "Open the review".to_owned(),
            Control::Open(_) => "Open".to_owned(),
            Control::AnswerAt(id) => format!("Answer {}", short(id)),
        }
    }

    /// Carries out `intent`, which `panel` resolved, against the studio
    /// `view` and the loaded `review`. Returns what the app does next; the
    /// app fills the panel again after.
    pub fn intent(
        &mut self,
        intent: Intent,
        panel: &mut Panel,
        view: Option<&View>,
        review: Option<&TaskReview>,
    ) -> Vec<Effect> {
        self.said = None;
        match intent {
            Intent::Action(index) => match self.controls.get(index).cloned() {
                Some(control) => self.press(control, panel, view, review, Instant::now()),
                None => Vec::new(),
            },
            Intent::Submit => {
                let draft = panel.take_draft();
                self.submit(&draft, panel, view, review)
            }
            Intent::Show(_) | Intent::Close => Vec::new(),
        }
    }

    /// Carries out a press of `control` at `now`.
    fn press(
        &mut self,
        control: Control,
        panel: &mut Panel,
        view: Option<&View>,
        review: Option<&TaskReview>,
        now: Instant,
    ) -> Vec<Effect> {
        if control.confirms() && !self.confirmed(&control, now) {
            return Vec::new();
        }
        if !control.confirms() {
            self.confirm = None;
        }
        match control {
            Control::Act(action) => self.send(action, ""),
            Control::Spawn(seat) => self.send(Action::Resume(seat), ""),
            Control::Pick { decision, option } => {
                let step = self
                    .flows
                    .get_mut(&decision)
                    .map_or(decision::Step::Stay, |flow| flow.select(option));
                self.step(&decision, step, view, "")
            }
            Control::Back(decision) => {
                if let Some(flow) = self.flows.get_mut(&decision) {
                    flow.back();
                }
                Vec::new()
            }
            Control::Cycle(step) => {
                self.cycle(step, view);
                Vec::new()
            }
            Control::Decide(verdict) => self.decide(verdict, view, review),
            Control::Reassign(task) => {
                let draft = panel.take_draft();
                self.reassign(&task, &draft, panel, view)
            }
            Control::Repository(label) => self.pick_repository(&label),
            Control::Open(kind) => vec![Effect::Open(kind)],
            Control::AnswerAt(id) => vec![Effect::Answer(id)],
        }
    }

    /// Sends `action`; `text` is what the person typed for it, which comes
    /// back to the composer if the host refuses it.
    fn send(&mut self, action: Action, text: &str) -> Vec<Effect> {
        let describe = action.describe();
        self.ack = Some(format!("Sending: {describe}…"));
        self.sending = Some(Sending {
            text: text.to_owned(),
            describe,
            ticket: None,
        });
        vec![Effect::Send(action)]
    }

    /// What became of the last [`Effect::Send`]: the host's ticket, whose
    /// answer [`Controller::fill`] acknowledges, or why it never left, in
    /// which case its text comes back to `panel`'s composer.
    pub fn sent(&mut self, result: Result<u64, String>, panel: &mut Panel) {
        let Some(sending) = &mut self.sending else {
            return;
        };
        if sending.ticket.is_some() {
            return;
        }
        match result {
            Ok(ticket) => sending.ticket = Some(ticket),
            Err(why) => {
                let text = std::mem::take(&mut sending.text);
                self.sending = None;
                self.ack = None;
                self.said = Some(why);
                if !text.is_empty() && panel.draft().is_empty() {
                    panel.set_draft(&text);
                }
            }
        }
    }

    /// Moves the podium `step` decisions along, wrapping.
    fn cycle(&mut self, step: isize, view: Option<&View>) {
        let Some(view) = view else { return };
        let open = intents::decisions(view);
        if open.is_empty() {
            return;
        }
        let at = self
            .answering(Some(view))
            .and_then(|current| open.iter().position(|d| d.decision == current.decision))
            .unwrap_or(0);
        let len = open.len() as isize;
        let next = (at as isize + step).rem_euclid(len) as usize;
        self.selected = Some(open[next].decision.clone());
    }

    /// Starts the goal waiting on a repository on `label`, which later
    /// goals start on too.
    fn pick_repository(&mut self, label: &str) -> Vec<Effect> {
        let Some(text) = self.picking.take() else {
            return Vec::new();
        };
        self.workspace = Some(label.to_owned());
        self.send(
            Action::SubmitGoal {
                text: text.clone(),
                workspace: label.to_owned(),
            },
            &text,
        )
    }

    /// Gives `task` to the seat `draft` names as `@seat`.
    fn reassign(
        &mut self,
        task: &str,
        draft: &str,
        panel: &mut Panel,
        view: Option<&View>,
    ) -> Vec<Effect> {
        let name = draft.trim().trim_start_matches('@');
        if name.is_empty() {
            self.said = Some("Type @seat in the composer, then press Reassign or Enter.".into());
            return Vec::new();
        }
        if !view.is_some_and(|view| view.seats.iter().any(|seat| seat.seat == name)) {
            self.said = Some(format!("No seat is named {name}."));
            panel.set_draft(draft);
            return Vec::new();
        }
        self.send(
            Action::Reassign {
                task: task.to_owned(),
                seat: name.to_owned(),
            },
            draft,
        )
    }

    /// Keeps `line` in the console's history.
    fn remember(&mut self, line: &str) {
        let line = line.trim();
        if !line.is_empty() && self.history.last().is_none_or(|last| last != line) {
            self.history.push(line.to_owned());
            if self.history.len() > MAX_HISTORY {
                self.history.remove(0);
            }
        }
        self.recall = None;
        self.stash.clear();
    }

    fn submit(
        &mut self,
        draft: &str,
        panel: &mut Panel,
        view: Option<&View>,
        review: Option<&TaskReview>,
    ) -> Vec<Effect> {
        let Some(view) = view else {
            self.said = Some("The studio has not loaded yet.".into());
            panel.set_draft(draft);
            return Vec::new();
        };
        match self.kind.clone() {
            PanelKind::Console => {
                self.remember(draft);
                self.completion = None;
                match intents::console(draft, view, self.workspace.as_deref()) {
                    Console::Act(action) => self.send(action, draft),
                    Console::Spawn { seat, text } => {
                        let mut effects = self.send(Action::Resume(seat.clone()), draft);
                        if let Some(text) = text {
                            effects.push(Effect::Send(Action::Message {
                                seat: Some(seat),
                                text,
                            }));
                        }
                        effects
                    }
                    Console::PickRepository(text) => {
                        self.picking = Some(text);
                        self.said = Some("Which repository? Press its number.".into());
                        Vec::new()
                    }
                    Console::Review => vec![Effect::Open(PanelKind::Review)],
                    Console::Decide => vec![Effect::Open(PanelKind::Decisions)],
                    Console::Status => {
                        self.said = Some(intents::status(view));
                        Vec::new()
                    }
                    Console::Help => {
                        self.said = Some(intents::HELP.into());
                        Vec::new()
                    }
                    Console::Clear => {
                        self.ack = None;
                        self.picking = None;
                        Vec::new()
                    }
                    Console::Sound(on) => {
                        self.said = Some(if on {
                            "Studio sounds are on.".into()
                        } else {
                            "Studio sounds are off for this session.".into()
                        });
                        vec![Effect::Sound(on)]
                    }
                    Console::Repository(label) => {
                        self.said = Some(format!("Goals start on {label} now."));
                        self.workspace = Some(label);
                        Vec::new()
                    }
                    Console::Refused(why) => {
                        self.said = Some(why);
                        // The text comes back so the person can fix it.
                        panel.set_draft(draft);
                        Vec::new()
                    }
                }
            }
            PanelKind::Seat(_) | PanelKind::Desk(_) => match seat(&self.kind, view) {
                Some(seat) => {
                    let action = Action::Message {
                        seat: Some(seat.seat.clone()),
                        text: draft.trim().to_owned(),
                    };
                    self.send(action, draft)
                }
                None => Vec::new(),
            },
            PanelKind::Decisions => {
                let Some(open) = self.answering(Some(view)) else {
                    self.said = Some("No decision waits on you.".into());
                    return Vec::new();
                };
                let id = open.decision.clone();
                let step = self
                    .flows
                    .entry(id.clone())
                    .or_insert_with(|| flow(open))
                    .answer_typed(draft);
                self.step(&id, step, Some(view), draft)
            }
            PanelKind::Review => {
                self.typing = false;
                let Some(review) = Self::decidable(Some(view), review) else {
                    self.said = Some("No review is open to comment on.".into());
                    return Vec::new();
                };
                let target = panel
                    .selected_line()
                    .zip(panel.diff_document())
                    .and_then(|(line, doc)| review_comments::target(doc, line));
                match target {
                    Some(target) => {
                        let location = target.location();
                        match self.comments.add(review, target, draft) {
                            Ok(_) => self.said = Some(format!("Commented on `{location}`.")),
                            Err(refusal) => {
                                self.said = Some(format!("The comment was not kept: {refusal:?}."));
                            }
                        }
                    }
                    None => {
                        draft.trim().clone_into(&mut self.note);
                        self.said =
                            Some("Your note goes with **Request changes** or **Reject**.".into());
                    }
                }
                Vec::new()
            }
            PanelKind::Task(task) => self.reassign(&task, draft, panel, Some(view)),
            PanelKind::Library => Vec::new(),
        }
    }

    /// What a step of decision `id`'s flow does: a finished flow sends its
    /// answer at the decision's own point. `typed` is the draft that
    /// finished it, if any.
    fn step(
        &mut self,
        id: &str,
        step: decision::Step,
        view: Option<&View>,
        typed: &str,
    ) -> Vec<Effect> {
        let decision::Step::Done(text) = step else {
            return Vec::new();
        };
        // **Always allow** sends the rule the host offered;
        // the host records and applies it.
        let always = self
            .flows
            .remove(id)
            .and_then(|flow| flow.always().map(str::to_owned));
        let Some(open) = view.and_then(|view| view.decisions.iter().find(|d| d.decision == id))
        else {
            self.said = Some("That decision was answered already.".into());
            return Vec::new();
        };
        if self.selected.as_deref() == Some(id) {
            self.selected = None;
        }
        let action = match always {
            Some(rule) => Action::AllowAlways {
                decision: open.decision.clone(),
                based_on: open.based_on,
                rule,
            },
            None => Action::Answer {
                decision: open.decision.clone(),
                based_on: open.based_on,
                text,
            },
        };
        self.send(action, typed)
    }

    fn decide(
        &mut self,
        verdict: Verdict,
        view: Option<&View>,
        review: Option<&TaskReview>,
    ) -> Vec<Effect> {
        let Some(review) = Self::decidable(view, review) else {
            self.said = Some("No review is open to decide.".into());
            return Vec::new();
        };
        let text = match verdict {
            Verdict::Merge => String::new(),
            Verdict::Reject => self.note.trim().to_owned(),
            Verdict::RequestChanges => match self.comments.follow_up(review, &self.note) {
                Ok(follow_up) => {
                    self.comments.sent(&follow_up);
                    follow_up.text
                }
                Err(_) => {
                    self.said = Some(
                        "Write a note, or comment on a line, before you request changes.".into(),
                    );
                    return Vec::new();
                }
            },
        };
        if verdict != Verdict::Merge {
            self.note.clear();
        }
        self.send(
            Action::Decide {
                review: Box::new(review.clone()),
                verdict,
                text,
            },
            "",
        )
    }

    /// A key pressed while `panel` has focus, at `now`, before the panel
    /// takes it. Returns what the app does when the controller took the
    /// key, or `None` when the panel takes it as usual.
    pub fn key(
        &mut self,
        key: Key,
        panel: &mut Panel,
        view: Option<&View>,
        review: Option<&TaskReview>,
        now: Instant,
    ) -> Option<Vec<Effect>> {
        if !matches!(key, Key::Tab | Key::BackTab) {
            self.completion = None;
        }
        match self.kind {
            PanelKind::Console => self.console_key(key, panel, view),
            PanelKind::Decisions => self.decision_key(key, panel, view, now),
            PanelKind::Review => self.review_key(key, panel, review),
            _ => None,
        }
    }

    fn console_key(
        &mut self,
        key: Key,
        panel: &mut Panel,
        view: Option<&View>,
    ) -> Option<Vec<Effect>> {
        let view = view?;
        if !panel.has_composer() {
            return None;
        }
        match key {
            Key::Tab | Key::BackTab => {
                let forward = key == Key::Tab;
                let cycling = self.completion.as_ref().is_some_and(|(lines, shown)| {
                    lines.get(*shown).is_some_and(|l| l == panel.draft())
                });
                if cycling && let Some((lines, shown)) = &mut self.completion {
                    let len = lines.len();
                    *shown = if forward {
                        (*shown + 1) % len
                    } else {
                        (*shown + len - 1) % len
                    };
                    panel.set_draft(&lines[*shown]);
                } else {
                    let lines = intents::complete(panel.draft(), view);
                    if lines.is_empty() {
                        self.completion = None;
                        self.said = Some(if panel.draft().is_empty() {
                            "Type @ or / and Tab completes seats and commands.".into()
                        } else {
                            "Nothing completes that.".into()
                        });
                    } else {
                        let shown = if forward { 0 } else { lines.len() - 1 };
                        panel.set_draft(&lines[shown]);
                        self.said = None;
                        self.completion = Some((lines, shown));
                    }
                }
                Some(Vec::new())
            }
            Key::Up if !self.history.is_empty() => {
                let index = match self.recall {
                    None => {
                        self.stash = panel.draft().to_owned();
                        self.history.len() - 1
                    }
                    Some(index) => index.saturating_sub(1),
                };
                self.recall = Some(index);
                panel.set_draft(&self.history[index]);
                Some(Vec::new())
            }
            Key::Down if self.recall.is_some() => {
                let index = self.recall.unwrap_or(0) + 1;
                if index < self.history.len() {
                    self.recall = Some(index);
                    panel.set_draft(&self.history[index]);
                } else {
                    self.recall = None;
                    let stash = std::mem::take(&mut self.stash);
                    panel.set_draft(&stash);
                }
                Some(Vec::new())
            }
            Key::Char(digit @ '1'..='9') if self.picking.is_some() && panel.draft().is_empty() => {
                let index = digit as usize - '1' as usize;
                let label = view.repositories.get(index)?.workspace.clone();
                self.said = None;
                Some(self.pick_repository(&label))
            }
            Key::Escape if self.picking.is_some() => {
                // The goal waits as a draft; the panel still takes Escape.
                if let Some(text) = self.picking.take()
                    && panel.draft().is_empty()
                {
                    panel.set_draft(&text);
                }
                None
            }
            _ => None,
        }
    }

    fn decision_key(
        &mut self,
        key: Key,
        panel: &mut Panel,
        view: Option<&View>,
        now: Instant,
    ) -> Option<Vec<Effect>> {
        let open = self.answering(view)?;
        match key {
            Key::Tab | Key::BackTab => {
                self.said = None;
                self.cycle(if key == Key::Tab { 1 } else { -1 }, view);
                Some(Vec::new())
            }
            Key::Char(digit @ '1'..='9') if self.operate && panel.draft().is_empty() => {
                let number = digit as usize - '0' as usize;
                let id = open.decision.clone();
                let takes = self
                    .flows
                    .entry(id.clone())
                    .or_insert_with(|| flow(open))
                    .takes_number(number);
                if !takes {
                    // Not an option here: the key types into the composer.
                    return None;
                }
                if self.locked(open, now) {
                    self.said = Some(
                        "That key came too soon after the decision came up, so it was not \
                         taken. Press it again."
                            .into(),
                    );
                    return Some(Vec::new());
                }
                self.said = None;
                let step = self
                    .flows
                    .get_mut(&id)
                    .map_or(decision::Step::Stay, |flow| flow.press_number(number));
                Some(self.step(&id, step, view, ""))
            }
            Key::Enter if !panel.draft().trim().is_empty() && self.locked(open, now) => {
                self.said = Some(
                    "Enter came too soon after the decision came up, so nothing was sent. \
                     Press it again."
                        .into(),
                );
                Some(Vec::new())
            }
            _ => None,
        }
    }

    fn review_key(
        &mut self,
        key: Key,
        panel: &mut Panel,
        review: Option<&TaskReview>,
    ) -> Option<Vec<Effect>> {
        if panel.tab() != Tab::Changes || !panel.draft().is_empty() || self.typing {
            if key == Key::Escape {
                self.typing = false;
            }
            return None;
        }
        review?;
        let top = self.cursor.unwrap_or_else(|| panel.diff_top());
        let doc = panel.diff_document()?;
        let jump = match key {
            Key::Char('n') => landmark(doc, top, changes::Kind::File, true),
            Key::Char('p') => landmark(doc, top, changes::Kind::File, false),
            Key::Char(']') => landmark(doc, top, changes::Kind::Hunk, true),
            Key::Char('[') => landmark(doc, top, changes::Kind::Hunk, false),
            Key::Char('c') => {
                let at = panel.selected_line().or(self.cursor).unwrap_or(top);
                return Some(match path_at(doc, at) {
                    Some(path) => {
                        self.said = Some(format!("Copied `{path}`."));
                        vec![Effect::Copy(path)]
                    }
                    None => {
                        self.said = Some("No file is shown to copy the path of.".into());
                        Vec::new()
                    }
                });
            }
            Key::Enter => {
                self.typing = true;
                self.said =
                    Some("Type your note, or a comment on the picked line; Enter keeps it.".into());
                return Some(Vec::new());
            }
            _ => return None,
        };
        if let Some(line) = jump {
            self.cursor = Some(line);
            panel.scroll_diff_to(line);
        }
        Some(Vec::new())
    }

    /// Says `text` in the panel, such as why a send was refused before it
    /// left.
    pub fn say(&mut self, text: impl Into<String>) {
        self.said = Some(text.into());
    }
}

/// At most the first 12 characters of an identity, for a button.
fn short(id: &str) -> &str {
    id.char_indices().nth(12).map_or(id, |(end, _)| &id[..end])
}

/// The next (or, backward, the previous) line of `kind` after (before)
/// line `from`, wrapping around the diff.
fn landmark(
    doc: &changes::Document,
    from: usize,
    kind: changes::Kind,
    forward: bool,
) -> Option<usize> {
    let found: Vec<usize> = doc
        .lines()
        .iter()
        .enumerate()
        .filter(|(_, line)| line.kind == kind)
        .map(|(index, _)| index)
        .collect();
    if forward {
        found
            .iter()
            .copied()
            .find(|&index| index > from)
            .or_else(|| found.first().copied())
    } else {
        found
            .iter()
            .rev()
            .copied()
            .find(|&index| index < from)
            .or_else(|| found.last().copied())
    }
}

/// The path of the file diff line `at` belongs to.
fn path_at(doc: &changes::Document, at: usize) -> Option<String> {
    let lines = doc.lines();
    let end = at.min(lines.len().checked_sub(1)?);
    let header = lines[..=end]
        .iter()
        .rev()
        .find(|line| line.kind == changes::Kind::File)
        .or_else(|| lines.iter().find(|line| line.kind == changes::Kind::File))?;
    header
        .text
        .rsplit_once(" b/")
        .map(|(_, path)| path.trim().trim_matches('"').to_owned())
        .filter(|path| !path.is_empty())
}

/// Shows `kind` for `view` in `panel`: its title, rows, and for the review,
/// the diff in the changes tab.
pub fn fill(panel: &mut Panel, kind: &PanelKind, view: Option<&View>, review: Option<&TaskReview>) {
    panel.set_title(&title(kind, view));
    panel.set_rows(rows(kind, view, review));
    if *kind == PanelKind::Review {
        let diff = review.map_or("", |r| r.diff.as_str());
        if panel.diff_source() != Some(diff) {
            panel.set_diff(diff);
        }
    }
}

/// A new panel showing `kind`; the review opens on its changes tab.
#[must_use]
pub fn open(kind: &PanelKind, view: Option<&View>, review: Option<&TaskReview>) -> Panel {
    let mut panel = Panel::new(&title(kind, view));
    fill(&mut panel, kind, view, review);
    if *kind == PanelKind::Review && review.is_some() {
        panel.apply(Intent::Show(Tab::Changes));
    }
    panel
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zones::everglade::studio::Attention;

    #[test]
    fn the_roster_and_the_lamps_rank_seats_alike() {
        let all = [
            Activity::Idle,
            Activity::Reading,
            Activity::Editing,
            Activity::Running,
            Activity::Testing,
            Activity::Judging,
            Activity::Thinking,
            Activity::Waiting,
            Activity::Blocked,
            Activity::Paused,
            Activity::Done,
            Activity::Failed,
        ];
        let lamp_rank = |a: Attention| match a {
            Attention::AwaitingInput => 0,
            Attention::Errored => 1,
            Attention::Working => 2,
            Attention::Completed => 4,
            Attention::Idle => 5,
        };
        for activity in all {
            assert_eq!(
                indicator(activity).rank(),
                lamp_rank(Attention::of(activity)),
                "{activity:?}"
            );
        }
    }

    fn studio() -> View {
        use coder_access::studio::{Goal, GoalStatus, Role, Station};
        View {
            goals: vec![Goal {
                goal: "g1-0011aabb".into(),
                text: "Add a flag".into(),
                workspace: "app".into(),
                lead: "lead".into(),
                status: GoalStatus::Running,
                final_tasks: 1,
                total_tasks: 2,
                submitted_at: 1_790_000_000,
                spend: Default::default(),
            }],
            seats: vec![Seat {
                seat: "ada".into(),
                role: Role::Worker,
                route: "codex:gpt-6".into(),
                look: "default".into(),
                desk: 1,
                activity: Activity::Waiting,
                station: Station::Podium,
                task: Some("studio-g1-0011aabb-b".into()),
                paused: false,
                spend: Default::default(),
            }],
            tasks: vec![
                Task {
                    task: "studio-g1-0011aabb-a".into(),
                    goal: "g1-0011aabb".into(),
                    entry: "a".into(),
                    position: 1,
                    title: "Parse the flag".into(),
                    seat: "ada".into(),
                    depends_on: Vec::new(),
                    status: TaskStatus::Done,
                    spend: Default::default(),
                },
                Task {
                    task: "studio-g1-0011aabb-b".into(),
                    goal: "g1-0011aabb".into(),
                    entry: "b".into(),
                    position: 2,
                    title: "Ship the flag".into(),
                    seat: "ada".into(),
                    depends_on: Vec::new(),
                    status: TaskStatus::Waiting,
                    spend: Default::default(),
                },
            ],
            decisions: vec![Decision {
                decision: "studio-g1-0011aabb-b".into(),
                goal: "g1-0011aabb".into(),
                task: Some("studio-g1-0011aabb-b".into()),
                seat: Some("ada".into()),
                kind: DecisionKind::Approval,
                text: "Add CHANGELOG.md?".into(),
                based_on: 7,
                approval: None,
            }],
            ..View::default()
        }
    }

    fn review() -> TaskReview {
        TaskReview {
            task: "studio-g1-0011aabb-a".into(),
            base: "a".repeat(40),
            head_commit: "c".repeat(40),
            head: "b".repeat(40),
            files: Vec::new(),
            files_total: 1,
            added: 1,
            removed: 0,
            uncounted: 0,
            diff: "diff --git a/flag.rs b/flag.rs\n--- a/flag.rs\n+++ b/flag.rs\n@@ -1,1 +1,2 @@\n fn main() {}\n+fn flag() {}\n".into(),
            completeness: coder_access::review::Completeness::Complete,
            publication: None,
        }
    }

    const ALL: [Right; 3] = [Right::Observe, Right::Operate, Right::Review];

    #[test]
    fn a_seat_panel_offers_its_intents_only_with_the_operate_right() {
        let view = studio();
        let kind = PanelKind::Desk(1);
        let mut panel = Panel::new("seat");
        let mut controller = Controller::new(kind.clone());
        controller.fill(&mut panel, 1, Some(&view), None, &[Right::Observe], None);
        assert!(controller.controls.is_empty(), "observing offers nothing");
        assert!(panel.draft().is_empty());
        controller.fill(&mut panel, 1, Some(&view), None, &ALL, None);
        assert_eq!(
            controller.intent(Intent::Action(0), &mut panel, Some(&view), None),
            vec![Effect::Send(Action::Pause("ada".into()))]
        );
        assert_eq!(
            controller.intent(Intent::Action(1), &mut panel, Some(&view), None),
            vec![Effect::Send(Action::Stop("ada".into()))]
        );
        for ch in "Keep it short".chars() {
            panel.key(crate::panels::Key::Char(ch));
        }
        assert_eq!(
            controller.intent(Intent::Submit, &mut panel, Some(&view), None),
            vec![Effect::Send(Action::Message {
                seat: Some("ada".into()),
                text: "Keep it short".into()
            })]
        );
    }

    #[test]
    fn the_podium_answers_the_oldest_decision_through_its_flow() {
        let view = studio();
        let mut panel = Panel::new("decisions");
        let mut controller = Controller::new(PanelKind::Decisions);
        controller.fill(&mut panel, 1, Some(&view), None, &ALL, None);
        // An approval's first option is **Allow once**.
        assert_eq!(
            controller.intent(Intent::Action(0), &mut panel, Some(&view), None),
            vec![Effect::Send(Action::Answer {
                decision: "studio-g1-0011aabb-b".into(),
                based_on: 7,
                text: decision::ALLOWED.into()
            })]
        );
    }

    const RULE: &str = "ada may run shell `cargo test` in /work/repo without asking again";

    /// The fixture's approval with its step named, as a host shows it.
    fn named(risk: Risk, always: Option<&str>) -> View {
        let mut view = studio();
        view.decisions[0].approval = Some(Approval {
            tool: "shell".into(),
            command: "cargo test".into(),
            cwd: "/work/repo".into(),
            reason: "checks the flag".into(),
            risk,
            always: always.map(str::to_owned),
        });
        view
    }

    #[test]
    fn the_podium_shows_a_named_step_with_its_risk_and_rule() {
        let view = named(Risk::Medium, Some(RULE));
        let text = decision_body(&view.decisions[0]);
        for shown in [
            "**shell** · Medium risk",
            "cargo test",
            "checks the flag",
            "In `/work/repo`",
            "\"Always allow\" covers:",
            "2. Always allow",
            RULE,
        ] {
            assert!(text.contains(shown), "{shown}: {text}");
        }
        let high = named(Risk::High, None);
        let text = decision_body(&high.decisions[0]);
        assert!(text.contains("High risk"), "{text}");
        assert!(!text.contains("covers:"), "{text}");
    }

    #[test]
    fn always_allow_sends_the_offered_rule_for_the_host_to_record() {
        let view = named(Risk::Low, Some(RULE));
        let mut panel = Panel::new("decisions");
        let mut controller = Controller::new(PanelKind::Decisions);
        controller.fill(&mut panel, 1, Some(&view), None, &ALL, None);
        // The options come first: Allow once, Always allow,
        // and Deny.
        assert_eq!(
            controller.intent(Intent::Action(1), &mut panel, Some(&view), None),
            vec![Effect::Send(Action::AllowAlways {
                decision: "studio-g1-0011aabb-b".into(),
                based_on: 7,
                rule: RULE.into(),
            })]
        );
        let mut panel = Panel::new("decisions");
        let mut controller = Controller::new(PanelKind::Decisions);
        controller.fill(&mut panel, 1, Some(&view), None, &ALL, None);
        assert_eq!(
            controller.intent(Intent::Action(0), &mut panel, Some(&view), None),
            vec![Effect::Send(Action::Answer {
                decision: "studio-g1-0011aabb-b".into(),
                based_on: 7,
                text: decision::ALLOWED.into()
            })],
            "Allow once keeps no rule"
        );
    }

    #[test]
    fn request_changes_carries_the_note_and_the_line_comments() {
        let view = studio();
        let review = review();
        let mut panel = Panel::new("review");
        let mut controller = Controller::new(PanelKind::Review);
        controller.fill(&mut panel, 1, Some(&view), Some(&review), &ALL, None);
        // Nothing to ask for yet.
        let request = 1;
        assert!(
            controller
                .intent(
                    Intent::Action(request),
                    &mut panel,
                    Some(&view),
                    Some(&review)
                )
                .is_empty()
        );
        for ch in "Name the flag.".chars() {
            panel.key(crate::panels::Key::Char(ch));
        }
        assert!(
            controller
                .intent(Intent::Submit, &mut panel, Some(&view), Some(&review))
                .is_empty(),
            "a note waits for the decision"
        );
        let effects = controller.intent(
            Intent::Action(request),
            &mut panel,
            Some(&view),
            Some(&review),
        );
        let [
            Effect::Send(Action::Decide {
                verdict: Verdict::RequestChanges,
                text,
                review: decided,
            }),
        ] = effects.as_slice()
        else {
            panic!("a request for changes, got {effects:?}");
        };
        assert!(text.contains("Name the flag."));
        assert_eq!(decided.head, review.head);
        // Without the `review` right the station offers no decision.
        controller.fill(
            &mut panel,
            2,
            Some(&view),
            Some(&review),
            &[Right::Observe],
            None,
        );
        assert!(controller.controls.is_empty());
    }

    #[test]
    fn a_refusal_shows_its_code() {
        let answer = Answer {
            ticket: 3,
            operation: "studio.merge.decide",
            result: Err(coder_access::Error::new(
                Code::Stale,
                "the host refused the merge decision",
            )),
        };
        let text = status_text(&answer);
        assert!(text.contains("`stale`"), "{text}");
        assert!(text.contains("nothing landed"), "{text}");
        assert_eq!(code_word(Code::MissingRight), "missing_right");
    }

    #[test]
    fn the_console_and_a_seat_panel_show_what_was_spent() {
        let mut view = studio();
        view.goals[0].spend = Spend {
            microusd: 1_500_000,
            unpriced: 0,
        };
        view.seats[0].spend = Spend {
            microusd: 1_250_000,
            unpriced: 1,
        };
        view.tasks[1].spend = Spend {
            microusd: 730_000,
            unpriced: 0,
        };
        assert_eq!(spent(&view).label(), "$1.50");
        let console = rows(&PanelKind::Console, Some(&view), None);
        assert_eq!(console[0].key, "spend");
        assert!(format!("{:?}", console[0]).contains("$1.50"));
        let goal = console.iter().find(|row| row.key.starts_with("goal-"));
        assert!(format!("{goal:?}").contains("$1.50"));
        let seat = rows(&PanelKind::Desk(1), Some(&view), None);
        let body = format!("{:?}", seat[0]);
        assert!(body.contains("$1.25+"), "{body}");
        assert!(body.contains("$0.73"), "{body}");
    }

    #[test]
    fn a_seat_panel_shows_whether_its_engine_read_each_message() {
        use coder_access::studio::{DeliveryMode, DeliveryState, SeatMessage, message_key};
        let mut view = studio();
        view.messages = vec![
            SeatMessage {
                message: message_key(4),
                seat: "ada".into(),
                from: None,
                at: 1_790_000_000,
                text: "Keep it short".into(),
                mode: DeliveryMode::MidTurn,
                state: DeliveryState::Accepted,
                task: Some("studio-g1-0011aabb-b".into()),
            },
            SeatMessage {
                message: message_key(5),
                seat: "grace".into(),
                from: Some("ada".into()),
                at: 1_790_000_001,
                text: "Not for this seat".into(),
                mode: DeliveryMode::TurnBoundary,
                state: DeliveryState::Waiting,
                task: None,
            },
        ];
        let seat = rows(&PanelKind::Desk(1), Some(&view), None);
        let shown: Vec<String> = seat
            .iter()
            .filter(|row| row.key.starts_with("message-"))
            .map(|row| format!("{row:?}"))
            .collect();
        assert_eq!(shown.len(), 1, "only this seat's messages");
        assert!(shown[0].contains("Keep it short"), "{}", shown[0]);
        assert!(
            shown[0].contains("sent mid-turn, not read yet"),
            "{}",
            shown[0]
        );
        view.messages[0].state = DeliveryState::Consumed;
        let seat = rows(&PanelKind::Desk(1), Some(&view), None);
        let read = seat
            .iter()
            .find(|row| row.key.starts_with("message-"))
            .unwrap();
        assert!(format!("{read:?}").contains("read mid-turn"));
    }

    #[test]
    fn an_unloaded_studio_says_so_in_every_panel() {
        for kind in [
            PanelKind::Console,
            PanelKind::Desk(0),
            PanelKind::Decisions,
            PanelKind::Review,
        ] {
            let rows = rows(&kind, None, None);
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].key, "not-loaded");
        }
    }

    fn type_in(panel: &mut Panel, text: &str) {
        for ch in text.chars() {
            panel.key(Key::Char(ch));
        }
    }

    /// Hands `key` to the controller first, then to the panel, as the app
    /// does, and carries out a submit.
    fn press_key(
        controller: &mut Controller,
        panel: &mut Panel,
        key: Key,
        view: &View,
        review: Option<&TaskReview>,
        now: Instant,
    ) -> Vec<Effect> {
        if let Some(effects) = controller.key(key, panel, Some(view), review, now) {
            return effects;
        }
        match panel.key(key) {
            Some(intent) => controller.intent(intent, panel, Some(view), review),
            None => Vec::new(),
        }
    }

    fn console_view() -> View {
        use coder_access::studio::Repository;
        let mut view = studio();
        view.repositories = vec![Repository {
            workspace: "app".into(),
            goals: 1,
            open_tasks: 1,
        }];
        view
    }

    #[test]
    fn the_console_completes_recalls_and_acknowledges_in_place() {
        let view = console_view();
        let mut panel = Panel::new("console");
        let mut controller = Controller::new(PanelKind::Console);
        controller.fill(&mut panel, 1, Some(&view), None, &ALL, None);
        let now = Instant::now();
        // Tab completes a seat.
        type_in(&mut panel, "@a");
        press_key(&mut controller, &mut panel, Key::Tab, &view, None, now);
        assert_eq!(panel.draft(), "@ada");
        // Tab cycles commands, forward and back.
        panel.take_draft();
        type_in(&mut panel, "/st");
        press_key(&mut controller, &mut panel, Key::Tab, &view, None, now);
        assert_eq!(panel.draft(), "/status");
        press_key(&mut controller, &mut panel, Key::Tab, &view, None, now);
        assert_eq!(panel.draft(), "/stop");
        press_key(&mut controller, &mut panel, Key::BackTab, &view, None, now);
        assert_eq!(panel.draft(), "/status");
        // A message goes out, and its acknowledgment takes the draft's place.
        panel.take_draft();
        type_in(&mut panel, "@ada Keep it short");
        let effects = press_key(&mut controller, &mut panel, Key::Enter, &view, None, now);
        assert_eq!(
            effects,
            vec![Effect::Send(Action::Message {
                seat: Some("ada".into()),
                text: "Keep it short".into()
            })]
        );
        assert!(panel.draft().is_empty());
        controller.sent(Ok(7), &mut panel);
        let refused = Answer {
            ticket: 7,
            operation: "studio.seat.message",
            result: Err(coder_access::Error::new(Code::MissingRight, "no operate")),
        };
        controller.fill(&mut panel, 2, Some(&view), None, &ALL, Some(&refused));
        assert_eq!(
            panel.draft(),
            "@ada Keep it short",
            "a refused draft comes back"
        );
        assert!(controller.ack.as_deref().unwrap().starts_with("Refused"));
        // Up walks the history; Down comes back to the draft.
        panel.take_draft();
        type_in(&mut panel, "half");
        press_key(&mut controller, &mut panel, Key::Up, &view, None, now);
        assert_eq!(panel.draft(), "@ada Keep it short");
        press_key(&mut controller, &mut panel, Key::Down, &view, None, now);
        assert_eq!(panel.draft(), "half");
        // A refused line comes back to be fixed.
        panel.take_draft();
        type_in(&mut panel, "/dance");
        assert!(press_key(&mut controller, &mut panel, Key::Enter, &view, None, now).is_empty());
        assert_eq!(panel.draft(), "/dance");
        // What the console keeps while closed comes back.
        let kept = controller.recall(&panel);
        let mut again = Panel::new("console");
        let mut reopened = Controller::new(PanelKind::Console);
        reopened.fill(&mut again, 3, Some(&view), None, &ALL, None);
        reopened.restore(kept, &mut again);
        assert_eq!(again.draft(), "/dance");
        assert_eq!(reopened.history.last().map(String::as_str), Some("/dance"));
    }

    #[test]
    fn a_goal_with_several_repositories_waits_for_a_pick() {
        use coder_access::studio::Repository;
        let mut view = console_view();
        view.repositories.push(Repository {
            workspace: "site".into(),
            goals: 0,
            open_tasks: 0,
        });
        let mut panel = Panel::new("console");
        let mut controller = Controller::new(PanelKind::Console);
        controller.fill(&mut panel, 1, Some(&view), None, &ALL, None);
        type_in(&mut panel, "Add a flag");
        let now = Instant::now();
        assert!(press_key(&mut controller, &mut panel, Key::Enter, &view, None, now).is_empty());
        controller.fill(&mut panel, 1, Some(&view), None, &ALL, None);
        assert_eq!(
            controller.controls,
            vec![
                Control::Repository("app".into()),
                Control::Repository("site".into())
            ]
        );
        let effects = press_key(
            &mut controller,
            &mut panel,
            Key::Char('2'),
            &view,
            None,
            now,
        );
        assert_eq!(
            effects,
            vec![Effect::Send(Action::SubmitGoal {
                text: "Add a flag".into(),
                workspace: "site".into()
            })]
        );
        assert_eq!(controller.workspace.as_deref(), Some("site"));
    }

    #[test]
    fn the_console_spawns_and_opens_the_podium() {
        let view = console_view();
        let mut panel = Panel::new("console");
        let mut controller = Controller::new(PanelKind::Console);
        controller.fill(&mut panel, 1, Some(&view), None, &ALL, None);
        let now = Instant::now();
        type_in(&mut panel, "/spawn @ada Read the docs");
        assert_eq!(
            press_key(&mut controller, &mut panel, Key::Enter, &view, None, now),
            vec![
                Effect::Send(Action::Resume("ada".into())),
                Effect::Send(Action::Message {
                    seat: Some("ada".into()),
                    text: "Read the docs".into()
                })
            ]
        );
        type_in(&mut panel, "/decide");
        assert_eq!(
            press_key(&mut controller, &mut panel, Key::Enter, &view, None, now),
            vec![Effect::Open(PanelKind::Decisions)]
        );
        type_in(&mut panel, "/sound off");
        assert_eq!(
            press_key(&mut controller, &mut panel, Key::Enter, &view, None, now),
            vec![Effect::Sound(false)]
        );
    }

    fn two_decisions() -> View {
        let mut view = studio();
        view.decisions.push(Decision {
            decision: "g1-0011aabb".into(),
            goal: "g1-0011aabb".into(),
            task: None,
            seat: Some("ada".into()),
            kind: DecisionKind::NoPlan,
            text: "Answer with a plan.".into(),
            based_on: 3,
            approval: None,
        });
        view.canonicalize();
        view
    }

    #[test]
    fn the_podium_takes_number_keys_after_its_lock_and_tab_moves_on() {
        let view = two_decisions();
        let mut panel = Panel::new("decisions");
        let mut controller = Controller::new(PanelKind::Decisions);
        controller.fill(&mut panel, 1, Some(&view), None, &ALL, None);
        // The approval comes first, before the goal's plan decision.
        let (front, since) = controller.front.clone().unwrap();
        assert_eq!(front, "studio-g1-0011aabb-b");
        // Enter alone sends nothing.
        assert!(press_key(&mut controller, &mut panel, Key::Enter, &view, None, since).is_empty());
        // A number key during the lock is dropped and says so.
        let early = since + INPUT_LOCK / 2;
        assert!(
            press_key(
                &mut controller,
                &mut panel,
                Key::Char('1'),
                &view,
                None,
                early
            )
            .is_empty()
        );
        assert!(controller.said.as_deref().unwrap().contains("too soon"));
        assert!(panel.draft().is_empty(), "the dropped key typed nothing");
        // After it, 2 denies the approval at once.
        let later = since + INPUT_LOCK * 2;
        assert_eq!(
            press_key(
                &mut controller,
                &mut panel,
                Key::Char('2'),
                &view,
                None,
                later
            ),
            vec![Effect::Send(Action::Answer {
                decision: "studio-g1-0011aabb-b".into(),
                based_on: 7,
                text: decision::DENIED.into()
            })]
        );
        // Tab moves to the next decision, and a typed answer goes there.
        controller.fill(&mut panel, 2, Some(&view), None, &ALL, None);
        press_key(&mut controller, &mut panel, Key::Tab, &view, None, later);
        controller.fill(&mut panel, 2, Some(&view), None, &ALL, None);
        let (_, since) = controller.front.clone().unwrap();
        type_in(&mut panel, "Plan below");
        let effects = press_key(
            &mut controller,
            &mut panel,
            Key::Enter,
            &view,
            None,
            since + INPUT_LOCK * 2,
        );
        assert!(matches!(
            effects.as_slice(),
            [Effect::Send(Action::Answer { decision, text, .. })]
                if decision == "g1-0011aabb" && text == "Plan below"
        ));
    }

    #[test]
    fn reject_asks_for_a_second_press() {
        let view = studio();
        let review = review();
        let mut panel = Panel::new("review");
        let mut controller = Controller::new(PanelKind::Review);
        controller.fill(&mut panel, 1, Some(&view), Some(&review), &ALL, None);
        let reject = 2;
        assert!(
            controller
                .intent(
                    Intent::Action(reject),
                    &mut panel,
                    Some(&view),
                    Some(&review)
                )
                .is_empty(),
            "the first press only arms it"
        );
        controller.fill(&mut panel, 1, Some(&view), Some(&review), &ALL, None);
        assert_eq!(
            controller.label(&controller.controls[reject], reject),
            "Confirm reject"
        );
        let effects = controller.intent(
            Intent::Action(reject),
            &mut panel,
            Some(&view),
            Some(&review),
        );
        assert!(matches!(
            effects.as_slice(),
            [Effect::Send(Action::Decide {
                verdict: Verdict::Reject,
                ..
            })]
        ));
    }

    #[test]
    fn review_keys_move_between_files_and_copy_a_path() {
        let view = studio();
        let mut review = review();
        review.diff = "diff --git a/one.rs b/one.rs\n--- a/one.rs\n+++ b/one.rs\n@@ -1,1 +1,2 @@\n fn a() {}\n+fn b() {}\n@@ -9,1 +10,2 @@\n fn c() {}\n+fn d() {}\ndiff --git a/two.rs b/two.rs\n--- a/two.rs\n+++ b/two.rs\n@@ -1,1 +1,2 @@\n fn e() {}\n+fn f() {}\n".into();
        let mut panel = Panel::new("review");
        let mut controller = Controller::new(PanelKind::Review);
        controller.fill(&mut panel, 1, Some(&view), Some(&review), &ALL, None);
        panel.apply(Intent::Show(Tab::Changes));
        let now = Instant::now();
        let hit = |controller: &mut Controller, panel: &mut Panel, key| {
            press_key(controller, panel, key, &view, Some(&review), now)
        };
        hit(&mut controller, &mut panel, Key::Char(']'));
        assert_eq!(controller.cursor, Some(3), "the first hunk");
        hit(&mut controller, &mut panel, Key::Char(']'));
        assert_eq!(controller.cursor, Some(6), "the second hunk");
        hit(&mut controller, &mut panel, Key::Char('n'));
        assert_eq!(controller.cursor, Some(9), "the second file");
        assert_eq!(
            hit(&mut controller, &mut panel, Key::Char('c')),
            vec![Effect::Copy("two.rs".into())]
        );
        hit(&mut controller, &mut panel, Key::Char('n'));
        assert_eq!(
            controller.cursor,
            Some(0),
            "moving on wraps to the first file"
        );
        hit(&mut controller, &mut panel, Key::Char('p'));
        assert_eq!(controller.cursor, Some(9));
        // Enter starts a note: then the keys type.
        hit(&mut controller, &mut panel, Key::Enter);
        hit(&mut controller, &mut panel, Key::Char('n'));
        assert_eq!(panel.draft(), "n");
    }

    #[test]
    fn the_agent_card_shows_owned_decisions_and_spawns_a_stopped_seat() {
        let mut view = studio();
        let rows = rows(&PanelKind::Seat("ada".into()), Some(&view), None);
        assert!(
            rows.iter()
                .any(|row| row.key == "owned-studio-g1-0011aabb-b")
        );
        let mut panel = Panel::new("seat");
        let mut controller = Controller::new(PanelKind::Seat("ada".into()));
        controller.fill(&mut panel, 1, Some(&view), None, &ALL, None);
        assert_eq!(
            controller.controls[2],
            Control::AnswerAt("studio-g1-0011aabb-b".into())
        );
        assert_eq!(
            controller.intent(Intent::Action(2), &mut panel, Some(&view), None),
            vec![Effect::Answer("studio-g1-0011aabb-b".into())]
        );
        // A stopped seat is paused without a task; it spawns again.
        view.seats[0].paused = true;
        view.seats[0].task = None;
        controller.fill(&mut panel, 2, Some(&view), None, &ALL, None);
        assert_eq!(controller.controls[0], Control::Spawn("ada".into()));
        assert_eq!(
            controller.intent(Intent::Action(0), &mut panel, Some(&view), None),
            vec![Effect::Send(Action::Resume("ada".into()))]
        );
    }

    #[test]
    fn task_details_send_the_task_intents_and_cancel_confirms() {
        let mut view = studio();
        view.tasks[1].status = TaskStatus::Held;
        let id = "studio-g1-0011aabb-b".to_owned();
        let kind = PanelKind::Task(id.clone());
        assert!(
            rows(&kind, Some(&view), None)
                .iter()
                .any(|row| row.key == "task")
        );
        let mut panel = Panel::new("task");
        let mut controller = Controller::new(kind.clone());
        controller.fill(&mut panel, 1, Some(&view), None, &[Right::Observe], None);
        assert!(controller.controls.is_empty(), "observing offers nothing");
        controller.fill(&mut panel, 1, Some(&view), None, &ALL, None);
        assert_eq!(
            controller.controls,
            vec![
                Control::Act(Action::Prioritize(id.clone())),
                Control::Reassign(id.clone()),
                Control::Act(Action::Cancel(id.clone())),
            ]
        );
        assert_eq!(
            controller.intent(Intent::Action(0), &mut panel, Some(&view), None),
            vec![Effect::Send(Action::Prioritize(id.clone()))]
        );
        type_in(&mut panel, "@ada");
        assert_eq!(
            controller.intent(Intent::Action(1), &mut panel, Some(&view), None),
            vec![Effect::Send(Action::Reassign {
                task: id.clone(),
                seat: "ada".into()
            })]
        );
        assert!(
            controller
                .intent(Intent::Action(2), &mut panel, Some(&view), None)
                .is_empty()
        );
        assert_eq!(
            controller.intent(Intent::Action(2), &mut panel, Some(&view), None),
            vec![Effect::Send(Action::Cancel(id.clone()))]
        );
        // A failed task retries.
        view.tasks[1].status = TaskStatus::Failed;
        controller.fill(&mut panel, 2, Some(&view), None, &ALL, None);
        assert_eq!(
            controller.controls,
            vec![Control::Act(Action::Retry(id.clone()))]
        );
    }

    #[test]
    fn the_library_pins_the_plan_with_its_progress() {
        use coder_access::studio::memory_key;
        let mut view = studio();
        view.memory = vec![
            Memory {
                entry: memory_key(1),
                kind: MemoryKind::Plan,
                author: "@lead".into(),
                goal: Some("g1-0011aabb".into()),
                text: "Plan for goal g1-0011aabb:\n- a: Parse the flag [@ada]\n- b: Ship the flag [@ada] (after a)".into(),
                pinned: true,
            },
            Memory {
                entry: memory_key(2),
                kind: MemoryKind::Convention,
                author: "the person".into(),
                goal: None,
                text: "Keep the palette amber.".into(),
                pinned: false,
            },
        ];
        let rows = rows(&PanelKind::Library, Some(&view), None);
        assert_eq!(rows[0].key, "plan", "the plan is pinned first");
        assert_eq!(rows[1].key, format!("memory-{}", memory_key(2)));
        let plan = annotated(&view.memory[0], &view);
        assert!(plan.contains("Parse the flag [@ada] · done"), "{plan}");
        assert!(plan.contains("(after a) · waiting on you"), "{plan}");
        view.memory.clear();
        assert_eq!(
            super::rows(&PanelKind::Library, Some(&view), None)[0].key,
            "no-memory"
        );
    }
}
