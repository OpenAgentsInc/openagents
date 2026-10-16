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
//! ("ran 3 commands") when the turn ends. The question or approval the run
//! waits on is the paged decision panel ([`crate::decision`]).

pub mod children;
pub mod configuration;
pub mod input;
mod native;
pub mod resources;
pub mod workspace;

use crate::attention::Activity;
use crate::coder_tab::{Choice, Mode};
use openagents_chat::coder_events::{self, CoderEvent, Line, StepKind};
use openagents_chat::tool_groups::{self, Entry, Item, Shown, Stretch};
use rust_native::style::{Color, Space, Style, TextWeight};
use rust_native::{Axis, Element, MessageRole, Node, TextRole, ToolState, markdown};
use std::collections::{BTreeMap, VecDeque};
use std::time::{Duration, Instant};

/// The host a thread's binding names for a run on this computer, as
/// `coder::task::local::LOCAL_HOST` spells it.
pub const LOCAL: &str = "local";
/// The most lines one chat keeps; the oldest go first.
pub const MAX_LINES: usize = 4_000;
/// The empty composer's words, the web's exactly (`openagents-web` `pages/chat.rs`).
pub const PLACEHOLDER: &str = "Ask OpenAgents anything";
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
        /// The draft's images, as their exact bytes; the adapter keeps them
        /// with the task ([`crate::attachments::Drafts::uploads`]).
        images: Vec<crate::attachments::Upload>,
        /// The coding engine the person asked for, from the reply's typed
        /// offer (#10076): the start puts it first and says why when
        /// another runs.
        engine: Option<nostr::cj_conversation::Engine>,
        configuration: Option<route_contract::session::control::Request>,
    },
    /// Read owner-issued coding choices without starting a task.
    Options {
        task: Option<String>,
        dirs: Vec<String>,
    },
    ContinueInput {
        task: String,
        message: input::Message,
        configuration: Option<route_contract::session::control::Request>,
    },
    ContinueConfigured {
        task: String,
        text: String,
        configuration: route_contract::session::control::Request,
    },
    ResourceRead {
        task: String,
        generation: u64,
        reference: route_contract::resources::Reference,
    },
    ChildRead {
        generation: u64,
        task: String,
        read: route_contract::children::Read,
    },
    WorkspaceOpen {
        task: String,
    },
    WorkspaceRead {
        task: String,
        request: route_contract::workspace::Request,
    },
    /// The task's events since the last poll.
    Poll {
        task: String,
    },
    /// Stop the running turn.
    Stop {
        task: String,
    },
    /// Answer the question, or continue the ended task, with `text`.
    Continue {
        task: String,
        text: String,
    },
    /// Reply to one live SDK callback without starting another turn.
    Respond {
        task: String,
        reply: route_contract::pending::Reply,
    },
    /// Ask the person for a project folder.
    Choose,
    /// What the finished turn changed, at exact revisions
    /// ([`crate::changes::Need::Read`]).
    Review {
        task: String,
    },
    /// Publish the reviewed change once ([`crate::changes::Need::Publish`]).
    Publish {
        task: String,
        base: String,
        head_commit: String,
        head: String,
    },
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
    Options(route_contract::session::control::Offer),
    Resource(resources::Content),
    ChildPage(route_contract::children::Page),
    WorkspaceReference(route_contract::workspace::Reference),
    WorkspaceRead(route_contract::workspace::Output),
    Lines {
        children: Option<route_contract::children::Snapshot>,
        configuration: Option<route_contract::session::control::Offer>,
        native: Option<route_contract::pending::Snapshot>,
        lines: Vec<Line>,
        state: State,
    },
    Stopping,
    Responded(route_contract::pending::Receipt),
    Continued,
    InputAccepted {
        input: route_contract::resources::Input,
        receipt: route_contract::resources::Receipt,
    },
    /// The folder the person chose, or `None` when they cancelled.
    Folder(Option<String>),
    /// What the task changed, at exact revisions.
    Review(Box<coder_host::access::review::TaskReview>),
    /// The publication of the reviewed change, including a refused or
    /// uncertain one.
    Published(Box<coder_host::access::review::Publication>),
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
    /// A decision panel control ([`crate::decision`]).
    Decide(crate::decision::Control),
    NativeDecision {
        request: route_contract::Digest,
        control: crate::decision::Control,
    },
    NativeCancel(route_contract::Digest),
    Configure(configuration::Target),
    Workspace(workspace::Target),
    Child(children::Target),
    Resource(resources::Target),
    Retry,
    EditFailedInput,
    ChooseFolder,
    RemoveQueued(usize),
    SendQueuedNow(usize),
    /// A control of the task's plan panel ([`crate::plan_panel`]).
    Plan(crate::plan_panel::Control),
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
                | Self::Decide(_)
                | Self::ChooseFolder
                | Self::SendQueuedNow(_)
                | Self::RemoveQueued(_)
                | Self::Plan(_)
                | Self::Resource(_)
        )
    }
}

/// What a row button does and what it does it to: two equal targets do the
/// same thing, so a click begun on one revision may end on the next.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub action: Action,
    pub task: Option<String>,
    /// The question or approval answered (its event's `seq`), the queued message sent
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
    /// The images the start carries until a task holds them.
    images: Vec<crate::attachments::Upload>,
    /// The engine the person asked for, which the start carries (#10076).
    engine: Option<nostr::cj_conversation::Engine>,
    /// The start that carried images was accepted; the adapter drops the
    /// draft's images once ([`Run::take_delivered`]).
    delivered: bool,
    lines: VecDeque<Line>,
    session: Option<route_contract::session::Descriptor>,
    observations: Option<openagents_chat::observations::Projection>,
    observation_rows: BTreeMap<String, u64>,
    cursor: u64,
    history_incomplete: bool,
    state: State,
    queue: VecDeque<input::Message>,
    /// A message to send once the stop it asked for ends the turn.
    after_stop: Option<input::Message>,
    restored_input: Option<input::Message>,
    image_ids: Vec<String>,
    delivered_ids: Vec<String>,
    error: Option<String>,
    notice: Option<String>,
    failed: Option<Request>,
    pending: BTreeMap<u64, Request>,
    next_ticket: u64,
    poll: Instant,
    /// When the task last sent an event or changed state, for the stale
    /// rule ([`crate::attention::STALE_AFTER`]).
    heard: Instant,
    /// A start or continue to send at the next tick.
    due: Option<Request>,
    /// The task started here and its thread should record it.
    bind: Option<(String, String)>,
    /// What the last finished turn changed, and its publication.
    reviewer: crate::changes::Reviewer,
    /// The result the reviewer follows.
    reviewed_seq: Option<u64>,
    /// The plan panel's state for this run's task (#10471).
    plan: crate::plan_panel::Panel,
    /// The question or approval the run waits on: the asking event's
    /// `seq`, and the decision panel's state.
    decision: Option<(u64, crate::decision::Flow)>,
    native: native::Pending,
    configuration: configuration::Choice,
    start_configuration: Option<route_contract::session::control::Request>,
    consumed_input: bool,
    configuration_due: bool,
    resources: resources::Inspector,
    resource_due: Option<route_contract::resources::Reference>,
    children: children::Inspector,
    child_due: Option<route_contract::children::Read>,
    workspace: workspace::Inspector,
    workspace_file: Option<crate::workspace_tabs::Open>,
    workspace_due: Option<workspace::Need>,
}

impl Run {
    /// A run that starts Coder for `chat`: `dirs` are the chat's own
    /// project and the picked one, in that order.
    #[must_use]
    pub fn start(chat: &str, title: &str, prompt: &str, dirs: Vec<String>, now: Instant) -> Self {
        Self::start_with_images(chat, title, prompt, dirs, Vec::new(), now)
    }

    /// [`Run::start`] carrying the draft's `images` to the task.
    #[must_use]
    pub fn start_with_images(
        chat: &str,
        title: &str,
        prompt: &str,
        dirs: Vec<String>,
        images: Vec<crate::attachments::Upload>,
        now: Instant,
    ) -> Self {
        let mut run = Self::new(chat, now);
        run.title = title.into();
        run.prompt = prompt.into();
        run.dirs = dirs;
        run.images = images;
        run.phase = Phase::Starting;
        run.due = Some(run.start_request());
        run
    }

