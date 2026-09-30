//! A Coder run on this computer, shown in a chat as its event stream.
//!
//! When a person asks for coding in a chat on the computer they are using,
//! Coder runs right there (`coder::task::local`, the runner `openagents
//! chat` uses), and the chat shows the same [`coder_events`] the CLI
//! prints: the provider chosen and why, each step and command with its
//! bounded output, provider switches, a question or approval answered from
//! the composer, the result with the files it changed, or why it failed or
//! stopped. [`Run`] holds one chat's run: the lines it has seen, the
//! composer's mode, and the requests the adapter carries out. It starts,
//! answers, and stops nothing itself; the adapter runs each [`Request`]
//! with the shared runner and hands back an [`Answer`].
//!
//! Before a task exists the run is starting: in the chat's own project, the
//! project the person picked on this computer, the last one Coder used, or,
//! when none of those is a Git checkout, a folder the person chooses in
//! the chat. A chat bound to a task ([`LOCAL`]) follows it from its first
//! event, so a thread started in `openagents chat` shows its whole history
//! and keeps streaming while it runs.
//!
//! The rows reimplement Zeron's transcript in Rust Native (public MIT
//! zeronsh/zeron, `crates/ui/src/transcript.rs`): one tool row a command
//! with its output inside, thoughts as their own row, the reply as the
//! assistant's message, and a "Worked for" line with the turn's summary
//! ("ran 3 commands") when the turn ends. Zeron's composer question is a
//! card with one answer control, as the port audit adapts it.

use crate::coder_tab::{Choice, Mode};
use openagents_chat::coder_events::{self, CoderEvent, Line, StepKind};
use rust_native::style::{Color, Space, Style, TextWeight};
use rust_native::{Axis, Element, MessageRole, Node, TextRole, ToolState, markdown};
use std::collections::{BTreeMap, VecDeque};
use std::time::{Duration, Instant};

/// The host a thread's binding names for a run on this computer, as
/// `coder::task::local::LOCAL_HOST` spells it.
pub const LOCAL: &str = "local";
/// The most lines one chat keeps; the oldest go first.
pub const MAX_LINES: usize = 4_000;
/// The most messages a run holds for its next turn.
pub const MAX_QUEUED: usize = 8;
/// The most bytes of one message to Coder.
pub const MAX_MESSAGE: usize = 16 * 1024;

/// What the adapter does for a run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Request {
    /// Start Coder for the chat with `prompt`, in the first of `dirs` that
    /// is a Git checkout, then the project Coder last used.
    Start {
        title: String,
        prompt: String,
        dirs: Vec<String>,
    },
    /// The task's events since the last poll.
    Poll { task: String },
    /// Stop the running turn.
    Stop { task: String },
    /// Answer the question, or continue the ended task, with `text`.
    Continue { task: String, text: String },
    /// Ask the person for a project folder.
    Choose,
    /// What the finished turn changed, as a unified diff.
    Diff { task: String },
}

/// Where a task is, as the runner's follower says.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum State {
    #[default]
    Running,
    /// The last turn asked and waits for an answer.
    Waiting,
    Ended,
}

/// What the adapter answers.
#[derive(Clone, Debug, PartialEq)]
pub enum Answer {
    Started {
        task: String,
        project: String,
        checkout: String,
    },
    /// No folder given was a Git checkout; `why` says so plainly.
    NeedsProject {
        why: String,
    },
    Lines {
        lines: Vec<Line>,
        state: State,
    },
    Stopping,
    Continued,
    /// The folder the person chose, or `None` when they cancelled.
    Folder(Option<String>),
    /// The unified diff of what the task changed.
    Diff(String),
}

// Outcomes compare whole; a line's seconds are never NaN.
impl Eq for Answer {}

/// What a run's controls do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Stop,
    /// The composer's own control: answer, continue, or queue by mode.
    Send,
    Queue,
    /// Stop the running turn and continue with the message.
    Steer,
    Approve,
    Deny,
    Retry,
    ChooseFolder,
    RemoveQueued(usize),
    SendQueuedNow(usize),
}

impl Action {
    /// Whether a click on this control may end on the revision after the
    /// one it began on ([`rust_native::Press`]): the controls that must
    /// work while events stream.
    #[must_use]
    pub fn late(&self) -> bool {
        matches!(
            self,
            Self::Stop
                | Self::Approve
                | Self::Deny
                | Self::ChooseFolder
                | Self::SendQueuedNow(_)
                | Self::RemoveQueued(_)
        )
    }
}

/// What a row button does and what it does it to: two equal targets do the
/// same thing, so a click begun on one revision may end on the next.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub action: Action,
    pub task: Option<String>,
    /// The approval answered (its event's `seq`), the queued message sent
    /// or removed, or why a folder is needed.
    pub about: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Phase {
    /// Waiting for the adapter to start the task.
    Starting,
    /// No folder was a checkout; the person chooses one.
    NeedsProject(String),
    /// Following the task.
    Following,
    /// The start failed; Retry tries again.
    Failed,
}

