//! Coder chats synced to the account (#11046): a small API that a signed-in
//! `coder-new` calls with its own app token (`docs/auth/README.md`, "Device
//! sign-in"), and the store rules behind it.
//!
//! | Route | What |
//! | --- | --- |
//! | `PUT /coder/sessions/{session}` `{computer, title, messages}` | Save the chat's messages. `200 {chat, changed}`; `410 deleted` when it was deleted on the website; `413 full` or `too_large`; `422 secret` |
//! | `POST /coder/sessions/{session}/status` `{working}` | Coder is replying (or not). `200`, `404 unknown`, `410 deleted` |
//! | `DELETE /coder/sessions/{session}` | Deleted in Coder: remove it here. `200 {deleted}` |
//! | `GET /coder/sessions` | `{sessions: [{session, deleted}]}`, so Coder learns of chats deleted on the website |
//! | `POST /coder/check-in` `{computer}` | Coder runs on this computer with sync on (#11048). `200 {waiting: [session], sync}`: its chats with replies from the website, or with messages from a Cloud computer it hasn't taken, and the computer's sync choice |
//! | `GET /coder/sync?computer=` | The computer's sync choice (#11089): `200 {choice: "all" \| "local" \| null}` (null: not asked yet) |
//! | `PUT /coder/sync` `{computer, choice}` | Coder's answer to "Sync all my chats?" for this computer. `200 {choice}` |
//! | `POST /coder/sessions/{session}/replies` | Take what waits: the replies sent on the website join the transcript as the person's messages and the chat shows Working; `continued` are the messages added while the computer was offline (#11050), oldest first, for Coder to add to its own copy before answering the replies (#11052). Each is handed out once. `200 {replies: [{id, text}], continued: [{role, text}]}`, `404 unknown`, `410 deleted` |
//!
//! Every route takes `Authorization: Bearer sess_…`. A synced chat is an
//! ordinary account chat ([`crate::chat_store::account_owner`]) with a
//! [`Terminal`] record, so it lists, pins, renames, archives, and searches
//! like a web chat. On the web it is read-only unless Coder on its computer
//! checked in within [`ONLINE_SECONDS`] (#11048): then a reply typed there
//! waits on the chat ([`queue_reply`]) until Coder takes it
//! ([`take_replies`]), runs the turn, and uploads the answer as usual. A
//! reply is taken once: if Coder never hears back from its take, the reply
//! shows in the chat but isn't run. Messages are screened for
//! credential shapes (`secret_screen`) here too; Coder screens them before
//! sending.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use axum::Router;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::App;
use crate::chat_store::{
    Computers, Conversation, Error, MAX_COMPUTERS, MAX_REPLY_IDS, MAX_WAITING_CHATS,
    MAX_WEB_REPLIES, Message, Role, Store, SyncChoice, Terminal, WebReply, account_owner, now_unix,
};
use crate::cloud::protect;
use crate::cloud::session::SessionError;

/// The largest upload a chat may send.
pub(crate) const MAX_BODY: usize = 8 * 1024 * 1024;
/// The most messages one synced chat keeps.
pub(crate) const MAX_MESSAGES: usize = 4000;
/// The longest one message may be.
pub(crate) const MAX_TEXT: usize = 256 * 1024;
/// The most chats (web and Coder) an account may have before new Coder
/// chats are refused; the store's own list limit is 256.
pub(crate) const MAX_CHATS: usize = 200;

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/coder/sessions", get(list))
        .route("/coder/sessions/{session}", put(upload).delete(forget))
        .route("/coder/sessions/{session}/status", post(status))
        .route("/coder/sessions/{session}/replies", post(replies))
        .route("/coder/check-in", post(check_in_route))
        .route("/coder/sync", get(choice_route).put(choose_route))
        .layer(DefaultBodyLimit::max(MAX_BODY))
}

/// What Coder sends for one chat.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Upload {
    pub computer: String,
    pub title: String,
    pub messages: Vec<WireMessage>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WireMessage {
    pub role: String,
    pub text: String,
}

/// What became of an upload or a status.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Saved {
    Saved {
        chat: String,
        changed: bool,
    },
    /// Deleted on the website: Coder deletes its copy and stops.
    Deleted,
    /// No chat with that session (a status before the first upload).
    Unknown,
    /// The account has [`MAX_CHATS`] chats.
    Full,
    /// A message looks like it holds a credential.
    Secret,
    Invalid(&'static str),
}

/// The web chat id for a Coder session: a version 4 UUID shape derived
/// from the owner and the session, so the same session always lands on the
/// same chat and two accounts never share one.
pub(crate) fn chat_id(owner: &str, session: &str) -> String {
    let mut hash = Sha256::new();
    hash.update(b"openagents.web.coder.chat.v1\0");
    hash.update(owner.as_bytes());
    hash.update(b"\0");
    hash.update(session.as_bytes());
    let digest = hash.finalize();
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    )
}

/// Coder's session ids: letters, numbers, `_`, and `-`, up to 128 bytes
/// (`coder-new` `sessions::validate_id`).
pub(crate) fn valid_session(session: &str) -> bool {
    !session.is_empty()
        && session.len() <= 128
        && session
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

/// One line of plain text, at most `limit` characters.
pub(crate) fn line(value: &str, limit: usize) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .filter(|c| !c.is_control())
        .take(limit)
        .collect()
}

/// The checked chat: title, computer, messages, and the upload's digest.
fn checked(upload: &Upload) -> Result<(String, String, Vec<Message>, String), Saved> {
    let computer = line(&upload.computer, 64);
    if computer.is_empty() {
        return Err(Saved::Invalid("Send the computer's name."));
    }
    let mut title = line(&upload.title, 120);
    if title.is_empty() {
        title = "Coder chat".into();
    }
    if upload.messages.len() > MAX_MESSAGES {
        return Err(Saved::Invalid("too_large"));
    }
    let mut messages = Vec::with_capacity(upload.messages.len());
    for message in &upload.messages {
        let role = match message.role.as_str() {
            "user" => Role::User,
            "assistant" => Role::Assistant,
            "tool" => Role::Tool,
            _ => return Err(Saved::Invalid("A message has an unknown role.")),
        };
        if message.text.len() > MAX_TEXT {
            return Err(Saved::Invalid("too_large"));
        }
        if message.text.trim().is_empty() {
            continue;
        }
        if secret_screen::credential_in(&message.text).is_some() {
            return Err(Saved::Secret);
        }
        messages.push(Message {
            role,
            text: message.text.clone(),
            request_id: None,
        });
    }
    if secret_screen::credential_in(&title).is_some() {
        return Err(Saved::Secret);
    }
    let digest = format!(
        "{:x}",
        Sha256::digest(
            serde_json::to_vec(&json!({
                "computer": computer,
                "title": title,
                "messages": messages
                    .iter()
                    .map(|m| json!([m.role, m.text]))
                    .collect::<Vec<_>>(),
            }))
            .unwrap_or_default()
        )
    );
    Ok((title, computer, messages, digest))
}

