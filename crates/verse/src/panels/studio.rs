//! The Agent Studio's panels over Everglade (`docs/verse/agent-studio.md`,
//! "The studio in Verse"): the console at the notice board, a seat's panel
//! at its desk, the decisions at the podium, and the diff review at the
//! merge station.
//!
//! Each panel is a [`Panel`] of transcript rows in the chat palette, built
//! from the studio snapshot Everglade draws. The decisions read each
//! question or approval with the desktop app's question flow
//! (`openagents_chat_app::decision`), and the roster orders seats by the
//! app's attention value (`openagents_chat_app::attention`). The review is
//! the task's real diff in the changes tab.
//!
//! A [`Controller`] turns a panel's controls into studio intents
//! ([`intents`]): the console's composer starts goals and messages seats,
//! a seat's panel pauses, resumes, stops, and messages its seat, the
//! podium answers the oldest decision page by page with the question
//! flow, and the merge station decides **Merge**, **Request changes**
//! (with line comments from `openagents_chat_app::review_comments`), or
//! **Reject**. A panel offers a control only when the source's rights
//! hold its right, and the host checks it again. The host's newest answer,
//! or its refusal code, shows in every studio panel.

use super::{Intent, Panel, Tab, message, tool};
use crate::zones::everglade::studio::intents::{self, Action, Console};
use crate::zones::everglade::studio::{Answer, PanelKind, word};
use coder_access::review::TaskReview;
use coder_access::studio::{
    Activity, Decision, DecisionKind, Seat, Task, TaskStatus, Verdict, View,
};
use coder_access::{Code, Outcome, Right};
use openagents_chat_app::{attention, decision, review_comments};
use rust_native::{MessageRole, Node, ToolState};
use std::collections::BTreeMap;

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

fn note(key: &str, text: &str) -> Node<()> {
    message(key, MessageRole::System, text)
}

fn console(view: &View, rights: &[Right]) -> Vec<Node<()>> {
    let mut rows = Vec::new();
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
                "**Goal** · {status}\n\n{}\n\n{} of {} tasks over · lead {} · {}",
                goal.text, goal.final_tasks, goal.total_tasks, goal.lead, goal.workspace
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
                1 => "1 decision waits at the podium.".to_owned(),
                n => format!("{n} decisions wait at the podium."),
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
    let mut body = format!(
        "**{}** · {role} · {}\n\n{}{}",
        seat.seat,
        seat.route,
        word(seat.activity),
        if seat.paused { " · paused" } else { "" }
    );
    if let Some(task) = task {
        body.push_str(&format!("\n\nTask: {}", task.title));
    }
    rows.push(message("seat", MessageRole::Assistant, &body));
    let lines = view
        .logs
        .iter()
        .find(|log| log.seat == seat.seat)
        .map_or(&[][..], |log| log.lines.as_slice());
    let working = indicator(seat.activity) == attention::Indicator::Working;
    for (i, line) in lines.iter().enumerate() {
        let state = if matches!(line.activity, Activity::Failed | Activity::Blocked) {
            ToolState::Failed
        } else if working && i + 1 == lines.len() {
            ToolState::Running
        } else {
            ToolState::Done
        };
        rows.push(tool(
            &format!("log-{i}"),
            word(line.activity),
            &line.text,
            "",
            state,
        ));
    }
    if lines.is_empty() {
        rows.push(note("no-log", "Nothing in this seat's log yet."));
    }
    rows
}