/// One chat's Coder run.
pub struct Run {
    pub chat: String,
    pub task: Option<String>,
    pub project: Option<String>,
    /// Bumped whenever the rows change.
    pub revision: u64,
    /// Row button keys to what they do, from the last [`Run::rows`].
    pub actions: BTreeMap<String, Action>,
    phase: Phase,
    title: String,
    prompt: String,
    dirs: Vec<String>,
    lines: VecDeque<Line>,
    state: State,
    queue: VecDeque<String>,
    /// A message to send once the stop it asked for ends the turn.
    after_stop: Option<String>,
    error: Option<String>,
    notice: Option<String>,
    failed: Option<Request>,
    pending: BTreeMap<u64, Request>,
    next_ticket: u64,
    poll: Instant,
    /// A start or continue to send at the next tick.
    due: Option<Request>,
    /// The task started here and its thread should record it.
    bind: Option<(String, String)>,
    /// What the last finished turn changed, and the `seq` of its result.
    diff: Option<(u64, String)>,
    /// The result a diff was asked for.
    diff_asked: Option<u64>,
}

impl Run {
    /// A run that starts Coder for `chat`: `dirs` are the chat's own
    /// project and the picked one, in that order.
    #[must_use]
    pub fn start(chat: &str, title: &str, prompt: &str, dirs: Vec<String>, now: Instant) -> Self {
        let mut run = Self::new(chat, now);
        run.title = title.into();
        run.prompt = prompt.into();
        run.dirs = dirs;
        run.phase = Phase::Starting;
        run.due = Some(run.start_request());
        run
    }

    /// A run that follows `task`, bound to `chat`, from its first event.
    #[must_use]
    pub fn follow(chat: &str, task: &str, project: Option<String>, now: Instant) -> Self {
        let mut run = Self::new(chat, now);
        run.task = Some(task.into());
        run.project = project;
        run
    }

    fn new(chat: &str, now: Instant) -> Self {
        Self {
            chat: chat.into(),
            task: None,
            project: None,
            revision: 1,
            actions: BTreeMap::new(),
            phase: Phase::Following,
            title: String::new(),
            prompt: String::new(),
            dirs: vec![],
            lines: VecDeque::new(),
            state: State::Running,
            queue: VecDeque::new(),
            after_stop: None,
            error: None,
            notice: None,
            failed: None,
            pending: BTreeMap::new(),
            next_ticket: 1,
            poll: now,
            due: None,
            bind: None,
            diff: None,
            diff_asked: None,
        }
    }

    fn start_request(&self) -> Request {
        Request::Start {
            title: self.title.clone(),
            prompt: self.prompt.clone(),
            dirs: self.dirs.clone(),
        }
    }

    /// What the row button `key` does now, and to what.
    #[must_use]
    pub fn target(&self, key: &str) -> Option<Target> {
        let action = self.actions.get(key)?.clone();
        let about = match &action {
            Action::Approve | Action::Deny => self
                .asking_approval()
                .then(|| self.lines.back().map(|line| line.seq.to_string()))
                .flatten(),
            Action::SendQueuedNow(index) | Action::RemoveQueued(index) => {
                self.queue.get(*index).cloned()
            }
            Action::ChooseFolder => match &self.phase {
                Phase::NeedsProject(why) => Some(why.clone()),
                _ => None,
            },
            _ => None,
        };
        Some(Target {
            action,
            task: self.task.clone(),
            about,
        })
    }

    /// Every line seen, oldest first.
    pub fn lines(&self) -> impl Iterator<Item = &Line> {
        self.lines.iter()
    }

    #[must_use]
    pub fn state(&self) -> State {
        self.state
    }

    /// A start, stop, or continue is on its way.
    #[must_use]
    pub fn busy(&self) -> bool {
        self.pending.values().any(mutation)
    }

    /// Coder is starting or working.
    #[must_use]
    pub fn active(&self) -> bool {
        matches!(self.phase, Phase::Starting)
            || (self.phase == Phase::Following && self.state == State::Running)
    }

    /// The task started here and its project, for the thread to record,
    /// once.
    pub fn take_bind(&mut self) -> Option<(String, String)> {
        self.bind.take()
    }

    /// What the composer's control does now.
    #[must_use]
    pub fn mode(&self) -> Mode {
        match (&self.phase, self.state) {
            (Phase::Following, State::Waiting) => Mode::Answer,
            (Phase::Following, State::Ended) => Mode::Send,
            _ => Mode::Queue,
        }
    }

