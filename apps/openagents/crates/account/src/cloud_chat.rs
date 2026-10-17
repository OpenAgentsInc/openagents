//! A chat answered by OpenAgents (#11978, #11979): the phone's first
//! screen. It is always an account chat (`openagents::chats`): every phone
//! has its own account from first launch (#11995), so the visitor chat and
//! its claim are retired here (the server's visitor routes keep answering
//! older builds).
use crate::{Controller, Lease};
use openagents::{
    CancellationToken, Error, MessageRole,
    chats::{self, Event, Stream},
};
use std::{
    collections::BTreeMap,
    future::Future,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::watch;

/// How long a first send waits for the phone's silent sign-in.
const SIGN_IN_WAIT: Duration = Duration::from_secs(20);
/// What a send says when the account isn't ready yet.
const NOT_READY: &str = "We're still setting up your account. Try again in a moment.";

/// Who a line is from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
    Tool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    pub role: Role,
    pub text: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    /// The open chat's id, once its first message was sent.
    pub chat: Option<String>,
    pub lines: Vec<Line>,
    /// An answer is being written.
    pub answering: bool,
    /// Text that wasn't sent, to put back in the composer.
    pub unsent: Option<String>,
    pub error: Option<String>,
    /// The open chat is the account's. Always so once a message was sent.
    pub account_chat: bool,
}

struct State {
    generation: u64,
    cancel: CancellationToken,
    snapshot: Snapshot,
    /// The server's message index of each line, for the answer being written.
    slots: BTreeMap<usize, usize>,
    /// The request being answered, to stop it.
    answering: Option<(String, String)>,
}

pub struct CloudChat {
    controller: Arc<Controller>,
    state: Mutex<State>,
    updates: watch::Sender<Snapshot>,
}

fn role(role: &MessageRole) -> Role {
    match role {
        MessageRole::User => Role::User,
        MessageRole::Tool => Role::Tool,
        _ => Role::Assistant,
    }
}

/// What a person reads for a failed send.
fn says(error: &Error) -> String {
    match error {
        Error::Refused {
            code: Some("secret"),
            ..
        } => "This looks like it holds a password or key, so it wasn't sent.".into(),
        Error::Refused {
            code: Some("busy"), ..
        } => "OpenAgents is still answering. Wait for it, or stop it.".into(),
        Error::Refused {
            code: Some("quota_exhausted"),
            ..
        } => "You've used today's free messages. Add credit on openagents.com, or try again tomorrow.".into(),
        Error::RateLimited { .. } => "Too many messages right now. Try again in a minute.".into(),
        other => other.to_string(),
    }
}

impl CloudChat {
    pub fn new(controller: Arc<Controller>) -> Arc<Self> {
        let (updates, _) = watch::channel(Snapshot::default());
        Arc::new(Self {
            controller,
            state: Mutex::new(State {
                generation: 0,
                cancel: CancellationToken::new(),
                snapshot: Snapshot::default(),
                slots: BTreeMap::new(),
                answering: None,
            }),
            updates,
        })
    }
    pub fn watch(&self) -> watch::Receiver<Snapshot> {
        self.updates.subscribe()
    }
    pub fn snapshot(&self) -> Snapshot {
        self.state.lock().unwrap().snapshot.clone()
    }
    fn publish(&self, s: &State) {
        self.updates.send_replace(s.snapshot.clone());
    }

    /// Start over with an empty chat.
    pub fn new_chat(&self) {
        let mut s = self.state.lock().unwrap();
        s.cancel.cancel();
        s.generation += 1;
        s.cancel = CancellationToken::new();
        s.slots.clear();
        s.answering = None;
        s.snapshot = Snapshot::default();
        self.publish(&s);
    }

    /// The account's lease, waiting a little while the phone's silent
    /// sign-in finishes, as on a first launch.
    async fn lease(&self, cancel: &CancellationToken) -> Option<Lease> {
        if let Some(lease) = self.controller.lease() {
            return Some(lease);
        }
        let mut watch = self.controller.watch();
        let wait = async {
            loop {
                if let Some(lease) = self.controller.lease() {
                    return Some(lease);
                }
                if watch.changed().await.is_err() {
                    return None;
                }
            }
        };
        tokio::select! {
            biased;
            _ = cancel.cancelled() => None,
            lease = tokio::time::timeout(SIGN_IN_WAIT, wait) => lease.ok().flatten(),
        }
    }

