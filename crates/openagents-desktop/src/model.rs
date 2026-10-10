//! The window's state and what each click does.
//!
//! The model never touches the socket, the clipboard, or the file chooser.
//! [`Model::tick`] and [`Model::activate`] return [`Request`]s; the shell
//! runs them off the UI thread and hands each [`Outcome`] back through
//! [`Model::outcome`]. That keeps every rule here testable against
//! [`crate::fake::FakeHost`] with an injected clock, and it keeps the
//! window process to public data: the only secret it ever holds is the code
//! it is showing, which is on the screen anyway.

use crate::codes::{Action, Codes, Conditions};
use crate::control::{
    Autostart, ControlError, Device, EngineReport, NearbyPrompt, PickError, Project, Status,
};
pub use crate::folder::Chosen;
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// How often the window asks the host for its status while a code shows:
/// a new phone ends the code screen.
pub const FAST_POLL: Duration = Duration::from_secs(2);
/// How often it asks otherwise.
pub const SLOW_POLL: Duration = Duration::from_secs(5);
/// How often Coder's tasks and sign-ins are read.
pub const CODER_POLL: Duration = Duration::from_secs(15);
/// How often the window reads Coder's engine and usage while it is shown.
/// It also reads them at once when it is shown or focused, on a new chat,
/// and after a run is passed over or refused for capacity (#10105).
pub const ENGINE_POLL: Duration = Duration::from_secs(60);
/// How soon to read again while a usage refresh is due.
pub const ENGINE_WAIT: Duration = Duration::from_secs(2);
/// What the project row says when no folder chooser opened, with the way
/// forward.
pub const NO_CHOOSER: &str = "No folder chooser opened on this computer. \
     Install zenity or kdialog, or start the desktop portal, then choose again.";
/// What the project row says when the host would not take a folder.
pub const FOLDER_REFUSED: &str = "Coder couldn't use that folder. Choose another one.";
/// What the window says when a setting did not change: the host did not
/// answer for the whole of [`crate::control::PATIENCE`], or refused.
pub const SETTING_FAILED: &str = "Couldn't change that setting. Try again.";
/// How many times the window sends one chosen folder's swap before it
/// gives up on it: the first try, then once a refresh after each failure.
pub const SAVE_TRIES: u32 = 4;
/// How long a copied code stays on the clipboard.
pub const CLIPBOARD_LIFE: Duration = Duration::from_secs(60);
/// How long Coder may go without answering before the screens say so and
/// offer **Try again**, instead of a line that waits forever.
pub const STALL: Duration = Duration::from_secs(20);
/// While Coder does not answer, how often the app starts it again on its
/// own (the launch start: register the login agent, upgrading an earlier
/// setup first). Polling for an answer goes on every [`SLOW_POLL`].
pub const RESTART: Duration = Duration::from_secs(60);

/// A folder the person chose that the host has not finished taking on: the
/// project row shows it with **Saving…** at once, and until the swap is
/// done. A swap that failed partway is sent again on the next answer from
/// the host ([`crate::control::pick_project`] finishes it), up to
/// [`SAVE_TRIES`] times.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Saving {
    /// The folder chosen.
    pub path: PathBuf,
    /// The project it replaces.
    pub replace: Option<String>,
    /// Whether the switch is on for it.
    pub autostart: bool,
    /// Whether its [`Request::AddProject`] is out.
    pub running: bool,
    /// How many times it was sent.
    pub tries: u32,
}

impl Saving {
    fn request(&self) -> Request {
        Request::AddProject {
            path: self.path.clone(),
            replace: self.replace.clone(),
            autostart: self.autostart,
        }
    }
}

/// The screens: `DSK-01` to `DSK-03`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Screen {
    /// `DSK-01`: the code.
    Connect,
    /// `DSK-02`: a phone just connected; set up Coder.
    Connected { device: String },
    /// `DSK-03`: status, phones, and Coder.
    Home,
}

/// What a click asks for. Every one is resolved against the view that
/// showed it, then checked against the model's state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Intent {
    /// An action admitted by the Grid's current native controls.
    Grid { key: String },
    /// A local hosted-chat action; the shell checks its current conversation.
    Chat { action: crate::chat_action::Action },
    /// Navigate the desktop shell without starting work.
    Navigate { action: crate::chrome::Action },
    /// A choice on Settings ([`crate::settings`]).
    Settings { action: crate::settings::Action },
    /// A control on the Map page ([`crate::map_action`]).
    Map { action: crate::map_action::Action },
    /// A control on the Terminal pane ([`crate::terminal_action`]).
    Terminal {
        action: crate::terminal_action::Action,
    },
    /// "Can't scan? Copy a code instead".
    CopyCode,
    /// Bring the code back after it was hidden.
    ShowCode,
    /// "Choose folder…".
    ChooseFolder,
    /// Flip "Let my phone start Coder here".
    ToggleAutostart,
    /// Leave the connected screen for home.
    Done,
    /// "Connect another phone".
    ConnectAnother,
    /// Back from the code screen to home.
    Back,
    /// "Remove", before the confirmation.
    AskRemove { device: String },
    /// "Remove" in the confirmation.
    Remove { device: String },
    /// "Keep" in the confirmation.
    Keep,
    /// "Open Login Items".
    OpenLoginItems,
    /// `DSK-04`: "Connect", for the request the prompt showed.
    NearbyConnect { id: u64 },
    /// `DSK-04`: "Don't connect".
    NearbyDecline { id: u64 },
    /// "Try again", once Coder has not answered for [`STALL`].
    Retry,
}

/// A request for the shell to run.
#[derive(Clone, PartialEq, Eq)]
pub enum Request {
    Saved {
        ticket: u64,
        request: openagents_chat_app::retained::Request,
    },
    TaskChat {
        chat: String,
        ticket: u64,
        request: openagents_chat_app::task_chat::Request,
    },
    /// A Coder run on this computer for a chat
    /// ([`openagents_chat_app::coder_run`]).
    CoderRun {
        chat: String,
        ticket: u64,
        request: openagents_chat_app::coder_run::Request,
    },

    Chat {
        ticket: u64,
        command: openagents_chat::service::Command,
    },
    /// Status, devices, projects, and the auto-start policy together.
    Refresh,
    Code(Action),
    Revoke {
        device: String,
    },
    /// Make the folder at `path` the shown project
    /// ([`crate::control::pick_project`]): it replaces `replace`, and the
    /// switch follows it, on when `autostart`.
    AddProject {
        path: PathBuf,
        replace: Option<String>,
        autostart: bool,
    },
    SetAutostart(Autostart),
    /// Put `code` on the clipboard.
    Copy {
        code: String,
    },
    /// Clear the clipboard if it still holds `code`.
    ClearClipboard {
        code: String,
    },
    ChooseFolder,
    /// Whether Codex, Claude Code, and Grok Build are signed in, and the
    /// recent tasks.
    Coder,
    /// Coder's engine, model, sign-in, and usage. Read-only.
    Engine,
    OpenLoginItems,
    /// Start Coder under this app, upgrading an earlier setup silently
    /// first ([`crate::migrate::start`]). Sent once, on launch.
    Start,
    /// Answer the nearby request `id` (`DSK-04`).
    NearbyDecide {
        id: u64,
        connect: bool,
    },
}

