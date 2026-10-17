//! A chat answered by OpenAgents, with or without an account (#11978,
//! #11979): the phone's first screen.
//!
//! Signed out, the chat belongs to a visitor (`openagents::visitor`): a
//! server-signed token kept in a small file beside the account data, with a
//! few messages before the server asks for a sign-in. Signed in, it is a new
//! account chat (`openagents::chats`). After a sign-in, [`CloudChat::claim`]
//! moves the visitor's chats to the account; the open chat keeps its id, so
//! the conversation carries on as the account's.
use crate::Controller;
use openagents::{
    CancellationToken, Client, Error, MessageRole,
    chats::{self, Event, Stream},
    visitor::{self, Message, Visitor},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    future::Future,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use tokio::sync::watch;

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
    /// Signed out: messages left before a sign-in is needed.
    pub left: Option<u32>,
    /// Signed out and out of messages: the app asks the person to sign in.
    pub sign_in_required: bool,
    /// Text that wasn't sent, to put back in the composer.
    pub unsent: Option<String>,
    pub error: Option<String>,
    /// The open chat is the account's (signed in, or claimed after sign-in).
    pub account_chat: bool,
}

#[derive(Default, Serialize, Deserialize)]
struct Saved {
    token: String,
    #[serde(default)]
    chat: Option<String>,
    #[serde(default)]
    left: Option<u32>,
}

/// Where the open chat lives.
#[derive(Clone)]
enum Home {
    Visitor(Visitor),
    Account,
}

struct State {
    generation: u64,
    cancel: CancellationToken,
    home: Option<Home>,
    snapshot: Snapshot,
    /// The server's message index of each line, for the answer being written.
    slots: BTreeMap<usize, usize>,
    /// The request being answered, to stop it.
    answering: Option<(String, String)>,
}

pub struct CloudChat {
    controller: Arc<Controller>,
    path: PathBuf,
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

fn read(path: &Path) -> Option<Saved> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

fn write(path: &Path, saved: &Saved) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let Ok(bytes) = serde_json::to_vec(saved) else {
        return;
    };
    let partial = path.with_extension("partial");
    if std::fs::write(&partial, bytes).is_ok() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&partial, std::fs::Permissions::from_mode(0o600));
        }
        let _ = std::fs::rename(&partial, path);
    }
}