/// The question flow a decision is answered with: an approval's two
/// options, or the question's pages. A goal's plan decision is answered
/// with a plan as free text.
#[must_use]
pub fn flow(open: &Decision) -> decision::Flow {
    match open.kind {
        DecisionKind::Approval => decision::Flow::approval(&open.text),
        _ => decision::Flow::question(&open.text),
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

fn decision_rows(view: &View) -> Vec<Node<()>> {
    let mut rows = Vec::new();
    for (i, open) in intents::decisions(view).into_iter().enumerate() {
        let flow = flow(open);
        let heading = heading(open);
        let pages = flow.pages();
        let mut body = heading;
        for (n, page) in pages.iter().enumerate() {
            if pages.len() > 1 {
                body.push_str(&format!("\n\n*{} of {}*", n + 1, pages.len()));
            }
            if !page.prompt.is_empty() {
                body.push_str("\n\n");
                body.push_str(&page.prompt);
            }
            for (k, option) in page.options.iter().enumerate() {
                body.push_str(&format!("\n{}. {option}", k + 1));
            }
        }
        rows.push(message(
            &format!("decision-{i}-{}", open.decision),
            MessageRole::Assistant,
            &body,
        ));
    }
    if rows.is_empty() {
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
    let title = view
        .tasks
        .iter()
        .find(|t| t.task == review.task)
        .map_or(review.task.as_str(), |t| t.title.as_str());
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
    let help = if intents::allows(rights, Right::Review) {
        "Pick a line in **What changed** and type to comment on it; with no line picked, \
         what you type is your note. **Request changes** sends the note and the comments."
    } else {
        "**Merge**, **Request changes**, and **Reject** need a host connection with the \
         `review` right."
    };
    vec![
        message("review", MessageRole::Assistant, &body),
        note("review-help", help),
    ]
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
        PanelKind::Decisions => decision_rows(view),
        PanelKind::Review => review_rows(view, review, rights),
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

/// What a studio panel's control asks the app to do.
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    /// Send this intent to the host ([`crate::zones::everglade::studio::Studio::send`]).
    Send(Action),
    /// Show this panel instead.
    Open(PanelKind),
}

fn review_identity(review: Option<&TaskReview>) -> Option<(String, String)> {
    review.map(|review| (review.task.clone(), review.head.clone()))
}

/// What one of a studio panel's action buttons does.
#[derive(Clone, Debug, PartialEq)]
enum Control {
    Act(Action),
    /// Pick an option on the decision's current page.
    Pick {
        decision: String,
        option: usize,
    },
    /// Go back a page in the decision's flow.
    Back(String),
    /// Decide the open review.
    Decide(Verdict),
}

/// The state behind one open studio panel: which panel it is, the studio
/// revision it shows, its controls, the decisions' question flows, and
/// the review's line comments and note.
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

    /// The open decision the podium answers now: the oldest.
    fn oldest<'a>(&self, view: Option<&'a View>) -> Option<&'a Decision> {
        view.and_then(|view| intents::decisions(view).into_iter().next())
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
        self.reviewed = review_identity(review);
        if let Some(view) = view {
            self.flows
                .retain(|id, _| view.decisions.iter().any(|open| open.decision == *id));
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
        let mut controls = Vec::new();
        let mut composer = None;
        match &self.kind {
            PanelKind::Console => {
                if operate && view.is_some() {
                    composer = Some("Type a goal, @seat and a message, or a /command".to_owned());
                }
                if let Some(workspace) = &self.workspace {
                    rows.push(note(
                        "console-repository",
                        &format!("Goals start on **{workspace}**."),
                    ));
                }
            }
            PanelKind::Seat(_) | PanelKind::Desk(_) => {
                if let Some(seat) = view.and_then(|view| seat(&self.kind, view))
                    && operate
                {
                    let name = seat.seat.clone();
                    controls.push(if seat.paused {
                        Control::Act(Action::Resume(name.clone()))
                    } else {
                        Control::Act(Action::Pause(name.clone()))
                    });
                    controls.push(Control::Act(Action::Stop(name.clone())));
                    composer = Some(format!("Message {name}"));
                }
            }
            PanelKind::Decisions => {
                if let Some(open) = self.oldest(view)
                    && operate
                {
                    let flow = self
                        .flows
                        .entry(open.decision.clone())
                        .or_insert_with(|| flow(open));
                    let page = flow.current();
                    rows.push(message(
                        "answering",
                        MessageRole::System,
                        &format!(
                            "Answering: {} · {}{}",
                            heading(open),
                            flow.counter(),
                            if page.options.is_empty() {
                                " · type the answer".to_owned()
                            } else {
                                " · pick an option or type the answer".to_owned()
                            }
                        ),
                    ));
                    for option in 0..page.options.len() {
                        controls.push(Control::Pick {
                            decision: open.decision.clone(),
                            option,
                        });
                    }
                    if flow.page() > 0 {
                        controls.push(Control::Back(open.decision.clone()));
                    }
                    composer = Some("Type the answer".to_owned());
                }
            }
            PanelKind::Review => {
                if let Some(review) = Self::decidable(view, review)
                    && intents::allows(rights, Right::Review)
                {
                    for verdict in [Verdict::Merge, Verdict::RequestChanges, Verdict::Reject] {
                        controls.push(Control::Decide(verdict));
                    }
                    composer = Some("Type a note, or a comment on the picked line".to_owned());
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
        }
        if let Some(said) = &self.said {
            rows.push(note("said", said));
        }
        if let Some(status) = status {
            rows.push(note("status", &status_text(status)));
        }
        let labels: Vec<(String, bool)> = controls
            .iter()
            .map(|control| {
                let label = match control {
                    Control::Act(Action::Pause(_)) => "Pause".to_owned(),
                    Control::Act(Action::Resume(_)) => "Resume".to_owned(),
                    Control::Act(Action::Stop(_)) => "Stop".to_owned(),
                    Control::Act(_) => "Send".to_owned(),
                    Control::Pick { decision, option } => self
                        .flows
                        .get(decision)
                        .and_then(|flow| flow.current().options.get(*option))
                        .map_or_else(
                            || format!("Option {}", option + 1),
                            |label| format!("{}. {label}", option + 1),
                        ),
                    Control::Back(_) => "Back".to_owned(),
                    Control::Decide(Verdict::Merge) => "Merge".to_owned(),
                    Control::Decide(Verdict::RequestChanges) => "Request changes".to_owned(),
                    Control::Decide(Verdict::Reject) => "Reject".to_owned(),
                };
                (label, true)
            })
            .collect();
        self.controls = controls;
        panel.set_rows(rows);
        panel.set_actions(labels);
        panel.set_composer(composer.as_deref());
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
                Some(Control::Act(action)) => vec![Effect::Send(action)],
                Some(Control::Pick { decision, option }) => {
                    let step = self
                        .flows
                        .get_mut(&decision)
                        .map_or(decision::Step::Stay, |flow| flow.select(option));
                    self.step(&decision, step, view)
                }
                Some(Control::Back(decision)) => {
                    if let Some(flow) = self.flows.get_mut(&decision) {
                        flow.back();
                    }
                    Vec::new()
                }
                Some(Control::Decide(verdict)) => self.decide(verdict, view, review),
                None => Vec::new(),
            },
            Intent::Submit => {
                let draft = panel.take_draft();
                self.submit(&draft, panel, view, review)
            }
            Intent::Show(_) | Intent::Close => Vec::new(),
        }
    }

    fn submit(
        &mut self,
        draft: &str,
        panel: &Panel,
        view: Option<&View>,
        review: Option<&TaskReview>,
    ) -> Vec<Effect> {
        let Some(view) = view else {
            self.said = Some("The studio has not loaded yet.".into());
            return Vec::new();
        };
        match &self.kind {
            PanelKind::Console => match intents::console(draft, view, self.workspace.as_deref()) {
                Console::Act(action) => vec![Effect::Send(action)],
                Console::Review => vec![Effect::Open(PanelKind::Review)],
                Console::Status => {
                    self.said = Some(intents::status(view));
                    Vec::new()
                }
                Console::Repository(label) => {
                    self.said = Some(format!("Goals start on {label} now."));
                    self.workspace = Some(label);
                    Vec::new()
                }
                Console::Refused(why) => {
                    self.said = Some(why);
                    Vec::new()
                }
            },
            PanelKind::Seat(_) | PanelKind::Desk(_) => match seat(&self.kind, view) {
                Some(seat) => vec![Effect::Send(Action::Message {
                    seat: Some(seat.seat.clone()),
                    text: draft.trim().to_owned(),
                })],
                None => Vec::new(),
            },
            PanelKind::Decisions => {
                let Some(open) = self.oldest(Some(view)) else {
                    self.said = Some("No decision waits on you.".into());
                    return Vec::new();
                };
                let id = open.decision.clone();
                let step = self
                    .flows
                    .entry(id.clone())
                    .or_insert_with(|| flow(open))
                    .answer_typed(draft);
                self.step(&id, step, Some(view))
            }
            PanelKind::Review => {
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
        }
    }

    /// What a step of decision `id`'s flow does: a finished flow sends its
    /// answer at the decision's own point.
    fn step(&mut self, id: &str, step: decision::Step, view: Option<&View>) -> Vec<Effect> {
        let decision::Step::Done(text) = step else {
            return Vec::new();
        };
        self.flows.remove(id);
        let Some(open) = view.and_then(|view| view.decisions.iter().find(|d| d.decision == id))
        else {
            self.said = Some("That decision was answered already.".into());
            return Vec::new();
        };
        vec![Effect::Send(Action::Answer {
            decision: open.decision.clone(),
            based_on: open.based_on,
            text,
        })]
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
        vec![Effect::Send(Action::Decide {
            review: Box::new(review.clone()),
            verdict,
            text,
        })]
    }

    /// Says `text` in the panel, such as why a send was refused before it
    /// left.
    pub fn say(&mut self, text: impl Into<String>) {
        self.said = Some(text.into());
    }
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
}
