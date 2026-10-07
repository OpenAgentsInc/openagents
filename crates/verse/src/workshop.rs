//! The workshop agent in Everglade (`docs/verse/workshop-agent.md`): Alice
//! at her workstation in the owner's house, as a client of the resident
//! host.
//!
//! The host is the only authority over her: it plans each request, gives
//! each command its effect class, journals every step, and holds her
//! proposals for your CONFIRM or REJECT (`coder::task::agent_host`). This
//! window asks for `studio.agent.list` over the host's same-user control
//! socket and draws what comes back: her seat, drawn as her own
//! character at her workstation in the owner's house, with her activity
//! and status on her nameplate, walking to the console by the east wall
//! (her Workbench) while a command runs and to the lectern (her Podium)
//! while a proposal waits ([`Workshop::seats`]).
//!
//! Walk up to her and press the interact key to open her panel, anchored
//! to the bottom of the window ([`Workshop::rows`]): a status row that
//! always shows, her transcript, a pending proposal when one waits, the
//! input line, and the key strip. ENTER sends a request, or CONFIRMs a
//! proposal on an empty line; ESC REJECTs it, or closes the panel. F2 shows
//! her memory, F4 her journal, F7 stops her, and F8 pauses or resumes her,
//! each of the last two only after CONFIRM.
//!
//! She does her work in Coder V1 (#10753): the host runs each request as a
//! turn of her own Coder session, and Coder's approvals are her proposals.
//! A request from this window asks for a typist: while the turn runs, this
//! window shows her pane, titled `driven by alice`, running Coder's own
//! terminal following her session, so you watch Coder work. A key you
//! press in her pane takes it over: this window tells the host
//! (`studio.agent.ran`), which stops her turn, and the session is yours to
//! type in. When the host stops her, this window releases her pane.
//!
//! When the host has no Alice yet, her panel sets her up in the world
//! ([`Setup`]): she introduces herself, asks which Git checkout she works
//! in from the ones the host knows (`studio.agent.workspaces`) or a typed
//! path, shows a summary, and on CONFIRM the host makes her
//! (`studio.agent.new`) with a key it attests with the owner key it holds.
//! When no host answers, CONFIRM starts this computer's host the way
//! `openagents studio up` does ([`Starter`]) and setup continues. The owner
//! picks and confirms; nothing asks for a key or a command line.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use coder_access::agent::{self as wire, AgentView, Mode};
use coder_access::studio::{self as seat_wire, Activity};
use coder_access::{Operation, Outcome};
use coder_ui::theme::Intensity;
use glam::Vec3;
use terminal_gfx::layout::PaneId;

use crate::zones::everglade::studio::live::{ControlSocket, Transport};

/// The workshop agent's name.
pub const NAME: &str = crate::zones::everglade::studio::WORKSHOP_AGENT;
/// Her seat's desk number: none of the workshop's four, which stay the
/// studio's. She works at her own workstation in the owner's house
/// (`everglade::layout::estate::AliceSpot`).
pub const DESK: u32 = 100;
/// Her look: Alice's own character.
pub const LOOK: &str = "alice";
/// How near her the player stands to talk to her, m.
pub const REACH: f32 = crate::zones::everglade::studio::TALK_REACH;
/// How far in front of her, across her workstation, `--workshop-ask` and
/// the captures stand the player, m: within [`REACH`], clear of the desk.
pub const WALK_UP: f32 = 2.2;
/// How often the worker asks the host for her.
const POLL: Duration = Duration::from_millis(400);
/// The most transcript lines her panel keeps of its own.
const LINES: usize = 400;
/// The most characters the input line takes.
const INPUT_MAX: usize = 2000;

/// What the frame asks the worker to send.
enum ToHost {
    Send(Operation),
}

/// What the worker tells the frame.
enum FromHost {
    /// Her, as the host holds her now; `None` when the host has no agent
    /// by her name.
    View(Option<Box<AgentView>>),
    /// The host answered a page's read.
    Page(Page, Vec<String>),
    /// The host refused an operation, or did not answer.
    Refused(String),
    /// The host does not answer its socket.
    Unreachable(String),
    /// The checkouts the host offers her as a workspace.
    Places(Vec<wire::Place>),
    /// The host made her.
    Made(wire::Made),
    /// The host refused to make her.
    SetupRefused(String),
    /// The host answers, but she is not this window's: it refused to list
    /// her.
    NotOwner(String),
}

/// Where her setup stands, while the host has no agent by her name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Setup {
    /// She exists, or nothing has been asked yet.
    Off,
    /// She introduces herself; ENTER continues.
    Hello,
    /// No host answers; CONFIRM starts one on this computer.
    NoHost,
    /// The host is starting.
    Starting,
    /// Which checkout she works in: one the host offers, or a typed path.
    Pick { selected: usize },
    /// What she will be; CONFIRM makes her and REJECT makes nothing.
    Summary { workspace: String },
    /// The host is making her.
    Making { workspace: String },
}

/// Starts this computer's host, as `openagents studio up` does, and says
/// what it did in a sentence.
pub type Starter = Arc<dyn Fn() -> Result<String, String> + Send + Sync>;

/// What she suggests as a first request once she is set up: read-only.
pub const FIRST_REQUEST: &str = "show the last five commits and tell me what changed";

/// What her panel shows in the transcript's place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Transcript,
    Memory,
    Journal,
}

/// An action that waits for CONFIRM.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Asking {
    Stop,
    Pause,
    Resume,
}

/// The Coder turn her pane follows.
struct Job {
    step: u64,
}

/// The connection to the host.
struct Worker {
    to: Sender<ToHost>,
    from: Receiver<FromHost>,
}

/// Makes a transport, once, when she is first loaded.
pub type Connect = Box<dyn FnOnce() -> Option<Box<dyn Transport>> + Send>;

/// The workshop agent as this window shows and drives her.
pub struct Workshop {
    connect: Option<Connect>,
    worker: Option<Worker>,
    /// Her, as the host last answered.
    view: Option<AgentView>,
    /// Why the host is out of reach, or refused the last operation.
    trouble: Option<String>,
    /// Her panel shows and takes the keys.
    pub open: bool,
    pub input: String,
    /// What this window says beside her transcript: refusals and how to
    /// start the host.
    notes: VecDeque<String>,
    /// Lines scrolled back from the newest.
    scroll: usize,
    page: Page,
    page_lines: Vec<String>,
    asking: Option<Asking>,
    job: Option<Job>,
    pane: Option<PaneId>,
    /// The host's stop counter this window has acted on.
    released: u64,
    /// The steps this window typed, so a step is typed once.
    typed: VecDeque<u64>,
    /// The proposal step this window answered last.
    answered: Option<u64>,
    loaded: bool,
    /// The host answered and has no agent by her name.
    absent: bool,
    /// Her setup while she does not exist.
    setup: Setup,
    /// The checkouts setup offers, most likely first; `None` until read.
    places: Option<Vec<wire::Place>>,
    /// What setup says under its step: a refused path or a host's answer.
    setup_note: Option<String>,
    /// Starts this computer's host when the owner confirms.
    starter: Option<Starter>,
    /// The host start under way.
    starting: Option<Receiver<Result<String, String>>>,
    /// The host refused this window her list: she is not its owner's.
    not_owner: bool,
    /// What she said when setup ended, shown before her transcript.
    greeting: Vec<String>,
}

impl Default for Workshop {
    fn default() -> Self {
        Self {
            connect: None,
            worker: None,
            view: None,
            trouble: None,
            open: false,
            input: String::new(),
            notes: VecDeque::new(),
            scroll: 0,
            page: Page::Transcript,
            page_lines: Vec::new(),
            asking: None,
            job: None,
            pane: None,
            released: 0,
            typed: VecDeque::new(),
            answered: None,
            loaded: false,
            absent: false,
            setup: Setup::Off,
            places: None,
            setup_note: None,
            starter: None,
            starting: None,
            not_owner: false,
            greeting: Vec::new(),
        }
    }
}

/// `text` as a workspace for her: an absolute folder in a Git checkout,
/// canonical, with a leading `~/` read as the home folder. The host
/// checks the same again before it makes her.
///
/// # Errors
/// A plain sentence that says what is wrong with the path.
pub fn checkout(text: &str) -> Result<String, String> {
    let text = text.trim();
    let path = match text.strip_prefix("~/") {
        Some(rest) => std::env::var_os("HOME")
            .map(|home| PathBuf::from(home).join(rest))
            .unwrap_or_else(|| PathBuf::from(text)),
        None if text == "~" => std::env::var_os("HOME").map_or_else(|| text.into(), PathBuf::from),
        None => PathBuf::from(text),
    };
    if !path.is_absolute() {
        return Err(format!("{text} is not a full path; start it with / or ~/."));
    }
    let Ok(canonical) = path.canonicalize() else {
        return Err(format!("{text} does not exist."));
    };
    if !canonical.is_dir() {
        return Err(format!("{text} is not a folder."));
    }
    if !canonical.ancestors().any(|dir| dir.join(".git").exists()) {
        return Err(format!("{text} is not in a Git repository."));
    }
    Ok(canonical.display().to_string())
}