    /// A run that follows `task`, bound to `chat`, from its first event.
    #[must_use]
    pub fn follow(chat: &str, task: &str, project: Option<String>, now: Instant) -> Self {
        let mut run = Self::new(chat, now);
        run.task = Some(task.into());
        run.children = children::Inspector::new(task);
        run.resources = resources::Inspector::new(task);
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
            images: vec![],
            engine: None,
            delivered: false,
            lines: VecDeque::new(),
            session: None,
            observations: None,
            observation_rows: BTreeMap::new(),
            cursor: 0,
            history_incomplete: false,
            state: State::Running,
            queue: VecDeque::new(),
            after_stop: None,
            restored_input: None,
            image_ids: vec![],
            delivered_ids: vec![],
            error: None,
            notice: None,
            failed: None,
            pending: BTreeMap::new(),
            next_ticket: 1,
            poll: now,
            heard: now,
            due: None,
            bind: None,
            reviewer: crate::changes::Reviewer::new(),
            reviewed_seq: None,
            plan: crate::plan_panel::Panel::default(),
            decision: None,
            native: native::Pending::default(),
            configuration: configuration::Choice::default(),
            start_configuration: None,
            consumed_input: false,
            configuration_due: false,
            resources: resources::Inspector::default(),
            resource_due: None,
            children: children::Inspector::default(),
            child_due: None,
            workspace: workspace::Inspector::default(),
            workspace_file: None,
            workspace_due: None,
        }
    }

    fn start_request(&self) -> Request {
        Request::Start {
            title: self.title.clone(),
            prompt: self.prompt.clone(),
            dirs: self.dirs.clone(),
            images: self.images.clone(),
            engine: self.engine,
            configuration: self.start_configuration.clone(),
        }
    }

    /// This start, for a person who asked for `engine` (#10076).
    #[must_use]
    pub fn requesting(mut self, engine: Option<nostr::cj_conversation::Engine>) -> Self {
        self.engine = engine;
        if matches!(self.due, Some(Request::Start { .. })) {
            self.due = Some(self.start_request());
        }
        self
    }

    /// Use an owner-issued choice for this new task.
    pub fn configured(
        mut self,
        offer: route_contract::session::control::Offer,
        request: route_contract::session::control::Request,
    ) -> Result<Self, String> {
        if !offer.accepts(&request) {
            return Err("This coding choice changed. Choose it again.".into());
        }
        self.configuration.adopt(offer, Some(&request));
        self.start_configuration = Some(request);
        self.engine = None;
        if matches!(self.due, Some(Request::Start { .. })) {
            self.due = Some(self.start_request());
        }
        Ok(self)
    }

    pub fn refresh_configuration(&mut self) -> Option<(u64, Request)> {
        self.configuration.open();
        self.configuration_due = true;
        self.revision += 1;
        let request = Request::Options {
            task: self.task.clone(),
            dirs: self.dirs.clone(),
        };
        let requested = self.request(request);
        if requested.is_some() {
            self.configuration_due = false;
        }
        requested
    }

    pub fn configuration_view<I>(
        &self,
        intent: impl Fn(configuration::Target) -> I,
    ) -> Option<configuration::View<I>> {
        self.configuration.view(
            "coder-session",
            !self.busy(),
            self.state == State::Ended,
            intent,
        )
    }

    pub fn children_view<I>(&self, intent: impl Fn(children::Target) -> I) -> Option<Node<I>> {
        self.children.view(intent)
    }

    pub fn take_workspace_file(&mut self) -> Option<crate::workspace_tabs::Open> {
        self.workspace_file.take()
    }
    pub fn workspace(&self) -> &workspace::Inspector {
        &self.workspace
    }
    pub fn workspace_query(&mut self, query: &str) {
        self.workspace.set_query(query);
        self.revision += 1;
    }
    pub fn workspace_view<I>(
        &self,
        search: Option<Node<I>>,
        intent: impl Fn(workspace::Target) -> I,
    ) -> Option<Node<I>> {
        self.task.as_ref()?;
        Some(self.workspace.view(
            "workspace",
            !self.busy()
                && !self.pending.values().any(|p| {
                    matches!(
                        p,
                        Request::WorkspaceOpen { .. } | Request::WorkspaceRead { .. }
                    )
                }),
            search,
            intent,
        ))
    }
    fn workspace_request(&self, need: &workspace::Need) -> Option<Request> {
        let task = self.task.clone()?;
        Some(match need {
            workspace::Need::File(_) => return None,
            workspace::Need::Reference => Request::WorkspaceOpen { task },
            workspace::Need::Read(request) => Request::WorkspaceRead {
                task,
                request: request.clone(),
            },
        })
    }
    fn continuation(&self, text: String) -> Result<Request, String> {
        let task = self
            .task
            .clone()
            .ok_or("This coding task is unavailable.")?;
        match self.configuration.request()? {
            Some(configuration) => Ok(Request::ContinueConfigured {
                task,
                text,
                configuration,
            }),
            None => Ok(Request::Continue { task, text }),
        }
    }

    pub fn take_consumed_input(&mut self) -> bool {
        std::mem::take(&mut self.consumed_input)
    }

    fn dispatch_continuation(&mut self, text: String) -> Option<(u64, Request)> {
        self.dispatch_message(text.into())
    }
    fn dispatch_message(&mut self, message: input::Message) -> Option<(u64, Request)> {
        let request = if message.images.is_empty() {
            self.continuation(message.text)
        } else if !self.accepts_images() {
            Err("Choose Claude Code with live approvals to send these images.".into())
        } else {
            self.configuration
                .request()
                .map(|configuration| Request::ContinueInput {
                    task: self.task.clone().unwrap_or_default(),
                    message,
                    configuration,
                })
        };
        match request {
            Ok(request) => self.request(request),
            Err(error) => {
                if self.notice.as_ref() != Some(&error) {
                    self.notice = Some(error);
                    self.revision += 1;
                }
                None
            }
        }
    }

    /// Whether a start that carried the draft's images was accepted since
    /// the last call: the task holds them, so the draft lets them go. A
    /// refused or lost start keeps them.
    pub fn take_delivered(&mut self) -> bool {
        std::mem::take(&mut self.delivered)
    }

    pub fn resource_open(&self) -> bool {
        self.resources.open()
    }
    pub fn resource_view<I>(&self, intent: impl Fn(resources::Target) -> I) -> Option<Node<I>> {
        self.resources.preview_view(|_, target| intent(target))
    }
    pub fn resource_preview(&self) -> Option<&resources::Preview> {
        self.resources.preview()
    }
    pub fn with_image_ids(mut self, ids: Vec<String>) -> Self {
        self.image_ids = ids;
        self
    }
    pub fn take_delivered_ids(&mut self) -> Vec<String> {
        std::mem::take(&mut self.delivered_ids)
    }
    pub fn take_restored_input(&mut self) -> Option<input::Message> {
        self.restored_input.take()
    }
    pub fn accepts_images(&self) -> bool {
        if let Ok(Some(request)) = self.configuration.request() {
            return self.configuration.offer().is_some_and(|offer| {
                offer.models.iter().any(|model| {
                    model.adapter == request.adapter
                        && model.selection.provider == request.selection.provider
                        && model.selection.model == request.selection.model
                        && model.image_followups
                })
            });
        }
        self.session().is_some_and(|session| {
            session.resolved.native.image_followups
                || self.configuration.offer().is_some_and(|offer| {
                    offer.models.iter().any(|model| {
                        model.adapter == session.resolved.adapter
                            && model.selection.provider == session.resolved.selection.provider
                            && model.selection.model == session.resolved.selection.model
                            && model.image_followups
                    })
                })
        })
    }
    pub fn held_image_sizes(&self) -> Vec<(String, usize)> {
        let mut entries = BTreeMap::new();
        let mut add = |message: &input::Message| {
            for (index, image) in message.images.iter().enumerate() {
                let id = message
                    .drafts
                    .get(index)
                    .map(|image| image.id.clone())
                    .unwrap_or_else(|| format!("{}:{index}", message.id));
                entries.insert(id, image.bytes.len());
            }
        };
        for message in &self.queue {
            add(message);
        }
        drop(add);
        for (index, image) in self.images.iter().enumerate() {
            entries.insert(
                self.image_ids
                    .get(index)
                    .cloned()
                    .unwrap_or_else(|| format!("start:{index}")),
                image.bytes.len(),
            );
        }
        let mut add = |message: &input::Message| {
            for (index, image) in message.images.iter().enumerate() {
                let id = message
                    .drafts
                    .get(index)
                    .map(|image| image.id.clone())
                    .unwrap_or_else(|| format!("{}:{index}", message.id));
                entries.insert(id, image.bytes.len());
            }
        };
        if let Some(message) = &self.after_stop {
            add(message);
        }
        for request in self
            .pending
            .values()
            .chain(self.failed.iter())
            .chain(self.due.iter())
        {
            if let Request::ContinueInput { message, .. } = request {
                add(message);
            }
        }
        entries.into_iter().collect()
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
            // The question and the page a decision control was drawn for.
            Action::Decide(_) => self
                .decision()
                .zip(self.asking())
                .map(|(flow, (seq, ..))| format!("{seq}:{}", flow.page())),
            Action::SendQueuedNow(index) | Action::RemoveQueued(index) => {
                self.queue.get(*index).map(|message| message.id.clone())
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

    /// The current turn's description, retained even after old rows are evicted.
    pub fn session(&self) -> Option<&route_contract::session::Descriptor> {
        self.session.as_ref().filter(|session| {
            self.task
                .as_deref()
                .is_some_and(|task| session.applies_to(task, session.turn))
        })
    }

    fn supports(&self, check: impl FnOnce(&route_contract::session::Controls) -> bool) -> bool {
        // Older task streams have a fixed stop/continue/queue contract. This
        // compatibility path grants no native provider operations.
        match &self.session {
            None => true,
            Some(_) => self
                .session()
                .is_some_and(|session| check(&session.controls)),
        }
    }

    pub fn can_stop(&self) -> bool {
        self.supports(|controls| controls.stop)
    }
    pub fn can_queue(&self) -> bool {
        self.supports(|controls| controls.queue)
    }
    fn can_continue(&self) -> bool {
        self.supports(|controls| controls.continue_task)
    }
    fn can_answer(&self) -> bool {
        self.native.ready() || self.supports(|controls| controls.answer)
    }

    /// Whether Send can submit through this task's current composer mode.
    pub fn can_send(&self) -> bool {
        match self.mode() {
            Mode::Send => self.configuration.request().is_ok(),
            Mode::Queue => self.can_queue() || self.steer_choice().is_some(),
            Mode::Answer => self.can_answer() && self.native_typed(),
        }
    }

    /// Compact reading information, without adapter or protocol terminology.
    pub fn session_label(&self) -> Option<String> {
        let session = self.session();
        let (selected_provider, selected_model) = match session {
            Some(session) => (
                session.resolved.selection.provider.as_str(),
                session.resolved.selection.model.as_str(),
            ),
            None => {
                let state = self.observations.as_ref()?;
                (state.provider.as_deref()?, state.model.as_deref()?)
            }
        };
        if selected_provider.is_empty() || selected_model.is_empty() {
            return None;
        }
        let provider = coder_events::provider_name(&serde_json::json!(selected_provider));
        let using = format!("Using {provider} · {}", clip(selected_model, 120));
        Some(
            match session.and_then(|session| session.requested.as_ref()) {
                Some(requested)
                    if requested.provider != selected_provider
                        || (!requested.model.is_empty() && requested.model != selected_model) =>
                {
                    let provider =
                        coder_events::provider_name(&serde_json::json!(requested.provider));
                    let model = if requested.model.is_empty() {
                        String::new()
                    } else {
                        format!(" · {}", clip(&requested.model, 120))
                    };
                    format!("Requested {provider}{model} · {using}")
                }
                _ => using,
            },
        )
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

    /// Bind a keyboard choice to the question currently displayed.
    pub fn decision_action(&self, control: crate::decision::Control) -> Action {
        self.native
            .request()
            .map_or(Action::Decide(control), |request| Action::NativeDecision {
                request: request.id.clone(),
                control,
            })
    }

    /// Whether the current question offers a text answer.
    pub fn native_typed(&self) -> bool {
        let Some(request) = self.native.request() else {
            return true;
        };
        match &request.form {
            route_contract::pending::Form::Questions { form } => {
                self.native.ready()
                    && self
                        .native
                        .flow()
                        .is_some_and(|flow| form.questions[flow.page()].free_text)
            }
            route_contract::pending::Form::Approval => false,
        }
    }

    /// A separate editing lifetime for each live question and page.
    pub fn answer_context(&self) -> Option<String> {
        let request = self.native.request()?;
        Some(format!(
            "{}-{}",
            request.id.as_str(),
            self.native.flow()?.page()
        ))
    }

    /// What the composer's control does now.
    #[must_use]
    pub fn mode(&self) -> Mode {
        if self.native.request().is_some() {
            return Mode::Answer;
        }
        match (&self.phase, self.state) {
            (Phase::Following, State::Waiting) => Mode::Answer,
            (Phase::Following, State::Ended) => Mode::Send,
            _ => Mode::Queue,
        }
    }

    /// The composer's words: once the run's turn has ended, a message
    /// goes to OpenAgents, whose router answers it or hands it to Coder
    /// (#10094); while Coder works it waits for Coder's next turn.
    #[must_use]
    pub fn placeholder(&self) -> &'static str {
        match self.mode() {
            Mode::Send => PLACEHOLDER,
            // The send control sends now while a turn runs (#11685).
            Mode::Queue if self.steer_choice().is_some() => "Message Coder…",
            Mode::Queue => "Queue a message for Coder's next turn…",
            Mode::Answer => "Answer Coder…",
        }
    }

    /// Whether the composer's message goes to the chat's router rather
    /// than straight to Coder (#10094): the run's turn has ended, with a
    /// result, a failure, or a stop. While Coder works the message waits
    /// for its next turn, and a question Coder asked is answered to it.
    #[must_use]
    pub fn routes_followups(&self) -> bool {
        self.mode() == Mode::Send && self.task.is_some()
    }

    /// What the run's last turn did, for a follow-up's context, once that
    /// turn has ended ([`openagents_chat::coder_events::run_result`]).
    #[must_use]
    pub fn result(&self) -> Option<openagents_chat::router::CoderRun> {
        if !self.routes_followups() {
            return None;
        }
        let lines: Vec<Line> = self.lines.iter().cloned().collect();
        coder_events::run_result(&lines)
    }

    /// Hand `text` to Coder as the next turn of this task, in the same
    /// worktree: the router judged a follow-up is more work for it
    /// (#10094). The continue goes at the next [`Run::tick`]. `false`
    /// unless the turn has ended and nothing else is due.
    pub fn continue_with(&mut self, text: &str) -> bool {
        let text = text.trim();
        if text.is_empty()
            || text.len() > MAX_MESSAGE
            || !self.routes_followups()
            || self.due.is_some()
            || self.busy()
            || !self.can_continue()
        {
            return false;
        }
        let Some(task) = self.task.clone() else {
            return false;
        };
        let Ok(request) = self.continuation(text.into()) else {
            return false;
        };
        let _ = task;
        self.due = Some(request);
        self.revision += 1;
        true
    }

    /// The other way to send while Coder works: stop and send.
    #[must_use]
    pub fn steer_choice(&self) -> Option<Choice> {
        (self.phase == Phase::Following
            && self.state == State::Running
            && self.task.is_some()
            && self.can_stop()
            && self.can_continue()
            && self.supports(|controls| controls.stop_and_send))
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

    /// The question or approval the run waits on: the asking event's
    /// `seq`, its kind, and its text.
    fn asking(&self) -> Option<(u64, crate::decision::Kind, &str)> {
        use crate::decision::Kind;
        if self.state != State::Waiting {
            return None;
        }
        let line = self.lines.back()?;
        match &line.event {
            CoderEvent::Question(asked) => Some((line.seq, Kind::Question, &asked.text)),
            CoderEvent::Approval(asked) => Some((line.seq, Kind::Approval, &asked.text)),
            _ => None,
        }
    }

    /// Follow the asking event with a decision panel: a new one for a new
    /// question or approval, none once the run stops waiting.
    fn sync_decision(&mut self) {
        use crate::decision::{Flow, Kind};
        let Some((seq, kind, text)) = self.asking() else {
            self.decision = None;
            return;
        };
        if self.decision.as_ref().is_some_and(|(at, _)| *at == seq) {
            return;
        }
        let flow = match kind {
            Kind::Question => Flow::question(text),
            Kind::Approval => Flow::approval(text),
        };
        self.decision = Some((seq, flow));
    }

    /// The decision panel's state while the run waits on a question or an
    /// approval.
    #[must_use]
    pub fn decision(&self) -> Option<&crate::decision::Flow> {
        if let Some(flow) = self.native.flow() {
            return Some(flow);
        }
        let (seq, ..) = self.asking()?;
        self.decision
            .as_ref()
            .filter(|(at, _)| *at == seq)
            .map(|(_, flow)| flow)
    }

    /// Carry a decision step out: send the answer once every page is
    /// answered, or redraw the moved panel.
    fn decided(&mut self, step: crate::decision::Step) -> Option<(u64, Request)> {
        match step {
            crate::decision::Step::Stay => None,
            crate::decision::Step::Moved => {
                self.consumed_input = true;
                self.revision += 1;
                None
            }
            crate::decision::Step::Done(text) => {
                self.revision += 1;
                self.dispatch_continuation(text)
            }
        }
    }

    fn supports_request(&self, request: &Request) -> bool {
        match request {
            Request::ContinueConfigured {
                task,
                configuration,
                ..
            } => {
                self.task.as_ref() == Some(task)
                    && self
                        .configuration
                        .offer()
                        .is_some_and(|offer| offer.accepts(configuration))
                    && if self.state == State::Waiting {
                        self.can_answer()
                    } else {
                        self.can_continue()
                    }
            }
            Request::Start {
                configuration: Some(configuration),
                ..
            } => self
                .configuration
                .offer()
                .is_some_and(|offer| offer.accepts(configuration)),
            Request::Respond { task, reply } => {
                self.task.as_ref() == Some(task) && self.native.accepts(reply)
            }
            Request::Stop { .. } => self.can_stop(),
            Request::Continue { .. } if self.state == State::Waiting => self.can_answer(),
            Request::Continue { .. } => self.can_continue(),
            _ => true,
        }
    }

    fn request(&mut self, request: Request) -> Option<(u64, Request)> {
        if !self.supports_request(&request) {
            return None;
        }
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
        if self
            .due
            .as_ref()
            .is_some_and(|due| self.supports_request(due))
            && let Some(due) = self.due.take()
        {
            return self.request(due);
        }
        if !self.pending.values().any(|request| !mutation(request))
            && let Some(reference) = self.resource_due.clone()
            && let Some(task) = self.task.clone()
            && let Some(request) = self.request(Request::ResourceRead {
                task,
                generation: self.resources.generation(),
                reference,
            })
        {
            self.resource_due = None;
            return Some(request);
        }
        if !self.pending.values().any(|request| !mutation(request))
            && let Some(read) = self.child_due.clone()
            && let Some(task) = self.task.clone()
            && let Some(request) = self.request(Request::ChildRead {
                generation: self.children.generation(),
                task,
                read,
            })
        {
            self.child_due = None;
            return Some(request);
        }
        if !self.pending.values().any(|request| !mutation(request))
            && let Some(need) = self.workspace_due.clone()
            && let Some(request) = self.workspace_request(&need)
            && let Some(request) = self.request(request)
        {
            self.workspace_due = None;
            return Some(request);
        }
        if self.configuration_due && !self.pending.values().any(|request| !mutation(request)) {
            self.configuration_due = false;
            return self.request(Request::Options {
                task: self.task.clone(),
                dirs: self.dirs.clone(),
            });
        }
        let task = self.task.clone()?;
        if self.phase != Phase::Following {
            return None;
        }
        if self.state != State::Running
            && !matches!(self.failed, Some(Request::ContinueInput { .. }))
            && self.can_continue()
            && (self.state != State::Waiting || self.can_answer())
            && !self.pending.values().any(|p| !mutation(p))
        {
            if let Some(text) = self.after_stop.clone()
                && let Some(request) = self.dispatch_message(text)
            {
                self.after_stop = None;
                return Some(request);
            }
            if self.state == State::Ended
                && let Some(text) = self.queue.front().cloned()
                && let Some(request) = self.dispatch_message(text)
            {
                self.queue.pop_front();
                self.revision += 1;
                return Some(request);
            }
        }
        // A finished turn's change for the "What changed" card: read once,
        // then again now and then to notice a moved worktree, and
        // published when the person asks.
        let seq = self.result_seq();
        if seq.is_some() && seq != self.reviewed_seq {
            self.reviewed_seq = seq;
            self.reviewer.reset(Some(&task));
        }
        if !self.pending.values().any(|p| !mutation(p))
            && let Some(need) = self.reviewer.tick(now, seq.is_some(), true)
        {
            return self.request(match need {
                crate::changes::Need::Read { task } => Request::Review { task },
                crate::changes::Need::Publish {
                    task,
                    base,
                    head_commit,
                    head,
                } => Request::Publish {
                    task,
                    base,
                    head_commit,
                    head,
                },
            });
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

    /// What the run is doing, for [`crate::attention::indicator`], and how
    /// long it has been since the task was last heard from at `now`.
    #[must_use]
    pub fn activity(&self, now: Instant) -> (Activity, Duration) {
        let activity = match (&self.phase, self.state) {
            (Phase::Starting, _) | (Phase::Following, State::Running) => Activity::Working,
            (Phase::NeedsProject(_), _) | (Phase::Following, State::Waiting) => {
                Activity::AwaitingInput
            }
            (Phase::Failed, _) => Activity::Failed,
            (Phase::Following, State::Ended) => match self.lines.back().map(|line| &line.event) {
                Some(CoderEvent::Result(_)) => Activity::Completed,
                Some(CoderEvent::Failure(_)) => Activity::Failed,
                _ => Activity::Idle,
            },
        };
        (activity, now.saturating_duration_since(self.heard))
    }

    /// The last event's sequence number: what a person who has seen the
    /// run's ending has seen.
    #[must_use]
    pub fn mark(&self) -> u64 {
        self.lines.back().map_or(0, |line| line.seq)
    }

    /// Whether the task's last turn finished with a result.
    #[must_use]
    pub fn finished(&self) -> bool {
        self.result_seq().is_some()
    }

    /// What the finished turn changed, once read: the card's state.
    #[must_use]
    pub fn reviewer(&self) -> Option<&crate::changes::Reviewer> {
        let seq = self.result_seq()?;
        (self.reviewed_seq == Some(seq)).then_some(&self.reviewer)
    }

    /// The same, to fill the pane's syntax spans, refresh a stale view, or
    /// ask to publish.
    pub fn reviewer_mut(&mut self) -> Option<&mut crate::changes::Reviewer> {
        let seq = self.result_seq()?;
        if self.reviewed_seq == Some(seq) {
            Some(&mut self.reviewer)
        } else {
            None
        }
    }

    /// When the run next wants a tick.
    #[must_use]
    pub fn next_wake(&self, now: Instant) -> Instant {
        if self.due.is_some()
            || self.workspace_due.is_some()
            || self.child_due.is_some()
            || self.resource_due.is_some()
        {
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
                match &request {
                    // The card says why; the result card still names every
                    // file.
                    Request::ResourceRead {
                        generation,
                        reference,
                        ..
                    } => {
                        self.resources.received(*generation, reference, Err(error));
                        self.revision += 1;
                        return false;
                    }
                    Request::ChildRead {
                        generation, read, ..
                    } => {
                        self.children.fail(*generation, read, error);
                        self.revision += 1;
                        return false;
                    }
                    Request::WorkspaceOpen { .. } | Request::WorkspaceRead { .. } => {
                        self.workspace.fail(error);
                        self.revision += 1;
                        return false;
                    }
                    Request::Review { .. } => {
                        self.reviewer
                            .read(Err(crate::changes::ReadFailure::Failed(error)), now);
                        self.revision += 1;
                        return false;
                    }
                    Request::Publish { .. } => {
                        self.reviewer.published(Err(error));
                        self.revision += 1;
                        return false;
                    }
                    _ => {}
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
                if matches!(self.failed, Some(Request::ContinueInput { .. }))
                    && !matches!(request, Request::ContinueInput { .. })
                {
                    self.notice = Some(error);
                    self.revision += 1;
                    return false;
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
                self.children = children::Inspector::new(&task);
                self.resources = resources::Inspector::new(&task);
                self.resource_due = None;
                self.child_due = None;
                self.task = Some(task.clone());
                self.project = Some(project.clone());
                self.bind = Some((task, project));
                if !self.images.is_empty() {
                    self.delivered_ids.extend(self.image_ids.clone());
                    self.images.clear();
                    self.delivered = true;
                }
                // The chat's own project, should it start again.
                self.dirs.retain(|known| *known != checkout);
                self.dirs.insert(0, checkout);
                self.phase = Phase::Following;
                self.state = State::Running;
                self.error = None;
                self.failed = None;
                self.poll = now;
                self.heard = now;
            }
            (Request::Start { .. }, Answer::NeedsProject { why }) => {
                self.phase = Phase::NeedsProject(why);
            }
            (
                Request::Poll { task },
                Answer::Lines {
                    children,
                    lines,
                    state,
                    configuration,
                    native,
                },
            ) if self.task.as_ref() == Some(task) => {
                let config_changed = self
                    .configuration
                    .update(configuration.filter(|offer| offer.task.as_ref() == Some(task)));
                let native_changed = self.native.update(native, task, state);
                let mut last = self.cursor;
                let child_changed = children
                    .as_ref()
                    .is_some_and(|snapshot| self.children.activity.merge_sdk(snapshot));
                let mut changed = native_changed || config_changed || child_changed;
                for mut line in lines {
                    // A new follower replays from the first event; keep one
                    // copy of each.
                    if line.seq <= last || line.task != *task {
                        continue;
                    }
                    if line.seq != last + 1 {
                        self.history_incomplete = true;
                    }
                    last = line.seq;
                    self.cursor = last;
                    match &line.event {
                        CoderEvent::CoderStarted(started) => {
                            if self
                                .observations
                                .as_ref()
                                .is_some_and(|state| state.turn() > started.turn)
                            {
                                continue;
                            }
                            if self
                                .workspace()
                                .reference_turn()
                                .is_some_and(|turn| turn != started.turn)
                            {
                                self.workspace.reset();
                                self.workspace_due = None;
                            }
                            self.session = started.session.as_ref().map(|session| {
                                if session.applies_to(task, started.turn)
                                    && session.resolved.selection.provider == started.provider
                                    && session.resolved.selection.model == started.model
                                {
                                    session.clone()
                                } else {
                                    route_contract::session::Descriptor::default()
                                }
                            });
                            if let Some(session) = self.session() {
                                self.configuration.current(
                                    session.resolved.selection.clone(),
                                    session.resolved.adapter.clone(),
                                );
                            }
                            let host = self
                                .session()
                                .map_or(LOCAL, |session| session.owner.as_str());
                            self.observations =
                                Some(openagents_chat::observations::Projection::new(
                                    host,
                                    task,
                                    started.turn,
                                ));
                        }
                        CoderEvent::Observation { observation } => {
                            changed |= self.children.activity.consume(observation);
                            changed |= self.resources.consume(observation);
                            if self.observations.is_none()
                                && matches!(
                                    observation.body,
                                    route_contract::observation::Body::Started { .. }
                                )
                                && observation.applies_to(LOCAL, task, observation.scope.turn)
                            {
                                self.observations =
                                    Some(openagents_chat::observations::Projection::new(
                                        LOCAL,
                                        task,
                                        observation.scope.turn,
                                    ));
                            }
                            let Some(state) = &mut self.observations else {
                                continue;
                            };
                            let Some(update) = state.apply(observation) else {
                                continue;
                            };
                            changed = true;
                            self.history_incomplete |= state.incomplete;
                            if let Some(session) = &mut self.session {
                                if let (Some(provider), Some(model)) =
                                    (&state.provider, &state.model)
                                {
                                    if session.resolved.selection.provider != *provider {
                                        session.switch_to(Some(&format!("{provider}:{model}")));
                                    }
                                    session.resolved.selection.model = model.clone();
                                }
                                session.native_session.clone_from(&state.native_session);
                                session.resolved.version.clone_from(&state.version);
                            }
                            let Some(event) = update.event else { continue };
                            line.event = event;
                            if let Some(key) = update.key {
                                if let Some(seq) = self.observation_rows.get(&key)
                                    && let Some(existing) =
                                        self.lines.iter_mut().find(|line| line.seq == *seq)
                                {
                                    existing.event = line.event;
                                    continue;
                                }
                                self.observation_rows.insert(key, line.seq);
                            }
                        }
                        CoderEvent::ProviderSwitched(switched) => {
                            if let Some(session) = &mut self.session
                                && session.applies_to(task, switched.turn)
                            {
                                session.switch_to(switched.to.as_deref());
                            }
                        }
                        _ => {}
                    }
                    self.lines.push_back(line);
                    changed = true;
                }
                while self.lines.len() > MAX_LINES {
                    self.lines.pop_front();
                    self.history_incomplete = true;
                }
                let retained: std::collections::BTreeSet<_> =
                    self.lines.iter().map(|line| line.seq).collect();
                self.observation_rows
                    .retain(|_, seq| retained.contains(seq));
                if state != self.state {
                    if state != State::Running {
                        // A stop that was asked for has happened.
                        self.notice = None;
                    }
                    self.state = state;
                    changed = true;
                }
                if !matches!(self.failed, Some(Request::ContinueInput { .. }))
                    && self.error.take().is_some()
                {
                    changed = true;
                }
                if changed {
                    self.revision += 1;
                    self.poll = now;
                    self.heard = now;
                }
                return false;
            }
            (Request::Respond { reply, .. }, Answer::Responded(receipt))
                if reply.request == receipt.request =>
            {
                self.native.ack(&reply.request);
                self.poll = now;
                accepted = true;
            }
            (Request::Stop { .. }, Answer::Stopping) => {
                self.notice = Some("Asked Coder to stop. Its turn ends as stopped.".into());
                self.poll = now;
            }
            (Request::Continue { .. } | Request::ContinueConfigured { .. }, Answer::Continued) => {
                self.workspace.reset();
                self.workspace_due = None;
                self.state = State::Running;
                self.poll = now;
                self.heard = now;
                accepted = true;
            }
            (
                Request::ChildRead {
                    generation, read, ..
                },
                Answer::ChildPage(page),
            ) => {
                self.children.received(*generation, read, page);
            }
            (
                Request::ContinueInput {
                    task,
                    message,
                    configuration,
                },
                Answer::InputAccepted { input, receipt },
            ) if self.task.as_ref() == Some(task)
                && message.acknowledged(task, configuration.as_ref(), &input, &receipt) =>
            {
                self.phase = Phase::Following;
                self.workspace.reset();
                self.workspace_due = None;
                self.state = State::Running;
                self.delivered_ids.extend(message.image_ids());
                for reference in &input.resources {
                    self.resources.register(reference);
                }
                self.error = None;
                self.failed = None;
                self.poll = now;
                self.heard = now;
                accepted = true;
            }
            (Request::ContinueInput { .. }, _) => {
                self.failed = Some(request.clone());
                self.error = Some("These images weren't confirmed. Try again.".into());
                self.revision += 1;
                return false;
            }
            (
                Request::ResourceRead {
                    generation,
                    reference,
                    ..
                },
                Answer::Resource(content),
            ) => {
                self.resources.received(*generation, reference, Ok(content));
            }
            (Request::WorkspaceOpen { task }, Answer::WorkspaceReference(reference)) => {
                if self
                    .session()
                    .is_some_and(|session| session.turn > reference.turn)
                {
                    self.workspace
                        .fail("This workspace changed. Refresh the files.".into());
                } else {
                    self.workspace_due = self.workspace.referenced(reference, task);
                }
            }
            (Request::WorkspaceRead { request, .. }, Answer::WorkspaceRead(output)) => {
                if !self.workspace.received(request, output) {
                    self.workspace
                        .fail("These files changed. Refresh the workspace.".into());
                }
            }
            (Request::Options { task, .. }, Answer::Options(offer)) if offer.task == *task => {
                self.configuration.update(Some(offer));
            }
            (Request::Choose, Answer::Folder(Some(dir))) => {
                self.dirs.retain(|known| *known != dir);
                self.dirs.insert(0, dir);
                self.phase = Phase::Starting;
                self.error = None;
                self.due = Some(self.start_request());
            }
            (Request::Choose, Answer::Folder(None)) => {}
            (Request::Review { .. }, Answer::Review(review)) => {
                self.reviewer.read(Ok(*review), now);
            }
            (Request::Publish { .. }, Answer::Published(publication)) => {
                self.reviewer.published(Ok(*publication));
            }
            _ => {
                self.error = Some("Something went wrong. Try again.".into());
            }
        }
        self.sync_decision();
        if mutation(&request) || matches!(request, Request::Choose) {
            if self.failed.as_ref() == Some(&request) {
                self.failed = None;
            }
            if (accepted || !matches!(request, Request::Start { .. }))
                && !matches!(self.failed, Some(Request::ContinueInput { .. }))
            {
                self.error = None;
            }
        }
        self.revision += 1;
        accepted
    }

    /// Carry out `action` with the composer's `text`.
    pub fn action(&mut self, action: Action, text: &str) -> Option<(u64, Request)> {
        self.action_with_images(action, text, vec![], vec![])
    }
    pub fn action_with_images(
        &mut self,
        action: Action,
        text: &str,
        images: Vec<crate::attachments::Upload>,
        drafts: Vec<crate::attachments::Image>,
    ) -> Option<(u64, Request)> {
        self.consumed_input = false;
        if matches!(self.failed, Some(Request::ContinueInput { .. }))
            && matches!(
                action,
                Action::Send | Action::Queue | Action::Steer | Action::SendQueuedNow(_)
            )
            && !(action == Action::Send && images.is_empty() && self.native.ready())
        {
            self.notice = Some("Retry or edit the earlier image message first.".into());
            self.revision += 1;
            return None;
        }
        let message = match input::Message::with_images(text.trim(), images, drafts) {
            Ok(message) => message,
            Err(error) => {
                self.notice = Some(error);
                self.revision += 1;
                return None;
            }
        };
        if !message.images.is_empty()
            && matches!(action, Action::Send | Action::Queue | Action::Steer)
        {
            let error = if message.text.is_empty() {
                Some("Write a message to send with these images.")
            } else if (self.native.request().is_some() && action == Action::Send)
                || self.state == State::Waiting
            {
                Some("Answer the current question before sending images.")
            } else if !self.accepts_images() {
                Some("Choose Claude Code with live approvals to send these images.")
            } else {
                None
            };
            if let Some(error) = error {
                self.notice = Some(error.into());
                self.revision += 1;
                return None;
            }
        }
        let text = text.trim();
        match action {
            Action::EditFailedInput => {
                if self.state != State::Ended || self.busy() {
                    return None;
                }
                if !matches!(self.failed, Some(Request::ContinueInput { .. })) {
                    return None;
                }
                if let Some(Request::ContinueInput { message, .. }) = self.failed.take() {
                    self.restored_input = Some(message);
                    self.error = None;
                    self.revision += 1;
                }
                None
            }
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
            Action::Resource(target) => {
                if let Some(reference) = self.resources.apply(&target) {
                    self.resource_due = Some(reference);
                } else if !self.resources.open() {
                    self.resource_due = None;
                }
                self.revision += 1;
                None
            }
            Action::Child(target) => {
                if let Some(read) = self.children.apply(&target) {
                    self.child_due = Some(read);
                } else if !self.children.open() {
                    self.child_due = None;
                }
                self.revision += 1;
                None
            }
            Action::Workspace(target) => {
                let need = self.workspace.apply(&target);
                if !self.workspace.visible {
                    self.workspace_due = None;
                } else if let Some(workspace::Need::File(open)) = need {
                    self.workspace_file = Some(open);
                } else if let Some(need) = need {
                    self.workspace_due = Some(need);
                }
                self.revision += 1;
                None
            }
            Action::Configure(target) => {
                if self.busy() {
                    return None;
                }
                if target.control == configuration::Control::Refresh {
                    return self.refresh_configuration();
                }
                if self.configuration.apply(&target) {
                    if self.task.is_none() {
                        match self.configuration.request() {
                            Ok(configuration) => {
                                self.start_configuration = configuration;
                                let next = self.start_request();
                                if matches!(self.failed, Some(Request::Start { .. })) {
                                    self.failed = Some(next.clone());
                                }
                                if matches!(self.due, Some(Request::Start { .. })) {
                                    self.due = Some(next);
                                }
                            }
                            Err(error) => self.notice = Some(error),
                        }
                    }
                    self.revision += 1;
                }
                None
            }
            Action::NativeDecision { request, control } => {
                if self.busy() {
                    return None;
                }
                let answer = self.native.control(&request, control);
                self.revision += 1;
                if let Some(reply) = answer {
                    let task = self.task.clone()?;
                    return self.request(Request::Respond { task, reply });
                }
                None
            }
            Action::NativeCancel(id) => {
                if self.busy() {
                    return None;
                }
                let reply = self
                    .native
                    .reply(&id, route_contract::pending::Answer::Cancel)?;
                let task = self.task.clone()?;
                self.request(Request::Respond { task, reply })
            }
            Action::Approve | Action::Deny => {
                if !self.asking_approval() || !self.can_answer() {
                    return None;
                }
                let text = if action == Action::Approve {
                    "Approved."
                } else {
                    "Denied."
                };
                self.dispatch_continuation(text.into())
            }
            Action::Decide(control) => {
                if !self.can_answer() {
                    return None;
                }
                self.sync_decision();
                if self.busy() {
                    return None;
                }
                let step = self.decision.as_mut()?.1.control(control);
                self.decided(step)
            }
            Action::Send | Action::Queue | Action::Steer => {
                if action == Action::Send && self.native.request().is_some() {
                    if self.busy() {
                        return None;
                    }
                    let before = self.native.flow().cloned();
                    let reply = self.native.typed(text);
                    self.consumed_input = reply.is_some() || before != self.native.flow().cloned();
                    self.revision += 1;
                    if let Some(reply) = reply {
                        let task = self.task.clone()?;
                        return self.request(Request::Respond { task, reply });
                    }
                    return None;
                }
                if text.is_empty() || text.len() > MAX_MESSAGE {
                    return None;
                }
                // Enter sends now (#11685): while Coder works the send
                // control stops the turn and continues with the message;
                // Ctrl+Enter ([`Action::Queue`]) queues it for later.
                let action = if action == Action::Send
                    && self.mode() == Mode::Queue
                    && self.steer_choice().is_some()
                {
                    Action::Steer
                } else {
                    action
                };
                let mode = if action == Action::Send {
                    self.mode()
                } else {
                    Mode::Queue
                };
                if action == Action::Steer {
                    let task = self.task.clone()?;
                    self.steer_choice()?;
                    let request = self.request(Request::Stop { task })?;
                    self.after_stop = Some(message.clone());
                    self.notice = Some("Coder stops, then continues with your message.".into());
                    self.revision += 1;
                    self.consumed_input = true;
                    return Some(request);
                }
                match mode {
                    Mode::Queue => {
                        if !self.can_queue() {
                            return None;
                        }
                        if self.queue.len() >= MAX_QUEUED {
                            self.notice = Some(format!(
                                "Coder holds {MAX_QUEUED} messages for its next turn at most."
                            ));
                        } else {
                            self.queue.push_back(message.clone());
                            self.consumed_input = true;
                            self.notice = None;
                        }
                        self.revision += 1;
                        None
                    }
                    // A question's panel takes the typed answer for its
                    // page; an approval's answer in the person's own words
                    // goes as it is.
                    Mode::Answer
                        if action == Action::Send
                            && self.decision().is_some_and(|flow| {
                                flow.kind() == crate::decision::Kind::Question
                            }) =>
                    {
                        if self.busy() {
                            return None;
                        }
                        let step = self.decision.as_mut()?.1.answer_typed(text);
                        self.decided(step)
                    }
                    Mode::Answer | Mode::Send => self.dispatch_message(message.clone()),
                }
            }
            Action::RemoveQueued(index) => {
                self.queue.remove(index)?;
                self.revision += 1;
                None
            }
            Action::Plan(control) => {
                let items = self.plan_items()?;
                self.plan.apply(control, &items);
                self.revision += 1;
                None
            }
            Action::SendQueuedNow(index) => {
                if (self.state == State::Running && self.steer_choice().is_none())
                    || (self.state != State::Running && !self.can_continue())
                    || (self.state == State::Waiting && !self.can_answer())
                {
                    return None;
                }
                let text = self.queue.get(index)?.clone();
                let request = if self.state == State::Running {
                    let task = self.task.clone()?;
                    let request = self.request(Request::Stop { task });
                    if request.is_some() {
                        self.after_stop = Some(text.clone());
                    }
                    request
                } else {
                    self.dispatch_message(text)
                };
                if request.is_some() {
                    self.queue.remove(index);
                    self.revision += 1;
                }
                request
            }
        }
    }

    /// The task's plan as it stands: the latest one a turn recorded, or
    /// `None` when it recorded none or cleared it.
    #[must_use]
    pub fn plan_items(&self) -> Option<Vec<openagents_chat::plan::Item>> {
        if let Some(items) = self
            .observations
            .as_ref()
            .and_then(|state| state.plan.as_ref())
        {
            return Some(items.clone());
        }
        openagents_chat::plan::latest(self.lines.iter().map(|line| &line.event)).map(<[_]>::to_vec)
    }

    /// Whether the composer's text was queued rather than sent: the draft
    /// clears at once.
    #[must_use]
    pub fn queued(&self) -> usize {
        self.queue.len()
    }

    /// The run as transcript rows.
    pub fn rows(&mut self) -> Vec<Node<()>> {
        self.rows_anchored(&|_| None)
            .into_iter()
            .map(|(_, row)| row)
            .collect()
    }

    /// The run as transcript rows, each with the chat message it follows
    /// (#10094): `anchor` finds, for the message that started a Coder
    /// turn, the index of the chat's reply that handed it to Coder, and
    /// that turn's rows follow it. The chat shows that message itself, so
    /// the run does not repeat it. A turn with no such reply (an answer, a
    /// queued message) follows the turn before it, and rows with no anchor
    /// (`None`), with the run's controls, go last. Within a turn the
    /// person's message comes before its "Coder continued" card.
    pub fn rows_anchored(
        &mut self,
        anchor: &dyn Fn(&str) -> Option<usize>,
    ) -> Vec<(Option<usize>, Node<()>)> {
        self.actions.clear();
        let mut out = Rows::default();
        // Each turn's prompt: the last message its trajectory opens with,
        // after the conversation it carries.
        let mut previous = None;
        for line in &self.lines {
            if let CoderEvent::Step(step) = &line.event
                && step.kind == StepKind::Message
            {
                out.prompts.insert(step.turn, step.text.trim().to_owned());
            }
        }
        let prompts = out.prompts.clone();
        for (turn, prompt) in &prompts {
            let asked =
                crate::basic_chats::handoff_request(prompt).unwrap_or_else(|| prompt.clone());
            let found = anchor(asked.trim());
            let at = match (found, previous) {
                (Some(at), Some(before)) => Some(at.max(before)),
                (Some(at), None) => Some(at),
                (None, before) => before,
            };
            if let (Some(at), true) = (at, found.is_some()) {
                out.anchored.insert(*turn, at);
            } else if let Some(at) = at {
                out.following.insert(*turn, at);
            }
            previous = at;
        }
        for line in &self.lines {
            out.line(line);
        }
        let last_turn = self.lines.back().map(turn_of);
        out.close(self.state == State::Running && self.phase == Phase::Following);
        let mut anchored: Vec<(Option<usize>, Node<()>)> = Vec::with_capacity(out.rows.len());
        let mut bounds = out.bounds.iter().peekable();
        let mut at = None;
        for (index, row) in out.rows.into_iter().enumerate() {
            while let Some((start, anchor)) = bounds.peek() {
                if *start > index {
                    break;
                }
                at = *anchor;
                bounds.next();
            }
            anchored.push((at, row));
        }
        let mut rows = Vec::new();
        if let Some(label) = self.session_label() {
            rows.push(status("coder-session", &label));
        }
        if let Some(state) = &self.observations {
            if state.incomplete || self.history_incomplete {
                rows.push(status(
                    "coder-history-gap",
                    "Some earlier output is unavailable.",
                ));
            }
            if let Some((Some(used), capacity)) = state.context {
                let text = capacity.map_or_else(
                    || format!("Context: {used} tokens"),
                    |capacity| format!("Context: {used} of {capacity} tokens"),
                );
                rows.push(status("coder-context", &text));
            }
            if let Some(usage) = &state.usage {
                let mut parts = Vec::new();
                if let Some(input) = usage.input {
                    parts.push(format!("{input} input tokens"));
                }
                if let Some(output) = usage.output {
                    parts.push(format!("{output} output tokens"));
                }
                if !parts.is_empty() {
                    rows.push(status("coder-usage", &parts.join(" · ")));
                }
            }
            if let Some(policy) = &state.policy {
                rows.push(status("coder-policy", policy));
            }
        }
        // The task's plan, above what the run is doing now (#10471).
        if let Some(items) = self.plan_items() {
            let live = self.state == State::Running;
            if let Some((panel, controls)) = self.plan.view("coder-plan", &items, live) {
                for (key, control) in controls {
                    self.actions.insert(key, Action::Plan(control));
                }
                rows.push(panel);
            }
        }
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
                    // Who is starting, from the moment the start is asked
                    // (#10115).
                    label: self.engine.map_or_else(
                        || "Starting Coder…".to_owned(),
                        |engine| format!("Starting {}…", engine.name()),
                    ),
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
        // The question or approval the run waits on is the decision panel,
        // in place of its card.
        self.sync_decision();
        if let Some((seq, flow)) = &self.decision {
            let panel = flow.view("coder", !self.busy() && self.can_answer());
            let card = format!("coder-{seq}");
            for (key, control) in panel.controls {
                let action = match control {
                    crate::decision::Control::Pick(0)
                        if flow.kind() == crate::decision::Kind::Approval =>
                    {
                        Action::Approve
                    }
                    crate::decision::Control::Pick(_)
                        if flow.kind() == crate::decision::Kind::Approval =>
                    {
                        Action::Deny
                    }
                    control => Action::Decide(control),
                };
                self.actions.insert(key, action);
            }
            match anchored.iter_mut().find(|(_, row)| row.key == card) {
                Some((_, row)) => *row = panel.node,
                None => rows.push(panel.node),
            }
        }
        if let Some(request) = self.native.request().cloned() {
            let prefix = format!("coder-native-{}", request.id.as_str());
            let flow = self.native.flow().expect("a pending request has a flow");
            let panel = flow.view(&prefix, self.native.ready() && !self.busy());
            for (key, control) in panel.controls {
                self.actions.insert(
                    key,
                    Action::NativeDecision {
                        request: request.id.clone(),
                        control,
                    },
                );
            }
            rows.push(panel.node);
            rows.push(self.buttons(
                &format!("{prefix}-cancel-row"),
                &[(
                    &format!("{prefix}-cancel"),
                    "Cancel request",
                    Action::NativeCancel(request.id),
                )],
            ));
        }
        if self.active() && self.task.is_some() && self.can_stop() {
            rows.push(self.buttons(
                "coder-stop-row",
                &[("coder-stop", "Stop Coder", Action::Stop)],
            ));
        }
        let actions = &mut self.actions;
        if let Some(view) = self.resources.list_view(|key, target| {
            actions.insert(key.into(), Action::Resource(target));
            ()
        }) {
            rows.push(view);
        }
        let queued: Vec<String> = self.queue.iter().map(ToString::to_string).collect();
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
                if self.state == State::Ended
                    && matches!(&self.failed,Some(Request::ContinueInput {message,..}) if !message.drafts.is_empty())
                {
                    rows.push(self.buttons(
                        "coder-edit-input-row",
                        &[("coder-edit-input", "Edit message", Action::EditFailedInput)],
                    ));
                }
            }
        }
        anchored.extend(rows.into_iter().map(|row| (None, row)));
        anchored
    }

    /// A run's controls (Stop Coder, Approve and Deny, Retry, a queued
    /// message's Send now and Remove): each as wide as its words in the
    /// transcript, as the phone draws its Coder controls (#10075, #10091),
    /// never spanning the reading band.
    fn buttons(&mut self, key: &str, buttons: &[(&str, &str, Action)]) -> Node<()> {
        let enabled = !self.busy();
        let children = buttons
            .iter()
            .map(|(key, label, action)| {
                self.actions.insert((*key).into(), action.clone());
                let supported = match action {
                    Action::Stop => self.can_stop(),
                    Action::NativeCancel(id) => {
                        self.native.ready()
                            && self
                                .native
                                .request()
                                .is_some_and(|request| &request.id == id)
                    }
                    Action::Retry => self
                        .failed
                        .as_ref()
                        .is_some_and(|request| self.supports_request(request)),
                    Action::SendQueuedNow(_) if self.state == State::Running => {
                        self.steer_choice().is_some()
                    }
                    Action::SendQueuedNow(_) => self.can_continue(),
                    _ => true,
                };
                let mut node = button(key, label, enabled && supported);
                node.style.intrinsic_width = Some(true);
                node
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
        Request::ResourceRead { .. }
            | Request::ChildRead { .. }
            | Request::WorkspaceOpen { .. }
            | Request::WorkspaceRead { .. }
            | Request::Poll { .. }
            | Request::Options { .. }
            | Request::Choose
            | Request::Review { .. }
            | Request::Publish { .. }
    )
}

/// The turn an event belongs to.
#[must_use]
pub fn turn_of(line: &Line) -> usize {
    match &line.event {
        CoderEvent::Observation { observation: e } => e.scope.turn,
        CoderEvent::CoderStarted(e) => e.turn,
        CoderEvent::Step(e) => e.turn,
        CoderEvent::Output(e) => e.turn,
        CoderEvent::ProviderSwitched(e) => e.turn,
        CoderEvent::Question(e) | CoderEvent::Approval(e) => e.turn,
        CoderEvent::Progress(e) => e.turn,
        CoderEvent::Status(e) => e.turn,
        CoderEvent::Result(e) => e.turn,
        CoderEvent::Failure(e) => e.turn,
        CoderEvent::Stopped(e) => e.turn,
    }
}

/// One tool row: a group, a command, a tool call, or a thought.
struct Tool {
    key: String,
    name: String,
    detail: String,
    body: Vec<String>,
    state: ToolState,
}

/// The rows of a run, built one line at a time.
#[derive(Default)]
struct Rows {
    rows: Vec<Node<()>>,
    /// The reply being written: its key and text.
    reply: Option<(String, String)>,
    /// The commands, tool calls, and thoughts since the last other row,
    /// drawn grouped when something else comes (#10117).
    stretch: Stretch,
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
    /// Each turn's prompt, the message that started it.
    prompts: BTreeMap<usize, String>,
    /// The turns whose prompt the chat shows, and the chat message each
    /// follows: their prompt is not shown again.
    anchored: BTreeMap<usize, usize>,
    /// The turns that follow an earlier turn's anchor.
    following: BTreeMap<usize, usize>,
    /// Where each turn's rows start, and the chat message they follow.
    bounds: Vec<(usize, Option<usize>)>,
    /// A later turn's start card, drawn after the message that started
    /// it.
    held: Option<Node<()>>,
}

impl Rows {
    fn line(&mut self, line: &Line) {
        if matches!(line.event, CoderEvent::Observation { .. }) {
            return;
        }
        let key = format!("coder-{}", line.seq);
        self.turn = turn_of(line);
        // A later turn's trajectory opens with the conversation it
        // carries, messages and replies, then its prompt: the card waits
        // for the prompt, and anything else draws it.
        let carried = matches!(
            &line.event,
            CoderEvent::Step(step) if matches!(step.kind, StepKind::Message | StepKind::Reply)
        );
        if !carried && !matches!(line.event, CoderEvent::CoderStarted(_)) {
            self.flush_held();
        }
        match &line.event {
            CoderEvent::Step(step) if step.kind == StepKind::Reply => {
                self.draw_stretch();
                match &mut self.reply {
                    Some((_, text)) => text.push_str(&step.text),
                    None => self.reply = Some((key, step.text.clone())),
                }
                return;
            }
            _ => self.flush_reply(),
        }
        // A command, a tool call, a thought, or what one returned joins
        // the stretch; anything else draws it first. Progress does neither.
        if !matches!(line.event, CoderEvent::Progress(_)) {
            if self.stretch.push(line.seq, &line.event) {
                match &line.event {
                    CoderEvent::Step(step) => match step.kind {
                        StepKind::Thinking => self.thoughts += 1,
                        StepKind::Command => self.commands += 1,
                        StepKind::ToolCall => self.calls += 1,
                        _ => {}
                    },
                    CoderEvent::Output(output) => {
                        self.failed += usize::from(output.timed_out || output.exit != Some(0));
                    }
                    _ => {}
                }
                return;
            }
            self.draw_stretch();
        }
        match &line.event {
            CoderEvent::CoderStarted(started) => {
                self.close_turn();
                // One short line (#10115): who works, and why only when
                // that is news. The task and its worktree stay in the
                // result and the export.
                let lines: Vec<(String, bool)> = started
                    .news()
                    .map(|news| (news.to_owned(), false))
                    .into_iter()
                    .collect();
                // A later turn's card keeps its turn, which follows the
                // message that started it.
                let title = if started.turn == 1 {
                    started.line()
                } else {
                    format!(
                        "Coder continued on {} (turn {})",
                        coder_events::provider_name(&serde_json::json!(started.provider)),
                        started.turn
                    )
                };
                self.flush_held();
                let anchor = self
                    .anchored
                    .get(&started.turn)
                    .or_else(|| self.following.get(&started.turn))
                    .copied();
                self.bounds.push((self.rows.len(), anchor));
                // A later turn's card follows the message that started it.
                if started.turn == 1 {
                    self.rows.push(card(&key, &title, &lines));
                } else {
                    self.held = Some(card(&key, &title, &lines));
                }
            }
            CoderEvent::Step(step) => match step.kind {
                StepKind::Message => {
                    // The handoff prompt is the person's own message, which
                    // the chat shows just above the run: shown once, there,
                    // never again as "Continued from the OpenAgents app"
                    // (#10076). Its provenance stays in the task's prompt.
                    let handoff = crate::basic_chats::handoff_request(&step.text).is_some();
                    let prompt =
                        self.prompts.get(&step.turn).map(String::as_str) == Some(step.text.trim());
                    // A prompt the chat already shows, above this turn.
                    let shown_by_chat = prompt && self.anchored.contains_key(&step.turn);
                    let new = self.shown.insert((true, step.text.trim().to_owned()));
                    if !handoff && !shown_by_chat && (new || self.turn == 1) {
                        self.rows.push(message(&key, MessageRole::User, &step.text));
                    }
                    if prompt {
                        self.flush_held();
                    }
                }
                // A stretch takes these; only an observation with no call
                // before it reaches here.
                StepKind::Thinking | StepKind::Command | StepKind::ToolCall => {}
                StepKind::Observation => self.rows.push(status(&key, &step.text)),
                // A plan update draws in the plan panel, not the transcript.
                StepKind::Note if step.plan.is_some() => {}
                StepKind::Note => self.rows.push(status(&key, &step.text)),
                StepKind::Reply => {}
            },
            CoderEvent::Output(output) => {
                // An output whose command the stretch does not hold.
                let failed = output.timed_out || output.exit != Some(0);
                if failed {
                    self.failed += 1;
                }
                let mut text = output.text.trim_end().to_owned();
                if output.truncated {
                    text.push_str("\n…");
                }
                self.rows.push(tool_row(Tool {
                    key,
                    name: "Output".into(),
                    detail: first_line(&output.command),
                    body: vec![text],
                    state: if failed {
                        ToolState::Failed
                    } else {
                        ToolState::Done
                    },
                }));
            }
            // Only which engine runs now (#10120): the provider's refusal
            // stays in the record.
            CoderEvent::ProviderSwitched(switch) => {
                self.rows.push(notice(&key, &switch.line()));
            }
            CoderEvent::Progress(progress) => {
                self.seconds = progress.seconds;
                // "Working · step 5 · ≈40% done · 9s": Jev's
                // estimate of how much is complete, never a budget.
                self.progress = Some(progress.line());
            }
            // The terminal's status line; the run's own rows say the rest.
            CoderEvent::Status(_) | CoderEvent::Observation { .. } => {}
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
                if let Some(issue) = &result.issue {
                    lines.push((issue.line(), false));
                }
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
                    if let Some(status) = crate::landing::Status::from_result(&result) {
                        children.push(status.node(&format!("{key}-landing")));
                    }
                }
                self.rows.push(node);
            }
            CoderEvent::Failure(failure) => {
                self.close_turn();
                let mut lines = vec![(failure.shown(), false)];
                if let Some(issue) = &failure.issue {
                    lines.push((issue.line(), false));
                }
                let mut node = card(&key, "Coder didn't finish", &lines);
                if let Element::Stack { children, .. } = &mut node.element
                    && let Some(status) = failure
                        .issue
                        .as_ref()
                        .and_then(crate::landing::Status::from_issue)
                {
                    children.push(status.node(&format!("{key}-landing")));
                }
                self.rows.push(node);
            }
            CoderEvent::Stopped(stopped) => {
                self.close_turn();
                self.rows.push(notice(&key, &stopped.message));
            }
        }
    }

    fn flush_held(&mut self) {
        if let Some(card) = self.held.take() {
            self.rows.push(card);
        }
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

    /// Draw the stretch, grouped (#10117): a group's label, each call one
    /// line, and a long run folded; each expands to its calls and output.
    fn draw_stretch(&mut self) {
        let stretch = std::mem::take(&mut self.stretch);
        for item in stretch.items(true) {
            let node = item_row(&item);
            self.rows.push(node);
        }
    }

    /// A turn ended: its "Worked for" line.
    fn close_turn(&mut self) {
        self.stretch.settle();
        self.draw_stretch();
        self.flush_reply();
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
        self.flush_held();
        if !running {
            self.stretch.settle();
        }
        self.draw_stretch();
        self.flush_reply();
        if !running {
            self.progress = None;
        }
    }
}

/// One item of a stretch as a transcript row: a group's label (its calls
/// and their output inside), a call (its command and output inside), a
/// thought, or the fold over a long run's earlier lines.
fn item_row(item: &Item<'_>) -> Node<()> {
    match item {
        Item::Group {
            key,
            label,
            members,
        } => {
            let mut body = Vec::new();
            for member in members {
                match member {
                    // Each call's line, what it returned indented under
                    // it: the code font has no diamond.
                    Entry::Call(shown) => {
                        body.push(shown.line());
                        if let Some(command) = shown.command() {
                            body.push(format!("  $ {command}"));
                        }
                        if !shown.output.is_empty() {
                            body.push(indent(&shown.output));
                        }
                    }
                    Entry::Thought { text, .. } => body.push(format!("· {text}")),
                }
            }
            tool_row(Tool {
                key: format!("coder-{key}"),
                name: label.text.clone(),
                detail: failures(label),
                body,
                state: label_state(label),
            })
        }
        Item::Call(shown) => call_row(shown),
        Item::Thought { seq, text } => tool_row(Tool {
            key: format!("coder-{seq}"),
            name: "Thinking".into(),
            detail: first_line(text),
            body: vec![(*text).to_owned()],
            state: ToolState::Done,
        }),
        Item::More { key, label, hidden } => tool_row(Tool {
            key: format!("coder-{key}-more"),
            name: label.text.clone(),
            detail: failures(label),
            body: hidden
                .iter()
                .map(|item| match item {
                    Item::Call(shown) => shown.line(),
                    Item::Thought { text, .. } => format!("· {}", first_line(text)),
                    Item::Group { label, .. } | Item::More { label, .. } => label.line(),
                })
                .collect(),
            state: label_state(label),
        }),
    }
}

/// One call: its verb, then its target and any failure; inside, a
/// command's own line and what the call returned.
fn call_row(shown: &Shown) -> Node<()> {
    let (name, target) = match shown.verb() {
        Some(verb) => (verb.to_owned(), shown.target()),
        None => (shown.target(), String::new()),
    };
    let detail = match (target.is_empty(), shown.result()) {
        (_, None) => target,
        (true, Some(result)) => result,
        (false, Some(result)) => format!("{target} · {result}"),
    };
    let mut body = Vec::new();
    if let Some(command) = shown.command() {
        body.push(format!("$ {command}"));
    }
    if !shown.output.is_empty() {
        body.push(shown.output.clone());
    }
    tool_row(Tool {
        key: format!("coder-{}", shown.seq),
        name,
        detail,
        body,
        state: if shown.running {
            ToolState::Running
        } else if shown.failed() {
            ToolState::Failed
        } else {
            ToolState::Done
        },
    })
}

/// `text` with every line two spaces in.
fn indent(text: &str) -> String {
    text.lines()
        .map(|line| format!("  {line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn failures(label: &tool_groups::Label) -> String {
    match label.failed {
        0 => String::new(),
        n => format!("{n} failed"),
    }
}

fn label_state(label: &tool_groups::Label) -> ToolState {
    if label.running {
        ToolState::Running
    } else if label.failed > 0 {
        ToolState::Failed
    } else {
        ToolState::Done
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
            // Nothing inside: one line that does not expand.
            children: if tool.body.iter().all(|part| part.trim().is_empty()) {
                Vec::new()
            } else {
                vec![Node {
                    key: format!("{key}-body"),
                    style: Style {
                        foreground: Some(gray()),
                        ..Style::default()
                    },
                    element: Element::Text {
                        value: tool.body.join("\n"),
                        role: TextRole::Code,
                    },
                }]
            },
        },
    }
}

/// Receded text, from the theme seam ([`crate::visual::inks`]).
fn gray() -> Color {
    crate::visual::inks().quiet
}

/// The card fill, from the theme seam.
fn card_fill() -> Color {
    crate::visual::inks().card
}

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
            background: Some(card_fill()),
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
