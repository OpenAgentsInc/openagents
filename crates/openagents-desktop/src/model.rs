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
use crate::control::{Autostart, Device, NearbyPrompt, Project, Status};
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
/// How long a copied code stays on the clipboard.
pub const CLIPBOARD_LIFE: Duration = Duration::from_secs(60);

/// The screens: `DSK-01` to `DSK-03`, and the adoption question.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Screen {
    /// `DSK-01`: the code.
    Connect,
    /// `DSK-02`: a phone just connected; set up Coder.
    Connected { device: String },
    /// `DSK-03`: status, phones, and Coder.
    Home,
    /// A Mac set up the old way: use that setup?
    Adopt,
}

/// What a click asks for. Every one is resolved against the view that
/// showed it, then checked against the model's state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Intent {
    /// Flip "Let this phone open a terminal on this Mac".
    ToggleTerminal,
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
    /// "Use it" on the adoption question.
    Adopt,
    /// "Not now" on the adoption question.
    NotNow,
    /// "Open Login Items".
    OpenLoginItems,
    /// `DSK-04`: flip "Let this phone open a terminal on this Mac".
    NearbyTerminal,
    /// `DSK-04`: "Connect", for the request the prompt showed.
    NearbyConnect { id: u64 },
    /// `DSK-04`: "Don't connect".
    NearbyDecline { id: u64 },
}

/// A request for the shell to run.
#[derive(Clone, PartialEq, Eq)]
pub enum Request {
    /// Status, devices, projects, and the auto-start policy together.
    Refresh,
    Code(Action),
    Revoke {
        device: String,
    },
    AddProject {
        path: PathBuf,
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
    /// Whether Codex and Claude Code are signed in, and the recent tasks.
    Coder,
    OpenLoginItems,
    /// Run `coder host adopt`.
    Adopt,
    /// Answer the nearby request `id` (`DSK-04`).
    NearbyDecide {
        id: u64,
        connect: bool,
        terminal: bool,
    },
}

impl std::fmt::Debug for Request {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Request::Copy { .. } => f.write_str("Copy { .. }"),
            Request::ClearClipboard { .. } => f.write_str("ClearClipboard { .. }"),
            Request::Refresh => f.write_str("Refresh"),
            Request::Code(action) => match action {
                Action::Create { ticket, terminal } => {
                    write!(
                        f,
                        "Code(Create {{ ticket: {ticket}, terminal: {terminal} }})"
                    )
                }
                other => write!(f, "Code({other:?})"),
            },
            Request::Revoke { device } => write!(f, "Revoke {{ {device} }}"),
            Request::AddProject { path } => write!(f, "AddProject {{ {} }}", path.display()),
            Request::SetAutostart(policy) => write!(f, "SetAutostart({policy:?})"),
            Request::ChooseFolder => f.write_str("ChooseFolder"),
            Request::Coder => f.write_str("Coder"),
            Request::OpenLoginItems => f.write_str("OpenLoginItems"),
            Request::Adopt => f.write_str("Adopt"),
            Request::NearbyDecide {
                id,
                connect,
                terminal,
            } => write!(
                f,
                "NearbyDecide {{ id: {id}, connect: {connect}, terminal: {terminal} }}"
            ),
        }
    }
}

/// One recent Coder task.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Task {
    pub title: String,
    /// The task store's status word, such as `running` or `finished`.
    pub status: String,
}

/// What Coder can use on this Mac.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Agents {
    pub codex: bool,
    pub claude: bool,
}

/// Whether the login agent that runs Coder is registered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Agent {
    /// Registered and allowed to run.
    Enabled,
    /// Registered, but the person must allow it in System Settings.
    NeedsApproval,
    /// Not registered: an earlier setup still runs Coder, or this is not an
    /// app bundle.
    NotRegistered,
    /// Registration failed.
    Failed(String),
}

/// What an old-style setup keeps, as the adoption question says it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OldSetup {
    /// Phones that can reach this Mac now; `None` when this app's Coder
    /// can't count them (it doesn't read the keychain, or gave no answer).
    pub phones: Option<usize>,
    /// Whether this app's Coder can take it over yet.
    pub ready: bool,
}