impl std::fmt::Debug for Request {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Request::Chat { ticket, .. } => write!(f, "Chat {{ ticket: {ticket}, .. }}"),
            Request::Saved { ticket, .. } => write!(f, "Saved {{ ticket: {ticket}, .. }}"),
            Request::TaskChat { ticket, .. } => write!(f, "TaskChat {{ ticket: {ticket}, .. }}"),
            Request::CoderRun { ticket, .. } => write!(f, "CoderRun {{ ticket: {ticket}, .. }}"),
            Request::Copy { .. } => f.write_str("Copy { .. }"),
            Request::ClearClipboard { .. } => f.write_str("ClearClipboard { .. }"),
            Request::Refresh => f.write_str("Refresh"),
            Request::Code(action) => match action {
                Action::Create { ticket } => write!(f, "Code(Create {{ ticket: {ticket} }})"),
                other => write!(f, "Code({other:?})"),
            },
            Request::Revoke { device } => write!(f, "Revoke {{ {device} }}"),
            Request::AddProject {
                path,
                replace,
                autostart,
            } => write!(
                f,
                "AddProject {{ {}, replace: {replace:?}, autostart: {autostart} }}",
                path.display()
            ),
            Request::SetAutostart(policy) => write!(f, "SetAutostart({policy:?})"),
            Request::ChooseFolder => f.write_str("ChooseFolder"),
            Request::Coder => f.write_str("Coder"),
            Request::Engine => f.write_str("Engine"),
            Request::OpenLoginItems => f.write_str("OpenLoginItems"),
            Request::Start => f.write_str("Start"),
            Request::NearbyDecide { id, connect } => {
                write!(f, "NearbyDecide {{ id: {id}, connect: {connect} }}")
            }
        }
    }
}

/// One recent Coder task.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Task {
    pub title: String,
    /// The task store's status word, such as `running` or `finished`.
    pub status: String,
    /// Why a stopped task stopped, such as
    /// `Couldn't start: Claude Code isn't set up on this computer.`
    pub reason: Option<String>,
}

/// What Coder can use on this Mac.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Agents {
    pub codex: bool,
    pub claude: bool,
    /// Grok Build, allowed by default (#10091): `None` when it is not
    /// installed here, else whether it is signed in.
    pub grok: Option<bool>,
}

/// Grok Build for the person whose home is `home` ([`Agents::grok`]): `None`
/// when no `grok` is installed, else whether it has a login. The same check
/// Coder's readiness makes (`acp_client::grok`): the auth file's size and
/// whether `XAI_API_KEY` is set, never their contents.
#[must_use]
pub fn grok(home: &std::path::Path) -> Option<bool> {
    let variable = |name: &str| {
        if name == "HOME" {
            Some(home.as_os_str().to_owned())
        } else {
            std::env::var_os(name)
        }
    };
    acp_client::grok::binary(&variable)?;
    Some(acp_client::grok::signed_in(&variable))
}

/// Codex's login for the person whose home is `home`: `$CODEX_HOME/auth.json`
/// when `CODEX_HOME` is set and not empty, else `home/.codex/auth.json`.
/// The same place the engine's readiness check and its runs look
/// (`codex_transport::codex::Login::default_path`, #10083).
#[must_use]
pub fn codex_login(home: &std::path::Path) -> PathBuf {
    std::env::var_os("CODEX_HOME")
        .filter(|codex| !codex.is_empty())
        .map(PathBuf::from)
        .map_or_else(
            || home.join(".codex"),
            |codex| std::path::absolute(&codex).unwrap_or(codex),
        )
        .join("auth.json")
}

/// Whether the login agent that runs Coder is registered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Agent {
    /// Registered and allowed to run.
    Enabled,
    /// Registered, but the person must allow it in System Settings.
    NeedsApproval,
    /// Being started on launch, after upgrading an earlier setup.
    Starting,
    /// Not registered: an earlier setup still runs Coder, or this is not an
    /// app bundle.
    NotRegistered,
    /// Registration failed.
    Failed(String),
}

/// How starting Coder on launch went.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Started {
    pub agent: Agent,
    /// One quiet line when an earlier setup had to keep running.
    pub note: Option<String>,
}

/// A finished request.
#[derive(Clone, PartialEq, Eq)]
pub enum Outcome {
    Saved {
        ticket: u64,
        result: Box<
            Result<openagents_chat_app::retained::Answer, openagents_chat_app::retained::Failure>,
        >,
    },
    TaskChat {
        chat: String,
        ticket: u64,
        result: Box<crate::control::ControlResult<openagents_chat_app::task_chat::Answer>>,
    },
    CoderRun {
        chat: String,
        ticket: u64,
        result: Box<Result<openagents_chat_app::coder_run::Answer, String>>,
    },

    Chat {
        ticket: u64,
        result: Box<crate::control::ControlResult<openagents_chat::service::Snapshot>>,
    },
    /// The host's state, or `None` when it does not answer.
    Refreshed(Option<Box<Refreshed>>),
    Created {
        ticket: u64,
        invitation: String,
        code: String,
    },
    CreateFailed {
        ticket: u64,
    },
    /// A request failed; `message` is for the person.
    Failed {
        message: String,
    },
    Folder(Chosen),
    /// A [`Request::AddProject`] finished.
    Picked(Result<(), PickError>),
    Coder {
        agents: Agents,
        tasks: Vec<Task>,
    },
    /// The engine report, or why this read failed.
    Engine(crate::control::ControlResult<EngineReport>),
    Copied,
    Started(Started),
}

impl std::fmt::Debug for Outcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Outcome::Created { ticket, .. } => write!(f, "Created {{ ticket: {ticket}, .. }}"),
            Outcome::Refreshed(state) => write!(f, "Refreshed({})", state.is_some()),
            other => write!(f, "{}", outcome_name(other)),
        }
    }
}

fn outcome_name(outcome: &Outcome) -> &'static str {
    match outcome {
        Outcome::Chat { .. } => "Chat",
        Outcome::Saved { .. } => "Saved",
        Outcome::TaskChat { .. } => "TaskChat",
        Outcome::CoderRun { .. } => "CoderRun",
        Outcome::Refreshed(_) => "Refreshed",
        Outcome::Created { .. } => "Created",
        Outcome::CreateFailed { .. } => "CreateFailed",
        Outcome::Failed { .. } => "Failed",
        Outcome::Folder(_) => "Folder",
        Outcome::Picked(_) => "Picked",
        Outcome::Coder { .. } => "Coder",
        Outcome::Engine(_) => "Engine",
        Outcome::Copied => "Copied",
        Outcome::Started(_) => "Started",
    }
}

/// The host's state in one answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refreshed {
    pub status: Status,
    pub devices: Vec<Device>,
    pub projects: Vec<Project>,
    pub autostart: Autostart,
    /// A phone nearby waiting for a click (`DSK-04`).
    pub nearby: Option<NearbyPrompt>,
    /// The background watchers running on this computer, by name ("disk
    /// cleanup"), for the sidebar's line.
    pub watchers: Vec<String>,
    /// The newest background notice (`(when, line)`), for a desktop
    /// notification.
    pub background: Option<(u64, String)>,
}

