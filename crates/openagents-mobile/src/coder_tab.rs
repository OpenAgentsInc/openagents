//! The Coder tab: conversations with Coder, on the phone and on your
//! computers.
//!
//! The tab opens on a new chat, ready to type: a composer with the cursor
//! in it, what the message will start (the basic Coder, or Coder on a
//! connected computer and in which of its workspaces), and a few suggested
//! actions above the field, each from what the phone knows: the computer's
//! other workspaces, the newest chats to continue, and connecting a computer
//! when none is added. When a computer this device may operate is ready,
//! the composer targets it; **Chat here instead** and **Start on …**
//! switch between it and the basic Coder, from the screen's controls and
//! never from the message text.
//!
//! Previous chats sit behind the menu button at the top left, newest message
//! first: basic conversations and every Coder task on the computers, painted
//! from the kept list at once while the computers are read again. No other
//! harness's sessions show; a session Coder delegated to Claude Code, Codex,
//! OpenCode, or Devin belongs inside the Coder chat that delegated it,
//! through the task's own transcript.
//!
//! A basic conversation is a hosted chat that needs no computer
//! ([`BasicChats`]), streamed over NIP-CJ. From one the person can run Coder
//! on a connected computer, which starts a task there with the conversation
//! so far, or is sent to connect one.
//!
//! A computer-backed chat is a NIP-HOST `task.create` on the chosen
//! computer, the same
//! operation as the Computers surface's Order work; with the host's
//! auto-start policy on, the host runs Coder's engine on it right away. An
//! open chat reads the task's ATIF transcript through the computer's
//! read-only history observer (its `coder` source) and polls it while the
//! task runs; the host's signed activity summaries say whether it is
//! running.
//!
//! Everything sent to an open chat is a NIP-HOST `task.command` kept in the
//! durable [`Outbox`] until the computer answers, so it survives a relaunch
//! and bad connectivity and never runs twice. A command that cannot reach
//! its computer leaves the computer a relay nudge, and goes again as soon as
//! the computer answers with fresh presence. The composer's one action
//! follows the task's state and this device's `operate` right, never the
//! text: in a finished chat it sends a follow-up that continues the same
//! task; while Coder works it queues the message for the next turn; and
//! when Coder asked a question it answers it. A long press on the send
//! control offers the other ways to send while Coder works: queue for the
//! next turn, steer a turn that has not started, or stop and send (the
//! engine's emulated steering, chosen explicitly). **Stop** interrupts the
//! current turn. An approval request adds **Approve** and **Deny**.
//!
//! **Edit queue** opens the computer's queue for the chat (NIP-HOST
//! `task.queue`) under an edit lease this device renews while the panel is
//! open, so nothing runs a message being edited: edit, move up, send now,
//! or remove this device's own queued messages.

use std::sync::{Arc, Mutex};

use crate::basic_chats::{BasicChats, Tail, handoff};
use crate::basic_coder::Role as TurnRole;
use crate::chats::{Chats, Head};
use crate::cli_run::{self, RemoteCli};
use crate::coder_list::{List, Row, Store};
use crate::conversation::{Conversation, Pending};
use crate::outbox::{Attempt, Draft, Outbox};
use crate::router::{Context, Meta, Offer, RunsOn, Screen};
use crate::transcripts::Transcripts;
use coder_computers::{
    Action, Capabilities, Computers, Denial, HostRecord, HostStatus, OfflineCause, Platform,
    Snapshot, authority,
};
use coder_host::{CommandAction, QueueEdit, TaskQueue};
use nostr::activity_summary::{ActivitySummary, Attention, Phase, SubjectKind};
use rust_native::layout::source;
use rust_native::style::{Color, Space, Style, TextAlign, TextWeight};
use rust_native::{
    Activation, Axis, ComposerChoice, Element, Glyph, Icon, MessageRole, Node, TextRole,
    ValidatedView, View,
};
use serde::{Deserialize, Serialize};

/// The largest message, as NIP-HOST `task.create` allows.
const MAX_PROMPT_BYTES: usize = 16 * 1024;
const SHOWN_TASKS: usize = 50;
/// The most basic conversations the list shows.
const SHOWN_TALKS: usize = 50;
/// The newest chats a new chat offers to continue.
const SUGGESTED_CHATS: usize = 2;
/// The most other workspaces a new chat on a computer offers.
const SUGGESTED_WORKSPACES: usize = 3;

/// One chat in the previous chats: its last message time and summary time,
/// for ordering, its title, and its row.
struct Recent {
    last: Option<u64>,
    updated: u64,
    title: String,
    row: Node<Intent>,
}

/// The key of a workspace of a computer in the list's record of when this
/// device last started a chat there.
fn used_key(host: &str, workspace: &str) -> String {
    format!("{host} {workspace}")
}

/// At most `limit` characters of `text`'s first line, with an ellipsis when
/// cut.
fn clip(text: &str, limit: usize) -> String {
    let line = text.lines().next().unwrap_or_default().trim();
    if line.chars().count() <= limit {
        return line.to_owned();
    }
    let mut clipped: String = line.chars().take(limit.saturating_sub(1)).collect();
    clipped.push('…');
    clipped
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Intent {
    /// Open or close the new chat's target selector.
    Pick,
    /// Start the next chat on this computer, or, with none, with the basic
    /// Coder in the cloud.
    Target {
        host: Option<String>,
    },
    Open {
        host: String,
        task: String,
    },
    Back,
    Earlier,
    Stop,
    /// Close any chat and show a new one, ready to type.
    NewChat,
    /// Show the previous chats.
    Menu,
    /// Start the next chat in this workspace of the chosen computer.
    Workspace {
        label: String,
    },
    /// Open the chat's queue on the computer, taking its edit lease.
    EditQueue,
    /// Close the queue and give the lease up.
    DoneQueue,
    /// Put a queued message's text in the composer to edit it.
    EditQueued {
        command: String,
    },
    RemoveQueued {
        command: String,
    },
    /// Send a queued message now: stop the turn and continue with it.
    SendQueuedNow {
        command: String,
    },
    MoveQueuedUp {
        command: String,
    },
    /// Answer Coder's request for approval.
    Approve,
    Deny,
    /// Open a basic conversation.
    OpenTalk {
        id: String,
    },
    /// Run Coder on the chosen computer with the open conversation.
    RunCoder,
    /// Send the person to connect a computer.
    ConnectComputer,
    /// Ask the basic Coder again for the last message's reply.
    Retry,
    /// Open the chat of the task the open conversation started.
    OpenSpawned,
    /// Open the screen an offer under the last reply names.
    OpenScreen {
        screen: Screen,
    },
    /// Send the last reply's suggested follow-up at `index` as a message.
    Followup {
        index: usize,
    },
    /// Run the read-only command the last reply's offer at `index` proposes.
    RunCli {
        index: usize,
    },
    /// Say the last reply, a prepared answer, was wrong: shows what would
    /// be sent, and asks.
    WrongAnswer,
    /// Send the wrong-answer report the chat showed.
    SendWrongAnswer,
    CancelWrongAnswer,
}

/// A screen of another tab the host should show, once, or something the
/// host should do for the tab.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Go {
    /// Account > Computers, to connect a computer.
    Computers,
    /// The Wallet tab.
    Wallet,
    /// Account > Identity keys.
    Keys,
    /// Account > Playtest.
    Playtest,
    /// Report a problem, for the chat on screen.
    Report,
    /// File the wrong-answer report the person confirmed: the host sends
    /// `report_wrong_answer` with the Verse world key and device facts.
    WrongAnswer,
    /// The Verse tab, walked into the Gym before its EVALS board: the host
    /// sends the world `go_evals`.
    VerseGym,
}

impl Go {
    fn of(screen: Screen) -> Self {
        match screen {
            Screen::Wallet => Go::Wallet,
            Screen::Computers => Go::Computers,
            Screen::Keys => Go::Keys,
            Screen::Playtest => Go::Playtest,
            Screen::Report => Go::Report,
            Screen::VerseGym => Go::VerseGym,
        }
    }
}

/// A command the person ran from an offer, and what came of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CliOutcome {
    /// The command's output, one line each.
    Output(Vec<String>),
    /// It is running on the named computer.
    Running(String),
    /// Why it did not run.
    Refused(String),
}

/// Where a wrong-answer report stands.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Flag {
    /// Showing what would be sent, waiting for **Send**.
    Confirm,
    /// The host is sending it.
    Sending,
    /// Filed: the line to show.
    Filed(String),
    Failed(String),
}

/// Another way to send the composer's text while Coder works, which a long
/// press on the send control offers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Choice {
    /// Queue the message for the next turn: the send control's own action.
    Queue,
    /// Replace the instructions of a turn that has not started.
    SteerNow,
    /// Stop the running turn and continue with the message: the engine's
    /// emulated steering.
    StopAndSend,
}

impl Choice {
    fn label(self) -> &'static str {
        match self {
            Choice::Queue => "Queue for next turn",
            Choice::SteerNow => "Steer now",
            Choice::StopAndSend => "Stop and send",
        }
    }

    fn command(self) -> (CommandAction, bool) {
        match self {
            Choice::Queue => (CommandAction::Queue, false),
            Choice::SteerNow => (CommandAction::Steer, false),
            Choice::StopAndSend => (CommandAction::Steer, true),
        }
    }

    /// The choices a busy chat offers: a turn that has not started takes new
    /// instructions; a running one stops for the message.
    pub(crate) fn offered(phase: Option<Phase>) -> Vec<Choice> {
        match phase {
            Some(Phase::Queued) => vec![Choice::Queue, Choice::SteerNow],
            None | Some(Phase::Running) => vec![Choice::Queue, Choice::StopAndSend],
            _ => vec![],
        }
    }
}

/// The least time between two reads of an open chat's queue while its
/// summary does not move, in seconds.
const QUEUE_READ_EVERY: u64 = 15;
/// How often a running chat's computer is asked for its newest chats, so a
/// turn's transcript is found as soon as it starts, in seconds.
const HEAD_EVERY: u64 = 3;
/// The same, on a direct connection whose computer nudges the phone when
/// its chat list changes.
const HEAD_NUDGED_EVERY: u64 = 30;
/// How often a running chat's transcript is kept for the next opening, in
/// seconds; an ended one is kept at once.
const KEEP_EVERY: u64 = 5;
/// How long a sent message shows before the transcript does, at most, in
/// seconds.
const ECHO_FOR: u64 = 30 * 60;
/// How often the open queue panel renews its edit lease, in seconds; the
/// host holds a lease for 60.
const LEASE_RENEW_EVERY: u64 = 20;

/// An open chat: one task on one computer.
struct Open {
    host: String,
    task: String,
    conversation: Option<Conversation>,
    /// The chat whose transcript is shown: a later turn is a newer chat.
    chat: Option<String>,
    /// The summary sequence last seen, to notice a new turn.
    seen: Option<u64>,
    /// The computer's queue for this task as last read, and the summary
    /// sequence and time it was read at.
    queue: Option<TaskQueue>,
    listed: Option<(u64, u64)>,
    /// The computer does not list queues, as an older host.
    unlisted: bool,
    /// The queue panel is open, holding the edit lease this device last
    /// renewed at this time.
    leased_at: Option<u64>,
    /// The queued message the composer edits.
    editing: Option<String>,
    /// A read of the computer's newest chats is wanted, as when the task's
    /// summary moved; when one last started; and the read that started
    /// after the summary last moved.
    head_wanted: bool,
    head_at: u64,
    head_round: Option<u64>,
    /// The ended summary sequence whose transcript was read after the
    /// summary said so, and the read that is to show it.
    settled: Option<u64>,
    settle_read: Option<u64>,
    /// The transcript version last kept for the next opening, and when.
    kept: (u64, u64),
    /// A reader for each delegate session the transcript's notes name, by
    /// session: its rows show under the note, read-only.
    delegates: std::collections::BTreeMap<String, Conversation>,
}

/// A message this device sent, shown in its chat until the transcript
/// shows it.
struct Echo {
    task: String,
    key: u64,
    text: String,
    /// The command's outbox ID; a new chat's first message has none.
    command: Option<String>,
    queued: bool,
    /// How many of the user's messages with this text the chat showed
    /// when it was sent.
    shown: usize,
    sent_at: u64,
}

