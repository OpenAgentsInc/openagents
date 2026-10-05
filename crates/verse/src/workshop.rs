//! The workshop agent in Everglade, phase 1 (`docs/verse/workshop-agent.md`,
//! "Roadmap").
//!
//! One named agent, `ada`, whose record and journal live under the host's
//! root (`coder::task::agent`). She is drawn as a resident studio seat at
//! her desk ([`Workshop::seats`]), with her activity on her nameplate, and
//! walks to the station her work maps to: her desk while she thinks, the
//! Workbench while a command runs, and the Podium while a proposal waits
//! for you.
//!
//! Walk up to her and press the interact key to open her panel, which is
//! anchored to the bottom of the window ([`Workshop::rows`]): a status row
//! that always shows, her transcript, a pending proposal when one waits,
//! the input line, and the key strip. ENTER sends a request, or confirms a
//! pending proposal on an empty line; ESC rejects it, or closes the panel.
//!
//! A request runs on a worker thread through `coder::task::agent::handle`:
//! one structured model call per step on the first provider with capacity
//! (or the scripted plan `VERSE_AGENT_SCRIPT` names), and each read-only
//! command typed into a terminal pane she opens and drives, titled `driven
//! by ada`. The frame owns that pane: [`Workshop::frame`] types the command
//! a few characters a frame, waits for the shell's completion mark, and
//! hands the command's status and output block back. A key you press in
//! her pane takes it back, and she stops.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use coder::task::agent::{self, Decision, Doing, Outcome, Ran, Record, Report, Store};
use coder_access::studio as wire;
use coder_ui::theme::Intensity;
use glam::Vec3;
use terminal_gfx::layout::PaneId;

/// The phase 1 agent's name.
pub const NAME: &str = agent::DEFAULT_NAME;
/// Her desk in the workshop hall: the last of the four.
pub const DESK: u32 = 3;
/// How near her the player stands to talk to her, m.
pub const REACH: f32 = crate::zones::everglade::studio::TALK_REACH;
/// The time between two characters she types.
const TYPE_EVERY: Duration = Duration::from_millis(35);
/// How long she waits for a new pane's prompt before typing anyway.
const READY_WAIT: Duration = Duration::from_secs(5);
/// How long one command may run before she reports it lost.
const COMMAND_LIMIT: Duration = Duration::from_secs(30 * 60);
/// The most transcript lines her panel keeps.
const LINES: usize = 400;
/// The most characters the input line takes.
const INPUT_MAX: usize = 2000;
/// The scripted plan in place of a model, for an offline demo: a JSON
/// list of next actions.
pub const SCRIPT_VAR: &str = "VERSE_AGENT_SCRIPT";
/// The directory her terminal opens in when she is first made, in place
/// of the checkout Verse runs in.
pub const WORKSPACE_VAR: &str = "OPENAGENTS_AGENT_WORKSPACE";

/// What the worker tells the frame.
enum ToFrame {
    Doing(Doing),
    Line(String),
    Run(String, Sender<Ran>),
    Decide {
        command: String,
        why: String,
        answer: Sender<Decision>,
    },
    Model(String),
    Done(Report),
}

/// A proposal waiting for your CONFIRM or REJECT.
pub struct Pending {
    pub command: String,
    pub why: String,
    answer: Sender<Decision>,
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
        since: Instant,
    },
}

/// One command the worker asked the frame to run, after any setup lines.
struct Job {
    lines: VecDeque<String>,
    phase: Phase,
    reply: Sender<Ran>,
}

/// The workshop agent as this window shows and drives it.
pub struct Workshop {
    root: Option<PathBuf>,
    store: Option<Store>,
    record: Option<Record>,
    /// Why she could not be loaded.
    error: Option<String>,
    /// Her panel shows and takes the keys.
    pub open: bool,
    pub input: String,
    lines: VecDeque<String>,
    /// Lines scrolled back from the newest.
    scroll: usize,
    doing: Doing,
    headline: String,
    model: String,
    pending: Option<Pending>,
    worker: Option<Receiver<ToFrame>>,
    job: Option<Job>,
    pane: Option<PaneId>,
    /// The last request's outcome, for the status row.
    outcome: Option<Outcome>,
}

