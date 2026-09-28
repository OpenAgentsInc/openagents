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

use crate::chats::Chats;
use crate::coder_list::{List, Row, Store};
use crate::conversation::Conversation;
use crate::outbox::{Attempt, Draft, Outbox};
use coder_computers::{
    Action, Capabilities, Computers, Denial, HostRecord, HostStatus, OfflineCause, Platform,
    Snapshot, authority,
};
use coder_host::{CommandAction, QueueEdit, TaskQueue};
use nostr::activity_summary::{ActivitySummary, Attention, Phase, SubjectKind};
use rust_native::style::{Color, Space, Style, TextAlign, TextWeight};
use rust_native::{
    Activation, Axis, ComposerChoice, Element, Glyph, Icon, MessageRole, Node, TextRole,
    ValidatedView, View,
};
use serde::{Deserialize, Serialize};

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
    /// Open the New chat screen.
    NewChat,
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
    /// The New chat screen shows, where a first message starts a chat.
    composing: bool,
    outbox: Outbox,
    /// The current composer's choices: each token this tab minted and what
    /// it sends.
    choices: Vec<(String, Choice)>,
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
            list: Store::open(None),
            open: None,
            composing: false,
            outbox: Outbox::open(None),
            choices: Vec::new(),
        }
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
                Attempt::Refused(reason) => self.notice = Some(reason.clone()),
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
            Intent::NewChat => {
                self.notice = None;
                self.composing = true;
            }
            // Back from a chat or the New chat screen: the chats list.
            Intent::Back => {
                self.open = None;
                self.composing = false;
            }
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
        self.open = Some(Open {
            host,
            task,
            conversation: None,
            chat: None,
            seen: None,
            queue: None,
            listed: None,
            unlisted: false,
            leased_at: None,
            editing: None,
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
        // Only the New chat screen starts a chat.
        if !self.composing {
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
                let now = computers.snapshot().now;
                self.list.list.titles.insert(task.clone(), title);
                self.list.list.sent.insert(task.clone(), now);
                self.list.save();
                self.notice = None;
                self.composing = false;
                self.open(host, task, chats);
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
                None if self.composing => self.new_chat(computers),
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

    fn home(&self, computers: Option<&Computers>, chats: &Chats) -> Node<Intent> {
        let availability = self.availability(computers);
        let Some(computers) =
            computers.filter(|_| !matches!(availability, Availability::NotConfigured))
        else {
            return page(vec![
                heading("coder-title", "Coder"),
                body(
                    "coder-empty",
                    "Add a computer under Account > Computers, then chat with Coder on it.",
                ),
            ]);
        };
        let mut new = icon_button(
            "coder-new",
            "New chat",
            Glyph::Compose,
            true,
            Intent::NewChat,
        );
        new.style.align = Some(TextAlign::End);
        let mut children = vec![row(
            "coder-header",
            vec![heading("coder-title", "Coder"), new],
        )];
        if let Some(line) = Self::unavailable(&availability) {
            children.push(line);
        }
        if let Some(notice) = &self.notice {
            children.push(status("coder-notice", notice));
        }
        let saved = |host: &str, task: &str| chats.coder_chat(host, task).map(|(_, _, chat)| chat);
        let rows = tasks(
            computers.snapshot(),
            &self.activity(computers),
            &self.list.list,
            &saved,
        );
        if rows.is_empty() {
            children.push(status(
                "coder-none",
                "No chats yet. Tap New chat to start one.",
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

    /// The New chat screen: where the chat will run, and a composer whose
    /// first message starts it.
    fn new_chat(&self, computers: Option<&Computers>) -> Node<Intent> {
        let availability = self.availability(computers);
        let mut children = vec![row(
            "coder-new-header",
            vec![icon_button(
                "coder-back",
                "Coder",
                Glyph::Back,
                false,
                Intent::Back,
            )],
        )];
        let ready = match (&availability, computers) {
            (Availability::Ready(host), Some(computers)) => {
                let place = match Self::workspace(host) {
                    Some(workspace) => format!("On {} · {workspace}", host.label),
                    None => format!("On {}", host.label),
                };
                let mut place_row = vec![status("coder-computer", &place)];
                if Self::hosts(computers).len() > 1 {
                    place_row.push(button("coder-next", "Change", Intent::NextComputer));
                }
                children.push(row("coder-place", place_row));
                Some(host.label.clone())
            }
            (Availability::NotConfigured, _) => {
                children.push(body(
                    "coder-empty",
                    "Add a computer under Account > Computers, then chat with Coder on it.",
                ));
                None
            }
            _ => {
                children.extend(Self::unavailable(&availability));
                None
            }
        };
        if let Some(notice) = &self.notice {
            children.push(status("coder-notice", notice));
        }
        // An empty conversation fills the screen above the composer.
        children.push(node(
            "coder-new-transcript",
            Element::Transcript {
                label: "New chat".into(),
                children: vec![],
                earlier: None,
            },
        ));
        let placeholder = match &availability {
            Availability::Ready(host)
            | Availability::Connecting(host)
            | Availability::Offline(host) => {
                format!("Message Coder on {}", host.label)
            }
            Availability::NotConfigured => "Message Coder".to_owned(),
        };
        // The New Chat screen exists to write a message: it opens ready to type.
        children.push(self.composer_with(placeholder, ready.is_some(), false, &[], None, true));
        page(children)
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
                if let Some(title) = self.list.list.titles.get(&open.task) {
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
pub(crate) fn tasks(
    snapshot: &Snapshot,
    activity: &[ActivitySummary],
    known: &List,
    saved: &Saved<'_>,
) -> Vec<Node<Intent>> {
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