pub struct CoderTab {
    instance: String,
    revision: u64,
    current: Option<ValidatedView<Intent>>,
    selected: Option<String>,
    notice: Option<String>,
    composers: u64,
    /// The chats list as last seen, with the first line and send time of
    /// each chat this device started; it survives a relaunch.
    list: Store,
    open: Option<Open>,
    /// The previous chats show over the tab.
    drawer: bool,
    outbox: Outbox,
    /// The current composer's choices: each token this tab minted and what
    /// it sends.
    choices: Vec<(String, Choice)>,
    /// Each chat's transcript as last shown; it survives a relaunch.
    transcripts: Transcripts,
    echoes: Vec<Echo>,
    echoed: u64,
    /// The host's transcript layout reads a chat's rows from Rust
    /// (`rust_native::layout::source`), so they stay out of the view.
    pulled: bool,
    /// Conversations with the basic Coder.
    basic: BasicChats,
    /// The open basic conversation.
    talk: Option<String>,
    /// Whether a new chat starts on a computer, as the person chose it; with
    /// no choice, on a computer when one is ready.
    target: Option<bool>,
    /// The new chat's target selector is open.
    picking: bool,
    /// The tab shows: the app opens on it.
    shown: bool,
    /// The workspace the person chose for a new chat on the chosen
    /// computer.
    workspace: Option<String>,
    /// A screen of another tab to show, taken by the next packet.
    go: Option<Go>,
    /// The most turns the open conversation shows; fewer when a long one
    /// outgrows one view.
    talk_turns: usize,
    /// The app's version and build, as `1.0.0 (19)`, for the router's
    /// context.
    app_build: Option<String>,
    /// The last command run from an offer: its conversation, its words, and
    /// what came of it.
    cli: Option<(String, Vec<String>, CliOutcome)>,
    /// Runs a command on a connected computer, when the phone can reach
    /// computers.
    remote: Option<Arc<dyn RemoteCli>>,
    /// Where the running command's outcome lands.
    running: Option<Arc<Mutex<Option<CliOutcome>>>>,
    /// A wrong-answer report for a conversation's last reply.
    flag: Option<(String, Flag)>,
    /// The wrong-answer report the host is to file, taken once.
    flagged: Option<playtest::report::SharedChat>,
    /// Debug builds' screenshot script: messages to send one reply at a
    /// time, and `!run` (the first offered command) or `!wrong` (Wrong
    /// answer) steps.
    script: std::collections::VecDeque<String>,
}

/// The most turns an open basic conversation shows at first.
const TALK_TURNS: usize = 200;

impl CoderTab {
    pub fn new(instance: String) -> Self {
        Self {
            instance,
            revision: 0,
            current: None,
            selected: None,
            notice: None,
            composers: 1,
            list: Store::open(None),
            open: None,
            drawer: false,
            outbox: Outbox::open(None),
            choices: Vec::new(),
            transcripts: Transcripts::open(None),
            echoes: Vec::new(),
            echoed: 0,
            pulled: false,
            basic: BasicChats::empty(),
            talk: None,
            target: None,
            picking: false,
            shown: true,
            workspace: None,
            go: None,
            talk_turns: TALK_TURNS,
            app_build: None,
            cli: None,
            remote: None,
            running: None,
            flag: None,
            flagged: None,
            script: std::collections::VecDeque::new(),
        }
    }

    /// Play `steps` in the chat at launch, for simulator screenshots.
    /// Honored only in debug builds.
    pub fn with_script(mut self, steps: Vec<String>) -> Self {
        if cfg!(debug_assertions) {
            self.script = steps.into();
        }
        self
    }

    /// The next screenshot-script step, once the last reply ended.
    fn play(&mut self, computers: Option<&Computers>) {
        if self.talk.as_ref().is_some_and(|id| self.basic.busy(id)) {
            return;
        }
        let Some(step) = self.script.pop_front() else {
            return;
        };
        let now = unix_now();
        match (step.as_str(), self.talk.clone()) {
            ("!wrong", Some(id)) => {
                if self.basic.wrong_answer(&id).is_some() {
                    self.flag = Some((id, Flag::Confirm));
                }
            }
            ("!run", Some(id)) => {
                let offer = self.basic.last_meta(&id).and_then(|meta| {
                    meta.offers
                        .into_iter()
                        .find(|offer| matches!(offer, Offer::Cli { .. }))
                });
                if let Some(Offer::Cli { argv, runs_on }) = offer {
                    self.run_offer(id, argv, runs_on, computers);
                }
            }
            (text, talk) => {
                self.basic.set_context(self.router_context(computers));
                match talk {
                    Some(id) => {
                        self.basic.send(&id, text, now);
                    }
                    None => self.talk = self.basic.start(text, now),
                }
            }
        }
    }

    /// Run offered commands that run on a computer with `remote`.
    pub(crate) fn with_remote_cli(mut self, remote: Option<Arc<dyn RemoteCli>>) -> Self {
        self.remote = remote;
        self
    }

    /// Run a command an offer proposed, after the person's Run tap: the
    /// phone's own core answers `computer` commands; a command that runs
    /// on the computer starts there off this thread, and its card shows
    /// "Running on …" until [`Self::poll_cli`] reads the outcome.
    fn run_offer(
        &mut self,
        id: String,
        argv: Vec<String>,
        runs_on: RunsOn,
        computers: Option<&Computers>,
    ) {
        if self.running.is_some() {
            return;
        }
        let local = runs_on == RunsOn::ThisDevice || argv.first().is_some_and(|g| g == "computer");
        let outcome = if local {
            run_cli(&argv, runs_on, computers, self.selected.as_deref())
        } else {
            match cli_run::target(&argv, computers, self.selected.as_deref()) {
                Err(refused) => refused,
                Ok((host, label)) => match self.remote.clone() {
                    None => CliOutcome::Refused(
                        "This phone can't reach your computers right now.".into(),
                    ),
                    Some(remote) => {
                        self.running = Some(cli_run::spawn(
                            remote,
                            host,
                            label.clone(),
                            cli_run::command(&argv),
                            crate::wake::ring,
                        ));
                        CliOutcome::Running(label)
                    }
                },
            }
        };
        self.cli = Some((id, argv, outcome));
    }

    /// Put a finished command's outcome on its card.
    fn poll_cli(&mut self) {
        let Some(slot) = &self.running else { return };
        let landed = slot.lock().unwrap_or_else(|p| p.into_inner()).take();
        if let Some(outcome) = landed {
            self.running = None;
            if let Some((_, _, shown)) = self.cli.as_mut() {
                *shown = outcome;
            }
        }
    }

    /// Tell the chat router this app's version and build, as `1.0.0 (19)`.
    pub fn with_app_build(mut self, build: Option<String>) -> Self {
        self.app_build = build;
        self
    }

    /// The open chat with OpenAgents as **Share this chat** would send it.
    pub(crate) fn shared_chat(&mut self) -> Option<playtest::report::SharedChat> {
        let id = self.talk.clone()?;
        self.basic.shared(&id)
    }

    /// The wrong-answer report the person confirmed, for the host to file.
    pub(crate) fn take_wrong_answer(&mut self) -> Option<playtest::report::SharedChat> {
        self.flagged.take()
    }

    /// What came of filing the wrong-answer report: its code, or why not.
    pub(crate) fn wrong_answer_filed(&mut self, filed: Result<crate::playtest::Row, String>) {
        let Some((_, flag)) = self.flag.as_mut() else {
            return;
        };
        *flag = match filed {
            Ok(row) => Flag::Filed(match (row.status, row.code) {
                (crate::playtest::Status::Waiting, _) => {
                    "Saved on this phone. A later build sends it to the OpenAgents team.".into()
                }
                (_, Some(code)) => format!("Sent to the OpenAgents team as {code}. Thank you."),
                (_, None) => "Sent to the OpenAgents team. Thank you.".into(),
            }),
            Err(why) => Flag::Failed(why),
        };
    }

    /// What the next basic turn tells the worker: whether a computer is
    /// ready, and the build. No computer's name or workspace.
    fn router_context(&self, computers: Option<&Computers>) -> Context {
        Context {
            computer_ready: matches!(self.availability(computers), Availability::Ready(_)),
            app_build: self.app_build.clone(),
        }
    }

    /// Keep basic conversations in `basic`.
    pub fn with_basic(mut self, basic: BasicChats) -> Self {
        self.basic = basic;
        self
    }

    /// The screen of another tab to show now, once.
    pub fn take_go(&mut self) -> Option<Go> {
        self.go.take()
    }

    /// A basic reply is streaming: ask for packets quickly.
    /// The tab shows, or another tab does: the basic Coder's relay
    /// connection stays open while the tab shows.
    pub fn show(&mut self, shown: bool) {
        self.shown = shown;
        if shown {
            self.basic.warm();
        } else {
            self.basic.rest();
        }
    }

    /// The app came to the foreground or went to the background: open the
    /// basic Coder's relay connection when the tab shows, and close it in
    /// the background once no reply waits on it.
    pub fn lifecycle(&mut self, active: bool) {
        if !active {
            self.basic.rest();
        } else if self.shown {
            self.basic.warm();
        }
    }

    pub fn streaming(&self) -> bool {
        // A screenshot script waiting for its next step keeps packets coming.
        self.basic.streaming() || !self.script.is_empty()
    }

    /// Whether a chat, basic or on a computer, is open.
    pub(crate) fn in_chat(&self) -> bool {
        self.open.is_some() || self.talk.is_some()
    }

    /// Publish each chat's rows for the host's transcript layout instead of
    /// listing them in the view, so a long chat never outgrows one view.
    pub fn with_pulled_transcripts(mut self, pulled: bool) -> Self {
        self.pulled = pulled;
        self
    }

    /// Keep each chat's transcript in `transcripts`, which survives a
    /// relaunch.
    pub fn with_transcripts(mut self, transcripts: Transcripts) -> Self {
        self.transcripts = transcripts;
        self
    }

    /// Keep the chats list in `list`, which survives a relaunch.
    pub fn with_list(mut self, list: Store) -> Self {
        self.list = list;
        self
    }

    /// Keep chat commands in `outbox`, which survives a relaunch.
    pub fn with_outbox(mut self, outbox: Outbox) -> Self {
        self.outbox = outbox;
        self
    }

    /// Send every command that is due, keeping any the computer did not
    /// answer for a later try with the same ID. A refusal shows its reason.
    /// A computer that was not reached gets a relay nudge, and a command
    /// goes again as soon as the computer publishes presence after its
    /// failed try. Then keep the open chat's queue current.
    pub fn flush(&mut self, computers: Option<&mut Computers>) {
        let Some(computers) = computers else { return };
        let now = computers.snapshot().now;
        let due = {
            let snapshot = computers.snapshot();
            let awake = |host: &str| {
                snapshot
                    .host(host)
                    .and_then(|record| record.presence.as_ref())
                    .map(|received| received.presence.observed_at)
            };
            self.outbox.due_or_awake(now, &awake)
        };
        let mut unreached: Vec<String> = Vec::new();
        for pending in due {
            let attempt = match computers.command_task(&pending.host, &pending.command) {
                Ok(()) => Attempt::Answered,
                Err(refusal) if unreachable(&refusal) => Attempt::Unreached,
                Err(refusal) => Attempt::Refused(refusal.reason()),
            };
            match &attempt {
                Attempt::Refused(reason) => {
                    self.notice = Some(reason.clone());
                    let refused = Some(&pending.command.command);
                    self.echoes.retain(|echo| echo.command.as_ref() != refused);
                }
                Attempt::Unreached if !unreached.contains(&pending.host) => {
                    unreached.push(pending.host.clone());
                }
                Attempt::Answered if pending.command.action == CommandAction::Queue => {
                    // Read the queue again to show the message there.
                    if let Some(open) = self.open.as_mut() {
                        open.listed = None;
                    }
                }
                _ => {}
            }
            self.outbox.settle(&pending.command.command, &attempt, now);
        }
        // Best effort: the command itself stays in the outbox.
        for host in unreached {
            let _ = computers.nudge_host(&host);
        }
        self.keep_queue(computers, now);
    }

    /// Read the open chat's queue when its summary moved or it is old, and
    /// renew the edit lease while the queue panel is open.
    fn keep_queue(&mut self, computers: &mut Computers, now: u64) {
        let Some(open) = self.open.as_mut() else {
            return;
        };
        if open.unlisted {
            return;
        }
        let summary = Self::summary(computers.snapshot(), &open.host, &open.task);
        let sequence = summary.map_or(0, |summary| summary.sequence);
        let busy = summary.is_none_or(|summary| Self::running(summary.phase));
        let edit = match open.leased_at {
            Some(at) if now.saturating_sub(at) >= LEASE_RENEW_EVERY => QueueEdit::Lease {},
            Some(_) => return,
            None if !busy && open.queue.as_ref().is_none_or(|q| q.items.is_empty()) => return,
            None if open.listed.is_some_and(|(seen, at)| {
                seen == sequence && now.saturating_sub(at) < QUEUE_READ_EVERY
            }) =>
            {
                return;
            }
            None => QueueEdit::List {},
        };
        let (host, task) = (open.host.clone(), open.task.clone());
        let result = computers.queue_task(&host, &task, &edit);
        let Some(open) = self.open.as_mut() else {
            return;
        };
        match result {
            Ok(queue) => {
                if matches!(edit, QueueEdit::Lease {}) {
                    open.leased_at = Some(now);
                }
                open.queue = Some(queue);
                open.listed = Some((sequence, now));
            }
            Err(refusal) => match refusal_code(&refusal) {
                // An older host has no queue to read.
                Some(coder_host::Code::Malformed | coder_host::Code::Unsupported) => {
                    open.unlisted = true;
                }
                Some(coder_host::Code::Conflict) => {
                    open.leased_at = None;
                    open.editing = None;
                    self.notice = Some("Another device is editing this queue.".into());
                }
                _ => open.listed = Some((sequence, now)),
            },
        }
    }