/// The checkouts this window adds to the host's: the one Verse was built
/// from and the one it started in.
fn own_places() -> Vec<wire::Place> {
    let built = option_env!("CARGO_MANIFEST_DIR").map(|dir| Path::new(dir).join("../.."));
    let mut places = Vec::new();
    for (path, from) in [
        (built, "this Verse's checkout"),
        (std::env::current_dir().ok(), "where Verse started"),
    ] {
        if let Some(Ok(path)) = path.map(|p| checkout(&p.display().to_string())) {
            places.push(wire::Place {
                path,
                from: from.into(),
            });
        }
    }
    places
}

/// This computer's host started by `openagents studio host`, found beside
/// this program or on `PATH`, on the control socket `socket`.
fn start_host(socket: &Path) -> Result<String, String> {
    let beside = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("openagents")));
    let on_path = std::env::var_os("PATH")
        .map(|path| {
            std::env::split_paths(&path)
                .map(|dir| dir.join("openagents"))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let program = beside
        .into_iter()
        .chain(on_path)
        .find(|path| path.is_file())
        .ok_or("I can't find the openagents program to start your host.")?;
    let output = std::process::Command::new(&program)
        .args(["studio", "host", "--control-socket"])
        .arg(socket)
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| format!("I can't start your host: {e}."))?;
    let said = |bytes: &[u8]| {
        String::from_utf8_lossy(bytes)
            .lines()
            .rfind(|line| !line.trim().is_empty())
            .unwrap_or("")
            .trim()
            .to_string()
    };
    if output.status.success() {
        Ok(said(&output.stdout))
    } else {
        // What the program said is for a terminal; her panel never shows
        // a command line or a path.
        Err(DID_NOT_START.into())
    }
}

/// What her panel says when the host did not start.
const DID_NOT_START: &str =
    "Your host didn't start. CONFIRM to try again, or REJECT to leave it off.";

/// A fresh 64-hex request identity.
fn mint() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let count =
        u128::from(NEXT.fetch_add(1, Ordering::Relaxed)) ^ (u128::from(std::process::id()) << 64);
    format!("{nanos:032x}{count:032x}")
}

/// The worker: asks for her every [`POLL`], and sends what the frame asks.
fn work(mut transport: Box<dyn Transport>, to: &Receiver<ToHost>, from: &Sender<FromHost>) {
    let mut reachable = true;
    loop {
        loop {
            match to.try_recv() {
                Ok(ToHost::Send(operation)) => {
                    let page = match &operation {
                        Operation::ListAgentMemory { .. } => Some(Page::Memory),
                        Operation::AgentLog { .. } => Some(Page::Journal),
                        _ => None,
                    };
                    let setup = matches!(operation, Operation::NewAgent { .. });
                    match transport.call(&mint(), &operation) {
                        Ok(Outcome::Agent { agent }) => {
                            if let Some(page) = page {
                                let _ = from.send(FromHost::Page(page, page_lines(page, &agent)));
                            }
                            match &operation {
                                Operation::ListAgentWorkspaces {} => {
                                    let places: wire::Places =
                                        serde_json::from_value(*agent).unwrap_or_default();
                                    let _ = from.send(FromHost::Places(places.places));
                                }
                                Operation::NewAgent { .. } => {
                                    let _ = from.send(
                                        match serde_json::from_value::<wire::Made>(*agent) {
                                            Ok(made) => FromHost::Made(made),
                                            Err(e) => FromHost::SetupRefused(format!(
                                                "the host's answer did not say: {e}"
                                            )),
                                        },
                                    );
                                }
                                _ => {}
                            }
                        }
                        Ok(_) => {}
                        Err(error) if setup => {
                            let _ = from.send(FromHost::SetupRefused(error.message));
                        }
                        Err(error) => {
                            let _ = from.send(FromHost::Refused(format!(
                                "the host refused {}: {}",
                                operation.name(),
                                error.message
                            )));
                        }
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => return,
            }
        }
        match transport.call(&mint(), &Operation::ListAgents {}) {
            Ok(Outcome::Agent { agent }) => {
                reachable = true;
                let agents: wire::Agents = serde_json::from_value(*agent).unwrap_or_default();
                let her = agents.agents.into_iter().find(|a| a.name == NAME);
                if from.send(FromHost::View(her.map(Box::new))).is_err() {
                    return;
                }
            }
            Ok(_) => {}
            Err(error) if error.code == coder_access::Code::Forbidden => {
                // The host answers, and not for this window's key: she is
                // someone else's.
                reachable = true;
                if from.send(FromHost::NotOwner(error.message)).is_err() {
                    return;
                }
            }
            Err(error) => {
                if reachable || error.code != coder_access::Code::Unavailable {
                    let _ = from.send(FromHost::Unreachable(error.message));
                }
                reachable = false;
            }
        }
        std::thread::sleep(POLL);
    }
}

fn page_lines(page: Page, value: &serde_json::Value) -> Vec<String> {
    match page {
        Page::Memory => {
            let memory: wire::Memory = serde_json::from_value(value.clone()).unwrap_or_default();
            if memory.memory.is_empty() {
                return vec!["She remembers nothing yet.".into()];
            }
            memory
                .memory
                .iter()
                .map(|m| format!("{:>3} {:<10} {:<9} {}", m.id, m.kind, m.state, m.text))
                .collect()
        }
        Page::Journal => {
            let journal: wire::Journal = serde_json::from_value(value.clone()).unwrap_or_default();
            journal
                .journal
                .iter()
                .map(|row| {
                    format!(
                        "{:>4} {:<9} {}{}",
                        row.seq,
                        row.kind,
                        row.text,
                        row.status
                            .map(|s| format!(" (exit {s})"))
                            .unwrap_or_default()
                    )
                })
                .collect()
        }
        Page::Transcript => Vec::new(),
    }
}

/// `text` in printable ASCII on one line.
fn ascii(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '\n' | '\t' => ' ',
            '\u{2018}' | '\u{2019}' => '\'',
            '\u{201c}' | '\u{201d}' => '"',
            '\u{2013}' | '\u{2014}' => '-',
            c if c.is_ascii_control() => ' ',
            c if c.is_ascii() => c,
            _ => '?',
        })
        .collect()
}

impl Workshop {
    /// A workshop whose agent lives on the host answering at `socket`.
    #[must_use]
    pub fn control(socket: Option<PathBuf>) -> Self {
        Self {
            starter: socket
                .clone()
                .map(|path| Arc::new(move || start_host(&path)) as Starter),
            connect: socket.map(|path| {
                Box::new(move || Some(Box::new(ControlSocket::new(path)) as Box<dyn Transport>))
                    as Connect
            }),
            ..Self::default()
        }
    }

    /// Start the host with `starter` instead, as a test does.
    #[must_use]
    pub fn with_starter(mut self, starter: Starter) -> Self {
        self.starter = Some(starter);
        self
    }

    /// Whether this window is her owner's: it has its own host to ask,
    /// running or not, so F opens her panel and sets her up when she does
    /// not exist yet. A window with no host of its own, such as the web
    /// build, only sees her.
    #[must_use]
    pub fn owner(&self) -> bool {
        self.worker.is_some() && !self.not_owner
    }

    /// Where her setup stands.
    #[must_use]
    pub fn setup(&self) -> &Setup {
        &self.setup
    }

    /// Opens her panel: her requests when she exists, else her setup.
    pub fn open_panel(&mut self) {
        self.open = true;
        self.begin_setup();
    }

    /// Starts her setup when the panel is open, she is not on the host,
    /// and the host has said why: it has no agent by her name, or it does
    /// not answer.
    fn begin_setup(&mut self) {
        if self.not_owner {
            self.open = false;
            self.setup = Setup::Off;
            return;
        }
        if self.worker.is_none() || !self.open || self.view.is_some() || self.setup != Setup::Off {
            return;
        }
        if self.trouble.is_some() {
            self.setup = Setup::NoHost;
        } else if self.absent {
            self.setup = Setup::Hello;
        }
    }