/// Save one Coder chat for `owner`.
pub(crate) async fn save(
    store: &Store,
    owner: &str,
    session: &str,
    upload: &Upload,
) -> Result<Saved, Error> {
    if !valid_session(session) {
        return Ok(Saved::Invalid("That isn't a Coder session id."));
    }
    let (title, computer, messages, digest) = match checked(upload) {
        Ok(checked) => checked,
        Err(saved) => return Ok(saved),
    };
    let id = chat_id(owner, session);
    for _ in 0..4 {
        let Some(loaded) = store.load(owner, &id).await? else {
            if store.list_with_deleted(owner).await?.len() >= MAX_CHATS {
                return Ok(Saved::Full);
            }
            let chat = Conversation {
                id: id.clone(),
                owner: owner.to_owned(),
                revision: 1,
                title: title.clone(),
                messages: messages.clone(),
                pending: None,
                requests: Vec::new(),
                selection: None,
                updated_unix: now_unix(),
                pinned_unix: None,
                archived_unix: None,
                project: None,
                terminal: Some(Terminal {
                    computer: computer.clone(),
                    session: session.to_owned(),
                    title: title.clone(),
                    digest: digest.clone(),
                    working_unix: None,
                    deleted_unix: None,
                    replies: Vec::new(),
                    reply_ids: Vec::new(),
                    continued: Vec::new(),
                    continued_taken: 0,
                }),
                environment: None,
                tasks: Vec::new(),
                opened_unix: None,
                branch: None,
            };
            match store.create(&chat).await {
                Ok(_) => {
                    return Ok(Saved::Saved {
                        chat: id,
                        changed: true,
                    });
                }
                Err(Error::Conflict) => continue,
                Err(error) => return Err(error),
            }
        };
        let current = &loaded.conversation;
        let Some(terminal) = &current.terminal else {
            return Ok(Saved::Invalid("That chat isn't a Coder chat."));
        };
        if terminal.deleted_unix.is_some() {
            return Ok(Saved::Deleted);
        }
        if terminal.digest == digest {
            return Ok(Saved::Saved {
                chat: id,
                changed: false,
            });
        }
        let mut next = current.clone();
        // A name given on the website stays.
        if current.title == terminal.title {
            next.title = title.clone();
        }
        let (all, continued, taken) = with_continued(&messages, terminal);
        next.messages = all;
        next.revision += 1;
        next.updated_unix = now_unix();
        if let Some(terminal) = &mut next.terminal {
            terminal.continued = continued;
            terminal.continued_taken = taken;
            terminal.computer = computer.clone();
            terminal.title = title.clone();
            terminal.digest = digest.clone();
        }
        match store.compare_and_swap(&loaded, &next).await {
            Ok(_) => {
                return Ok(Saved::Saved {
                    chat: id,
                    changed: true,
                });
            }
            Err(Error::Conflict) => continue,
            Err(error) => return Err(error),
        }
    }
    Err(Error::Conflict)
}

/// Coder's transcript, then the messages added on the website by runs on
/// a Cloud computer (#11050) that it doesn't carry yet, newest
/// [`MAX_MESSAGES`] kept; with the `continued` and `continued_taken` to
/// keep. A message Coder took into its own copy (#11052) is dropped once
/// an upload carries it, since it is in Coder's transcript, where Coder put
/// it; the others follow Coder's transcript, oldest first.
fn with_continued(
    messages: &[Message],
    terminal: &Terminal,
) -> (Vec<Message>, Vec<Message>, usize) {
    let taken = terminal.continued_taken.min(terminal.continued.len());
    let mut kept = Vec::new();
    let mut kept_taken = 0;
    for (n, added) in terminal.continued.iter().enumerate() {
        if n < taken {
            if messages
                .iter()
                .rev()
                .any(|uploaded| carries(uploaded, added))
            {
                continue;
            }
            kept_taken += 1;
        }
        kept.push(added.clone());
    }
    let mut all = messages.to_vec();
    all.extend(kept.iter().cloned());
    if all.len() > MAX_MESSAGES {
        all.drain(..all.len() - MAX_MESSAGES);
    }
    (all, kept, kept_taken)
}

/// Whether `uploaded`, from Coder's transcript, is `added` (Coder cuts a
/// very long message and ends it with "…").
fn carries(uploaded: &Message, added: &Message) -> bool {
    uploaded.role == added.role
        && (uploaded.text == added.text
            || uploaded
                .text
                .strip_suffix("\n…")
                .is_some_and(|cut| !cut.is_empty() && added.text.starts_with(cut)))
}

/// Coder says it is replying (`working`) or idle.
pub(crate) async fn set_status(
    store: &Store,
    owner: &str,
    session: &str,
    working: bool,
) -> Result<Saved, Error> {
    if !valid_session(session) {
        return Ok(Saved::Invalid("That isn't a Coder session id."));
    }
    let id = chat_id(owner, session);
    for _ in 0..4 {
        let Some(loaded) = store.load(owner, &id).await? else {
            return Ok(Saved::Unknown);
        };
        let Some(terminal) = &loaded.conversation.terminal else {
            return Ok(Saved::Unknown);
        };
        if terminal.deleted_unix.is_some() {
            return Ok(Saved::Deleted);
        }
        if !working && terminal.working_unix.is_none() {
            return Ok(Saved::Saved {
                chat: id,
                changed: false,
            });
        }
        let mut next = loaded.conversation.clone();
        next.revision += 1;
        if let Some(terminal) = &mut next.terminal {
            terminal.working_unix = working.then(now_unix);
        }
        match store.compare_and_swap(&loaded, &next).await {
            Ok(_) => {
                return Ok(Saved::Saved {
                    chat: id,
                    changed: true,
                });
            }
            Err(Error::Conflict) => continue,
            Err(error) => return Err(error),
        }
    }
    Err(Error::Conflict)
}

/// Deleted in Coder: remove the chat here for good (a chat deleted on the
/// website too). Returns whether there was one.
pub(crate) async fn remove(store: &Store, owner: &str, session: &str) -> Result<bool, Error> {
    if !valid_session(session) {
        return Ok(false);
    }
    let id = chat_id(owner, session);
    for _ in 0..4 {
        let Some(loaded) = store.load(owner, &id).await? else {
            return Ok(false);
        };
        if loaded.conversation.terminal.is_none() {
            return Ok(false);
        }
        match store.delete(owner, &id, &loaded.generation).await {
            Ok(removed) => return Ok(removed),
            Err(Error::Conflict) => continue,
            Err(error) => return Err(error),
        }
    }
    Err(Error::Conflict)
}