    /// Change the open chat's queue on the computer and show the result.
    fn edit_queue(&mut self, edit: QueueEdit, computers: &mut Computers) {
        let Some(open) = &self.open else { return };
        let (host, task) = (open.host.clone(), open.task.clone());
        let now = computers.snapshot().now;
        let result = computers.queue_task(&host, &task, &edit);
        let Some(open) = self.open.as_mut() else {
            return;
        };
        match result {
            Ok(queue) => {
                self.notice = None;
                match edit {
                    QueueEdit::Lease {} => open.leased_at = Some(now),
                    QueueEdit::Release {} => {
                        open.leased_at = None;
                        open.editing = None;
                    }
                    _ => {}
                }
                open.queue = Some(queue);
                open.listed = Some((open.seen.unwrap_or(0), now));
            }
            Err(refusal) => {
                if matches!(edit, QueueEdit::Lease {} | QueueEdit::Release {}) {
                    open.leased_at = None;
                    open.editing = None;
                }
                self.notice = Some(match refusal_code(&refusal) {
                    Some(coder_host::Code::Conflict) if matches!(edit, QueueEdit::Lease {}) => {
                        "Another device is editing this queue.".into()
                    }
                    Some(coder_host::Code::Malformed | coder_host::Code::Unsupported) => {
                        open.unlisted = true;
                        "This computer can't edit queued messages yet.".into()
                    }
                    _ => refusal.reason(),
                });
            }
        }
    }

    /// Keep a command for the open chat and try to send it now.
    fn command(
        &mut self,
        action: CommandAction,
        text: &str,
        emulate: bool,
        computers: &mut Computers,
    ) -> bool {
        let Some(open) = &self.open else { return false };
        let based_on = Self::summary(computers.snapshot(), &open.host, &open.task)
            .map_or(1, |summary| summary.sequence);
        let (host, task) = (open.host.clone(), open.task.clone());
        let now = computers.snapshot().now;
        let draft = Draft {
            task: &task,
            action,
            based_on,
            text,
            emulate,
        };
        let Some(command) = self
            .outbox
            .push(&host, draft, now)
            .map(|pending| pending.command.command.clone())
        else {
            self.notice = Some("Too many messages are waiting to send. Try again later.".into());
            return false;
        };
        // The message shows in the chat at once; an interrupt is not one.
        if action != CommandAction::Interrupt {
            self.echo(
                &task,
                text,
                Some(command),
                action == CommandAction::Queue,
                now,
            );
        }
        self.notice = None;
        self.flush(Some(computers));
        true
    }

    /// Show `text` in the chat of `task` until its transcript does.
    fn echo(&mut self, task: &str, text: &str, command: Option<String>, queued: bool, now: u64) {
        let shown = self
            .open
            .as_ref()
            .filter(|open| open.task == task)
            .and_then(|open| open.conversation.as_ref())
            .map_or(0, |conversation| conversation.sent(text));
        self.echoed += 1;
        self.echoes.push(Echo {
            task: task.to_owned(),
            key: self.echoed,
            text: text.trim().to_owned(),
            command,
            queued,
            shown,
            sent_at: now,
        });
    }

    /// Stop showing sent messages the open chat's transcript now shows, and
    /// any shown for too long.
    fn settle_echoes(&mut self, now: u64) {
        let open = self.open.as_ref();
        self.echoes.retain(|echo| {
            if now.saturating_sub(echo.sent_at) > ECHO_FOR {
                return false;
            }
            let conversation = open
                .filter(|open| open.task == echo.task)
                .and_then(|open| open.conversation.as_ref());
            conversation.is_none_or(|conversation| conversation.sent(&echo.text) <= echo.shown)
        });
    }

    /// Whether the open chat is changing on its own, so the host should
    /// ask for a new packet sooner: its task runs, its ending has not been
    /// read yet, or a message it sent does not show yet.
    pub fn live(&self, computers: Option<&Computers>) -> bool {
        if self.basic.streaming() || self.running.is_some() {
            return true;
        }
        let Some(open) = &self.open else {
            return false;
        };
        let summary = computers.and_then(|c| Self::summary(c.snapshot(), &open.host, &open.task));
        summary.is_none_or(|summary| {
            Self::running(summary.phase) || open.settled != Some(summary.sequence)
        }) || self.echoes.iter().any(|echo| echo.task == open.task)
            || open
                .conversation
                .as_ref()
                .is_some_and(Conversation::loading)
    }

    /// Whether the tab shows a problem notice now, for the playtest
    /// session log (which records that one showed, never its words).
    pub(crate) fn notice_shown(&self) -> bool {
        self.notice.is_some()
    }

    /// The host and task of the open chat.
    pub(crate) fn open_task(&self) -> Option<(String, String)> {
        self.open
            .as_ref()
            .map(|open| (open.host.clone(), open.task.clone()))
    }

    /// The computers this device may order work on, in snapshot order.
    fn hosts(computers: &Computers) -> Vec<&HostRecord> {
        computers
            .snapshot()
            .hosts
            .iter()
            .filter(|host| computers.can_operate(&host.key))
            .collect()
    }