    #[must_use]
    pub fn placeholder(&self) -> &'static str {
        match self.mode() {
            Mode::Send => "Message Coder…",
            Mode::Queue => "Queue a message for Coder's next turn…",
            Mode::Answer => "Answer Coder…",
        }
    }

    /// The other way to send while Coder works: stop and send.
    #[must_use]
    pub fn steer_choice(&self) -> Option<Choice> {
        (self.phase == Phase::Following && self.state == State::Running && self.task.is_some())
            .then_some(Choice::StopAndSend)
    }

    /// The last turn's ending, if it asked for approval.
    fn asking_approval(&self) -> bool {
        self.state == State::Waiting
            && self
                .lines
                .back()
                .is_some_and(|line| matches!(line.event, CoderEvent::Approval(_)))
    }

    fn request(&mut self, request: Request) -> Option<(u64, Request)> {
        let busy = if mutation(&request) {
            self.busy()
        } else {
            self.pending.values().any(|pending| !mutation(pending))
        };
        if busy {
            return None;
        }
        let ticket = self.next_ticket;
        self.next_ticket += 1;
        self.pending.insert(ticket, request.clone());
        Some((ticket, request))
    }

    /// The request to send now, if any: a due start or continue, the
    /// queued message once a turn ends, or the next poll.
    pub fn tick(&mut self, now: Instant) -> Option<(u64, Request)> {
        if self.busy() {
            return None;
        }
        if let Some(due) = self.due.take() {
            return self.request(due);
        }
        let task = self.task.clone()?;
        if self.phase != Phase::Following {
            return None;
        }
        if self.state != State::Running && !self.pending.values().any(|p| !mutation(p)) {
            if let Some(text) = self.after_stop.take() {
                return self.request(Request::Continue { task, text });
            }
            if self.state == State::Ended
                && let Some(text) = self.queue.pop_front()
            {
                self.revision += 1;
                return self.request(Request::Continue { task, text });
            }
        }
        // A finished turn's change, once, for the "What changed" pane.
        if let Some(seq) = self.result_seq()
            && self.diff_asked != Some(seq)
            && !self.pending.values().any(|p| !mutation(p))
        {
            self.diff_asked = Some(seq);
            return self.request(Request::Diff { task });
        }
        if now < self.poll {
            return None;
        }
        self.poll = now
            + match self.state {
                State::Running => Duration::from_millis(250),
                _ => Duration::from_secs(2),
            };
        self.request(Request::Poll { task })
    }

    /// The `seq` of the last event when it is a result.
    fn result_seq(&self) -> Option<u64> {
        self.lines
            .back()
            .filter(|line| matches!(line.event, CoderEvent::Result(_)))
            .map(|line| line.seq)
    }

    /// Whether the task's last turn finished with a result.
    #[must_use]
    pub fn finished(&self) -> bool {
        self.result_seq().is_some()
    }

    /// The unified diff of what the finished turn changed, once read.
    #[must_use]
    pub fn unified_diff(&self) -> Option<&str> {
        let seq = self.result_seq()?;
        self.diff
            .as_ref()
            .filter(|(at, _)| *at == seq)
            .map(|(_, text)| text.as_str())
    }

    /// When the run next wants a tick.
    #[must_use]
    pub fn next_wake(&self, now: Instant) -> Instant {
        if self.due.is_some() {
            return now;
        }
        self.poll.max(now + Duration::from_millis(100))
    }

    pub fn pending_request(&self, ticket: u64) -> Option<&Request> {
        self.pending.get(&ticket)
    }

    /// Apply the adapter's answer to `ticket`. Returns whether a message
    /// the composer sent was accepted, so its draft can clear.
    pub fn outcome(&mut self, ticket: u64, result: Result<Answer, String>, now: Instant) -> bool {
        let Some(request) = self.pending.remove(&ticket) else {
            return false;
        };
        let answer = match result {
            Ok(answer) => answer,
            Err(error) => {
                if matches!(request, Request::Diff { .. }) {
                    // The pane stays closed; the result card still names
                    // every file.
                    return false;
                }
                if matches!(request, Request::Poll { .. }) {
                    // A read that failed is read again.
                    if self.error.as_deref() != Some(error.as_str()) {
                        self.error = Some(error);
                        self.revision += 1;
                    }
                    return false;
                }
                if matches!(request, Request::Start { .. }) {
                    self.phase = Phase::Failed;
                }
                self.error = Some(error);
                self.failed = Some(request);
                self.revision += 1;
                return false;
            }
        };
        let mut accepted = false;
        match (&request, answer) {
            (
                Request::Start { .. },
                Answer::Started {
                    task,
                    project,
                    checkout,
                },
            ) => {
                self.task = Some(task.clone());
                self.project = Some(project.clone());
                self.bind = Some((task, project));
                // The chat's own project, should it start again.
                self.dirs.retain(|known| *known != checkout);
                self.dirs.insert(0, checkout);
                self.phase = Phase::Following;
                self.state = State::Running;
                self.error = None;
                self.failed = None;
                self.poll = now;
            }
            (Request::Start { .. }, Answer::NeedsProject { why }) => {
                self.phase = Phase::NeedsProject(why);
            }
            (Request::Poll { task }, Answer::Lines { lines, state })
                if self.task.as_ref() == Some(task) =>
            {
                let last = self.lines.back().map_or(0, |line| line.seq);
                let mut changed = false;
                for line in lines {
                    // A new follower replays from the first event; keep one
                    // copy of each.
                    if line.seq <= last || line.task != *task {
                        continue;
                    }
                    self.lines.push_back(line);
                    changed = true;
                }
                while self.lines.len() > MAX_LINES {
                    self.lines.pop_front();
                }
                if state != self.state {
                    if state != State::Running {
                        // A stop that was asked for has happened.
                        self.notice = None;
                    }
                    self.state = state;
                    changed = true;
                }
                if self.error.take().is_some() {
                    changed = true;
                }
                if changed {
                    self.revision += 1;
                    self.poll = now;
                }
                return false;
            }
            (Request::Stop { .. }, Answer::Stopping) => {
                self.notice = Some("Asked Coder to stop. Its turn ends as stopped.".into());
                self.poll = now;
            }
            (Request::Continue { .. }, Answer::Continued) => {
                self.state = State::Running;
                self.poll = now;
                accepted = true;
            }
            (Request::Choose, Answer::Folder(Some(dir))) => {
                self.dirs.retain(|known| *known != dir);
                self.dirs.insert(0, dir);
                self.phase = Phase::Starting;
                self.error = None;
                self.due = Some(self.start_request());
            }
            (Request::Choose, Answer::Folder(None)) => {}
            (Request::Diff { .. }, Answer::Diff(text)) => {
                if let Some(seq) = self.result_seq() {
                    self.diff = Some((seq, text));
                }
            }
            _ => {
                self.error = Some("Coder answered another request.".into());
            }
        }
        if mutation(&request) || matches!(request, Request::Choose) {
            if self.failed.as_ref() == Some(&request) {
                self.failed = None;
            }
            if accepted || !matches!(request, Request::Start { .. }) {
                self.error = None;
            }
        }
        self.revision += 1;
        accepted
    }

    /// Carry out `action` with the composer's `text`.
    pub fn action(&mut self, action: Action, text: &str) -> Option<(u64, Request)> {
        let text = text.trim();
        match action {
            Action::Retry => {
                let failed = self.failed.take()?;
                self.error = None;
                self.revision += 1;
                if matches!(failed, Request::Start { .. }) {
                    self.phase = Phase::Starting;
                    return self.request(self.start_request());
                }
                self.request(failed)
            }
            Action::ChooseFolder => self.request(Request::Choose),
            Action::Stop => {
                let task = self.task.clone()?;
                (self.state == State::Running).then_some(())?;
                self.request(Request::Stop { task })
            }
            Action::Approve | Action::Deny => {
                if !self.asking_approval() {
                    return None;
                }
                let task = self.task.clone()?;
                let text = if action == Action::Approve {
                    "Approved."
                } else {
                    "Denied."
                };
                self.request(Request::Continue {
                    task,
                    text: text.into(),
                })
            }
            Action::Send | Action::Queue | Action::Steer => {
                if text.is_empty() || text.len() > MAX_MESSAGE {
                    return None;
                }
                let mode = if action == Action::Send {
                    self.mode()
                } else {
                    Mode::Queue
                };
                if action == Action::Steer {
                    let task = self.task.clone()?;
                    self.steer_choice()?;
                    self.after_stop = Some(text.into());
                    self.notice = Some("Coder stops, then continues with your message.".into());
                    self.revision += 1;
                    return self.request(Request::Stop { task });
                }
                match mode {
                    Mode::Queue => {
                        if self.queue.len() >= MAX_QUEUED {
                            self.notice = Some(format!(
                                "Coder holds {MAX_QUEUED} messages for its next turn at most."
                            ));
                        } else {
                            self.queue.push_back(text.into());
                            self.notice = None;
                        }
                        self.revision += 1;
                        None
                    }
                    Mode::Answer | Mode::Send => {
                        let task = self.task.clone()?;
                        self.request(Request::Continue {
                            task,
                            text: text.into(),
                        })
                    }
                }
            }
            Action::RemoveQueued(index) => {
                self.queue.remove(index)?;
                self.revision += 1;
                None
            }
            Action::SendQueuedNow(index) => {
                let text = self.queue.remove(index)?;
                self.revision += 1;
                if self.state == State::Running {
                    self.action(Action::Steer, &text)
                } else {
                    let task = self.task.clone()?;
                    self.request(Request::Continue { task, text })
                }
            }
        }
    }

    /// Whether the composer's text was queued rather than sent: the draft
    /// clears at once.
    #[must_use]
    pub fn queued(&self) -> usize {
        self.queue.len()
    }

    /// The run as transcript rows.
    pub fn rows(&mut self) -> Vec<Node<()>> {
        self.actions.clear();
        let mut out = Rows::default();
        for line in &self.lines {
            out.line(line);
        }
        let last_turn = self.lines.back().map(turn_of);
        out.close(self.state == State::Running && self.phase == Phase::Following);
        let mut rows = out.rows;
        if self.state == State::Running && self.phase == Phase::Following && self.task.is_some() {
            rows.push(Node {
                key: "coder-working".into(),
                style: Style::default(),
                element: Element::Working {
                    label: out.progress.unwrap_or_else(|| {
                        if last_turn.is_some() {
                            "Coder is working…".into()
                        } else {
                            "Starting Coder…".into()
                        }
                    }),
                },
            });
        }
        match &self.phase {
            Phase::Starting => rows.push(Node {
                key: "coder-starting".into(),
                style: Style::default(),
                element: Element::Working {
                    label: "Starting Coder on this computer…".into(),
                },
            }),
            Phase::NeedsProject(why) => {
                let why = why.clone();
                rows.push(self.card(
                    "coder-project",
                    "Coder needs a project",
                    &[(why, false)],
                    &[("coder-choose", "Choose folder…", Action::ChooseFolder)],
                ));
            }
            Phase::Following | Phase::Failed => {}
        }
        if self.asking_approval() {
            rows.push(self.buttons(
                "coder-approval",
                &[
                    ("coder-approve", "Approve", Action::Approve),
                    ("coder-deny", "Deny", Action::Deny),
                ],
            ));
        }
        if self.active() && self.task.is_some() {
            rows.push(self.buttons(
                "coder-stop-row",
                &[("coder-stop", "Stop Coder", Action::Stop)],
            ));
        }
        let queued: Vec<String> = self.queue.iter().cloned().collect();
        for (index, text) in queued.iter().enumerate() {
            rows.push(status(
                &format!("coder-queued-{index}"),
                &format!("Queued for the next turn: {}", clip(text, 200)),
            ));
            rows.push(self.buttons(
                &format!("coder-queued-{index}-controls"),
                &[
                    (
                        &format!("coder-queued-{index}-now"),
                        "Send now",
                        Action::SendQueuedNow(index),
                    ),
                    (
                        &format!("coder-queued-{index}-remove"),
                        "Remove",
                        Action::RemoveQueued(index),
                    ),
                ],
            ));
        }
        if let Some(notice) = &self.notice {
            rows.push(status("coder-notice", notice));
        }
        if let Some(error) = self.error.clone() {
            rows.push(status("coder-error", &error));
            if self.failed.is_some() && !matches!(self.phase, Phase::NeedsProject(_)) {
                rows.push(self.buttons(
                    "coder-retry-row",
                    &[("coder-retry", "Retry", Action::Retry)],
                ));
            }
        }
        rows
    }

    fn buttons(&mut self, key: &str, buttons: &[(&str, &str, Action)]) -> Node<()> {
        let enabled = !self.busy();
        let children = buttons
            .iter()
            .map(|(key, label, action)| {
                self.actions.insert((*key).into(), action.clone());
                button(key, label, enabled)
            })
            .collect();
        Node {
            key: key.into(),
            style: Style {
                gap: Some(Space::Sm),
                ..Style::default()
            },
            element: Element::Stack {
                axis: Axis::Horizontal,
                children,
            },
        }
    }

    fn card(
        &mut self,
        key: &str,
        title: &str,
        lines: &[(String, bool)],
        buttons: &[(&str, &str, Action)],
    ) -> Node<()> {
        let mut node = card(key, title, lines);
        if !buttons.is_empty()
            && let Element::Stack { children, .. } = &mut node.element
        {
            children.push(self.buttons(&format!("{key}-buttons"), buttons));
        }
        node
    }
}