    /// Asks the host which checkouts she may work in, and goes to the
    /// pick.
    fn pick(&mut self) {
        self.setup = Setup::Pick { selected: 0 };
        self.places = None;
        self.input.clear();
        self.send(Operation::ListAgentWorkspaces {});
    }

    /// One key during her setup.
    fn setup_key(&mut self, key: PanelKey) {
        match (self.setup.clone(), key) {
            (Setup::Pick { .. }, PanelKey::Char(c)) => {
                if !c.is_control() && self.input.chars().count() < INPUT_MAX {
                    self.input.push(if c.is_ascii() { c } else { '?' });
                }
            }
            (Setup::Pick { .. }, PanelKey::Backspace) => {
                self.input.pop();
            }
            (Setup::Pick { selected }, PanelKey::Up) => {
                self.setup = Setup::Pick {
                    selected: selected.saturating_sub(1),
                };
            }
            (Setup::Pick { selected }, PanelKey::Down) => {
                let last = self
                    .places
                    .as_ref()
                    .map_or(0, |p| p.len().saturating_sub(1));
                self.setup = Setup::Pick {
                    selected: (selected + 1).min(last),
                };
            }
            (Setup::Hello, PanelKey::Enter) => {
                self.setup_note = None;
                self.pick();
            }
            (Setup::NoHost, PanelKey::Enter) => self.start(),
            (Setup::NoHost, PanelKey::Escape) => {
                self.setup = Setup::Off;
                self.setup_note = None;
                self.open = false;
            }
            (Setup::Pick { selected }, PanelKey::Enter) => {
                let typed = self.input.trim().to_string();
                let places = self.places.clone().unwrap_or_default();
                let chosen = match typed.parse::<usize>() {
                    Ok(n) if (1..=places.len()).contains(&n) => Ok(places[n - 1].path.clone()),
                    _ if typed.is_empty() => places
                        .get(selected)
                        .map(|p| p.path.clone())
                        .ok_or_else(|| "Type a path first.".to_string()),
                    _ => Ok(typed),
                };
                match chosen.and_then(|path| checkout(&path)) {
                    Ok(workspace) => {
                        self.input.clear();
                        self.setup_note = None;
                        self.setup = Setup::Summary { workspace };
                    }
                    Err(why) => self.setup_note = Some(why),
                }
            }
            (Setup::Summary { workspace }, PanelKey::Enter) => {
                self.setup_note = None;
                self.send(Operation::NewAgent {
                    agent: NAME.into(),
                    workspace: workspace.clone(),
                });
                self.setup = Setup::Making { workspace };
            }
            (Setup::Summary { workspace }, PanelKey::Escape) => {
                let selected = self
                    .places
                    .as_ref()
                    .and_then(|p| p.iter().position(|p| p.path == workspace))
                    .unwrap_or(0);
                self.setup = Setup::Pick { selected };
                self.setup_note =
                    Some("Nothing was made. Pick again, or press ESC to close.".into());
            }
            (_, PanelKey::Escape) => self.open = false,
            _ => {}
        }
    }

    /// Starts this computer's host on a thread of its own.
    fn start(&mut self) {
        let Some(starter) = self.starter.clone() else {
            self.setup_note = Some("This window cannot start a host.".into());
            return;
        };
        let (tx, rx) = mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("verse-start-host".into())
            .spawn(move || {
                let _ = tx.send(starter());
            });
        match spawned {
            Ok(_) => {
                self.starting = Some(rx);
                self.setup = Setup::Starting;
                self.setup_note = None;
            }
            Err(e) => self.setup_note = Some(format!("I can't start your host: {e}.")),
        }
    }

    /// What she says during setup, in place of her transcript.
    fn setup_lines(&self) -> Vec<String> {
        let me = format!("{NAME}:");
        let mut lines: Vec<String> = match &self.setup {
            Setup::Off => Vec::new(),
            Setup::Hello => vec![
                format!("{me} Hello, I'm Alice, your workshop agent."),
                format!(
                    "{me} I run commands in a terminal I drive, make code changes in my own \
                     worktree, and report back here. Anything that is not read-only waits for \
                     your CONFIRM."
                ),
                format!("{me} Only you, my owner, can give me work. Let's set me up."),
            ],
            Setup::NoHost => vec![
                format!("{me} No host is running on this computer, so I have nowhere to work."),
                format!(
                    "{me} CONFIRM starts your host here, the one the studio uses. REJECT leaves \
                     it off."
                ),
            ],
            Setup::Starting => vec![
                format!("{me} Starting your host... This can take a minute."),
                format!(
                    "{me} If macOS asks whether OpenAgents may use your keychain, enter your \
                     password and choose Always Allow."
                ),
            ],
            Setup::Pick { selected } => {
                let mut lines = vec![format!(
                    "{me} Which workspace do I work in? My terminal opens there."
                )];
                match &self.places {
                    None => lines.push("  reading the host's checkouts...".into()),
                    Some(places) if places.is_empty() => lines.push(format!(
                        "{me} The host knows no checkouts yet. Type a path, then press ENTER."
                    )),
                    Some(places) => {
                        for (i, place) in places.iter().enumerate() {
                            let mark = if i == *selected { '>' } else { ' ' };
                            lines.push(format!(
                                "{mark} {}  {}  ({})",
                                i + 1,
                                place.path,
                                place.from
                            ));
                        }
                        lines.push(format!(
                            "{me} Choose with UP and DOWN, or type a number or a path, then \
                             press ENTER."
                        ));
                    }
                }
                lines
            }
            Setup::Summary { workspace } => vec![
                format!("{me} Here is what I will be. CONFIRM makes me; REJECT makes nothing."),
                format!("  name       {NAME}"),
                format!("  workspace  {workspace}"),
                "  key        a new key of my own, attested by your owner key on this host"
                    .to_string(),
                "  answers    only you".to_string(),
                "  charter    read-only commands run without asking; anything else waits for \
                 your CONFIRM"
                    .to_string(),
            ],
            Setup::Making { .. } => vec![format!("{me} Making my key and my record...")],
        };
        if let Some(note) = &self.setup_note {
            lines.push(format!("{me} {note}"));
        }
        lines
    }

    /// What she says once the host made her, and her first suggestion.
    fn ready(&mut self, made: &wire::Made) {
        self.setup = Setup::Off;
        self.setup_note = None;
        self.absent = false;
        self.greeting = [
            format!("{NAME}: I'm ready. I work in {}.", made.workspace),
            format!(
                "{NAME}: {}",
                if made.attested_until.is_some() {
                    "My key is my own, attested by your owner key for a year."
                } else {
                    "This host does not hold your owner key, so my key is not attested yet. I \
                     still answer only you."
                }
            ),
            format!(
                "{NAME}: A first idea: ask me what changed lately. ENTER sends it, or type your \
                 own."
            ),
        ]
        .iter()
        .map(|line| ascii(line))
        .collect();
        self.scroll = 0;
        self.input = FIRST_REQUEST.into();
    }

    /// A workshop over `transport`, as a test or a scratch host uses.
    #[must_use]
    pub fn with_transport(transport: Box<dyn Transport>) -> Self {
        Self {
            connect: Some(Box::new(move || Some(transport))),
            ..Self::default()
        }
    }

    /// A workshop that shows `view`, as the host answered it, with no
    /// connection: for an offline capture of her panel and seat.
    #[must_use]
    pub fn showing(view: AgentView) -> Self {
        Self {
            view: Some(view),
            loaded: true,
            open: true,
            ..Self::default()
        }
    }

    /// Starts asking the host for her. Called on entering Everglade; once
    /// started it does nothing.
    pub fn load(&mut self) {
        if self.loaded {
            return;
        }
        self.loaded = true;
        let Some(transport) = self.connect.take().and_then(|connect| connect()) else {
            self.note(&format!(
                "{NAME} works on her owner's computer; this window has no host to ask."
            ));
            return;
        };
        let (to, rx) = mpsc::channel();
        let (tx, from) = mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("verse-workshop-agent".into())
            .spawn(move || work(transport, &rx, &tx));
        match spawned {
            Ok(_) => self.worker = Some(Worker { to, from }),
            Err(error) => self.note(&format!("{NAME}'s connection did not start: {error}")),
        }
    }

    /// Whether she is in the workshop: always, once Everglade loads.
    #[must_use]
    pub fn loaded(&self) -> bool {
        self.loaded
    }

    /// Whether the host answers for her.
    #[must_use]
    pub fn connected(&self) -> bool {
        self.view.is_some()
    }

    /// Whether a request is under way.
    #[must_use]
    pub fn busy(&self) -> bool {
        self.view.as_ref().is_some_and(|v| v.busy)
    }