    fn chosen<'a>(&self, computers: &'a Computers) -> Option<&'a HostRecord> {
        let hosts = Self::hosts(computers);
        self.selected
            .as_ref()
            .and_then(|key| hosts.iter().find(|host| &host.key == key).copied())
            .or_else(|| hosts.first().copied())
    }

    /// The workspace a new chat on `host` uses: the one the person chose
    /// when the computer lists it, else the one this device used there last,
    /// else `openagents`, else the computer's first.
    fn workspace(&self, host: &HostRecord) -> Option<String> {
        let listed = host.workspaces.as_ref()?;
        if let Some(chosen) = self.workspace.as_ref().filter(|w| listed.contains(w)) {
            return Some(chosen.clone());
        }
        listed
            .iter()
            .filter_map(|label| Some((self.used(&host.key, label)?, label)))
            .max()
            .map(|(_, label)| label)
            .or_else(|| listed.iter().find(|label| *label == "openagents"))
            .or_else(|| listed.first())
            .cloned()
    }

    /// When this device last started a chat in `workspace` on `host`.
    fn used(&self, host: &str, workspace: &str) -> Option<u64> {
        self.list.list.used.get(&used_key(host, workspace)).copied()
    }

    /// Whether a new chat starts on a computer: only as the person chose.
    /// By default every chat goes to OpenAgents, which dispatches Coder.
    fn on_computer(&self, computers: Option<&Computers>) -> bool {
        match self.availability(computers) {
            Availability::NotConfigured => false,
            Availability::Ready(_) => self.target.unwrap_or(false),
            Availability::Connecting(_) | Availability::Offline(_) => self.target.unwrap_or(false),
        }
    }

    /// The newest summary of `task` on `host`.
    fn summary<'a>(snapshot: &'a Snapshot, host: &str, task: &str) -> Option<&'a ActivitySummary> {
        snapshot
            .activity
            .iter()
            .filter(|s| s.subject_kind == SubjectKind::Task && s.host == host && s.subject == task)
            .max_by_key(|s| s.sequence)
    }

    fn running(phase: Phase) -> bool {
        matches!(phase, Phase::Queued | Phase::Running | Phase::Waiting)
    }

    pub fn activate(
        &mut self,
        event: &Activation,
        computers: Option<&mut Computers>,
        chats: &mut Chats,
    ) {
        let Some(intent) = self
            .current
            .as_ref()
            .and_then(|view| view.activate(event).ok())
            .cloned()
        else {
            return;
        };
        // Any other action closes the target selector.
        if intent != Intent::Pick {
            self.picking = false;
        }
        match intent {
            Intent::Pick => {
                self.notice = None;
                self.picking = !self.picking;
            }
            Intent::Target { host } => {
                self.notice = None;
                self.picking = false;
                self.target = Some(host.is_some());
                if host.is_some() {
                    self.selected = host;
                }
                // A new composer, so the field's focus follows the switch.
                self.composers += 1;
            }
            Intent::Open { host, task } => {
                self.drawer = false;
                self.talk = None;
                self.open(host, task, chats);
            }
            // Back from the previous chats: the screen under them.
            Intent::Back if self.drawer => self.drawer = false,
            // A new chat, or back from a chat: a new chat, ready to type.
            Intent::NewChat | Intent::Back => {
                self.keep(true);
                self.open = None;
                self.talk = None;
                self.drawer = false;
                self.notice = None;
                // A new composer, so the field takes the cursor again.
                self.composers += 1;
            }
            Intent::Menu => {
                self.drawer = true;
                self.notice = None;
            }
            Intent::OpenTalk { id } => {
                self.keep(true);
                self.open = None;
                self.drawer = false;
                self.notice = None;
                self.talk_turns = TALK_TURNS;
                self.talk = Some(id);
            }
            // Picking a workspace is choosing to run Coder there.
            Intent::Workspace { label } => {
                self.notice = None;
                self.workspace = Some(label);
                self.target = Some(true);
            }
            Intent::Retry => {
                self.basic
                    .set_context(self.router_context(computers.as_deref()));
                if let Some(id) = &self.talk {
                    self.basic.retry(id);
                }
            }
            Intent::ConnectComputer => self.go = Some(Go::Computers),
            Intent::OpenScreen { screen } => {
                // Only a screen an offer under the last reply names.
                let offered = self
                    .talk
                    .as_ref()
                    .and_then(|id| self.basic.last_meta(id))
                    .is_some_and(|meta| meta.offers.contains(&Offer::OpenScreen { screen }));
                if offered {
                    self.go = Some(Go::of(screen));
                }
            }
            Intent::Followup { index } => {
                let Some(id) = self.talk.clone() else { return };
                let Some(followup) = self
                    .basic
                    .last_meta(&id)
                    .and_then(|meta| meta.followups.get(index).cloned())
                else {
                    return;
                };
                self.basic
                    .set_context(self.router_context(computers.as_deref()));
                if self.basic.send(&id, &followup.label, unix_now()) {
                    self.notice = None;
                }
            }
            Intent::RunCli { index } => {
                let Some(id) = self.talk.clone() else { return };
                let Some(Offer::Cli { argv, runs_on }) = self
                    .basic
                    .last_meta(&id)
                    .and_then(|meta| meta.offers.get(index).cloned())
                else {
                    return;
                };
                self.run_offer(id, argv, runs_on, computers.as_deref());
            }
            Intent::WrongAnswer => {
                if let Some(id) = self.talk.clone()
                    && self.basic.wrong_answer(&id).is_some()
                {
                    self.flag = Some((id, Flag::Confirm));
                }
            }
            Intent::CancelWrongAnswer => self.flag = None,
            Intent::SendWrongAnswer => {
                let Some((id, Flag::Confirm)) = self.flag.clone() else {
                    return;
                };
                let Some(chat) = self.basic.wrong_answer(&id) else {
                    self.flag = None;
                    return;
                };
                self.flagged = Some(chat);
                self.flag = Some((id, Flag::Sending));
                self.go = Some(Go::WrongAnswer);
            }
            Intent::OpenSpawned => {
                let spawned = self
                    .talk
                    .as_ref()
                    .and_then(|id| self.basic.get(id))
                    .and_then(|summary| summary.coder.clone());
                if let Some(spawned) = spawned {
                    self.talk = None;
                    self.open(spawned.host, spawned.task, chats);
                }
            }
            Intent::RunCoder => {
                let Some(computers) = computers else {
                    self.go = Some(Go::Computers);
                    return;
                };
                self.run_coder(computers, chats);
            }
            Intent::Earlier => {
                if let Some(conversation) = self.open.as_ref().and_then(|o| o.conversation.as_ref())
                {
                    conversation.earlier();
                }
            }
            Intent::Stop if self.talk.is_some() => {
                if let Some(id) = self.talk.clone() {
                    self.basic.stop(&id, unix_now());
                }
            }
            Intent::Stop => {
                let Some(computers) = computers else { return };
                let reason = "Stopped from a phone.";
                self.command(CommandAction::Interrupt, reason, false, computers);
            }
            Intent::Approve | Intent::Deny => {
                let Some(computers) = computers else { return };
                let answer = if intent == Intent::Approve {
                    "Approved."
                } else {
                    "Denied."
                };
                self.command(CommandAction::Answer, answer, false, computers);
            }
            Intent::EditQueue => {
                let Some(computers) = computers else { return };
                self.edit_queue(QueueEdit::Lease {}, computers);
            }
            Intent::DoneQueue => {
                let Some(computers) = computers else { return };
                self.edit_queue(QueueEdit::Release {}, computers);
            }
            Intent::EditQueued { command } => {
                if let Some(open) = self.open.as_mut() {
                    open.editing = Some(command);
                    // A new composer takes the message as its draft.
                    self.composers += 1;
                }
            }
            Intent::RemoveQueued { command } => {
                let Some(computers) = computers else { return };
                self.edit_queue(QueueEdit::Remove { command }, computers);
            }
            Intent::SendQueuedNow { command } => {
                let Some(computers) = computers else { return };
                self.edit_queue(QueueEdit::SendNow { command }, computers);
            }
            Intent::MoveQueuedUp { command } => {
                let Some(computers) = computers else { return };
                let Some(mut order) = self.open.as_ref().and_then(|open| {
                    open.queue.as_ref().map(|queue| {
                        queue
                            .items
                            .iter()
                            .filter(|item| !item.priority)
                            .map(|item| item.command.clone())
                            .collect::<Vec<_>>()
                    })
                }) else {
                    return;
                };
                let Some(at) = order.iter().position(|item| *item == command) else {
                    return;
                };
                if at == 0 {
                    return;
                }
                order.swap(at - 1, at);
                self.edit_queue(QueueEdit::Reorder { commands: order }, computers);
            }
        }
    }

    fn open(&mut self, host: String, task: String, chats: &mut Chats) {
        self.keep(true);
        // The chat as last shown, at once, while its computer is read again.
        let (chat, conversation) = match (self.transcripts.get(&task), chats.coder_client(&host)) {
            (Some(cached), Some(client)) => (
                Some(cached.chat.id.clone()),
                Some(Conversation::resume(
                    chats.runtime(),
                    client,
                    cached.chat.clone(),
                    Some(cached),
                )),
            ),
            _ => (None, None),
        };
        self.open = Some(Open {
            host,
            task,
            conversation,
            chat,
            seen: None,
            queue: None,
            listed: None,
            unlisted: false,
            leased_at: None,
            editing: None,
            head_wanted: true,
            head_at: 0,
            head_round: None,
            settled: None,
            settle_read: None,
            kept: (0, 0),
            delegates: std::collections::BTreeMap::new(),
        });
        self.attach(chats);
    }

    /// Keep the open chat's transcript for its next opening: when `now`, or
    /// when it changed and was last kept a while ago.
    fn keep(&mut self, now: bool) {
        let at = unix_now();
        let Some(open) = self.open.as_mut() else {
            return;
        };
        let Some(conversation) = &open.conversation else {
            return;
        };
        let version = conversation.version();
        if version == open.kept.0 || !now && at.saturating_sub(open.kept.1) < KEEP_EVERY {
            return;
        }
        if let Some(cached) = conversation.cached() {
            open.kept = (version, at);
            self.transcripts.put(&open.task, cached);
        }
    }

    /// Find the open chat's transcript once the computer lists it, and move
    /// to a later turn's transcript when one appears.
    fn attach(&mut self, chats: &mut Chats) {
        let Some(open) = self.open.as_mut() else {
            return;
        };
        let Some((_, client, chat)) = chats.coder_chat(&open.host, &open.task) else {
            return;
        };
        if open.chat.as_ref() == Some(&chat.id) {
            return;
        }
        match &mut open.conversation {
            // A later turn's transcript is a newer chat for the same task;
            // it carries the earlier turns, so it replaces the one shown.
            // A list read before that turn began names an older one.
            Some(conversation) => {
                if chat.updated_at <= conversation.chat.updated_at {
                    return;
                }
                open.chat = Some(chat.id.clone());
                conversation.switch(chat);
            }
            None => {
                open.chat = Some(chat.id.clone());
                open.conversation = Some(Conversation::open(chats.runtime(), client, chat));
            }
        }
    }

    /// Follow the open chat: ask the computer for its newest chats when the
    /// task's summary moves and every few seconds while it runs, so each
    /// turn's transcript is found as it starts; read the transcript while
    /// the task runs; and once it ends, read it again after the newest chats
    /// were, so the turn's last reply shows.
    fn follow(&mut self, computers: Option<&Computers>, chats: &mut Chats) {
        let now = unix_now();
        let Some(open) = self.open.as_mut() else {
            return;
        };
        let summary = computers.and_then(|c| Self::summary(c.snapshot(), &open.host, &open.task));
        let sequence = summary.map(|s| s.sequence);
        let busy = summary.is_none_or(|s| Self::running(s.phase));
        if sequence != open.seen {
            open.seen = sequence;
            open.head_wanted = true;
            open.head_round = None;
            open.settle_read = None;
        }
        // A computer that says when its chat list changes is read on its
        // nudge; the timed read is then only a backstop.
        let every = if chats.nudged(&open.host) {
            HEAD_NUDGED_EVERY
        } else {
            HEAD_EVERY
        };
        if busy && now.saturating_sub(open.head_at) >= every {
            open.head_wanted = true;
        }
        if let Some(round) = open.head_round
            && chats.head(&open.host, round) == Head::Failed
        {
            open.head_wanted = true;
        }
        if open.head_wanted
            && let Some(round) = chats.refresh_head(&open.host)
        {
            open.head_wanted = false;
            open.head_at = now;
            open.head_round = Some(round);
        }
        self.attach(chats);
        let Some(open) = self.open.as_mut() else {
            return;
        };
        let Some(conversation) = &open.conversation else {
            return;
        };
        // Each session the task delegated, read through the same observer
        // once the computer lists its copy, and shown under its note.
        for (agent, session) in conversation.delegates() {
            if !open.delegates.contains_key(&session) {
                let agent = match agent.as_str() {
                    "devin" => coder_history::Harness::Devin,
                    _ => coder_history::Harness::OpenCode,
                };
                if let Some((client, chat)) =
                    chats.delegate_chat(&open.host, &open.task, agent, &session)
                {
                    let reader = Conversation::open(chats.runtime(), client, chat);
                    open.delegates.insert(session.clone(), reader);
                }
            }
            if let Some(reader) = open.delegates.get(&session) {
                if busy || reader.failed() {
                    reader.poll();
                }
                conversation.delegated(&session, reader.rows());
            }
        }
        if busy || conversation.failed() {
            conversation.poll();
            return;
        }
        if open.settled == sequence {
            return;
        }
        // Ended: one more read, which shows the turn's last reply, while the
        // newest chats are read in case the turn's transcript is a newer
        // one; a move to it reads that too before the chat settles.
        let ticket = *open.settle_read.get_or_insert_with(|| conversation.reads());
        conversation.poll();
        let listed = open
            .head_round
            .is_some_and(|round| chats.head(&open.host, round) == Head::Read);
        if listed && conversation.read_since(ticket) {
            open.settled = sequence;
        }
    }

    /// Accept a composer's message: a new chat on the chosen computer, or a
    /// command for the open chat, whose action the chat's state chose.
    pub fn submit(
        &mut self,
        token: &str,
        value: &str,
        computers: Option<&mut Computers>,
        chats: &mut Chats,
    ) {
        let Some(view) = self.current.as_ref() else {
            return;
        };
        if view.accept_composer(token, value).is_err() {
            return;
        }
        let prompt = value.trim();
        if prompt.is_empty() {
            return;
        }
        // A basic conversation, open or new, needs no computer.
        self.basic
            .set_context(self.router_context(computers.as_deref()));
        if let Some(id) = self.talk.clone() {
            if self.basic.send(&id, prompt, unix_now()) {
                self.composers += 1;
                self.notice = None;
            }
            return;
        }
        if self.open.is_none() && !self.on_computer(computers.as_deref()) {
            if let Some(id) = self.basic.start(prompt, unix_now()) {
                self.composers += 1;
                self.notice = None;
                self.talk_turns = TALK_TURNS;
                self.talk = Some(id);
            }
            return;
        }
        let Some(computers) = computers else { return };
        if let Some(open) = &self.open {
            // The composer edits a queued message.
            if let Some(command) = open.editing.clone() {
                self.composers += 1;
                if let Some(open) = self.open.as_mut() {
                    open.editing = None;
                }
                let edit = QueueEdit::Edit {
                    command,
                    text: prompt.to_owned(),
                };
                self.edit_queue(edit, computers);
                return;
            }
            let summary = Self::summary(computers.snapshot(), &open.host, &open.task);
            let (phase, attention) = (
                summary.map(|summary| summary.phase),
                summary.map(|summary| summary.attention),
            );
            // The send control's token sends as the chat's state says; a
            // choice's token, one this tab minted, sends its own way.
            let chosen = self
                .choices
                .iter()
                .find(|(minted, _)| minted == token)
                .map(|(_, choice)| *choice);
            let (action, emulate) = match chosen {
                Some(choice) => choice.command(),
                None => match Mode::of(phase, attention) {
                    Mode::Send => (CommandAction::Send, false),
                    Mode::Queue => (CommandAction::Queue, false),
                    Mode::Answer => (CommandAction::Answer, false),
                },
            };
            if self.command(action, prompt, emulate, computers) {
                self.composers += 1;
            }
            return;
        }
        // A new chat on the chosen computer.
        let host = match self.chosen(computers) {
            Some(record) => record.key.clone(),
            None => return,
        };
        if computers
            .snapshot()
            .host(&host)
            .and_then(|record| self.workspace(record))
            .is_none()
            && let Err(refusal) = computers.refresh_workspaces(&host)
        {
            self.notice = Some(refusal.reason());
            return;
        }
        let Some(record) = computers.snapshot().host(&host) else {
            return;
        };
        let label = record.label.clone();
        let Some(workspace) = self.workspace(record) else {
            self.notice = Some(format!("{label} lists no workspace for Coder yet."));
            return;
        };
        self.composers += 1;
        match computers.start_task(&host, &workspace, prompt) {
            Ok(task) => {
                let title: String = prompt
                    .lines()
                    .next()
                    .unwrap_or(prompt)
                    .chars()
                    .take(80)
                    .collect();
                let now = computers.snapshot().now;
                self.list.list.titles.insert(task.clone(), title);
                self.list.list.sent.insert(task.clone(), now);
                self.list.list.used.insert(used_key(&host, &workspace), now);
                self.list.save();
                self.notice = None;
                self.open(host, task.clone(), chats);
                self.echo(&task, prompt, None, false, now);
            }
            Err(refusal) => self.notice = Some(refusal.reason()),
        }
    }

    /// Run Coder on the chosen computer with the open conversation: a task
    /// there that starts from the conversation so far. Without a computer
    /// the person is sent to connect one.
    fn run_coder(&mut self, computers: &mut Computers, chats: &mut Chats) {
        let Some(id) = self.talk.clone() else { return };
        let host = match self.availability(Some(computers)) {
            Availability::Ready(host) => host.key.clone(),
            Availability::Connecting(host) => {
                self.notice = Some(format!("Connecting to {}…", host.label));
                return;
            }
            Availability::Offline(host) => {
                self.notice = Some(format!("{} is offline.", host.label));
                return;
            }
            Availability::NotConfigured => {
                self.go = Some(Go::Computers);
                return;
            }
        };
        if computers
            .snapshot()
            .host(&host)
            .and_then(|record| self.workspace(record))
            .is_none()
            && let Err(refusal) = computers.refresh_workspaces(&host)
        {
            self.notice = Some(refusal.reason());
            return;
        }
        let Some(record) = computers.snapshot().host(&host) else {
            return;
        };
        let label = record.label.clone();
        let Some(workspace) = self.workspace(record) else {
            self.notice = Some(format!("{label} lists no workspace for Coder yet."));
            return;
        };
        let title = self
            .basic
            .get(&id)
            .map_or_else(|| "Chat".to_owned(), |summary| summary.title.clone());
        let prompt = handoff(&title, self.basic.turns(&id), MAX_PROMPT_BYTES);
        match computers.start_task(&host, &workspace, &prompt) {
            Ok(task) => {
                let now = computers.snapshot().now;
                self.list.list.titles.insert(task.clone(), title);
                self.list.list.sent.insert(task.clone(), now);
                self.list.list.used.insert(used_key(&host, &workspace), now);
                self.list.save();
                self.basic.spawned(&id, &host, &task, now);
                self.notice = None;
                self.talk = None;
                self.open(host, task.clone(), chats);
                self.echo(&task, &prompt, None, false, now);
            }
            Err(refusal) => self.notice = Some(refusal.reason()),
        }
    }

    /// The task summaries the list shows: every live one, and the cached
    /// row of each task on an added computer that no live summary names
    /// yet, as right after a relaunch.
    fn activity(&self, computers: &Computers) -> Vec<ActivitySummary> {
        let snapshot = computers.snapshot();
        let mut activity = snapshot.activity.clone();
        for row in &self.list.list.rows {
            let live = snapshot
                .activity
                .iter()
                .any(|s| s.host == row.host && s.subject == row.task);
            if !live && snapshot.host(&row.host).is_some() {
                activity.push(row.summary());
            }
        }
        activity
    }

    /// Keep the list as shown, with each chat's catalog title and last
    /// message time, for the next launch.
    fn remember(&mut self, computers: &Computers, chats: &Chats) {
        let snapshot = computers.snapshot();
        let mut newest: Vec<ActivitySummary> = vec![];
        for summary in self.activity(computers) {
            if summary.subject_kind != SubjectKind::Task {
                continue;
            }
            match newest
                .iter_mut()
                .find(|known| known.host == summary.host && known.subject == summary.subject)
            {
                Some(known) if known.sequence < summary.sequence => *known = summary,
                Some(_) => {}
                None => newest.push(summary),
            }
        }
        let mut rows: Vec<Row> = newest
            .iter()
            .filter(|summary| snapshot.host(&summary.host).is_some())
            .filter_map(|summary| {
                let chat = chats.coder_chat(&summary.host, &summary.subject);
                let chat = chat.map(|(_, _, chat)| chat);
                if archived(chat.as_ref()) {
                    return None;
                }
                let cached = self
                    .list
                    .list
                    .rows
                    .iter()
                    .find(|row| row.host == summary.host && row.task == summary.subject);
                let title = chat
                    .as_ref()
                    .map(|chat| chat.title.clone())
                    .filter(|title| named(title))
                    .or_else(|| cached.and_then(|row| row.title.clone()));
                let last = chat
                    .as_ref()
                    .and_then(|chat| chat.updated_at.as_deref().and_then(unix_seconds))
                    .or_else(|| cached.and_then(|row| row.last));
                Some(Row::of(summary, title, last))
            })
            .collect();
        rows.sort_by_key(|row| {
            std::cmp::Reverse((
                row.last
                    .or_else(|| self.list.list.sent.get(&row.task).copied()),
                row.updated_at,
            ))
        });
        self.list.list.rows = rows;
        self.list.save();
    }

    pub fn render(
        &mut self,
        computers: Option<&Computers>,
        chats: &mut Chats,
    ) -> Option<serde_json::Value> {
        if let Some(computers) = computers {
            self.remember(computers, chats);
        }
        self.basic.settle(unix_now());
        self.poll_cli();
        self.play(computers);
        self.follow(computers, chats);
        self.settle_echoes(computers.map_or_else(unix_now, |c| c.snapshot().now));
        let ended = self
            .open
            .as_ref()
            .is_some_and(|open| open.seen.is_some() && open.settled == open.seen);
        self.keep(ended);
        if self.open.is_none() && self.talk.is_none() && !self.drawer && !self.picking {
            let availability = self.availability(computers);
            let on_computer = self.on_computer(computers);
            let candidates = self
                .candidates(computers, chats, &availability, on_computer)
                .into_iter()
                .filter_map(|(id, chip)| match chip.element {
                    Element::Button { label, .. } => Some((id, label)),
                    _ => None,
                })
                .collect();
            self.basic.want_rank(candidates);
        }
        self.revision += 1;
        let view = loop {
            let mut root = match (&self.open, &self.talk) {
                _ if self.drawer => self.previous(computers, chats),
                (Some(open), _) => self.chat(open, computers),
                (None, Some(id)) => {
                    let id = id.clone();
                    self.talk_view(&id, computers)
                }
                (None, None) => self.landing(computers, chats),
            };
            let detached = !self.pulled || source::detach(&mut root, &self.instance).is_ok();
            match View::new(self.instance.clone(), self.revision, root)
                .validate()
                .ok()
                .filter(|_| detached)
            {
                Some(view) => break view,
                // A long chat can outgrow one view: show less of each tool's
                // output, then keep its newest half.
                None if self.drawer => return None,
                None if self.open.is_none() && self.talk.is_some() => {
                    if self.talk_turns <= 1 {
                        return None;
                    }
                    self.talk_turns /= 2;
                }
                None => {
                    let conversation = self.open.as_ref()?.conversation.as_ref()?;
                    if !conversation.shrink() {
                        return None;
                    }
                }
            }
        };
        let value = serde_json::to_value(view.view()).ok();
        self.current = Some(view);
        self.choices = self.current_choices(computers);
        value
    }

    /// The choices the open chat's composer offers now, with their tokens.
    fn current_choices(&self, computers: Option<&Computers>) -> Vec<(String, Choice)> {
        let Some(open) = &self.open else {
            return Vec::new();
        };
        let summary = computers.and_then(|c| Self::summary(c.snapshot(), &open.host, &open.task));
        let (phase, attention) = (summary.map(|s| s.phase), summary.map(|s| s.attention));
        if open.editing.is_some() || Mode::of(phase, attention) != Mode::Queue {
            return Vec::new();
        }
        self.tokens(&Choice::offered(phase)).1
    }

    /// The composer's tokens: the send control's, and one per choice.
    fn tokens(&self, choices: &[Choice]) -> (String, Vec<(String, Choice)>) {
        let token = format!("coder-composer-{}", self.composers);
        let minted = choices
            .iter()
            .enumerate()
            .map(|(index, choice)| (format!("{token}-choice-{index}"), *choice))
            .collect();
        (token, minted)
    }

    fn composer_with(
        &self,
        placeholder: String,
        enabled: bool,
        busy: bool,
        choices: &[Choice],
        draft: Option<String>,
        focus: bool,
    ) -> Node<Intent> {
        let (token, minted) = self.tokens(choices);
        node(
            "coder-composer",
            Element::Composer {
                token,
                placeholder,
                max_bytes: MAX_PROMPT_BYTES,
                enabled,
                busy,
                stop: busy.then_some(Intent::Stop),
                choices: minted
                    .into_iter()
                    .map(|(token, choice)| ComposerChoice {
                        token,
                        label: choice.label().into(),
                    })
                    .collect(),
                draft,
                focus,
            },
        )
    }

    /// Whether a new chat can start now on the chosen computer.
    fn availability<'a>(&self, computers: Option<&'a Computers>) -> Availability<'a> {
        computers.map_or(Availability::NotConfigured, |computers| {
            availability(computers.snapshot(), self.selected.as_deref())
        })
    }

    /// Every chat, newest message first: basic conversations and the
    /// computers' Coder chats. A chat with no known message time goes last.
    fn recent(&self, computers: Option<&Computers>, chats: &Chats) -> Vec<Recent> {
        let availability = self.availability(computers);
        let now = computers.map_or_else(unix_now, |c| c.snapshot().now);
        let mut rows: Vec<Recent> = self
            .basic
            .list()
            .iter()
            .map(|summary| {
                let place = match &summary.coder {
                    Some(spawned) => {
                        let label = computers
                            .and_then(|c| c.snapshot().host(&spawned.host))
                            .map_or("a computer", |host| host.label.as_str());
                        format!("Coder · running on {label}")
                    }
                    None => "OpenAgents".to_owned(),
                };
                Recent {
                    last: Some(summary.updated),
                    updated: summary.updated,
                    title: summary.title.clone(),
                    row: button(
                        &format!("talk-{}", &summary.id[..16.min(summary.id.len())]),
                        &format!("{}\n{place} · {}", summary.title, ago(now, summary.updated)),
                        Intent::OpenTalk {
                            id: summary.id.clone(),
                        },
                    ),
                }
            })
            .collect();
        if let Some(computers) =
            computers.filter(|_| !matches!(availability, Availability::NotConfigured))
        {
            let saved =
                |host: &str, task: &str| chats.coder_chat(host, task).map(|(_, _, chat)| chat);
            rows.extend(
                task_rows(
                    computers.snapshot(),
                    &self.activity(computers),
                    &self.list.list,
                    &saved,
                )
                .into_iter()
                .map(|(last, updated, title, row)| Recent {
                    last,
                    updated,
                    title,
                    row,
                }),
            );
        }
        rows.sort_by_key(|recent| std::cmp::Reverse((recent.last, recent.updated)));
        rows.truncate(SHOWN_TASKS + SHOWN_TALKS);
        rows
    }

    /// The previous chats, behind the menu button: every chat, newest
    /// message first, from the kept list while the computers are read.
    fn previous(&self, computers: Option<&Computers>, chats: &Chats) -> Node<Intent> {
        let availability = self.availability(computers);
        let mut new = icon_button(
            "coder-new",
            "New chat",
            Glyph::Compose,
            true,
            Intent::NewChat,
        );
        new.style.align = Some(TextAlign::End);
        let mut children = vec![header(
            "coder-header",
            vec![
                icon_button("coder-back", "OpenAgents", Glyph::Back, false, Intent::Back),
                heading("coder-title", "Chats"),
                new,
            ],
        )];
        if let Some(line) = Self::unavailable(&availability) {
            children.push(line);
        }
        if let Some(notice) = &self.notice {
            children.push(status("coder-notice", notice));
        }
        let rows: Vec<Node<Intent>> = self
            .recent(computers, chats)
            .into_iter()
            .map(|recent| recent.row)
            .collect();
        if rows.is_empty() {
            children.push(status("coder-none", "No chats yet."));
        } else {
            children.push(node(
                "coder-chats",
                Element::List {
                    label: "Chats".into(),
                    children: rows,
                },
            ));
        }
        page(children)
    }

    /// Why no computer can take a chat right now, when one is added.
    fn unavailable(availability: &Availability<'_>) -> Option<Node<Intent>> {
        match availability {
            Availability::Connecting(host) => Some(status(
                "coder-connecting",
                &format!("Connecting to {}…", host.label),
            )),
            Availability::Offline(host) => Some(status(
                "coder-offline",
                &format!("{} is offline.", host.label),
            )),
            Availability::Ready(_) | Availability::NotConfigured => None,
        }
    }

    /// A new chat, where the tab opens: a composer ready to type, the
    /// target its first message starts (the basic Coder in the cloud, or a
    /// task on the chosen computer in the chosen workspace) as a selector in
    /// the header, and suggested actions as chips above the composer.
    fn landing(&self, computers: Option<&Computers>, chats: &Chats) -> Node<Intent> {
        let availability = self.availability(computers);
        let on_computer = self.on_computer(computers);
        // The target selector: where the first message goes.
        let (target, glyph) = match (&availability, on_computer) {
            (Availability::Ready(host), true) => (
                match self.workspace(host) {
                    Some(workspace) => format!("{} · {workspace}", host.label),
                    None => host.label.clone(),
                },
                Glyph::Computer,
            ),
            (Availability::Connecting(host) | Availability::Offline(host), true) => {
                (host.label.clone(), Glyph::Computer)
            }
            _ => ("Cloud".to_owned(), Glyph::Cloud),
        };
        let mut selector = pill("coder-target", &clip(&target, 30), glyph, Intent::Pick);
        selector.style.align = Some(TextAlign::End);
        let mut children = vec![header(
            "coder-header",
            vec![
                icon_button(
                    "coder-menu",
                    "Previous chats",
                    Glyph::Menu,
                    true,
                    Intent::Menu,
                ),
                heading("coder-title", "OpenAgents"),
                selector,
            ],
        )];
        if self.picking {
            children.push(self.targets(computers, on_computer));
        }
        let ready = if on_computer {
            let ready = matches!(availability, Availability::Ready(_));
            children.extend(Self::unavailable(&availability));
            ready
        } else {
            true
        };
        if let Some(notice) = &self.notice {
            children.push(status("coder-notice", notice));
        }
        // An empty conversation fills the screen above the suggestions.
        children.push(node(
            "coder-new-transcript",
            Element::Transcript {
                label: "New chat".into(),
                children: vec![],
                earlier: None,
                source: None,
            },
        ));
        // The selector's choices replace the suggestions while it is open.
        let chips = if self.picking {
            vec![]
        } else {
            self.suggestions(computers, chats, &availability, on_computer)
        };
        if !chips.is_empty() {
            // Inset as the composer's field is, so the chips line up with it.
            children.push(Node {
                key: "coder-suggestions".into(),
                style: Style {
                    gap: Some(Space::Sm),
                    padding_start: Some(Space::Sm),
                    padding_end: Some(Space::Sm),
                    ..Style::default()
                },
                element: Element::Stack {
                    axis: Axis::Wrap,
                    children: chips,
                },
            });
        }
        let placeholder = match (&availability, on_computer) {
            (
                Availability::Ready(host)
                | Availability::Connecting(host)
                | Availability::Offline(host),
                true,
            ) => format!("Message OpenAgents on {}", host.label),
            _ => "Message OpenAgents".to_owned(),
        };
        // The tab exists to write a message: it opens ready to type.
        children.push(self.composer_with(placeholder, ready, false, &[], None, true));
        page(children)
    }

    /// The target selector's choices: each computer this device may operate,
    /// the basic Coder in the cloud, and connecting a computer. The current
    /// one carries a check.
    fn targets(&self, computers: Option<&Computers>, on_computer: bool) -> Node<Intent> {
        let chosen = computers
            .filter(|_| on_computer)
            .and_then(|computers| self.chosen(computers))
            .map(|host| host.key.clone());
        let mut options: Vec<Node<Intent>> = computers
            .map(Self::hosts)
            .unwrap_or_default()
            .into_iter()
            .enumerate()
            .map(|(index, host)| {
                let current = chosen.as_deref() == Some(host.key.as_str());
                pill(
                    &format!("coder-target-{index}"),
                    &clip(&host.label, 40),
                    if current {
                        Glyph::Check
                    } else {
                        Glyph::Computer
                    },
                    Intent::Target {
                        host: Some(host.key.clone()),
                    },
                )
            })
            .collect();
        options.push(pill(
            "coder-target-cloud",
            "Cloud",
            if on_computer {
                Glyph::Cloud
            } else {
                Glyph::Check
            },
            Intent::Target { host: None },
        ));
        options.push(pill(
            "coder-connect",
            "Connect a computer",
            Glyph::Add,
            Intent::ConnectComputer,
        ));
        Node {
            key: "coder-targets".into(),
            style: Style {
                gap: Some(Space::Sm),
                ..Style::default()
            },
            element: Element::Stack {
                axis: Axis::Wrap,
                children: options,
            },
        }
    }

    /// Suggested actions above the composer, each from what the phone
    /// knows: the newest chats to continue, the chosen computer's other
    /// workspaces, and connecting a computer when none is added. The worker's
    /// ranking orders them when it answered for this set.
    fn suggestions(
        &self,
        computers: Option<&Computers>,
        chats: &Chats,
        availability: &Availability<'_>,
        on_computer: bool,
    ) -> Vec<Node<Intent>> {
        let mut chips = self.candidates(computers, chats, availability, on_computer);
        self.basic.rank_order(&mut chips);
        chips.into_iter().map(|(_, chip)| chip).collect()
    }

    /// The suggestions in the phone's order, each with the ID a rank job
    /// knows it by.
    fn candidates(
        &self,
        computers: Option<&Computers>,
        chats: &Chats,
        availability: &Availability<'_>,
        on_computer: bool,
    ) -> Vec<(String, Node<Intent>)> {
        let mut chips: Vec<(String, Node<Intent>)> = self
            .recent(computers, chats)
            .into_iter()
            .take(SUGGESTED_CHATS)
            .enumerate()
            .filter_map(|(index, recent)| {
                let Element::Button { intent, .. } = recent.row.element else {
                    return None;
                };
                let id = match &intent {
                    Intent::OpenTalk { id } => format!("talk:{}", &id[..16.min(id.len())]),
                    Intent::Open { task, .. } => format!("task:{}", &task[..16.min(task.len())]),
                    _ => format!("chat:{index}"),
                };
                Some((
                    id,
                    pill(
                        &format!("coder-continue-{index}"),
                        &clip(&recent.title, 32),
                        Glyph::History,
                        intent,
                    ),
                ))
            })
            .collect();
        if let (Availability::Ready(host), true) = (availability, on_computer) {
            let current = self.workspace(host);
            chips.extend(
                self.other_workspaces(host, current.as_deref())
                    .into_iter()
                    .take(SUGGESTED_WORKSPACES)
                    .enumerate()
                    .map(|(index, label)| {
                        (
                            format!("workspace:{}", clip(&label, 40)),
                            pill(
                                &format!("coder-repo-{index}"),
                                &clip(&label, 32),
                                Glyph::Folder,
                                Intent::Workspace { label },
                            ),
                        )
                    }),
            );
        }
        if matches!(availability, Availability::NotConfigured) {
            chips.push((
                "connect".into(),
                pill(
                    "coder-connect",
                    "Connect a computer",
                    Glyph::Add,
                    Intent::ConnectComputer,
                ),
            ));
        }
        chips
    }

    /// The workspaces of `host` other than `current`, the ones this device
    /// used most recently first, then in the computer's order.
    fn other_workspaces(&self, host: &HostRecord, current: Option<&str>) -> Vec<String> {
        let mut listed: Vec<(usize, &String)> = host
            .workspaces
            .iter()
            .flatten()
            .filter(|label| Some(label.as_str()) != current)
            .enumerate()
            .collect();
        listed
            .sort_by_key(|(index, label)| (std::cmp::Reverse(self.used(&host.key, label)), *index));
        listed.into_iter().map(|(_, label)| label.clone()).collect()
    }

    /// An open basic conversation: its turns, the reply as it streams, and
    /// a way to run Coder on a computer with it.
    fn talk_view(&mut self, id: &str, computers: Option<&Computers>) -> Node<Intent> {
        let availability = self.availability(computers);
        let summary = self.basic.get(id).cloned();
        let busy = self.basic.busy(id);
        let tail = self.basic.tail(id);
        let limit = self.talk_turns;
        let turns = self.basic.turns(id);
        let mut children = vec![chat_header(status("coder-chat-place", "OpenAgents"))];
        if let Some(notice) = &self.notice {
            children.push(status("coder-notice", notice));
        }
        let skipped = turns.len().saturating_sub(limit);
        let mut rows: Vec<Node<Intent>> = turns
            .iter()
            .enumerate()
            .skip(skipped)
            .map(|(index, turn)| {
                let role = match turn.role {
                    TurnRole::User => MessageRole::User,
                    TurnRole::Assistant => MessageRole::Assistant,
                };
                let mut row = message(
                    &format!("talk-m{index}"),
                    role,
                    rust_native::markdown::parse(&turn.text),
                );
                // Where a reply came from, quietly: a prepared answer is
                // reviewed text, not the model's.
                if turn.meta.as_ref().is_some_and(Meta::canned)
                    && let Element::Message { note, .. } = &mut row.element
                {
                    *note = Some(PREPARED.into());
                }
                row
            })
            .collect();
        match tail {
            Tail::None => {}
            Tail::Thinking => rows.push(node(
                "talk-working",
                Element::Working {
                    label: "Thinking".into(),
                },
            )),
            Tail::Streaming(blocks) => rows.push(message(
                &format!("talk-m{}", turns.len()),
                MessageRole::Assistant,
                blocks,
            )),
            Tail::Failed(why) => rows.push(node(
                "talk-failed",
                Element::Message {
                    role: MessageRole::System,
                    note: None,
                    children: vec![status("talk-failed-text", &why)],
                },
            )),
        }
        let failed = rows.last().is_some_and(|row| row.key == "talk-failed");
        children.push(node(
            "coder-transcript",
            Element::Transcript {
                label: "Messages".into(),
                children: rows,
                earlier: None,
                source: None,
            },
        ));
        if failed {
            children.push(row(
                "talk-retry-row",
                vec![button("talk-retry", "Try again", Intent::Retry)],
            ));
        }
        // Coder on a computer: the task this conversation started, and a way
        // to start one, or to connect a computer first.
        let spawned = summary.as_ref().and_then(|summary| summary.coder.clone());
        let mut agents = vec![];
        if let Some(spawned) = &spawned {
            let label = computers
                .and_then(|c| c.snapshot().host(&spawned.host))
                .map_or("your computer", |host| host.label.as_str());
            agents.push(button(
                "coder-spawned",
                &format!("Open Coder on {label}"),
                Intent::OpenSpawned,
            ));
        }
        // What the router said about the last reply: its offers, follow-ups,
        // and whether it was a prepared answer. Nothing here acts until a
        // tap, and each tap's meaning is the phone's own.
        let meta = if failed {
            None
        } else {
            self.basic.last_meta(id)
        };
        let offers = meta
            .as_ref()
            .map(|meta| meta.offers.clone())
            .unwrap_or_default();
        // The worker's judgment placed the last message on a computer, or
        // offered to dispatch Coder: the way there is a chip.
        let judged = self.basic.lane(id) == Some(crate::basic_coder::Lane::Computer)
            || offers.contains(&Offer::RunCoder);
        match &availability {
            Availability::Ready(host) => {
                let label = format!("Run Coder on {}", host.label);
                agents.push(if judged {
                    pill("coder-run", &label, Glyph::Computer, Intent::RunCoder)
                } else {
                    button("coder-run", &label, Intent::RunCoder)
                });
            }
            Availability::Connecting(_) | Availability::Offline(_) => {
                agents.extend(Self::unavailable(&availability));
            }
            // No computer: chat stays the whole screen. The way to connect
            // one shows only when the worker's judgment says this message
            // needs a computer.
            Availability::NotConfigured if judged => {
                agents.push(pill(
                    "coder-connect",
                    "Connect a computer",
                    Glyph::Add,
                    Intent::ConnectComputer,
                ));
            }
            Availability::NotConfigured => {}
        }
        // Screens the router offered, named by the phone.
        for (index, offer) in offers.iter().enumerate() {
            let Offer::OpenScreen { screen } = offer else {
                continue;
            };
            let connecting =
                *screen == Screen::Computers && matches!(availability, Availability::NotConfigured);
            if connecting && judged {
                continue;
            }
            let (label, glyph) = screen_chip(*screen, connecting);
            agents.push(pill(
                &format!("coder-screen-{index}"),
                label,
                glyph,
                Intent::OpenScreen { screen: *screen },
            ));
        }
        children.push(wrap("coder-agents", agents));
        // Proposed read-only commands, each a card with the exact command
        // and a Run button.
        for (index, offer) in offers.iter().enumerate() {
            if let Offer::Cli { argv, runs_on } = offer {
                children.push(self.cli_card(id, index, argv, *runs_on, &availability));
            }
        }
        if let Some(meta) = meta.as_ref().filter(|_| !busy) {
            // Suggested next questions under a prepared answer.
            let chips: Vec<Node<Intent>> = meta
                .followups
                .iter()
                .enumerate()
                .map(|(index, followup)| {
                    pill(
                        &format!("coder-followup-{index}"),
                        &clip(&followup.label, 60),
                        Glyph::Ask,
                        Intent::Followup { index },
                    )
                })
                .collect();
            if !chips.is_empty() {
                children.push(wrap("coder-followups", chips));
            }
            if meta.canned() {
                children.extend(self.wrong_answer(id));
            }
        }
        children.push(self.composer_with(
            "Message OpenAgents".to_owned(),
            true,
            busy,
            &[],
            None,
            false,
        ));
        page(children)
    }

    /// A proposed read-only command: the command itself, where it runs,
    /// and a Run button; once run, what came of it.
    fn cli_card(
        &self,
        id: &str,
        index: usize,
        argv: &[String],
        runs_on: RunsOn,
        availability: &Availability<'_>,
    ) -> Node<Intent> {
        let key = format!("coder-cli-{index}");
        let place = match (runs_on, availability) {
            (RunsOn::ThisDevice, _) => "Reads only. Runs on this phone.".to_owned(),
            (RunsOn::ConnectedComputer, Availability::Ready(host)) => {
                format!("Reads only. Runs on {}.", host.label)
            }
            (RunsOn::ConnectedComputer, _) => "Reads only. Runs on your computer.".to_owned(),
        };
        let mut children = vec![
            text(
                &format!("{key}-command"),
                &Offer::command_line(argv),
                TextRole::Code,
                WHITE,
                false,
            ),
            status(&format!("{key}-where"), &place),
        ];
        match &self.cli {
            Some((talk, ran, outcome)) if talk == id && ran.as_slice() == argv => match outcome {
                CliOutcome::Output(lines) => {
                    children.extend(lines.iter().enumerate().map(|(at, line)| {
                        text(
                            &format!("{key}-out-{at}"),
                            line,
                            TextRole::Code,
                            GRAY,
                            false,
                        )
                    }));
                    children.push(button(
                        &format!("{key}-run"),
                        "Run again",
                        Intent::RunCli { index },
                    ));
                }
                CliOutcome::Running(label) => {
                    children.push(status(&format!("{key}-running"), &format!("Running on {label}…")));
                }
                CliOutcome::Refused(why) => children.push(status(&format!("{key}-why"), why)),
            },
            _ => children.push(icon_button(
                &format!("{key}-run"),
                "Run",
                Glyph::Terminal,
                false,
                Intent::RunCli { index },
            )),
        }
        Node {
            key,
            style: Style {
                gap: Some(Space::Xs),
                ..Style::default()
            },
            element: Element::Stack {
                axis: Axis::Vertical,
                children,
            },
        }
    }

    /// Under a prepared answer: **Wrong answer**, then what it would send
    /// and a choice, then what came of it.
    fn wrong_answer(&self, id: &str) -> Vec<Node<Intent>> {
        let flag = self
            .flag
            .as_ref()
            .filter(|(talk, _)| talk == id)
            .map(|(_, flag)| flag);
        match flag {
            None => vec![row(
                "coder-wrong-row",
                vec![icon_button(
                    "coder-wrong",
                    "Wrong answer",
                    Glyph::Flag,
                    false,
                    Intent::WrongAnswer,
                )],
            )],
            Some(Flag::Confirm) => vec![
                status("coder-wrong-what", WRONG_ANSWER_SENDS),
                row(
                    "coder-wrong-choice",
                    vec![
                        button("coder-wrong-send", "Send", Intent::SendWrongAnswer),
                        button("coder-wrong-cancel", "Cancel", Intent::CancelWrongAnswer),
                    ],
                ),
            ],
            Some(Flag::Sending) => vec![status("coder-wrong-status", "Sending…")],
            Some(Flag::Filed(line) | Flag::Failed(line)) => {
                vec![status("coder-wrong-status", line)]
            }
        }
    }

    fn chat(&self, open: &Open, computers: Option<&Computers>) -> Node<Intent> {
        let summary = computers.and_then(|c| Self::summary(c.snapshot(), &open.host, &open.task));
        let phase = summary.map(|s| s.phase);
        let attention = summary.map(|s| s.attention);
        let mode = Mode::of(phase, attention);
        // The host's typed note, such as a missing model capacity. A
        // waiting question says so below instead.
        let note = summary
            .filter(|s| {
                s.headline != nostr::activity_summary::generic_headline(SubjectKind::Task, s.phase)
                    && mode != Mode::Answer
            })
            .map(|s| s.headline.clone());
        let running = phase.is_none_or(Self::running);
        let label = computers
            .and_then(|c| c.snapshot().host(&open.host))
            .map_or_else(|| "your computer".to_owned(), |h| h.label.clone());
        let working = match phase {
            Some(Phase::Queued) | None => Some("Queued"),
            Some(Phase::Running) => Some("Coder is working"),
            // Coder asked this device: the answer goes in the composer.
            Some(Phase::Waiting) if mode == Mode::Answer => None,
            Some(Phase::Waiting) => Some("Waiting for you on the computer"),
            _ => None,
        };
        // Only the breadcrumb bar heads a chat; the transcript follows it.
        let mut children = vec![chat_header(status(
            "coder-chat-place",
            &format!("{} · {label}", phase.map_or("Starting", phase_label)),
        ))];
        if let Some(note) = &note {
            children.push(status("coder-chat-note", note));
        }
        if let Some(notice) = &self.notice {
            children.push(status("coder-notice", notice));
        }
        // Messages this device sent that the transcript does not show yet.
        let echoes: Vec<Pending<'_>> = self
            .echoes
            .iter()
            .filter(|echo| echo.task == open.task)
            .map(|echo| Pending {
                key: format!("coder-sent-{}", echo.key),
                text: &echo.text,
                note: if echo
                    .command
                    .as_ref()
                    .is_some_and(|command| self.outbox.holds(command))
                {
                    Some("Sending")
                } else if echo.queued {
                    Some("Queued")
                } else {
                    None
                },
            })
            .collect();
        let transcript = match &open.conversation {
            Some(conversation) => {
                conversation.transcript("coder-transcript", Intent::Earlier, &echoes, working)
            }
            None => {
                // The computer has not listed the transcript yet: show what
                // this device sent, or the chat's first line after a
                // relaunch.
                let mut rows = vec![];
                if echoes.is_empty()
                    && let Some(title) = self.list.list.titles.get(&open.task)
                {
                    rows.push(node(
                        "coder-sent",
                        Element::Message {
                            role: MessageRole::User,
                            note: None,
                            children: vec![node(
                                "coder-sent-text",
                                Element::Markdown {
                                    blocks: rust_native::markdown::parse(title),
                                },
                            )],
                        },
                    ));
                }
                let mut transcript = Conversation::pending_transcript(
                    "coder-transcript",
                    &echoes,
                    Some(working.unwrap_or("Loading the chat")),
                );
                if let Element::Transcript { children, .. } = &mut transcript.element {
                    rows.append(children);
                    *children = rows;
                }
                transcript
            }
        };
        children.push(transcript);
        let waiting = self.outbox.waiting(&open.task);
        if waiting > 0 {
            children.push(status(
                "coder-outbox",
                &if waiting == 1 {
                    format!("1 message waiting to reach {label}")
                } else {
                    format!("{waiting} messages waiting to reach {label}")
                },
            ));
        }
        if mode == Mode::Answer {
            let approval = attention == Some(Attention::Approval);
            children.push(status(
                "coder-asked",
                if approval {
                    "Coder is waiting for your approval."
                } else {
                    "Coder is waiting for your answer."
                },
            ));
            if approval {
                children.push(row(
                    "coder-approval",
                    vec![
                        button("coder-approve", "Approve", Intent::Approve),
                        button("coder-deny", "Deny", Intent::Deny),
                    ],
                ));
            }
        }
        let me = computers.map(|c| c.snapshot().device.clone());
        let queued = open.queue.as_ref().map_or(0, |queue| queue.items.len());
        if open.leased_at.is_some() {
            children.extend(self.queue_panel(open, me.as_deref()));
        }
        if running && mode != Mode::Answer {
            let mut controls = vec![button("coder-stop", "Stop", Intent::Stop)];
            if queued > 0 && open.leased_at.is_none() && !open.unlisted {
                controls.push(button("coder-edit-queue", "Edit queue", Intent::EditQueue));
            }
            children.push(row("coder-controls", controls));
        }
        let editing = open.editing.as_ref().and_then(|command| {
            open.queue
                .as_ref()?
                .items
                .iter()
                .find(|item| item.command == *command)
                .and_then(|item| item.text.clone())
        });
        let placeholder = match (editing.is_some(), mode) {
            (true, _) => "Edit your queued message".to_owned(),
            (false, Mode::Send) => format!("Message OpenAgents on {label}"),
            (false, Mode::Queue) => "Queue a message for Coder's next turn".to_owned(),
            (false, Mode::Answer) => "Answer Coder".to_owned(),
        };
        let choices = if editing.is_none() && mode == Mode::Queue {
            Choice::offered(phase)
        } else {
            Vec::new()
        };
        let allowed = computers.is_some_and(|c| c.can_operate(&open.host));
        children.push(self.composer_with(placeholder, allowed, false, &choices, editing, false));
        page(children)
    }

    /// The open queue: each held message in the order it runs, with this
    /// device's own messages editable under the lease.
    fn queue_panel(&self, open: &Open, me: Option<&str>) -> Vec<Node<Intent>> {
        let mut children = vec![row(
            "coder-queue-header",
            vec![
                heading("coder-queue-title", "Queued messages"),
                button("coder-queue-done", "Done", Intent::DoneQueue),
            ],
        )];
        let items = open
            .queue
            .as_ref()
            .map(|queue| queue.items.as_slice())
            .unwrap_or_default();
        if items.is_empty() {
            children.push(status("coder-queue-empty", "No messages are queued."));
            return children;
        }
        let first_queued = items.iter().position(|item| !item.priority);
        for (index, item) in items.iter().enumerate() {
            let key = &item.command[..16.min(item.command.len())];
            let own = me == Some(item.device.as_str());
            let text = match (&item.text, item.priority) {
                (Some(text), false) => text.clone(),
                (Some(text), true) => format!("Sending next: {text}"),
                (None, _) => "A message from another device".to_owned(),
            };
            let mut row_children = vec![body(&format!("coder-queued-{key}"), &text)];
            if own && !item.priority {
                let command = item.command.clone();
                row_children.push(button(
                    &format!("coder-queued-edit-{key}"),
                    "Edit",
                    Intent::EditQueued {
                        command: command.clone(),
                    },
                ));
                if first_queued.is_some_and(|first| index > first) {
                    row_children.push(button(
                        &format!("coder-queued-up-{key}"),
                        "Move up",
                        Intent::MoveQueuedUp {
                            command: command.clone(),
                        },
                    ));
                }
                row_children.push(button(
                    &format!("coder-queued-now-{key}"),
                    "Send now",
                    Intent::SendQueuedNow {
                        command: command.clone(),
                    },
                ));
                row_children.push(button(
                    &format!("coder-queued-remove-{key}"),
                    "Remove",
                    Intent::RemoveQueued { command },
                ));
            }
            children.push(node(
                &format!("coder-queued-row-{key}"),
                Element::Stack {
                    axis: Axis::Vertical,
                    children: row_children,
                },
            ));
        }
        children
    }
}