/// The window's state.
#[derive(Debug)]
pub struct Model {
    pub screen: Screen,
    pub codes: Codes,
    /// The last answer from the host; `None` until one arrives or while it
    /// does not answer.
    pub host: Option<Refreshed>,
    /// Whether the host has answered since the window opened.
    pub reached: bool,
    pub agents: Agents,
    pub tasks: Vec<Task>,
    /// The latest engine report. `None` until one arrives.
    pub engine: Option<EngineReport>,
    /// Why the latest engine read failed, when the window can say.
    pub engine_note: Option<String>,
    pub agent: Agent,
    /// One quiet line about how Coder started, when it matters.
    pub note: Option<String>,
    /// The device a Remove confirmation is showing for.
    pub confirming: Option<String>,
    /// The last copy, for its line under the link.
    pub copied_at: Option<Instant>,
    /// A line for the person about the last thing that went wrong.
    pub problem: Option<String>,
    /// The folder being saved as the project, while it is.
    pub saving: Option<Saving>,
    pub visible: bool,
    pub unlocked: bool,
    /// Devices known when the code screen opened; a new one is a pairing.
    known: Option<BTreeSet<String>>,
    next_poll: Instant,
    /// When Coder's tasks and sign-ins are next read.
    next_coder: Instant,
    /// When the engine report is next read.
    next_engine: Instant,
    /// Clipboard entries to clear, and when.
    clear: Vec<(String, Instant)>,
    /// Whether [`Request::Start`] is still to be sent.
    start: bool,
    /// Whether this app starts Coder (it launched with [`Agent::Starting`]),
    /// so it may start it again when Coder stops answering. Not with
    /// `--no-login-agent`, and not against the in-process host.
    restartable: bool,
    /// Whether a [`Request::Start`] is out and not yet answered.
    starting: bool,
    /// Since when the host has not answered: launch, or the first ask it
    /// left unanswered after answering. `None` while it answers.
    unanswered_since: Option<Instant>,
    /// Coder has not answered for [`STALL`]: the screens say so plainly and
    /// offer **Try again**.
    pub stalled: bool,
    /// When the app next starts Coder again on its own while it is stalled.
    next_restart: Option<Instant>,
    /// What the screens call this computer: "Mac" on a Mac, "computer" on
    /// Linux and Windows ([`crate::words::COMPUTER`]).
    pub computer: &'static str,
}

impl Model {
    /// A model that opens on `screen`. With [`Agent::Starting`] its first
    /// tick asks to start Coder.
    pub fn new(now: Instant, screen: Screen, agent: Agent) -> Model {
        let start = agent == Agent::Starting;
        Model {
            screen,
            codes: Codes::new(now),
            host: None,
            reached: false,
            agents: Agents::default(),
            tasks: Vec::new(),
            engine: None,
            engine_note: None,
            agent,
            note: None,
            confirming: None,
            copied_at: None,
            problem: None,
            saving: None,
            visible: true,
            unlocked: true,
            known: None,
            next_poll: now,
            next_coder: now,
            next_engine: now,
            clear: Vec::new(),
            start,
            restartable: start,
            starting: false,
            unanswered_since: Some(now),
            stalled: false,
            next_restart: None,
            computer: crate::words::COMPUTER,
        }
    }

    /// The phone nearby waiting for a click, if any. It shows over every
    /// screen until it is answered, withdrawn, or expires.
    pub fn nearby(&self) -> Option<&NearbyPrompt> {
        self.host.as_ref().and_then(|host| host.nearby.as_ref())
    }

    fn conditions(&self) -> Conditions {
        Conditions {
            visible: self.visible,
            unlocked: self.unlocked,
            connect_screen: self.screen == Screen::Connect && self.host.is_some(),
        }
    }

    /// Phones that can reach this Mac.
    pub fn phones(&self) -> Vec<&Device> {
        self.host
            .as_ref()
            .map(|host| host.devices.iter().filter(|d| !d.revoked).collect())
            .unwrap_or_default()
    }

    /// The first project, which the connected screen shows.
    pub fn project(&self) -> Option<&Project> {
        self.host.as_ref().and_then(|host| host.projects.first())
    }

    /// Whether phones may start Coder here: the policy is on and names the
    /// shown project, so the switch follows the project on screen.
    pub fn autostart(&self) -> bool {
        let (Some(host), Some(project)) = (&self.host, self.project()) else {
            return false;
        };
        host.autostart.enabled && host.autostart.projects.contains(&project.label)
    }

    /// Whether a chosen folder's swap is out now.
    pub fn saving_now(&self) -> bool {
        self.saving.as_ref().is_some_and(|saving| saving.running)
    }

    /// Brings the model up to `now`.
    pub fn tick(&mut self, now: Instant) -> Vec<Request> {
        let mut requests = Vec::new();
        if std::mem::take(&mut self.start) {
            self.starting = true;
            requests.push(Request::Start);
        }
        if let Some(since) = self.unanswered_since
            && now.duration_since(since) >= STALL
        {
            self.stalled = true;
        }
        // Stalled: start Coder again now and then, never over a start
        // still running, and never past a switch only the person can flip.
        if self.stalled
            && self.restartable
            && !self.starting
            && self.agent != Agent::NeedsApproval
            && self.next_restart.is_none_or(|at| now >= at)
        {
            self.starting = true;
            self.next_restart = Some(now + RESTART);
            requests.push(Request::Start);
        }
        if now >= self.next_poll {
            requests.push(Request::Refresh);
            if self.screen != Screen::Connect && now >= self.next_coder {
                requests.push(Request::Coder);
                self.next_coder = now + CODER_POLL;
            }
            let wait = if self.screen == Screen::Connect || self.nearby().is_some() {
                FAST_POLL
            } else {
                SLOW_POLL
            };
            self.next_poll = now + wait;
        }
        // The sidebar and Settings → Coder show the engine on every screen,
        // so it is read while the window shows, whatever the screen
        // (#10105); a hidden window reads it when shown again.
        if self.visible && now >= self.next_engine {
            requests.push(Request::Engine);
            self.next_engine = now + ENGINE_POLL;
        }
        let conditions = self.conditions();
        requests.extend(
            self.codes
                .tick(now, conditions)
                .into_iter()
                .map(Request::Code),
        );
        let (due, later): (Vec<_>, Vec<_>) = self.clear.drain(..).partition(|(_, at)| *at <= now);
        self.clear = later;
        requests.extend(
            due.into_iter()
                .map(|(code, _)| Request::ClearClipboard { code }),
        );
        requests
    }

    /// When the model next needs a tick.
    pub fn next_wake(&self) -> Instant {
        let mut wake = self.next_poll;
        if let Some(since) = self.unanswered_since
            && !self.stalled
        {
            wake = wake.min(since + STALL);
        }
        if let Some(at) = self
            .next_restart
            .filter(|_| self.stalled && self.restartable)
        {
            wake = wake.min(at);
        }
        if let Some(at) = self.codes.next_wake() {
            wake = wake.min(at);
        }
        for (_, at) in &self.clear {
            wake = wake.min(*at);
        }
        if self.visible {
            wake = wake.min(self.next_engine);
        }
        wake
    }

    /// The window was shown or hidden. Shown again, it reads the engine at
    /// once.
    pub fn shown(&mut self, visible: bool, now: Instant) {
        if visible && !self.visible {
            self.read_engine(now);
        }
        self.visible = visible;
        if visible {
            self.codes.input(now);
        }
    }

    /// Read Coder's engine and usage on the next tick rather than at the
    /// next poll: the window was focused, a new chat opened, or a run was
    /// passed over or refused for capacity (#10105). The host reads a
    /// provider's usage again when its reading is due.
    pub fn read_engine(&mut self, now: Instant) {
        self.next_engine = self.next_engine.min(now);
    }

