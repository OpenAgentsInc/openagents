//! Portable client state for host-owned hosted conversations.
use openagents_chat::basic_chats::Summary;
use openagents_chat::service::{Command, Snapshot};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct PendingSend {
    pub chat: String,
    pub request: String,
    pub text: String,
}

/// A client owns selection and observations; its host owns encrypted records.
/// Native drafts, focus, and signing credentials are adapter responsibilities.
pub struct Session {
    pub cards: crate::cards::Cards,
    pub images: crate::attachments::Drafts,
    pub selected: Option<String>,
    pub states: BTreeMap<String, Snapshot>,
    pub summaries: Vec<Summary>,
    pub send: BTreeMap<String, PendingSend>,
    pub error: Option<String>,
    pub revision: u64,
    pub reading: bool,
    pending: BTreeMap<u64, Command>,
    handoff_errors: BTreeMap<String, String>,
    next_ticket: u64,
    poll: Instant,
    listed: bool,
    observed: BTreeMap<String, u64>,
    list_ticket: u64,
    list_version: u64,
    list_more: Option<usize>,
}

impl Session {
    pub fn new(now: Instant) -> Self {
        Self {
            cards: crate::cards::Cards::default(),
            images: crate::attachments::Drafts::default(),
            selected: None,
            states: BTreeMap::new(),
            summaries: vec![],
            send: BTreeMap::new(),
            error: None,
            revision: 0,
            reading: false,
            pending: BTreeMap::new(),
            handoff_errors: BTreeMap::new(),
            next_ticket: 1,
            poll: now,
            listed: false,
            observed: BTreeMap::new(),
            list_ticket: 0,
            list_version: 0,
            list_more: None,
        }
    }
    pub fn request(&mut self, command: Command) -> (u64, Command) {
        let ticket = self.next_ticket;
        self.next_ticket += 1;
        self.pending.insert(ticket, command.clone());
        (ticket, command)
    }
    pub fn state(&self) -> Option<&Snapshot> {
        self.selected.as_ref().and_then(|id| self.states.get(id))
    }
    pub fn busy(&self) -> bool {
        self.state().is_some_and(|state| state.busy)
            || self
                .selected
                .as_ref()
                .is_some_and(|id| self.send.contains_key(id))
    }
    pub fn select(&mut self, id: &str) -> bool {
        if self.selected.as_deref() == Some(id) {
            return false;
        }
        self.selected = Some(id.into());
        self.error = None;
        self.reading = false;
        self.revision += 1;
        true
    }
    pub fn new_chat(&mut self) -> (u64, Command) {
        let chat = uuid::Uuid::new_v4().simple().to_string();
        self.select(&chat);
        self.request(Command::Create { chat })
    }
    pub fn tick(&mut self, now: Instant) -> Option<(u64, Command)> {
        if now < self.poll
            || self.pending.values().any(|command| {
                matches!(
                    command,
                    Command::List { .. } | Command::ListMore { .. } | Command::Read { .. }
                )
            })
        {
            return None;
        }
        self.poll = now + Duration::from_millis(if self.busy() { 100 } else { 1000 });
        if let Some(after) = self.list_more.take() {
            return Some(self.request(Command::ListMore {
                after,
                version: self.list_version,
            }));
        }
        if self.listed && self.selected.is_none() && self.error.is_none() {
            return Some(self.new_chat());
        }
        Some(
            self.request(
                self.selected
                    .as_ref()
                    .map_or(Command::List {}, |chat| Command::Read {
                        chat: chat.clone(),
                        before: None,
                    }),
            ),
        )
    }
    pub fn next_wake(&self, now: Instant) -> Instant {
        self.poll.max(now + Duration::from_millis(100))
    }
    /// Bind a send to its exact bytes until a durable host acknowledgment arrives.
    pub fn submit(&mut self, request: String, text: String) -> Option<(u64, Command)> {
        let chat = self.selected.clone()?;
        if let Some(reason) = self.images.hosted_send_refusal(&chat) {
            self.error = Some(reason.into());
            return None;
        }
        if self.busy() || text.trim().is_empty() {
            return None;
        }
        let send = PendingSend {
            chat: chat.clone(),
            request: request.clone(),
            text: text.clone(),
        };
        self.send.insert(chat.clone(), send);
        self.error = None;
        Some(self.request(Command::Send {
            chat,
            request,
            text,
        }))
    }
    pub fn retry(&mut self) -> Option<(u64, Command)> {
        let chat = self.selected.clone()?;
        self.error = None;
        let default = if self.handoff_errors.contains_key(&chat) {
            Command::RunCoder { chat: chat.clone() }
        } else if self.states.contains_key(&chat) {
            Command::Retry { chat: chat.clone() }
        } else {
            Command::Create { chat: chat.clone() }
        };
        let command = self.send.get(&chat).map_or(default, |send| Command::Send {
            chat: send.chat.clone(),
            request: send.request.clone(),
            text: send.text.clone(),
        });
        Some(self.request(command))
    }
    pub fn run_coder(&mut self) -> Option<(u64, Command)> {
        let chat = self.selected.clone()?;
        if self.busy()
            || self.state()?.ready_computer.is_none()
            || self.state()?.coder.is_some()
            || self
                .pending
                .values()
                .any(|command| matches!(command, Command::RunCoder { chat: id } if id == &chat))
        {
            return None;
        }
        // The host's handoff starts Coder from the conversation's text; it
        // carries no images, so it never drops the draft's silently.
        if let Some(reason) = self.images.hosted_send_refusal(&chat) {
            self.error = Some(reason.into());
            return None;
        }
        self.error = None;
        Some(self.request(Command::RunCoder { chat }))
    }
    pub fn earlier(&mut self) -> Option<(u64, Command)> {
        let chat = self.selected.clone()?;
        let before = self.state()?.start;
        if before == 0 {
            return None;
        }
        self.reading = true;
        Some(self.request(Command::Read {
            chat,
            before: Some(before),
        }))
    }
    pub fn outcome(
        &mut self,
        ticket: u64,
        result: Result<Snapshot, String>,
    ) -> Vec<(String, String)> {
        let previous_error = self.error.clone();
        let mut accepted = vec![];
        let Some(command) = self.pending.remove(&ticket) else {
            return accepted;
        };
        match result {
            Err(error) => {
                if matches!(command, Command::ListMore { .. }) {
                    self.list_more = None;
                }
                let target = match &command {
                    Command::List {} | Command::ListMore { .. } => None,
                    Command::UseSuggestion { chat, .. }
                    | Command::RunCoder { chat }
                    | Command::BindCoder { chat, .. }
                    | Command::Create { chat }
                    | Command::Read { chat, .. }
                    | Command::Send { chat, .. }
                    | Command::Retry { chat }
                    | Command::Stop { chat }
                    | Command::Rename { chat, .. }
                    | Command::Pin { chat, .. }
                    | Command::Archive { chat }
                    | Command::Restore { chat } => Some(chat),
                };
                if target.is_some_and(|id| self.observed.get(id).is_some_and(|seen| *seen > ticket))
                {
                    return accepted;
                }
                if let Command::RunCoder { chat } = &command {
                    self.handoff_errors.insert(chat.clone(), error.clone());
                }
                if target.is_none() || target == self.selected.as_ref() {
                    self.error = Some(error);
                }
            }
            Ok(mut snapshot) => {
                self.listed = true;
                let earlier = matches!(
                    command,
                    Command::Read {
                        before: Some(_),
                        ..
                    }
                );
                let stale = snapshot
                    .chat
                    .as_ref()
                    .is_some_and(|id| self.observed.get(id).is_some_and(|seen| *seen > ticket));
                if stale && !earlier {
                    return accepted;
                }
                if !stale
                    && let Some(id) = &snapshot.chat
                    && snapshot.coder.is_some()
                {
                    self.handoff_errors.remove(id);
                }
                if !stale && (snapshot.chat.is_none() || snapshot.chat == self.selected) {
                    self.error = snapshot
                        .storage_error
                        .as_ref()
                        .map(|_| "Couldn't save chat. Check available disk space.".into())
                        .or_else(|| {
                            snapshot
                                .chat
                                .as_ref()
                                .and_then(|id| self.handoff_errors.get(id).cloned())
                        });
                }
                if ticket >= self.list_ticket {
                    if snapshot.list_start == 0 {
                        if snapshot.list_version != self.list_version || snapshot.list_total == 0 {
                            self.summaries = snapshot.chats.clone();
                        } else {
                            for summary in &snapshot.chats {
                                if let Some(old) =
                                    self.summaries.iter_mut().find(|s| s.id == summary.id)
                                {
                                    *old = summary.clone();
                                }
                            }
                        }
                        self.list_version = snapshot.list_version;
                    } else if snapshot.list_version == self.list_version
                        && snapshot.list_start == self.summaries.len()
                    {
                        self.summaries.extend(snapshot.chats.clone());
                    } else {
                        self.list_more = None;
                        return accepted;
                    }
                    if self.summaries.len() < snapshot.list_total {
                        self.list_more = Some(self.summaries.len());
                    }
                    self.list_ticket = ticket;
                }
                if let Some(id) = snapshot.chat.clone() {
                    if matches!(
                        command,
                        Command::Read {
                            before: Some(_),
                            ..
                        }
                    ) {
                        if let Some(previous) = self.states.get(&id)
                            && snapshot.start + snapshot.turns.len() == previous.start
                        {
                            snapshot.turns.extend(previous.turns.clone());
                            if stale {
                                snapshot.total = previous.total;
                                snapshot.busy = previous.busy;
                                snapshot.partial = previous.partial.clone();
                                snapshot.failure = previous.failure.clone();
                                snapshot.storage_error = previous.storage_error.clone();
                                snapshot.coder = previous.coder.clone();
                                snapshot.ready_computer = previous.ready_computer.clone();
                                snapshot.computer = previous.computer;
                            }
                        } else if stale {
                            return accepted;
                        }
                    } else if self.reading
                        && let Some(previous) = self.states.get(&id)
                        && previous.start <= snapshot.start
                        && previous.total <= snapshot.total
                    {
                        let count = snapshot.start - previous.start;
                        let mut earlier =
                            previous.turns[..count.min(previous.turns.len())].to_vec();
                        earlier.extend(snapshot.turns);
                        snapshot.turns = earlier;
                        snapshot.start = previous.start;
                    }
                    if let Some(send) = self.send.get(&id)
                        && snapshot.storage_error.is_none()
                        && snapshot
                            .turns
                            .iter()
                            .any(|turn| turn.request.as_deref() == Some(&send.request))
                    {
                        accepted.push((id.clone(), send.request.clone()));
                        self.send.remove(&id);
                    }
                    if self.selected.as_ref() == Some(&id)
                        && self.states.get(&id).is_none_or(|previous| {
                            previous.turns != snapshot.turns
                                || previous.start != snapshot.start
                                || previous.busy != snapshot.busy
                                || previous.partial != snapshot.partial
                                || previous.failure != snapshot.failure
                                || previous.storage_error != snapshot.storage_error
                                || previous.used != snapshot.used
                                || previous.computer != snapshot.computer
                                || previous.ready_computer != snapshot.ready_computer
                                || previous.coder != snapshot.coder
                        })
                    {
                        self.revision += 1;
                    }
                    self.observed
                        .entry(id.clone())
                        .and_modify(|seen| *seen = (*seen).max(ticket))
                        .or_insert(ticket);
                    self.states.insert(id, snapshot);
                }
                if self.error.is_none()
                    && matches!(&command, Command::Archive { chat } if self.selected.as_ref() == Some(chat))
                    && let Some(id) = self.selected.take()
                {
                    self.send.remove(&id);
                }
                if self.selected.is_none()
                    && let Some(id) = self
                        .summaries
                        .iter()
                        .find(|summary| !summary.archived)
                        .map(|summary| summary.id.clone())
                {
                    self.select(&id);
                }
            }
        }
        if self.error != previous_error {
            self.revision += 1;
        }
        accepted
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openagents_chat::basic_coder::Turn;

    fn page(chat: &str, start: usize, total: usize) -> Snapshot {
        Snapshot {
            chat: Some(chat.into()),
            start,
            total,
            turns: (start..total)
                .map(|i| Turn::user(format!("Message {i}")))
                .collect(),
            ..Snapshot::default()
        }
    }

    #[test]
    fn handoff_failure_retries_dispatch_and_binding_rebuilds_cards() {
        let mut session = Session::new(Instant::now());
        let id = "a".repeat(32);
        session.select(&id);
        let ready = Snapshot {
            chat: Some(id.clone()),
            ready_computer: Some("Studio Mac".into()),
            ..Default::default()
        };
        let (ticket, _) = session.request(Command::Read {
            chat: id.clone(),
            before: None,
        });
        session.outcome(ticket, Ok(ready.clone()));
        let revision = session.revision;
        let (ticket, _) = session.run_coder().unwrap();
        assert!(session.run_coder().is_none());
        session.outcome(ticket, Err("Connection lost".into()));
        let (ticket, _) = session.request(Command::Read {
            chat: id.clone(),
            before: None,
        });
        session.outcome(ticket, Ok(ready.clone()));
        assert_eq!(session.error.as_deref(), Some("Connection lost"));
        let (ticket, command) = session.retry().unwrap();
        assert_eq!(command, Command::RunCoder { chat: id });
        let bound = Snapshot {
            coder: Some(openagents_chat::basic_chats::Spawned {
                host: "b".repeat(64),
                task: "c".repeat(64),
                project: Some("openagents".into()),
                at: Some(10),
            }),
            ..ready
        };
        session.outcome(ticket, Ok(bound));
        assert!(session.error.is_none());
        assert!(session.revision > revision);
        assert!(session.run_coder().is_none());
        session
            .cards
            .rows(session.state().cloned().as_ref().unwrap());
        assert!(
            !session
                .cards
                .actions
                .values()
                .any(|action| *action == crate::cards::Action::RunCoder)
        );
    }

    #[test]
    fn a_large_catalog_loads_all_revision_bound_pages_and_refreshes_metadata() {
        let mut host = openagents_chat::basic_chats::BasicChats::new(None, None, None);
        for n in 0..512 {
            assert!(host.create(&format!("{n:032x}"), n));
        }
        let now = Instant::now();
        let mut session = Session::new(now);
        for step in 0..4 {
            let (ticket, command) = session.tick(now + Duration::from_secs(step)).unwrap();
            let result = openagents_chat::service::apply(&mut host, command, 600);
            session.outcome(ticket, result);
        }
        assert_eq!(session.summaries.len(), 512);
        assert_eq!(
            session
                .summaries
                .iter()
                .map(|s| &s.id)
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            512
        );
        let id = session.summaries[400].id.clone();
        host.rename(&id, "Phone and desktop see this title")
            .unwrap();
        host.pin(&id, true).unwrap();
        for step in 4..8 {
            let (ticket, command) = session.tick(now + Duration::from_secs(step)).unwrap();
            session.outcome(
                ticket,
                openagents_chat::service::apply(&mut host, command, 600),
            );
        }
        let row = session.summaries.iter().find(|s| s.id == id).unwrap();
        assert_eq!(row, host.get(&id).unwrap());
        assert_eq!(crate::chat_list::search(&session.summaries, "")[0].id, id);
    }

    #[test]
    fn image_drafts_cannot_be_silently_dropped_by_a_text_send() {
        let mut session = Session::new(Instant::now());
        session.select("a");
        let image = crate::attachments::Image::pixels(1, 1, vec![0; 4]).unwrap();
        let id = image.id.clone();
        session.images.add("a", image).unwrap();
        assert!(session.submit("one".into(), "Draft text".into()).is_none());
        assert!(session.error.as_deref().unwrap().contains("text only"));
        assert!(session.send.is_empty());
        assert_eq!(session.images.get("a").len(), 1);
        session.select("b");
        assert!(session.submit("two".into(), "Other chat".into()).is_some());
        session.select("a");
        session.images.remove("a", &id);
        assert!(
            session
                .submit("three".into(), "Draft text".into())
                .is_some()
        );
    }
    #[test]
    fn late_acknowledgment_and_archive_stay_bound_to_their_conversation() {
        let mut state = Session::new(Instant::now());
        state.select("a");
        let (send, original) = state
            .submit("request".into(), "Keep these bytes  ".into())
            .unwrap();
        state.outcome(send, Err("Connection lost".into()));
        let (retry, repeated) = state.retry().unwrap();
        assert_eq!(repeated, original);
        let (archive, _) = state.request(Command::Archive { chat: "a".into() });
        state.select("b");
        let mut snapshot = page("a", 0, 1);
        snapshot.turns[0].request = Some("request".into());
        assert_eq!(
            state.outcome(retry, Ok(snapshot)),
            vec![("a".into(), "request".into())]
        );
        state.outcome(archive, Ok(Snapshot::default()));
        assert_eq!(state.selected.as_deref(), Some("b"));
        assert!(state.error.is_none());
    }

    #[test]
    fn a_failed_write_keeps_the_send_until_a_durable_read_acknowledges_it() {
        let mut state = Session::new(Instant::now());
        state.select("a");
        let (send, _) = state.submit("request".into(), "Hello".into()).unwrap();
        let mut snapshot = page("a", 0, 1);
        snapshot.turns[0].request = Some("request".into());
        snapshot.storage_error = Some("Disk full".into());
        assert!(state.outcome(send, Ok(snapshot.clone())).is_empty());
        assert!(state.busy());
        assert!(
            state
                .submit("another".into(), "Don't duplicate".into())
                .is_none()
        );
        let (read, _) = state.request(Command::Read {
            chat: "a".into(),
            before: None,
        });
        snapshot.storage_error = None;
        assert_eq!(
            state.outcome(read, Ok(snapshot)),
            vec![("a".into(), "request".into())]
        );
        assert!(!state.busy());
    }

    #[test]
    fn earlier_pages_survive_new_streaming_observations() {
        let mut state = Session::new(Instant::now());
        state.select("a");
        let (read, _) = state.request(Command::Read {
            chat: "a".into(),
            before: None,
        });
        state.outcome(read, Ok(page("a", 16, 32)));
        let (earlier, _) = state.earlier().unwrap();
        let mut old = page("a", 0, 16);
        old.total = 32;
        state.outcome(earlier, Ok(old));
        assert_eq!(state.state().unwrap().turns.len(), 32);
        let (read, _) = state.request(Command::Read {
            chat: "a".into(),
            before: None,
        });
        state.outcome(read, Ok(page("a", 17, 33)));
        let current = state.state().unwrap();
        assert_eq!(current.start, 0);
        assert_eq!(current.turns.len(), 33);
        assert_eq!(current.turns[0].text, "Message 0");
        assert_eq!(current.turns.last().unwrap().text, "Message 32");
    }

    #[test]
    fn idle_ticks_neither_overlap_reads_nor_schedule_a_past_wake() {
        let now = Instant::now();
        let mut state = Session::new(now);
        let (list, _) = state.tick(now).unwrap();
        assert!(state.tick(now + Duration::from_secs(2)).is_none());
        state.outcome(list, Ok(Snapshot::default()));
        assert!(matches!(
            state.tick(now + Duration::from_secs(2)).unwrap().1,
            Command::Create { .. }
        ));
        assert!(state.next_wake(now + Duration::from_secs(5)) > now + Duration::from_secs(5));
    }
}

#[cfg(test)]
mod response_order_tests {
    use super::*;
    use openagents_chat::basic_coder::Turn;
    #[test]
    fn an_old_read_cannot_replace_a_newer_send_or_its_error() {
        let mut state = Session::new(Instant::now());
        state.select("a");
        let (read, _) = state.request(Command::Read {
            chat: "a".into(),
            before: None,
        });
        let (send, _) = state.submit("request".into(), "Hello".into()).unwrap();
        let mut user = Turn::user("Hello");
        user.request = Some("request".into());
        state.outcome(
            send,
            Ok(Snapshot {
                chat: Some("a".into()),
                turns: vec![user],
                total: 1,
                busy: true,
                ..Snapshot::default()
            }),
        );
        state.outcome(
            read,
            Ok(Snapshot {
                chat: Some("a".into()),
                storage_error: Some("Old error".into()),
                ..Snapshot::default()
            }),
        );
        assert_eq!(state.state().unwrap().turns.len(), 1);
        assert!(state.busy());
        assert!(state.error.is_none());
    }
}