/// Whether a refusal means the computer was not reached, so the command
/// waits and tries again: a transport failure, or a computer that is offline
/// or out of date right now.
fn unreachable(refusal: &coder_computers::Refusal) -> bool {
    match refusal {
        coder_computers::Refusal::Failed(error) => matches!(
            error.code,
            coder_host::Code::Transport | coder_host::Code::Unavailable
        ),
        coder_computers::Refusal::Denied(Denial::Offline | Denial::OutOfDate) => true,
        _ => false,
    }
}

/// The host's refusal code, when the computer answered.
fn refusal_code(refusal: &coder_computers::Refusal) -> Option<coder_host::Code> {
    match refusal {
        coder_computers::Refusal::Failed(error) => Some(error.code),
        _ => None,
    }
}

/// Whether Coder can take a new chat. A computer that is still connecting
/// is not the same as none added: the first shows the chats while it
/// connects, and only the second asks for a computer.
#[derive(Debug, PartialEq)]
pub(crate) enum Availability<'a> {
    /// No computer this device may run work on.
    NotConfigured,
    /// A computer this device may run work on is connecting, as after launch.
    Connecting(&'a HostRecord),
    /// A computer this device may run work on is offline or out of date.
    Offline(&'a HostRecord),
    /// A computer can take a chat now.
    Ready(&'a HostRecord),
}

/// Whether a new chat can start now, from the typed authority check and host
/// status, never from the words on screen. A ready computer is `selected`
/// when that one is ready, else the first.
pub(crate) fn availability<'a>(snapshot: &'a Snapshot, selected: Option<&str>) -> Availability<'a> {
    let caps = Capabilities {
        platform: Platform::Phone,
        camera: false,
    };
    let operate =
        |host: &HostRecord| authority::check(snapshot, caps, Action::Operate { host: &host.key });
    let ready: Vec<&HostRecord> = snapshot
        .hosts
        .iter()
        .filter(|host| operate(host).is_ok())
        .collect();
    if let Some(host) = ready
        .iter()
        .find(|host| Some(host.key.as_str()) == selected)
        .or_else(|| ready.first())
    {
        return Availability::Ready(host);
    }
    // A computer this device may run work on once it is reachable.
    let waiting = |denial: Denial| {
        snapshot
            .hosts
            .iter()
            .find(|host| operate(host).as_ref() == Err(&denial))
    };
    if let Some(host) = waiting(Denial::Offline) {
        return match HostStatus::derive(host, snapshot.now) {
            HostStatus::Connecting { .. }
            | HostStatus::Offline {
                cause: OfflineCause::NotConnected | OfflineCause::Retrying { .. },
            } => Availability::Connecting(host),
            _ => Availability::Offline(host),
        };
    }
    match waiting(Denial::OutOfDate) {
        Some(host) => Availability::Offline(host),
        None => Availability::NotConfigured,
    }
}

