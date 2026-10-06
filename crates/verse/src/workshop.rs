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
//! A request from this window asks for a typist: the host hands each
//! command it checked to this window, which types it into a terminal pane
//! titled `driven by alice` a few characters a frame, waits for the shell's
//! completion mark, and reports the command's status and output block
//! (`studio.agent.ran`). A key you press in her pane takes it back, and she
//! stops. When the host stops her, this window releases her pane and sends
//! `Ctrl+C` to the command she started.

use std::collections::VecDeque;
use std::path::PathBuf;
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
/// The time between two characters she types.
const TYPE_EVERY: Duration = Duration::from_millis(35);
/// How long she waits for a new pane's prompt before typing anyway.
const READY_WAIT: Duration = Duration::from_secs(5);
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
}

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

/// Where typing one command into her pane stands.
enum Phase {
    /// The pane is new: wait for its prompt.
    Ready {
        since: Instant,
    },
    Typing {
        chars: Vec<char>,
        at: usize,
        next: Instant,
        seen: Option<u64>,
    },
    Waiting {
        seen: Option<u64>,
    },
}

/// One command the host asked this window to type, after any setup lines.
struct Job {
    step: u64,
    lines: VecDeque<String>,
    phase: Phase,
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
    /// The directory her pane is in.
    pane_cwd: Option<String>,
    /// The host's stop counter this window has acted on.
    released: u64,
    /// The steps this window typed, so a step is typed once.
    typed: VecDeque<u64>,
    /// The proposal step this window answered last.
    answered: Option<u64>,
    loaded: bool,
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
            pane_cwd: None,
            released: 0,
            typed: VecDeque::new(),
            answered: None,
            loaded: false,
        }
    }
}

