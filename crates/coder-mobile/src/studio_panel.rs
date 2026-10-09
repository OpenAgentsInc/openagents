//! The Agent Studio's panels on phones (`docs/verse/agent-studio.md`, "The
//! studio in Verse"; `docs/verse/everglade.md`, "The workspace").
//!
//! In Everglade, the zone's **Interact** control opens the panel of the
//! station in reach: the console at the notice board, a seat's panel at its
//! desk, the decisions at the podium, and the diff review at the merge
//! station. On a phone the panel is a Rust Native view that the iOS and
//! Android hosts mount with their native renderers, as they mount the
//! chat screens. It uses only the primitives both renderers draw (stack,
//! list, text, and button), so a host needs no studio-specific code.
//!
//! The desktop's panels (`verse::panels::studio`) need the desktop painter,
//! so this module builds the same content from the same shared models: the
//! seat roster ordered by the app's attention value
//! (`openagents_chat_app::attention`) and each question or approval read
//! with the app's question flow (`openagents_chat_app::decision`).
//!
//! The panel acts through the studio's source. On a phone that source is a
//! paired computer ([`host_source`]): the Computers service's supervised
//! NIP-HOST link, under the grant the phone holds for that computer, read
//! through Verse's live studio worker. The panel offers only what the
//! grant's rights allow, and the host checks the grant again on every
//! message:
//!
//! - With `operate`: an option button for each one-page question or
//!   approval at the podium, **Pause** or **Resume** and **Stop** at a
//!   seat's desk, and typed text ([`typed`]) that answers the podium's
//!   first decision, messages the desk's seat, or, at the console,
//!   messages every seat.
//! - With `review`: **Merge** and **Reject** at the merge station, at the
//!   exact revisions its review shows, and typed text that requests
//!   changes with that note.
//!
//! The host's answer to the last intent shows as the panel's first row.

use coder_access::review::TaskReview;
use coder_access::studio::{
    Activity, DecisionKind, GoalStatus, MergeDecision, Role, Seat, Task, TaskStatus, Verdict,
    View as Studio,
};
use coder_access::{Code, Error as AccessError, Operation, Outcome, Right};
use coder_ui::theme::{Intensity, NEAR_BLACK};
use openagents_chat_app::{attention, decision};
use rust_native::style::{Color, Space, Style};
use rust_native::{Activation, Axis, Element, Node, TextRole, ValidatedView, View, ViewError};
use serde::{Deserialize, Serialize};
use tokio::runtime::Handle;
use verse::runtime::WorldRuntime;
use verse::zones::everglade::studio::live::{Live, Transport};
use verse::zones::everglade::studio::{Answer, PanelKind, intents, word};

/// The most rows a panel shows. Seats, goals, decisions, and log lines are
/// each bounded by the host; this keeps the whole view inside Rust Native's
/// node limit whatever the host sends.
pub const MAX_ROWS: usize = 240;
/// The most bytes of a diff the review shows, inside Rust Native's text
/// limit. A longer diff is cut at a line and says so.
pub const MAX_DIFF_BYTES: usize = 48 * 1024;
/// The most bytes of any other row's text.
const MAX_ROW_BYTES: usize = 8 * 1024;
/// The most bytes of a control's label.
const MAX_LABEL_BYTES: usize = 160;
/// The most bytes of text all of a panel's rows hold together, well inside
/// Rust Native's encoded view bound.
const MAX_ROWS_BYTES: usize = 320 * 1024;

/// What a studio panel's controls do. Each one but [`Intent::Close`] is
/// one NIP-HOST studio intent, bound to what the view showed when it was
/// built: the decision's point, or the review's exact revisions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Intent {
    /// Close the panel and return to the world.
    Close,
    /// Answer the open decision `decision` at `based_on` with `text`: an
    /// option of a one-page question, or an approval's **Allow once** or
    /// **Deny**.
    Answer {
        decision: String,
        based_on: u64,
        text: String,
    },
    /// **Merge** or **Reject** task `task`'s change at the revisions its
    /// review showed.
    Decide {
        task: String,
        base: String,
        head_commit: String,
        head: String,
        verdict: Verdict,
    },
    /// Pause a seat: it keeps its task and takes no new one.
    Pause { seat: String },
    /// Resume a paused seat.
    Resume { seat: String },
    /// Stop a seat: cancel its task and pause it.
    Stop { seat: String },
}

impl Intent {
    /// The operation this intent sends, issued at `now` (Unix seconds),
    /// or `None` for [`Intent::Close`]. An answer and a merge decision get
    /// a fresh command identity, which a retry of the same request keeps.
    #[must_use]
    pub fn operation(&self, now: u64) -> Option<Operation> {
        Some(match self.clone() {
            Self::Close => return None,
            Self::Answer {
                decision,
                based_on,
                text,
            } => Operation::AnswerDecision {
                decision,
                based_on,
                text,
                command: intents::mint(),
                issued_at: now,
            },
            Self::Decide {
                task,
                base,
                head_commit,
                head,
                verdict,
            } => decide(
                Reviewed {
                    task,
                    base,
                    head_commit,
                    head,
                },
                verdict,
                String::new(),
                now,
            ),
            Self::Pause { seat } => Operation::PauseSeat { seat },
            Self::Resume { seat } => Operation::ResumeSeat { seat },
            Self::Stop { seat } => Operation::StopSeat { seat },
        })
    }
}

/// The task and revisions a merge decision names.
struct Reviewed {
    task: String,
    base: String,
    head_commit: String,
    head: String,
}

impl Reviewed {
    fn of(review: &TaskReview) -> Self {
        Self {
            task: review.task.clone(),
            base: review.base.clone(),
            head_commit: review.head_commit.clone(),
            head: review.head.clone(),
        }
    }
}

/// A merge decision at the reviewed revisions, under a fresh command.
fn decide(reviewed: Reviewed, verdict: Verdict, text: String, now: u64) -> Operation {
    Operation::DecideMerge {
        decision: Box::new(MergeDecision {
            task: reviewed.task,
            base: reviewed.base,
            head_commit: reviewed.head_commit,
            head: reviewed.head,
            verdict,
            text,
            command: intents::mint(),
            issued_at: now,
        }),
    }
}