    /// The proposal waiting for you, if one does.
    #[must_use]
    pub fn pending(&self) -> Option<&wire::Proposal> {
        self.view
            .as_ref()
            .and_then(|v| v.pending.as_ref())
            .filter(|p| self.answered != Some(p.step))
    }

    /// The pane she drives, once she opened one.
    #[must_use]
    pub fn pane(&self) -> Option<PaneId> {
        self.pane
    }

    /// What she is doing now.
    #[must_use]
    pub fn activity(&self) -> Activity {
        self.view.as_ref().map_or(Activity::Idle, |v| v.activity)
    }

    fn note(&mut self, line: &str) {
        self.notes.push_back(ascii(line));
        while self.notes.len() > LINES {
            self.notes.pop_front();
        }
        self.scroll = 0;
    }

    fn send(&mut self, operation: Operation) {
        match &self.worker {
            Some(worker) if worker.to.send(ToHost::Send(operation)).is_ok() => {}
            _ => self.note("The host is out of reach; nothing was sent."),
        }
    }

    /// Sends `text` to her as a request from you, with this window as her
    /// typist.
    pub fn ask(&mut self, text: &str) {
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        if self.view.is_none() {
            self.note(&format!(
                "{NAME} is not on a host yet. {}",
                self.trouble.clone().unwrap_or_default()
            ));
            return;
        }
        self.send(Operation::AskAgent {
            agent: NAME.into(),
            text: text.into(),
            workspace: None,
            context: String::new(),
            mode: Mode::Auto,
            typist: true,
        });
    }

    /// Answers the pending proposal.
    pub fn decide(&mut self, confirm: bool) {
        let Some(step) = self.pending().map(|p| p.step) else {
            return;
        };
        self.answered = Some(step);
        self.send(Operation::AnswerAgent {
            agent: NAME.into(),
            step,
            confirm,
        });
    }

    /// Takes what the host said and drives her pane, once a frame.
    pub fn frame(&mut self, terminal: &mut terminal_gfx::Overlay) {
        self.take();
        self.follow(terminal);
        self.drive(terminal, Instant::now());
    }