/// `path` quoted for a POSIX shell.
fn quoted(path: &str) -> String {
    format!("'{}'", path.replace('\'', "'\\''"))
}

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
                    match transport.call(&mint(), &operation) {
                        Ok(Outcome::Agent { agent }) => {
                            if let Some(page) = page {
                                let _ = from.send(FromHost::Page(page, page_lines(page, &agent)));
                            }
                        }
                        Ok(_) => {}
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
            connect: socket.map(|path| {
                Box::new(move || Some(Box::new(ControlSocket::new(path)) as Box<dyn Transport>))
                    as Connect
            }),
            ..Self::default()
        }
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
                "No host to ask for {NAME}. Start one with `openagents studio up` or \
                 `coder host serve --control`."
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
        let messages: Vec<FromHost> = self
            .worker
            .as_ref()
            .map(|w| w.from.try_iter().collect())
            .unwrap_or_default();
        for message in messages {
            match message {
                FromHost::View(view) => {
                    self.trouble = None;
                    match view {
                        Some(view) => self.view = Some(*view),
                        None => {
                            if self.view.take().is_some() || self.notes.is_empty() {
                                self.note(&format!(
                                    "The host has no agent named {NAME}. Make her with \
                                     `openagents agent new {NAME}`."
                                ));
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
                    self.note(&format!(
                        "The host does not answer: {why}. Start it with `openagents studio up`."
                    ));
                    self.trouble = Some(why);
                }
            }
        }
        self.follow(terminal);
        self.drive(terminal, Instant::now());
    }

    /// Acts on what the host asks of this window: a stop releases her
    /// pane, and a step for a typist is typed once.
    fn follow(&mut self, terminal: &mut terminal_gfx::Overlay) {
        let Some(view) = self.view.clone() else {
            return;
        };
        if view.release > self.released {
            self.released = view.release;
            if let Some(id) = self.pane {
                if self.job.take().is_some() {
                    terminal.send_to(id, &[0x03]);
                }
                if let Some(pane) = terminal.panes.get_mut(&id) {
                    pane.typist = None;
                    pane.render_revision += 1;
                }
            }
            self.note(&format!("{NAME} was stopped: her pane is yours again."));
        }
        let Some(step) = view.run.filter(|s| s.typist) else {
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

    fn start_job(&mut self, terminal: &mut terminal_gfx::Overlay, step: wire::Step) {
        let mut lines = VecDeque::new();
        let usable = self
            .pane
            .and_then(|id| terminal.panes.get(&id))
            .is_some_and(|pane| !pane.ended && pane.session.exited.is_none());
        if !usable {
            self.pane = terminal.open_typist(NAME);
            self.pane_cwd = None;
            if self.pane.is_none() {
                let why = terminal
                    .notice
                    .clone()
                    .unwrap_or_else(|| "the terminal did not start".into());
                self.report(
                    step.step,
                    wire::Ran {
                        lost: Some(why),
                        ..wire::Ran::default()
                    },
                );
                return;
            }
        }
        if self.pane_cwd.as_deref() != Some(step.cwd.as_str()) {
            lines.push_back(format!(
                "cd {} && export PAGER=cat GIT_PAGER=cat",
                quoted(&step.cwd)
            ));
            self.pane_cwd = Some(step.cwd.clone());
        }
        let Some(id) = self.pane else { return };
        terminal.show_pane(id);
        if let Some(pane) = terminal.panes.get_mut(&id) {
            // Her own pane: she takes it up again for a new command.
            pane.typist = Some(NAME.into());
            pane.taken_back = false;
            pane.render_revision += 1;
        }
        lines.push_back(step.command);
        self.job = Some(Job {
            step: step.step,
            lines,
            phase: Phase::Ready {
                since: Instant::now(),
            },
        });
    }

    fn report(&mut self, step: u64, ran: wire::Ran) {
        self.send(Operation::AgentRan {
            agent: NAME.into(),
            step,
            ran,
        });
    }

    /// Types, waits, and reads for the job under way.
    fn drive(&mut self, terminal: &mut terminal_gfx::Overlay, now: Instant) {
        let Some(job) = &mut self.job else { return };
        let step = job.step;
        let lost = |why: &str| wire::Ran {
            lost: Some(why.into()),
            ..wire::Ran::default()
        };
        let Some(id) = self.pane else {
            self.job = None;
            self.report(step, lost("she has no pane"));
            return;
        };
        let Some(pane) = terminal.panes.get_mut(&id) else {
            self.job = None;
            self.report(step, lost("you closed her terminal"));
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
            return;
        }
        if pane.ended || pane.session.exited.is_some() {
            self.job = None;
            self.report(step, lost("her terminal ended"));
            return;
        }
        let blocks = &pane.session.blocks;
        let newest = blocks.records.back().map(|b| b.id);
        let mut send: Vec<u8> = Vec::new();
        let mut done: Option<wire::Ran> = None;
        match &mut job.phase {
            Phase::Ready { since } => {
                if blocks.at_prompt || since.elapsed() > READY_WAIT {
                    match job.lines.front() {
                        Some(line) => {
                            job.phase = Phase::Typing {
                                chars: line.chars().collect(),
                                at: 0,
                                next: now,
                                seen: newest,
                            };
                        }
                        None => done = Some(lost("nothing to type")),
                    }
                }
            }
            Phase::Typing {
                chars,
                at,
                next,
                seen,
            } => {
                while *at < chars.len() && *next <= now {
                    let mut buffer = [0u8; 4];
                    send.extend_from_slice(chars[*at].encode_utf8(&mut buffer).as_bytes());
                    *at += 1;
                    *next += TYPE_EVERY;
                }
                if *at == chars.len() {
                    send.push(b'\r');
                    job.phase = Phase::Waiting { seen: *seen };
                }
            }
            Phase::Waiting { seen } => {
                let finished = blocks
                    .records
                    .iter()
                    .filter(|b| seen.is_none_or(|seen| b.id > seen))
                    .find(|b| b.status.is_some());
                if let Some(block) = finished {
                    let status = block.status.unwrap_or(-1);
                    // A block keeps the head of a long output; the summary
                    // a test run ends with is on the screen.
                    let output = if block.truncated {
                        format!(
                            "{}\n[the output is cut here; the screen at its end:]\n{}",
                            block.output,
                            pane.session.vt.text()
                        )
                    } else {
                        block.output.clone()
                    };
                    job.lines.pop_front();
                    if job.lines.is_empty() {
                        done = Some(wire::Ran {
                            status: Some(status),
                            output: tail(&output, wire::MAX_OUTPUT),
                            ..wire::Ran::default()
                        });
                    } else {
                        job.phase = Phase::Ready { since: now };
                    }
                }
            }
        }
        if !send.is_empty() {
            terminal.send_to(id, &send);
        }
        if let Some(ran) = done {
            self.job = None;
            self.report(step, ran);
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
            return "owner only".into();
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
        match self.page {
            Page::Transcript => {
                let mut lines: Vec<String> = self
                    .view
                    .as_ref()
                    .map(|v| v.lines.iter().map(|l| ascii(l)).collect())
                    .unwrap_or_default();
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
        let status = match &self.view {
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
        let waiting = self.pending().is_some() || self.asking.is_some();
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
            let tone = if line.starts_with("you") {
                Intensity::ThreeQuarters
            } else if line.starts_with(&format!("{NAME}: $")) {
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
        if let Some(asking) = self.asking {
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
        let label = match self.page {
            Page::Memory => format!("MEMORY {upper} >"),
            _ => format!("ASK {upper} >"),
        };
        out.push((fit(&format!("{label} {}_", self.input)), Intensity::Full));
        let paused = self
            .view
            .as_ref()
            .is_some_and(|v| v.state == "paused" || v.state == "stopped");
        let keys = if self.asking.is_some() || self.pending().is_some() {
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
        match key {
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

fn tail(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_string();
    }
    let mut start = text.len() - max;
    while !text.is_char_boundary(start) {
        start += 1;
    }
    text[start..].to_string()
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