    /// The screen was locked or unlocked.
    pub fn set_locked(&mut self, locked: bool) {
        self.unlocked = !locked;
    }

    /// The person used the window.
    pub fn input(&mut self, now: Instant) {
        self.codes.input(now);
    }

    fn go(&mut self, screen: Screen, now: Instant) {
        if screen == Screen::Connect {
            self.known = None;
            self.codes.show_again(now);
            self.next_poll = now;
        }
        self.confirming = None;
        self.problem = None;
        self.screen = screen;
    }

    /// Runs a click.
    pub fn activate(&mut self, intent: Intent, now: Instant) -> Vec<Request> {
        self.codes.input(now);
        match intent {
            Intent::Grid { .. } | Intent::Chat { .. } => Vec::new(),
            Intent::Navigate { .. }
            | Intent::Settings { .. }
            | Intent::Map { .. }
            | Intent::Terminal { .. } => Vec::new(),
            Intent::CopyCode => match self.codes.shown() {
                Some(shown) => {
                    let code = shown.text.clone();
                    self.copied_at = Some(now);
                    self.clear.push((code.clone(), now + CLIPBOARD_LIFE));
                    vec![Request::Copy { code }]
                }
                None => Vec::new(),
            },
            Intent::ShowCode => {
                self.codes.show_again(now);
                self.tick(now)
            }
            // One folder at a time: the row says Saving… meanwhile.
            Intent::ChooseFolder if self.saving_now() => Vec::new(),
            Intent::ChooseFolder => vec![Request::ChooseFolder],
            Intent::ToggleAutostart => {
                if self.saving_now() {
                    return Vec::new();
                }
                let Some(host) = &self.host else {
                    return Vec::new();
                };
                let projects: Vec<String> = host.projects.iter().map(|p| p.label.clone()).collect();
                if projects.is_empty() {
                    return Vec::new();
                }
                let enabled = !self.autostart();
                // A new try replaces the last refusal; a refusal of this
                // one shows again.
                self.problem = None;
                vec![Request::SetAutostart(Autostart {
                    enabled,
                    projects,
                    max_running: host.autostart.max_running.max(1),
                })]
            }
            Intent::Done | Intent::Back | Intent::Keep => {
                if intent == Intent::Keep {
                    self.confirming = None;
                } else {
                    self.go(Screen::Home, now);
                }
                vec![Request::Refresh, Request::Coder]
            }
            Intent::ConnectAnother => {
                self.go(Screen::Connect, now);
                self.tick(now)
            }
            Intent::AskRemove { device } => {
                self.confirming = Some(device);
                Vec::new()
            }
            Intent::Remove { device } => {
                if self.confirming.as_deref() != Some(device.as_str()) {
                    return Vec::new();
                }
                self.confirming = None;
                vec![Request::Revoke { device }, Request::Refresh]
            }
            Intent::OpenLoginItems => vec![Request::OpenLoginItems],
            Intent::Retry => {
                if self.host.is_some() {
                    return Vec::new();
                }
                // Back to "Starting…" for another [`STALL`]; ask now, and
                // start Coder again unless a start is still running.
                self.stalled = false;
                self.unanswered_since = Some(now);
                self.next_poll = now;
                let mut requests = Vec::new();
                if self.restartable && !self.starting {
                    self.starting = true;
                    self.next_restart = Some(now + RESTART);
                    requests.push(Request::Start);
                }
                requests.extend(self.tick(now));
                requests
            }
            Intent::NearbyConnect { id } | Intent::NearbyDecline { id } => {
                // Only the request on screen, and only once.
                if self.nearby().map(|prompt| prompt.id) != Some(id) {
                    return Vec::new();
                }
                let connect = matches!(intent, Intent::NearbyConnect { .. });
                if let Some(host) = &mut self.host {
                    host.nearby = None;
                }
                vec![Request::NearbyDecide { id, connect }, Request::Refresh]
            }
        }
    }