/// What a panel's controls may do: the rights the studio's host
/// connection holds, and the host's newest answer to show.
#[derive(Clone, Copy, Debug, Default)]
pub struct Controls<'a> {
    pub rights: &'a [Right],
    pub status: Option<&'a Answer>,
}

impl Controls<'_> {
    fn allows(&self, right: Right) -> bool {
        self.rights.contains(&right)
    }
}

/// An open studio panel: what it shows, the studio revision it was built
/// from, the review it shows (task and tree), and the validated view the
/// host mounts.
pub(crate) struct Open {
    pub kind: PanelKind,
    pub shown: u64,
    pub reviewed: Option<(String, String)>,
    pub view: ValidatedView<Intent>,
}

/// The current link to a paired computer, as the Computers service's
/// supervisor holds it (`coder_computers::live::Terminals::links`). It
/// answers a transport error while the computer is not connected.
pub type Links = coder_computers::terminal::session::Links;

/// The studio's [`Transport`] over a paired computer: each operation is
/// one NIP-HOST message on the supervisor's current link, signed by this
/// device and checked by the host against the device's grant.
///
/// The link mints each message's own request identity, so the `request`
/// a retry keeps does not reach the host. An answer and a merge decision
/// carry their own command identity, which the host answers again without
/// repeating the effect; a pause, resume, or stop sent twice has the same
/// effect as once, and a message sent again after a lost answer can arrive
/// twice.
pub struct HostLink {
    links: Links,
    runtime: Handle,
}

impl Transport for HostLink {
    fn call(&mut self, _request: &str, operation: &Operation) -> coder_access::Result<Outcome> {
        operation.validate()?;
        let link = (self.links)()?;
        let outcome = self
            .runtime
            .block_on(link.call(operation.clone()))
            .map_err(refusal)?;
        if !outcome.answers(operation) {
            return Err(AccessError::new(
                Code::Malformed,
                "the computer answered a different request",
            ));
        }
        outcome.validate()?;
        Ok(outcome)
    }
}

/// A host client's failure as the studio reports it: the host's own
/// refusal with its code, or `transport` while the computer is not
/// reachable, which the studio's worker retries once.
fn refusal(error: coder_host::Error) -> AccessError {
    match error {
        coder_host::Error::Access(error) => error,
        coder_host::Error::Closed(Some(code)) if code == "revoked" || code == "stale" => {
            AccessError::new(Code::Revoked, "the computer ended this phone's access")
        }
        coder_host::Error::Closed(_)
        | coder_host::Error::Transport(_)
        | coder_host::Error::Reach(_) => {
            AccessError::new(Code::Transport, "the computer did not answer")
        }
        _ => AccessError::new(Code::Unavailable, "this phone cannot reach the computer"),
    }
}

/// Everglade's studio from the paired computer `links` reaches, with the
/// rights this device's grant for it holds. Its worker runs each operation
/// on `runtime` from a thread of its own, so the caller may hold any
/// runtime's handle. It starts observing when the player enters Everglade.
///
/// # Errors
/// The computer is not connected now; a later call can try again.
pub fn host_source(links: Links, runtime: Handle) -> Result<(Live, Vec<Right>), AccessError> {
    let rights: Vec<Right> = links()?.device().access().grant.rights.iter().collect();
    let source = Live::new(
        Box::new(move || {
            Box::new(HostLink {
                links: links.clone(),
                runtime: runtime.clone(),
            }) as Box<dyn Transport>
        }),
        rights.clone(),
    );
    Ok((source, rights))
}

impl crate::verse_app::Scene {
    /// Makes the paired computer `links` reaches Everglade's studio
    /// source ([`host_source`]), replacing any other. Returns the rights
    /// the panels may use.
    ///
    /// # Errors
    /// The computer is not connected now.
    pub(crate) fn connect_studio(
        &mut self,
        links: Links,
        runtime: Handle,
    ) -> Result<Vec<Right>, AccessError> {
        let (source, rights) = host_source(links, runtime)?;
        self.world.set_studio_source(Box::new(source));
        Ok(rights)
    }
}

impl crate::verse_ffi::VerseHandle {
    /// Connects Everglade's Agent Studio to the paired computer `links`
    /// reaches, under this device's grant for it, so the studio panels can
    /// act as that grant allows. `runtime` runs the host calls, from the
    /// studio's own thread. Returns the grant's rights.
    ///
    /// # Errors
    /// The computer is not connected now.
    pub fn connect_studio(
        &mut self,
        links: Links,
        runtime: Handle,
    ) -> Result<Vec<Right>, AccessError> {
        self.scene.connect_studio(links, runtime)
    }
}

/// The attention a seat doing `activity` shows in the roster: the app's
/// indicator over the seat's activity. A seat whose task is over is shown
/// as seen, so only a waiting, failed, or working seat stands out. This is
/// the same mapping the desktop roster uses.
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
pub fn seat<'a>(kind: &PanelKind, view: &'a Studio) -> Option<&'a Seat> {
    match kind {
        PanelKind::Seat(name) => view.seats.iter().find(|s| s.seat == *name),
        PanelKind::Desk(desk) => view.seats.iter().find(|s| s.desk == *desk),
        _ => None,
    }
}

/// The panel's title.
#[must_use]
pub fn title(kind: &PanelKind, view: Option<&Studio>) -> String {
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
        PanelKind::Task(id) => view
            .and_then(|v| v.tasks.iter().find(|t| &t.task == id))
            .map_or_else(|| "Task".into(), |task| task.title.clone()),
        PanelKind::Library => "Library".into(),
    }
}

/// One task's details on a phone: its seat, status, and dependencies. The
/// task actions stay on the desktop's task panel.
fn task_rows(view: &Studio, id: &str) -> Vec<Node<Intent>> {
    let Some(task) = view.tasks.iter().find(|t| t.task == id) else {
        return vec![note(
            "task-missing",
            "This task is no longer on the Task Wall.",
        )];
    };
    let mut body = format!("**{}**\n\n{} · {:?}", task.title, task.seat, task.status);
    if !task.depends_on.is_empty() {
        body.push_str(&format!("\n\nWaits on: {}", task.depends_on.join(", ")));
    }
    vec![markdown("studio-task".into(), &body)]
}