    /// Takes what the host and a host start said, and moves her setup on.
    fn take(&mut self) {
        let messages: Vec<FromHost> = self
            .worker
            .as_ref()
            .map(|w| w.from.try_iter().collect())
            .unwrap_or_default();
        for message in messages {
            match message {
                FromHost::View(view) => {
                    self.trouble = None;
                    self.not_owner = false;
                    match view {
                        Some(view) => {
                            self.view = Some(*view);
                            self.absent = false;
                            if !matches!(self.setup, Setup::Off) {
                                self.setup = Setup::Off;
                                self.setup_note = None;
                            }
                        }
                        None => {
                            self.view = None;
                            // A started host that has no Alice goes on to
                            // her setup; one being made waits for her.
                            if !matches!(self.setup, Setup::Making { .. }) {
                                self.absent = true;
                            }
                            if matches!(self.setup, Setup::NoHost | Setup::Starting) {
                                self.setup = Setup::Hello;
                            }
                        }
                    }
                }
                FromHost::Page(page, lines) => {
                    if self.page == page {
                        self.page_lines = lines;
                    }
                }
                FromHost::Refused(why) => self.note(&why),
                FromHost::Unreachable(why) => {
                    self.view = None;
                    self.absent = false;
                    // The status row says so; her setup offers to start
                    // one.
                    self.trouble = Some(why);
                    if matches!(
                        self.setup,
                        Setup::Hello | Setup::Pick { .. } | Setup::Summary { .. }
                    ) {
                        self.setup = Setup::NoHost;
                    }
                }
                FromHost::Places(found) => {
                    let mut places: Vec<wire::Place> = Vec::new();
                    for place in found.into_iter().chain(own_places()) {
                        if !places.iter().any(|p| p.path == place.path) {
                            places.push(place);
                        }
                    }
                    if let Setup::Pick { selected } = &mut self.setup {
                        *selected = (*selected).min(places.len().saturating_sub(1));
                    }
                    self.places = Some(places);
                }
                FromHost::Made(made) => self.ready(&made),
                FromHost::NotOwner(why) => {
                    if !self.not_owner {
                        self.note(&why);
                    }
                    self.not_owner = true;
                    self.view = None;
                }
                FromHost::SetupRefused(why) => {
                    if let Setup::Making { workspace } = self.setup.clone() {
                        self.setup = Setup::Summary { workspace };
                    }
                    self.setup_note = Some(format!("The host did not make me: {why}"));
                }
            }
        }
        if let Some(rx) = &self.starting {
            match rx.try_recv() {
                Ok(Ok(_)) => {
                    self.starting = None;
                    self.setup_note = Some("Your host is running on this computer.".into());
                }
                Ok(Err(why)) => {
                    self.starting = None;
                    self.setup = Setup::NoHost;
                    self.setup_note = Some(why);
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => self.starting = None,
            }
        }
        self.begin_setup();
    }

    /// Acts on what the host asks of this window: a stop releases her
    /// pane, and a Coder turn for a typist shows her pane once.
    fn follow(&mut self, terminal: &mut terminal_gfx::Overlay) {
        let Some(view) = self.view.clone() else {
            return;
        };
        if view.release > self.released {
            self.released = view.release;
            // The host stops her Coder turn itself; her pane keeps
            // Coder's terminal for you.
            self.job = None;
            if let Some(id) = self.pane {
                if let Some(pane) = terminal.panes.get_mut(&id) {
                    pane.typist = None;
                    pane.render_revision += 1;
                }
            }
            self.note(&format!("{NAME} was stopped: her pane is yours again."));
        }
        let Some(step) = view.run.filter(|s| s.typist) else {
            // Her turn ended: the pane stays, and is yours.
            if self.job.take().is_some()
                && let Some(pane) = self.pane.and_then(|id| terminal.panes.get_mut(&id))
            {
                pane.typist = None;
                pane.render_revision += 1;
            }
            return;
        };
        if self.typed.contains(&step.step) || self.job.is_some() {
            return;
        }
        self.typed.push_back(step.step);
        while self.typed.len() > 64 {
            self.typed.pop_front();
        }
        self.start_job(terminal, step);
    }

    /// Shows her pane following her Coder session for `step`: the pane
    /// she has when Coder's terminal still runs in it, else a new one.
    fn start_job(&mut self, terminal: &mut terminal_gfx::Overlay, step: wire::Step) {
        let Some(coder) = step.coder.clone() else {
            // A host from before Coder V1 types commands; this window no
            // longer does.
            self.report(
                step.step,
                wire::Ran {
                    lost: Some("this window shows Coder sessions only".into()),
                    ..wire::Ran::default()
                },
            );
            return;
        };
        let usable = self
            .pane
            .and_then(|id| terminal.panes.get(&id))
            .is_some_and(|pane| !pane.ended && pane.session.exited.is_none());
        if !usable {
            let Some((program, args)) = coder.argv.split_first() else {
                self.note("Coder's terminal isn't installed here; her work shows in this panel.");
                return;
            };
            let program = terminal_gfx::pty::Program::Command {
                program: PathBuf::from(program),
                args: args.to_vec(),
                label: format!("coder: {NAME}"),
            };
            self.pane = terminal.open_typist_running(NAME, &program);
            if self.pane.is_none() {
                let why = terminal
                    .notice
                    .clone()
                    .unwrap_or_else(|| "her Coder terminal did not start".into());
                self.note(&why);
                return;
            }
        }
        let Some(id) = self.pane else { return };
        terminal.show_pane(id);
        if let Some(pane) = terminal.panes.get_mut(&id) {
            // Her own pane: she takes it up again for a new turn.
            pane.typist = Some(NAME.into());
            pane.taken_back = false;
            pane.render_revision += 1;
        }
        self.job = Some(Job { step: step.step });
    }

    fn report(&mut self, step: u64, ran: wire::Ran) {
        self.send(Operation::AgentRan {
            agent: NAME.into(),
            step,
            ran,
        });
    }

    /// Watches her pane while Coder works: a key you pressed there takes
    /// her session over.
    fn drive(&mut self, terminal: &mut terminal_gfx::Overlay, _now: Instant) {
        let Some(job) = &self.job else { return };
        let step = job.step;
        let Some(pane) = self.pane.and_then(|id| terminal.panes.get_mut(&id)) else {
            // You closed her pane; Coder keeps working.
            self.job = None;
            return;
        };
        if pane.taken_back {
            pane.taken_back = false;
            self.job = None;
            self.report(
                step,
                wire::Ran {
                    taken_back: true,
                    ..wire::Ran::default()
                },
            );
            self.note("You took over her Coder session; it is yours in her pane.");
        } else if pane.ended || pane.session.exited.is_some() {
            self.job = None;
        }
    }

    /// Her studio seat: at her desk, with what she is doing and her status
    /// on her nameplate. She sits there whether or not a host answers.
    #[must_use]
    pub fn seats(&self) -> Vec<seat_wire::Seat> {
        if !self.loaded {
            return Vec::new();
        }
        let activity = self.activity();
        let station = match activity {
            Activity::Running | Activity::Testing | Activity::Editing => {
                seat_wire::Station::Workbench
            }
            Activity::Waiting => seat_wire::Station::Podium,
            _ => seat_wire::Station::Desk,
        };
        vec![seat_wire::Seat {
            seat: NAME.into(),
            role: seat_wire::Role::Worker,
            route: self.plate_status(),
            look: LOOK.into(),
            desk: DESK,
            activity,
            station,
            task: None,
            paused: self
                .view
                .as_ref()
                .is_some_and(|v| v.state == "paused" || v.state == "stopped"),
            spend: seat_wire::Spend::default(),
        }]
    }

    /// The nameplate's third row: her last outcome, else her model.
    fn plate_status(&self) -> String {
        let Some(view) = &self.view else {
            return if self.owner() && self.absent {
                "not set up".into()
            } else if self.owner() && self.trouble.is_some() {
                "no host".into()
            } else {
                "owner only".into()
            };
        };
        if view.state != "active" {
            return view.state.clone();
        }
        if self.pending().is_some() {
            return "needs you".into();
        }
        if let Some(change) = &view.change
            && change.stage == "merge"
        {
            return "change at merge".into();
        }
        if !view.headline.is_empty() && !view.busy {
            return view.headline.clone();
        }
        view.route.clone()
    }

    /// Whether `player` stands near enough to `at`, her position, to talk.
    #[must_use]
    pub fn within_reach(player: Vec3, at: Vec3) -> bool {
        (player.x - at.x).hypot(player.z - at.z) <= REACH
    }

    /// Every transcript line: the host's, then this window's notes.
    fn transcript(&self) -> Vec<String> {
        if self.setup != Setup::Off {
            return self.setup_lines().iter().map(|l| ascii(l)).collect();
        }
        match self.page {
            Page::Transcript => {
                // Her setup's last words come first, so her reports stay
                // the newest lines.
                let mut lines: Vec<String> = self.greeting.clone();
                lines.extend(
                    self.view
                        .as_ref()
                        .map(|v| v.lines.iter().map(|l| ascii(l)).collect::<Vec<_>>())
                        .unwrap_or_default(),
                );
                lines.extend(self.notes.iter().cloned());
                lines
            }
            Page::Memory | Page::Journal => self.page_lines.iter().map(|l| ascii(l)).collect(),
        }
    }

    /// The panel's rows, top to bottom, `cols` characters wide and `rows`
    /// rows tall: the status row, the transcript, a pending proposal, the
    /// input line, and the key strip. Every character is ASCII.
    #[must_use]
    pub fn rows(&self, cols: usize, rows: usize) -> Vec<(String, Intensity)> {
        let cols = cols.max(20);
        let fit = |text: &str| -> String {
            let ascii = ascii(text);
            if ascii.chars().count() <= cols {
                return ascii;
            }
            let mut cut: String = ascii.chars().take(cols.saturating_sub(3)).collect();
            cut.push_str("...");
            cut
        };
        let upper = NAME.to_uppercase();
        let step = match &self.setup {
            Setup::Off => None,
            Setup::Hello => Some("hello"),
            Setup::NoHost => Some("no host"),
            Setup::Starting => Some("starting the host"),
            Setup::Pick { .. } => Some("pick a workspace"),
            Setup::Summary { .. } => Some("confirm"),
            Setup::Making { .. } => Some("making her"),
        };
        let status = match &self.view {
            None if step.is_some() => format!(
                "{upper} | setup | {} | owner only",
                step.unwrap_or_default()
            ),
            None => format!(
                "{upper} | {}",
                self.trouble
                    .as_deref()
                    .map_or("waiting for the host".to_string(), |t| format!(
                        "no host: {t}"
                    ))
            ),
            Some(view) => {
                let activity = serde_json::to_value(view.activity)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .unwrap_or_default();
                format!(
                    "{upper} | {} | {activity} | {} | model: {} | jobs {}/{} | key {}",
                    view.state,
                    if view.headline.is_empty() {
                        "no report yet"
                    } else {
                        &view.headline
                    },
                    view.route,
                    view.jobs[0],
                    view.jobs[1],
                    if view.attested_until.is_some() {
                        "attested"
                    } else if view.pubkey.is_some() {
                        "unattested"
                    } else {
                        "none"
                    }
                )
            }
        };
        let mut out = vec![(fit(&status), Intensity::Full)];
        // A short panel, under the terminal's panes, drops the rule.
        let rule = rows >= 8;
        let question = match &self.setup {
            Setup::NoHost => Some("START THE HOST ON THIS COMPUTER?".to_string()),
            Setup::Summary { workspace } => Some(format!("SET UP {upper} IN {workspace}?")),
            _ => None,
        };
        let waiting = self.pending().is_some() || self.asking.is_some() || question.is_some();
        let footer = 2 + usize::from(waiting) + usize::from(rule);
        let room = rows.saturating_sub(1 + footer).max(1);
        let mut wrapped: Vec<String> = Vec::new();
        for line in self.transcript() {
            let chars: Vec<char> = line.chars().collect();
            if chars.is_empty() {
                wrapped.push(String::new());
            }
            // One column stays for the scroll bar.
            for chunk in chars.chunks(cols.saturating_sub(2).max(1)) {
                wrapped.push(chunk.iter().collect());
            }
        }
        let total = wrapped.len();
        let end = total.saturating_sub(self.scroll.min(total));
        let start = end.saturating_sub(room);
        let shown = &wrapped[start..end];
        let bar = |row: usize| -> char {
            if total <= room {
                return '|';
            }
            let thumb = start * room / total.max(1);
            let size = (room * room / total.max(1)).max(1);
            if (thumb..thumb + size).contains(&row) {
                '#'
            } else {
                '|'
            }
        };
        let blank = room - shown.len();
        for row in 0..blank {
            out.push((
                format!("{:<w$}{}", "", bar(row), w = cols - 1),
                Intensity::Quarter,
            ));
        }
        for (i, line) in shown.iter().enumerate() {
            let tone = if line.starts_with("you") || line.starts_with("  ") {
                Intensity::ThreeQuarters
            } else if line.starts_with(&format!("{NAME}: $")) || line.starts_with('>') {
                Intensity::Full
            } else {
                Intensity::Half
            };
            let text = fit(line);
            let pad = cols.saturating_sub(1).saturating_sub(text.chars().count());
            out.push((format!("{text}{}{}", " ".repeat(pad), bar(blank + i)), tone));
        }
        if rule {
            out.push(("-".repeat(cols), Intensity::Quarter));
        }
        if let Some(question) = &question {
            out.push((fit(question), Intensity::Full));
        } else if let Some(asking) = self.asking {
            let what = match asking {
                Asking::Stop => format!(
                    "STOP {upper}? Her jobs go off, her pane is released, her work is cancelled."
                ),
                Asking::Pause => {
                    format!("PAUSE {upper}? She keeps everything and starts nothing new.")
                }
                Asking::Resume => format!("RESUME {upper}?"),
            };
            out.push((fit(&what), Intensity::Full));
        } else if let Some(pending) = self.pending() {
            out.push((
                fit(&format!("PROPOSED: {}  ({})", pending.command, pending.why)),
                Intensity::Full,
            ));
        }
        let line = match (&self.setup, self.page) {
            (Setup::Pick { .. }, _) => format!("PATH > {}_", self.input),
            (Setup::Off, Page::Memory) => format!("MEMORY {upper} > {}_", self.input),
            (Setup::Off, _) => format!("ASK {upper} > {}_", self.input),
            _ => format!("SETUP {upper} >"),
        };
        out.push((fit(&line), Intensity::Full));
        let paused = self
            .view
            .as_ref()
            .is_some_and(|v| v.state == "paused" || v.state == "stopped");
        let keys = if let Some(keys) = match &self.setup {
            Setup::Off => None,
            Setup::Hello => Some("ENTER CONTINUE  ESC CLOSE"),
            Setup::NoHost | Setup::Summary { .. } => Some("ENTER CONFIRM  ESC REJECT"),
            Setup::Pick { .. } => Some("UP DOWN CHOOSE  ENTER PICK  ESC CLOSE"),
            Setup::Starting | Setup::Making { .. } => Some("ESC CLOSE"),
        } {
            keys.to_string()
        } else if self.asking.is_some() || self.pending().is_some() {
            "ENTER CONFIRM  ESC REJECT".to_string()
        } else {
            format!(
                "ENTER SEND  ESC CLOSE  F2 MEMORY  F4 JOURNAL  F7 STOP  F8 {}  PGUP PGDN{}",
                if paused { "RESUME" } else { "PAUSE" },
                if self.busy() {
                    "  CTRL+` HER PANE (ANY KEY TAKES IT BACK)"
                } else {
                    ""
                }
            )
        };
        out.push((fit(&keys), Intensity::Half));
        out
    }

    fn open_page(&mut self, page: Page) {
        self.page = if self.page == page {
            Page::Transcript
        } else {
            page
        };
        self.page_lines = vec!["reading...".into()];
        self.scroll = 0;
        match self.page {
            Page::Memory => self.send(Operation::ListAgentMemory {
                agent: NAME.into(),
                after: None,
            }),
            Page::Journal => self.send(Operation::AgentLog {
                agent: NAME.into(),
                after: None,
            }),
            Page::Transcript => {}
        }
    }

    /// A line typed on the memory page: `accept N`, `reject N`, `forget
    /// N`, or a note.
    fn memory_line(&mut self, text: &str) {
        let words: Vec<&str> = text.split_whitespace().collect();
        let edit = match words.as_slice() {
            [verb @ ("accept" | "reject" | "forget"), id] => match id.parse() {
                Ok(id) => match *verb {
                    "accept" => wire::MemoryEdit::Accept { id },
                    "reject" => wire::MemoryEdit::Reject { id },
                    _ => wire::MemoryEdit::Forget { id },
                },
                Err(_) => {
                    self.note("Name an entry by its number.");
                    return;
                }
            },
            _ => wire::MemoryEdit::Note { text: text.into() },
        };
        self.send(Operation::EditAgentMemory {
            agent: NAME.into(),
            edit,
        });
        self.send(Operation::ListAgentMemory {
            agent: NAME.into(),
            after: None,
        });
    }

    /// One key while her panel is open. Returns whether the panel took
    /// it, which it does for every key while open.
    pub fn key(&mut self, key: PanelKey) -> bool {
        if !self.open {
            return false;
        }
        if self.setup != Setup::Off {
            self.setup_key(key);
            return true;
        }
        match key {
            PanelKey::Up | PanelKey::Down => {}
            PanelKey::Char(c) => {
                if !c.is_control() && self.input.chars().count() < INPUT_MAX {
                    self.input.push(if c.is_ascii() { c } else { '?' });
                }
            }
            PanelKey::Backspace => {
                self.input.pop();
            }
            PanelKey::Enter => {
                if let Some(asking) = self.asking.take() {
                    match asking {
                        Asking::Stop => self.send(Operation::StopAgent {
                            agent: NAME.into(),
                            reason: "stopped at her desk".into(),
                        }),
                        Asking::Pause => self.send(Operation::PauseSeat { seat: NAME.into() }),
                        Asking::Resume => self.send(Operation::ResumeSeat { seat: NAME.into() }),
                    }
                } else if self.pending().is_some() {
                    if self.input.trim().is_empty() {
                        self.decide(true);
                    } else {
                        self.note("Answer the proposal first: ENTER confirms, ESC rejects.");
                    }
                } else {
                    let text = std::mem::take(&mut self.input);
                    if self.page == Page::Memory {
                        if !text.trim().is_empty() {
                            self.memory_line(text.trim());
                        }
                    } else {
                        self.ask(&text);
                    }
                }
            }
            PanelKey::Escape => {
                if self.asking.take().is_some() {
                } else if self.pending().is_some() {
                    self.decide(false);
                } else if self.page != Page::Transcript {
                    self.page = Page::Transcript;
                } else {
                    self.open = false;
                }
            }
            PanelKey::Memory => self.open_page(Page::Memory),
            PanelKey::Journal => self.open_page(Page::Journal),
            PanelKey::Stop => self.asking = Some(Asking::Stop),
            PanelKey::Pause => {
                let paused = self
                    .view
                    .as_ref()
                    .is_some_and(|v| v.state == "paused" || v.state == "stopped");
                self.asking = Some(if paused {
                    Asking::Resume
                } else {
                    Asking::Pause
                });
            }
            PanelKey::PageUp => self.scroll = (self.scroll + 5).min(self.transcript().len()),
            PanelKey::PageDown => self.scroll = self.scroll.saturating_sub(5),
        }
        true
    }
}

/// A key her panel reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelKey {
    Char(char),
    Backspace,
    Enter,
    Escape,
    PageUp,
    PageDown,
    /// UP and DOWN choose a checkout during her setup.
    Up,
    Down,
    /// F2: her memory.
    Memory,
    /// F4: her journal.
    Journal,
    /// F7: stop her, after CONFIRM.
    Stop,
    /// F8: pause or resume her, after CONFIRM.
    Pause,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// A host that answers `list` with `view` and records what it was sent.
    #[derive(Clone)]
    struct Fake {
        view: Arc<Mutex<Option<AgentView>>>,
        sent: Arc<Mutex<Vec<Operation>>>,
    }