/// A finished request.
#[derive(Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The host's state, or `None` when it does not answer.
    Refreshed(Option<Box<Refreshed>>),
    Created {
        ticket: u64,
        invitation: String,
        code: String,
        terminal: bool,
    },
    CreateFailed {
        ticket: u64,
    },
    /// A request failed; `message` is for the person.
    Failed {
        message: String,
    },
    Folder(Option<PathBuf>),
    Coder {
        agents: Agents,
        tasks: Vec<Task>,
    },
    Copied,
    Adopted(Result<(), String>),
}

impl std::fmt::Debug for Outcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Outcome::Created {
                ticket, terminal, ..
            } => write!(
                f,
                "Created {{ ticket: {ticket}, terminal: {terminal}, .. }}"
            ),
            Outcome::Refreshed(state) => write!(f, "Refreshed({})", state.is_some()),
            other => write!(f, "{}", outcome_name(other)),
        }
    }
}

fn outcome_name(outcome: &Outcome) -> &'static str {
    match outcome {
        Outcome::Refreshed(_) => "Refreshed",
        Outcome::Created { .. } => "Created",
        Outcome::CreateFailed { .. } => "CreateFailed",
        Outcome::Failed { .. } => "Failed",
        Outcome::Folder(_) => "Folder",
        Outcome::Coder { .. } => "Coder",
        Outcome::Copied => "Copied",
        Outcome::Adopted(_) => "Adopted",
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
    pub agent: Agent,
    pub old: Option<OldSetup>,
    /// The device a Remove confirmation is showing for.
    pub confirming: Option<String>,
    /// The last copy, for its line under the link.
    pub copied_at: Option<Instant>,
    /// A line for the person about the last thing that went wrong.
    pub problem: Option<String>,
    pub visible: bool,
    pub unlocked: bool,
    /// Devices known when the code screen opened; a new one is a pairing.
    known: Option<BTreeSet<String>>,
    next_poll: Instant,
    /// When Coder's tasks and sign-ins are next read.
    next_coder: Instant,
    /// Clipboard entries to clear, and when.
    clear: Vec<(String, Instant)>,
    adopting: bool,
    /// `DSK-04`'s terminal checkbox, and the request it was set for.
    nearby_terminal: (u64, bool),
    /// What the screens call this computer: "Mac" on a Mac, "computer" on
    /// Linux and Windows ([`crate::words::COMPUTER`]).
    pub computer: &'static str,
}

impl Model {
    /// A model that opens on `screen`.
    pub fn new(now: Instant, screen: Screen, agent: Agent, old: Option<OldSetup>) -> Model {
        Model {
            screen,
            codes: Codes::new(now),
            host: None,
            reached: false,
            agents: Agents::default(),
            tasks: Vec::new(),
            agent,
            old,
            confirming: None,
            copied_at: None,
            problem: None,
            visible: true,
            unlocked: true,
            known: None,
            next_poll: now,
            next_coder: now,
            clear: Vec::new(),
            adopting: false,
            nearby_terminal: (0, false),
            computer: crate::words::COMPUTER,
        }
    }

    /// The phone nearby waiting for a click, if any. It shows over every
    /// screen until it is answered, withdrawn, or expires.
    pub fn nearby(&self) -> Option<&NearbyPrompt> {
        self.host.as_ref().and_then(|host| host.nearby.as_ref())
    }

    /// `DSK-04`'s terminal checkbox for the shown request; off for a new one.
    pub fn nearby_terminal(&self) -> bool {
        self.nearby()
            .is_some_and(|prompt| self.nearby_terminal == (prompt.id, true))
    }

