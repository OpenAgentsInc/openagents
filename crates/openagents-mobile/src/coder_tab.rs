//! The Coder surface: chats with Coder on your computers.
//!
//! A new chat is a NIP-HOST `task.create` on the chosen computer, the same
//! operation as the Computers surface's Order work; with the host's
//! auto-start policy on, the host runs Coder's engine on it right away. An
//! open chat reads the task's ATIF transcript through the computer's
//! read-only history observer (its `coder` source) and polls it while the
//! task runs; the host's signed activity summaries say whether it is
//! running.
//!
//! Everything sent to an open chat is a NIP-HOST `task.command` kept in the
//! durable [`Outbox`] until the computer answers, so it survives a relaunch
//! and bad connectivity and never runs twice. The composer's one action
//! follows the task's state and this device's `operate` right, never the
//! text: in a finished chat it sends a follow-up that continues the same
//! task; while Coder works it queues the message for the next turn, or,
//! after **Steer now**, stops the turn and continues with the message (the
//! engine's emulated steering, chosen explicitly). **Stop** interrupts the
//! current turn.

use crate::chats::Chats;
use crate::conversation::Conversation;
use crate::outbox::{Attempt, Draft, Outbox};
use coder_computers::{
    Action, Capabilities, Computers, Denial, HostRecord, HostStatus, OfflineCause, Platform,
    Snapshot, authority,
};
use coder_host::CommandAction;
use nostr::activity_summary::{ActivitySummary, Phase, SubjectKind};
use rust_native::style::{Color, Space, Style, TextWeight};
use rust_native::{
    Activation, Axis, Element, Glyph, Icon, MessageRole, Node, TextRole, ValidatedView, View,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The largest message, as NIP-HOST `task.create` allows.
const MAX_PROMPT_BYTES: usize = 16 * 1024;
const SHOWN_TASKS: usize = 50;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Intent {
    /// Use the next computer that can take work.
    NextComputer,
    Open {
        host: String,
        task: String,
    },
    Back,
    Earlier,
    Stop,
    /// Switch the running chat's composer between queueing and steering.
    Steer,
}

/// An open chat: one task on one computer.
struct Open {
    host: String,
    task: String,
    conversation: Option<Conversation>,
    /// The chat whose transcript is shown: a later turn is a newer chat.
    chat: Option<String>,
    /// The summary sequence last seen, to notice a new turn.
    seen: Option<u64>,
    /// The composer steers the running turn instead of queueing.
    steer: bool,
}

pub struct CoderTab {
    instance: String,
    revision: u64,
    current: Option<ValidatedView<Intent>>,
    selected: Option<String>,
    notice: Option<String>,
    composers: u64,
    /// The first line of each chat this device started, by task ID.
    titles: BTreeMap<String, String>,
    /// When this device sent each chat's first message, by task ID: its last
    /// message until the computer lists the chat.
    sent: BTreeMap<String, u64>,
    open: Option<Open>,
    outbox: Outbox,
}

impl CoderTab {
    pub fn new(instance: String) -> Self {
        Self {
            instance,
            revision: 0,
            current: None,
            selected: None,
            notice: None,
            composers: 1,
            titles: BTreeMap::new(),
            sent: BTreeMap::new(),
            open: None,
            outbox: Outbox::open(None),
        }
    }

    /// Keep chat commands in `outbox`, which survives a relaunch.
    pub fn with_outbox(mut self, outbox: Outbox) -> Self {
        self.outbox = outbox;
        self
    }

    /// Send every command that is due, keeping any the computer did not
    /// answer for a later try with the same ID. A refusal shows its reason.
    pub fn flush(&mut self, computers: Option<&mut Computers>) {
        let Some(computers) = computers else { return };
        let now = computers.snapshot().now;
        for pending in self.outbox.due(now) {
            let attempt = match computers.command_task(&pending.host, &pending.command) {
                Ok(()) => Attempt::Answered,
                Err(coder_computers::Refusal::Failed(error))
                    if matches!(
                        error.code,
                        coder_host::Code::Transport | coder_host::Code::Unavailable
                    ) =>
                {
                    Attempt::Unreached
                }
                Err(refusal) => Attempt::Refused(refusal.reason()),
            };
            if let Attempt::Refused(reason) = &attempt {
                self.notice = Some(reason.clone());
            }
            self.outbox.settle(&pending.command.command, &attempt, now);
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
        if self.outbox.push(&host, draft, now).is_none() {
            self.notice = Some("Too many messages are waiting to send. Try again later.".into());
            return false;
        }
        self.notice = None;
        self.flush(Some(computers));
        true
    }

    /// The host and task of the open chat.
    #[cfg(test)]
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

    /// The workspace a new chat uses: `openagents` when the computer lists
    /// it, else its first.
    fn workspace(host: &HostRecord) -> Option<String> {
        let listed = host.workspaces.as_ref()?;
        listed
            .iter()
            .find(|label| *label == "openagents")
            .or_else(|| listed.first())
            .cloned()
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
        match intent {
            Intent::NextComputer => {
                let Some(computers) = computers else { return };
                let hosts = Self::hosts(computers);
                let at = self
                    .chosen(computers)
                    .and_then(|chosen| hosts.iter().position(|h| h.key == chosen.key))
                    .unwrap_or(0);
                self.selected = hosts
                    .get((at + 1) % hosts.len().max(1))
                    .map(|host| host.key.clone());
            }
            Intent::Open { host, task } => self.open(host, task, chats),
            Intent::Back => self.open = None,
            Intent::Earlier => {
                if let Some(conversation) = self.open.as_ref().and_then(|o| o.conversation.as_ref())
                {
                    conversation.earlier();
                }
            }
            Intent::Stop => {
                let Some(computers) = computers else { return };
                let reason = "Stopped from a phone.";
                self.command(CommandAction::Interrupt, reason, false, computers);
            }
            Intent::Steer => {
                if let Some(open) = self.open.as_mut() {
                    open.steer = !open.steer;
                }
            }
        }
    }

    fn open(&mut self, host: String, task: String, chats: &mut Chats) {
        self.open = Some(Open {
            host,
            task,
            conversation: None,
            chat: None,
            seen: None,
            steer: false,
        });
        self.attach(chats);
    }

    /// Find the open chat's transcript once the computer lists it.
    fn attach(&mut self, chats: &mut Chats) {
        let Some(open) = self.open.as_mut() else {
            return;
        };
        match chats.coder_chat(&open.host, &open.task) {
            // A later turn's transcript is a newer chat for the same task;
            // it carries the earlier turns, so it replaces the one shown.
            Some((_, client, chat)) if open.chat.as_ref() != Some(&chat.id) => {
                open.chat = Some(chat.id.clone());
                open.conversation = Some(Conversation::open(chats.runtime(), client, chat));
            }
            Some(_) => {}
            None => chats.refresh_linked(&open.host),
        }
    }

    /// Read the computer's chat list again when the open task's summary
    /// moves, so a new turn's transcript is found.
    fn follow(&mut self, computers: Option<&Computers>, chats: &mut Chats) {
        let (Some(open), Some(computers)) = (self.open.as_mut(), computers) else {
            return;
        };
        let sequence =
            Self::summary(computers.snapshot(), &open.host, &open.task).map(|s| s.sequence);
        if sequence != open.seen {
            if open.seen.is_some() {
                chats.refresh_linked(&open.host);
            }
            open.seen = sequence;
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
        let Some(computers) = computers else { return };
        if prompt.is_empty() {
            return;
        }
        if let Some(open) = &self.open {
            let phase = Self::summary(computers.snapshot(), &open.host, &open.task)
                .map(|summary| summary.phase);
            let (action, emulate) = match Mode::of(phase, open.steer) {
                Mode::Send => (CommandAction::Send, false),
                Mode::Queue => (CommandAction::Queue, false),
                Mode::Steer => (CommandAction::Steer, true),
            };
            if self.command(action, prompt, emulate, computers) {
                self.composers += 1;
                if let Some(open) = self.open.as_mut() {
                    open.steer = false;
                }
            }
            return;
        }
        let host = match &self.open {
            Some(open) => open.host.clone(),
            None => match self.chosen(computers) {
                Some(record) => record.key.clone(),
                None => return,
            },
        };
        if computers
            .snapshot()
            .host(&host)
            .and_then(Self::workspace)
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
        let Some(workspace) = Self::workspace(record) else {
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
                self.titles.insert(task.clone(), title);
                self.sent.insert(task.clone(), computers.snapshot().now);
                self.notice = None;
                self.open(host, task, chats);
            }
            Err(refusal) => self.notice = Some(refusal.reason()),
        }
    }

    pub fn render(
        &mut self,
        computers: Option<&Computers>,
        chats: &mut Chats,
    ) -> Option<serde_json::Value> {
        self.follow(computers, chats);
        self.attach(chats);
        // Follow a running chat's transcript.
        if let (Some(open), Some(computers)) = (&self.open, computers)
            && let Some(conversation) = &open.conversation
            && Self::summary(computers.snapshot(), &open.host, &open.task)
                .is_none_or(|s| Self::running(s.phase))
        {
            conversation.poll();
        }
        self.revision += 1;
        let view = loop {
            let root = match &self.open {
                Some(open) => self.chat(open, computers),
                None => self.home(computers, chats),
            };
            match View::new(self.instance.clone(), self.revision, root).validate() {
                Ok(view) => break view,
                // A long chat can outgrow one view; keep its newest half.
                Err(_) => {
                    let conversation = self.open.as_ref()?.conversation.as_ref()?;
                    conversation.shrink();
                    if conversation.is_empty() {
                        return None;
                    }
                }
            }
        };
        let value = serde_json::to_value(view.view()).ok();
        self.current = Some(view);
        value
    }

    fn composer(&self, placeholder: String, enabled: bool, busy: bool) -> Node<Intent> {
        node(
            "coder-composer",
            Element::Composer {
                token: format!("coder-composer-{}", self.composers),
                placeholder,
                max_bytes: MAX_PROMPT_BYTES,
                enabled,
                busy,
                stop: busy.then_some(Intent::Stop),
            },
        )
    }

    /// Whether a new chat can start now on the chosen computer.
    fn availability<'a>(&self, computers: Option<&'a Computers>) -> Availability<'a> {
        computers.map_or(Availability::NotConfigured, |computers| {
            availability(computers.snapshot(), self.selected.as_deref())
        })
    }

    fn home(&self, computers: Option<&Computers>, chats: &Chats) -> Node<Intent> {
        let mut children = vec![heading("coder-title", "Coder")];
        let availability = self.availability(computers);
        let Some(computers) =
            computers.filter(|_| !matches!(availability, Availability::NotConfigured))
        else {
            children.push(body(
                "coder-empty",
                "Add a computer under Account > Computers, then chat with Coder on it.",
            ));
            return page(children);
        };
        let host = match availability {
            Availability::Ready(host) => {
                let place = match Self::workspace(host) {
                    Some(workspace) => format!("On {} · {workspace}", host.label),
                    None => format!("On {}", host.label),
                };
                let mut place_row = vec![status("coder-computer", &place)];
                if Self::hosts(computers).len() > 1 {
                    place_row.push(button("coder-next", "Change", Intent::NextComputer));
                }
                children.push(row("coder-place", place_row));
                Some(host)
            }
            Availability::Connecting(host) => {
                children.push(status(
                    "coder-connecting",
                    &format!("Connecting to {}…", host.label),
                ));
                None
            }
            Availability::Offline(host) => {
                children.push(status(
                    "coder-offline",
                    &format!("{} is offline.", host.label),
                ));
                None
            }
            Availability::NotConfigured => None,
        };
        if let Some(notice) = &self.notice {
            children.push(status("coder-notice", notice));
        }
        let saved = |host: &str, task: &str| chats.coder_chat(host, task).map(|(_, _, chat)| chat);
        let rows = tasks(computers.snapshot(), &self.titles, &self.sent, &saved);
        if rows.is_empty() {
            children.push(status(
                "coder-none",
                "No chats yet. Write to Coder below to start one.",
            ));
        } else {
            children.push(node(
                "coder-chats",
                Element::List {
                    label: "Chats with Coder".into(),
                    children: rows,
                },
            ));
        }
        if let Some(host) = host {
            children.push(self.composer(format!("Message Coder on {}", host.label), true, false));
        }
        page(children)
    }

    fn chat(&self, open: &Open, computers: Option<&Computers>) -> Node<Intent> {
        let summary = computers.and_then(|c| Self::summary(c.snapshot(), &open.host, &open.task));
        let phase = summary.map(|s| s.phase);
        // The host's typed note, such as a missing model capacity.
        let note = summary
            .filter(|s| {
                s.headline != nostr::activity_summary::generic_headline(SubjectKind::Task, s.phase)
            })
            .map(|s| s.headline.clone());
        let running = phase.is_none_or(Self::running);
        let label = computers
            .and_then(|c| c.snapshot().host(&open.host))
            .map_or_else(|| "your computer".to_owned(), |h| h.label.clone());
        let working = match phase {
            Some(Phase::Queued) | None => Some("Queued"),
            Some(Phase::Running) => Some("Coder is working"),
            Some(Phase::Waiting) => Some("Waiting for you on the computer"),
            _ => None,
        };
        // Only the breadcrumb bar heads a chat; the transcript follows it.
        let mut children = vec![
            // The breadcrumb bar: back to the chats list, and where the
            // chat runs.
            row(
                "coder-chat-header",
                vec![
                    icon_button("coder-back", "Coder", Glyph::Back, false, Intent::Back),
                    status(
                        "coder-chat-place",
                        &format!("{} · {label}", phase.map_or("Starting", phase_label)),
                    ),
                ],
            ),
        ];
        if let Some(note) = &note {
            children.push(status("coder-chat-note", note));
        }
        if let Some(notice) = &self.notice {
            children.push(status("coder-notice", notice));
        }
        let transcript = match &open.conversation {
            Some(conversation) => {
                conversation.transcript("coder-transcript", Intent::Earlier, working)
            }
            None => {
                // The computer has not listed the transcript yet: show the
                // message this device sent.
                let mut rows = vec![];
                if let Some(title) = self.titles.get(&open.task) {
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
                rows.push(node(
                    "coder-waiting",
                    Element::Working {
                        label: working.unwrap_or("Loading the chat").into(),
                    },
                ));
                node(
                    "coder-transcript",
                    Element::Transcript {
                        label: "Messages".into(),
                        children: rows,
                        earlier: None,
                    },
                )
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
        let mode = Mode::of(phase, open.steer);
        if running {
            children.push(row(
                "coder-controls",
                vec![
                    button("coder-stop", "Stop", Intent::Stop),
                    button(
                        "coder-steer",
                        if open.steer {
                            "Queue instead"
                        } else {
                            "Steer now"
                        },
                        Intent::Steer,
                    ),
                ],
            ));
        }
        let placeholder = match mode {
            Mode::Send => format!("Message Coder on {label}"),
            Mode::Queue => "Queue a message for Coder's next turn".to_owned(),
            Mode::Steer => "Steer Coder: stop this turn and continue with your message".to_owned(),
        };
        let allowed = computers.is_some_and(|c| c.can_operate(&open.host));
        children.push(self.composer(placeholder, allowed, false));
        page(children)
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

/// What the open chat's composer does, from the task's phase and the
/// device's explicit steer choice; never from the text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    /// Continue a finished chat with a follow-up turn.
    Send,
    /// Queue the message for the next turn.
    Queue,
    /// Stop the running turn and continue with the message.
    Steer,
}

impl Mode {
    pub(crate) fn of(phase: Option<Phase>, steer: bool) -> Self {
        match phase {
            None | Some(Phase::Queued | Phase::Running | Phase::Waiting) if steer => Mode::Steer,
            None | Some(Phase::Queued | Phase::Running | Phase::Waiting) => Mode::Queue,
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
pub(crate) fn tasks(
    snapshot: &Snapshot,
    titles: &BTreeMap<String, String>,
    sent: &BTreeMap<String, u64>,
    saved: &Saved<'_>,
) -> Vec<Node<Intent>> {
    let mut newest: Vec<&ActivitySummary> = vec![];
    for summary in snapshot
        .activity
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
    // A summary's time is when the host last published it, which a host
    // restart resets for every task; a chat's time is its last message.
    let last = |summary: &ActivitySummary| {
        saved(&summary.host, &summary.subject)
            .and_then(|chat| chat.updated_at.as_deref().and_then(unix_seconds))
            .or_else(|| sent.get(&summary.subject).copied())
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
            // The first line this device sent, else the transcript's title,
            // else the host's generic headline.
            let title = titles.get(&summary.subject).cloned().unwrap_or_else(|| {
                saved(&summary.host, &summary.subject)
                    .map(|chat| chat.title)
                    .filter(|title| !title.is_empty() && !title.starts_with("Saved "))
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
            button(
                &format!("task-{}", &summary.subject[..16.min(summary.subject.len())]),
                &format!(
                    "{title}\n{} · {label}{when}{note}",
                    phase_label(summary.phase),
                ),
                Intent::Open {
                    host: summary.host.clone(),
                    task: summary.subject.clone(),
                },
            )
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

fn ago(now: u64, then: u64) -> String {
    let seconds = now.saturating_sub(then);
    match seconds {
        0..=59 => "just now".into(),
        60..=3_599 => format!("{} min ago", seconds / 60),
        3_600..=86_399 => format!("{} h ago", seconds / 3_600),
        _ => format!("{} d ago", seconds / 86_400),
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
        *icon = Some(Icon { glyph, circular });
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