    impl Transport for Fake {
        fn call(&mut self, _request: &str, op: &Operation) -> coder_access::Result<Outcome> {
            if !matches!(op, Operation::ListAgents {}) {
                self.sent.lock().unwrap().push(op.clone());
            }
            let value = match op {
                Operation::ListAgents {} => serde_json::to_value(wire::Agents {
                    agents: self.view.lock().unwrap().clone().into_iter().collect(),
                })
                .unwrap(),
                _ => serde_json::json!({"dispatched": "alice"}),
            };
            Ok(Outcome::Agent {
                agent: Box::new(value),
            })
        }
    }

    fn view() -> AgentView {
        AgentView {
            name: NAME.into(),
            look: LOOK.into(),
            route: "codex gpt-6-luna".into(),
            state: "active".into(),
            activity: Activity::Idle,
            headline: String::new(),
            desk: DESK,
            pubkey: Some("ab".repeat(32)),
            attested_until: Some(10),
            lines: vec!["you: run the atif tests".into()],
            pending: None,
            run: None,
            release: 0,
            change: None,
            service: wire::Service::default(),
            busy: false,
            jobs: [0, 1],
            candidates: 0,
        }
    }

    fn connected(view: AgentView) -> (Workshop, Fake) {
        let fake = Fake {
            view: Arc::new(Mutex::new(Some(view))),
            sent: Arc::new(Mutex::new(Vec::new())),
        };
        let mut workshop = Workshop::with_transport(Box::new(fake.clone()));
        workshop.load();
        let start = Instant::now();
        while workshop.view.is_none() {
            let messages: Vec<FromHost> =
                workshop.worker.as_ref().unwrap().from.try_iter().collect();
            for message in messages {
                if let FromHost::View(Some(v)) = message {
                    workshop.view = Some(*v);
                }
            }
            assert!(start.elapsed() < Duration::from_secs(5));
            std::thread::sleep(Duration::from_millis(10));
        }
        (workshop, fake)
    }