impl Default for Workshop {
    fn default() -> Self {
        Self {
            root: None,
            store: None,
            record: None,
            error: None,
            open: false,
            input: String::new(),
            lines: VecDeque::new(),
            scroll: 0,
            doing: Doing::Idle,
            headline: String::new(),
            model: String::new(),
            pending: None,
            worker: None,
            job: None,
            pane: None,
            outcome: None,
        }
    }
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// The checkout Verse runs in, or the working directory.
fn default_workspace() -> PathBuf {
    if let Some(path) = std::env::var_os(WORKSPACE_VAR).filter(|p| !p.is_empty()) {
        return PathBuf::from(path);
    }
    let here = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
    here.ancestors()
        .find(|dir| dir.join(".git").exists())
        .map_or_else(|| here.clone(), Path::to_path_buf)
}

/// `path` quoted for a POSIX shell.
fn quoted(path: &str) -> String {
    format!("'{}'", path.replace('\'', "'\\''"))
}

impl Workshop {
    /// A workshop whose agents live under `root` instead of the host's
    /// root in the home directory, as a test or a scratch host uses.
    #[must_use]
    pub fn with_root(root: PathBuf) -> Self {
        Self {
            root: Some(root),
            ..Self::default()
        }
    }

    /// Loads her record, making it the first time, and her journal's last
    /// report. Called on entering Everglade; once loaded it does nothing.
    pub fn load(&mut self) {
        if self.record.is_some() || self.error.is_some() {
            return;
        }
        let root = match self.root.clone().or_else(host_root) {
            Some(root) => root,
            None => {
                self.error = Some("no home directory for the host's root".into());
                return;
            }
        };
        let result = Store::new(&root, NAME).and_then(|store| {
            let record = store.open(&default_workspace(), now())?;
            Ok((store, record))
        });
        match result {
            Ok((store, record)) => {
                if let Ok(journal) = store.journal(agent_journal_tail()) {
                    for entry in journal {
                        if matches!(entry.kind, agent::Kind::Request) {
                            self.say(&format!("you: {}", entry.text));
                        } else if matches!(entry.kind, agent::Kind::Report) {
                            self.say(&format!("{NAME}: {}", entry.text));
                        }
                    }
                }
                self.say(&format!(
                    "{NAME} works in {}. Her journal is {}.",
                    record.workspace,
                    store.dir().join("journal.jsonl").display()
                ));
                self.store = Some(store);
                self.record = Some(record);
            }
            Err(error) => self.error = Some(error),
        }
    }

    /// Whether she exists here.
    #[must_use]
    pub fn loaded(&self) -> bool {
        self.record.is_some()
    }

    /// Whether a request is under way.
    #[must_use]
    pub fn busy(&self) -> bool {
        self.worker.is_some()
    }

    /// The proposal waiting for you, if one does.
    #[must_use]
    pub fn pending(&self) -> Option<&Pending> {
        self.pending.as_ref()
    }

    /// The pane she drives, once she opened one.
    #[must_use]
    pub fn pane(&self) -> Option<PaneId> {
        self.pane
    }

    /// What she is doing now.
    #[must_use]
    pub fn doing(&self) -> Doing {
        self.doing
    }

    fn say(&mut self, line: &str) {
        for line in agent::ascii(line).lines() {
            self.lines.push_back(line.to_string());
        }
        let over = self.lines.len().saturating_sub(LINES);
        self.lines.drain(..over);
        self.scroll = 0;
    }