    /// Send `text` in the open chat, or start one.
    pub fn send(self: &Arc<Self>, text: String) -> impl Future<Output = ()> + Send + 'static {
        let this = self.clone();
        async move { this.send_inner(text).await }
    }

    async fn send_inner(self: Arc<Self>, text: String) {
        let text = text.trim().to_owned();
        if text.is_empty() {
            return;
        }
        let (generation, cancel, chat) = {
            let mut s = self.state.lock().unwrap();
            if s.snapshot.answering {
                return;
            }
            if text.chars().count() > 4000 {
                s.snapshot.error = Some("Write a message of up to 4,000 characters.".into());
                self.publish(&s);
                return;
            }
            s.generation += 1;
            s.cancel = CancellationToken::new();
            s.snapshot.answering = true;
            s.snapshot.error = None;
            s.snapshot.unsent = None;
            s.snapshot.lines.push(Line {
                role: Role::User,
                text: text.clone(),
            });
            s.slots.clear();
            self.publish(&s);
            (s.generation, s.cancel.clone(), s.snapshot.chat.clone())
        };
        let Some(lease) = self.lease(&cancel).await else {
            return self.failed(generation, &text, NOT_READY);
        };
        let (client, session) = (lease.client(), lease.session());
        let message = match &chat {
            Some(chat) => client
                .chat(session, chat)
                .and_then(|c| chats::Send::new(c, text.clone())),
            None => chats::Send::first(client, session, text.clone()),
        };
        let message = match message {
            Ok(m) => m,
            Err(e) => return self.failed(generation, &text, &says(&e)),
        };
        let sent = if chat.is_some() {
            chats::send(client, session, &message, &cancel).await
        } else {
            chats::start(client, session, &message, &cancel).await
        };
        if let Err(e) = sent {
            return self.failed(generation, &text, &says(&e));
        }
        {
            let mut s = self.state.lock().unwrap();
            if s.generation != generation {
                return;
            }
            s.snapshot.chat = Some(message.chat().id().to_owned());
            s.snapshot.account_chat = true;
            s.answering = Some((
                message.chat().id().to_owned(),
                message.request_id().to_owned(),
            ));
            self.publish(&s);
        }
        let stream = chats::stream(client, session, &message.turn(), &cancel).await;
        self.follow(generation, stream).await;
    }

    fn failed(&self, generation: u64, text: &str, error: &str) {
        let mut s = self.state.lock().unwrap();
        if s.generation != generation {
            return;
        }
        s.snapshot.answering = false;
        s.answering = None;
        if s.snapshot
            .lines
            .last()
            .is_some_and(|l| l.role == Role::User && l.text == text)
        {
            s.snapshot.lines.pop();
        }
        s.snapshot.unsent = Some(text.to_owned());
        s.snapshot.error = Some(error.to_owned());
        self.publish(&s);
    }

    async fn follow(&self, generation: u64, stream: Result<Stream, Error>) {
        let mut stream = match stream {
            Ok(stream) => stream,
            Err(e) => return self.ended(generation, Some(says(&e))),
        };
        loop {
            match stream.next().await {
                Ok(Some(event)) => {
                    if !self.apply(generation, event) {
                        return;
                    }
                }
                Ok(None) => return self.ended(generation, None),
                Err(Error::Cancelled) => return,
                Err(e) => return self.ended(generation, Some(says(&e))),
            }
        }
    }

    /// Apply one event; `false` once the answer has ended or was replaced.
    fn apply(&self, generation: u64, event: Event) -> bool {
        let mut s = self.state.lock().unwrap();
        if s.generation != generation {
            return false;
        }
        let more = match event {
            Event::Text {
                index,
                role: r,
                text,
                replace,
                ..
            }
            | Event::Tool {
                index,
                role: r,
                text,
                replace,
                ..
            } => {
                let r = role(&r);
                if r != Role::User {
                    let slot = match s.slots.get(&index) {
                        Some(slot) => *slot,
                        None => {
                            s.snapshot.lines.push(Line {
                                role: r,
                                text: String::new(),
                            });
                            let slot = s.snapshot.lines.len() - 1;
                            s.slots.insert(index, slot);
                            slot
                        }
                    };
                    let line = &mut s.snapshot.lines[slot];
                    if replace {
                        line.text = text;
                    } else {
                        line.text.push_str(&text);
                    }
                }
                true
            }
            Event::Cards { .. } => true,
            Event::Done { .. } => {
                s.snapshot.answering = false;
                s.answering = None;
                false
            }
            Event::Error { error, .. } => {
                s.snapshot.answering = false;
                s.answering = None;
                s.snapshot.error = Some(error.message);
                false
            }
        };
        self.publish(&s);
        more
    }

    fn ended(&self, generation: u64, error: Option<String>) {
        let mut s = self.state.lock().unwrap();
        if s.generation != generation {
            return;
        }
        s.snapshot.answering = false;
        s.answering = None;
        if error.is_some() {
            s.snapshot.error = error;
        }
        self.publish(&s);
    }

    /// Stop the answer being written.
    pub fn stop(self: &Arc<Self>) -> impl Future<Output = ()> + Send + 'static {
        let this = self.clone();
        async move {
            let Some((chat, request)) = this.state.lock().unwrap().answering.clone() else {
                return;
            };
            let Some(lease) = this.controller.lease() else {
                return;
            };
            let cancel = CancellationToken::new();
            let turn = lease
                .client()
                .chat(lease.session(), &chat)
                .and_then(|c| chats::Turn::new(c, request));
            let stopped = match turn {
                Ok(turn) => chats::stop(lease.client(), lease.session(), &turn, &cancel)
                    .await
                    .is_ok(),
                Err(_) => false,
            };
            if stopped {
                let mut s = this.state.lock().unwrap();
                s.cancel.cancel();
                s.generation += 1;
                s.snapshot.answering = false;
                s.answering = None;
                this.publish(&s);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refusals_read_as_one_plain_line() {
        let secret = Error::Refused {
            status: 400,
            code: Some("secret"),
        };
        assert!(says(&secret).contains("wasn't sent"));
        assert!(says(&Error::RateLimited { retry_after: None }).contains("Try again"));
        assert!(oa_copy::violations(NOT_READY, &[]).is_empty());
        assert!(!NOT_READY.to_lowercase().contains("sign in"));
    }
}