    fn sent(fake: &Fake, start: Instant) -> Vec<Operation> {
        loop {
            let sent = fake.sent.lock().unwrap().clone();
            if !sent.is_empty() || start.elapsed() > Duration::from_secs(5) {
                return sent;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn her_seat_follows_the_hosts_view_and_she_sits_without_a_host() {
        let mut alone = Workshop::control(None);
        assert!(alone.seats().is_empty());
        alone.load();
        let seat = &alone.seats()[0];
        assert_eq!((seat.seat.as_str(), seat.look.as_str()), (NAME, LOOK));
        assert_eq!(seat.station, seat_wire::Station::Desk);
        assert_eq!(seat.route, "owner only");
        let (mut workshop, _) = connected(view());
        let mut busy = view();
        busy.activity = Activity::Testing;
        workshop.view = Some(busy.clone());
        assert_eq!(workshop.seats()[0].station, seat_wire::Station::Workbench);
        busy.activity = Activity::Waiting;
        busy.pending = Some(wire::Proposal {
            step: 4,
            command: "touch notes.txt".into(),
            why: "`touch` is not on the read-only list".into(),
        });
        workshop.view = Some(busy);
        assert_eq!(workshop.seats()[0].station, seat_wire::Station::Podium);
        assert_eq!(workshop.seats()[0].route, "needs you");
    }

    #[test]
    fn the_panel_is_ascii_anchored_rows_and_answers_go_to_the_host() {
        let (mut workshop, fake) = connected(view());
        workshop.open = true;
        for c in "run the atif tests \u{2014} now".chars() {
            workshop.key(PanelKey::Char(c));
        }
        let rows = workshop.rows(80, 12);
        assert_eq!(rows.len(), 12);
        assert!(
            rows.iter()
                .all(|(row, _)| row.is_ascii() && row.len() <= 80)
        );
        assert!(
            rows[0].0.starts_with("ALICE | active | idle"),
            "{}",
            rows[0].0
        );
        assert!(
            rows[10]
                .0
                .starts_with("ASK ALICE > run the atif tests ? now")
        );
        assert!(rows[11].0.starts_with("ENTER SEND"));
        // The scroll bar shows at the right edge at all times.
        assert!(rows[1].0.ends_with('|') || rows[1].0.ends_with('#'));
        let start = Instant::now();
        workshop.key(PanelKey::Enter);
        let asked = sent(&fake, start);
        assert!(matches!(
            &asked[0],
            Operation::AskAgent { typist: true, text, .. } if text.starts_with("run the atif")
        ));
        fake.sent.lock().unwrap().clear();
        let mut waiting = view();
        waiting.pending = Some(wire::Proposal {
            step: 4,
            command: "touch notes.txt".into(),
            why: "`touch` is not on the read-only list".into(),
        });
        workshop.view = Some(waiting);
        let rows = workshop.rows(80, 12);
        assert!(
            rows.iter()
                .any(|(row, _)| row.starts_with("PROPOSED: touch notes.txt"))
        );
        assert!(rows[11].0.starts_with("ENTER CONFIRM  ESC REJECT"));
        let short = workshop.rows(80, 5);
        assert_eq!(short.len(), 5);
        assert!(short[2].0.starts_with("PROPOSED: touch notes.txt"));
        let start = Instant::now();
        assert!(workshop.key(PanelKey::Escape));
        let answered = sent(&fake, start);
        assert_eq!(
            answered[0],
            Operation::AnswerAgent {
                agent: NAME.into(),
                step: 4,
                confirm: false
            }
        );
        assert!(
            workshop.open,
            "ESC answered the proposal and kept the panel"
        );
        assert!(workshop.pending().is_none(), "answered once");
        // F7 asks before it stops her.
        fake.sent.lock().unwrap().clear();
        workshop.key(PanelKey::Stop);
        assert!(
            workshop
                .rows(80, 12)
                .iter()
                .any(|(r, _)| r.starts_with("STOP ALICE?"))
        );
        let start = Instant::now();
        workshop.key(PanelKey::Enter);
        assert!(matches!(sent(&fake, start)[0], Operation::StopAgent { .. }));
        workshop.key(PanelKey::Escape);
        assert!(!workshop.open);
        assert!(!workshop.key(PanelKey::Enter));
    }

    /// A host for her setup: down until started, refusing everything to a
    /// window that is not its owner's, offering `places`, and making her
    /// on `studio.agent.new`.
    #[derive(Clone)]
    struct Setting {
        up: Arc<std::sync::atomic::AtomicBool>,
        owner: bool,
        view: Arc<Mutex<Option<AgentView>>>,
        sent: Arc<Mutex<Vec<Operation>>>,
        places: Vec<wire::Place>,
    }

    impl Setting {
        fn new(up: bool, owner: bool, places: Vec<wire::Place>) -> Self {
            Self {
                up: Arc::new(std::sync::atomic::AtomicBool::new(up)),
                owner,
                view: Arc::new(Mutex::new(None)),
                sent: Arc::new(Mutex::new(Vec::new())),
                places,
            }
        }

        fn sent(&self) -> Vec<Operation> {
            self.sent.lock().unwrap().clone()
        }
    }

    impl Transport for Setting {
        fn call(&mut self, _request: &str, op: &Operation) -> coder_access::Result<Outcome> {
            use std::sync::atomic::Ordering;
            if !self.up.load(Ordering::SeqCst) {
                return Err(coder_access::Error::new(
                    coder_access::Code::Unavailable,
                    "no host answers the control socket",
                ));
            }
            if !self.owner {
                return Err(coder_access::Error::new(
                    coder_access::Code::Forbidden,
                    "The workshop agent answers only her owner.",
                ));
            }
            if !matches!(op, Operation::ListAgents {}) {
                self.sent.lock().unwrap().push(op.clone());
            }
            let value = match op {
                Operation::ListAgents {} => serde_json::to_value(wire::Agents {
                    agents: self.view.lock().unwrap().clone().into_iter().collect(),
                })
                .unwrap(),
                Operation::ListAgentWorkspaces {} => serde_json::to_value(wire::Places {
                    places: self.places.clone(),
                })
                .unwrap(),
                Operation::NewAgent { workspace, .. } => {
                    let mut her = view();
                    her.lines = Vec::new();
                    *self.view.lock().unwrap() = Some(her);
                    serde_json::to_value(wire::Made {
                        agent: NAME.into(),
                        workspace: workspace.clone(),
                        pubkey: "ab".repeat(32),
                        attested_until: Some(10),
                        existed: false,
                    })
                    .unwrap()
                }
                _ => serde_json::json!({"dispatched": "alice"}),
            };
            Ok(Outcome::Agent {
                agent: Box::new(value),
            })
        }
    }

    /// Takes what the host says until `done` holds.
    fn pump(workshop: &mut Workshop, done: impl Fn(&Workshop) -> bool) {
        let start = Instant::now();
        loop {
            workshop.take();
            if done(workshop) {
                return;
            }
            assert!(
                start.elapsed() < Duration::from_secs(10),
                "{:?} {:?}",
                workshop.setup,
                workshop.rows(100, 12)
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn typed(workshop: &mut Workshop, text: &str) {
        while !workshop.input.is_empty() {
            workshop.key(PanelKey::Backspace);
        }
        for c in text.chars() {
            workshop.key(PanelKey::Char(c));
        }
    }

    fn shows(workshop: &Workshop, text: &str) -> bool {
        workshop
            .rows(160, 14)
            .iter()
            .any(|(row, _)| row.contains(text))
    }

    /// A Git checkout and a plain folder under a scratch directory.
    fn checkouts() -> (tempfile::TempDir, String, String) {
        let dir = tempfile::tempdir().unwrap();
        let app = dir.path().join("app");
        std::fs::create_dir_all(app.join(".git")).unwrap();
        let plain = dir.path().join("plain");
        std::fs::create_dir_all(&plain).unwrap();
        let app = app.canonicalize().unwrap().display().to_string();
        let plain = plain.canonicalize().unwrap().display().to_string();
        (dir, app, plain)
    }

    /// Goes from her introduction to her first request: the pick, a path
    /// refused, REJECT at the summary, CONFIRM, and a request after.
    fn set_her_up(workshop: &mut Workshop, host: &Setting, app: &str, plain: &str) {
        assert_eq!(workshop.setup(), &Setup::Hello);
        assert!(shows(workshop, "Hello, I'm Alice, your workshop agent."));
        assert!(shows(workshop, "Only you, my owner, can give me work."));
        assert!(shows(workshop, "ENTER CONTINUE  ESC CLOSE"));
        let rows = workshop.rows(100, 12);
        assert_eq!(rows.len(), 12);
        assert!(rows.iter().all(|(r, _)| r.is_ascii() && r.len() <= 100));
        workshop.key(PanelKey::Enter);
        pump(workshop, |w| w.places.is_some());
        assert_eq!(workshop.setup(), &Setup::Pick { selected: 0 });
        assert!(shows(
            workshop,
            &format!("> 1  {app}  (the studio's repository)")
        ));
        assert!(shows(workshop, "PATH > _"));
        // A typed path is checked before anything is sent.
        typed(workshop, plain);
        workshop.key(PanelKey::Enter);
        assert!(shows(workshop, "is not in a Git repository."));
        typed(workshop, &format!("{plain}/missing"));
        workshop.key(PanelKey::Enter);
        assert!(shows(workshop, "does not exist."));
        typed(workshop, "relative/path");
        workshop.key(PanelKey::Enter);
        assert!(shows(workshop, "is not a full path"));
        assert!(matches!(workshop.setup(), Setup::Pick { .. }));
        // The offered checkout, by number; REJECT at the summary makes
        // nothing.
        typed(workshop, "1");
        workshop.key(PanelKey::Enter);
        assert_eq!(
            workshop.setup(),
            &Setup::Summary {
                workspace: app.into()
            }
        );
        assert!(shows(workshop, &format!("SET UP ALICE IN {app}?")));
        assert!(shows(workshop, "ENTER CONFIRM  ESC REJECT"));
        workshop.key(PanelKey::Escape);
        assert!(workshop.open, "REJECT keeps the panel");
        assert_eq!(workshop.setup(), &Setup::Pick { selected: 0 });
        assert!(shows(workshop, "Nothing was made."));
        assert!(
            !host
                .sent()
                .iter()
                .any(|op| matches!(op, Operation::NewAgent { .. }))
        );
        // ENTER takes the selected one; CONFIRM makes her.
        workshop.key(PanelKey::Enter);
        workshop.key(PanelKey::Enter);
        pump(workshop, |w| w.connected() && *w.setup() == Setup::Off);
        let made: Vec<Operation> = host
            .sent()
            .into_iter()
            .filter(|op| matches!(op, Operation::NewAgent { .. }))
            .collect();
        assert_eq!(
            made,
            vec![Operation::NewAgent {
                agent: NAME.into(),
                workspace: app.into()
            }]
        );
        assert!(shows(
            workshop,
            &format!("alice: I'm ready. I work in {app}.")
        ));
        assert!(shows(workshop, "attested by your owner key"));
        assert_eq!(workshop.input, FIRST_REQUEST);
        assert!(shows(workshop, "ENTER SEND"));
        // Her first request goes to the host as a request from you.
        workshop.key(PanelKey::Enter);
        let start = Instant::now();
        loop {
            if host.sent().iter().any(|op| {
                matches!(op, Operation::AskAgent { text, typist: true, .. } if text == FIRST_REQUEST)
            }) {
                break;
            }
            assert!(start.elapsed() < Duration::from_secs(5));
            std::thread::sleep(Duration::from_millis(10));
        }
        // Her report is the newest line, under what she said at setup.
        host.view.lock().unwrap().as_mut().unwrap().lines = vec![
            format!("you: {FIRST_REQUEST}"),
            "alice: Five commits, each adding a line.".into(),
        ];
        pump(workshop, |w| {
            w.transcript()
                .last()
                .is_some_and(|l| l.ends_with("Five commits, each adding a line."))
        });
        assert!(workshop.transcript()[0].starts_with("alice: I'm ready."));
    }

    #[test]
    fn with_no_agent_her_panel_sets_her_up_and_a_request_follows() {
        let (_dir, app, plain) = checkouts();
        let host = Setting::new(
            true,
            true,
            vec![wire::Place {
                path: app.clone(),
                from: "the studio's repository".into(),
            }],
        );
        let mut workshop = Workshop::with_transport(Box::new(host.clone()));
        workshop.load();
        pump(&mut workshop, |w| w.absent);
        assert!(workshop.owner());
        assert_eq!(workshop.seats()[0].route, "not set up");
        assert_eq!(workshop.setup(), &Setup::Off, "nothing until F");
        workshop.open_panel();
        set_her_up(&mut workshop, &host, &app, &plain);
    }

    #[test]
    fn with_no_host_her_panel_offers_to_start_one_and_goes_on() {
        let (_dir, app, plain) = checkouts();
        let host = Setting::new(
            false,
            true,
            vec![wire::Place {
                path: app.clone(),
                from: "the studio's repository".into(),
            }],
        );
        let up = host.up.clone();
        let starts = Arc::new(Mutex::new(0));
        let counted = starts.clone();
        let mut workshop =
            Workshop::with_transport(Box::new(host.clone())).with_starter(Arc::new(move || {
                *counted.lock().unwrap() += 1;
                up.store(true, std::sync::atomic::Ordering::SeqCst);
                Ok("Started the host (pid 4242).".into())
            }));
        workshop.load();
        pump(&mut workshop, |w| w.trouble.is_some());
        assert!(workshop.owner(), "her owner's window, with no host yet");
        workshop.open_panel();
        assert_eq!(workshop.setup(), &Setup::NoHost);
        assert!(shows(&workshop, "No host is running on this computer"));
        assert!(shows(&workshop, "START THE HOST ON THIS COMPUTER?"));
        assert!(shows(&workshop, "ENTER CONFIRM  ESC REJECT"));
        // REJECT leaves it off and closes her panel.
        workshop.key(PanelKey::Escape);
        assert!(!workshop.open);
        assert_eq!(*starts.lock().unwrap(), 0);
        // CONFIRM starts it, and setup goes on once it answers.
        workshop.open_panel();
        assert_eq!(workshop.setup(), &Setup::NoHost);
        workshop.key(PanelKey::Enter);
        assert_eq!(workshop.setup(), &Setup::Starting);
        pump(&mut workshop, |w| *w.setup() == Setup::Hello);
        assert_eq!(*starts.lock().unwrap(), 1);
        set_her_up(&mut workshop, &host, &app, &plain);
    }

    #[test]
    fn a_failed_start_says_why_and_offers_again() {
        let host = Setting::new(false, true, Vec::new());
        let starts = Arc::new(Mutex::new(0));
        let counted = starts.clone();
        let mut workshop =
            Workshop::with_transport(Box::new(host)).with_starter(Arc::new(move || {
                *counted.lock().unwrap() += 1;
                Err(DID_NOT_START.into())
            }));
        workshop.load();
        pump(&mut workshop, |w| w.trouble.is_some());
        workshop.open_panel();
        workshop.key(PanelKey::Enter);
        assert!(shows(&workshop, "Always Allow"));
        pump(&mut workshop, |w| *w.setup() == Setup::NoHost);
        assert!(shows(
            &workshop,
            "Your host didn't start. CONFIRM to try again"
        ));
        // CONFIRM tries again from the panel.
        workshop.key(PanelKey::Enter);
        pump(&mut workshop, |w| *w.setup() == Setup::NoHost);
        assert_eq!(*starts.lock().unwrap(), 2);
    }

    #[test]
    fn her_panel_never_shows_a_command_line_or_a_path() {
        for line in [DID_NOT_START] {
            assert!(!line.contains('`') && !line.contains('/'), "{line}");
        }
        let mut alone = Workshop::control(None);
        alone.load();
        for line in alone.notes.iter() {
            assert!(!line.contains('`') && !line.contains('/'), "{line}");
        }
    }

    #[test]
    fn a_window_that_is_not_her_owners_cannot_set_her_up() {
        // Without a host of its own, such as the web build, a window only
        // sees her.
        let mut alone = Workshop::control(None);
        alone.load();
        assert!(!alone.owner());
        assert_eq!(alone.seats()[0].route, "owner only");
        // A host that refuses this window's key: no setup, nothing sent.
        let host = Setting::new(true, false, Vec::new());
        let mut workshop = Workshop::with_transport(Box::new(host.clone()));
        workshop.load();
        pump(&mut workshop, |w| w.not_owner);
        assert!(!workshop.owner());
        workshop.open_panel();
        workshop.take();
        assert!(!workshop.open);
        assert_eq!(workshop.setup(), &Setup::Off);
        workshop.key(PanelKey::Enter);
        assert!(host.sent().is_empty());
    }

    #[test]
    fn a_typed_workspace_must_be_a_git_checkout() {
        let (dir, app, plain) = checkouts();
        assert_eq!(checkout(&app), Ok(app.clone()));
        std::fs::create_dir_all(dir.path().join("app/src")).unwrap();
        assert!(
            checkout(&format!("{app}/src")).is_ok(),
            "a folder inside one"
        );
        assert!(
            checkout(&plain)
                .unwrap_err()
                .ends_with("is not in a Git repository.")
        );
        assert!(
            checkout("/no/such/place")
                .unwrap_err()
                .ends_with("does not exist.")
        );
        assert!(checkout("code/app").is_err());
    }

    #[test]
    fn reach_is_measured_on_the_ground() {
        assert_eq!(NAME, "alice");
        let at = Vec3::new(1.0, 0.0, 1.0);
        assert!(Workshop::within_reach(Vec3::new(2.0, 5.0, 2.0), at));
        assert!(!Workshop::within_reach(Vec3::new(4.0, 0.0, 1.0), at));
    }

    #[test]
    fn her_seat_shows_in_the_studio_without_a_host() {
        let mut workshop = Workshop::control(None);
        workshop.load();
        let mut studio = crate::zones::everglade::studio::Studio::default();
        studio.set_resident(workshop.seats());
        assert!(
            studio.seat_position(NAME).is_none(),
            "nothing shows outside Everglade"
        );
        studio.set_active(true);
        assert!(studio.seat_position(NAME).is_some());
        studio.set_active(false);
        assert!(studio.seat_position(NAME).is_none());
    }
}