/// What the open chat's send control does, from the task's phase and the
/// attention its summary asks for; never from the text. A long press
/// offers the other ways to send ([`Choice`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    /// Continue a finished chat with a follow-up turn.
    Send,
    /// Queue the message for the next turn.
    Queue,
    /// Answer the question or approval request Coder ended its turn with.
    Answer,
}

impl Mode {
    pub(crate) fn of(phase: Option<Phase>, attention: Option<Attention>) -> Self {
        match (phase, attention) {
            (Some(Phase::Waiting), Some(Attention::Input | Attention::Approval)) => Mode::Answer,
            (None | Some(Phase::Queued | Phase::Running | Phase::Waiting), _) => Mode::Queue,
            _ => Mode::Send,
        }
    }
}

/// Whether the Coder list leaves a task out: its host's owner or a device
/// archived it, which the task's saved chat says.
pub(crate) fn archived(chat: Option<&coder_history::Chat>) -> bool {
    chat.is_some_and(|chat| chat.archived)
}

/// The saved chat of a task on a host, when the computer listed it.
pub(crate) type Saved<'a> = dyn Fn(&str, &str) -> Option<coder_history::Chat> + 'a;

/// One tappable row per task, newest message first, from the newest summary
/// of each. Archived tasks are left out.
#[cfg(test)]
pub(crate) fn tasks(
    snapshot: &Snapshot,
    activity: &[ActivitySummary],
    known: &List,
    saved: &Saved<'_>,
) -> Vec<Node<Intent>> {
    task_rows(snapshot, activity, known, saved)
        .into_iter()
        .map(|(_, _, _, row)| row)
        .collect()
}