    /// Sends `text` to her as a request from you.
    pub fn ask(&mut self, text: &str) {
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        let (Some(store), Some(record)) = (self.store.clone(), self.record.clone()) else {
            self.say("She is not here yet; walk into the workshop first.");
            return;
        };
        if self.busy() {
            self.say(&format!("{NAME} is still working; wait for her report."));
            return;
        }
        self.say(&format!("you: {text}"));
        self.outcome = None;
        self.headline.clear();
        let (tx, rx) = mpsc::channel();
        let text = text.to_string();
        let spawned = std::thread::Builder::new()
            .name("verse-workshop-agent".into())
            .spawn(move || work(&store, &record, &text, &tx));
        match spawned {
            Ok(_) => {
                self.worker = Some(rx);
                self.doing = Doing::Thinking;
            }
            Err(error) => self.say(&format!("She could not start: {error}")),
        }
    }

    /// Answers the pending proposal.
    pub fn decide(&mut self, decision: Decision) {
        if let Some(pending) = self.pending.take() {
            let word = match decision {
                Decision::Confirm => "confirmed",
                Decision::Reject => "rejected",
            };
            self.say(&format!("you {word}: {}", pending.command));
            let _ = pending.answer.send(decision);
        }
    }

    /// Takes what the worker sent and drives her pane, once a frame.
    pub fn frame(&mut self, terminal: &mut terminal_gfx::Overlay) {
        let mut ended = false;
        if let Some(worker) = &self.worker {
            let messages: Vec<ToFrame> = worker.try_iter().collect();
            if messages.is_empty() && self.job.is_none() && self.pending.is_none() {
                // A worker that ended without a report hung up.
                if let Err(mpsc::TryRecvError::Disconnected) = worker.try_recv() {
                    ended = true;
                }
            }
            for message in messages {
                match message {
                    ToFrame::Doing(doing) => self.doing = doing,
                    ToFrame::Line(line) => self.say(&format!("{NAME}: {line}")),
                    ToFrame::Model(model) => self.model = model,
                    ToFrame::Decide {
                        command,
                        why,
                        answer,
                    } => {
                        self.pending = Some(Pending {
                            command,
                            why,
                            answer,
                        });
                    }
                    ToFrame::Run(command, reply) => self.start_job(terminal, command, reply),
                    ToFrame::Done(report) => {
                        self.headline = report.headline;
                        self.outcome = Some(report.outcome);
                        ended = true;
                    }
                }
            }
        }
        if ended {
            self.worker = None;
            self.pending = None;
            if let Some(job) = self.job.take() {
                let _ = job.reply.send(Ran::Lost("the request ended".into()));
            }
        }
        self.drive(terminal, Instant::now());
    }

    fn start_job(
        &mut self,
        terminal: &mut terminal_gfx::Overlay,
        command: String,
        reply: Sender<Ran>,
    ) {
        let mut lines = VecDeque::new();
        let usable = self
            .pane
            .and_then(|id| terminal.panes.get(&id))
            .is_some_and(|pane| !pane.ended && pane.session.exited.is_none());
        if !usable {
            self.pane = terminal.open_typist(NAME);
            let Some(record) = &self.record else {
                let _ = reply.send(Ran::Lost("she has no record".into()));
                return;
            };
            if self.pane.is_none() {
                let _ = reply.send(Ran::Lost(
                    terminal
                        .notice
                        .clone()
                        .unwrap_or_else(|| "the terminal did not start".into()),
                ));
                return;
            }
            lines.push_back(format!(
                "cd {} && export PAGER=cat GIT_PAGER=cat",
                quoted(&record.workspace)
            ));
        }
        let Some(id) = self.pane else { return };
        terminal.show_pane(id);
        if let Some(pane) = terminal.panes.get_mut(&id) {
            // Her own pane: she takes it up again for a new command.
            pane.typist = Some(NAME.into());
            pane.taken_back = false;
            pane.render_revision += 1;
        }
        lines.push_back(command);
        self.job = Some(Job {
            lines,
            phase: Phase::Ready {
                since: Instant::now(),
            },
            reply,
        });
    }