/// The shared memory on a phone: the pinned plan first, then the newest
/// entries.
fn library_rows(view: &Studio) -> Vec<Node<Intent>> {
    if view.memory.is_empty() {
        return vec![note(
            "library-empty",
            "Nothing is in the shared memory yet.",
        )];
    }
    let mut entries: Vec<_> = view.memory.iter().collect();
    entries.sort_by_key(|m| (!m.pinned, std::cmp::Reverse(m.entry.clone())));
    entries
        .into_iter()
        .enumerate()
        .map(|(i, memory)| {
            let heading = if memory.pinned { "Plan" } else { "Note" };
            markdown(
                format!("studio-memory-{i}"),
                &format!("**{heading}** · {}\n\n{}", memory.author, memory.text),
            )
        })
        .collect()
}

/// When the goal named `goal` was submitted, for ordering.
fn submitted(view: &Studio, goal: &str) -> u64 {
    view.goals
        .iter()
        .find(|g| g.goal == goal)
        .map_or(0, |g| g.submitted_at)
}

/// The tasks the merge station can review, newest first: plan tasks that
/// are done.
#[must_use]
pub fn reviewable(view: &Studio) -> Vec<&Task> {
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

/// The review the merge station shows: the newest done task whose review
/// the studio's source holds. Other panels show none.
pub(crate) fn review(world: &mut WorldRuntime, kind: &PanelKind) -> Option<TaskReview> {
    if *kind != PanelKind::Review {
        return None;
    }
    let tasks: Vec<String> = world
        .studio()
        .view()
        .map(|view| {
            reviewable(view)
                .into_iter()
                .map(|task| task.task.clone())
                .collect()
        })
        .unwrap_or_default();
    tasks.iter().find_map(|task| world.studio_review(task))
}

/// The panel of `kind` built from the studio as `world` holds it now: its
/// rows, the controls the source's rights allow, and the host's newest
/// answer, as the view `instance` at `revision`.
///
/// # Errors
/// Returns the view's validation error ([`project`]).
pub(crate) fn open(
    world: &mut WorldRuntime,
    kind: PanelKind,
    instance: &str,
    revision: u64,
) -> Result<Open, ViewError> {
    let review = review(world, &kind);
    let studio = world.studio();
    let rights = studio.rights();
    let controls = Controls {
        rights: &rights,
        status: studio.status(),
    };
    let view = project(
        &kind,
        studio.view(),
        review.as_ref(),
        &controls,
        instance,
        revision,
    )?;
    Ok(Open {
        shown: studio.revision(),
        reviewed: review.map(|review| (review.task, review.head)),
        kind,
        view,
    })
}

/// Whether `open` still shows the studio as `world` holds it: the same
/// studio revision and, at the merge station, the same review, which
/// arrives from the host after the panel opens.
pub(crate) fn current(open: &Open, world: &mut WorldRuntime) -> bool {
    if open.shown != world.studio().revision() {
        return false;
    }
    if open.kind != PanelKind::Review {
        return true;
    }
    review(world, &open.kind).map(|review| (review.task, review.head)) == open.reviewed
}

/// Sends the studio intent a control of the open panel runs.
///
/// # Errors
/// The intent sends nothing, or the studio refused it before sending: it
/// is not loaded, or its connection lacks the intent's right.
pub(crate) fn act(world: &mut WorldRuntime, intent: &Intent) -> Result<(), String> {
    let operation = intent
        .operation(intents::now())
        .ok_or("That control sends nothing to the computer")?;
    send(world, operation)
}

/// Sends `text`, which the person typed into the open panel: at the
/// podium, the answer to its first decision (a plan answers a goal's plan
/// decision); at a desk, a message to its seat; at the console, a message
/// to every seat; and at the merge station, **Request changes** with
/// `text` as the note, at the revisions the review shows.
///
/// # Errors
/// The panel takes no text, it names nothing to send to, or the studio
/// refused the intent before sending.
pub(crate) fn typed(world: &mut WorldRuntime, open: &Open, text: &str) -> Result<(), String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("Type something to send".into());
    }
    if open.kind == PanelKind::Review {
        let shown = review(world, &open.kind).ok_or("No review is open to decide")?;
        let operation = decide(
            Reviewed::of(&shown),
            Verdict::RequestChanges,
            text.to_owned(),
            intents::now(),
        );
        return send(world, operation);
    }
    let operation = {
        let view = world
            .studio()
            .view()
            .ok_or("The studio has not loaded yet")?;
        match &open.kind {
            PanelKind::Console => Operation::MessageSeat {
                seat: None,
                text: text.to_owned(),
            },
            PanelKind::Seat(_) | PanelKind::Desk(_) => {
                let at = seat(&open.kind, view).ok_or("No seat sits at this desk")?;
                Operation::MessageSeat {
                    seat: Some(at.seat.clone()),
                    text: text.to_owned(),
                }
            }
            PanelKind::Decisions => {
                let first = intents::decisions(view)
                    .into_iter()
                    .next()
                    .ok_or("No decision waits on you")?;
                Operation::AnswerDecision {
                    decision: first.decision.clone(),
                    based_on: first.based_on,
                    text: text.to_owned(),
                    command: intents::mint(),
                    issued_at: intents::now(),
                }
            }
            PanelKind::Review | PanelKind::Task(_) | PanelKind::Library => {
                return Err("This panel takes no text".into());
            }
        }
    };
    send(world, operation)
}

/// Sends `operation` through the studio's source. Its answer arrives at a
/// later frame as the studio's status.
fn send(world: &mut WorldRuntime, operation: Operation) -> Result<(), String> {
    world
        .studio_send(operation)
        .map(|_| ())
        .map_err(|error| error.message)
}

/// The word a refusal code is spelled with on the wire, such as `stale`.
fn code_word(code: Code) -> String {
    serde_json::to_value(code)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| format!("{code:?}"))
}