fn mutation(request: &Request) -> bool {
    !matches!(
        request,
        Request::Poll { .. } | Request::Choose | Request::Diff { .. }
    )
}

/// The turn an event belongs to.
#[must_use]
pub fn turn_of(line: &Line) -> usize {
    match &line.event {
        CoderEvent::CoderStarted(e) => e.turn,
        CoderEvent::Step(e) => e.turn,
        CoderEvent::Output(e) => e.turn,
        CoderEvent::ProviderSwitched(e) => e.turn,
        CoderEvent::Question(e) | CoderEvent::Approval(e) => e.turn,
        CoderEvent::Progress(e) => e.turn,
        CoderEvent::Result(e) => e.turn,
        CoderEvent::Failure(e) => e.turn,
        CoderEvent::Stopped(e) => e.turn,
    }
}

/// A tool row being built: a command, a tool call, or a thought.
struct Tool {
    key: String,
    name: String,
    detail: String,
    body: Vec<String>,
    state: ToolState,
    /// A command waiting for its output.
    command: Option<String>,
}

/// The rows of a run, built one line at a time.
#[derive(Default)]
struct Rows {
    rows: Vec<Node<()>>,
    /// The reply being written: its key and text.
    reply: Option<(String, String)>,
    tools: Vec<Tool>,
    /// Where each open tool row goes in `rows`.
    slots: Vec<usize>,
    /// The latest progress while the turn runs.
    progress: Option<String>,
    /// This turn's commands, thoughts, and tool calls, for its summary.
    commands: usize,
    thoughts: usize,
    calls: usize,
    failed: usize,
    seconds: f64,
    /// The turn the last line belonged to.
    turn: usize,
    /// Messages and replies already shown: a later turn's trajectory
    /// starts with the conversation so far, shown once.
    shown: std::collections::BTreeSet<(bool, String)>,
    /// The reply row just drawn, which an ending that repeats it replaces.
    last_reply: Option<(usize, String)>,
}