/// The account's Coder chats: each session, and whether it was deleted on
/// the website.
pub(crate) async fn sessions(store: &Store, owner: &str) -> Result<Vec<(String, bool)>, Error> {
    Ok(store
        .list_with_deleted(owner)
        .await?
        .into_iter()
        .filter_map(|chat| {
            let deleted = chat.deleted();
            chat.terminal.map(|terminal| (terminal.session, deleted))
        })
        .collect())
}

/// How recently Coder on a computer must have checked in for the website
/// to offer a reply box on its chats. Coder checks in every 10 seconds.
pub(crate) const ONLINE_SECONDS: u64 = 60;
/// A computer's check-in is written at most this often.
const SEEN_EVERY: u64 = 20;
/// A computer that hasn't checked in for this long is forgotten.
const FORGET_AFTER: u64 = 30 * 24 * 3600;

/// Whether Coder on `computer` checked in recently.
pub(crate) fn online(computers: &Computers, computer: &str) -> bool {
    computers
        .seen
        .get(computer)
        .is_some_and(|at| now_unix().saturating_sub(*at) <= ONLINE_SECONDS)
}

/// Coder on `computer` checked in: remember when, and return its chats
/// with replies from the website waiting.
pub(crate) async fn check_in(
    store: &Store,
    owner: &str,
    computer: &str,
) -> Result<Result<Vec<String>, Saved>, Error> {
    let computer = line(computer, 64);
    if computer.is_empty() {
        return Ok(Err(Saved::Invalid("Send the computer's name.")));
    }
    let name = computer.clone();
    let computers = store
        .update_computers(owner, move |computers| {
            let now = now_unix();
            if computers
                .seen
                .get(&name)
                .is_some_and(|at| now.saturating_sub(*at) < SEEN_EVERY)
            {
                return false;
            }
            computers
                .seen
                .retain(|_, at| now.saturating_sub(*at) < FORGET_AFTER);
            computers.seen.insert(name.clone(), now);
            while computers.seen.len() > MAX_COMPUTERS {
                let Some(oldest) = computers
                    .seen
                    .iter()
                    .min_by_key(|(_, at)| **at)
                    .map(|(name, _)| name.clone())
                else {
                    break;
                };
                computers.seen.remove(&oldest);
            }
            true
        })
        .await?;
    Ok(Ok(computers
        .waiting
        .into_iter()
        .filter(|(_, on)| *on == computer)
        .map(|(session, _)| session)
        .collect()))
}

/// The computer's sync choice (#11089); `None` when it hasn't been asked.
pub(crate) async fn choice(
    store: &Store,
    owner: &str,
    computer: &str,
) -> Result<Option<SyncChoice>, Error> {
    let computer = line(computer, 64);
    Ok(store.computers(owner).await?.sync.get(&computer).copied())
}

/// Remember the computer's sync choice (#11089), from Coder or the
/// website. The oldest choices go first past [`MAX_COMPUTERS`].
pub(crate) async fn choose(
    store: &Store,
    owner: &str,
    computer: &str,
    choice: SyncChoice,
) -> Result<Result<SyncChoice, Saved>, Error> {
    let computer = line(computer, 64);
    if computer.is_empty() {
        return Ok(Err(Saved::Invalid("Send the computer's name.")));
    }
    store
        .update_computers(owner, move |computers| {
            if computers.sync.get(&computer) == Some(&choice) {
                return false;
            }
            computers.sync.insert(computer.clone(), choice);
            while computers.sync.len() > MAX_COMPUTERS {
                // Forget a computer not seen lately (else any other one).
                let Some(oldest) = computers
                    .sync
                    .keys()
                    .filter(|name| **name != computer)
                    .min_by_key(|name| computers.seen.get(*name).copied().unwrap_or(0))
                    .cloned()
                else {
                    break;
                };
                computers.sync.remove(&oldest);
            }
            true
        })
        .await?;
    Ok(Ok(choice))
}

/// What became of a reply typed on the website.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Queued {
    /// Waiting for Coder (or it already was: the same reply id).
    Queued,
    /// Coder on this computer hasn't checked in lately.
    Offline(String),
    /// [`MAX_WEB_REPLIES`] replies already wait.
    Full,
    /// The reply looks like it holds a credential.
    Secret,
    /// No such Coder chat (or it was deleted).
    Missing,
}

/// Queue a reply typed on the website (`id` is the form's request id) for
/// the Coder chat `chat`, when Coder on its computer is online.
pub(crate) async fn queue_reply(
    store: &Store,
    owner: &str,
    chat: &str,
    id: &str,
    text: &str,
) -> Result<Queued, Error> {
    if secret_screen::credential_in(text).is_some() {
        return Ok(Queued::Secret);
    }
    for _ in 0..4 {
        let Some(loaded) = store.load(owner, chat).await? else {
            return Ok(Queued::Missing);
        };
        let Some(terminal) = loaded.conversation.terminal.clone() else {
            return Ok(Queued::Missing);
        };
        if terminal.deleted_unix.is_some() {
            return Ok(Queued::Missing);
        }
        if terminal.reply_ids.iter().any(|sent| sent == id) {
            return Ok(Queued::Queued);
        }
        if terminal.replies.len() >= MAX_WEB_REPLIES {
            return Ok(Queued::Full);
        }
        if !online(&store.computers(owner).await?, &terminal.computer) {
            return Ok(Queued::Offline(terminal.computer));
        }
        // Mark the chat as waiting first, so Coder finds every queued
        // reply; a mark with nothing behind it is cleared when Coder takes.
        let (session, computer) = (terminal.session.clone(), terminal.computer.clone());
        let marked = store
            .update_computers(owner, move |computers| {
                if computers.waiting.get(&session) == Some(&computer)
                    || (!computers.waiting.contains_key(&session)
                        && computers.waiting.len() >= MAX_WAITING_CHATS)
                {
                    return false;
                }
                computers.waiting.insert(session.clone(), computer.clone());
                true
            })
            .await?;
        if !marked.waiting.contains_key(&terminal.session) {
            return Ok(Queued::Full);
        }
        let mut next = loaded.conversation.clone();
        next.revision += 1;
        next.updated_unix = now_unix();
        if let Some(terminal) = &mut next.terminal {
            terminal.replies.push(WebReply {
                id: id.to_owned(),
                text: text.to_owned(),
                sent_unix: now_unix(),
            });
            terminal.reply_ids.push(id.to_owned());
            if terminal.reply_ids.len() > MAX_REPLY_IDS {
                terminal
                    .reply_ids
                    .drain(..terminal.reply_ids.len() - MAX_REPLY_IDS);
            }
        }
        match store.compare_and_swap(&loaded, &next).await {
            Ok(_) => return Ok(Queued::Queued),
            Err(Error::Conflict) => continue,
            Err(error) => return Err(error),
        }
    }
    Err(Error::Conflict)
}