/// The host's answer to the last intent or read, as a status row.
fn status_row(answer: &Answer) -> Node<Intent> {
    let operation = answer.operation;
    let line = match &answer.result {
        Ok(Outcome::Dispatched { receipt }) => {
            format!("**Sent** · `{operation}` · {}", receipt.reference)
        }
        Ok(Outcome::Merged { merged }) => match (merged.verdict, &merged.publication) {
            (_, Some(publication)) => format!("**Merge** · {}", publication.note),
            (Verdict::RequestChanges, None) => {
                "**Changes requested** · the seat takes them as its next turn".to_owned()
            }
            (_, None) => {
                "**Rejected** · its files stay on the computer until it is archived".to_owned()
            }
        },
        Ok(_) => format!("**Done** · `{operation}`"),
        Err(error) => {
            let mut line = format!(
                "**Refused** · `{operation}` · `{}`: {}",
                code_word(error.code),
                error.message
            );
            if error.code == Code::Stale && operation == "studio.merge.decide" {
                line.push_str(
                    " The change was updated after you read it, so nothing was merged. Read \
                     the new version before you decide.",
                );
            }
            line
        }
    };
    markdown("studio-status".into(), &line)
}

/// A control that runs `intent`.
fn button(key: String, label: &str, intent: Intent) -> Node<Intent> {
    Node {
        key,
        style: Style::default(),
        element: Element::Button {
            label: bounded(label, MAX_LABEL_BYTES),
            enabled: true,
            icon: None,
            shortcut: None,
            intent,
        },
    }
}

fn color(rgb: u32) -> Color {
    Color::rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

/// `text` cut to at most `max` bytes at a character boundary.
fn bounded(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_owned();
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &text[..end])
}

fn text(key: String, value: &str, role: TextRole) -> Node<Intent> {
    Node {
        key,
        style: Style::default(),
        element: Element::Text {
            value: bounded(value, MAX_ROW_BYTES),
            role,
        },
    }
}

/// A row of Markdown the reader can select.
fn markdown(key: String, value: &str) -> Node<Intent> {
    text(key, value, TextRole::Markdown)
}

/// A short muted note.
fn note(key: &str, value: &str) -> Node<Intent> {
    let mut node = text(format!("studio-{key}"), value, TextRole::Status);
    node.style.foreground = Some(color(Intensity::Half.color()));
    node
}