    /// Applies a finished request.
    pub fn outcome(&mut self, outcome: Outcome, now: Instant) -> Vec<Request> {
        match outcome {
            Outcome::Chat { .. }
            | Outcome::TaskChat { .. }
            | Outcome::CoderRun { .. }
            | Outcome::Saved { .. } => Vec::new(),
            Outcome::Refreshed(Some(state)) => {
                let state = *state;
                self.reached = true;
                self.unanswered_since = None;
                self.stalled = false;
                self.next_restart = None;
                let live: BTreeSet<String> = state
                    .devices
                    .iter()
                    .filter(|device| !device.revoked)
                    .map(|device| device.device.clone())
                    .collect();
                let first = self.host.is_none();
                self.host = Some(state);
                let mut requests = Vec::new();
                match &self.screen {
                    Screen::Connect => match &self.known {
                        None => {
                            // The first answer on this screen. A Mac with a
                            // phone already opens on home.
                            if first && !live.is_empty() && self.codes.shown().is_none() {
                                self.go(Screen::Home, now);
                                requests.push(Request::Coder);
                            } else {
                                self.known = Some(live);
                            }
                        }
                        Some(known) => {
                            if let Some(device) = live.difference(known).next().cloned() {
                                requests.extend(self.codes.paired().into_iter().map(Request::Code));
                                self.go(Screen::Connected { device }, now);
                                requests.push(Request::Coder);
                            }
                        }
                    },
                    Screen::Connected { device } if !live.contains(device) => {
                        self.go(Screen::Home, now);
                    }
                    _ => {}
                }
                // A swap that failed partway: the host answers again, so
                // finish it.
                if let Some(saving) = &mut self.saving
                    && !saving.running
                {
                    saving.running = true;
                    saving.tries += 1;
                    requests.push(saving.request());
                }
                requests.extend(self.tick(now));
                requests
            }
            Outcome::Refreshed(None) => {
                self.host = None;
                self.unanswered_since.get_or_insert(now);
                self.tick(now)
            }
            Outcome::Created {
                ticket,
                invitation,
                code,
            } => self
                .codes
                .created(ticket, invitation, code, now)
                .into_iter()
                .map(Request::Code)
                .collect(),
            Outcome::CreateFailed { ticket } => {
                self.codes.failed(ticket, now);
                Vec::new()
            }
            Outcome::Failed { message } => {
                self.problem = Some(message);
                Vec::new()
            }
            Outcome::Folder(Chosen::Cancelled) => Vec::new(),
            Outcome::Folder(Chosen::Unavailable) => {
                self.problem = Some(NO_CHOOSER.into());
                Vec::new()
            }
            Outcome::Folder(Chosen::Folder(_)) if self.saving_now() => Vec::new(),
            Outcome::Folder(Chosen::Folder(path)) => {
                if !path.join(".git").exists() {
                    self.problem = Some(
                        "That folder isn't a Git project. Choose the folder that holds your code."
                            .into(),
                    );
                    return Vec::new();
                }
                self.problem = None;
                // The chosen folder replaces the one project shown, and the
                // switch follows it; picking the first project turns it on.
                let replace = self
                    .host
                    .as_ref()
                    .filter(|host| host.projects.len() == 1)
                    .and_then(|host| host.projects.first())
                    .map(|project| project.label.clone());
                let autostart = self.autostart() || self.project().is_none();
                let saving = Saving {
                    path,
                    replace,
                    autostart,
                    running: true,
                    tries: 1,
                };
                let request = saving.request();
                self.saving = Some(saving);
                vec![request]
            }
            Outcome::Picked(result) => {
                if let Some(saving) = &mut self.saving {
                    saving.running = false;
                    match result {
                        Ok(()) => {
                            self.saving = None;
                            self.problem = None;
                        }
                        Err(PickError::Folder) => {
                            self.saving = None;
                            self.problem = Some(FOLDER_REFUSED.into());
                        }
                        // The host did not come back in time, or refused a
                        // step: say so, and finish on its next answer.
                        Err(PickError::Setting) => {
                            if saving.tries >= SAVE_TRIES {
                                self.saving = None;
                            }
                            self.problem = Some(SETTING_FAILED.into());
                        }
                    }
                }
                self.next_poll = now;
                vec![Request::Refresh]
            }
            Outcome::Coder { agents, tasks } => {
                self.agents = agents;
                self.tasks = tasks;
                Vec::new()
            }
            Outcome::Engine(Ok(report)) => {
                if report.refresh_due {
                    self.next_engine = now + ENGINE_WAIT;
                }
                self.engine = Some(report);
                self.engine_note = None;
                Vec::new()
            }
            Outcome::Engine(Err(ControlError::Unreachable)) => Vec::new(),
            Outcome::Engine(Err(ControlError::Refused { message, .. })) => {
                self.engine_note = Some(message);
                Vec::new()
            }
            Outcome::Engine(Err(ControlError::Malformed)) => {
                self.engine_note = Some("Coder's engine report was unreadable.".into());
                Vec::new()
            }
            Outcome::Copied => Vec::new(),
            Outcome::Started(started) => {
                self.starting = false;
                self.agent = started.agent;
                self.note = started.note;
                self.next_poll = now;
                vec![Request::Refresh]
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::control::HostControl;
    use crate::fake::FakeHost;

    /// Runs the model's requests against a fake host, the way the shell
    /// does, until none are left.
    pub(crate) struct Rig {
        pub model: Model,
        pub host: FakeHost,
        pub start: Instant,
        pub now: Instant,
        pub clipboard: Option<String>,
        pub folder: Chosen,
        /// How many times the model asked to start Coder.
        pub starts: usize,
        /// How long a call waits for the host starting again.
        pub patience: crate::control::Patience,
    }

    impl Rig {
        pub fn new() -> Rig {
            let start = Instant::now();
            Rig {
                model: Model::new(start, Screen::Connect, Agent::Enabled),
                host: FakeHost::new("Studio Mac", 1_790_000_000),
                start,
                now: start,
                clipboard: None,
                folder: Chosen::Cancelled,
                starts: 0,
                patience: crate::control::Patience {
                    wait: Duration::from_millis(200),
                    every: Duration::from_millis(1),
                },
            }
        }

        pub fn at(&mut self, seconds: u64) {
            self.now = self.start + Duration::from_secs(seconds);
            self.host.set_now(1_790_000_000 + seconds);
        }

        pub fn run(&mut self, requests: Vec<Request>) {
            let mut queue: std::collections::VecDeque<Request> = requests.into();
            while let Some(request) = queue.pop_front() {
                let outcome = match request {
                    Request::Chat { .. }
                    | Request::TaskChat { .. }
                    | Request::CoderRun { .. }
                    | Request::Saved { .. } => {
                        panic!("computer model does not dispatch chat")
                    }
                    Request::Refresh => {
                        let mut host = self.host.clone();
                        let state = host.status().ok().map(|status| {
                            Box::new(Refreshed {
                                status,
                                devices: host.devices().unwrap_or_default(),
                                projects: host.projects().unwrap_or_default(),
                                autostart: host.autostart().expect("a policy"),
                                nearby: host.nearby_pending().unwrap_or_default(),
                                watchers: Vec::new(),
                                background: None,
                            })
                        });
                        Some(Outcome::Refreshed(state))
                    }
                    Request::Code(Action::Create { ticket }) => Some(match self.host.invite() {
                        Ok(invite) => Outcome::Created {
                            ticket,
                            invitation: invite.invitation,
                            code: invite.code,
                        },
                        Err(_) => Outcome::CreateFailed { ticket },
                    }),
                    Request::Code(Action::Cancel { invitation }) => {
                        let _ = self.host.cancel(&invitation);
                        None
                    }
                    Request::Code(Action::CancelAll) => {
                        let _ = self.host.cancel_all();
                        None
                    }
                    Request::Revoke { device } => {
                        let _ = self.host.revoke(&device);
                        None
                    }
                    Request::AddProject {
                        path,
                        replace,
                        autostart,
                    } => Some(Outcome::Picked(crate::control::pick_project_with(
                        &mut self.host,
                        &path.to_string_lossy(),
                        replace.as_deref(),
                        autostart,
                        self.patience,
                    ))),
                    Request::SetAutostart(policy) => {
                        crate::control::set_autostart(&mut self.host, policy, self.patience)
                            .err()
                            .map(|_| Outcome::Failed {
                                message: SETTING_FAILED.into(),
                            })
                    }
                    Request::Copy { code } => {
                        self.clipboard = Some(code);
                        Some(Outcome::Copied)
                    }
                    Request::ClearClipboard { code } => {
                        if self.clipboard.as_deref() == Some(code.as_str()) {
                            self.clipboard = None;
                        }
                        None
                    }
                    Request::ChooseFolder => Some(Outcome::Folder(self.folder.clone())),
                    Request::Coder => Some(Outcome::Coder {
                        agents: Agents {
                            codex: true,
                            claude: false,
                            grok: None,
                        },
                        tasks: vec![Task {
                            title: "Fix the login test".into(),
                            status: "running".into(),
                            reason: None,
                        }],
                    }),
                    Request::Engine => Some(Outcome::Engine(Err(ControlError::Unreachable))),
                    Request::OpenLoginItems => None,
                    Request::Start => {
                        self.starts += 1;
                        Some(Outcome::Started(Started {
                            agent: Agent::Enabled,
                            note: None,
                        }))
                    }
                    Request::NearbyDecide { id, connect } => {
                        let _ = self.host.nearby_decide(id, connect);
                        None
                    }
                };
                if let Some(outcome) = outcome {
                    queue.extend(self.model.outcome(outcome, self.now));
                }
            }
        }

        pub fn tick(&mut self, seconds: u64) {
            self.at(seconds);
            let requests = self.model.tick(self.now);
            self.run(requests);
        }

        pub fn click(&mut self, intent: Intent) {
            let requests = self.model.activate(intent, self.now);
            self.run(requests);
        }
    }

    #[test]
    fn a_scan_moves_the_window_to_connected_and_cancels_the_codes() {
        let mut rig = Rig::new();
        rig.tick(0);
        rig.tick(1);
        let code = rig.model.codes.shown().expect("a code").invitation.clone();
        rig.host.redeem(&code, "Kai's iPhone").expect("the scan");
        rig.tick(3);
        let Screen::Connected { device } = &rig.model.screen else {
            panic!("still on {:?}", rig.model.screen);
        };
        assert_eq!(
            rig.model.phones().first().map(|d| d.device.as_str()),
            Some(device.as_str())
        );
        assert!(rig.host.open().is_empty());
        assert!(rig.model.codes.shown().is_none());
    }

    /// Launch starts Coder once, with no question, and goes on to the
    /// normal screens.
    #[test]
    fn launch_starts_coder_once_and_asks_nothing() {
        let start = Instant::now();
        let mut model = Model::new(start, Screen::Connect, Agent::Starting);
        let first = model.tick(start);
        assert_eq!(first.first(), Some(&Request::Start));
        assert!(!model.tick(start).contains(&Request::Start));
        let requests = model.outcome(
            Outcome::Started(Started {
                agent: Agent::NotRegistered,
                note: Some(crate::migrate::KEPT_RUNNING.into()),
            }),
            start,
        );
        assert_eq!(requests, [Request::Refresh]);
        assert_eq!(model.screen, Screen::Connect);
        assert_eq!(model.note.as_deref(), Some(crate::migrate::KEPT_RUNNING));
        let mut rig = Rig::new();
        rig.model = Model::new(rig.start, Screen::Connect, Agent::Starting);
        rig.tick(0);
        assert_eq!(rig.model.agent, Agent::Enabled);
        assert!(rig.model.codes.shown().is_some());
    }

    #[test]
    fn the_window_reads_the_engine_and_cannot_change_it() {
        use crate::control::EngineReport;
        let start = Instant::now();
        let mut home = Model::new(start, Screen::Home, Agent::Enabled);
        let requests = home.tick(start);
        assert!(requests.contains(&Request::Engine));
        assert!(
            !requests
                .iter()
                .any(|request| matches!(request, Request::SetAutostart(_)))
        );
        // Every screen shows the engine in the sidebar, so the code screen
        // reads it too; a hidden window does not, until it is shown.
        let mut connect = Model::new(start, Screen::Connect, Agent::Enabled);
        assert!(connect.tick(start).contains(&Request::Engine));
        let mut hidden = Model::new(start, Screen::Home, Agent::Enabled);
        hidden.visible = false;
        assert!(!hidden.tick(start).contains(&Request::Engine));
        hidden.shown(true, start + Duration::from_secs(1));
        assert!(
            hidden
                .tick(start + Duration::from_secs(1))
                .contains(&Request::Engine)
        );
        let report = EngineReport {
            enabled: true,
            adapter: String::new(),
            model: "gpt-6-luna".into(),
            routes: vec![],
            accounts: vec![],
            usage_probe: None,
            refresh_due: true,
        };
        let followed = home.outcome(Outcome::Engine(Ok(report.clone())), start);
        assert!(followed.is_empty());
        assert_eq!(home.engine.as_ref(), Some(&report));
        assert!(home.engine_note.is_none());
        assert!(home.next_wake() <= start + ENGINE_WAIT);
        home.outcome(
            Outcome::Engine(Err(ControlError::Unreachable)),
            start + ENGINE_WAIT,
        );
        assert_eq!(home.engine.as_ref(), Some(&report));
        assert!(home.engine_note.is_none());
        home.outcome(
            Outcome::Engine(Err(ControlError::Refused {
                code: "unavailable".into(),
                message: "This computer cannot read Coder's engine.".into(),
            })),
            start,
        );
        assert_eq!(
            home.engine_note.as_deref(),
            Some("This computer cannot read Coder's engine.")
        );
        assert_eq!(home.engine.as_ref(), Some(&report));
    }

    /// A host that never answers: "Starting…" for [`STALL`], then a plain
    /// line and **Try again**, and the app starts Coder again on its own
    /// every [`RESTART`] while polling goes on. It never waits silently.
    #[test]
    fn a_host_that_never_answers_says_so_and_starts_coder_again() {
        let mut rig = Rig::new();
        rig.model = Model::new(rig.start, Screen::Home, Agent::Starting);
        rig.host.set_down(true);
        rig.tick(0);
        assert_eq!(rig.starts, 1, "launch starts Coder once");
        for second in (5..20).step_by(5) {
            rig.tick(second);
            assert!(!rig.model.stalled, "stalled at {second}s");
        }
        assert!(rig.model.next_wake() <= rig.start + STALL);
        rig.tick(20);
        assert!(rig.model.stalled);
        assert_eq!(rig.starts, 2, "a stall starts Coder again");
        rig.tick(40);
        rig.tick(75);
        assert_eq!(rig.starts, 2, "at most one start a minute");
        rig.tick(80);
        assert_eq!(rig.starts, 3);
        // Coder answers: the stall is over and nothing starts again.
        rig.host.set_down(false);
        rig.tick(85);
        assert!(!rig.model.stalled);
        assert!(rig.model.host.is_some());
        rig.tick(200);
        assert_eq!(rig.starts, 3);
        // It stops answering later: "stopped answering" first, then the
        // stall again after another STALL.
        rig.host.set_down(true);
        rig.tick(205);
        assert!(rig.model.host.is_none() && !rig.model.stalled);
        rig.tick(225);
        assert!(rig.model.stalled);
        assert_eq!(rig.starts, 4);
    }

    #[test]
    fn try_again_asks_now_and_starts_coder_again() {
        let mut rig = Rig::new();
        rig.model = Model::new(rig.start, Screen::Home, Agent::Starting);
        rig.host.set_down(true);
        for second in [0, 5, 10, 15, 20] {
            rig.tick(second);
        }
        assert!(rig.model.stalled);
        let starts = rig.starts;
        rig.at(30);
        rig.host.set_down(false);
        rig.click(Intent::Retry);
        assert_eq!(rig.starts, starts + 1);
        assert!(!rig.model.stalled);
        assert!(rig.model.host.is_some(), "Try again asks at once");
        // With an answer, Try again does nothing.
        rig.click(Intent::Retry);
        assert_eq!(rig.starts, starts + 1);
    }

    /// Without the login agent (`--no-login-agent`, the in-process host) or
    /// while the person must allow it, a stall says so but starts nothing.
    #[test]
    fn a_stall_starts_nothing_the_app_does_not_manage() {
        for (agent, restart) in [
            (Agent::Enabled, false),
            (Agent::NotRegistered, false),
            (Agent::Starting, true),
        ] {
            let mut rig = Rig::new();
            rig.model = Model::new(rig.start, Screen::Home, agent.clone());
            rig.host.set_down(true);
            rig.tick(0);
            if restart {
                // The start reported that the person must allow it.
                rig.model.agent = Agent::NeedsApproval;
            }
            let starts = rig.starts;
            for second in (5..=120).step_by(5) {
                rig.tick(second);
            }
            assert!(rig.model.stalled, "{agent:?}");
            assert_eq!(rig.starts, starts, "{agent:?}");
        }
    }

    /// The in-process host answers for as long as the window runs: two
    /// hours of polls, code rotations, idle holds, a hidden window, and a
    /// locked screen, and the window always has its answer and never says
    /// Coder is starting or not answering.
    #[test]
    fn the_in_process_host_keeps_answering_for_hours() {
        let mut rig = Rig::new();
        rig.model = Model::new(rig.start, Screen::Home, Agent::Enabled);
        rig.tick(0);
        rig.click(Intent::ConnectAnother);
        let mut second = 0;
        while second < 2 * 60 * 60 {
            second += 1;
            match second % 1_800 {
                600 => rig.model.shown(false, rig.now),
                660 => rig.model.shown(true, rig.now),
                900 => rig.model.set_locked(true),
                960 => rig.model.set_locked(false),
                1_200 => rig.click(Intent::Back),
                1_210 => rig.click(Intent::ConnectAnother),
                _ => {}
            }
            rig.tick(second);
            assert!(rig.model.host.is_some(), "no answer at {second}s");
            assert!(!rig.model.stalled, "stalled at {second}s");
            let words = crate::screens::words(&crate::screens::root(&rig.model, 0));
            for text in words {
                assert!(
                    !text.contains("answering") && !text.contains("Starting Coder"),
                    "{text:?} at {second}s"
                );
            }
        }
        assert_eq!(rig.starts, 0, "nothing starts Coder against the fake");
    }

    #[test]
    fn a_mac_with_a_phone_opens_on_home() {
        let mut rig = Rig::new();
        let invite = rig.host.clone().invite().expect("an invite");
        rig.host
            .redeem(&invite.invitation, "Kai's iPhone")
            .expect("a scan");
        rig.tick(0);
        assert_eq!(rig.model.screen, Screen::Home);
        assert!(rig.host.open().is_empty());
    }

    #[test]
    fn picking_a_project_turns_on_starting_coder() {
        let mut rig = Rig::new();
        rig.tick(0);
        rig.tick(1);
        let code = rig.model.codes.shown().expect("a code").invitation.clone();
        rig.host.redeem(&code, "Kai's iPhone").expect("the scan");
        rig.tick(3);
        assert!(!rig.model.autostart());
        // A folder that is not a Git checkout is refused with a reason.
        let plain = tempfile::tempdir().expect("a folder");
        rig.folder = Chosen::Folder(plain.path().to_path_buf());
        rig.click(Intent::ChooseFolder);
        assert!(rig.model.problem.is_some());
        assert!(rig.model.project().is_none());
        let repo = tempfile::tempdir().expect("a folder");
        std::fs::create_dir(repo.path().join(".git")).expect("a .git");
        rig.folder = Chosen::Folder(repo.path().to_path_buf());
        rig.click(Intent::ChooseFolder);
        assert!(rig.model.problem.is_none());
        assert!(rig.model.project().is_some());
        assert!(rig.model.autostart());
        rig.click(Intent::ToggleAutostart);
        rig.click(Intent::Done);
        assert!(!rig.model.autostart());
        assert_eq!(rig.model.screen, Screen::Home);
    }

    /// A Git checkout named `name` in a fresh folder.
    fn checkout(name: &str) -> (tempfile::TempDir, PathBuf) {
        let parent = tempfile::tempdir().expect("a folder");
        let path = parent.path().join(name);
        std::fs::create_dir_all(path.join(".git")).expect("a .git");
        (parent, path)
    }

    /// A rig with a phone connected, on the connected screen.
    fn connected_rig() -> Rig {
        let mut rig = Rig::new();
        rig.tick(0);
        rig.tick(1);
        let code = rig.model.codes.shown().expect("a code").invitation.clone();
        rig.host.redeem(&code, "Kai's iPhone").expect("the scan");
        rig.tick(3);
        rig
    }

    /// Choosing another folder replaces the shown project, and the switch
    /// follows it, even when the host labels the new one `NAME-2` because
    /// the old one held the name.
    #[test]
    fn choosing_another_folder_replaces_the_project_and_the_switch_follows_it() {
        let mut rig = connected_rig();
        let (_first_parent, first) = checkout("openagents");
        rig.folder = Chosen::Folder(first);
        rig.click(Intent::ChooseFolder);
        assert_eq!(
            rig.model.project().map(|p| p.label.as_str()),
            Some("openagents")
        );
        assert!(rig.model.autostart());

        let (_second_parent, second) = checkout("openagents");
        rig.folder = Chosen::Folder(second.clone());
        rig.click(Intent::ChooseFolder);
        assert!(rig.model.problem.is_none());
        let projects = rig.host.clone().projects().expect("projects");
        assert_eq!(projects.len(), 1, "{projects:?}");
        assert_eq!(projects[0].label, "openagents-2");
        assert_eq!(
            projects[0].folder.as_deref(),
            Some(second.to_string_lossy().as_ref())
        );
        let policy = rig.host.clone().autostart().expect("a policy");
        assert!(policy.enabled);
        assert_eq!(policy.projects, ["openagents-2"]);
        assert!(rig.model.autostart());

        // With the switch off, the new project comes in off.
        rig.click(Intent::ToggleAutostart);
        rig.tick(20);
        assert!(!rig.model.autostart());
        let (_third_parent, third) = checkout("website");
        rig.folder = Chosen::Folder(third);
        rig.click(Intent::ChooseFolder);
        assert_eq!(
            rig.model.project().map(|p| p.label.as_str()),
            Some("website")
        );
        assert!(!rig.model.autostart());
        assert!(!rig.host.clone().autostart().expect("a policy").enabled);
        rig.click(Intent::ToggleAutostart);
        rig.tick(40);
        assert!(rig.model.autostart());
        assert_eq!(
            rig.host.clone().autostart().expect("a policy").projects,
            ["website"]
        );
    }

    /// The host starts again after every project change (`coder host
    /// serve` re-execs to serve it), so each call after one goes
    /// unanswered for a moment. The chosen folder shows at once with
    /// Saving…, the window waits for the host, and the swap finishes with
    /// no problem shown: one project, and the switch on for its label.
    #[test]
    fn a_host_that_starts_again_after_each_change_still_swaps_the_project() {
        let mut rig = connected_rig();
        rig.host.set_restart_calls(5);
        let (_first_parent, first) = checkout("openagents");
        rig.folder = Chosen::Folder(first);
        rig.click(Intent::ChooseFolder);
        assert_eq!(
            rig.model.project().map(|p| p.label.as_str()),
            Some("openagents")
        );
        assert!(rig.model.autostart());

        let (_second_parent, second) = checkout("omarchy");
        let requests = rig
            .model
            .outcome(Outcome::Folder(Chosen::Folder(second.clone())), rig.now);
        assert!(rig.model.saving_now());
        let words = crate::screens::words(&crate::screens::root(&rig.model, 0));
        assert!(words.iter().any(|w| w == "Saving…"), "{words:?}");
        assert!(
            words.iter().any(|w| *w == second.display().to_string()),
            "{words:?}"
        );
        // One folder at a time.
        assert!(rig.model.activate(Intent::ChooseFolder, rig.now).is_empty());
        rig.run(requests);
        assert_eq!(rig.model.saving, None);
        assert_eq!(rig.model.problem, None);
        let projects = rig.host.clone().projects().expect("projects");
        assert_eq!(projects.len(), 1, "{projects:?}");
        assert_eq!(projects[0].label, "omarchy");
        let policy = rig.host.clone().autostart().expect("a policy");
        assert!(policy.enabled);
        assert_eq!(policy.projects, ["omarchy"]);
        assert!(rig.model.autostart());
    }

    /// The owner's case: the folder went in, then the host stayed away
    /// longer than the window waits, so the old project and the switch did
    /// not follow. The window says so, and the next answer from the host
    /// finishes the swap: one project, the switch carried over to the label
    /// the host gave the new one, and no dead label.
    #[test]
    fn a_swap_that_fails_partway_finishes_on_the_next_refresh() {
        let mut rig = connected_rig();
        let (_first_parent, first) = checkout("openagents");
        rig.folder = Chosen::Folder(first);
        rig.click(Intent::ChooseFolder);
        assert!(rig.model.autostart());

        // Away for far longer than the rig's patience after the add.
        rig.host.set_restart_calls(1_000_000);
        let (_second_parent, second) = checkout("openagents");
        rig.folder = Chosen::Folder(second.clone());
        rig.click(Intent::ChooseFolder);
        assert_eq!(rig.model.problem.as_deref(), Some(SETTING_FAILED));
        let saving = rig.model.saving.clone().expect("still saving");
        assert!(!saving.running);
        assert_eq!(saving.replace.as_deref(), Some("openagents"));
        assert!(saving.autostart);
        rig.host.set_restart_calls(0);
        let mut host = rig.host.clone();
        assert_eq!(host.projects().expect("projects").len(), 2);
        assert_eq!(host.autostart().expect("a policy").projects, ["openagents"]);

        // The host answers again: the swap finishes.
        rig.tick(10);
        assert_eq!(rig.model.saving, None);
        assert_eq!(rig.model.problem, None);
        let projects = host.projects().expect("projects");
        assert_eq!(projects.len(), 1, "{projects:?}");
        assert_eq!(projects[0].label, "openagents-2");
        assert_eq!(
            projects[0].folder.as_deref(),
            Some(second.to_string_lossy().as_ref())
        );
        let policy = host.autostart().expect("a policy");
        assert!(policy.enabled);
        assert_eq!(policy.projects, ["openagents-2"]);
        assert!(rig.model.autostart());
        // Nothing more to send.
        rig.tick(20);
        assert_eq!(host.projects().expect("projects").len(), 1);
    }

    /// A host that never takes the folder: the window tries the swap
    /// [`SAVE_TRIES`] times in all, once an answer from the host, then
    /// stops saying Saving… and leaves the reason.
    #[test]
    fn a_swap_is_given_up_after_its_tries() {
        let mut rig = connected_rig();
        let (_parent, folder) = checkout("website");
        rig.host.set_down(true);
        rig.folder = Chosen::Folder(folder);
        rig.click(Intent::ChooseFolder);
        assert_eq!(rig.model.problem.as_deref(), Some(SETTING_FAILED));
        let answer = || {
            let mut host = FakeHost::default();
            Outcome::Refreshed(Some(Box::new(Refreshed {
                status: host.status().expect("a status"),
                devices: vec![],
                projects: vec![],
                autostart: host.autostart().expect("a policy"),
                nearby: None,
                watchers: Vec::new(),
                background: None,
            })))
        };
        for tries in 2..=SAVE_TRIES {
            let requests = rig.model.outcome(answer(), rig.now);
            assert!(
                requests
                    .iter()
                    .any(|r| matches!(r, Request::AddProject { .. })),
                "{requests:?}"
            );
            assert_eq!(rig.model.saving.as_ref().map(|s| s.tries), Some(tries));
            let _ = rig
                .model
                .outcome(Outcome::Picked(Err(PickError::Setting)), rig.now);
        }
        assert_eq!(rig.model.saving, None);
        assert_eq!(rig.model.problem.as_deref(), Some(SETTING_FAILED));
        let requests = rig.model.outcome(answer(), rig.now);
        assert!(
            !requests
                .iter()
                .any(|r| matches!(r, Request::AddProject { .. }))
        );
    }

    /// A policy that names only a project the host no longer has shows as
    /// off, not as on for a project whose tasks never start.
    #[test]
    fn a_policy_for_a_project_that_is_gone_shows_off() {
        let mut rig = connected_rig();
        let (_parent, path) = checkout("openagents");
        rig.folder = Chosen::Folder(path);
        rig.click(Intent::ChooseFolder);
        assert!(rig.model.autostart());
        let host = rig.model.host.as_mut().expect("a host");
        host.autostart.projects = vec!["gone".into()];
        assert!(host.autostart.enabled);
        assert!(!rig.model.autostart());
    }

    /// With no folder chooser on the computer, the button says so instead
    /// of doing nothing.
    #[test]
    fn with_no_folder_chooser_the_screen_says_so() {
        let mut rig = connected_rig();
        rig.folder = Chosen::Unavailable;
        rig.click(Intent::ChooseFolder);
        assert_eq!(rig.model.problem.as_deref(), Some(NO_CHOOSER));
        assert!(rig.model.project().is_none());
        // Cancelling says nothing.
        rig.model.problem = None;
        rig.folder = Chosen::Cancelled;
        rig.click(Intent::ChooseFolder);
        assert!(rig.model.problem.is_none());
    }

    #[test]
    fn remove_asks_first_then_revokes() {
        let mut rig = Rig::new();
        let invite = rig.host.clone().invite().expect("an invite");
        let phone = rig
            .host
            .redeem(&invite.invitation, "Kai's iPhone")
            .expect("a scan");
        rig.tick(0);
        // A Remove that was not asked for first does nothing.
        rig.click(Intent::Remove {
            device: phone.device.clone(),
        });
        assert_eq!(rig.model.phones().len(), 1);
        rig.click(Intent::AskRemove {
            device: phone.device.clone(),
        });
        rig.click(Intent::Remove {
            device: phone.device.clone(),
        });
        assert!(rig.model.phones().is_empty());
    }

    #[test]
    fn a_copied_code_leaves_the_clipboard_after_a_minute() {
        let mut rig = Rig::new();
        rig.tick(0);
        rig.tick(1);
        rig.click(Intent::CopyCode);
        let code = rig.model.codes.shown().expect("a code").text.clone();
        assert_eq!(rig.clipboard.as_deref(), Some(code.as_str()));
        rig.model.input(rig.now);
        rig.tick(60);
        assert!(rig.clipboard.is_some());
        rig.tick(61);
        assert!(rig.clipboard.is_none());
    }

    #[test]
    fn no_code_is_made_while_the_host_does_not_answer() {
        let mut rig = Rig::new();
        rig.host.set_down(true);
        rig.tick(0);
        rig.tick(5);
        assert!(rig.host.issued().is_empty());
        assert!(rig.model.codes.shown().is_none());
        rig.host.set_down(false);
        rig.tick(10);
        assert!(rig.model.codes.shown().is_some());
    }

    #[test]
    fn a_nearby_phone_is_connected_only_by_the_click_on_its_prompt() {
        let mut rig = Rig::new();
        rig.tick(0);
        let id = rig.host.ask_nearby("Kai's iPhone", "482913");
        rig.tick(5);
        assert_eq!(rig.model.nearby().map(|p| p.code.as_str()), Some("482913"));
        // A click for another request does nothing.
        rig.click(Intent::NearbyConnect { id: id + 1 });
        assert!(rig.host.nearby_answers().is_empty());
        rig.click(Intent::NearbyConnect { id });
        assert_eq!(rig.host.nearby_answers(), vec![(id, true)]);
        assert!(rig.model.nearby().is_none());
        // A second click on the same prompt sends nothing more.
        rig.click(Intent::NearbyConnect { id });
        assert_eq!(rig.host.nearby_answers().len(), 1);
    }

    #[test]
    fn dont_connect_answers_no_and_a_new_request_takes_its_place() {
        let mut rig = Rig::new();
        rig.tick(0);
        let first = rig.host.ask_nearby("Kai's iPhone", "111111");
        rig.tick(5);
        rig.click(Intent::NearbyDecline { id: first });
        assert_eq!(rig.host.nearby_answers(), vec![(first, false)]);
        let second = rig.host.ask_nearby("Kai's iPad", "222222");
        rig.tick(10);
        assert_eq!(rig.model.nearby().map(|p| p.id), Some(second));
    }
}