    /// Types, waits, and reads for the job under way.
    fn drive(&mut self, terminal: &mut terminal_gfx::Overlay, now: Instant) {
        let Some(job) = &mut self.job else { return };
        let finish = |job: Job, ran: Ran| {
            let _ = job.reply.send(ran);
        };
        let Some(id) = self.pane else {
            if let Some(job) = self.job.take() {
                finish(job, Ran::Lost("she has no pane".into()));
            }
            return;
        };
        let Some(pane) = terminal.panes.get_mut(&id) else {
            if let Some(job) = self.job.take() {
                finish(job, Ran::Lost("you closed her terminal".into()));
            }
            return;
        };
        if pane.taken_back {
            pane.taken_back = false;
            if let Some(job) = self.job.take() {
                finish(job, Ran::TakenBack);
            }
            return;
        }
        if pane.ended || pane.session.exited.is_some() {
            if let Some(job) = self.job.take() {
                finish(job, Ran::Lost("her terminal ended".into()));
            }
            return;
        }
        let blocks = &pane.session.blocks;
        let newest = blocks.records.back().map(|b| b.id);
        let mut send: Vec<u8> = Vec::new();
        let mut done: Option<Ran> = None;
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
                        None => done = Some(Ran::Lost("nothing to type".into())),
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
                    job.phase = Phase::Waiting {
                        seen: *seen,
                        since: now,
                    };
                }
            }
            Phase::Waiting { seen, since } => {
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
                        done = Some(Ran::Exited { status, output });
                    } else {
                        job.phase = Phase::Ready { since: now };
                    }
                } else if since.elapsed() > COMMAND_LIMIT {
                    done = Some(Ran::Lost(format!(
                        "no completion mark in {} minutes",
                        COMMAND_LIMIT.as_secs() / 60
                    )));
                }
            }
        }
        if !send.is_empty() {
            terminal.send_to(id, &send);
        }
        if done.is_some() {
            self.end_job(done);
        }
    }

    fn end_job(&mut self, ran: Option<Ran>) {
        if let (Some(job), Some(ran)) = (self.job.take(), ran) {
            let _ = job.reply.send(ran);
        }
    }

    /// Her studio seat: at her desk, with what she is doing and her
    /// status on her nameplate.
    #[must_use]
    pub fn seats(&self) -> Vec<wire::Seat> {
        if self.record.is_none() {
            return Vec::new();
        }
        let (activity, station) = match self.doing {
            Doing::Idle => (wire::Activity::Idle, wire::Station::Desk),
            Doing::Thinking => (wire::Activity::Thinking, wire::Station::Desk),
            Doing::Running => (wire::Activity::Running, wire::Station::Workbench),
            Doing::Testing => (wire::Activity::Testing, wire::Station::Workbench),
            Doing::Waiting => (wire::Activity::Waiting, wire::Station::Podium),
            Doing::Done => (wire::Activity::Done, wire::Station::Desk),
            Doing::Failed => (wire::Activity::Failed, wire::Station::Desk),
        };
        vec![wire::Seat {
            seat: NAME.into(),
            role: wire::Role::Worker,
            route: self.plate_status(),
            look: "teal".into(),
            desk: DESK,
            activity,
            station,
            task: None,
            paused: false,
            spend: wire::Spend::default(),
        }]
    }

    /// The nameplate's third row: her last outcome, else her model.
    fn plate_status(&self) -> String {
        if !self.headline.is_empty() && !self.busy() {
            return self.headline.clone();
        }
        if self.pending.is_some() {
            return "needs you".into();
        }
        if !self.model.is_empty() {
            return self.model.clone();
        }
        "workshop agent".into()
    }

    /// Whether `player` stands near enough to `at`, her position, to talk.
    #[must_use]
    pub fn within_reach(player: Vec3, at: Vec3) -> bool {
        (player.x - at.x).hypot(player.z - at.z) <= REACH
    }

    /// The panel's rows, top to bottom, `cols` characters wide and `rows`
    /// rows tall: the status row, the transcript, a pending proposal, the
    /// input line, and the key strip. Every character is ASCII.
    #[must_use]
    pub fn rows(&self, cols: usize, rows: usize) -> Vec<(String, Intensity)> {
        let cols = cols.max(20);
        let fit = |text: &str| -> String {
            let ascii: String = agent::ascii(text).replace('\n', " ");
            if ascii.chars().count() <= cols {
                return ascii;
            }
            let mut cut: String = ascii.chars().take(cols.saturating_sub(3)).collect();
            cut.push_str("...");
            cut
        };
        let word = match self.doing {
            Doing::Idle => "idle",
            Doing::Thinking => "thinking",
            Doing::Running => "running",
            Doing::Testing => "testing",
            Doing::Waiting => "waiting on you",
            Doing::Done => "done",
            Doing::Failed => "failed",
        };
        let status = match (&self.error, &self.record) {
            (Some(error), _) => format!("{} | not loaded: {error}", NAME.to_uppercase()),
            (None, Some(record)) => format!(
                "{} | {word} | {} | model: {} | in {}",
                NAME.to_uppercase(),
                if self.headline.is_empty() {
                    "no report yet"
                } else {
                    &self.headline
                },
                if self.model.is_empty() {
                    "first with capacity"
                } else {
                    &self.model
                },
                record.workspace
            ),
            (None, None) => format!("{} | loading", NAME.to_uppercase()),
        };
        let mut out = vec![(fit(&status), Intensity::Full)];
        // A short panel, under the terminal's panes, drops the rule.
        let rule = rows >= 8;
        let footer = 2 + usize::from(self.pending.is_some()) + usize::from(rule);
        let room = rows.saturating_sub(1 + footer).max(1);
        let mut wrapped: Vec<String> = Vec::new();
        for line in &self.lines {
            let chars: Vec<char> = line.chars().collect();
            if chars.is_empty() {
                wrapped.push(String::new());
            }
            for chunk in chars.chunks(cols) {
                wrapped.push(chunk.iter().collect());
            }
        }
        let end = wrapped.len().saturating_sub(self.scroll.min(wrapped.len()));
        let start = end.saturating_sub(room);
        let shown = &wrapped[start..end];
        for _ in shown.len()..room {
            out.push((String::new(), Intensity::Quarter));
        }
        for line in shown {
            let tone = if line.starts_with("you") {
                Intensity::ThreeQuarters
            } else if line.starts_with(&format!("{NAME}: $")) {
                Intensity::Full
            } else {
                Intensity::Half
            };
            out.push((fit(line), tone));
        }
        if rule {
            out.push(("-".repeat(cols), Intensity::Quarter));
        }
        if let Some(pending) = &self.pending {
            out.push((
                fit(&format!("PROPOSED: {}  ({})", pending.command, pending.why)),
                Intensity::Full,
            ));
        }
        out.push((
            fit(&format!("ASK {} > {}_", NAME.to_uppercase(), self.input)),
            Intensity::Full,
        ));
        let keys = if self.pending.is_some() {
            "ENTER CONFIRM  ESC REJECT  PGUP PGDN SCROLL"
        } else if self.busy() {
            "WORKING  ESC CLOSE  PGUP PGDN SCROLL  CTRL+` HER TERMINAL (ANY KEY TAKES IT BACK)"
        } else {
            "ENTER SEND  ESC CLOSE  PGUP PGDN SCROLL"
        };
        out.push((fit(keys), Intensity::Half));
        out
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
                if self.pending.is_some() {
                    if self.input.trim().is_empty() {
                        self.decide(Decision::Confirm);
                    } else {
                        self.say("Answer the proposal first: ENTER confirms, ESC rejects.");
                    }
                } else {
                    let text = std::mem::take(&mut self.input);
                    self.ask(&text);
                }
            }
            PanelKey::Escape => {
                if self.pending.is_some() {
                    self.decide(Decision::Reject);
                } else {
                    self.open = false;
                }
            }
            PanelKey::PageUp => self.scroll = (self.scroll + 5).min(self.lines.len()),
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
}