/// What a person reads for a failed send.
fn says(error: &Error) -> String {
    match error {
        Error::Refused {
            code: Some("sign_in_required"),
            ..
        } => "Sign in to keep chatting. It's free.".into(),
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
    /// `dir` holds the visitor file (one per deployment).
    pub fn new(controller: Arc<Controller>, dir: &Path) -> Arc<Self> {
        let deployment = controller.client().deployment();
        let name = format!(
            "visitor-{:x}.json",
            <sha2::Sha256 as sha2::Digest>::digest(
                format!("{}\n{}", deployment.api_base(), deployment.website()).as_bytes()
            )
        );
        let (updates, _) = watch::channel(Snapshot::default());
        Arc::new(Self {
            controller,
            path: dir.join(name),
            state: Mutex::new(State {
                generation: 0,
                cancel: CancellationToken::new(),
                home: None,
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
    fn client(&self) -> &Client {
        self.controller.client()
    }
    fn visitor(&self) -> Option<(Visitor, Saved)> {
        let saved = read(&self.path)?;
        let visitor =
            Visitor::restore(self.client().deployment().clone(), saved.token.clone()).ok()?;
        Some((visitor, saved))
    }

    /// Signed out: reopen the visitor's last chat, if there is one.
    pub fn resume(self: &Arc<Self>) -> impl Future<Output = ()> + Send + 'static {
        let this = self.clone();
        async move {
            if this.controller.lease().is_some() {
                return;
            }
            let Some((visitor, saved)) = this.visitor() else {
                return;
            };
            let Some(chat) = saved.chat.clone() else {
                let mut s = this.state.lock().unwrap();
                s.snapshot.left = saved.left;
                this.publish(&s);
                return;
            };
            let generation = this.state.lock().unwrap().generation;
            let cancel = CancellationToken::new();
            let lines = visitor::messages(this.client(), &visitor, &chat, &cancel).await;
            let mut s = this.state.lock().unwrap();
            if s.generation != generation || s.snapshot.chat.is_some() {
                return;
            }
            if let Ok(lines) = lines {
                s.snapshot.lines = lines
                    .into_iter()
                    .filter(|l| !l.text.trim().is_empty())
                    .map(|l| Line {
                        role: role(&l.role),
                        text: l.text,
                    })
                    .collect();
                s.snapshot.chat = Some(chat);
                s.home = Some(Home::Visitor(visitor));
            }
            s.snapshot.left = saved.left;
            s.snapshot.sign_in_required = saved.left == Some(0);
            this.publish(&s);
        }
    }

    /// Start over with an empty chat.
    pub fn new_chat(&self) {
        let mut s = self.state.lock().unwrap();
        s.cancel.cancel();
        s.generation += 1;
        s.cancel = CancellationToken::new();
        s.home = None;
        s.slots.clear();
        s.answering = None;
        let left = s.snapshot.left;
        let sign_in_required = s.snapshot.sign_in_required && self.controller.lease().is_none();
        s.snapshot = Snapshot {
            left,
            sign_in_required,
            ..Snapshot::default()
        };
        if let Some((_, mut saved)) = self.visitor() {
            saved.chat = None;
            write(&self.path, &saved);
        }
        self.publish(&s);
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
        let (generation, cancel, home, chat) = {
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
            (
                s.generation,
                s.cancel.clone(),
                s.home.clone(),
                s.snapshot.chat.clone(),
            )
        };
        let signed_in = self.controller.lease();
        let home = match (home, &signed_in) {
            (Some(home), _) => home,
            (None, Some(_)) => Home::Account,
            (None, None) => match self.visitor() {
                Some((visitor, _)) => Home::Visitor(visitor),
                None => match visitor::begin(self.client(), &cancel).await {
                    Ok((visitor, left)) => {
                        write(
                            &self.path,
                            &Saved {
                                token: visitor.token().to_owned(),
                                chat: None,
                                left: Some(left),
                            },
                        );
                        Home::Visitor(visitor)
                    }
                    Err(e) => return self.failed(generation, &text, &e),
                },
            },
        };
        match home {
            Home::Visitor(visitor) => {
                let message = match chat {
                    Some(chat) => Message::reply(chat, text.clone()),
                    None => Message::first(text.clone()),
                };
                let message = match message {
                    Ok(m) => m,
                    Err(e) => return self.failed(generation, &text, &e),
                };
                let left = match visitor::send(self.client(), &visitor, &message, &cancel).await {
                    Ok(left) => left,
                    Err(e) => return self.failed(generation, &text, &e),
                };
                write(
                    &self.path,
                    &Saved {
                        token: visitor.token().to_owned(),
                        chat: Some(message.chat().to_owned()),
                        left: Some(left),
                    },
                );
                {
                    let mut s = self.state.lock().unwrap();
                    if s.generation != generation {
                        return;
                    }
                    s.home = Some(Home::Visitor(visitor.clone()));
                    s.snapshot.chat = Some(message.chat().to_owned());
                    s.snapshot.left = Some(left);
                    s.answering =
                        Some((message.chat().to_owned(), message.request_id().to_owned()));
                    self.publish(&s);
                }
                let stream = visitor::stream(self.client(), &visitor, &message, &cancel).await;
                self.follow(generation, stream).await;
            }
            Home::Account => {
                let Some(lease) = signed_in else {
                    return self.failed(generation, &text, &Error::Unauthorized);
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
                    Err(e) => return self.failed(generation, &text, &e),
                };
                let sent = if chat.is_some() {
                    chats::send(client, session, &message, &cancel).await
                } else {
                    chats::start(client, session, &message, &cancel).await
                };
                if let Err(e) = sent {
                    return self.failed(generation, &text, &e);
                }
                {
                    let mut s = self.state.lock().unwrap();
                    if s.generation != generation {
                        return;
                    }
                    s.home = Some(Home::Account);
                    s.snapshot.chat = Some(message.chat().id().to_owned());
                    s.snapshot.account_chat = true;
                    s.snapshot.left = None;
                    s.answering = Some((
                        message.chat().id().to_owned(),
                        message.request_id().to_owned(),
                    ));
                    self.publish(&s);
                }
                let stream = chats::stream(client, session, &message.turn(), &cancel).await;
                self.follow(generation, stream).await;
            }
        }
    }

    fn failed(&self, generation: u64, text: &str, error: &Error) {
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
        if matches!(
            error,
            Error::Refused {
                code: Some("sign_in_required"),
                ..
            }
        ) {
            s.snapshot.sign_in_required = true;
            s.snapshot.left = Some(0);
            if let Some((_, mut saved)) = self.visitor() {
                saved.left = Some(0);
                write(&self.path, &saved);
            }
        } else {
            s.snapshot.error = Some(says(error));
        }
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
            let (home, answering) = {
                let s = this.state.lock().unwrap();
                (s.home.clone(), s.answering.clone())
            };
            let (Some(home), Some((chat, request))) = (home, answering) else {
                return;
            };
            let cancel = CancellationToken::new();
            let stopped = match home {
                Home::Visitor(visitor) => {
                    let Ok(message) = Message::reply(chat.clone(), "stop".into()) else {
                        return;
                    };
                    let message = message.with_request(request);
                    visitor::stop(this.client(), &visitor, &message, &cancel)
                        .await
                        .is_ok()
                }
                Home::Account => {
                    let Some(lease) = this.controller.lease() else {
                        return;
                    };
                    let turn = lease
                        .client()
                        .chat(lease.session(), &chat)
                        .and_then(|c| chats::Turn::new(c, request));
                    match turn {
                        Ok(turn) => chats::stop(lease.client(), lease.session(), &turn, &cancel)
                            .await
                            .is_ok(),
                        Err(_) => false,
                    }
                }
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

    /// Signed in: move the visitor's chats to the account, and keep the open
    /// chat going as the account's. Nothing happens without a visitor.
    pub fn claim(self: &Arc<Self>) -> impl Future<Output = ()> + Send + 'static {
        let this = self.clone();
        async move {
            let Some(lease) = this.controller.lease() else {
                return;
            };
            let Some((visitor, _)) = this.visitor() else {
                return;
            };
            let cancel = CancellationToken::new();
            match visitor::claim(lease.client(), lease.session(), &visitor, &cancel).await {
                Ok(_) => {
                    let _ = std::fs::remove_file(&this.path);
                    let mut s = this.state.lock().unwrap();
                    if matches!(s.home, Some(Home::Visitor(_))) {
                        s.home = Some(Home::Account);
                        s.snapshot.account_chat = true;
                    }
                    s.snapshot.left = None;
                    s.snapshot.sign_in_required = false;
                    this.publish(&s);
                }
                // A visitor the server no longer knows: start fresh.
                Err(Error::Refused { status: 400, .. }) | Err(Error::Unauthorized) => {
                    let _ = std::fs::remove_file(&this.path);
                }
                Err(_) => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_visitor_file_round_trips_and_ignores_damage() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("v.json");
        assert!(read(&path).is_none());
        write(
            &path,
            &Saved {
                token: "ovis_abc".into(),
                chat: Some("c".into()),
                left: Some(3),
            },
        );
        let saved = read(&path).unwrap();
        assert_eq!(
            (saved.token.as_str(), saved.chat.as_deref(), saved.left),
            ("ovis_abc", Some("c"), Some(3))
        );
        std::fs::write(&path, b"{").unwrap();
        assert!(read(&path).is_none());
    }

    #[test]
    fn refusals_read_as_one_plain_line() {
        let signin = Error::Refused {
            status: 409,
            code: Some("sign_in_required"),
        };
        assert_eq!(says(&signin), "Sign in to keep chatting. It's free.");
        assert!(says(&Error::RateLimited { retry_after: None }).contains("Try again"));
    }
}