/// What became of a chat started on the website for Coder on a computer.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Started {
    /// The chat (its id) waits for Coder, with the message as its first
    /// reply; also when this request already started it.
    Started(String),
    /// Coder on the computer hasn't checked in lately.
    Offline,
    /// The account has [`MAX_CHATS`] chats.
    Full,
    /// The message looks like it holds a credential.
    Secret,
    /// [`MAX_WAITING_CHATS`] chats already wait for Coder.
    Busy,
}

/// The Coder session a chat started on the website gets (`request` is the
/// composer's request id, a UUID): Coder knows these by the prefix and
/// opens a new conversation under the id when it takes the first message.
pub(crate) fn web_session(request: &str) -> String {
    format!("web-{request}")
}

/// Start a Coder chat from the website (the composer's "Coder on
/// {computer}"): a Coder chat with no messages yet, whose first message
/// waits for Coder on `computer` like a reply ([`queue_reply`]). Coder
/// takes it at its next check-in, opens a new conversation for it, and
/// answers; its upload fills the chat as usual. The same `request` starts
/// it once.
pub(crate) async fn start_from_web(
    store: &Store,
    owner: &str,
    computer: &str,
    request: &str,
    text: &str,
    project: Option<String>,
) -> Result<Started, Error> {
    if secret_screen::credential_in(text).is_some() {
        return Ok(Started::Secret);
    }
    let session = web_session(request);
    if !valid_session(&session) {
        return Ok(Started::Offline);
    }
    let id = chat_id(owner, &session);
    if store.load(owner, &id).await?.is_none() {
        let computer = line(computer, 64);
        if computer.is_empty() || !online(&store.computers(owner).await?, &computer) {
            return Ok(Started::Offline);
        }
        if store.list_with_deleted(owner).await?.len() >= MAX_CHATS {
            return Ok(Started::Full);
        }
        let chat = Conversation {
            id: id.clone(),
            owner: owner.to_owned(),
            revision: 1,
            title: text
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .chars()
                .take(64)
                .collect(),
            messages: Vec::new(),
            pending: None,
            requests: Vec::new(),
            selection: None,
            updated_unix: now_unix(),
            pinned_unix: None,
            archived_unix: None,
            project,
            terminal: Some(Terminal {
                computer,
                session: session.clone(),
                // Empty until Coder's first upload, so the website's title
                // (the first message) stays.
                title: String::new(),
                digest: String::new(),
                working_unix: None,
                deleted_unix: None,
                replies: Vec::new(),
                reply_ids: Vec::new(),
                continued: Vec::new(),
                continued_taken: 0,
            }),
            environment: None,
            tasks: Vec::new(),
            opened_unix: None,
            branch: None,
        };
        match store.create(&chat).await {
            Ok(_) | Err(Error::Conflict) => {}
            Err(error) => return Err(error),
        }
    }
    Ok(match queue_reply(store, owner, &id, request, text).await? {
        Queued::Queued => Started::Started(id),
        Queued::Offline(_) => Started::Offline,
        Queued::Full => Started::Busy,
        Queued::Secret => Started::Secret,
        Queued::Missing => Started::Offline,
    })
}

/// What Coder took from a chat.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Taken {
    Replies {
        replies: Vec<WebReply>,
        /// Messages added here while the computer was offline (#11050),
        /// oldest first, that Coder hadn't taken yet: it adds them to its
        /// own copy before answering `replies`.
        continued: Vec<Message>,
    },
    Deleted,
    Unknown,
}

/// Whether a Coder chat has something for Coder to take: replies, or
/// messages added while its computer was offline.
fn has_waiting(terminal: &Terminal) -> bool {
    terminal.deleted_unix.is_none()
        && (!terminal.replies.is_empty() || terminal.continued_taken < terminal.continued.len())
}

/// Mark a Coder chat as waiting for Coder on its computer when it has
/// messages Coder hasn't taken (see [`take_replies`]), so its next
/// check-in finds them.
pub(crate) async fn mark_waiting(
    store: &Store,
    owner: &str,
    chat: &Conversation,
) -> Result<(), Error> {
    let Some(terminal) = chat.terminal.as_ref().filter(|t| has_waiting(t)) else {
        return Ok(());
    };
    let (session, computer) = (terminal.session.clone(), terminal.computer.clone());
    store
        .update_computers(owner, move |computers| {
            if computers.waiting.get(&session) == Some(&computer)
                || (!computers.waiting.contains_key(&session)
                    && computers.waiting.len() >= MAX_WAITING_CHATS)
            {
                return false;
            }
            computers.waiting.insert(session.clone(), computer.clone());
            true
        })
        .await?;
    Ok(())
}

/// Coder takes what waits in `session`: the replies typed on the website,
/// which join the transcript as the person's messages and make the chat
/// show Working, and the messages added while its computer was offline,
/// which are already in the transcript here (#11052). The chat's waiting
/// mark is cleared.
pub(crate) async fn take_replies(
    store: &Store,
    owner: &str,
    session: &str,
) -> Result<Taken, Error> {
    if !valid_session(session) {
        return Ok(Taken::Unknown);
    }
    let id = chat_id(owner, session);
    let mut taken = None;
    for _ in 0..4 {
        let Some(loaded) = store.load(owner, &id).await? else {
            taken = Some(Taken::Unknown);
            break;
        };
        let Some(terminal) = &loaded.conversation.terminal else {
            taken = Some(Taken::Unknown);
            break;
        };
        if terminal.deleted_unix.is_some() {
            taken = Some(Taken::Deleted);
            break;
        }
        let continued =
            terminal.continued[terminal.continued_taken.min(terminal.continued.len())..].to_vec();
        if terminal.replies.is_empty() && continued.is_empty() {
            taken = Some(Taken::Replies {
                replies: Vec::new(),
                continued,
            });
            break;
        }
        let replies = terminal.replies.clone();
        let mut next = loaded.conversation.clone();
        next.revision += 1;
        next.updated_unix = now_unix();
        next.messages.extend(replies.iter().map(|reply| Message {
            role: Role::User,
            text: reply.text.clone(),
            request_id: None,
        }));
        if next.messages.len() > MAX_MESSAGES {
            next.messages.drain(..next.messages.len() - MAX_MESSAGES);
        }
        if let Some(terminal) = &mut next.terminal {
            if !replies.is_empty() {
                terminal.replies.clear();
                terminal.working_unix = Some(now_unix());
            }
            terminal.continued_taken = terminal.continued.len();
        }
        match store.compare_and_swap(&loaded, &next).await {
            Ok(_) => {
                taken = Some(Taken::Replies { replies, continued });
                break;
            }
            Err(Error::Conflict) => continue,
            Err(error) => return Err(error),
        }
    }
    let Some(taken) = taken else {
        return Err(Error::Conflict);
    };
    let unmark = session.to_owned();
    store
        .update_computers(owner, move |computers| {
            computers.waiting.remove(&unmark).is_some()
        })
        .await?;
    // A reply (or a Cloud computer's answer) added between the take and the
    // unmark is marked again.
    if let Some(loaded) = store.load(owner, &id).await?
        && let Some(terminal) = loaded.conversation.terminal
        && has_waiting(&terminal)
    {
        let (session, computer) = (terminal.session, terminal.computer);
        store
            .update_computers(owner, move |computers| {
                computers.waiting.len() < MAX_WAITING_CHATS
                    && computers
                        .waiting
                        .insert(session.clone(), computer.clone())
                        .is_none()
            })
            .await?;
    }
    Ok(taken)
}