fn host_root() -> Option<PathBuf> {
    // A test never reaches the real home.
    if cfg!(test) {
        return None;
    }
    agent::host_root()
}

fn agent_journal_tail() -> usize {
    40
}

/// The worker: one request through `coder::task::agent::handle`.
fn work(store: &Store, record: &Record, text: &str, tx: &Sender<ToFrame>) {
    let mut model: Box<dyn agent::Model> = match scripted() {
        Some(Ok(script)) => {
            let _ = tx.send(ToFrame::Model("scripted".into()));
            Box::new(script)
        }
        Some(Err(error)) => {
            let _ = tx.send(ToFrame::Line(format!("{SCRIPT_VAR}: {error}")));
            return finish(tx, Outcome::Failed, "no script");
        }
        None => match agent::LiveModel::new() {
            Ok(live) => {
                let _ = tx.send(ToFrame::Model(live.standing()));
                Box::new(Live(live, tx.clone()))
            }
            Err(error) => {
                let _ = tx.send(ToFrame::Line(format!("I have no model: {error}")));
                return finish(tx, Outcome::Failed, "no model");
            }
        },
    };
    let mut terminal = Bridge(tx.clone());
    let mut watch = Bridge(tx.clone());
    let report = agent::handle(
        store,
        record,
        text,
        model.as_mut(),
        &mut terminal,
        &mut watch,
        now,
    );
    let _ = tx.send(ToFrame::Done(report));
}