fn console(view: &Studio, controls: &Controls) -> Vec<Node<Intent>> {
    let mut rows = Vec::new();
    let mut goals: Vec<_> = view.goals.iter().collect();
    goals.sort_by(|a, b| b.submitted_at.cmp(&a.submitted_at));
    for (i, goal) in goals.into_iter().enumerate() {
        let status = match goal.status {
            GoalStatus::Planning => "planning",
            GoalStatus::Decision => "waiting on a decision",
            GoalStatus::Running => "running",
            GoalStatus::Done => "done",
        };
        rows.push(markdown(
            format!("studio-goal-{i}"),
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
    for (i, seat) in seats.into_iter().enumerate() {
        let label = indicator(seat.activity).label().unwrap_or("Idle");
        rows.push(text(
            format!("studio-roster-{i}"),
            &format!(
                "{} · {label} · {} · {}",
                seat.seat,
                word(seat.activity),
                seat.route
            ),
            TextRole::Body,
        ));
    }
    match view.decisions.len() {
        0 => {}
        1 => rows.push(note("decisions-open", "1 decision waits at the podium.")),
        n => rows.push(note(
            "decisions-open",
            &format!("{n} decisions wait at the podium."),
        )),
    }
    rows.push(if controls.allows(Right::Operate) {
        note(
            "console-help",
            "Text you send from this panel messages every seat. Start a goal from the \
             desktop's console or with `openagents studio goal submit`.",
        )
    } else {
        note(
            "console-help",
            "This phone can only watch. Messaging seats needs a computer connection \
             that lets this phone operate it.",
        )
    });
    rows
}

fn seat_rows(kind: &PanelKind, view: &Studio, controls: &Controls) -> Vec<Node<Intent>> {
    let Some(seat) = seat(kind, view) else {
        return vec![note("no-seat", "No seat sits at this desk.")];
    };
    let role = match seat.role {
        Role::Lead => "lead",
        Role::Worker => "worker",
    };
    let mut body = format!(
        "**{}** · {role} · {}\n\n{}{}",
        seat.seat,
        seat.route,
        word(seat.activity),
        if seat.paused { " · paused" } else { "" }
    );
    if let Some(task) = seat
        .task
        .as_deref()
        .and_then(|id| view.tasks.iter().find(|t| t.task == id))
    {
        body.push_str(&format!("\n\nTask: {}", task.title));
    }
    let mut rows = vec![markdown("studio-seat".into(), &body)];
    if controls.allows(Right::Operate) {
        let name = seat.seat.clone();
        rows.push(if seat.paused {
            button(
                "studio-seat-resume".into(),
                "Resume",
                Intent::Resume { seat: name.clone() },
            )
        } else {
            button(
                "studio-seat-pause".into(),
                "Pause",
                Intent::Pause { seat: name.clone() },
            )
        });
        rows.push(button(
            "studio-seat-stop".into(),
            "Stop",
            Intent::Stop { seat: name },
        ));
        rows.push(note(
            "seat-help",
            "Text you send from this panel messages this seat.",
        ));
    }
    let lines = view
        .logs
        .iter()
        .find(|log| log.seat == seat.seat)
        .map_or(&[][..], |log| log.lines.as_slice());
    for (i, line) in lines.iter().enumerate() {
        let mut row = text(
            format!("studio-log-{i}"),
            &format!("{} · {}", word(line.activity), line.text),
            TextRole::Code,
        );
        if matches!(line.activity, Activity::Failed | Activity::Blocked) {
            row.style.weight = Some(rust_native::style::TextWeight::Bold);
        }
        rows.push(row);
    }
    if lines.is_empty() {
        rows.push(note("no-log", "Nothing in this seat's log yet."));
    }
    // Each message to the seat with its delivery: an accepted steer is
    // not a consumed one.
    for sent in view.messages.iter().filter(|sent| sent.seat == seat.seat) {
        rows.push(text(
            format!("studio-message-{}", sent.message),
            &format!("{}: {} · {}", sent.sender(), sent.text, sent.delivery()),
            TextRole::Body,
        ));
    }
    rows
}

/// The open decisions in the podium's order (`intents::decisions`):
/// approvals, then questions, then goal decisions, oldest first. Typed
/// text answers the first. With the operate right, a one-page question or
/// an approval offers a button for each option.
fn decision_rows(view: &Studio, controls: &Controls) -> Vec<Node<Intent>> {
    let operate = controls.allows(Right::Operate);
    let decisions = intents::decisions(view);
    let mut rows = Vec::new();
    for (i, open) in decisions.iter().enumerate() {
        let who = open.seat.as_deref().unwrap_or("The studio");
        let (flow, heading) = match open.kind {
            DecisionKind::Approval => (
                decision::Flow::approval(&open.text),
                format!("**{who}** asks to go ahead"),
            ),
            DecisionKind::Question => (
                decision::Flow::question(&open.text),
                format!("**{who}** asks"),
            ),
            DecisionKind::InvalidPlan => (
                decision::Flow::question(&open.text),
                "**The plan has a problem**: answer with a corrected plan".to_owned(),
            ),
            DecisionKind::NoPlan => (
                decision::Flow::question(&open.text),
                "**The lead finished without a plan**: answer with a plan".to_owned(),
            ),
            DecisionKind::LeadFailed => (
                decision::Flow::question(&open.text),
                "**The lead failed**: answer with a plan, or retry the lead".to_owned(),
            ),
            DecisionKind::DependencyFailed => (
                decision::Flow::question(&open.text),
                "**A dependency did not finish**: retry or cancel the task".to_owned(),
            ),
        };
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
        rows.push(markdown(format!("studio-decision-{i}"), &body));
        if !operate {
            continue;
        }
        if let [page] = pages {
            for (k, option) in page.options.iter().enumerate() {
                let mut answering = flow.clone();
                if let decision::Step::Done(text) = answering.select(k) {
                    rows.push(button(
                        format!("studio-decision-{i}-option-{}", k + 1),
                        option,
                        Intent::Answer {
                            decision: open.decision.clone(),
                            based_on: open.based_on,
                            text,
                        },
                    ));
                }
            }
        }
        if i == 0 {
            rows.push(note(
                "decision-reply",
                "Text you send from this panel answers this decision. A plan decision \
                 takes the plan as its answer.",
            ));
        }
    }
    if decisions.is_empty() {
        rows.push(note("no-decisions", "No decision waits on you."));
    } else if !operate {
        rows.push(note(
            "decisions-help",
            "Answering needs a computer connection that lets this phone operate it.",
        ));
    }
    rows
}

fn review_rows(
    view: &Studio,
    review: Option<&TaskReview>,
    controls: &Controls,
) -> Vec<Node<Intent>> {
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
    let mut rows = vec![markdown("studio-review".into(), &body)];
    if !review.diff.is_empty() {
        let (diff, cut) = diff_text(&review.diff);
        rows.push(Node {
            key: "studio-diff".into(),
            style: Style::default(),
            element: Element::Text {
                value: diff,
                role: TextRole::Code,
            },
        });
        if cut {
            rows.push(note(
                "diff-cut",
                "The diff is longer than a phone panel shows. Open the review on a \
                 computer to read all of it.",
            ));
        }
    }
    if controls.allows(Right::Review) {
        for (key, label, verdict) in [
            ("studio-merge", "Merge", Verdict::Merge),
            ("studio-reject", "Reject", Verdict::Reject),
        ] {
            rows.push(button(
                key.into(),
                label,
                Intent::Decide {
                    task: review.task.clone(),
                    base: review.base.clone(),
                    head_commit: review.head_commit.clone(),
                    head: review.head.clone(),
                    verdict,
                },
            ));
        }
        rows.push(note(
            "review-help",
            "Merge adds the change to the project's current branch on the computer \
             without pushing it. Text you send from this panel requests changes with that note.",
        ));
    } else {
        rows.push(note(
            "review-help",
            "Merge, Request changes, and Reject need a computer connection that lets this \
             phone review changes.",
        ));
    }
    rows
}

/// The diff, cut at the last whole line within [`MAX_DIFF_BYTES`], and
/// whether it was cut.
fn diff_text(diff: &str) -> (String, bool) {
    if diff.len() <= MAX_DIFF_BYTES {
        return (diff.to_owned(), false);
    }
    let mut end = MAX_DIFF_BYTES;
    while !diff.is_char_boundary(end) {
        end -= 1;
    }
    let end = diff[..end].rfind('\n').map_or(end, |at| at + 1);
    (diff[..end].to_owned(), true)
}

/// The rows a panel of `kind` shows for `view`, and for the review panel,
/// `review`, at most [`MAX_ROWS`]: the host's newest answer first, then
/// the panel's content with the controls `controls` allows.
#[must_use]
pub fn rows(
    kind: &PanelKind,
    view: Option<&Studio>,
    review: Option<&TaskReview>,
    controls: &Controls,
) -> Vec<Node<Intent>> {
    let mut rows: Vec<Node<Intent>> = controls.status.map(status_row).into_iter().collect();
    let Some(view) = view else {
        rows.push(note(
            "not-loaded",
            "The studio has not loaded. It loads while you are in Everglade with a computer \
             connected.",
        ));
        return rows;
    };
    rows.extend(match kind {
        PanelKind::Console => console(view, controls),
        PanelKind::Seat(_) | PanelKind::Desk(_) => seat_rows(kind, view, controls),
        PanelKind::Decisions => decision_rows(view, controls),
        PanelKind::Review => review_rows(view, review, controls),
        PanelKind::Task(id) => task_rows(view, id),
        PanelKind::Library => library_rows(view),
    });
    // Keep the whole view inside Rust Native's encoded bound: stop at the
    // row count or the byte budget, whichever comes first.
    let mut bytes = 0;
    let fits = rows
        .iter()
        .position(|row| {
            bytes += match &row.element {
                Element::Text { value, .. } => value.len(),
                Element::Button { label, intent, .. } => {
                    label.len()
                        + match intent {
                            Intent::Answer { text, .. } => text.len(),
                            _ => 0,
                        }
                }
                _ => 0,
            };
            bytes > MAX_ROWS_BYTES
        })
        .unwrap_or(rows.len())
        .min(MAX_ROWS);
    if fits < rows.len() {
        rows.truncate(fits.min(MAX_ROWS - 1));
        rows.push(note(
            "more",
            "There's more on the computer than this panel shows.",
        ));
    }
    rows
}

/// The panel of `kind` as a validated Rust Native view: its title and a
/// close control over a list of its rows, with the controls `controls`
/// allows. `instance` and `revision` follow Rust Native's identity rules:
/// one instance per surface lifetime, and a revision that is never reused
/// for different content.
///
/// # Errors
///
/// Returns the view's validation error. The bounds above keep a studio
/// within Rust Native's limits, so an error means a bound is wrong.
pub fn project(
    kind: &PanelKind,
    view: Option<&Studio>,
    review: Option<&TaskReview>,
    controls: &Controls,
    instance: &str,
    revision: u64,
) -> Result<ValidatedView<Intent>, ViewError> {
    let name = title(kind, view);
    let mut heading = text("studio-title".into(), &name, TextRole::Heading);
    heading.style.weight = Some(rust_native::style::TextWeight::Bold);
    let header = Node {
        key: "studio-header".into(),
        style: Style {
            gap: Some(Space::Sm),
            ..Style::default()
        },
        element: Element::Stack {
            axis: Axis::Horizontal,
            children: vec![
                heading,
                Node {
                    key: "studio-close".into(),
                    style: Style::default(),
                    element: Element::Button {
                        label: "Back to world".into(),
                        enabled: true,
                        icon: None,
                        shortcut: None,
                        intent: Intent::Close,
                    },
                },
            ],
        },
    };
    let list = Node {
        key: "studio-rows".into(),
        style: Style {
            gap: Some(Space::Sm),
            ..Style::default()
        },
        element: Element::List {
            label: name,
            children: rows(kind, view, review, controls),
        },
    };
    View::new(
        instance,
        revision,
        Node {
            key: "studio".into(),
            style: Style {
                foreground: Some(color(Intensity::Full.color())),
                background: Some(color(NEAR_BLACK)),
                padding_top: Some(Space::Md),
                padding_end: Some(Space::Md),
                padding_bottom: Some(Space::Md),
                padding_start: Some(Space::Md),
                gap: Some(Space::Sm),
                ..Style::default()
            },
            element: Element::Stack {
                axis: Axis::Vertical,
                children: vec![header, list],
            },
        },
    )
    .validate()
}

/// The intent the host's activation of `node` in the open panel runs.
///
/// # Errors
///
/// Returns a message when the activation names another view, a stale
/// revision, or a node that is not an enabled control.
pub(crate) fn activate(open: &Open, event: &Activation) -> Result<Intent, String> {
    open.view
        .activate(event)
        .cloned()
        .map_err(|_| "That studio control is no longer on screen".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use coder_access::review::{Completeness, FileCount, FileStatus};
    use coder_access::studio::{Decision, Goal, Log, LogLine, Station};

    /// A connection with no rights and no answer yet.
    const NONE: Controls<'static> = Controls {
        rights: &[],
        status: None,
    };

    fn seat(name: &str, desk: u32, activity: Activity) -> Seat {
        Seat {
            seat: name.into(),
            role: if desk == 0 { Role::Lead } else { Role::Worker },
            route: "codex:gpt-6".into(),
            look: "default".into(),
            desk,
            activity,
            station: Station::Desk,
            task: Some(format!("task-{name}")),
            paused: false,
            spend: Default::default(),
        }
    }

    fn studio() -> Studio {
        Studio {
            goals: vec![Goal {
                goal: "g1".into(),
                text: "Ship the phone panels".into(),
                workspace: "openagents".into(),
                lead: "ada".into(),
                status: GoalStatus::Running,
                final_tasks: 1,
                total_tasks: 3,
                submitted_at: 10,
                spend: Default::default(),
            }],
            seats: vec![
                seat("ada", 0, Activity::Thinking),
                seat("bo b", 1, Activity::Waiting),
                seat("cy", 2, Activity::Done),
            ],
            tasks: vec![
                Task {
                    task: "task-ada".into(),
                    goal: "g1".into(),
                    entry: "lead".into(),
                    position: 0,
                    title: "Plan".into(),
                    seat: "ada".into(),
                    depends_on: Vec::new(),
                    status: TaskStatus::Running,
                    spend: Default::default(),
                },
                Task {
                    task: "task-cy".into(),
                    goal: "g1".into(),
                    entry: "e2".into(),
                    position: 2,
                    title: "Mount the panel".into(),
                    seat: "cy".into(),
                    depends_on: Vec::new(),
                    status: TaskStatus::Done,
                    spend: Default::default(),
                },
            ],
            decisions: vec![Decision {
                decision: "task-bo b".into(),
                goal: "g1".into(),
                task: None,
                seat: Some("bo b".into()),
                kind: DecisionKind::Approval,
                text: "Run the migration?".into(),
                based_on: 3,
                approval: None,
            }],
            repositories: Vec::new(),
            logs: vec![Log {
                seat: "ada".into(),
                task: Some("task-ada".into()),
                lines: vec![
                    LogLine {
                        at: 1,
                        activity: Activity::Reading,
                        text: "read docs/verse/mobile.md".into(),
                    },
                    LogLine {
                        at: 2,
                        activity: Activity::Failed,
                        text: "cargo test failed".into(),
                    },
                ],
            }],
            memory: Vec::new(),
            messages: Vec::new(),
        }
    }

    fn keys(rows: &[Node<Intent>]) -> Vec<&str> {
        rows.iter().map(|row| row.key.as_str()).collect()
    }

    fn values(rows: &[Node<Intent>]) -> String {
        rows.iter()
            .filter_map(|row| match &row.element {
                Element::Text { value, .. } => Some(value.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn every_panel_projects_to_a_valid_view_both_renderers_draw() {
        let view = studio();
        for kind in [
            PanelKind::Console,
            PanelKind::Desk(0),
            PanelKind::Seat("bo b".into()),
            PanelKind::Decisions,
            PanelKind::Review,
        ] {
            for loaded in [None, Some(&view)] {
                let projected = project(
                    &kind,
                    loaded,
                    None,
                    &Controls::default(),
                    "verse.mount.1.studio",
                    1,
                )
                .unwrap_or_else(|e| panic!("{kind:?}: {e:?}"));
                // Android's renderer draws stacks, lists, text, and buttons
                // only, so the studio must not use any other primitive.
                let mut pending = vec![&projected.view().root];
                while let Some(node) = pending.pop() {
                    match &node.element {
                        Element::Stack { children, .. } | Element::List { children, .. } => {
                            pending.extend(children)
                        }
                        Element::Text { .. } | Element::Button { .. } => {}
                        other => panic!("{kind:?} uses {other:?}"),
                    }
                }
            }
        }
    }

    #[test]
    fn the_console_orders_the_roster_by_attention_and_counts_decisions() {
        let rows = rows(&PanelKind::Console, Some(&studio()), None, &NONE);
        let text = values(&rows);
        let waiting = text.find("bo b · Needs you").unwrap();
        let working = text.find("ada · Working").unwrap();
        let done = text.find("cy · Done").unwrap();
        assert!(waiting < working && working < done, "{text}");
        assert!(text.contains("Ship the phone panels"));
        assert!(text.contains("1 decision waits at the podium."));
        // Keys never carry seat names, which may hold any character.
        assert!(keys(&rows).iter().all(|key| !key.contains(' ')));
    }

    #[test]
    fn a_desk_shows_its_seat_and_log_tail() {
        let view = studio();
        assert_eq!(title(&PanelKind::Desk(0), Some(&view)), "ada · codex:gpt-6");
        assert_eq!(title(&PanelKind::Desk(3), Some(&view)), "Desk 4");
        let rows = rows(&PanelKind::Desk(0), Some(&view), None, &NONE);
        assert_eq!(keys(&rows), ["studio-seat", "studio-log-0", "studio-log-1"]);
        assert!(values(&rows).contains("failed · cargo test failed"));
        let empty = super::rows(&PanelKind::Desk(3), Some(&view), None, &NONE);
        assert_eq!(keys(&empty), ["studio-no-seat"]);
    }

    #[test]
    fn decisions_read_with_the_app_question_flow() {
        let rows = rows(&PanelKind::Decisions, Some(&studio()), None, &NONE);
        // Without the operate right the podium only reads, and says why.
        assert_eq!(keys(&rows), ["studio-decision-0", "studio-decisions-help"]);
        let text = values(&rows);
        assert!(text.starts_with("**bo b** asks to go ahead"), "{text}");
        assert!(text.contains("Run the migration?"));
        let none = super::rows(&PanelKind::Decisions, Some(&Studio::default()), None, &NONE);
        assert_eq!(keys(&none), ["studio-no-decisions"]);
    }

    #[test]
    fn the_review_lists_done_plan_tasks_and_bounds_its_diff() {
        let view = studio();
        let reviewable = reviewable(&view);
        assert_eq!(reviewable.len(), 1);
        assert_eq!(reviewable[0].task, "task-cy");
        let mut review = TaskReview {
            task: "task-cy".into(),
            base: "a".repeat(40),
            head_commit: "b".repeat(40),
            head: "c".repeat(40),
            files: vec![FileCount {
                path: "src/lib.rs".into(),
                status: FileStatus::Modified,
                added: Some(3),
                removed: Some(1),
            }],
            files_total: 1,
            added: 3,
            removed: 1,
            uncounted: 0,
            diff: "diff --git a/src/lib.rs b/src/lib.rs\n+one\n".into(),
            completeness: Completeness::Complete,
            publication: None,
        };
        let rows = rows(&PanelKind::Review, Some(&view), Some(&review), &NONE);
        assert_eq!(
            keys(&rows),
            ["studio-review", "studio-diff", "studio-review-help"]
        );
        assert!(values(&rows).contains("Mount the panel"));
        review.diff = "+line\n".repeat(MAX_DIFF_BYTES);
        let rows = super::rows(&PanelKind::Review, Some(&view), Some(&review), &NONE);
        assert!(keys(&rows).contains(&"studio-diff-cut"));
        let Element::Text { value, .. } = &rows[1].element else {
            panic!("the diff is text");
        };
        assert!(value.len() <= MAX_DIFF_BYTES && value.ends_with('\n'));
        assert!(
            project(
                &PanelKind::Review,
                Some(&view),
                Some(&review),
                &NONE,
                "s",
                2
            )
            .is_ok()
        );
    }

    #[test]
    fn a_large_studio_stays_within_the_view_limits() {
        let mut view = studio();
        view.logs[0].lines = (0..2_000)
            .map(|at| LogLine {
                at,
                activity: Activity::Running,
                text: "x".repeat(400),
            })
            .collect();
        let rows = rows(&PanelKind::Desk(0), Some(&view), None, &NONE);
        assert_eq!(rows.len(), MAX_ROWS);
        assert_eq!(rows.last().unwrap().key, "studio-more");
        assert!(project(&PanelKind::Desk(0), Some(&view), None, &NONE, "s", 1).is_ok());
    }

    #[test]
    fn only_the_current_close_control_activates() {
        let open = Open {
            kind: PanelKind::Console,
            shown: 0,
            reviewed: None,
            view: project(&PanelKind::Console, None, None, &NONE, "studio-a", 4).unwrap(),
        };
        let event = |instance: &str, revision, node: &str| Activation {
            instance: instance.into(),
            revision,
            node: node.into(),
        };
        assert_eq!(
            activate(&open, &event("studio-a", 4, "studio-close")),
            Ok(Intent::Close)
        );
        assert!(activate(&open, &event("studio-a", 3, "studio-close")).is_err());
        assert!(activate(&open, &event("studio-b", 4, "studio-close")).is_err());
        assert!(activate(&open, &event("studio-a", 4, "studio-title")).is_err());
    }

    fn review() -> TaskReview {
        TaskReview {
            task: "task-cy".into(),
            base: "a".repeat(40),
            head_commit: "b".repeat(40),
            head: "c".repeat(40),
            files: Vec::new(),
            files_total: 1,
            added: 1,
            removed: 0,
            uncounted: 0,
            diff: "+one\n".into(),
            completeness: Completeness::Complete,
            publication: None,
        }
    }

    /// The control keyed `key` in `rows`, and its intent.
    fn control<'a>(rows: &'a [Node<Intent>], key: &str) -> Option<&'a Intent> {
        rows.iter()
            .find(|row| row.key == key)
            .and_then(|row| match &row.element {
                Element::Button {
                    enabled: true,
                    intent,
                    ..
                } => Some(intent),
                _ => None,
            })
    }

    #[test]
    fn each_control_appears_only_under_the_right_it_needs() {
        let mut view = studio();
        let review = review();
        let operate = Controls {
            rights: &[Right::Observe, Right::Operate],
            status: None,
        };
        let reviewing = Controls {
            rights: &[Right::Observe, Right::Review],
            status: None,
        };
        // An approval offers Allow once and Deny, which answer it at its
        // point with the app's flow's words.
        let podium = rows(&PanelKind::Decisions, Some(&view), None, &operate);
        assert_eq!(
            control(&podium, "studio-decision-0-option-1"),
            Some(&Intent::Answer {
                decision: "task-bo b".into(),
                based_on: 3,
                text: decision::ALLOWED.into(),
            })
        );
        assert_eq!(
            control(&podium, "studio-decision-0-option-2"),
            Some(&Intent::Answer {
                decision: "task-bo b".into(),
                based_on: 3,
                text: decision::DENIED.into(),
            })
        );
        assert!(keys(&podium).contains(&"studio-decision-reply"));
        let reading = rows(&PanelKind::Decisions, Some(&view), None, &reviewing);
        assert!(control(&reading, "studio-decision-0-option-1").is_none());
        // A goal's plan decision has no options: typed text answers it.
        view.decisions[0].kind = DecisionKind::NoPlan;
        let plan = rows(&PanelKind::Decisions, Some(&view), None, &operate);
        assert!(control(&plan, "studio-decision-0-option-1").is_none());
        assert!(keys(&plan).contains(&"studio-decision-reply"));

        // A desk steers its seat with the operate right only.
        let desk = rows(&PanelKind::Desk(0), Some(&view), None, &operate);
        assert_eq!(
            control(&desk, "studio-seat-pause"),
            Some(&Intent::Pause { seat: "ada".into() })
        );
        assert_eq!(
            control(&desk, "studio-seat-stop"),
            Some(&Intent::Stop { seat: "ada".into() })
        );
        view.seats[0].paused = true;
        let desk = rows(&PanelKind::Desk(0), Some(&view), None, &operate);
        assert!(control(&desk, "studio-seat-pause").is_none());
        assert_eq!(
            control(&desk, "studio-seat-resume"),
            Some(&Intent::Resume { seat: "ada".into() })
        );
        let desk = rows(&PanelKind::Desk(0), Some(&view), None, &reviewing);
        assert!(control(&desk, "studio-seat-stop").is_none());

        // The merge station decides at the review's exact revisions, with
        // the review right only.
        let station = rows(&PanelKind::Review, Some(&view), Some(&review), &reviewing);
        let merge = control(&station, "studio-merge").expect("Merge");
        assert_eq!(
            *merge,
            Intent::Decide {
                task: "task-cy".into(),
                base: review.base.clone(),
                head_commit: review.head_commit.clone(),
                head: review.head.clone(),
                verdict: Verdict::Merge,
            }
        );
        assert!(control(&station, "studio-reject").is_some());
        let station = rows(&PanelKind::Review, Some(&view), Some(&review), &operate);
        assert!(control(&station, "studio-merge").is_none());
        for kind in [PanelKind::Decisions, PanelKind::Desk(0), PanelKind::Review] {
            for controls in [&operate, &reviewing] {
                assert!(project(&kind, Some(&view), Some(&review), controls, "s", 1).is_ok());
            }
        }
    }

    #[test]
    fn each_intent_is_one_valid_studio_operation() {
        let review = review();
        let now = 1_790_000_000;
        assert!(Intent::Close.operation(now).is_none());
        let answer = Intent::Answer {
            decision: "g1".into(),
            based_on: 4,
            text: "Use the first host.".into(),
        }
        .operation(now)
        .unwrap();
        let Operation::AnswerDecision {
            decision,
            based_on,
            text,
            command,
            issued_at,
        } = &answer
        else {
            panic!("an answer is studio.decision.answer: {answer:?}");
        };
        assert_eq!(
            (decision.as_str(), *based_on, text.as_str(), *issued_at),
            ("g1", 4, "Use the first host.", now)
        );
        assert_eq!(command.len(), 64);
        assert!(answer.validate().is_ok());
        assert_eq!(answer.required(), Some(Right::Operate));
        let merge = Intent::Decide {
            task: review.task.clone(),
            base: review.base.clone(),
            head_commit: review.head_commit.clone(),
            head: review.head.clone(),
            verdict: Verdict::Merge,
        }
        .operation(now)
        .unwrap();
        let Operation::DecideMerge { decision } = &merge else {
            panic!("a merge is studio.merge.decide: {merge:?}");
        };
        assert_eq!(decision.verdict, Verdict::Merge);
        assert_eq!(decision.head, review.head);
        assert!(decision.text.is_empty());
        assert!(merge.validate().is_ok());
        assert_eq!(merge.required(), Some(Right::Review));
        for (intent, name) in [
            (Intent::Pause { seat: "ada".into() }, "studio.seat.pause"),
            (Intent::Resume { seat: "ada".into() }, "studio.seat.resume"),
            (Intent::Stop { seat: "ada".into() }, "studio.seat.stop"),
        ] {
            let operation = intent.operation(now).unwrap();
            assert_eq!(operation.name(), name);
            assert_eq!(operation.required(), Some(Right::Operate));
        }
    }

    #[test]
    fn the_host_answer_leads_the_panel() {
        let refused = Answer {
            ticket: 2,
            operation: "studio.merge.decide",
            result: Err(AccessError::new(Code::Stale, "the review moved")),
        };
        let controls = Controls {
            rights: &[Right::Review],
            status: Some(&refused),
        };
        let view = studio();
        for loaded in [None, Some(&view)] {
            let rows = rows(&PanelKind::Review, loaded, None, &controls);
            assert_eq!(rows[0].key, "studio-status");
            let shown = values(&rows[..1]);
            assert!(shown.contains("`stale`"), "{shown}");
            assert!(shown.contains("nothing was merged"), "{shown}");
        }
    }
}