impl Rows {
    fn line(&mut self, line: &Line) {
        let key = format!("coder-{}", line.seq);
        self.turn = turn_of(line);
        match &line.event {
            CoderEvent::Step(step) if step.kind == StepKind::Reply => {
                match &mut self.reply {
                    Some((_, text)) => text.push_str(&step.text),
                    None => self.reply = Some((key, step.text.clone())),
                }
                return;
            }
            _ => self.flush_reply(),
        }
        match &line.event {
            CoderEvent::CoderStarted(started) => {
                self.close_turn();
                let provider = coder_events::provider_name(&serde_json::json!(started.provider));
                let mut lines = vec![(started.reason.clone(), false)];
                lines.push((format!("{} · {}", started.project, started.worktree), true));
                if !started.fallbacks.is_empty() {
                    lines.push((
                        format!(
                            "Falls back to {}",
                            started
                                .fallbacks
                                .iter()
                                .map(|route| route_name(route))
                                .collect::<Vec<_>>()
                                .join(", then ")
                        ),
                        true,
                    ));
                }
                let title = if started.turn == 1 {
                    format!("Coder started on {provider} · {}", started.model)
                } else {
                    format!(
                        "Coder continued on {provider} · {} (turn {})",
                        started.model, started.turn
                    )
                };
                self.rows.push(card(&key, &title, &lines));
            }
            CoderEvent::Step(step) => match step.kind {
                StepKind::Message => {
                    if self.shown.insert((true, step.text.trim().to_owned())) || self.turn == 1 {
                        let text = crate::basic_chats::handoff_summary(&step.text)
                            .unwrap_or_else(|| step.text.clone());
                        self.rows.push(message(&key, MessageRole::User, &text));
                    }
                }
                StepKind::Thinking => {
                    self.thoughts += 1;
                    self.open(Tool {
                        key,
                        name: "Thinking".into(),
                        detail: first_line(&step.text),
                        body: vec![step.text.clone()],
                        state: ToolState::Done,
                        command: None,
                    });
                }
                StepKind::Command => {
                    self.commands += 1;
                    self.open(Tool {
                        key,
                        name: "Command".into(),
                        detail: first_line(&step.text),
                        body: vec![format!("$ {}", step.text)],
                        state: ToolState::Running,
                        command: Some(step.text.clone()),
                    });
                }
                StepKind::ToolCall => {
                    self.calls += 1;
                    let (name, rest) = step.text.split_once(' ').unwrap_or((&step.text, ""));
                    self.open(Tool {
                        key,
                        name: name.into(),
                        detail: first_line(rest),
                        body: vec![step.text.clone()],
                        state: ToolState::Done,
                        command: None,
                    });
                }
                StepKind::Observation => {
                    // A command's exit and time, or what an agent's tool
                    // returned: under the row before it.
                    match self.tools.last_mut() {
                        Some(tool) if tool.command.is_some() => {
                            tool.detail = format!("{} · {}", clip(&tool.detail, 80), step.text);
                        }
                        Some(tool) => tool.body.push(step.text.clone()),
                        None => self.rows.push(status(&key, &step.text)),
                    }
                }
                StepKind::Note => self.rows.push(status(&key, &step.text)),
                StepKind::Reply => {}
            },
            CoderEvent::Output(output) => {
                let found = self
                    .tools
                    .iter_mut()
                    .rev()
                    .find(|tool| tool.command.as_deref() == Some(output.command.as_str()));
                let failed = output.timed_out || output.exit != Some(0);
                if failed {
                    self.failed += 1;
                }
                let mut text = output.text.trim_end().to_owned();
                if output.truncated {
                    text.push_str("\n… (cut; the whole output is in the task's trajectory)");
                }
                match found {
                    Some(tool) => {
                        tool.command = None;
                        tool.state = if failed {
                            ToolState::Failed
                        } else {
                            ToolState::Done
                        };
                        if !text.is_empty() {
                            tool.body.push(text);
                        }
                    }
                    None => self.open(Tool {
                        key,
                        name: "Output".into(),
                        detail: first_line(&output.command),
                        body: vec![text],
                        state: if failed {
                            ToolState::Failed
                        } else {
                            ToolState::Done
                        },
                        command: None,
                    }),
                }
            }
            CoderEvent::ProviderSwitched(switch) => {
                let text = match &switch.to {
                    Some(to) => format!(
                        "Switched from {} to {}: {}.",
                        route_name(&switch.from),
                        route_name(to),
                        switch.reason
                    ),
                    None => format!("{}; no other provider has capacity.", switch.reason),
                };
                self.rows.push(notice(&key, &text));
            }
            CoderEvent::Progress(progress) => {
                self.seconds = progress.seconds;
                self.progress = Some(format!(
                    "Coder is working · step {}{}{} · {:.0}s",
                    progress.step,
                    progress
                        .max_steps
                        .map(|max| format!(" of {max}"))
                        .unwrap_or_default(),
                    progress
                        .done
                        .map(|done| format!(" · {:.0}% done", done * 100.0))
                        .unwrap_or_default(),
                    progress.seconds
                ));
            }
            CoderEvent::Question(asked) => {
                self.flush_reply();
                self.drop_repeated_reply(&asked.text);
                self.close_turn();
                self.rows
                    .push(asked_card(&key, "Coder asks", &asked.text, "Answer below."));
            }
            CoderEvent::Approval(asked) => {
                self.flush_reply();
                self.drop_repeated_reply(&asked.text);
                self.close_turn();
                self.rows.push(asked_card(
                    &key,
                    "Coder asks to go ahead",
                    &asked.text,
                    "Approve or deny below, or answer in your own words.",
                ));
            }
            CoderEvent::Result(result) => {
                self.flush_reply();
                self.drop_repeated_reply(&result.summary);
                self.close_turn();
                let mut lines = vec![];
                let files = result.files_changed.len();
                lines.push((
                    format!(
                        "{} file{} changed · +{} −{}",
                        files,
                        if files == 1 { "" } else { "s" },
                        result.insertions,
                        result.deletions
                    ),
                    false,
                ));
                for file in &result.files_changed {
                    let lines_changed = match (file.added, file.removed) {
                        (Some(added), Some(removed)) => format!("+{added} −{removed}"),
                        _ => "binary".into(),
                    };
                    lines.push((
                        format!("{} {} {lines_changed}", file.status, file.path),
                        true,
                    ));
                }
                lines.push((format!("In {}", result.worktree), true));
                let mut node = card(&key, "Coder finished", &[]);
                if let Element::Stack { children, .. } = &mut node.element {
                    if !result.summary.trim().is_empty() {
                        children.push(Node {
                            key: format!("{key}-summary"),
                            style: Style::default(),
                            element: Element::Markdown {
                                blocks: markdown::parse(&result.summary),
                            },
                        });
                    }
                    children.extend(card_lines(&key, &lines));
                }
                self.rows.push(node);
            }
            CoderEvent::Failure(failure) => {
                self.close_turn();
                self.rows.push(card(
                    &key,
                    "Coder didn't finish",
                    &[(failure.message.clone(), false)],
                ));
            }
            CoderEvent::Stopped(stopped) => {
                self.close_turn();
                self.rows.push(notice(&key, &stopped.message));
            }
        }
    }