fn finish(tx: &Sender<ToFrame>, outcome: Outcome, headline: &str) {
    let _ = tx.send(ToFrame::Doing(Doing::Failed));
    let _ = tx.send(ToFrame::Done(Report {
        outcome,
        reply: String::new(),
        headline: headline.into(),
    }));
}

/// The plan `VERSE_AGENT_SCRIPT` names, when it names one.
fn scripted() -> Option<Result<agent::Scripted, String>> {
    let path = std::env::var_os(SCRIPT_VAR).filter(|p| !p.is_empty())?;
    let read = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read {}: {e}", Path::new(&path).display()))
        .and_then(|text| {
            serde_json::from_str::<Vec<agent::NextAction>>(&text).map_err(|e| e.to_string())
        })
        .map(|actions| agent::Scripted {
            actions: actions.into(),
            prompts: Vec::new(),
        });
    Some(read)
}

/// The live model, which names the model that answered.
struct Live(agent::LiveModel, Sender<ToFrame>);

impl agent::Model for Live {
    fn next(&mut self, system: &str, prompt: &str) -> Result<agent::NextAction, String> {
        let action = self.0.next(system, prompt);
        if let Some(model) = &self.0.model {
            let _ = self.1.send(ToFrame::Model(model.clone()));
        }
        action
    }
}

/// The worker's side of the frame: the terminal it types into and the
/// watch that shows its work and asks you.
struct Bridge(Sender<ToFrame>);

impl agent::Terminal for Bridge {
    fn run(&mut self, command: &str) -> Ran {
        let (reply, answer) = mpsc::channel();
        if self.0.send(ToFrame::Run(command.into(), reply)).is_err() {
            return Ran::Lost("Verse closed".into());
        }
        answer
            .recv()
            .unwrap_or_else(|_| Ran::Lost("Verse closed".into()))
    }
}