/// [`tasks`], each row with its last message time, summary time, and title,
/// for merging with basic conversations.
pub(crate) fn task_rows(
    snapshot: &Snapshot,
    activity: &[ActivitySummary],
    known: &List,
    saved: &Saved<'_>,
) -> Vec<(Option<u64>, u64, String, Node<Intent>)> {
    let mut newest: Vec<&ActivitySummary> = vec![];
    for summary in activity
        .iter()
        .filter(|s| s.subject_kind == SubjectKind::Task)
    {
        match newest
            .iter_mut()
            .find(|known| known.host == summary.host && known.subject == summary.subject)
        {
            Some(known) if known.sequence < summary.sequence => *known = summary,
            Some(_) => {}
            None => newest.push(summary),
        }
    }
    newest.retain(|summary| !archived(saved(&summary.host, &summary.subject).as_ref()));
    let cached = |summary: &ActivitySummary| {
        known
            .rows
            .iter()
            .find(|row| row.host == summary.host && row.task == summary.subject)
    };
    // A summary's time is when the host last published it, which a host
    // restart resets for every task; a chat's time is its last message.
    let last = |summary: &ActivitySummary| {
        saved(&summary.host, &summary.subject)
            .and_then(|chat| chat.updated_at.as_deref().and_then(unix_seconds))
            .or_else(|| cached(summary).and_then(|row| row.last))
            .or_else(|| known.sent.get(&summary.subject).copied())
    };
    let mut newest: Vec<(&ActivitySummary, Option<u64>)> =
        newest.into_iter().map(|s| (s, last(s))).collect();
    // Newest message first; chats with no known message time go last.
    newest.sort_by_key(|(summary, last)| std::cmp::Reverse((*last, summary.updated_at)));
    newest
        .into_iter()
        .take(SHOWN_TASKS)
        .map(|(summary, last)| {
            let label = snapshot
                .host(&summary.host)
                .map_or("a computer", |host| host.label.as_str());
            // The first line this device sent, else the transcript's title
            // (as last listed), else the host's generic headline.
            let title = known
                .titles
                .get(&summary.subject)
                .cloned()
                .unwrap_or_else(|| {
                    saved(&summary.host, &summary.subject)
                        .map(|chat| chat.title)
                        .filter(|title| named(title))
                        .or_else(|| cached(summary).and_then(|row| row.title.clone()))
                        .unwrap_or_else(|| summary.headline.clone())
                });
            // A host's own note, such as "No model capacity until ...",
            // shows under the phase; the generic phrase adds nothing.
            let note = if summary.headline != title
                && summary.headline
                    != nostr::activity_summary::generic_headline(SubjectKind::Task, summary.phase)
            {
                format!("\n{}", summary.headline)
            } else {
                String::new()
            };
            let when = last.map_or_else(String::new, |at| format!(" · {}", ago(snapshot.now, at)));
            let row = button(
                &format!("task-{}", &summary.subject[..16.min(summary.subject.len())]),
                &format!(
                    "{title}\n{} · {label}{when}{note}",
                    phase_label(summary.phase),
                ),
                Intent::Open {
                    host: summary.host.clone(),
                    task: summary.subject.clone(),
                },
            );
            (last, summary.updated_at, title, row)
        })
        .collect()
}

