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
    pub selected: Option<String>,
    pub states: BTreeMap<String, Snapshot>,
    pub summaries: Vec<Summary>,
    pub send: BTreeMap<String, PendingSend>,
    pub error: Option<String>,
    pub revision: u64,
    pub reading: bool,
    pending: BTreeMap<u64, Command>,
    next_ticket: u64,
    poll: Instant,
    listed: bool,
    observed: BTreeMap<String, u64>,
    list_ticket: u64,
}

impl Session {
    pub fn new(now: Instant) -> Self {
        Self {
            cards: crate::cards::Cards::default(),
            selected: None,
            states: BTreeMap::new(),
            summaries: vec![],
            send: BTreeMap::new(),
            error: None,
            revision: 0,
            reading: false,
            pending: BTreeMap::new(),
            next_ticket: 1,
            poll: now,
            listed: false,
            observed: BTreeMap::new(),
            list_ticket: 0,
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
            || self
                .pending
                .values()
                .any(|command| matches!(command, Command::List { .. } | Command::Read { .. }))
        {
            return None;
        }
        self.poll = now + Duration::from_millis(if self.busy() { 100 } else { 1000 });
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
        let default = if self.states.contains_key(&chat) {
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
                let target = match &command {
                    Command::List {} => None,
                    Command::UseSuggestion { chat, .. }
                    | Command::Create { chat }
                    | Command::Read { chat, .. }
                    | Command::Send { chat, .. }
                    | Command::Retry { chat }
                    | Command::Stop { chat }
                    | Command::Archive { chat }
                    | Command::Restore { chat } => Some(chat),
                };
                if target.is_some_and(|id| self.observed.get(id).is_some_and(|seen| *seen > ticket))
                {
                    return accepted;
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
                if !stale && (snapshot.chat.is_none() || snapshot.chat == self.selected) {
                    self.error = snapshot
                        .storage_error
                        .as_ref()
                        .map(|_| "Couldn't save chat. Check available disk space.".into());
                }
                if ticket >= self.list_ticket {
                    self.summaries = snapshot.chats.clone();
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