    fn open(&mut self, tool: Tool) {
        self.slots.push(self.rows.len());
        self.rows.push(Node {
            key: tool.key.clone(),
            style: Style::default(),
            element: Element::Text {
                value: String::new(),
                role: TextRole::Status,
            },
        });
        self.tools.push(tool);
    }

    fn flush_reply(&mut self) {
        if let Some((key, text)) = self.reply.take()
            && !text.trim().is_empty()
            && (self.shown.insert((false, text.trim().to_owned())) || self.turn == 1)
        {
            self.last_reply = Some((self.rows.len(), text.trim().to_owned()));
            self.rows.push(message(&key, MessageRole::Assistant, &text));
        }
    }

    /// An ending that says what the reply just said shows it once, in
    /// its card.
    fn drop_repeated_reply(&mut self, said: &str) {
        if let Some((at, text)) = self.last_reply.take()
            && at + 1 == self.rows.len()
            && text == said.trim()
        {
            self.rows.pop();
        }
    }

    /// Draw the open tool rows in their places.
    fn draw_tools(&mut self) {
        for (tool, slot) in self.tools.drain(..).zip(self.slots.drain(..)) {
            self.rows[slot] = tool_row(tool);
        }
    }

    /// A turn ended: its "Worked for" line.
    fn close_turn(&mut self) {
        self.flush_reply();
        self.draw_tools();
        let mut parts = vec![];
        if self.commands > 0 {
            parts.push(plural(self.commands, "ran a command", "ran {} commands"));
        }
        if self.calls > 0 {
            parts.push(plural(self.calls, "made a call", "made {} calls"));
        }
        if self.thoughts > 0 {
            parts.push(plural(self.thoughts, "thought once", "thought {} times"));
        }
        if self.failed > 0 {
            parts.push(plural(self.failed, "1 failed", "{} failed"));
        }
        if !parts.is_empty() || self.seconds > 0.0 {
            let key = format!("coder-worked-{}", self.rows.len());
            let worked = if self.seconds > 0.0 {
                format!("Worked for {}", duration(self.seconds))
            } else {
                "Worked".into()
            };
            let text = if parts.is_empty() {
                worked
            } else {
                format!("{worked} · {}", parts.join(" · "))
            };
            self.rows.push(status(&key, &text));
        }
        self.commands = 0;
        self.calls = 0;
        self.thoughts = 0;
        self.failed = 0;
        self.seconds = 0.0;
        self.progress = None;
    }

