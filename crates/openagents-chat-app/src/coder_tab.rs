//! The Coder tab: conversations with Coder, on the phone and on your
//! computers.
//!
//! The tab opens on a new chat, ready to type: a composer with the cursor
//! in it, and, above the field, a few suggested questions the person has
//! not used yet (tapped, or sent the same words). Every new chat goes
//! to OpenAgents; there is no target to pick. Coder runs on a computer only
//! from a router offer under the reply that warrants it (**Run Coder**, or
//! **Connect a computer** with none), never from a control above the field.
//! A coding reply to a message sent here starts Coder at once, with no tap,
//! on a ready computer whose owner allows it ([`CoderTab::start_offered`],
//! #10101), and the reply shows that start with **Stop** instead.
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

use crate::basic_chats::{BasicChats, Tail};
use crate::chats::{Chats, Head};
use crate::cli_run::{self, RemoteCli};
use crate::coder_list::{List, Row, Store};
use crate::conversation::{Conversation, Pending};
use crate::outbox::{Attempt, Draft, Outbox};
use crate::router::{Context, Offer, RunsOn, Screen};
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

mod phone_sync;
mod shell;
pub use shell::{ShellAction, ShellView};

/// The largest message, as NIP-HOST `task.create` allows.
const MAX_PROMPT_BYTES: usize = 16 * 1024;
const SHOWN_TASKS: usize = 50;
/// The most basic conversations the list shows.
const SHOWN_TALKS: usize = 512;
/// One chat in the previous chats: its last message time and summary time,
/// for ordering, and its row.
struct Recent {
    group: crate::chat_list::Group,
    last: Option<u64>,
    updated: u64,
    row: Node<Intent>,
}

/// The shared list order (#10100): Pinned, then every chat newest first
/// whatever its project (the row names the project), then Archived, as
/// [`crate::chat_list::search`] orders the desktop sidebar.
fn order_recent(rows: &mut [Recent]) {
    rows.sort_by_key(|recent| {
        (
            recent.group.rank(),
            std::cmp::Reverse((recent.last, recent.updated)),
        )
    });
}

/// The key of a workspace of a computer in the list's record of when this
/// device last started a chat there.
fn used_key(host: &str, workspace: &str) -> String {
    format!("{host} {workspace}")
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Intent {
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
    /// Open a computer's own thread: one its desktop app or `openagents
    /// chat` started there.
    OpenThread {
        host: String,
        thread: String,
    },
    /// Stop the Coder task a computer's thread started, through the
    /// task's own stop, after its reply was stopped here.
    StopThreadCoder {
        host: String,
        task: String,
    },
    /// Run Coder on the chosen computer with the open conversation.
    RunCoder,
    /// Send the person to connect a computer.
    ConnectComputer,
    /// Ask the basic Coder again for the last message's reply.
    Retry,
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
    /// Back to the Gym menu, for a person who opted into the Gym.
    Hub,
    /// Start a new chat with the suggestion `id`'s words
    /// ([`crate::first_run::SUGGESTIONS`]).
    Starter {
        id: String,
    },
    /// A choice from a saved chat's card menu: the shared chat commands'
    /// menu (`commands::Kind::Menu`), as the desktop offers it.
    ChatMenu {
        id: String,
        action: ChatMenuAction,
    },
    /// Ask the host for a photo to attach to the draft.
    AttachImage,
    /// Take an attached image off the draft.
    RemoveImage {
        id: String,
    },
    /// Show the open chat's change line by line.
    OpenChanges,
    CloseChanges,
    /// Show the change as it is now, after the view went stale.
    RefreshChanges,
    /// Publish the reviewed change on the computer (`task.publish`).
    PublishChanges,
}

/// The chat card menu's choices the phone carries out. Rename needs a text
/// field of its own and stays on the desktop for now.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatMenuAction {
    Pin,
    Unpin,
    Archive,
    Restore,
}

/// A screen of another tab the host should show, once, or something the
/// host should do for the tab.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Go {
    /// Account > Computers.
    Computers,
    /// **Connect a computer** (`SCR-22`): the scanner. The app opens it
    /// itself and never hands this to the host.
    Connect,
    /// The Wallet tab.
    Wallet,
    /// Account > Identity keys.
    Keys,
    /// Account > Playtest.
    Playtest,
    /// Report a problem, for the chat on screen.
    Report,
    /// The Verse tab, walked into the Gym before its EVALS board: the host
    /// sends the world `go_evals`.
    VerseGym,
    /// The Verse: the Grid world, from a reply's **Enter the Grid**.
    Verse,
    /// The Chat tab, after **Train Coder** from another tab.
    Chat,
    /// The chat tab's own screens changed (menu, chat, or the Gym intro):
    /// the host shows the one `gym.screen` names.
    Gym,
    /// Open the system photo picker; the host sends the chosen image's
    /// bytes back to attach them to the draft.
    PickImage,
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
            // The Gym's sheets open inside the chat.
            Screen::GymResult | Screen::GymPublish | Screen::GymTestSet => Go::Gym,
            // The route map is the desktop app's; the worker offers it
            // only to a desktop turn, and the phone stays in the chat.
            Screen::RoutesMap => Go::Chat,
            Screen::Verse => Go::Verse,
        }
    }
}

/// A command the person ran from an offer, and what came of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CliOutcome {
    /// The command's output, one line each.
    Output(Vec<String>),
    /// It is running on the named computer.
    Running(String),
    /// Why it did not run.
    Refused(String),
}

/// Another way to send the composer's text while Coder works, which a long
/// press on the send control offers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    /// Queue the message for the next turn: the send control's own action.
    Queue,
    /// Replace the instructions of a turn that has not started.
    SteerNow,
    /// Stop the running turn and continue with the message: the engine's
    /// emulated steering.
    StopAndSend,
}

impl Choice {
    pub fn label(self) -> &'static str {
        match self {
            Choice::Queue => "Queue for next turn",
            Choice::SteerNow => "Steer now",
            Choice::StopAndSend => "Stop and send",
        }
    }

    pub fn command(self) -> (CommandAction, bool) {
        match self {
            Choice::Queue => (CommandAction::Queue, false),
            Choice::SteerNow => (CommandAction::Steer, false),
            Choice::StopAndSend => (CommandAction::Steer, true),
        }
    }

    /// The choices a busy chat offers: a turn that has not started takes new
    /// instructions; a running one stops for the message.
    pub fn offered(phase: Option<Phase>) -> Vec<Choice> {
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
    /// The finished task's change, its staleness, and its publication, as
    /// the computer reads it (`task.review`).
    review: crate::changes::Reviewer,
    /// The completed summary sequence the review follows.
    reviewed: Option<u64>,
    /// The change shows line by line.
    changes_open: bool,
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
    notice: Option<String>,
    composers: u64,
    /// The chats list as last seen, with the first line and send time of
    /// each chat this device started; it survives a relaunch.
    list: Store,
    open: Option<Open>,
    /// The previous chats show over the tab.
    drawer: bool,
    /// The intro's chat was reopened, or left, since launch: FLOW-01
    /// reopens it once per launch, and never after the person leaves it.
    intro_reopened: bool,
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
    projection: crate::projection::Projection,
    /// The open basic conversation.
    talk: Option<String>,
    /// The tab shows: the app opens on it.
    shown: bool,
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
    /// Debug builds' screenshot script: messages to send one reply at a
    /// time, and `!run` (the first offered command) steps.
    script: std::collections::VecDeque<String>,
    /// The Gym in chat: cards, sheets, runs, the menu, and the first run.
    pub gym: crate::gym::Gym,
    /// Text the next composer puts in its field, as **Change it** does.
    compose: Option<String>,
    /// The computer this device connected last: **Run Coder** goes there
    /// while it is ready.
    preferred: Option<String>,
    /// The computers' own threads, and the one open here.
    threads: crate::host_threads::HostThreads,
    /// How those threads are reached; `None` without the live client.
    thread_link: Option<Arc<dyn crate::host_threads::Link>>,
    /// The open computer thread's rendered turns.
    thread_projection: crate::projection::Projection,
    /// Images attached to each conversation's draft, bounded and decoded by
    /// the shared attachments code the desktop uses. Always empty while
    /// [`CoderTab::attachments_enabled`] is off.
    images: crate::attachments::Drafts,
    /// Whether the chat takes images ([`ATTACHMENTS_ENABLED`]).
    attachments: bool,
    /// What each conversation's Coder task did in its last turn, read from
    /// its chat when a follow-up left it for the conversation (#10094).
    talk_runs: std::collections::BTreeMap<String, openagents_chat::router::CoderRun>,
    /// The messages this device sent whose replies it waits for: each
    /// conversation's newest, by its user turn's index. A coding reply to
    /// one starts Coder at once where the computer allows it (#10101); a
    /// reply read any other way (an old chat, a relaunch) never does.
    awaiting: Vec<(String, usize)>,
    /// The read of the shown conversation's ended Coder task for the
    /// outcome its start card shows, and each ended summary already read,
    /// by task and sequence, so each is read once.
    outcome_read: Option<OutcomeRead>,
    outcome_tried: std::collections::BTreeSet<(String, u64)>,
    /// The phone's shell (#11126): the host draws the top bar, the drawer,
    /// and the feature cards, from [`CoderTab::shell_view`].
    shell: shell::Shell,
    /// The link cards under replies, and the pages they still need read.
    links: crate::links::LinkPreviews,
}

/// A read of an ended Coder task's chat for the one line its start card
/// shows: the computer's newest chats, then the task's transcript, as an
/// opened Coder chat reads them.
struct OutcomeRead {
    talk: String,
    host: String,
    task: String,
    sequence: u64,
    phase: Phase,
    round: Option<u64>,
    conversation: Option<Conversation>,
}

/// The most sent messages whose replies the tab waits for at once.
const MAX_AWAITING: usize = 32;

/// The most turns an open basic conversation shows at first.
const TALK_TURNS: usize = 200;

/// Attachments, everywhere (#10093 phone, #10095 desktop): off as of
/// 2026-10-01, so the phone and the desktop are text only. The phone chat
/// shows no attach control, never asks the host for a photo
/// ([`Go::PickImage`]), drops an image the host hands it
/// ([`CoderTab::attach_image`]) without a notice, and sends a draft's words
/// only, dropping any images it still holds. Hosts mount their photo picker
/// only while the packet says attachments are on. The desktop composer reads
/// this same switch: no attach control or image picker, text-only paste, a
/// dropped image dropped, and a draft's words only. The shared image
/// pipeline (`crate::attachments`, #10066/#10070) is unchanged; set this to
/// `true` to turn attachments back on in both apps.
pub const ATTACHMENTS_ENABLED: bool = false;

impl CoderTab {
    pub fn new(instance: String) -> Self {
        Self {
            instance,
            revision: 0,
            current: None,
            notice: None,
            composers: 1,
            list: Store::open(None),
            open: None,
            drawer: false,
            intro_reopened: false,
            outbox: Outbox::open(None),
            choices: Vec::new(),
            transcripts: Transcripts::open(None),
            echoes: Vec::new(),
            echoed: 0,
            pulled: false,
            basic: BasicChats::empty(),
            projection: crate::projection::Projection::default(),
            talk: None,
            shown: true,
            go: None,
            talk_turns: TALK_TURNS,
            app_build: None,
            cli: None,
            remote: None,
            running: None,
            script: std::collections::VecDeque::new(),
            gym: crate::gym::Gym::empty(),
            compose: None,
            preferred: None,
            threads: crate::host_threads::HostThreads::default(),
            thread_link: None,
            thread_projection: crate::projection::Projection::default(),
            images: crate::attachments::Drafts::default(),
            talk_runs: std::collections::BTreeMap::new(),
            awaiting: Vec::new(),
            outcome_read: None,
            outcome_tried: std::collections::BTreeSet::new(),
            attachments: ATTACHMENTS_ENABLED,
            shell: shell::Shell::default(),
            links: crate::links::LinkPreviews::default(),
        }
    }

    /// Whether the chat takes images: [`ATTACHMENTS_ENABLED`] unless
    /// [`CoderTab::set_attachments`] changed it. Hosts mount their photo
    /// picker only while this is on.
    pub fn attachments_enabled(&self) -> bool {
        self.attachments
    }

    /// Turn image attachments on or off for this tab; the shared image
    /// pipeline's phone tests turn them on. Turning them off hides any
    /// images a draft holds, and its next send or Run Coder drops them.
    pub fn set_attachments(&mut self, on: bool) {
        self.attachments = on;
    }

    /// While attachments are off, drop any images a draft still holds, so
    /// the draft is words only. Quiet: nothing to tell the person.
    fn text_only(&mut self) {
        if !self.attachments {
            self.images = crate::attachments::Drafts::default();
            if self.go == Some(Go::PickImage) {
                self.go = None;
            }
        }
    }

    /// Read the computers' own threads through `link`, ringing `threads`'
    /// wake when they change.
    pub fn with_threads(
        mut self,
        threads: crate::host_threads::HostThreads,
        link: Option<Arc<dyn crate::host_threads::Link>>,
    ) -> Self {
        self.threads = threads;
        self.thread_link = link;
        self
    }

    /// Read each computer this device may observe for its threads, when
    /// due.
    fn poll_threads(&self, computers: Option<&Computers>) {
        let (Some(link), Some(computers)) = (&self.thread_link, computers) else {
            return;
        };
        let hosts = computers
            .snapshot()
            .hosts
            .iter()
            .filter(|host| {
                matches!(
                    &host.enrollment,
                    coder_computers::Enrollment::Enrolled { rights, .. }
                        if rights.contains(coder_host::access::Right::Observe)
                )
            })
            .map(|host| (host.key.clone(), host.label.clone()))
            .collect();
        self.threads.poll(hosts, link);
    }

    /// Whether this device may send to `host`'s threads.
    fn operates(computers: Option<&Computers>, host: &str) -> bool {
        computers
            .and_then(|c| c.snapshot().host(host))
            .is_some_and(|record| {
                matches!(
                    &record.enrollment,
                    coder_computers::Enrollment::Enrolled { rights, .. }
                        if rights.contains(coder_host::access::Right::Operate)
                )
            })
    }

    /// Send **Run Coder** to `host` while it is ready, as after the person
    /// connected it.
    pub fn prefer(&mut self, host: String) {
        self.preferred = Some(host);
    }

    /// Keep the Gym's state in `gym`.
    pub fn with_gym(mut self, gym: crate::gym::Gym) -> Self {
        self.gym = gym;
        self
    }