impl agent::Watch for Bridge {
    fn doing(&mut self, doing: Doing) {
        let _ = self.0.send(ToFrame::Doing(doing));
    }
    fn line(&mut self, line: &str) {
        let _ = self.0.send(ToFrame::Line(line.into()));
    }
    fn decide(&mut self, command: &str, why: &str) -> Decision {
        let (answer, decision) = mpsc::channel();
        let asked = self.0.send(ToFrame::Decide {
            command: command.into(),
            why: why.into(),
            answer,
        });
        if asked.is_err() {
            return Decision::Reject;
        }
        decision.recv().unwrap_or(Decision::Reject)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn she_loads_once_and_her_seat_follows_her_work() {
        let dir = tempfile::tempdir().unwrap();
        let mut workshop = Workshop::with_root(dir.path().join("host"));
        assert!(workshop.seats().is_empty());
        workshop.load();
        assert!(workshop.loaded(), "{:?}", workshop.error);
        assert!(dir.path().join("host/agents/ada/agent.json").is_file());
        let seat = &workshop.seats()[0];
        assert_eq!(seat.seat, "ada");
        assert_eq!(seat.desk, DESK);
        assert_eq!(seat.station, wire::Station::Desk);
        workshop.doing = Doing::Testing;
        assert_eq!(workshop.seats()[0].station, wire::Station::Workbench);
        workshop.doing = Doing::Waiting;
        assert_eq!(workshop.seats()[0].station, wire::Station::Podium);
        // A second window reads the same record.
        let mut again = Workshop::with_root(dir.path().join("host"));
        again.load();
        assert_eq!(again.record, workshop.record);
    }

    #[test]
    fn the_panel_is_ascii_anchored_rows_with_confirm_and_reject() {
        let dir = tempfile::tempdir().unwrap();
        let mut workshop = Workshop::with_root(dir.path().join("host"));
        workshop.load();
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
        assert!(rows[0].0.starts_with("ADA | idle"));
        assert!(rows[10].0.starts_with("ASK ADA > run the atif tests ? now"));
        assert!(rows[11].0.starts_with("ENTER SEND"));
        let (answer, decision) = mpsc::channel();
        workshop.pending = Some(Pending {
            command: "touch notes.txt".into(),
            why: "`touch` is not on the read-only list".into(),
            answer,
        });
        workshop.input.clear();
        let rows = workshop.rows(80, 12);
        assert!(
            rows.iter()
                .any(|(row, _)| row.starts_with("PROPOSED: touch notes.txt"))
        );
        assert!(rows[11].0.starts_with("ENTER CONFIRM  ESC REJECT"));
        // Under the terminal's panes the panel is short and keeps every row
        // that matters.
        let short = workshop.rows(80, 5);
        assert_eq!(short.len(), 5);
        assert!(short[2].0.starts_with("PROPOSED: touch notes.txt"));
        assert!(short[4].0.starts_with("ENTER CONFIRM"));
        assert!(workshop.key(PanelKey::Escape));
        assert_eq!(decision.recv().unwrap(), Decision::Reject);
        assert!(
            workshop.open,
            "ESC answered the proposal and kept the panel"
        );
        assert!(workshop.key(PanelKey::Escape));
        assert!(!workshop.open);
        assert!(!workshop.key(PanelKey::Enter));
    }

    #[test]
    fn her_seat_shows_in_the_studio_without_a_host() {
        let dir = tempfile::tempdir().unwrap();
        let mut workshop = Workshop::with_root(dir.path().join("host"));
        workshop.load();
        let mut studio = crate::zones::everglade::studio::Studio::default();
        studio.set_resident(workshop.seats());
        assert!(
            studio.seat_position(NAME).is_none(),
            "nothing shows outside Everglade"
        );
        studio.set_active(true);
        assert!(studio.seat_position(NAME).is_some());
        assert_eq!(studio.view().unwrap().seats.len(), 1);
        studio.set_active(false);
        assert!(studio.seat_position(NAME).is_none());
    }

    #[test]
    fn reach_is_measured_on_the_ground() {
        assert_eq!(NAME, crate::zones::everglade::studio::WORKSHOP_AGENT);
        let at = Vec3::new(1.0, 0.0, 1.0);
        assert!(Workshop::within_reach(Vec3::new(2.0, 5.0, 2.0), at));
        assert!(!Workshop::within_reach(Vec3::new(4.0, 0.0, 1.0), at));
    }
}