pub(crate) fn answer(status: StatusCode, body: Value) -> Response {
    protect((status, axum::Json(body)).into_response())
}

pub(crate) fn refused(status: StatusCode, code: &str, message: &str) -> Response {
    answer(status, json!({"error": {"code": code, "message": message}}))
}

pub(crate) fn stored(error: &Error) -> Response {
    eprintln!("openagents-web: coder sync: {error}");
    refused(
        StatusCode::SERVICE_UNAVAILABLE,
        "unavailable",
        "Try again later.",
    )
}

fn respond(result: Result<Saved, Error>) -> Response {
    match result {
        Ok(Saved::Saved { chat, changed }) => {
            answer(StatusCode::OK, json!({"chat": chat, "changed": changed}))
        }
        Ok(Saved::Deleted) => refused(
            StatusCode::GONE,
            "deleted",
            "This chat was deleted on the website.",
        ),
        Ok(Saved::Unknown) => refused(StatusCode::NOT_FOUND, "unknown", "No such chat."),
        Ok(Saved::Full) => refused(
            StatusCode::PAYLOAD_TOO_LARGE,
            "full",
            "Your account has no room for more chats. Delete some on openagents.com.",
        ),
        Ok(Saved::Secret) => refused(
            StatusCode::UNPROCESSABLE_ENTITY,
            "secret",
            "A message looks like it holds a password or key, so it wasn't saved.",
        ),
        Ok(Saved::Invalid("too_large")) => refused(
            StatusCode::PAYLOAD_TOO_LARGE,
            "too_large",
            "This chat is too long to save to your account.",
        ),
        Ok(Saved::Invalid(message)) => refused(StatusCode::BAD_REQUEST, "invalid", message),
        Err(error) => stored(&error),
    }
}

/// The account a request's app token signs in, as its chat owner.
pub(crate) async fn owner(app: &App, headers: &HeaderMap) -> Result<String, Response> {
    let Some(service) = app.config.cloud.as_deref() else {
        return Err(refused(
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
            "This site doesn't offer accounts.",
        ));
    };
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or_default();
    let key: [u8; 32] = Sha256::digest(token.as_bytes()).into();
    if let Some(account) = remembered(&key) {
        return Ok(account_owner(&account));
    }
    match service.app_account(token).await {
        Ok(account) => {
            remember(key, &account);
            Ok(account_owner(&account))
        }
        Err(SessionError::Unauthenticated | SessionError::InvalidRequest) => Err(refused(
            StatusCode::UNAUTHORIZED,
            "signed_out",
            "Sign in again with coder login.",
        )),
        Err(_) => Err(refused(
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
            "Try again later.",
        )),
    }
}

/// How long a checked token is reused, so a chat's checkpoints don't each
/// ask the account service.
const REMEMBER: Duration = Duration::from_secs(30);

fn checked_tokens() -> &'static Mutex<HashMap<[u8; 32], (String, Instant)>> {
    static CHECKED: OnceLock<Mutex<HashMap<[u8; 32], (String, Instant)>>> = OnceLock::new();
    CHECKED.get_or_init(Default::default)
}

fn remembered(key: &[u8; 32]) -> Option<String> {
    let kept = checked_tokens().lock().ok()?;
    let (account, at) = kept.get(key)?;
    (at.elapsed() < REMEMBER).then(|| account.clone())
}

fn remember(key: [u8; 32], account: &str) {
    if let Ok(mut kept) = checked_tokens().lock() {
        kept.retain(|_, (_, at)| at.elapsed() < REMEMBER);
        if kept.len() > 4096 {
            kept.clear();
        }
        kept.insert(key, (account.to_owned(), Instant::now()));
    }
}

async fn upload(
    State(app): State<App>,
    headers: HeaderMap,
    Path(session): Path<String>,
    body: Bytes,
) -> Response {
    let owner = match owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    let Ok(upload) = serde_json::from_slice::<Upload>(&body) else {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid",
            "Send {computer, title, messages}.",
        );
    };
    respond(save(&app.config.chat_store, &owner, &session, &upload).await)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Status {
    working: bool,
}

async fn status(
    State(app): State<App>,
    headers: HeaderMap,
    Path(session): Path<String>,
    body: Bytes,
) -> Response {
    let owner = match owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    let Ok(sent) = serde_json::from_slice::<Status>(&body) else {
        return refused(StatusCode::BAD_REQUEST, "invalid", "Send {working}.");
    };
    respond(set_status(&app.config.chat_store, &owner, &session, sent.working).await)
}

async fn forget(
    State(app): State<App>,
    headers: HeaderMap,
    Path(session): Path<String>,
) -> Response {
    let owner = match owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    match remove(&app.config.chat_store, &owner, &session).await {
        Ok(deleted) => answer(StatusCode::OK, json!({"deleted": deleted})),
        Err(error) => stored(&error),
    }
}