    /// Hide the Gym in this build (the phone's release gate,
    /// `docs/mobile/1.0-audit.md`): no intro, menu, Profile, card, Gym
    /// starter chip, or Gym and game screen chip shows. See
    /// [`crate::gym::Gym::hide`].
    pub fn hide_gym(&mut self) {
        self.gym.hide();
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
                let context = self.router_context(computers);
                self.basic.set_context(context);
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
    pub fn with_remote_cli(mut self, remote: Option<Arc<dyn RemoteCli>>) -> Self {
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
            run_cli(&argv, runs_on, computers, None)
        } else {
            match cli_run::target(&argv, computers, None) {
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
    pub fn shared_chat(&mut self) -> Option<playtest::report::SharedChat> {
        let id = self.talk.clone()?;
        self.basic.shared(&id)
    }

    /// Where **Give feedback**'s `text` came from (#10127): the open chat
    /// with OpenAgents and the message whose transcript row is `row`
    /// (`talk-m3`), or the open Coder task when that is what shows.
    pub fn feedback_selection(
        &mut self,
        text: &str,
        row: Option<&str>,
    ) -> playtest::report::Selection {
        let index = row.and_then(|row| crate::feedback::turn_index(row, "talk-m"));
        if let Some(id) = self.talk.clone() {
            let turns = self.basic.turns(&id);
            return crate::feedback::selection(text, Some(&id), turns, 0, index);
        }
        let task = self.open.as_ref().map(|open| open.task.clone());
        crate::feedback::selection(text, task.as_deref(), &[], 0, None)
    }

    /// What the next basic turn tells the worker: whether a computer is
    /// ready and, when one is, its name (the label the person gave it), so
    /// the chat never asks them to connect one (#10077); the open chat's
    /// project folder by name when its Coder task named one, never a path;
    /// and the build. It also carries the open chat's test-set draft and
    /// its last try, which only the phone keeps.
    fn router_context(&mut self, computers: Option<&Computers>) -> Context {
        let (draft, tried) = match self.talk.clone() {
            Some(id) => {
                let turns = self.basic.turns(&id).to_vec();
                self.gym.context_for(&id, &turns)
            }
            None => (None, None),
        };
        // A paired computer is named even while it is offline: the person
        // has one, so the chat never asks them to connect one.
        let availability = self.availability(computers);
        let ready = matches!(availability, Availability::Ready(_));
        let paired = match availability {
            Availability::Ready(host)
            | Availability::Connecting(host)
            | Availability::Offline(host) => Some((host.label.clone(), engines_of(host))),
            Availability::NotConfigured => None,
        };
        let project = self
            .talk
            .as_deref()
            .and_then(|id| self.basic.get(id))
            .and_then(|summary| summary.coder.as_ref())
            .and_then(|spawned| spawned.project.clone())
            .map(|name| crate::router::Project { name, path: None });
        Context {
            surface: crate::router::Surface::Phone,
            client: None,
            computer_ready: ready,
            computer: paired
                .filter(|(name, _)| !name.trim().is_empty())
                .map(|(name, engines)| crate::router::Computer::Paired { name, engines }),
            project,
            coder_run: self.talk_run(computers),
            app_build: self.app_build.clone(),
            draft,
            tried,
            skip: self.gym.skip(),
            runs: Vec::new(),
        }
    }

    /// The open conversation's Coder task, as the router's context: once
    /// its turn has ended, what the task's chat showed when the follow-up
    /// left it, else only how it ended (#10094); while it runs or waits,
    /// that it does, with its computer's headline (#10143).
    fn talk_run(&self, computers: Option<&Computers>) -> Option<openagents_chat::router::CoderRun> {
        let id = self.talk.as_deref()?;
        let spawned = self.basic.get(id)?.coder.as_ref()?;
        let summary = Self::summary(computers?.snapshot(), &spawned.host, &spawned.task);
        let Some(ending) = summary.and_then(|summary| ending_of(summary.phase)) else {
            return Some(going_run(summary));
        };
        Some(
            self.talk_runs
                .get(id)
                .filter(|run| run.ending == ending)
                .cloned()
                .unwrap_or(openagents_chat::router::CoderRun {
                    ending,
                    turn: 1,
                    engine: None,
                    model: None,
                    summary: String::new(),
                    files: Vec::new(),
                    commands: Vec::new(),
                }),
        )
    }

    /// The conversation that started `task` on `host`, if this phone holds
    /// it.
    fn talk_of(&self, host: &str, task: &str) -> Option<String> {
        self.basic
            .list()
            .iter()
            .find(|summary| {
                summary
                    .coder
                    .as_ref()
                    .is_some_and(|spawned| spawned.host == host && spawned.task == task)
            })
            .map(|summary| summary.id.clone())
    }

    /// Whether `task` on `host` has ended its turn: a follow-up goes to
    /// OpenAgents, not straight to Coder (#10094).
    fn ended(computers: Option<&Computers>, host: &str, task: &str) -> bool {
        computers
            .and_then(|c| Self::summary(c.snapshot(), host, task))
            .is_some_and(|summary| ending_of(summary.phase).is_some())
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

    /// The conversation a draft's images belong to: the open basic chat,
    /// computer thread, or Coder chat, or the new chat.
    fn draft_key(&self) -> String {
        if let Some(id) = &self.talk {
            return format!("talk:{id}");
        }
        if let Some((host, thread)) = self.threads.opened() {
            return format!("thread:{host}:{thread}");
        }
        if let Some(open) = &self.open {
            return format!("task:{}:{}", open.host, open.task);
        }
        "new".into()
    }

    /// Attach an image the host's picker read. Decoding and its bounds are
    /// the shared attachments code's; a refusal shows as the tab's notice.
    ///
    /// While attachments are off ([`ATTACHMENTS_ENABLED`]) the image is
    /// dropped quietly: no decode, no notice, and the draft stays words only.
    pub fn attach_image(&mut self, name: &str, bytes: Vec<u8>) {
        if !self.attachments {
            return;
        }
        let key = self.draft_key();
        let result = crate::attachments::Image::decode(name, bytes)
            .and_then(|image| self.images.add(&key, image));
        self.notice = result.err();
    }

    /// The attached image an `image:{id}` surface shows, from the open
    /// draft.
    pub fn image(&self, resource: &str) -> Option<&crate::attachments::Image> {
        if !self.attachments {
            return None;
        }
        let id = resource.strip_prefix("image:")?;
        self.images
            .get(&self.draft_key())
            .iter()
            .find(|image| image.id == id)
    }

    /// The draft's attachments above the composer: an attach control and a
    /// card per image, whose surface shows the image and whose label is its
    /// alternative text. Nothing while attachments are off.
    fn attachments(&self) -> Option<Node<Intent>> {
        if !self.attachments {
            return None;
        }
        let mut children = vec![icon_button(
            "coder-attach",
            "Attach image",
            Glyph::Paperclip,
            true,
            Intent::AttachImage,
        )];
        for image in self.images.get(&self.draft_key()) {
            children.push(Node {
                key: format!("image-{}", image.id),
                style: Style {
                    gap: Some(Space::Xs),
                    ..Style::default()
                },
                element: Element::Stack {
                    axis: Axis::Vertical,
                    children: vec![
                        node(
                            &format!("image-preview-{}", image.id),
                            Element::Surface {
                                resource: format!("image:{}", image.id),
                                label: format!(
                                    "{} · {} × {}",
                                    image.name, image.width, image.height
                                ),
                            },
                        ),
                        button(
                            &format!("image-remove-{}", image.id),
                            "Remove",
                            Intent::RemoveImage {
                                id: image.id.clone(),
                            },
                        ),
                    ],
                },
            });
        }
        Some(Node {
            key: "coder-attachments".into(),
            style: Style {
                gap: Some(Space::Sm),
                padding_start: Some(Space::Sm),
                padding_end: Some(Space::Sm),
                ..Style::default()
            },
            element: Element::Stack {
                axis: Axis::Wrap,
                children,
            },
        })
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
        self.basic.streaming() || !self.script.is_empty() || self.threads.live()
    }

    /// Whether a chat, basic or on a computer, is open.
    pub fn in_chat(&self) -> bool {
        self.open.is_some() || self.talk.is_some() || self.threads.opened().is_some()
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
        self.keep_review(computers);
    }

    /// Read the open chat's change once its task completed, again now and
    /// then while it shows, and publish it when asked, all through the
    /// computer under this device's grant: `observe` to read, `operate` to
    /// publish.
    fn keep_review(&mut self, computers: &mut Computers) {
        let Some(open) = self.open.as_mut() else {
            return;
        };
        let completed = Self::summary(computers.snapshot(), &open.host, &open.task)
            .filter(|summary| summary.phase == Phase::Completed)
            .map(|summary| summary.sequence);
        if completed.is_some() && completed != open.reviewed {
            open.reviewed = completed;
            open.review.reset(Some(&open.task));
        }
        let Some(need) = open
            .review
            .tick(std::time::Instant::now(), completed.is_some(), true)
        else {
            return;
        };
        let (host, task) = (open.host.clone(), open.task.clone());
        match need {
            crate::changes::Need::Read { .. } => {
                let result = computers.review_task(&host, &task);
                let Some(open) = self.open.as_mut().filter(|open| open.task == task) else {
                    return;
                };
                open.review.read(
                    result.map_err(|refusal| match refusal_code(&refusal) {
                        // An older computer, or a task with no worktree of
                        // its own there: no card.
                        Some(coder_host::Code::Unsupported | coder_host::Code::Malformed) => {
                            crate::changes::ReadFailure::Unsupported
                        }
                        _ => crate::changes::ReadFailure::Failed(refusal.reason()),
                    }),
                    std::time::Instant::now(),
                );
            }
            crate::changes::Need::Publish {
                base,
                head_commit,
                head,
                ..
            } => {
                let result = computers.publish_task(&host, &task, &base, &head_commit, &head);
                if let Some(open) = self.open.as_mut().filter(|open| open.task == task) {
                    open.review
                        .published(result.map_err(|refusal| refusal.reason()));
                }
            }
        }
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
        let (host, task) = (open.host.clone(), open.task.clone());
        self.command_on(&host, &task, action, text, emulate, computers)
    }

    /// Send a command to `task` on `host`, open here or not.
    fn command_on(
        &mut self,
        host: &str,
        task: &str,
        action: CommandAction,
        text: &str,
        emulate: bool,
        computers: &mut Computers,
    ) -> bool {
        let based_on =
            Self::summary(computers.snapshot(), host, task).map_or(1, |summary| summary.sequence);
        let (host, task) = (host.to_owned(), task.to_owned());
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
        if self.basic.streaming() || self.running.is_some() || self.threads.live() {
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
    pub fn notice_shown(&self) -> bool {
        self.notice.is_some()
    }

    /// The host and task of the open chat.
    pub fn open_task(&self) -> Option<(String, String)> {
        self.open
            .as_ref()
            .map(|open| (open.host.clone(), open.task.clone()))
    }

    /// The workspace Coder runs in on `host`: the one this device used
    /// there last, else `openagents`, else the computer's first.
    fn workspace(&self, host: &HostRecord) -> Option<String> {
        let listed = host.workspaces.as_ref()?;
        openagents_chat::delegation::project(listed, |label| self.used(&host.key, label))
    }

    /// When this device last started a chat in `workspace` on `host`.
    fn used(&self, host: &str, workspace: &str) -> Option<u64> {
        self.list.list.used.get(&used_key(host, workspace)).copied()
    }

    /// Remember, across a relaunch, that reply `reply` of conversation `id`
    /// started or continued Coder task `task` at `now`.
    fn mark_started(&mut self, id: &str, reply: usize, task: &str, now: u64) {
        self.list.list.started.insert(
            id.to_owned(),
            crate::coder_list::Started {
                reply,
                task: task.to_owned(),
                at: now,
                outcome: None,
            },
        );
        self.list.save();
    }

    /// The reply in conversation `id` that started or continued its Coder
    /// task, by turn index.
    fn started_reply(&self, id: &str) -> Option<usize> {
        self.list.list.started.get(id).map(|started| started.reply)
    }

    /// Read the outcome of the shown conversation's Coder task once its turn
    /// ends, for its start card: the computer's newest chats, then the
    /// task's transcript, once per ended summary.
    fn read_outcome(&mut self, computers: Option<&Computers>, chats: &mut Chats) {
        let wanted = self.talk.as_deref().and_then(|id| {
            let spawned = self.basic.get(id)?.coder.as_ref()?;
            let started = self.list.list.started.get(id)?;
            if started.task != spawned.task || started.outcome.is_some() {
                return None;
            }
            let summary = Self::summary(computers?.snapshot(), &spawned.host, &spawned.task)?;
            matches!(summary.phase, Phase::Completed | Phase::Failed).then(|| {
                (
                    id.to_owned(),
                    spawned.host.clone(),
                    spawned.task.clone(),
                    summary.sequence,
                    summary.phase,
                )
            })
        });
        let Some((talk, host, task, sequence, phase)) = wanted else {
            self.outcome_read = None;
            return;
        };
        let current = self.outcome_read.as_ref().is_some_and(|read| {
            read.talk == talk && read.task == task && read.sequence == sequence
        });
        if !current {
            if !self.outcome_tried.insert((task.clone(), sequence)) {
                self.outcome_read = None;
                return;
            }
            let round = chats.refresh_head(&host);
            self.outcome_read = Some(OutcomeRead {
                talk,
                host,
                task,
                sequence,
                phase,
                round,
                conversation: None,
            });
        }
        let Some(read) = self.outcome_read.as_mut() else {
            return;
        };
        let Some(conversation) = &read.conversation else {
            if read
                .round
                .is_some_and(|round| chats.head(&read.host, round) == Head::Running)
            {
                return;
            }
            match chats.coder_chat(&read.host, &read.task) {
                Some((_, client, chat)) => {
                    read.conversation = Some(Conversation::open(chats.runtime(), client, chat));
                }
                None => self.outcome_read = None,
            }
            return;
        };
        if conversation.failed() {
            self.outcome_read = None;
            return;
        }
        if !conversation.read_since(0) {
            return;
        }
        let line = outcome(&conversation.rows(), read.phase);
        let talk = read.talk.clone();
        self.outcome_read = None;
        if let Some(line) = line
            && let Some(started) = self.list.list.started.get_mut(&talk)
        {
            started.outcome = Some(line);
            self.list.save();
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

    /// A stop, or a Coder link, tapped on a computer's thread just before
    /// the view the phone held was replaced. An open thread renders a new
    /// view every few hundred milliseconds while it streams or its Coder
    /// task runs, so the tap often names the previous one; it still counts
    /// when the current view offers the same control
    /// ([`ValidatedView::activate_late`]), resolved against the current
    /// view. That is safe: `thread.stop` names the message the thread
    /// answers now and changes nothing otherwise, and **Open Coder** and
    /// **Stop Coder too** name the task the open thread links to now.
    /// Every other stale tap is still ignored.
    fn late_thread_stop<'a>(
        &self,
        view: &'a ValidatedView<Intent>,
        event: &Activation,
    ) -> Option<&'a Intent> {
        self.threads.opened()?;
        let node = event.node.as_str();
        view.activate_late(event, |intent| match intent {
            Intent::Stop => node == "coder-composer",
            Intent::Open { .. } => node == "thread-coder-open",
            Intent::StopThreadCoder { .. } => node == "thread-coder-stop",
            _ => false,
        })
        .ok()
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
            .and_then(|view| {
                view.activate(event)
                    .ok()
                    .or_else(|| self.late_thread_stop(view, event))
            })
            .cloned()
        else {
            return;
        };
        self.run_intent(intent, computers, chats);
    }

    /// Carry out `intent`, from a tap on the view or from the shell.
    fn run_intent(&mut self, intent: Intent, computers: Option<&mut Computers>, chats: &mut Chats) {
        match intent {
            Intent::Open { host, task } => {
                self.drawer = false;
                self.talk = None;
                self.threads.close();
                self.open(host, task, chats);
            }
            // Back from the previous chats: the screen under them.
            Intent::Back if self.drawer => self.drawer = false,
            // A new chat, or back from a chat: a new chat, ready to type.
            Intent::NewChat | Intent::Back => {
                self.keep(true);
                self.open = None;
                self.talk = None;
                self.threads.close();
                self.drawer = false;
                self.notice = None;
                self.intro_reopened = true;
                // A new composer, so the field takes the cursor again.
                self.composers += 1;
            }
            Intent::Menu => {
                self.drawer = true;
                self.notice = None;
            }
            Intent::OpenThread { host, thread } => {
                let Some(link) = self.thread_link.clone() else {
                    return;
                };
                self.keep(true);
                self.open = None;
                self.talk = None;
                self.drawer = false;
                self.notice = None;
                self.composers += 1;
                self.thread_projection = crate::projection::Projection::default();
                self.threads.open(&host, &thread, link);
            }
            Intent::ChatMenu { id, action } => {
                let result = match action {
                    ChatMenuAction::Pin => self.basic.pin(&id, true),
                    ChatMenuAction::Unpin => self.basic.pin(&id, false),
                    ChatMenuAction::Archive => {
                        self.basic.archive(&id, unix_now());
                        Ok(())
                    }
                    ChatMenuAction::Restore => {
                        self.basic.restore(&id);
                        Ok(())
                    }
                };
                self.notice = result.err();
            }
            Intent::AttachImage => {
                if self.attachments {
                    self.go = Some(Go::PickImage);
                }
            }
            Intent::RemoveImage { id } => {
                let key = self.draft_key();
                self.images.remove(&key, &id);
            }
            Intent::OpenTalk { id } => {
                self.keep(true);
                self.threads.close();
                self.open = None;
                self.drawer = false;
                self.notice = None;
                self.talk_turns = TALK_TURNS;
                self.talk = Some(id);
            }
            Intent::Retry => {
                let context = self.router_context(computers.as_deref());
                self.basic.set_context(context);
                if let Some(id) = self.talk.clone() {
                    self.basic.retry(&id);
                    self.await_reply(&id);
                }
            }
            Intent::ConnectComputer => self.go = Some(Go::Connect),
            Intent::OpenScreen { screen } => {
                // Only a screen an offer under the last reply names.
                let offered = self
                    .talk
                    .as_ref()
                    .and_then(|id| self.basic.last_meta(id))
                    .is_some_and(|meta| meta.offers.contains(&Offer::OpenScreen { screen }));
                let hidden = self.gym.hidden() && crate::cards::preview_screen(screen);
                if offered && !hidden {
                    let draft = self
                        .talk
                        .as_ref()
                        .and_then(|id| crate::gym::Gym::draft_of(self.basic.turns(id)))
                        .is_some();
                    match screen {
                        Screen::GymPublish | Screen::GymResult | Screen::GymTestSet => {
                            self.gym.open_screen(screen, self.talk.as_deref(), draft);
                        }
                        // With no computer, the Computers offer is Connect a
                        // computer: the scanner.
                        Screen::Computers
                            if matches!(
                                self.availability(computers.as_deref()),
                                Availability::NotConfigured
                            ) =>
                        {
                            self.go = Some(Go::Connect);
                        }
                        // Under the shell, the switch goes to the Verse.
                        Screen::Verse if self.shell.on => {
                            self.shell.verse = true;
                        }
                        _ => self.go = Some(Go::of(screen)),
                    }
                }
            }
            Intent::Followup { index } if self.threads.opened().is_some() => {
                let label = self.threads.shown().and_then(|shown| {
                    shown.turns.last().and_then(|turn| {
                        turn.meta.as_ref().and_then(|meta| {
                            meta.followups
                                .get(index)
                                .map(|followup| followup.label.clone())
                        })
                    })
                });
                let Some(label) = label else { return };
                if self.threads.send(&label) {
                    self.notice = None;
                } else {
                    self.notice = Some("Wait for this reply to finish.".into());
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
                if let Some(answer) = &followup.answer {
                    self.basic.use_suggestion(answer);
                }
                let context = self.router_context(computers.as_deref());
                self.basic.set_context(context);
                if self.basic.send(&id, &followup.label, unix_now()) {
                    self.await_reply(&id);
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
            Intent::Hub => self.hub(),
            Intent::Starter { id } => {
                let Some(suggestion) = crate::first_run::SUGGESTIONS
                    .iter()
                    .find(|suggestion| suggestion.id == id)
                    .filter(|s| !(self.gym.hidden() && crate::cards::preview_answer(s.id)))
                else {
                    return;
                };
                // Tapped once, it never shows again on this device.
                self.basic.use_suggestion(suggestion.id);
                self.start_talk(suggestion.message, computers.as_deref());
            }
            Intent::RunCoder if self.threads.opened().is_some() => {
                let Some((host, _)) = self.threads.opened() else {
                    return;
                };
                // The thread's Coder task ended its turn: the message the
                // reply handed to Coder is that task's next turn (#10094).
                if let Some(shown) = self.threads.shown()
                    && let Some(coder) = shown.coder.clone()
                    && Self::ended(computers.as_deref(), &coder.host, &coder.task)
                {
                    let Some(computers) = computers else { return };
                    if !Self::operates(Some(computers), &coder.host) {
                        self.notice = Some("This phone can only read this chat".into());
                        return;
                    }
                    if let Some(text) = shown
                        .turns
                        .iter()
                        .rev()
                        .find(|turn| turn.role == crate::basic_coder::Role::User)
                        .map(|turn| turn.text.clone())
                    {
                        self.command_on(
                            &coder.host,
                            &coder.task,
                            CommandAction::Send,
                            &text,
                            false,
                            computers,
                        );
                    }
                    return;
                }
                let Some(link) = self.thread_link.clone() else {
                    self.notice = Some("Couldn't reach the computer.".into());
                    return;
                };
                if !Self::operates(computers.as_deref(), &host) {
                    self.notice = Some("This phone can only read this chat".into());
                    return;
                }
                if self.threads.run(link) {
                    self.notice = None;
                } else {
                    self.notice = Some("Coder is already starting.".into());
                }
            }
            Intent::RunCoder => {
                let Some(computers) = computers else {
                    self.go = Some(Go::Connect);
                    return;
                };
                self.run_coder(computers, chats);
            }
            Intent::Earlier if self.threads.opened().is_some() => {
                if let Some(link) = self.thread_link.clone() {
                    self.threads.earlier(link);
                }
            }
            // A computer's thread answers on the computer, which stops it
            // (NIP-HOST `thread.stop`); the control shows only when it can.
            Intent::Stop if self.threads.opened().is_some() => {
                if let Some(link) = self.thread_link.clone() {
                    self.threads.stop(link);
                }
            }
            Intent::StopThreadCoder { host, task } => {
                let Some(computers) = computers else { return };
                let reason = "Stopped from a phone.";
                self.command_on(
                    &host,
                    &task,
                    CommandAction::Interrupt,
                    reason,
                    false,
                    computers,
                );
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
            Intent::OpenChanges | Intent::CloseChanges => {
                if let Some(open) = self.open.as_mut() {
                    open.changes_open = intent == Intent::OpenChanges;
                }
            }
            Intent::RefreshChanges => {
                if let Some(open) = self.open.as_mut() {
                    open.review.refresh();
                }
            }
            Intent::PublishChanges => {
                if let Some(open) = self.open.as_mut() {
                    open.review.publish();
                }
                // Publish at once rather than at the next flush.
                if let Some(computers) = computers {
                    self.keep_review(computers);
                }
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

    /// Back to the Gym menu, for a person who opted into the Gym. The
    /// intro's chat has no way back until its first result; before the
    /// opt-in there is no menu.
    fn hub(&mut self) {
        use crate::gym::FirstRun;
        if !self.gym.opted_in() {
            return;
        }
        match self.gym.first_run() {
            FirstRun::Choose | FirstRun::EndCard => return,
            FirstRun::Chat if self.gym.first_result().is_none() => return,
            FirstRun::Chat => {
                self.gym.set_first_run(FirstRun::Done);
                self.gym.on_menu = true;
            }
            FirstRun::Done => self.gym.on_menu = true,
        }
        self.keep(true);
        self.open = None;
        self.talk = None;
        self.threads.close();
        self.drawer = false;
        self.notice = None;
        self.go = Some(Go::Gym);
    }

    /// A new chat with OpenAgents, starting with `text`.
    fn start_talk(&mut self, text: &str, computers: Option<&Computers>) {
        self.keep(true);
        self.threads.close();
        self.open = None;
        self.drawer = false;
        self.talk = None;
        let context = self.router_context(computers);
        self.basic.set_context(context);
        if let Some(id) = self.basic.start(text, unix_now()) {
            self.await_reply(&id);
            self.composers += 1;
            self.notice = None;
            self.talk_turns = TALK_TURNS;
            self.talk = Some(id);
        }
    }

    /// The back button an open chat's header starts with: to the Gym menu,
    /// for a person who opted in. None before the opt-in, and none in the
    /// intro's chat before its first result.
    fn back_button(&self) -> Option<Node<Intent>> {
        use crate::gym::FirstRun;
        if !self.gym.opted_in() {
            return None;
        }
        match self.gym.first_run() {
            FirstRun::Choose | FirstRun::EndCard => return None,
            FirstRun::Chat if self.gym.first_result().is_none() => return None,
            FirstRun::Chat | FirstRun::Done => {}
        }
        Some(icon_button(
            "coder-back",
            "Menu",
            Glyph::Back,
            true,
            Intent::Hub,
        ))
    }

    /// **Profile**, from Account: `SCR-11` as a sheet on the Chat tab, so
    /// the host switches to it.
    pub fn show_profile(&mut self) {
        if self.gym.hidden() {
            return;
        }
        self.gym.sheet = Some(crate::gym::Sheet::Profile);
        self.notice = None;
        self.go = Some(Go::Chat);
    }

    /// **Train Coder**, from the Verse's Gym board or Account: opt into the
    /// Gym and show its intro on the Chat tab.
    pub fn train_coder(&mut self) {
        if self.gym.hidden() {
            return;
        }
        self.gym.opt_in();
        self.gym.sheet = None;
        self.keep(true);
        self.open = None;
        self.talk = None;
        self.threads.close();
        self.drawer = false;
        self.notice = None;
        self.go = Some(Go::Chat);
    }

    /// A ready computer's name, where a run may go.
    fn ready_computer<'a>(&self, computers: Option<&'a Computers>) -> Option<&'a HostRecord> {
        match self.availability(computers) {
            Availability::Ready(host) => Some(host),
            _ => None,
        }
    }

    /// A tap on a Gym button: a card's, a sheet's, the menu's, or the first
    /// run's. Only an ID the last view minted does anything.
    pub fn gym_tap(&mut self, id: &str, mut computers: Option<&mut Computers>, chats: &mut Chats) {
        use crate::gym::{Effect, FirstRun};
        let computer = self
            .ready_computer(computers.as_deref())
            .map(|host| host.label.clone());
        let basic = &mut self.basic;
        let Some(effect) = self.gym.tap(
            id,
            |talk| crate::gym::Gym::draft_of(basic.turns(talk)),
            self.talk.as_deref(),
            computer.as_deref(),
        ) else {
            return;
        };
        let first_chat = matches!(effect, Effect::Fresh { .. })
            && self.gym.first_run() == FirstRun::Chat
            && self.gym.first_talk().is_none();
        match effect {
            Effect::None => {}
            Effect::Say { talk, text } => {
                if self.talk.as_deref() != Some(talk.as_str()) {
                    self.open = None;
                    self.drawer = false;
                    self.talk = Some(talk.clone());
                }
                let context = self.router_context(computers.as_deref());
                self.basic.set_context(context);
                if self.basic.send(&talk, &text, unix_now()) {
                    self.composers += 1;
                    self.notice = None;
                }
            }
            Effect::Fresh { text } => {
                self.gym.on_menu = false;
                self.start_talk(&text, computers.as_deref());
                if first_chat && let Some(talk) = self.talk.clone() {
                    self.gym.set_first_talk(&talk);
                }
            }
            Effect::Compose { text } => {
                self.compose = Some(text);
                self.composers += 1;
            }
            Effect::Computer { run, prompt } => {
                let started = match computers.as_deref_mut() {
                    Some(computers) => self.start_run_task(&prompt, computers),
                    None => Err("Connect a computer to run this.".into()),
                };
                self.gym.on_computer(&run, started);
            }
            Effect::Command {
                host,
                task,
                text,
                stop,
            } => {
                if let Some(computers) = computers.as_deref_mut() {
                    self.task_command(&host, &task, &text, stop, computers);
                }
            }
            Effect::OpenCoder { host, task } => {
                self.gym.sheet = None;
                self.drawer = false;
                self.talk = None;
                self.open(host, task, chats);
            }
            Effect::ConnectComputer => self.go = Some(Go::Connect),
            Effect::OpenChat { talk } => {
                self.gym.on_menu = false;
                self.open = None;
                self.drawer = false;
                self.talk = talk;
                self.composers += 1;
            }
            Effect::Menu => self.hub(),
            Effect::VerseGym => self.go = Some(Go::VerseGym),
        }
        if self.go.is_none() {
            self.go = Some(Go::Gym);
        }
        let _ = computers;
    }

    /// A Coder task on the ready computer that runs a test set, for a
    /// computer run: its host, the computer's name, and the task.
    fn start_run_task(
        &mut self,
        prompt: &str,
        computers: &mut Computers,
    ) -> Result<(String, String, String), String> {
        let host = match self.availability(Some(computers)) {
            Availability::Ready(host) => host.key.clone(),
            Availability::Connecting(host) => {
                return Err(format!(
                    "{} is still connecting. Try again in a moment.",
                    host.label
                ));
            }
            Availability::Offline(host) => {
                return Err(format!(
                    "{} is offline. Try again when it's back online.",
                    host.label
                ));
            }
            Availability::NotConfigured => return Err("Connect a computer to run this.".into()),
        };
        if computers
            .snapshot()
            .host(&host)
            .and_then(|record| self.workspace(record))
            .is_none()
        {
            computers
                .refresh_workspaces(&host)
                .map_err(|refusal| refusal.reason())?;
        }
        let record = computers
            .snapshot()
            .host(&host)
            .ok_or("That computer isn't on this phone anymore.")?;
        let label = record.label.clone();
        let workspace = self
            .workspace(record)
            .ok_or_else(|| format!("{label} lists no workspace for Coder yet."))?;
        let task = computers
            .start_task(&host, &workspace, prompt)
            .map_err(|refusal| refusal.reason())?;
        let now = computers.snapshot().now;
        self.list
            .list
            .titles
            .insert(task.clone(), "Testing a plugin".into());
        self.list.list.sent.insert(task.clone(), now);
        self.list.save();
        Ok((host, label, task))
    }

    /// Send `text` to a run's Coder task: a follow-up in its finished chat,
    /// or, with `stop`, an interrupt.
    fn task_command(
        &mut self,
        host: &str,
        task: &str,
        text: &str,
        stop: bool,
        computers: &mut Computers,
    ) {
        let based_on =
            Self::summary(computers.snapshot(), host, task).map_or(1, |summary| summary.sequence);
        let now = computers.snapshot().now;
        let draft = Draft {
            task,
            action: if stop {
                CommandAction::Interrupt
            } else {
                CommandAction::Send
            },
            based_on,
            text,
            emulate: false,
        };
        if self.outbox.push(host, draft, now).is_none() {
            self.gym.notice =
                Some("Too many messages are waiting to send. Try again later.".into());
            return;
        }
        self.flush(Some(computers));
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
            review: crate::changes::Reviewer::new(),
            reviewed: None,
            changes_open: false,
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

    /// Accept a composer's message: a new chat with OpenAgents, a message in
    /// the open one, or a command for the open Coder chat, whose action the
    /// chat's state chose.
    pub fn submit(
        &mut self,
        token: &str,
        value: &str,
        computers: Option<&mut Computers>,
        _chats: &mut Chats,
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
        // Text only while attachments are off: a draft that still holds
        // images sends its words alone.
        self.text_only();
        // A Coder chat whose turn has ended, started from a conversation
        // here: the follow-up goes to that conversation, whose router
        // answers it from the run's result or hands it back to Coder
        // (#10094).
        if self.talk.is_none()
            && self.threads.opened().is_none()
            && let Some(open) = &self.open
            && open.editing.is_none()
            && Self::ended(computers.as_deref(), &open.host, &open.task)
            && let Some(id) = self.talk_of(&open.host, &open.task)
        {
            let phase = computers
                .as_deref()
                .and_then(|c| Self::summary(c.snapshot(), &open.host, &open.task))
                .map(|summary| summary.phase);
            if let Some(run) = phase.and_then(|phase| open_result(open, phase)) {
                self.talk_runs.insert(id.clone(), run);
            }
            self.keep(true);
            self.open = None;
            self.talk_turns = TALK_TURNS;
            self.talk = Some(id);
        }
        // A computer's own thread and a Coder task's chat carry words
        // only: keep the images and the words rather than drop the images
        // silently. A message to OpenAgents sends its words and binds the
        // draft's images to it, for the Coder start its reply may lead to.
        let key = self.draft_key();
        let to_router =
            self.threads.opened().is_none() && (self.talk.is_some() || self.open.is_none());
        if !to_router && let Some(reason) = self.images.text_only_refusal(&key) {
            self.notice = Some(reason.into());
            return;
        }
        // A computer's own thread: the computer appends it and answers.
        if self.threads.opened().is_some() {
            if self.threads.send(prompt) {
                self.composers += 1;
                self.notice = None;
            }
            return;
        }
        // A basic conversation, open or new, needs no computer.
        let context = self.router_context(computers.as_deref());
        self.basic.set_context(context);
        let request = uuid::Uuid::new_v4().simple().to_string();
        if let Some(id) = self.talk.clone() {
            if self
                .basic
                .send_tagged(&id, prompt, unix_now(), Some(request.clone()))
            {
                self.await_reply(&id);
                self.composers += 1;
                self.notice = self
                    .images
                    .bind(&key, &request)
                    .then(|| crate::attachments::HELD_FOR_CODER.into());
                self.compose = None;
                self.gym.notice = None;
            }
            return;
        }
        // A new chat always goes to OpenAgents.
        let Some(open) = &self.open else {
            if let Some(id) = self
                .basic
                .start_tagged(prompt, unix_now(), Some(request.clone()))
            {
                self.await_reply(&id);
                self.composers += 1;
                let talk = format!("talk:{id}");
                self.images.rebind(&key, &talk);
                self.notice = self
                    .images
                    .bind(&talk, &request)
                    .then(|| crate::attachments::HELD_FOR_CODER.into());
                self.talk_turns = TALK_TURNS;
                self.talk = Some(id);
            }
            return;
        };
        let Some(computers) = computers else { return };
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
    }

    /// The replies to messages sent with images: a reply that leads to
    /// Coder leaves them in the draft for **Run Coder**, which carries them;
    /// any other keeps them there and says images go only to Coder
    /// ([`crate::attachments::Drafts::settle`]).
    fn settle_images(&mut self) {
        for key in self.images.bound_chats() {
            let Some(id) = key.strip_prefix("talk:") else {
                continue;
            };
            let busy = self.basic.busy(id);
            let lane = self.basic.lane(id) == Some(crate::basic_coder::Lane::Computer);
            let settled = self.images.settle(&key, self.basic.turns(id), busy, lane);
            if settled == Some(crate::attachments::Settled::Kept)
                && self.talk.as_deref() == Some(id)
            {
                self.notice = Some(crate::attachments::ONLY_TO_CODER.into());
            }
        }
    }

    /// Run Coder on the chosen computer with the open conversation: a task
    /// there that starts from the conversation so far. Without a computer
    /// the person is sent to connect one.
    fn run_coder(&mut self, computers: &mut Computers, chats: &mut Chats) {
        let Some(id) = self.talk.clone() else { return };
        self.run_coder_in(&id, true, computers, chats);
    }

    /// Run Coder for the conversation `id`, as **Run Coder** does; `open`
    /// opens the Coder chat it starts, as a tap does. Returns whether a
    /// task started or took the message.
    fn run_coder_in(
        &mut self,
        id: &str,
        open: bool,
        computers: &mut Computers,
        chats: &mut Chats,
    ) -> bool {
        let id = id.to_owned();
        // Text only while attachments are off: the task carries no images.
        self.text_only();
        let reply = self.basic.turns(&id).len().checked_sub(1);
        // The conversation's Coder task ended its turn, and the router
        // judged the latest message more work for it: the same task takes
        // it as its next turn, in the same worktree (#10094).
        if let Some(spawned) = self
            .basic
            .get(&id)
            .and_then(|summary| summary.coder.clone())
            && Self::ended(Some(computers), &spawned.host, &spawned.task)
        {
            let Some(text) = self
                .basic
                .turns(&id)
                .iter()
                .rev()
                .find(|turn| turn.role == crate::basic_coder::Role::User)
                .map(|turn| turn.text.clone())
            else {
                return false;
            };
            if self.command_on(
                &spawned.host,
                &spawned.task,
                CommandAction::Send,
                &text,
                false,
                computers,
            ) {
                self.talk_runs.remove(&id);
                if let Some(reply) = reply {
                    let now = computers.snapshot().now;
                    self.mark_started(&id, reply, &spawned.task, now);
                }
                if open {
                    self.talk = None;
                    self.open(spawned.host, spawned.task, chats);
                }
                return true;
            }
            return false;
        }
        let host = match self.availability(Some(computers)) {
            Availability::Ready(host) => host.key.clone(),
            Availability::Connecting(host) => {
                self.notice = Some(format!("Connecting to {}…", host.label));
                return false;
            }
            Availability::Offline(host) => {
                self.notice = Some(format!("{} is offline.", host.label));
                return false;
            }
            Availability::NotConfigured => {
                self.go = Some(Go::Connect);
                return false;
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
            return false;
        }
        let Some(record) = computers.snapshot().host(&host) else {
            return false;
        };
        let label = record.label.clone();
        let Some(workspace) = self.workspace(record) else {
            self.notice = Some(format!("{label} lists no workspace for Coder yet."));
            return false;
        };
        let chat_title = self
            .basic
            .get(&id)
            .map_or_else(|| "Chat".to_owned(), |summary| summary.title.clone());
        // Titled by the message that asked for the work (#10073).
        let title = openagents_chat::delegation::title(&chat_title, self.basic.turns(&id));
        let prompt = openagents_chat::delegation::prompt(&chat_title, self.basic.turns(&id));
        // The engine the reply's typed `run_coder` offer named (#10081),
        // never read from text: the computer puts it first among what its
        // owner's policy allows, and says why when it can't.
        let engine = openagents_chat::delegation::requested(self.basic.turns(&id));
        // The draft's images go to this computer with the task; the hosted
        // conversation never carries them. A refusal keeps the draft.
        let key = format!("talk:{id}");
        let uploads = match self.images.uploads(&key) {
            Ok(uploads) => uploads,
            Err(reason) => {
                self.notice = Some(reason);
                return false;
            }
        };
        match computers.start_task_requesting(&host, &workspace, &prompt, &uploads, engine) {
            Ok(task) => {
                self.images.clear(&key);
                let now = computers.snapshot().now;
                self.list.list.titles.insert(task.clone(), title);
                self.list.list.sent.insert(task.clone(), now);
                self.list.list.used.insert(used_key(&host, &workspace), now);
                self.list.save();
                self.basic
                    .spawned_in(&id, &host, &task, Some(&workspace), now);
                if let Some(reply) = reply {
                    self.mark_started(&id, reply, &task, now);
                }
                self.notice = None;
                if open {
                    self.talk = None;
                    self.open(host, task.clone(), chats);
                }
                self.echo(&task, &prompt, None, false, now);
                true
            }
            Err(refusal) => {
                self.notice = Some(refusal.reason());
                false
            }
        }
    }

    /// Wait for the reply to the message just sent in `id`: a coding reply
    /// to it may start Coder at once ([`CoderTab::start_offered`]). A newer
    /// message in the same conversation replaces the one it waited for.
    fn await_reply(&mut self, id: &str) {
        let turns = self.basic.turns(id);
        let Some(at) = turns.len().checked_sub(1) else {
            return;
        };
        if turns[at].role != crate::basic_coder::Role::User {
            return;
        }
        self.awaiting.retain(|(talk, _)| talk != id);
        if self.awaiting.len() >= MAX_AWAITING {
            self.awaiting.remove(0);
        }
        self.awaiting.push((id.to_owned(), at));
    }

    /// Start Coder at once for each coding reply that just arrived to a
    /// message this device sent (#10101), as the desktop does under
    /// `coder.start: at_once`: the reply's typed `run_coder` offer, by the
    /// shared precedence ([`openagents_chat::delegation::offered`]), on a
    /// paired computer that is ready, that this device may operate, and
    /// whose presence says its owner starts Coder at once
    /// ([`starts_at_once`]). The start is **Run Coder**'s own (the offer's
    /// engine, the message that asked), made once per reply and never for
    /// a reply that answered; the conversation stays where it is and shows
    /// the start ([`CoderTab::talk_view`]). A computer that asks first,
    /// predates the capability, or is offline or not paired keeps the
    /// reply's **Run Coder** or **Connect a computer**. A conversation
    /// whose Coder task ended its turn continues it with the message, as
    /// the desktop continues its run without asking again (#10094); one
    /// whose task still runs starts nothing more. The app calls this each
    /// tick, after [`CoderTab::flush`].
    pub fn start_offered(&mut self, computers: Option<&mut Computers>, chats: &mut Chats) {
        if self.awaiting.is_empty() {
            return;
        }
        // The reply that just finished, in the same tick that saw it end.
        self.basic.settle(unix_now());
        let mut due = Vec::new();
        let awaiting = std::mem::take(&mut self.awaiting);
        for (id, at) in awaiting {
            if self.basic.get(&id).is_none() {
                continue;
            }
            let busy = self.basic.busy(&id);
            let lane = self.basic.lane(&id) == Some(crate::basic_coder::Lane::Computer);
            let turns = self.basic.turns(&id);
            let Some(reply) = turns.get(at + 1).filter(|_| !busy) else {
                // Still streaming, or failed and waiting for Try again.
                if turns.len() > at {
                    self.awaiting.push((id, at));
                }
                continue;
            };
            // Only the conversation's newest reply, finished, that offers
            // Coder by the shared rule; anything else answered.
            if at + 2 == turns.len()
                && reply.role == crate::basic_coder::Role::Assistant
                && !reply.stopped
                && openagents_chat::delegation::offered(reply.meta.as_ref(), lane)
            {
                due.push((id, at + 1));
            }
        }
        let Some(computers) = computers else { return };
        for (id, reply) in due {
            if self.started_reply(&id) == Some(reply) {
                continue;
            }
            let spawned = self
                .basic
                .get(&id)
                .and_then(|summary| summary.coder.clone());
            let go = match &spawned {
                // The conversation's task ended its turn: this message is
                // its next one, on the computer the person started it on.
                Some(spawned) if Self::ended(Some(computers), &spawned.host, &spawned.task) => {
                    authority::check(
                        computers.snapshot(),
                        Capabilities {
                            platform: Platform::Phone,
                            camera: false,
                        },
                        Action::Operate {
                            host: &spawned.host,
                        },
                    )
                    .is_ok()
                }
                // Its task still runs: nothing more starts.
                Some(_) => false,
                None => match self.availability(Some(computers)) {
                    Availability::Ready(host) => starts_at_once(host),
                    _ => false,
                },
            };
            if go {
                // A refusal shows only in the conversation it is about.
                let shown = self.talk.as_deref() == Some(id.as_str()) && self.open.is_none();
                let notice = self.notice.clone();
                self.run_coder_in(&id, false, computers, chats);
                if !shown {
                    self.notice = notice;
                }
            }
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
        self.read_outcome(computers, chats);
        self.basic.settle(unix_now());
        self.settle_images();
        self.poll_threads(computers);
        self.poll_cli();
        self.gym.begin();
        let phase = |host: &str, task: &str| {
            computers
                .and_then(|c| Self::summary(c.snapshot(), host, task))
                .map(|summary| summary.phase)
        };
        self.gym.settle(&phase);
        self.gym.level_up();
        // FLOW-01: the intro's chat reopens where it was, once per launch,
        // and only for a person who opted into the Gym. A phone upgraded
        // from a build whose guided first run stopped at the chat keeps
        // that state with no opt-in, and must open on a fresh chat; and
        // once the person leaves the intro's chat (New chat, Back), it
        // stays left.
        if !self.intro_reopened
            && self.gym.opted_in()
            && self.gym.first_run() == crate::gym::FirstRun::Chat
            && self.open.is_none()
            && self.talk.is_none()
            && self.threads.opened().is_none()
            && !self.drawer
            && let Some(first) = self.gym.first_talk().map(str::to_owned)
            && self.basic.get(&first).is_some()
        {
            self.intro_reopened = true;
            self.talk = Some(first);
        }
        self.play(computers);
        self.follow(computers, chats);
        self.settle_echoes(computers.map_or_else(unix_now, |c| c.snapshot().now));
        let ended = self
            .open
            .as_ref()
            .is_some_and(|open| open.seen.is_some() && open.settled == open.seen);
        self.keep(ended);
        if self.open.is_none()
            && self.talk.is_none()
            && self.threads.opened().is_none()
            && !self.drawer
        {
            let candidates = self
                .candidates()
                .into_iter()
                .filter_map(|(id, chip)| match chip.element {
                    Element::Button { label, .. } => Some((id, label)),
                    _ => None,
                })
                .collect();
            self.basic.want_rank(candidates);
        }
        self.revision += 1;
        self.links.begin_view();
        let view = loop {
            let mut root = match (&self.open, &self.talk) {
                _ if self.drawer => self.previous(computers, chats),
                _ if self.threads.opened().is_some() => self.thread_view(computers),
                (Some(open), _) => {
                    let linked = chats.coder_client(&open.host).is_some();
                    self.chat(open, computers, linked)
                }
                (None, Some(id)) => {
                    let id = id.clone();
                    self.talk_view(&id, computers)
                }
                (None, None) => self.landing(),
            };
            if self.shell.on && !self.drawer {
                shell::strip_header(&mut root);
            }
            let detached = !self.pulled || source::detach(&mut root, &self.instance).is_ok();
            match View::new(self.instance.clone(), self.revision, root)
                .validate()
                .ok()
                .filter(|_| detached)
            {
                Some(view) => break view,
                // A long chat can outgrow one view: show less of each tool's
                // output, then keep its newest half.
                None if self.drawer || self.threads.opened().is_some() => return None,
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

    /// The Gym's part of the app packet: which of the chat tab's screens
    /// shows, the cards the last render drew, the sheet, the menu, and the
    /// first run. Call it after [`CoderTab::render`], whose cards it
    /// carries.
    pub fn gym_view(&mut self) -> crate::gym::View {
        let draft = self
            .gym
            .sheet_talk()
            .map(str::to_owned)
            .and_then(|talk| crate::gym::Gym::draft_of(self.basic.turns(&talk)));
        let sheet = self.gym.sheet_view(draft);
        let first_run = crate::first_run::first_run(&mut self.gym);
        let menu = crate::first_run::menu(&mut self.gym, self.app_build.as_deref());
        crate::gym::View {
            screen: crate::first_run::screen(&self.gym),
            first_run,
            menu,
            cards: self.gym.cards().clone(),
            sheet,
            share: self.gym.take_share(),
            live: self.gym.active().is_some(),
        }
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
            availability(computers.snapshot(), self.preferred.as_deref())
        })
    }

    /// Every chat, newest message first: basic conversations and the
    /// computers' Coder chats. A chat with no known message time goes last.
    fn recent(&self, computers: Option<&Computers>, chats: &Chats) -> Vec<Recent> {
        let availability = self.availability(computers);
        let now = computers.map_or_else(unix_now, |c| c.snapshot().now);
        let mut rows: Vec<Recent> = crate::chat_list::search(self.basic.list(), "")
            .into_iter()
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
                    group: crate::chat_list::group(summary),
                    last: Some(summary.updated),
                    updated: summary.updated,
                    row: chat_card(
                        summary,
                        button(
                            &format!("talk-{}", &summary.id[..16.min(summary.id.len())]),
                            &format!(
                                "{}\n{place} · {} · {}",
                                summary.title,
                                ago(now, summary.updated),
                                match crate::chat_list::group(summary) {
                                    crate::chat_list::Group::Pinned => "Pinned".into(),
                                    crate::chat_list::Group::Archived => "Archived".into(),
                                    crate::chat_list::Group::Project(p) => p,
                                    crate::chat_list::Group::Recent => "Saved".into(),
                                }
                            ),
                            Intent::OpenTalk {
                                id: summary.id.clone(),
                            },
                        ),
                    ),
                }
            })
            .collect();
        // Each computer's own threads, started in its desktop app or with
        // `openagents chat` there, labelled with the computer.
        rows.extend(self.threads.rows().into_iter().map(|(host, listed, row)| {
            // Rows kept from an earlier read say how old they are until the
            // computer answers again.
            let kept = self
                .threads
                .kept_at(&host)
                .map_or_else(String::new, |at| format!(" · Last read {}", ago(now, at)));
            let label = computers
                .and_then(|c| c.snapshot().host(&host))
                .map_or(listed, |record| record.label.clone());
            let group = if row.pinned {
                crate::chat_list::Group::Pinned
            } else {
                crate::chat_list::Group::Recent
            };
            let title = if row.title.is_empty() {
                "New chat"
            } else {
                row.title.as_str()
            };
            Recent {
                group,
                last: Some(row.updated),
                updated: row.updated,
                row: button(
                    &format!(
                        "thread-{}-{}",
                        &host[..8.min(host.len())],
                        &row.thread[..16.min(row.thread.len())]
                    ),
                    &format!("{title}\n{label} · {}{kept}", ago(now, row.updated)),
                    Intent::OpenThread {
                        host: host.clone(),
                        thread: row.thread.clone(),
                    },
                ),
            }
        }));
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
                .map(|(last, updated, _, row)| Recent {
                    group: crate::chat_list::Group::Recent,
                    last,
                    updated,
                    row,
                }),
            );
        }
        order_recent(&mut rows);
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

    /// A new chat, where the tab opens: the header with the previous chats
    /// behind the menu button, and a composer ready to type. Every new chat
    /// goes to OpenAgents; nothing on screen picks where. Above the field
    /// sit only suggested questions (see [`CoderTab::candidates`]); previous
    /// chats stay behind the menu, and Coder on a computer comes only from
    /// an offer under a reply.
    fn landing(&self) -> Node<Intent> {
        if self.shell.on {
            return self.shell_landing();
        }
        let mut top = vec![];
        top.extend(self.back_button());
        top.extend([
            icon_button(
                "coder-menu",
                "Previous chats",
                Glyph::Menu,
                true,
                Intent::Menu,
            ),
            heading("coder-title", "OpenAgents"),
        ]);
        let mut children = vec![header("coder-header", top)];
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
        let chips = self.suggestions();
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
        children.extend(self.attachments());
        // The tab exists to write a message: it opens ready to type.
        children.push(self.composer_with(
            self.ask_words().to_owned(),
            true,
            false,
            &[],
            None,
            true,
        ));
        page(children)
    }

    /// Suggested questions above the composer. The worker's ranking orders
    /// them when it answered for this set.
    fn suggestions(&self) -> Vec<Node<Intent>> {
        let mut chips = self.candidates();
        self.basic.rank_order(&mut chips);
        chips.into_iter().map(|(_, chip)| chip).collect()
    }

    /// The suggestions in the phone's order, each with the ID a rank job
    /// knows it by: the first few of [`crate::first_run::SUGGESTIONS`] not
    /// used on this device (tapped, or their words sent), then used ones,
    /// so every new chat shows four. Each is a question to send, never a
    /// previous chat (those are behind the menu) and never a way to run
    /// Coder on a computer (that is an offer under a reply).
    fn candidates(&self) -> Vec<(String, Node<Intent>)> {
        let used = self.basic.used_markers();
        // With the Gym hidden, its questions are left out, not just cut.
        let suggestions: Vec<&crate::first_run::Suggestion> = if self.gym.hidden() {
            crate::cards::released_suggestions(used).collect()
        } else {
            crate::cards::suggestions(used).collect()
        };
        suggestions
            .into_iter()
            .map(|suggestion| {
                (
                    suggestion.id.to_owned(),
                    pill(
                        &format!("coder-suggest-{}", suggestion.id),
                        suggestion.label,
                        Glyph::Ask,
                        Intent::Starter {
                            id: suggestion.id.to_owned(),
                        },
                    ),
                )
            })
            .collect()
    }

    /// An open basic conversation: its turns, the reply as it streams, and
    /// a way to run Coder on a computer with it.
    /// The start of the conversation's Coder task under the reply that
    /// started it: where it runs, the computer's note (such as which engine
    /// runs when the asked-for one can't), and **Open Coder**, with
    /// **Stop** while it runs. Shown while the newest reply (`newest`, its
    /// turn index) is the one that started or continued the task here, or
    /// while the computer says the task runs.
    fn start_card(
        &self,
        id: &str,
        newest: Option<usize>,
        computers: Option<&Computers>,
    ) -> Option<Node<Intent>> {
        let spawned = self.basic.get(id)?.coder.clone()?;
        let summary =
            computers.and_then(|c| Self::summary(c.snapshot(), &spawned.host, &spawned.task));
        let phase = summary.map(|summary| summary.phase);
        let running = phase.is_none_or(Self::running);
        let started = self
            .list
            .list
            .started
            .get(id)
            .filter(|started| started.task == spawned.task);
        let started_here = newest.is_some() && started.map(|s| s.reply) == newest;
        if !started_here && !(running && phase.is_some()) {
            return None;
        }
        let label = computers
            .and_then(|c| c.snapshot().host(&spawned.host))
            .map_or_else(|| "your computer".to_owned(), |host| host.label.clone());
        let title = if running {
            format!("Coder started on {label}")
        } else {
            format!("Coder on {label}: {}", phase.map_or("Unknown", phase_label))
        };
        let headline = summary
            .filter(|s| {
                s.headline != nostr::activity_summary::generic_headline(SubjectKind::Task, s.phase)
            })
            .map(|s| s.headline.clone());
        let note = if running {
            let note = headline.unwrap_or_else(|| phase.map_or("Starting", phase_label).to_owned());
            // A run can take many minutes, such as an iOS archive.
            let now = computers.map_or_else(unix_now, |c| c.snapshot().now);
            Some(
                match started
                    .map(|s| s.at)
                    .or(spawned.at)
                    .and_then(|at| worked(now.saturating_sub(at)))
                {
                    Some(worked) => format!("{note} · {worked}"),
                    None => note,
                },
            )
        } else {
            // The title says how it ended; the note says what it did, or
            // nothing rather than the title's word again.
            started.and_then(|s| s.outcome.clone()).or(headline)
        };
        let mut controls = vec![button(
            "coder-start-open",
            "Open Coder",
            Intent::Open {
                host: spawned.host.clone(),
                task: spawned.task.clone(),
            },
        )];
        // A task in an unknown state can be stopped too: the computer ends
        // it when its process is gone (#10124).
        if running || phase == Some(Phase::Unknown) {
            controls.push(button(
                "coder-start-stop",
                "Stop",
                Intent::StopThreadCoder {
                    host: spawned.host,
                    task: spawned.task,
                },
            ));
        }
        Some(node(
            "coder-start",
            Element::Stack {
                axis: Axis::Vertical,
                children: [text(
                    "coder-start-title",
                    &title,
                    TextRole::Body,
                    white(),
                    true,
                )]
                .into_iter()
                .chain(note.map(|note| status("coder-start-note", &note)))
                .chain([row("coder-start-controls", controls)])
                .collect(),
            },
        ))
    }

    fn talk_view(&mut self, id: &str, computers: Option<&Computers>) -> Node<Intent> {
        let availability = self.availability(computers);
        let busy = self.basic.busy(id);
        let tail = self.basic.tail(id);
        let limit = self.talk_turns;
        let mut header = chat_header(status("coder-chat-place", "OpenAgents"));
        if let Element::Stack { children, .. } = &mut header.element
            && let Some(back) = self.back_button()
        {
            children.insert(0, back);
        }
        let working_label = if self.basic.still_working(id) {
            crate::basic_coder::STILL_WORKING
        } else {
            "Working…"
        };
        let partial = self.basic.partial(id);
        let turns = self.basic.turns(id);
        let mut children = vec![header];
        if let Some(notice) = &self.notice {
            children.push(status("coder-notice", notice));
        }
        let skipped = turns.len().saturating_sub(limit);
        let failure = match &tail {
            Tail::Failed(why) => Some(why.as_str()),
            _ => None,
        };
        let mut rows: Vec<Node<Intent>> = self.projection.rows(
            &turns[skipped..],
            skipped,
            crate::projection::Reply {
                busy,
                partial: &partial,
                failure,
            },
            &crate::projection::Appearance {
                prefix: "talk-m",
                body_suffix: "-md",
                streaming_key: format!("talk-m{}", turns.len()),
                working_key: "talk-working",
                working_label,
                failed_key: "talk-failed",
                status_style: Style {
                    foreground: Some(gray()),
                    ..Style::default()
                },
                markdown_style: Style {
                    foreground: Some(white()),
                    ..Style::default()
                },
            },
        );
        let failed = rows.last().is_some_and(|row| row.key == "talk-failed");
        if self.shell.on {
            shell::worked(&mut rows, self.basic.turns(id), skipped);
            shell::link_cards(&mut rows, self.basic.turns(id), &self.links);
        }
        // What a proposed command printed scrolls with the conversation,
        // so a long result never pushes the composer off the screen.
        if let Some((talk, ran, CliOutcome::Output(lines))) = &self.cli
            && talk == id
        {
            rows.push(cli_output_row(ran, lines));
        }
        children.push(node(
            "coder-transcript",
            Element::Transcript {
                label: "Messages".into(),
                children: rows,
                earlier: None,
                source: None,
            },
        ));
        if failed || self.basic.turns(id).last().is_some_and(|turn| turn.stopped) {
            children.push(row(
                "talk-retry-row",
                vec![button("talk-retry", "Try again", Intent::Retry)],
            ));
        }
        // The Gym's cards under the newest reply: each a surface the host
        // draws from the packet's `gym.cards`, by ID.
        let here = crate::gym::Here { busy };
        let kept = self.basic.turns(id).to_vec();
        for card in self.gym.cards_for(id, &kept, &here) {
            children.push(node(
                &format!("gym-card-{card}"),
                Element::Surface {
                    resource: format!("gym-card:{card}"),
                    label: "Card".into(),
                },
            ));
        }
        if let Some(notice) = self.gym.notice.clone() {
            children.push(status("gym-notice", &notice));
        }
        // What the router said about the last reply: its offers and
        // follow-ups. Nothing here acts until a tap, and each tap's meaning
        // is the phone's own. None of it shows while the reply streams, and
        // nothing about Coder on a computer shows unless the reply offered
        // it: no standing Run Coder or Open Coder buttons above the field.
        let meta = crate::projection::actionable(self.basic.turns(id), busy, failed).cloned();
        let offers = meta
            .as_ref()
            .map(|meta| meta.offers.clone())
            .unwrap_or_default();
        let computer_lane = self.basic.lane(id) == Some(crate::basic_coder::Lane::Computer)
            && crate::projection::completed(self.basic.turns(id), busy, failed);
        let mut actions = crate::cards::reply_actions(
            meta.as_ref(),
            self.basic.used_markers(),
            computer_lane,
            &availability,
            self.gym.latest_result().is_some(),
        );
        if self.gym.hidden() {
            crate::cards::drop_preview_chips(&mut actions.chips, meta.as_ref());
        }
        // Under the shell, a reply offering the Verse shows **Enter the
        // Grid** as a card in the conversation rather than a chip.
        if self.shell.on {
            let portal = |chip: &crate::cards::Chip| {
                matches!(
                    chip.action,
                    crate::cards::Action::OpenScreen {
                        screen: Screen::Verse
                    }
                )
            };
            if actions.chips.iter().any(portal) {
                actions.chips.retain(|chip| !portal(chip));
                children.push(shell::verse_portal());
            }
        }
        // The reply that started Coder, at once or from a tap, shows the
        // start with Stop instead of offering it again (#10101).
        let newest = self.basic.turns(id).len().checked_sub(1);
        let start = self.start_card(id, newest, computers);
        if start.is_some() {
            actions.chips.retain(|chip| {
                !matches!(
                    chip.action,
                    crate::cards::Action::RunCoder | crate::cards::Action::ConnectComputer
                )
            });
            actions.notice = None;
        }
        let mut agents = vec![];
        if let Some((key, value)) = &actions.notice {
            agents.push(status(key, value));
        }
        for chip in &actions.chips {
            if !matches!(chip.action, crate::cards::Action::Followup { .. })
                && let Some(intent) = card_intent(chip.action.clone())
            {
                agents.push(pill(&chip.key, &chip.label, chip.glyph, intent));
            }
        }
        if !agents.is_empty() {
            children.push(wrap("coder-agents", agents));
        }
        children.extend(start);
        // Proposed read-only commands, each a card with the exact command
        // and a Run button.
        for (index, offer) in offers.iter().enumerate() {
            if let Offer::Cli { argv, runs_on } = offer {
                children.push(self.cli_card(id, index, argv, *runs_on, &availability));
            }
        }
        let chips: Vec<_> = actions
            .chips
            .iter()
            .filter_map(|chip| {
                if matches!(chip.action, crate::cards::Action::Followup { .. }) {
                    card_intent(chip.action.clone())
                        .map(|intent| pill(&chip.key, &chip.label, chip.glyph, intent))
                } else {
                    None
                }
            })
            .collect();
        if !chips.is_empty() {
            children.push(wrap("coder-followups", chips));
        }
        let compose = self.compose.clone();
        let focus = compose.is_some();
        children.extend(self.attachments());
        children.push(self.composer_with(
            self.ask_words().to_owned(),
            true,
            busy,
            &[],
            compose,
            focus,
        ));
        page(children)
    }

    /// An open thread of a computer's: its turns, the reply as it streams,
    /// the Coder work it started, and a composer that sends through the
    /// computer when this device may operate it.
    fn thread_view(&mut self, computers: Option<&Computers>) -> Node<Intent> {
        let Some(shown) = self.threads.shown() else {
            return self.landing();
        };
        let label = computers
            .and_then(|c| c.snapshot().host(&shown.host))
            .map(|host| host.label.clone())
            .or_else(|| self.threads.label(&shown.host))
            .unwrap_or_else(|| "A computer".to_owned());
        let mut children = vec![chat_header(status("coder-chat-place", &label))];
        if let Some(notice) = &self.notice {
            children.push(status("coder-notice", notice));
        }
        if let Some(error) = &shown.error {
            children.push(status("thread-error", error));
        }
        // The copy this phone kept, until the computer answers a read.
        if let Some(at) = shown.kept_at {
            let now = computers.map_or_else(unix_now, |c| c.snapshot().now);
            children.push(status(
                "thread-kept",
                &format!("Saved on this phone · last read {}", ago(now, at)),
            ));
        }
        // A follow-up the computer has not taken yet, while it is not
        // answering, waits rather than works.
        let waiting = format!("Waiting for {label}…");
        let working_label = if shown.queued && (shown.error.is_some() || shown.kept_at.is_some()) {
            waiting.as_str()
        } else {
            "Working…"
        };
        let rows: Vec<Node<Intent>> = if shown.loading {
            vec![node(
                "thread-loading",
                Element::Working {
                    label: "Loading the chat…".into(),
                },
            )]
        } else {
            self.thread_projection.rows(
                &shown.turns,
                usize::try_from(shown.start).unwrap_or(0),
                crate::projection::Reply {
                    busy: shown.busy,
                    partial: &shown.partial,
                    failure: shown.failure.as_deref(),
                },
                &crate::projection::Appearance {
                    prefix: "thread-m",
                    body_suffix: "-md",
                    streaming_key: format!("thread-m{}", shown.start as usize + shown.turns.len()),
                    working_key: "thread-working",
                    working_label,
                    failed_key: "thread-failed",
                    status_style: Style {
                        foreground: Some(gray()),
                        ..Style::default()
                    },
                    markdown_style: Style {
                        foreground: Some(white()),
                        ..Style::default()
                    },
                },
            )
        };
        children.push(node(
            "coder-transcript",
            Element::Transcript {
                label: "Messages".into(),
                children: rows,
                earlier: (shown.start > 0).then(|| rust_native::view::Earlier {
                    label: "Load earlier".into(),
                    loading: shown.loading_earlier,
                    intent: Intent::Earlier,
                }),
                source: None,
            },
        ));
        // The same cards and chips a thread on this phone shows. Run Coder
        // stays visible when this phone may only read; the tap says so.
        // A thread that already started Coder, or a computer that cannot
        // start it, drops that one chip. Proposed commands stay on this phone.
        let here = crate::gym::Here { busy: shown.busy };
        for card in self.gym.cards_for(&shown.thread, &shown.turns, &here) {
            children.push(node(
                &format!("gym-card-{card}"),
                Element::Surface {
                    resource: format!("gym-card:{card}"),
                    label: "Card".into(),
                },
            ));
        }
        let failed = shown.failure.is_some();
        let mut meta = crate::projection::actionable(&shown.turns, shown.busy, failed).cloned();
        // A thread whose Coder task ended keeps the offer: it continues
        // that task (#10094).
        let continues = shown
            .coder
            .as_ref()
            .is_some_and(|coder| Self::ended(computers, &coder.host, &coder.task));
        if ((shown.coder.is_some() && !continues) || !shown.runnable)
            && let Some(meta) = meta.as_mut()
        {
            meta.offers
                .retain(|offer| !matches!(offer, Offer::RunCoder));
        }
        let mut actions = crate::cards::reply_actions_for(
            meta.as_ref(),
            &[],
            false,
            &crate::cards::Target::Ready(&label),
            self.gym.latest_result().is_some(),
        );
        if self.gym.hidden() {
            crate::cards::drop_preview_chips(&mut actions.chips, meta.as_ref());
        }
        let mut agents = vec![];
        if let Some((key, value)) = &actions.notice {
            agents.push(status(key, value));
        }
        for chip in &actions.chips {
            if !matches!(chip.action, crate::cards::Action::Followup { .. })
                && let Some(intent) = card_intent(chip.action.clone())
            {
                agents.push(pill(&chip.key, &chip.label, chip.glyph, intent));
            }
        }
        if !agents.is_empty() {
            children.push(wrap("coder-agents", agents));
        }
        let followups: Vec<_> = actions
            .chips
            .iter()
            .filter_map(|chip| {
                if matches!(chip.action, crate::cards::Action::Followup { .. }) {
                    card_intent(chip.action.clone())
                        .map(|intent| pill(&chip.key, &chip.label, chip.glyph, intent))
                } else {
                    None
                }
            })
            .collect();
        if !followups.is_empty() {
            children.push(wrap("coder-followups", followups));
        }
        // Coder work the thread started opens as that task's chat, read
        // through the computer's history observer like any Coder chat.
        if let Some(coder) = &shown.coder {
            // A stopped reply does not stop Coder: while the task runs, the
            // phone that stopped the reply offers to stop it too.
            let running = computers
                .and_then(|c| Self::summary(c.snapshot(), &coder.host, &coder.task))
                .is_some_and(|summary| Self::running(summary.phase));
            let mut pills = vec![];
            if shown.stopped_here && running && Self::operates(computers, &coder.host) {
                pills.push(pill(
                    "thread-coder-stop",
                    "Stop Coder too",
                    Glyph::Stop,
                    Intent::StopThreadCoder {
                        host: coder.host.clone(),
                        task: coder.task.clone(),
                    },
                ));
            }
            pills.push(pill(
                "thread-coder-open",
                &coder
                    .project
                    .as_ref()
                    .map_or_else(|| "Open Coder".to_owned(), |p| format!("Open Coder · {p}")),
                Glyph::Ask,
                Intent::Open {
                    host: coder.host.clone(),
                    task: coder.task.clone(),
                },
            ));
            children.push(wrap("thread-coder", pills));
        } else if let Some(outside) = &shown.outside {
            // A run `openagents chat` started on the computer, in a task
            // store its host does not serve: said plainly, with no control
            // that could not reach it.
            children.push(status(
                "thread-coder-outside",
                &outside_words(&label, outside.project.as_deref()),
            ));
        }
        let operates = Self::operates(computers, &shown.host);
        let (token, _) = self.tokens(&[]);
        children.push(node(
            "coder-composer",
            Element::Composer {
                token,
                placeholder: if operates {
                    format!("Message OpenAgents on {label}")
                } else {
                    "This phone can only read this chat".to_owned()
                },
                max_bytes: MAX_PROMPT_BYTES,
                enabled: operates,
                busy: shown.busy,
                // The computer stops the reply (`thread.stop`). A computer
                // that cannot, or a phone that may not operate it, gets no
                // stop control at all, never one that does nothing.
                stop: (shown.busy && shown.stoppable && operates).then_some(Intent::Stop),
                choices: vec![],
                draft: None,
                focus: false,
            },
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
                white(),
                false,
            ),
            status(&format!("{key}-where"), &place),
        ];
        match &self.cli {
            Some((talk, ran, outcome)) if talk == id && ran.as_slice() == argv => match outcome {
                CliOutcome::Output(_) => {
                    children.push(button(
                        &format!("{key}-run"),
                        "Run again",
                        Intent::RunCli { index },
                    ));
                }
                CliOutcome::Running(label) => {
                    children.push(status(
                        &format!("{key}-running"),
                        &format!("Running on {label}…"),
                    ));
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

    /// `linked`: this device holds a chat pairing for the open task's
    /// computer, so its transcript can be read.
    fn chat(&self, open: &Open, computers: Option<&Computers>, linked: bool) -> Node<Intent> {
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
                // Without a chat pairing for the computer the transcript
                // never loads: say so instead of a spinner that never ends.
                let mut transcript = Conversation::pending_transcript(
                    "coder-transcript",
                    &echoes,
                    if linked {
                        Some(working.unwrap_or("Loading the chat"))
                    } else {
                        working
                    },
                );
                if let Element::Transcript { children, .. } = &mut transcript.element {
                    rows.append(children);
                    *children = rows;
                }
                transcript
            }
        };
        children.push(transcript);
        if open.conversation.is_none() && !linked {
            children.push(status(
                "coder-unlinked",
                &format!("This phone can't read Coder's messages on {label} yet."),
            ));
        }
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
        // An unknown task offers Stop as well: the computer ends it when
        // its process is gone, and never stops a live one but by asking
        // (#10124).
        if (running || phase == Some(Phase::Unknown)) && mode != Mode::Answer {
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
            // Once the turn ended, a follow-up goes to the conversation
            // that started it, whose router decides (#10094); without one
            // it goes straight to Coder.
            (false, Mode::Send) if self.talk_of(&open.host, &open.task).is_some() => {
                "Message OpenAgents…".to_owned()
            }
            (false, Mode::Send) => format!("Message Coder on {label}"),
            (false, Mode::Queue) => "Queue a message for Coder's next turn".to_owned(),
            (false, Mode::Answer) => "Answer Coder".to_owned(),
        };
        let choices = if editing.is_none() && mode == Mode::Queue {
            Choice::offered(phase)
        } else {
            Vec::new()
        };
        let allowed = computers.is_some_and(|c| c.can_operate(&open.host));
        if !running {
            children.extend(changes_view(open, allowed));
        }
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

/// The most diff lines the phone shows at once; the rest stays on the
/// computer, and the card says the change is longer.
const CHANGE_LINES: usize = 160;

/// The open chat's "What changed" card from the shared reviewer, and, when
/// opened, its diff line by line. `allowed` is this device's `operate`
/// right on the computer; without it no Publish shows.
fn changes_view(open: &Open, allowed: bool) -> Vec<Node<Intent>> {
    let Some(card) = open.review.card(allowed) else {
        return Vec::new();
    };
    let mut lines = vec![
        heading("coder-changes-title", "What changed"),
        status("coder-changes-summary", &card.summary),
    ];
    if let Some(revisions) = &card.revisions {
        lines.push(status("coder-changes-revisions", revisions));
    }
    for note in &card.notes {
        let color = match note.tone {
            crate::changes::Tone::Warning => crate::visual::inks().warning,
            crate::changes::Tone::Plain => gray(),
        };
        lines.push(text(
            &format!("coder-changes-note-{}", note.key),
            &note.text,
            TextRole::Status,
            color,
            false,
        ));
    }
    if let Some((label, url)) = &card.link {
        lines.push(node(
            "coder-changes-link",
            Element::Markdown {
                blocks: rust_native::markdown::parse(&format!("[{label}]({url})")),
            },
        ));
    }
    let mut buttons = Vec::new();
    for (action, label) in &card.actions {
        let (key, intent) = match action {
            crate::changes::CardAction::Open if open.changes_open => {
                ("coder-changes-close", Intent::CloseChanges)
            }
            crate::changes::CardAction::Open => ("coder-changes-open", Intent::OpenChanges),
            crate::changes::CardAction::Refresh => {
                ("coder-changes-refresh", Intent::RefreshChanges)
            }
            crate::changes::CardAction::Publish => {
                ("coder-changes-publish", Intent::PublishChanges)
            }
        };
        let label = if *action == crate::changes::CardAction::Open && open.changes_open {
            "Hide changes"
        } else {
            label
        };
        buttons.push(button(key, label, intent));
    }
    lines.push(row("coder-changes-buttons", buttons));
    if open.changes_open
        && let Some(document) = open.review.document()
    {
        for (index, line) in document.lines().iter().take(CHANGE_LINES).enumerate() {
            let color = match line.kind {
                crate::changes::Kind::Add => crate::visual::inks().added,
                crate::changes::Kind::Remove => crate::visual::inks().removed,
                crate::changes::Kind::Hunk | crate::changes::Kind::Meta => gray(),
                crate::changes::Kind::File | crate::changes::Kind::Context => white(),
            };
            // A long line is cut for the phone's view bound.
            let clipped: String = line.text.chars().take(240).collect();
            let mut shown = text(
                &format!("coder-changes-line-{index}"),
                if clipped.is_empty() { " " } else { &clipped },
                TextRole::Body,
                color,
                line.kind == crate::changes::Kind::File,
            );
            shown.style.monospace = Some(true);
            lines.push(shown);
        }
        if document.len() > CHANGE_LINES {
            lines.push(status(
                "coder-changes-more",
                &format!(
                    "{} more lines. Open the chat on the computer to read the rest.",
                    document.len() - CHANGE_LINES
                ),
            ));
        }
    }
    vec![node(
        "coder-changes",
        Element::Stack {
            axis: Axis::Vertical,
            children: lines,
        },
    )]
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
pub enum Availability<'a> {
    /// No computer this device may run work on.
    NotConfigured,
    /// A computer this device may run work on is connecting, as after launch.
    Connecting(&'a HostRecord),
    /// A computer this device may run work on is offline or out of date.
    Offline(&'a HostRecord),
    /// A computer can take a chat now.
    Ready(&'a HostRecord),
}

/// Whether `host`'s owner starts Coder at once for a device's coding reply
/// (#10101): its presence advertises
/// [`coder_host::access::protocol::CODER_START_AT_ONCE`] (its `coder.start`
/// is `at_once` and its auto-start policy is on). A computer that predates
/// the capability, or sent no presence yet, asks: the reply keeps **Run
/// Coder**. A hint for the phone's presentation only; the computer still
/// checks the grant and decides whether and how the task runs.
pub fn starts_at_once(host: &HostRecord) -> bool {
    host.presence.as_ref().is_some_and(|received| {
        received
            .presence
            .supports(coder_host::access::protocol::CODER_START_AT_ONCE)
    })
}

/// The coding agents `host`'s newest presence names, each with its state
/// (#10119): the list the host builds for its own chats, kept with the
/// computer's record and sent in this phone's chat context. Empty for a
/// computer that predates the flags or sent no presence yet.
pub fn engines_of(host: &HostRecord) -> Vec<crate::router::Engine> {
    let Some(received) = host.presence.as_ref() else {
        return Vec::new();
    };
    coder_host::access::protocol::engine_flags(
        received.presence.capabilities.iter().map(String::as_str),
    )
    .into_iter()
    .filter_map(|(engine, state)| {
        Some(crate::router::Engine {
            engine,
            state: crate::router::EngineState::of_word(state)?,
        })
    })
    .filter(crate::router::Engine::bounded)
    .take(crate::router::MAX_ENGINES)
    .collect()
}

/// Whether a new chat can start now, from the typed authority check and host
/// status, never from the words on screen. A ready computer is `selected`
/// when that one is ready, else the first.
pub fn availability<'a>(snapshot: &'a Snapshot, selected: Option<&str>) -> Availability<'a> {
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
pub enum Mode {
    /// Continue a finished chat with a follow-up turn.
    Send,
    /// Queue the message for the next turn.
    Queue,
    /// Answer the question or approval request Coder ended its turn with.
    Answer,
}

impl Mode {
    pub fn of(phase: Option<Phase>, attention: Option<Attention>) -> Self {
        match (phase, attention) {
            (Some(Phase::Waiting), Some(Attention::Input | Attention::Approval)) => Mode::Answer,
            (None | Some(Phase::Queued | Phase::Running | Phase::Waiting), _) => Mode::Queue,
            _ => Mode::Send,
        }
    }
}

/// How a task's turn ended, from its phase: `None` while it runs, waits,
/// or is unknown (#10094).
fn ending_of(phase: Phase) -> Option<openagents_chat::router::RunEnding> {
    use openagents_chat::router::RunEnding;
    match phase {
        Phase::Completed => Some(RunEnding::Finished),
        Phase::Failed => Some(RunEnding::Failed),
        Phase::Cancelled => Some(RunEnding::Stopped),
        Phase::Queued | Phase::Running | Phase::Waiting | Phase::Unknown => None,
    }
}

/// A task whose turn has not ended, as the router's context (#10143):
/// waiting for the person, else running (a task just started may have no
/// summary yet), with the computer's headline as what it is doing.
fn going_run(summary: Option<&ActivitySummary>) -> openagents_chat::router::CoderRun {
    use openagents_chat::router::{CoderRun, RunEnding};
    CoderRun {
        ending: if summary.is_some_and(|summary| summary.phase == Phase::Waiting) {
            RunEnding::Waiting
        } else {
            RunEnding::Running
        },
        turn: 1,
        engine: None,
        model: None,
        summary: summary
            .map(|summary| summary.headline.clone())
            .unwrap_or_default(),
        files: Vec::new(),
        commands: Vec::new(),
    }
}

/// What an ended task's chat shows of its last turn, as the router's
/// context (#10094): its last reply, the commands after the last message
/// the person sent, and the files its review read.
fn open_result(open: &Open, phase: Phase) -> Option<openagents_chat::router::CoderRun> {
    use openagents_chat::router::{CoderRun, RunFile};
    let ending = ending_of(phase)?;
    let rows = open
        .conversation
        .as_ref()
        .map(Conversation::rows)
        .unwrap_or_default();
    let asked = rows
        .iter()
        .rposition(|row| {
            matches!(
                &row.entry,
                crate::conversation::Entry::Message {
                    role: MessageRole::User,
                    ..
                }
            )
        })
        .map_or(0, |at| at + 1);
    let turn = &rows[asked.min(rows.len())..];
    let summary = turn
        .iter()
        .rev()
        .find_map(|row| match &row.entry {
            crate::conversation::Entry::Message {
                role: MessageRole::Assistant,
                text,
            } => Some(text.clone()),
            _ => None,
        })
        .unwrap_or_default();
    let commands = turn
        .iter()
        .filter_map(|row| match &row.entry {
            crate::conversation::Entry::Tool { detail, .. } if !detail.trim().is_empty() => {
                Some(detail.clone())
            }
            _ => None,
        })
        .collect();
    let files = open
        .review
        .review()
        .map(|review| {
            review
                .files
                .iter()
                .map(|file| RunFile {
                    path: file.path.clone(),
                    status: serde_json::to_value(file.status)
                        .ok()
                        .and_then(|value| value.as_str().map(str::to_owned))
                        .unwrap_or_else(|| "modified".to_owned()),
                })
                .collect()
        })
        .unwrap_or_default();
    Some(CoderRun {
        ending,
        turn: 1,
        engine: None,
        model: None,
        summary,
        files,
        commands,
    })
}

/// Whether the Coder list leaves a task out: its host's owner or a device
/// archived it, which the task's saved chat says.
pub fn archived(chat: Option<&coder_history::Chat>) -> bool {
    chat.is_some_and(|chat| chat.archived)
}

/// The saved chat of a task on a host, when the computer listed it.
pub type Saved<'a> = dyn Fn(&str, &str) -> Option<coder_history::Chat> + 'a;

/// One tappable row per task, newest message first, from the newest summary
/// of each. Archived tasks are left out.
#[cfg(any(test, feature = "test-support"))]
pub fn tasks(
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
pub fn task_rows(
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
pub fn unix_seconds(text: &str) -> Option<u64> {
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

/// How long Coder has worked, `seconds`, as its start card says it while
/// it runs: `12 min`, `1 h 5 min`. Nothing under a minute.
fn worked(seconds: u64) -> Option<String> {
    let minutes = seconds / 60;
    match minutes {
        0 => None,
        1..=59 => Some(format!("{minutes} min")),
        _ if minutes.is_multiple_of(60) => Some(format!("{} h", minutes / 60)),
        _ => Some(format!("{} h {} min", minutes / 60, minutes % 60)),
    }
}

/// The most characters of an outcome a start card shows.
const OUTCOME_CHARS: usize = 140;

/// One short line of how a Coder task's turn ended, from its chat's rows:
/// the first line of its last reply when it finished, or of the last
/// system message when it failed. Bounded to [`OUTCOME_CHARS`].
fn outcome(rows: &[crate::conversation::Row], phase: Phase) -> Option<String> {
    let role = match phase {
        Phase::Completed => MessageRole::Assistant,
        Phase::Failed => MessageRole::System,
        _ => return None,
    };
    fn said(row: &crate::conversation::Row, role: MessageRole) -> Option<&str> {
        match &row.entry {
            crate::conversation::Entry::Message { role: shown, text }
                if *shown == role && !text.trim().is_empty() =>
            {
                Some(text.as_str())
            }
            _ => None,
        }
    }
    let text = if role == MessageRole::Assistant {
        // The final reply: the messages after the run's last step, read
        // from its first (#10118: a reply that ends "The result:" and a
        // code block still opens with what happened).
        match rows
            .iter()
            .rposition(|row| !matches!(row.entry, crate::conversation::Entry::Message { .. }))
        {
            Some(last) => rows[last + 1..]
                .iter()
                .find_map(|row| said(row, role))
                .or_else(|| rows.iter().rev().find_map(|row| said(row, role)))?,
            // Only the end of the chat was read: the last message that
            // says something itself, not a lead-in to a block (#10118).
            None => rows
                .iter()
                .rev()
                .filter_map(|row| said(row, role))
                .find(|text| {
                    let first = text.trim_start().lines().next().unwrap_or_default().trim();
                    !first.ends_with(':') && !first.starts_with("```")
                })
                .or_else(|| rows.iter().rev().find_map(|row| said(row, role)))?,
        }
    } else {
        rows.iter().rev().find_map(|row| said(row, role))?
    };
    let line = text
        .lines()
        .map(|line| {
            line.trim()
                .trim_start_matches(['#', '*', '-', '>', ' '])
                .trim()
        })
        .find(|line| !line.is_empty())?;
    // Plain words: no inline code marks.
    let plain = line.replace('`', "");
    let mut line = plain.trim_matches('*').trim();
    // A long line keeps its first sentence.
    if line.chars().count() > OUTCOME_CHARS
        && let Some(end) = line.find(". ")
    {
        line = &line[..=end];
    }
    if line.chars().count() <= OUTCOME_CHARS {
        return Some(line.to_owned());
    }
    let cut: String = line.chars().take(OUTCOME_CHARS - 1).collect();
    Some(format!("{}…", cut.trim_end()))
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

/// Runs a read-only command an offer proposed on this phone, after the
/// person's tap. The phone's own Rust core answers `computer list`,
/// `show`, and `workspaces` from what it already knows; a command that
/// runs on the computer goes through [`cli_run`] instead.
pub fn run_cli(
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

/// Primary text, from the theme seam ([`crate::visual::inks`]).
fn white() -> Color {
    crate::visual::inks().text
}

/// Receded text, from the theme seam.
fn gray() -> Color {
    crate::visual::inks().quiet
}

fn node(key: &str, element: Element<Intent>) -> Node<Intent> {
    Node {
        key: key.into(),
        style: Style::default(),
        element,
    }
}

/// What a proposed command printed, as a code block in the conversation,
/// under the command itself.
fn cli_output_row(argv: &[String], lines: &[String]) -> Node<Intent> {
    node(
        "coder-cli-out",
        Element::Message {
            role: MessageRole::Assistant,
            note: Some(Offer::command_line(argv)),
            children: vec![Node {
                key: "coder-cli-out-md".into(),
                style: Style {
                    foreground: Some(gray()),
                    ..Style::default()
                },
                element: Element::Markdown {
                    blocks: vec![rust_native::markdown::Block::Code {
                        language: None,
                        text: lines.join("\n"),
                    }],
                },
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
    text(key, value, TextRole::Heading, white(), true)
}

fn body(key: &str, value: &str) -> Node<Intent> {
    text(key, value, TextRole::Body, white(), false)
}

/// What a phone says about Coder work a computer's thread ran outside that
/// computer's host.
fn outside_words(computer: &str, project: Option<&str>) -> String {
    let computer = if computer == "A computer" {
        "the computer"
    } else {
        computer
    };
    let task = project.map_or_else(
        || "Coder task".to_owned(),
        |p| format!("Coder task for {p}"),
    );
    format!(
        "This thread's {task} ran on {computer} outside the OpenAgents app, so this phone can't \
         open or stop it. Follow it on {computer} with openagents chat follow."
    )
}

fn status(key: &str, value: &str) -> Node<Intent> {
    text(key, value, TextRole::Status, gray(), false)
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
/// suggested question or an offer under a reply.
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

/// A saved chat's card with its context menu: the shared chat commands'
/// menu entries (`commands::Kind::Menu`) that apply to it, as items a long
/// press offers. Each item is a button in the view, so it activates only
/// against the revision that showed it.
fn chat_card(summary: &openagents_chat::basic_chats::Summary, row: Node<Intent>) -> Node<Intent> {
    use crate::commands::{Action, Kind, Overlay};
    let registry =
        crate::commands::registry(std::slice::from_ref(summary), Some(&summary.id), false);
    let mut overlay = Overlay::default();
    overlay.open(Kind::Menu);
    let items: Vec<Node<Intent>> = overlay
        .entries(&registry)
        .into_iter()
        .filter(|entry| entry.enabled)
        .filter_map(|entry| {
            let (action, glyph, suffix) = match entry.action {
                Action::Pin if summary.pinned => (ChatMenuAction::Unpin, Glyph::Pin, "unpin"),
                Action::Pin => (ChatMenuAction::Pin, Glyph::Pin, "pin"),
                Action::Archive => (ChatMenuAction::Archive, Glyph::Archive, "archive"),
                Action::Restore => (ChatMenuAction::Restore, Glyph::Restore, "restore"),
                _ => return None,
            };
            Some(icon_button(
                &format!("{}-{suffix}", row.key),
                &entry.label,
                glyph,
                false,
                Intent::ChatMenu {
                    id: summary.id.clone(),
                    action,
                },
            ))
        })
        .collect();
    if items.is_empty() {
        return row;
    }
    let mut children = vec![row];
    let key = format!("{}-card", children[0].key);
    children.extend(items);
    Node {
        key,
        style: Style {
            menu: Some(rust_native::style::Menu::Context),
            ..Style::default()
        },
        element: Element::Stack {
            axis: Axis::Vertical,
            children,
        },
    }
}

fn button(key: &str, label: &str, intent: Intent) -> Node<Intent> {
    Node {
        key: key.into(),
        style: Style {
            foreground: Some(white()),
            ..Style::default()
        },
        element: Element::Button {
            shortcut: None,
            label: label.into(),
            enabled: true,
            icon: None,
            intent,
        },
    }
}

fn card_intent(action: crate::cards::Action) -> Option<Intent> {
    use crate::cards::Action;
    Some(match action {
        Action::Retry => Intent::Retry,
        Action::RunCoder => Intent::RunCoder,
        Action::ConnectComputer => Intent::ConnectComputer,
        Action::OpenScreen { screen } => Intent::OpenScreen { screen },
        Action::Followup { index } => Intent::Followup { index },
        Action::Suggestion { id } => Intent::Starter { id },
        Action::RunCli { index } => Intent::RunCli { index },
        Action::Gym { .. } => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host_threads::{HostThreads, Link, Refusal};
    use rust_native::{Element, Node};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    const THREAD: &str = "0123456789abcdef0123456789abcdef";

    struct Fake {
        runs: AtomicUsize,
        sends: Mutex<Vec<String>>,
    }

    impl Link for Fake {
        fn list(&self, _host: &str) -> Result<Vec<coder_host::access::thread::ThreadRow>, Refusal> {
            Ok(vec![coder_host::access::thread::ThreadRow {
                thread: THREAD.into(),
                title: "Rain".into(),
                started: 1,
                updated: 2,
                pinned: false,
                coder: None,
            }])
        }
        fn read(
            &self,
            _host: &str,
            thread: &str,
            _before: Option<u64>,
        ) -> Result<coder_host::access::thread::ThreadPage, Refusal> {
            use coder_host::access::thread::{
                ThreadExtras, ThreadFollowup, ThreadRole, ThreadTurn,
            };
            let user = ThreadTurn {
                role: ThreadRole::User,
                text: "offer coder a haiku".into(),
                at: Some(1),
                stopped: false,
                model: None,
                request: None,
                extras: ThreadExtras::default(),
            };
            let reply = ThreadTurn {
                role: ThreadRole::Assistant,
                text: "Rain on the roof.".into(),
                at: Some(2),
                stopped: false,
                model: None,
                request: None,
                extras: ThreadExtras {
                    offers: vec![serde_json::json!({"offer": "run_coder"})],
                    followups: vec![ThreadFollowup {
                        answer: None,
                        label: "Say it shorter".into(),
                    }],
                    cards: vec![serde_json::json!({
                        "v": 2, "requires": [], "type": "card", "card": "news",
                        "items": [{
                            "title": "Rain", "line": "On the roof.",
                            "event": null, "path": "notes/rain"
                        }]
                    })],
                },
            };
            Ok(coder_host::access::thread::ThreadPage {
                thread: thread.into(),
                title: "Rain".into(),
                start: 0,
                total: 2,
                turns: vec![user, reply],
                busy: false,
                partial: String::new(),
                failure: None,
                coder: None,
                outside: None,
            })
        }
        fn send(
            &self,
            _host: &str,
            _thread: &str,
            _request: &str,
            text: &str,
        ) -> Result<(), Refusal> {
            self.sends.lock().unwrap().push(text.to_owned());
            Ok(())
        }
        fn stop(&self, _host: &str, _thread: &str, _request: Option<&str>) -> Result<(), Refusal> {
            Ok(())
        }
        fn run(&self, _host: &str, _thread: &str) -> Result<String, Refusal> {
            self.runs.fetch_add(1, Ordering::SeqCst);
            Ok("ab".repeat(32))
        }
    }

    fn until(mut done: impl FnMut() -> bool) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !done() {
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    fn buttons(node: &Node<Intent>, found: &mut Vec<(String, String)>) {
        if let Element::Button { label, .. } = &node.element {
            found.push((node.key.clone(), label.clone()));
        }
        let children = match &node.element {
            Element::Stack { children, .. }
            | Element::List { children, .. }
            | Element::Transcript { children, .. }
            | Element::Message { children, .. }
            | Element::Tool { children, .. } => children,
            _ => return,
        };
        for child in children {
            buttons(child, found);
        }
    }

    fn surfaces(node: &Node<Intent>, found: &mut Vec<String>) {
        if node.key.starts_with("gym-card-") {
            found.push(node.key.clone());
        }
        let children = match &node.element {
            Element::Stack { children, .. }
            | Element::List { children, .. }
            | Element::Transcript { children, .. }
            | Element::Message { children, .. }
            | Element::Tool { children, .. } => children,
            _ => return,
        };
        for child in children {
            surfaces(child, found);
        }
    }

    /// #10100: on the phone, as on the desktop, a new chat with no project is
    /// listed above older Coder chats in projects.
    #[test]
    fn the_phone_lists_a_new_chat_above_older_project_chats() {
        use crate::chat_list::Group;
        let row = |title: &str, group: Group, updated: u64| Recent {
            group,
            last: Some(updated),
            updated,
            row: button(title, title, Intent::NewChat),
        };
        let mut rows = vec![
            row("work on #10058", Group::Project("openagents".into()), 100),
            row(
                "work on #10061",
                Group::Project("openagents-host-tasks".into()),
                120,
            ),
            row("pinned", Group::Pinned, 1),
            row("archived", Group::Archived, 900),
            row("old", Group::Recent, 50),
            row("New chat", Group::Recent, 200),
        ];
        order_recent(&mut rows);
        let keys: Vec<&str> = rows.iter().map(|recent| recent.row.key.as_str()).collect();
        assert_eq!(
            keys,
            [
                "pinned",
                "New chat",
                "work on #10061",
                "work on #10058",
                "old",
                "archived"
            ]
        );
    }

    #[test]
    fn a_host_thread_shows_the_same_run_coder_chip_and_follow_ups() {
        let fake = Arc::new(Fake {
            runs: AtomicUsize::new(0),
            sends: Mutex::new(vec![]),
        });
        let link: Arc<dyn Link> = fake.clone();
        let threads = HostThreads::default();
        threads.poll(vec![("host".into(), "Studio Mac".into())], &link);
        until(|| !threads.rows().is_empty());
        threads.open("host", THREAD, link.clone());
        until(|| threads.shown().is_some_and(|shown| !shown.loading));
        let mut tab = CoderTab::new("coder:threads".into()).with_threads(threads, Some(link));
        let view = tab.thread_view(None);
        let meta = tab
            .threads
            .shown()
            .unwrap()
            .turns
            .last()
            .unwrap()
            .meta
            .clone()
            .unwrap();
        let shared = crate::cards::reply_actions_for(
            Some(&meta),
            &[],
            false,
            &crate::cards::Target::Ready("Studio Mac"),
            false,
        );
        let mut labels = vec![];
        buttons(&view, &mut labels);
        let run = labels.iter().find(|(key, _)| key == "coder-run").unwrap();
        let follow = labels
            .iter()
            .find(|(key, _)| key == "coder-followup-0")
            .unwrap();
        assert_eq!(run.1, "Run Coder on Studio Mac");
        assert_eq!(
            run.1,
            shared
                .chips
                .iter()
                .find(|chip| chip.key == "coder-run")
                .unwrap()
                .label
        );
        assert_eq!(follow.1, "Say it shorter");
        assert!(labels.iter().all(|(key, _)| !key.starts_with("coder-cli")));
        let mut cards = vec![];
        surfaces(&view, &mut cards);
        assert!(!cards.is_empty(), "the news card is on the thread");

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let secret = secp256k1::SecretKey::from_byte_array([0x31; 32]).unwrap();
        let mut chats =
            crate::chats::Chats::new(runtime.handle().clone(), secret, Err("test".into()));
        let rendered = tab.render(None, &mut chats).expect("the thread view");
        tab.activate(
            &rust_native::Activation {
                instance: rendered["instance"].as_str().unwrap().into(),
                revision: rendered["revision"].as_u64().unwrap(),
                node: "coder-run".into(),
            },
            None,
            &mut chats,
        );
        assert_eq!(
            tab.notice.as_deref(),
            Some("This phone can only read this chat")
        );
        assert_eq!(fake.runs.load(Ordering::SeqCst), 0);
        let rendered = tab.render(None, &mut chats).expect("the thread view");
        tab.activate(
            &rust_native::Activation {
                instance: rendered["instance"].as_str().unwrap().into(),
                revision: rendered["revision"].as_u64().unwrap(),
                node: "coder-followup-0".into(),
            },
            None,
            &mut chats,
        );
        until(|| {
            fake.sends
                .lock()
                .unwrap()
                .iter()
                .any(|text| text == "Say it shorter")
        });
    }

    /// Only a turn that ended sends a follow-up to the conversation's
    /// router; one that runs, waits, or is unknown keeps it Coder's
    /// (#10094).
    /// A start card says how long Coder has worked in short words (#10118).
    #[test]
    fn a_running_start_says_how_long_in_short_words() {
        assert_eq!(worked(59), None);
        assert_eq!(worked(60).as_deref(), Some("1 min"));
        assert_eq!(worked(12 * 60 + 59).as_deref(), Some("12 min"));
        assert_eq!(worked(2 * 3_600).as_deref(), Some("2 h"));
        assert_eq!(worked(3_600 + 5 * 60).as_deref(), Some("1 h 5 min"));
    }

    /// The outcome a finished start card shows is one bounded line of the
    /// task's last reply; a failed one, of the failure's message (#10118).
    #[test]
    fn an_ended_run_shows_one_line_of_how_it_ended() {
        use crate::conversation::Entry;
        let row = |role: MessageRole, text: &str| crate::conversation::Row {
            segment: 0,
            offset: 0,
            end: 0,
            part: 0,
            carried: false,
            entry: Entry::Message {
                role,
                text: text.into(),
            },
            blocks: vec![],
        };
        let rows = vec![
            row(MessageRole::User, "Archive the app and upload it"),
            row(MessageRole::Assistant, "Archiving now."),
            row(
                MessageRole::Assistant,
                "\n## Uploaded build 42 to TestFlight\n\nIt took 21 minutes.",
            ),
        ];
        assert_eq!(
            outcome(&rows, Phase::Completed).as_deref(),
            Some("Uploaded build 42 to TestFlight")
        );
        assert_eq!(outcome(&rows, Phase::Failed), None);
        assert_eq!(outcome(&rows, Phase::Running), None);
        let mut failed = rows.clone();
        failed.push(row(
            MessageRole::System,
            "xcodebuild exited with code 65\nsee the log",
        ));
        assert_eq!(
            outcome(&failed, Phase::Failed).as_deref(),
            Some("xcodebuild exited with code 65")
        );
        // The final reply is read from its first message, in plain words:
        // a lead-in to a code block is not the outcome (#10118).
        let tool = crate::conversation::Row {
            entry: Entry::Tool {
                name: "shell".into(),
                detail: "scripts/release/testflight.sh wait".into(),
                body: String::new(),
            },
            ..rows[0].clone()
        };
        let lead_in = vec![
            rows[0].clone(),
            row(MessageRole::Assistant, "Starting the release."),
            tool,
            row(
                MessageRole::Assistant,
                "Dry run succeeded for build `44`, from commit `603eebd19a`.",
            ),
            row(MessageRole::Assistant, "Release script's final result:"),
            row(MessageRole::Assistant, "```\nDone: build 44 archived\n```"),
        ];
        assert_eq!(
            outcome(&lead_in, Phase::Completed).as_deref(),
            Some("Dry run succeeded for build 44, from commit 603eebd19a.")
        );
        // Only the chat's end was read: the lead-in and its block are
        // passed over for the line that says what happened.
        assert_eq!(
            outcome(&lead_in[3..], Phase::Completed).as_deref(),
            Some("Dry run succeeded for build 44, from commit 603eebd19a.")
        );
        let long = format!("{}. {}", "a".repeat(60), "b".repeat(200));
        let shown = outcome(&[row(MessageRole::Assistant, &long)], Phase::Completed).unwrap();
        assert_eq!(shown, format!("{}.", "a".repeat(60)));
        let shown = outcome(
            &[row(MessageRole::Assistant, &"c".repeat(400))],
            Phase::Completed,
        )
        .unwrap();
        assert_eq!(shown.chars().count(), OUTCOME_CHARS);
        assert!(shown.ends_with('…'));
    }

    /// A conversation whose Coder task has not ended tells the router it
    /// runs or waits, with the computer's headline (#10143); a task just
    /// started, with no summary yet, runs.
    #[test]
    fn a_going_task_is_sent_as_running_or_waiting() {
        use openagents_chat::router::RunEnding;
        assert_eq!(going_run(None).ending, RunEnding::Running);
        assert!(going_run(None).summary.is_empty());
        let summary = |phase| ActivitySummary {
            host: "a".repeat(64),
            subject_kind: SubjectKind::Task,
            subject: "b".repeat(64),
            sequence: 1,
            phase,
            headline: "Reading acceptance-repo".into(),
            attention: Attention::None,
            updated_at: 0,
        };
        for (phase, ending) in [
            (Phase::Queued, RunEnding::Running),
            (Phase::Running, RunEnding::Running),
            (Phase::Unknown, RunEnding::Running),
            (Phase::Waiting, RunEnding::Waiting),
        ] {
            let run = going_run(Some(&summary(phase)));
            assert_eq!(run.ending, ending, "{phase:?}");
            assert_eq!(run.summary, "Reading acceptance-repo");
        }
    }

    #[test]
    fn only_an_ended_turn_routes_a_follow_up() {
        use openagents_chat::router::RunEnding;
        assert_eq!(ending_of(Phase::Completed), Some(RunEnding::Finished));
        assert_eq!(ending_of(Phase::Failed), Some(RunEnding::Failed));
        assert_eq!(ending_of(Phase::Cancelled), Some(RunEnding::Stopped));
        for phase in [
            Phase::Queued,
            Phase::Running,
            Phase::Waiting,
            Phase::Unknown,
        ] {
            assert_eq!(ending_of(phase), None, "{phase:?}");
        }
    }
}