fn phase_label(phase: Phase) -> &'static str {
    match phase {
        Phase::Queued => "Queued",
        Phase::Running => "Working",
        Phase::Waiting => "Waiting for you",
        Phase::Completed => "Done",
        Phase::Failed => "Failed",
        Phase::Cancelled => "Stopped",
        Phase::Unknown => "Unknown",
    }
}

/// Whether a catalog title names the chat, rather than a generic one.
fn named(title: &str) -> bool {
    !title.is_empty() && !title.starts_with("Saved ")
}

/// Unix seconds of an RFC 3339 time (`2026-09-28T07:21:00Z`, with an
/// optional fraction and offset) or a bare date, as history catalogs write.
pub(crate) fn unix_seconds(text: &str) -> Option<u64> {
    let number = |range: std::ops::Range<usize>| -> Option<i64> {
        let part = text.get(range)?;
        part.bytes()
            .all(|b| b.is_ascii_digit())
            .then(|| part.parse().ok())?
    };
    let (year, month, day) = (number(0..4)?, number(5..7)?, number(8..10)?);
    if text.get(4..5) != Some("-") || text.get(7..8) != Some("-") {
        return None;
    }
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    // Days since 1970-01-01 in the proleptic Gregorian calendar.
    let (y, m) = if month <= 2 {
        (year - 1, month + 9)
    } else {
        (year, month - 3)
    };
    let era = y.div_euclid(400);
    let of_era = y - era * 400;
    let of_year = (153 * m + 2) / 5 + day - 1;
    let of_cycle = of_era * 365 + of_era / 4 - of_era / 100 + of_year;
    let days = era * 146_097 + of_cycle - 719_468;
    let mut seconds = days * 86_400;
    if text.len() > 10 {
        if !matches!(text.get(10..11), Some("T" | "t" | " ")) {
            return None;
        }
        let (hour, minute, second) = (number(11..13)?, number(14..16)?, number(17..19)?);
        if hour > 23 || minute > 59 || second > 60 {
            return None;
        }
        seconds += hour * 3_600 + minute * 60 + second;
        let mut rest = &text[19..];
        if let Some(fraction) = rest.strip_prefix('.') {
            let digits = fraction.bytes().take_while(u8::is_ascii_digit).count();
            rest = &fraction[digits..];
        }
        match rest {
            "" | "Z" | "z" => {}
            offset if offset.len() == 6 && &offset[3..4] == ":" => {
                let sign = match &offset[..1] {
                    "+" => 1,
                    "-" => -1,
                    _ => return None,
                };
                let hours: i64 = offset[1..3].parse().ok()?;
                let minutes: i64 = offset[4..6].parse().ok()?;
                seconds -= sign * (hours * 3_600 + minutes * 60);
            }
            _ => return None,
        }
    }
    u64::try_from(seconds).ok()
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

fn ago(now: u64, then: u64) -> String {
    let seconds = now.saturating_sub(then);
    match seconds {
        0..=59 => "just now".into(),
        60..=3_599 => format!("{} min ago", seconds / 60),
        3_600..=86_399 => format!("{} h ago", seconds / 3_600),
        _ => format!("{} d ago", seconds / 86_400),
    }
}

/// The quiet note under a reply that is a prepared answer.
const PREPARED: &str = "Prepared answer";

/// What **Wrong answer** says it sends before the person chooses.
const WRONG_ANSWER_SENDS: &str = "We'll send your question, our answer, and how we chose it to \
the OpenAgents team, encrypted, to improve our answers. Nothing else from this chat is sent.";

/// The chip an `open_screen` offer shows: the phone's own name for the
/// screen, never the worker's words.
fn screen_chip(screen: Screen, connecting: bool) -> (&'static str, Glyph) {
    match screen {
        Screen::Wallet => ("Open Wallet", Glyph::Wallet),
        Screen::Computers if connecting => ("Connect a computer", Glyph::Add),
        Screen::Computers => ("Your computers", Glyph::Computer),
        Screen::Keys => ("Identity keys", Glyph::Key),
        Screen::Playtest => ("Playtest", Glyph::Flag),
        Screen::Report => ("Report a problem", Glyph::Flag),
        Screen::VerseGym => ("See the board", Glyph::Check),
    }
}

/// Runs a read-only command an offer proposed on this phone, after the
/// person's tap. The phone's own Rust core answers `computer list`,
/// `show`, and `workspaces` from what it already knows; a command that
/// runs on the computer goes through [`cli_run`] instead.
pub(crate) fn run_cli(
    argv: &[String],
    runs_on: RunsOn,
    computers: Option<&Computers>,
    selected: Option<&str>,
) -> CliOutcome {
    let words: Vec<&str> = argv.iter().map(String::as_str).collect();
    let snapshot = computers.map(Computers::snapshot);
    match (runs_on, words.as_slice(), snapshot) {
        (_, ["computer", "list"], Some(snapshot)) => {
            if snapshot.hosts.is_empty() {
                return CliOutcome::Output(vec!["No computers on this phone yet.".into()]);
            }
            CliOutcome::Output(
                snapshot
                    .hosts
                    .iter()
                    .map(|host| format!("{}  {}", host.label, host_state(host, snapshot.now)))
                    .collect(),
            )
        }
        (_, ["computer", "show" | "workspaces", rest @ ..], Some(snapshot)) => {
            let named = rest.first().copied();
            let host = match named {
                Some(name) => snapshot
                    .hosts
                    .iter()
                    .find(|host| host.label == name || host.key == name),
                None => match availability(snapshot, selected) {
                    Availability::Ready(host)
                    | Availability::Connecting(host)
                    | Availability::Offline(host) => Some(host),
                    Availability::NotConfigured => None,
                },
            };
            let Some(host) = host else {
                return CliOutcome::Refused(match named {
                    Some(name) => format!("No computer named {name} on this phone."),
                    None => "No computer on this phone yet.".into(),
                });
            };
            if words[1] == "show" {
                return CliOutcome::Output(vec![
                    host.label.clone(),
                    format!("State: {}", host_state(host, snapshot.now)),
                ]);
            }
            match &host.workspaces {
                Some(labels) if !labels.is_empty() => CliOutcome::Output(labels.clone()),
                Some(_) => CliOutcome::Output(vec![format!("{} lists no workspaces.", host.label)]),
                None => CliOutcome::Refused(format!(
                    "{} hasn't listed its workspaces yet. Try again in a moment.",
                    host.label
                )),
            }
        }
        (_, ["computer", ..], None) => {
            CliOutcome::Output(vec!["No computers on this phone yet.".into()])
        }
        _ => CliOutcome::Refused("This command doesn't run on the phone.".into()),
    }
}

/// A computer's state in a word or two.
fn host_state(host: &HostRecord, now: u64) -> &'static str {
    match HostStatus::derive(host, now) {
        HostStatus::Online { .. } => "online",
        HostStatus::Connecting { .. } => "connecting",
        HostStatus::Offline { .. } => "offline",
        HostStatus::OutOfDate { .. } => "out of date",
        HostStatus::NotEnrolled { .. } => "not linked",
        HostStatus::Revoked => "revoked",
    }
}

const WHITE: Color = Color::rgb(255, 255, 255);
const GRAY: Color = Color::rgb(153, 153, 153);

fn node(key: &str, element: Element<Intent>) -> Node<Intent> {
    Node {
        key: key.into(),
        style: Style::default(),
        element,
    }
}

/// One message of a basic conversation, drawn from its parsed Markdown.
fn message(
    key: &str,
    role: MessageRole,
    blocks: Vec<rust_native::markdown::Block>,
) -> Node<Intent> {
    node(
        key,
        Element::Message {
            role,
            note: None,
            children: vec![Node {
                key: format!("{key}-md"),
                style: Style {
                    foreground: Some(WHITE),
                    ..Style::default()
                },
                element: Element::Markdown { blocks },
            }],
        },
    )
}

fn page(children: Vec<Node<Intent>>) -> Node<Intent> {
    Node {
        key: "coder".into(),
        style: Style {
            gap: Some(Space::Sm),
            padding_top: Some(Space::Md),
            padding_end: Some(Space::Md),
            padding_start: Some(Space::Md),
            ..Style::default()
        },
        element: Element::Stack {
            axis: Axis::Vertical,
            children,
        },
    }
}

/// A row whose children wrap onto more lines, as chips do.
fn wrap(key: &str, children: Vec<Node<Intent>>) -> Node<Intent> {
    let mut node = row(key, children);
    node.style.gap = Some(Space::Sm);
    if let Element::Stack { axis, .. } = &mut node.element {
        *axis = Axis::Wrap;
    }
    node
}

fn row(key: &str, children: Vec<Node<Intent>>) -> Node<Intent> {
    Node {
        key: key.into(),
        style: Style {
            gap: Some(Space::Md),
            ..Style::default()
        },
        element: Element::Stack {
            axis: Axis::Horizontal,
            children,
        },
    }
}

/// An open chat's header: the menu button for the previous chats, where
/// the chat runs, and a button for a new chat.
fn chat_header(place: Node<Intent>) -> Node<Intent> {
    let mut new = icon_button(
        "coder-new",
        "New chat",
        Glyph::Compose,
        true,
        Intent::NewChat,
    );
    new.style.align = Some(TextAlign::End);
    header(
        "coder-chat-header",
        vec![
            icon_button(
                "coder-menu",
                "Previous chats",
                Glyph::Menu,
                true,
                Intent::Menu,
            ),
            place,
            new,
        ],
    )
}

/// A header row: its children share one line, centered on it, as a title
/// beside a round button or a back link beside its status.
fn header(key: &str, children: Vec<Node<Intent>>) -> Node<Intent> {
    let mut node = row(key, children);
    node.style.align = Some(TextAlign::Center);
    node
}

fn text(key: &str, value: &str, role: TextRole, foreground: Color, bold: bool) -> Node<Intent> {
    Node {
        key: key.into(),
        style: Style {
            foreground: Some(foreground),
            weight: bold.then_some(TextWeight::Bold),
            ..Style::default()
        },
        element: Element::Text {
            value: value.into(),
            role,
        },
    }
}

fn heading(key: &str, value: &str) -> Node<Intent> {
    text(key, value, TextRole::Heading, WHITE, true)
}

fn body(key: &str, value: &str) -> Node<Intent> {
    text(key, value, TextRole::Body, WHITE, false)
}

fn status(key: &str, value: &str) -> Node<Intent> {
    text(key, value, TextRole::Status, GRAY, false)
}

/// A button that draws `glyph`: in a circle when `circular`, with the label
/// as its spoken name, or before the visible label.
fn icon_button(
    key: &str,
    label: &str,
    glyph: Glyph,
    circular: bool,
    intent: Intent,
) -> Node<Intent> {
    let mut node = button(key, label, intent);
    if let Element::Button { icon, .. } = &mut node.element {
        *icon = Some(Icon {
            glyph,
            circular,
            pill: false,
        });
    }
    node
}

/// A chip: `glyph` and the visible label in a filled capsule, as a
/// suggested action or the target selector.
fn pill(key: &str, label: &str, glyph: Glyph, intent: Intent) -> Node<Intent> {
    let mut node = button(key, label, intent);
    if let Element::Button { icon, .. } = &mut node.element {
        *icon = Some(Icon {
            glyph,
            circular: false,
            pill: true,
        });
    }
    node
}

fn button(key: &str, label: &str, intent: Intent) -> Node<Intent> {
    Node {
        key: key.into(),
        style: Style {
            foreground: Some(WHITE),
            ..Style::default()
        },
        element: Element::Button {
            label: label.into(),
            enabled: true,
            icon: None,
            intent,
        },
    }
}