    /// The last line: a running turn keeps its rows open.
    fn close(&mut self, running: bool) {
        self.flush_reply();
        self.draw_tools();
        if !running {
            self.progress = None;
        }
    }
}

fn tool_row(tool: Tool) -> Node<()> {
    let key = tool.key.clone();
    Node {
        key: tool.key,
        style: Style::default(),
        element: Element::Tool {
            name: tool.name,
            detail: tool.detail,
            state: tool.state,
            children: vec![Node {
                key: format!("{key}-body"),
                style: Style {
                    foreground: Some(GRAY),
                    ..Style::default()
                },
                element: Element::Text {
                    value: tool.body.join("\n"),
                    role: TextRole::Code,
                },
            }],
        },
    }
}

const GRAY: Color = Color::rgb(153, 153, 153);
const CARD: Color = Color::rgb(26, 29, 34);

fn message(key: &str, role: MessageRole, text: &str) -> Node<()> {
    Node {
        key: key.into(),
        style: Style::default(),
        element: Element::Message {
            role,
            note: None,
            children: vec![Node {
                key: format!("{key}-md"),
                style: Style::default(),
                element: Element::Markdown {
                    blocks: markdown::parse(text),
                },
            }],
        },
    }
}

fn status(key: &str, text: &str) -> Node<()> {
    Node {
        key: key.into(),
        style: Style::default(),
        element: Element::Text {
            value: text.into(),
            role: TextRole::Status,
        },
    }
}