    /// The screen a first launch opens on: the adoption question when an
    /// old-style setup is here, the code when no phone is connected yet,
    /// and home otherwise (decided once the host answers).
    pub fn first_screen(old: &Option<OldSetup>) -> Screen {
        if old.is_some() {
            Screen::Adopt
        } else {
            Screen::Connect
        }
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

    /// Whether phones may start Coder here.
    pub fn autostart(&self) -> bool {
        self.host
            .as_ref()
            .is_some_and(|host| host.autostart.enabled && !host.autostart.projects.is_empty())
    }

    /// Brings the model up to `now`.
    pub fn tick(&mut self, now: Instant) -> Vec<Request> {
        let mut requests = Vec::new();
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
        if let Some(at) = self.codes.next_wake() {
            wake = wake.min(at);
        }
        for (_, at) in &self.clear {
            wake = wake.min(*at);
        }
        wake
    }

    /// The window was shown or hidden.
    pub fn shown(&mut self, visible: bool, now: Instant) {
        self.visible = visible;
        if visible {
            self.codes.input(now);
        }
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
            Intent::ToggleTerminal => {
                let on = !self.codes.terminal();
                self.codes
                    .set_terminal(on)
                    .into_iter()
                    .map(Request::Code)
                    .chain(self.tick(now))
                    .collect()
            }
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
            Intent::ChooseFolder => vec![Request::ChooseFolder],
            Intent::ToggleAutostart => {
                let Some(host) = &self.host else {
                    return Vec::new();
                };
                let projects: Vec<String> = host.projects.iter().map(|p| p.label.clone()).collect();
                if projects.is_empty() {
                    return Vec::new();
                }
                let enabled = !self.autostart();
                vec![Request::SetAutostart(Autostart {
                    enabled,
                    projects,
                    max_running: host.autostart.max_running.max(1),
                })]
            }
            Intent::Done | Intent::Back | Intent::Keep | Intent::NotNow => {
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
            Intent::Adopt => {
                if self.old.as_ref().is_some_and(|old| old.ready) && !self.adopting {
                    self.adopting = true;
                    vec![Request::Adopt]
                } else {
                    Vec::new()
                }
            }
            Intent::OpenLoginItems => vec![Request::OpenLoginItems],
            Intent::NearbyTerminal => {
                if let Some(id) = self.nearby().map(|prompt| prompt.id) {
                    self.nearby_terminal = (id, !self.nearby_terminal());
                }
                Vec::new()
            }
            Intent::NearbyConnect { id } | Intent::NearbyDecline { id } => {
                // Only the request on screen, and only once.
                if self.nearby().map(|prompt| prompt.id) != Some(id) {
                    return Vec::new();
                }
                let connect = matches!(intent, Intent::NearbyConnect { .. });
                let terminal = connect && self.nearby_terminal();
                if let Some(host) = &mut self.host {
                    host.nearby = None;
                }
                vec![
                    Request::NearbyDecide {
                        id,
                        connect,
                        terminal,
                    },
                    Request::Refresh,
                ]
            }
        }
    }

    /// Whether the adoption helper is running.
    pub fn adopting(&self) -> bool {
        self.adopting
    }

    /// Applies a finished request.
    pub fn outcome(&mut self, outcome: Outcome, now: Instant) -> Vec<Request> {
        match outcome {
            Outcome::Refreshed(Some(state)) => {
                let state = *state;
                self.reached = true;
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
                requests.extend(self.tick(now));
                requests
            }
            Outcome::Refreshed(None) => {
                self.host = None;
                self.tick(now)
            }
            Outcome::Created {
                ticket,
                invitation,
                code,
                terminal,
            } => self
                .codes
                .created(ticket, invitation, code, terminal, now)
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
            Outcome::Folder(None) => Vec::new(),
            Outcome::Folder(Some(path)) => {
                if !path.join(".git").exists() {
                    self.problem = Some(
                        "That folder isn't a Git project. Choose the folder that holds your code."
                            .into(),
                    );
                    return Vec::new();
                }
                self.problem = None;
                let mut requests = vec![Request::AddProject { path: path.clone() }];
                // Picking the first project turns the switch on.
                let label = path
                    .file_name()
                    .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
                let mut projects: Vec<String> = self
                    .host
                    .as_ref()
                    .map(|host| host.projects.iter().map(|p| p.label.clone()).collect())
                    .unwrap_or_default();
                if !projects.contains(&label) {
                    projects.push(label);
                }
                let enabled = self.autostart() || self.project().is_none();
                requests.push(Request::SetAutostart(Autostart {
                    enabled,
                    projects,
                    max_running: 1,
                }));
                requests.push(Request::Refresh);
                requests
            }
            Outcome::Coder { agents, tasks } => {
                self.agents = agents;
                self.tasks = tasks;
                Vec::new()
            }
            Outcome::Copied => Vec::new(),
            Outcome::Adopted(result) => {
                self.adopting = false;
                match result {
                    Ok(()) => {
                        self.old = None;
                        self.agent = Agent::Enabled;
                        self.go(Screen::Home, now);
                        vec![Request::Refresh, Request::Coder]
                    }
                    Err(message) => {
                        self.problem = Some(message);
                        Vec::new()
                    }
                }
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
        pub folder: Option<PathBuf>,
    }

    impl Rig {
        pub fn new() -> Rig {
            let start = Instant::now();
            Rig {
                model: Model::new(start, Screen::Connect, Agent::Enabled, None),
                host: FakeHost::new("Studio Mac", 1_790_000_000),
                start,
                now: start,
                clipboard: None,
                folder: None,
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
                    Request::Refresh => {
                        let mut host = self.host.clone();
                        let state = host.status().ok().map(|status| {
                            Box::new(Refreshed {
                                status,
                                devices: host.devices().unwrap_or_default(),
                                projects: host.projects().unwrap_or_default(),
                                autostart: host.autostart().expect("a policy"),
                                nearby: host.nearby_pending().unwrap_or_default(),
                            })
                        });
                        Some(Outcome::Refreshed(state))
                    }
                    Request::Code(Action::Create { ticket, terminal }) => {
                        Some(match self.host.invite(terminal) {
                            Ok(invite) => Outcome::Created {
                                ticket,
                                invitation: invite.invitation,
                                code: invite.code,
                                terminal,
                            },
                            Err(_) => Outcome::CreateFailed { ticket },
                        })
                    }
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
                    Request::AddProject { path } => {
                        let _ = self.host.add_project(&path.to_string_lossy());
                        None
                    }
                    Request::SetAutostart(policy) => {
                        let _ = self.host.set_autostart(policy);
                        None
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
                        },
                        tasks: vec![Task {
                            title: "Fix the login test".into(),
                            status: "running".into(),
                        }],
                    }),
                    Request::OpenLoginItems | Request::Adopt => None,
                    Request::NearbyDecide {
                        id,
                        connect,
                        terminal,
                    } => {
                        let _ = self.host.nearby_decide(id, connect, terminal);
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

    #[test]
    fn a_mac_with_a_phone_opens_on_home() {
        let mut rig = Rig::new();
        let invite = rig.host.clone().invite(false).expect("an invite");
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
        rig.folder = Some(plain.path().to_path_buf());
        rig.click(Intent::ChooseFolder);
        assert!(rig.model.problem.is_some());
        assert!(rig.model.project().is_none());
        let repo = tempfile::tempdir().expect("a folder");
        std::fs::create_dir(repo.path().join(".git")).expect("a .git");
        rig.folder = Some(repo.path().to_path_buf());
        rig.click(Intent::ChooseFolder);
        assert!(rig.model.problem.is_none());
        assert!(rig.model.project().is_some());
        assert!(rig.model.autostart());
        rig.click(Intent::ToggleAutostart);
        rig.click(Intent::Done);
        assert!(!rig.model.autostart());
        assert_eq!(rig.model.screen, Screen::Home);
    }

    #[test]
    fn remove_asks_first_then_revokes() {
        let mut rig = Rig::new();
        let invite = rig.host.clone().invite(true).expect("an invite");
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
        assert!(!rig.model.nearby_terminal(), "the checkbox starts off");
        // A click for another request does nothing.
        rig.click(Intent::NearbyConnect { id: id + 1 });
        assert!(rig.host.nearby_answers().is_empty());
        rig.click(Intent::NearbyTerminal);
        assert!(rig.model.nearby_terminal());
        rig.click(Intent::NearbyConnect { id });
        assert_eq!(rig.host.nearby_answers(), vec![(id, true, true)]);
        assert!(rig.model.nearby().is_none());
        // A second click on the same prompt sends nothing more.
        rig.click(Intent::NearbyConnect { id });
        assert_eq!(rig.host.nearby_answers().len(), 1);
    }

    #[test]
    fn dont_connect_answers_no_and_a_new_request_starts_without_a_terminal() {
        let mut rig = Rig::new();
        rig.tick(0);
        let first = rig.host.ask_nearby("Kai's iPhone", "111111");
        rig.tick(5);
        rig.click(Intent::NearbyTerminal);
        rig.click(Intent::NearbyDecline { id: first });
        assert_eq!(rig.host.nearby_answers(), vec![(first, false, false)]);
        let second = rig.host.ask_nearby("Kai's iPad", "222222");
        rig.tick(10);
        assert_eq!(rig.model.nearby().map(|p| p.id), Some(second));
        assert!(!rig.model.nearby_terminal());
    }
}