async fn list(State(app): State<App>, headers: HeaderMap) -> Response {
    let owner = match owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    match sessions(&app.config.chat_store, &owner).await {
        Ok(rows) => answer(
            StatusCode::OK,
            json!({"sessions": rows
                .into_iter()
                .map(|(session, deleted)| json!({"session": session, "deleted": deleted}))
                .collect::<Vec<_>>()}),
        ),
        Err(error) => stored(&error),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckIn {
    computer: String,
}

async fn check_in_route(State(app): State<App>, headers: HeaderMap, body: Bytes) -> Response {
    let owner = match owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    let Ok(sent) = serde_json::from_slice::<CheckIn>(&body) else {
        return refused(StatusCode::BAD_REQUEST, "invalid", "Send {computer}.");
    };
    match check_in(&app.config.chat_store, &owner, &sent.computer).await {
        Ok(Ok(waiting)) => {
            let sync = choice(&app.config.chat_store, &owner, &sent.computer)
                .await
                .ok()
                .flatten()
                .map(SyncChoice::as_str);
            answer(StatusCode::OK, json!({"waiting": waiting, "sync": sync}))
        }
        Ok(Err(saved)) => respond(Ok(saved)),
        Err(error) => stored(&error),
    }
}

#[derive(Deserialize)]
struct ChoiceQuery {
    #[serde(default)]
    computer: String,
}

async fn choice_route(
    State(app): State<App>,
    headers: HeaderMap,
    axum::extract::Query(query): axum::extract::Query<ChoiceQuery>,
) -> Response {
    let owner = match owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    match choice(&app.config.chat_store, &owner, &query.computer).await {
        Ok(found) => answer(
            StatusCode::OK,
            json!({"choice": found.map(SyncChoice::as_str)}),
        ),
        Err(error) => stored(&error),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Choose {
    computer: String,
    choice: String,
}

async fn choose_route(State(app): State<App>, headers: HeaderMap, body: Bytes) -> Response {
    let owner = match owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    let Some((computer, picked)) = serde_json::from_slice::<Choose>(&body)
        .ok()
        .and_then(|sent| Some((sent.computer, SyncChoice::parse(&sent.choice)?)))
    else {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid",
            "Send {computer, choice: all or local}.",
        );
    };
    match choose(&app.config.chat_store, &owner, &computer, picked).await {
        Ok(Ok(picked)) => answer(StatusCode::OK, json!({"choice": picked.as_str()})),
        Ok(Err(saved)) => respond(Ok(saved)),
        Err(error) => stored(&error),
    }
}

async fn replies(
    State(app): State<App>,
    headers: HeaderMap,
    Path(session): Path<String>,
) -> Response {
    let owner = match owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    match take_replies(&app.config.chat_store, &owner, &session).await {
        Ok(Taken::Replies { replies, continued }) => answer(
            StatusCode::OK,
            json!({
                "replies": replies
                    .into_iter()
                    .map(|reply| json!({"id": reply.id, "text": reply.text}))
                    .collect::<Vec<_>>(),
                "continued": continued
                    .into_iter()
                    .map(|message| json!({"role": message.role, "text": message.text}))
                    .collect::<Vec<_>>(),
            }),
        ),
        Ok(Taken::Deleted) => respond(Ok(Saved::Deleted)),
        Ok(Taken::Unknown) => respond(Ok(Saved::Unknown)),
        Err(error) => stored(&error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owner() -> String {
        account_owner("acct_one")
    }

    fn upload(title: &str, texts: &[(&str, &str)]) -> Upload {
        Upload {
            computer: "Studio".into(),
            title: title.into(),
            messages: texts
                .iter()
                .map(|(role, text)| WireMessage {
                    role: (*role).into(),
                    text: (*text).into(),
                })
                .collect(),
        }
    }

    #[test]
    fn chat_ids_are_stable_version_four_uuids_per_account() {
        let one = chat_id(&owner(), "session-1");
        assert_eq!(one, chat_id(&owner(), "session-1"));
        assert_ne!(one, chat_id(&account_owner("acct_two"), "session-1"));
        assert_eq!(one.len(), 36);
        assert_eq!(&one[14..15], "4");
        assert!(matches!(&one[19..20], "8" | "9" | "a" | "b"));
        assert!(valid_session("coder-new-20261009_abc"));
        assert!(!valid_session("../x") && !valid_session(""));
    }

    #[tokio::test]
    async fn an_upload_saves_updates_and_keeps_a_web_rename() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::local(dir.path().to_path_buf());
        let first = upload(
            "Fix the build",
            &[("user", "Fix it"), ("assistant", "Done.")],
        );
        let Saved::Saved { chat, changed } = save(&store, &owner(), "s1", &first).await.unwrap()
        else {
            panic!("not saved");
        };
        assert!(changed);
        // The same upload again writes nothing.
        assert_eq!(
            save(&store, &owner(), "s1", &first).await.unwrap(),
            Saved::Saved {
                chat: chat.clone(),
                changed: false
            }
        );
        let listed = store.list(&owner()).await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].title, "Fix the build");
        assert_eq!(listed[0].terminal.as_ref().unwrap().computer, "Studio");
        assert_eq!(listed[0].messages.len(), 2);

        // Renamed on the website: a later upload keeps the name.
        let loaded = store.load(&owner(), &chat).await.unwrap().unwrap();
        let mut renamed = loaded.conversation.clone();
        renamed.title = "My name".into();
        renamed.revision += 1;
        store.compare_and_swap(&loaded, &renamed).await.unwrap();
        let more = upload(
            "Fix the build",
            &[
                ("user", "Fix it"),
                ("assistant", "Done."),
                ("user", "Thanks"),
            ],
        );
        save(&store, &owner(), "s1", &more).await.unwrap();
        let chat_now = store
            .load(&owner(), &chat)
            .await
            .unwrap()
            .unwrap()
            .conversation;
        assert_eq!(chat_now.title, "My name");
        assert_eq!(chat_now.messages.len(), 3);
    }

    #[tokio::test]
    async fn messages_added_by_a_cloud_computer_stay_after_coder_uploads_again() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::local(dir.path().to_path_buf());
        let first = upload("Fix", &[("user", "Fix it"), ("assistant", "Looking.")]);
        let Saved::Saved { chat, .. } = save(&store, &owner(), "s1", &first).await.unwrap() else {
            panic!("not saved");
        };
        // Continued on a Cloud computer while the computer was offline.
        let loaded = store.load(&owner(), &chat).await.unwrap().unwrap();
        let mut next = loaded.conversation.clone();
        let added = [
            Message {
                role: Role::User,
                text: "Go on".into(),
                request_id: None,
            },
            Message {
                role: Role::Assistant,
                text: "Fixed in the cloud.".into(),
                request_id: None,
            },
        ];
        next.messages.extend(added.iter().cloned());
        next.terminal.as_mut().unwrap().continued = added.to_vec();
        next.revision += 1;
        store.compare_and_swap(&loaded, &next).await.unwrap();
        // Coder comes back and uploads its own copy, which lacks them.
        let more = upload(
            "Fix",
            &[
                ("user", "Fix it"),
                ("assistant", "Looking."),
                ("user", "Hi"),
            ],
        );
        save(&store, &owner(), "s1", &more).await.unwrap();
        let now = store.load(&owner(), &chat).await.unwrap().unwrap();
        let texts: Vec<&str> = now
            .conversation
            .messages
            .iter()
            .map(|m| m.text.as_str())
            .collect();
        assert_eq!(
            texts,
            ["Fix it", "Looking.", "Hi", "Go on", "Fixed in the cloud."]
        );

        // The chat waits for Coder on Studio, which takes them (#11052).
        mark_waiting(&store, &owner(), &now.conversation)
            .await
            .unwrap();
        assert_eq!(
            check_in(&store, &owner(), "Studio").await.unwrap(),
            Ok(vec!["s1".to_string()])
        );
        let Taken::Replies { replies, continued } =
            take_replies(&store, &owner(), "s1").await.unwrap()
        else {
            panic!("not taken");
        };
        assert!(replies.is_empty());
        assert_eq!(continued, added.to_vec());
        let taken = store.load(&owner(), &chat).await.unwrap().unwrap();
        assert!(!taken.conversation.working(), "nothing to answer");
        assert_eq!(taken.conversation.messages.len(), 5, "already shown");
        assert_eq!(
            check_in(&store, &owner(), "Studio").await.unwrap(),
            Ok(Vec::new())
        );
        assert_eq!(
            take_replies(&store, &owner(), "s1").await.unwrap(),
            Taken::Replies {
                replies: Vec::new(),
                continued: Vec::new()
            },
            "handed out once"
        );
        // An upload made before Coder added them still shows them.
        save(&store, &owner(), "s1", &more).await.unwrap();
        // Coder's copy now carries them where it put them, then goes on:
        // each shows once, in Coder's order.
        let carried = upload(
            "Fix",
            &[
                ("user", "Fix it"),
                ("assistant", "Looking."),
                ("user", "Hi"),
                ("user", "Go on"),
                ("assistant", "Fixed in the cloud."),
                ("user", "Thanks"),
            ],
        );
        save(&store, &owner(), "s1", &carried).await.unwrap();
        let last = store.load(&owner(), &chat).await.unwrap().unwrap();
        let texts: Vec<&str> = last
            .conversation
            .messages
            .iter()
            .map(|m| m.text.as_str())
            .collect();
        assert_eq!(
            texts,
            [
                "Fix it",
                "Looking.",
                "Hi",
                "Go on",
                "Fixed in the cloud.",
                "Thanks"
            ]
        );
        let terminal = last.conversation.terminal.unwrap();
        assert!(terminal.continued.is_empty() && terminal.continued_taken == 0);
    }

    #[test]
    fn a_taken_message_is_dropped_only_once_an_upload_carries_it() {
        let message = |role, text: &str| Message {
            role,
            text: text.into(),
            request_id: None,
        };
        let mut terminal = Terminal {
            computer: "Studio".into(),
            session: "s1".into(),
            title: "t".into(),
            digest: String::new(),
            working_unix: None,
            deleted_unix: None,
            replies: Vec::new(),
            reply_ids: Vec::new(),
            continued: vec![
                message(Role::User, "Go on"),
                message(Role::Assistant, &"long ".repeat(10)),
                message(Role::Assistant, "Later answer"),
            ],
            continued_taken: 2,
        };
        let uploaded = [
            message(Role::User, "Fix it"),
            message(Role::User, "Go on"),
            // Coder cut the long one.
            message(Role::Assistant, "long long\n…"),
        ];
        let (all, kept, taken) = with_continued(&uploaded, &terminal);
        assert_eq!(kept, vec![message(Role::Assistant, "Later answer")]);
        assert_eq!(taken, 0);
        assert_eq!(all.len(), 4);
        // Not carried yet: kept, still counted as taken.
        terminal.continued_taken = 1;
        let (_, kept, taken) = with_continued(&uploaded[..1], &terminal);
        assert_eq!((kept.len(), taken), (3, 1));
    }

    #[tokio::test]
    async fn a_planted_secret_is_refused_and_nothing_is_saved() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::local(dir.path().to_path_buf());
        // Assembled at run time so no credential-shaped literal sits here.
        let key = format!("sk-ant-{}", "a1".repeat(20));
        let planted = upload("Keys", &[("user", &format!("my key is {key}"))]);
        assert_eq!(
            save(&store, &owner(), "s1", &planted).await.unwrap(),
            Saved::Secret
        );
        assert!(store.list(&owner()).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn status_shows_working_and_a_web_delete_reaches_coder() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::local(dir.path().to_path_buf());
        assert_eq!(
            set_status(&store, &owner(), "s1", true).await.unwrap(),
            Saved::Unknown
        );
        let Saved::Saved { chat, .. } =
            save(&store, &owner(), "s1", &upload("A", &[("user", "Hi")]))
                .await
                .unwrap()
        else {
            panic!("not saved");
        };
        set_status(&store, &owner(), "s1", true).await.unwrap();
        assert!(store.list(&owner()).await.unwrap()[0].working());
        set_status(&store, &owner(), "s1", false).await.unwrap();
        assert!(!store.list(&owner()).await.unwrap()[0].working());

        // Deleted on the website: hidden, and the next upload hears so.
        let loaded = store.load(&owner(), &chat).await.unwrap().unwrap();
        assert!(store.remove(&loaded).await.unwrap());
        assert!(store.list(&owner()).await.unwrap().is_empty());
        assert_eq!(
            sessions(&store, &owner()).await.unwrap(),
            vec![("s1".to_string(), true)]
        );
        assert_eq!(
            save(
                &store,
                &owner(),
                "s1",
                &upload("A", &[("user", "Hi again")])
            )
            .await
            .unwrap(),
            Saved::Deleted
        );
        // Coder deletes its copy and says so: gone for good.
        assert!(remove(&store, &owner(), "s1").await.unwrap());
        assert!(sessions(&store, &owner()).await.unwrap().is_empty());
        assert!(!remove(&store, &owner(), "s1").await.unwrap());
    }

    #[tokio::test]
    async fn another_account_never_sees_or_removes_the_chat() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::local(dir.path().to_path_buf());
        save(&store, &owner(), "s1", &upload("A", &[("user", "Hi")]))
            .await
            .unwrap();
        let other = account_owner("acct_two");
        assert!(store.list(&other).await.unwrap().is_empty());
        assert!(!remove(&store, &other, "s1").await.unwrap());
        assert_eq!(store.list(&owner()).await.unwrap().len(), 1);
    }

    const REPLY: &str = "0f8fad5b-d9cb-469f-a165-70867728950e";
    const REPLY_TWO: &str = "7c9e6679-7425-40de-944b-e07fc1f90ae7";

    #[tokio::test]
    async fn a_web_reply_waits_for_coder_and_joins_the_transcript() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::local(dir.path().to_path_buf());
        let Saved::Saved { chat, .. } =
            save(&store, &owner(), "s1", &upload("A", &[("user", "Hi")]))
                .await
                .unwrap()
        else {
            panic!("not saved");
        };
        // Coder on Studio hasn't checked in: offline, nothing queued.
        assert_eq!(
            queue_reply(&store, &owner(), &chat, REPLY, "More please")
                .await
                .unwrap(),
            Queued::Offline("Studio".into())
        );
        assert_eq!(
            check_in(&store, &owner(), "Studio").await.unwrap(),
            Ok(Vec::new())
        );
        assert!(online(&store.computers(&owner()).await.unwrap(), "Studio"));
        assert!(!online(&store.computers(&owner()).await.unwrap(), "Laptop"));

        // Online: queued once, even when the form is sent twice.
        for _ in 0..2 {
            assert_eq!(
                queue_reply(&store, &owner(), &chat, REPLY, "More please")
                    .await
                    .unwrap(),
                Queued::Queued
            );
        }
        let waiting = store.load(&owner(), &chat).await.unwrap().unwrap();
        assert_eq!(
            waiting
                .conversation
                .terminal
                .as_ref()
                .unwrap()
                .replies
                .len(),
            1
        );
        // Another computer's Coder doesn't see it; Studio's does.
        assert_eq!(
            check_in(&store, &owner(), "Laptop").await.unwrap(),
            Ok(Vec::new())
        );
        assert_eq!(
            check_in(&store, &owner(), "Studio").await.unwrap(),
            Ok(vec!["s1".to_string()])
        );

        // Coder takes it: it joins the transcript, the chat shows Working,
        // and the chat no longer waits.
        let Taken::Replies { replies: taken, .. } =
            take_replies(&store, &owner(), "s1").await.unwrap()
        else {
            panic!("not taken");
        };
        assert_eq!(taken.len(), 1);
        assert_eq!(taken[0].text, "More please");
        let after = store
            .load(&owner(), &chat)
            .await
            .unwrap()
            .unwrap()
            .conversation;
        assert_eq!(after.messages.last().unwrap().text, "More please");
        assert_eq!(after.messages.last().unwrap().role, Role::User);
        assert!(after.working());
        assert!(after.terminal.as_ref().unwrap().replies.is_empty());
        assert_eq!(
            check_in(&store, &owner(), "Studio").await.unwrap(),
            Ok(Vec::new())
        );
        assert_eq!(
            take_replies(&store, &owner(), "s1").await.unwrap(),
            Taken::Replies {
                replies: Vec::new(),
                continued: Vec::new()
            }
        );
        // A resend of the taken reply isn't queued again.
        assert_eq!(
            queue_reply(&store, &owner(), &chat, REPLY, "More please")
                .await
                .unwrap(),
            Queued::Queued
        );
        assert!(
            store
                .load(&owner(), &chat)
                .await
                .unwrap()
                .unwrap()
                .conversation
                .terminal
                .unwrap()
                .replies
                .is_empty()
        );

        // Coder's next upload carries the reply and the answer as usual.
        save(
            &store,
            &owner(),
            "s1",
            &upload(
                "A",
                &[
                    ("user", "Hi"),
                    ("user", "More please"),
                    ("assistant", "Here."),
                ],
            ),
        )
        .await
        .unwrap();
        let synced = store
            .load(&owner(), &chat)
            .await
            .unwrap()
            .unwrap()
            .conversation;
        assert_eq!(synced.messages.len(), 3);
    }

    #[tokio::test]
    async fn web_replies_are_bounded_screened_and_owner_only() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::local(dir.path().to_path_buf());
        let Saved::Saved { chat, .. } =
            save(&store, &owner(), "s1", &upload("A", &[("user", "Hi")]))
                .await
                .unwrap()
        else {
            panic!("not saved");
        };
        check_in(&store, &owner(), "Studio").await.unwrap().unwrap();
        let key = format!("sk-ant-{}", "a1".repeat(20));
        assert_eq!(
            queue_reply(&store, &owner(), &chat, REPLY, &format!("use {key}"))
                .await
                .unwrap(),
            Queued::Secret
        );
        // Another account can't queue on it or take from it.
        let other = account_owner("acct_two");
        assert_eq!(
            queue_reply(&store, &other, &chat, REPLY, "Hi")
                .await
                .unwrap(),
            Queued::Missing
        );
        assert_eq!(
            take_replies(&store, &other, "s1").await.unwrap(),
            Taken::Unknown
        );
        // At most MAX_WEB_REPLIES wait.
        let ids = [
            REPLY,
            REPLY_TWO,
            "16fd2706-8baf-433b-82eb-8c7fada847da",
            "886313e1-3b8a-4372-9b90-0c9aee199e5d",
        ];
        for id in ids {
            assert_eq!(
                queue_reply(&store, &owner(), &chat, id, "again")
                    .await
                    .unwrap(),
                Queued::Queued
            );
        }
        assert_eq!(
            queue_reply(
                &store,
                &owner(),
                &chat,
                "a8098c1a-f86e-41da-9e2c-1e3f1a2b3c4d",
                "one more"
            )
            .await
            .unwrap(),
            Queued::Full
        );
        // Deleted on the website: nothing waits and Coder hears so.
        let loaded = store.load(&owner(), &chat).await.unwrap().unwrap();
        assert!(store.remove(&loaded).await.unwrap());
        assert_eq!(
            take_replies(&store, &owner(), "s1").await.unwrap(),
            Taken::Deleted
        );
        assert_eq!(
            check_in(&store, &owner(), "Studio").await.unwrap(),
            Ok(Vec::new())
        );
    }

    #[tokio::test]
    async fn each_computer_keeps_its_own_sync_choice() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::local(dir.path().to_path_buf());
        assert_eq!(choice(&store, &owner(), "Studio").await.unwrap(), None);
        choose(&store, &owner(), "Studio", SyncChoice::All)
            .await
            .unwrap()
            .unwrap();
        choose(&store, &owner(), " Laptop ", SyncChoice::Local)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            choice(&store, &owner(), "Studio").await.unwrap(),
            Some(SyncChoice::All)
        );
        assert_eq!(
            choice(&store, &owner(), "Laptop").await.unwrap(),
            Some(SyncChoice::Local)
        );
        // Another account's computers are its own.
        assert_eq!(
            choice(&store, &account_owner("acct_two"), "Studio")
                .await
                .unwrap(),
            None
        );
        assert!(matches!(
            choose(&store, &owner(), "  ", SyncChoice::All)
                .await
                .unwrap(),
            Err(Saved::Invalid(_))
        ));
        // Bounded: the oldest go first.
        for n in 0..MAX_COMPUTERS + 4 {
            choose(&store, &owner(), &format!("box-{n}"), SyncChoice::All)
                .await
                .unwrap()
                .unwrap();
        }
        assert_eq!(
            store.computers(&owner()).await.unwrap().sync.len(),
            MAX_COMPUTERS
        );
    }

    #[test]
    fn refusals_are_plain() {
        for text in [
            "This chat was deleted on the website.",
            "Your account has no room for more chats. Delete some on openagents.com.",
            "A message looks like it holds a password or key, so it wasn't saved.",
            "This chat is too long to save to your account.",
            "Sign in again with coder login.",
            // The chat page's words for a reply sent to Coder (#11048).
            "This chat runs in Coder on Studio. To reply here, open Coder there and type /sync on.",
            "Reply to Coder on Studio",
            "Waiting for Coder on Studio.",
            "Coder on Studio isn't online now. Open Coder there to reply.",
            "Wait for Coder to answer your earlier replies.",
            "This looks like it holds a password or key, so it wasn't sent.",
        ] {
            assert!(oa_copy::violations(text, &[]).is_empty(), "{text}");
        }
    }
}