fn notice(key: &str, text: &str) -> Node<()> {
    Node {
        key: key.into(),
        style: Style {
            weight: Some(TextWeight::Bold),
            ..Style::default()
        },
        element: Element::Text {
            value: text.into(),
            role: TextRole::Status,
        },
    }
}

fn button(key: &str, label: &str, enabled: bool) -> Node<()> {
    Node {
        key: key.into(),
        style: Style::default(),
        element: Element::Button {
            label: label.into(),
            shortcut: None,
            enabled,
            icon: None,
            intent: (),
        },
    }
}

fn card_lines(key: &str, lines: &[(String, bool)]) -> Vec<Node<()>> {
    lines
        .iter()
        .enumerate()
        .map(|(index, (text, quiet))| Node {
            key: format!("{key}-line-{index}"),
            style: Style::default(),
            element: Element::Text {
                value: text.clone(),
                role: if *quiet {
                    TextRole::Status
                } else {
                    TextRole::Body
                },
            },
        })
        .collect()
}

fn card(key: &str, title: &str, lines: &[(String, bool)]) -> Node<()> {
    let mut children = vec![Node {
        key: format!("{key}-title"),
        style: Style {
            weight: Some(TextWeight::Bold),
            ..Style::default()
        },
        element: Element::Text {
            value: title.into(),
            role: TextRole::Body,
        },
    }];
    children.extend(card_lines(key, lines));
    Node {
        key: key.into(),
        style: Style {
            background: Some(CARD),
            padding_top: Some(Space::Md),
            padding_bottom: Some(Space::Md),
            padding_start: Some(Space::Md),
            padding_end: Some(Space::Md),
            gap: Some(Space::Xs),
            ..Style::default()
        },
        element: Element::Stack {
            axis: Axis::Vertical,
            children,
        },
    }
}

fn asked_card(key: &str, title: &str, text: &str, hint: &str) -> Node<()> {
    let mut node = card(key, title, &[]);
    if let Element::Stack { children, .. } = &mut node.element {
        children.push(Node {
            key: format!("{key}-text"),
            style: Style::default(),
            element: Element::Markdown {
                blocks: markdown::parse(text.trim()),
            },
        });
        children.push(status(&format!("{key}-hint"), hint));
    }
    node
}

/// `codex:gpt-6-luna` as "Codex (gpt-6-luna)".
fn route_name(route: &str) -> String {
    match route.split_once(':') {
        Some((provider, model)) => format!(
            "{} ({model})",
            coder_events::provider_name(&serde_json::json!(provider))
        ),
        None => coder_events::provider_name(&serde_json::json!(route)),
    }
}

fn first_line(text: &str) -> String {
    clip(
        text.lines()
            .find(|line| !line.trim().is_empty())
            .unwrap_or(""),
        100,
    )
}

fn clip(text: &str, max: usize) -> String {
    let text = text.trim();
    if text.chars().count() <= max {
        return text.into();
    }
    let mut out: String = text.chars().take(max).collect();
    out.push('…');
    out
}

fn plural(count: usize, one: &str, many: &str) -> String {
    if count == 1 {
        one.into()
    } else {
        many.replace("{}", &count.to_string())
    }
}

fn duration(seconds: f64) -> String {
    let seconds = seconds.round() as u64;
    if seconds < 60 {
        format!("{seconds}s")
    } else {
        format!("{}m {:02}s", seconds / 60, seconds % 60)
    }
}

#[cfg(test)]
mod tests;
