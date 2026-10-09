//! The sidebar's live stream (#11035, `docs/web/sidebar.md` "Live
//! updates"): one server-sent events connection per tab, at
//! [`EVENTS`], that replaces a row's status slot (`#chat-row-status-{id}`,
//! [`row_status_slot`]) out of band when the chat's status changes, so a
//! chat answering in the background goes from Working to nothing (or
//! Failed) without a reload.
//!
//! It is cheap on purpose:
//! - Writes this process makes wake it at once ([`crate::chat_store::Store::changes`]);
//!   it waits a moment for a burst of writes (an answer streaming in) to
//!   settle and then reads only the chats that changed.
//! - A chat it last saw Working is read again every [`WORKING_EVERY`], in
//!   case another replica finishes it.
//! - Every [`ALL_EVERY`], and when a tab reconnects after a gap, it reads the
//!   list once and sends the rows changed since its cursor.
//! - No timer per chat, and the connection ends after [`LIFETIME`]; the
//!   browser reconnects by itself with the last event id (a unix time), and
//!   the stream resumes after it.
//!
//! Only the owner's chats are read (every read is keyed by the cookie's
//! owner), so only the owner's rows are ever sent. Cloud jobs and Coder tasks
//! are not linked to chats yet, so their statuses have no source here.

use std::collections::{HashMap, HashSet};

use tokio::sync::broadcast::{self, error::TryRecvError};
use tokio::time::Instant;

use super::*;
use crate::chat_store::Change;

pub(super) const EVENTS: &str = "/chats/events";
/// How long one connection lives before the browser reconnects.
const LIFETIME: Duration = Duration::from_secs(600);
/// How long to wait after a write before reading, so a burst reads once.
const SETTLE: Duration = Duration::from_secs(1);
/// How often a chat last seen Working is read again.
const WORKING_EVERY: Duration = Duration::from_secs(5);
/// How often the whole list is checked (for writes on other replicas).
const ALL_EVERY: Duration = Duration::from_secs(120);
/// A connection made this soon after the page was drawn skips the catch-up.
const FRESH_SECONDS: u64 = 5;
/// The most Working chats a page names in its connection address.
const MAX_WORKING: usize = 32;

pub(super) fn routes() -> Router<App> {
    Router::new().route(EVENTS, get(events))
}

/// The hidden element holding the tab's connection. It sits beside the list,
/// not inside it, so the list's own out-of-band replacements (pin, rename, a
/// new answer) keep the connection. `after` is when the page read the list;
/// `working` names the chats it drew as Working.
pub(super) fn connector<'a>(after: u64, working: impl IntoIterator<Item = &'a str>) -> Markup {
    let working: Vec<&str> = working.into_iter().take(MAX_WORKING).collect();
    let mut url = format!("{EVENTS}?after={after}");
    if !working.is_empty() {
        url.push_str("&working=");
        url.push_str(&working.join(","));
    }
    html! {
        div #chat-sidebar-live hidden hx-ext="sse" sse-connect=(url) {
            div sse-swap="status" hx-swap="none" {}
        }
    }
}

#[derive(Default, Deserialize)]
struct Resume {
    after: Option<u64>,
    #[serde(default)]
    working: String,
}

/// What the stream knows: each chat's status as last sent or drawn. A chat
/// missing from the map has an unknown status and is sent when next read.
struct Watch {
    app: App,
    owner: String,
    changes: broadcast::Receiver<Change>,
    shown: HashMap<String, Option<ChatStatus>>,
    dirty: HashSet<String>,
    /// Read the whole list next, sending rows changed since `cursor`.
    catch_up: bool,
    /// Unix time up to which changes have been sent.
    cursor: u64,
    ends: Instant,
    /// When to read again after the store failed.
    retry: Option<Instant>,
    next_working: Instant,
    next_all: Instant,
}

async fn events(
    State(app): State<App>,
    headers: HeaderMap,
    Query(resume): Query<Resume>,
) -> Response {
    let who = crate::chat_owner::who(&app, &headers).await;
    let Some(owner) = who.reader().map(str::to_owned) else {
        // No chats to follow; 204 tells the browser not to reconnect.
        return crate::chat_html::protect(StatusCode::NO_CONTENT.into_response());
    };
    let reconnect = headers
        .get("Last-Event-ID")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok());
    let now_unix = now();
    let cursor = reconnect.or(resume.after).unwrap_or(now_unix).min(now_unix);
    let working = parse_working(&resume.working);
    let watch = Watch::new(app, owner, cursor, working);
    let stream = futures_util::stream::unfold(watch, |mut watch| async move {
        let body = watch.next().await?;
        let event = Event::default()
            .id(watch.cursor.to_string())
            .event("status")
            .data(body);
        Some((Ok::<_, Infallible>(event), watch))
    });
    crate::chat_html::protect(
        Sse::new(stream)
            .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
            .into_response(),
    )
}

/// The chat ids a page drew as Working, invalid ones dropped.
fn parse_working(value: &str) -> Vec<String> {
    let mut ids: Vec<String> = value
        .split(',')
        .filter(|id| valid_id(id))
        .take(MAX_WORKING)
        .map(str::to_owned)
        .collect();
    ids.sort();
    ids.dedup();
    ids
}

impl Watch {
    /// Follows `owner`'s chats from `cursor` (a unix time), knowing the
    /// `working` chats were drawn as Working.
    fn new(app: App, owner: String, cursor: u64, working: Vec<String>) -> Self {
        let start = Instant::now();
        Self {
            changes: app.config.chat_store.changes(),
            app,
            owner,
            shown: working
                .iter()
                .map(|id| (id.clone(), Some(ChatStatus::Working)))
                .collect(),
            // Read the Working chats at once: one may have finished meanwhile.
            dirty: working.into_iter().collect(),
            catch_up: now().saturating_sub(cursor) > FRESH_SECONDS,
            cursor,
            ends: start + LIFETIME,
            retry: None,
            next_working: start + WORKING_EVERY,
            next_all: start + ALL_EVERY,
        }
    }

    /// Waits for the next batch of changed status slots; `None` when the
    /// connection's time is up.
    async fn next(&mut self) -> Option<String> {
        loop {
            if Instant::now() >= self.ends {
                return None;
            }
            if let Some(at) = self.retry.take() {
                tokio::time::sleep_until(at.min(self.ends)).await;
                continue;
            }
            if !self.catch_up && self.dirty.is_empty() {
                self.wait().await?;
            }
            let full = self.catch_up || Instant::now() >= self.next_all;
            let started = now();
            let slots = if full {
                self.read_all().await
            } else {
                self.read_dirty().await
            };
            match slots {
                Ok(slots) => {
                    // Only a full read covers other replicas' writes, so only
                    // it moves the cursor a reconnect resumes from. A write
                    // during the read is read again later, so the cursor
                    // stays a second behind its start.
                    if full {
                        self.cursor = self.cursor.max(started.saturating_sub(1));
                    }
                    if !slots.is_empty() {
                        return Some(slots);
                    }
                }
                // The store is failing: send what was read, and try the
                // rest again later, not at once.
                Err(slots) => {
                    self.retry = Some(Instant::now() + WORKING_EVERY);
                    if !slots.is_empty() {
                        return Some(slots);
                    }
                }
            }
        }
    }

    /// Sleeps until a write for this owner (then lets the burst settle), a
    /// Working chat is due for a check, or the full check is due.
    async fn wait(&mut self) -> Option<()> {
        loop {
            let working = self.shown.values().any(|s| *s == Some(ChatStatus::Working));
            let mut due = self.next_all.min(self.ends);
            if working {
                due = due.min(self.next_working);
            }
            tokio::select! {
                change = self.changes.recv() => match change {
                    Ok(change) => {
                        if self.mark(&change) {
                            tokio::time::sleep(SETTLE).await;
                            self.drain();
                            return Some(());
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        self.catch_up = true;
                        return Some(());
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        tokio::time::sleep_until(due).await;
                    }
                },
                () = tokio::time::sleep_until(due) => {}
            }
            let now = Instant::now();
            if now >= self.ends {
                return None;
            }
            if now >= self.next_all {
                return Some(());
            }
            if working && now >= self.next_working {
                self.next_working = now + WORKING_EVERY;
                self.dirty.extend(
                    self.shown
                        .iter()
                        .filter(|(_, s)| **s == Some(ChatStatus::Working))
                        .map(|(id, _)| id.clone()),
                );
                return Some(());
            }
        }
    }

    /// Notes a write; true when it is one of this owner's chats.
    fn mark(&mut self, change: &Change) -> bool {
        if *change.owner != *self.owner {
            return false;
        }
        self.dirty.insert(change.id.to_string());
        true
    }

    /// Takes every write announced since, without waiting.
    fn drain(&mut self) {
        loop {
            match self.changes.try_recv() {
                Ok(change) => {
                    self.mark(&change);
                }
                Err(TryRecvError::Lagged(_)) => self.catch_up = true,
                Err(TryRecvError::Empty | TryRecvError::Closed) => return,
            }
        }
    }

    /// Reads the chats marked changed. `Err` carries what was read before
    /// the store failed; the rest stay marked.
    async fn read_dirty(&mut self) -> Result<String, String> {
        let ids: Vec<String> = self.dirty.drain().collect();
        let mut slots = String::new();
        let store = self.app.config.chat_store.clone();
        let mut ids = ids.into_iter();
        while let Some(id) = ids.next() {
            match store.load(&self.owner, &id).await {
                Ok(Some(loaded)) => slots.push_str(&self.slot(&loaded.conversation)),
                // A deleted chat's row is gone with it.
                Ok(None) => {
                    self.shown.remove(&id);
                }
                Err(error) => {
                    eprintln!("openagents-web: sidebar events: {error}");
                    self.dirty.insert(id);
                    self.dirty.extend(ids);
                    return Err(slots);
                }
            }
        }
        Ok(slots)
    }

    /// Reads the whole list and sends the rows that changed since the
    /// cursor (or whose status differs from what was sent).
    async fn read_all(&mut self) -> Result<String, String> {
        self.next_all = Instant::now() + ALL_EVERY;
        let rows = match self.app.config.chat_store.list(&self.owner).await {
            Ok(rows) => rows,
            Err(error) => {
                eprintln!("openagents-web: sidebar events: {error}");
                self.catch_up = false;
                return Err(String::new());
            }
        };
        self.catch_up = false;
        self.dirty.clear();
        Ok(self.changed_rows(&rows))
    }

    /// The slots for `rows` changed since the cursor, and the baseline for
    /// the rest.
    fn changed_rows(&mut self, rows: &[Conversation]) -> String {
        let mut slots = String::new();
        for chat in rows {
            if chat.updated_unix >= self.cursor || self.shown.contains_key(&chat.id) {
                slots.push_str(&self.slot(chat));
            } else {
                self.shown.insert(chat.id.clone(), row_status(chat));
            }
        }
        slots
    }

    /// The chat's slot when its status differs from what the row shows.
    fn slot(&mut self, chat: &Conversation) -> String {
        let status = row_status(chat);
        if self.shown.get(&chat.id) == Some(&status) {
            return String::new();
        }
        self.shown.insert(chat.id.clone(), status);
        row_status_slot(chat, true).into_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "11111111-1111-4111-8111-111111111111";
    const B: &str = "22222222-2222-4222-8222-222222222222";

    #[test]
    fn the_connector_names_working_chats_and_swaps_nothing_itself() {
        let html = connector(42, [A, B]).into_string();
        assert!(
            html.contains(&format!(
                r#"sse-connect="/chats/events?after=42&amp;working={A},{B}""#
            )),
            "{html}"
        );
        assert!(html.contains(r#"hx-ext="sse""#));
        assert!(html.contains(r#"sse-swap="status" hx-swap="none""#));
        assert!(html.contains("hidden"));
        let quiet = connector(7, []).into_string();
        assert!(
            quiet.contains(r#"sse-connect="/chats/events?after=7""#),
            "{quiet}"
        );
    }

    #[test]
    fn working_ids_must_be_chat_ids() {
        assert_eq!(
            parse_working(&format!("{B},{A},../x,{A},not-a-uuid,")),
            vec![A.to_owned(), B.to_owned()]
        );
        assert!(parse_working("").is_empty());
    }

    const OWNER: &str = "0123456789abcdef0123456789abcdef";
    const OTHER: &str = "abcdef0123456789abcdef0123456789";

    fn app(directory: &tempfile::TempDir) -> App {
        let config = crate::Config::development(directory.path().join("tasks"));
        App(Arc::new(crate::Inner { config }))
    }

    fn chat(id: &str, owner: &str, updated_unix: u64) -> Conversation {
        Conversation {
            id: id.into(),
            owner: owner.into(),
            revision: 1,
            title: "A chat".into(),
            messages: vec![
                Message {
                    role: Role::User,
                    text: "Hello".into(),
                    request_id: Some(id.into()),
                },
                Message {
                    role: Role::Assistant,
                    text: String::new(),
                    request_id: Some(id.into()),
                },
            ],
            pending: Some(Pending {
                request_id: id.into(),
                started_unix: now(),
                job_id: None,
            }),
            requests: vec![Request {
                id: id.into(),
                digest: "d".repeat(64),
                outcome: Outcome::Pending,
                selection: None,
                cloud: None,
                reply: None,
            }],
            selection: None,
            updated_unix,
            pinned_unix: None,
            archived_unix: None,
        }
    }

    /// Ends the chat's answer with `outcome`, as the answer task does.
    async fn finish(app: &App, owner: &str, id: &str, outcome: Outcome) {
        let store = &app.config.chat_store;
        let loaded = store.load(owner, id).await.unwrap().unwrap();
        let mut next = loaded.conversation.clone();
        next.revision += 1;
        next.updated_unix = now();
        next.pending = None;
        next.requests[0].outcome = outcome;
        store.compare_and_swap(&loaded, &next).await.unwrap();
    }

    async fn next(watch: &mut Watch) -> String {
        tokio::time::timeout(Duration::from_secs(10), watch.next())
            .await
            .expect("a status in time")
            .expect("the connection is open")
    }

    #[tokio::test]
    async fn a_background_answer_clears_its_spinner_without_a_reload() {
        let directory = tempfile::tempdir().unwrap();
        let app = app(&directory);
        app.config
            .chat_store
            .create(&chat(A, OWNER, now()))
            .await
            .unwrap();
        app.config
            .chat_store
            .create(&chat(B, OWNER, now()))
            .await
            .unwrap();
        let mut watch = Watch::new(app.clone(), OWNER.into(), now(), vec![A.into(), B.into()]);
        // Still working, as drawn: nothing to send.
        assert_eq!(watch.read_dirty().await, Ok(String::new()));

        finish(&app, OWNER, A, Outcome::Answered).await;
        let slots = next(&mut watch).await;
        assert!(
            slots.contains(&format!(r#"id="chat-row-status-{A}""#)),
            "{slots}"
        );
        assert!(slots.contains(r#"hx-swap-oob="true""#), "{slots}");
        assert!(!slots.contains("data-status"), "{slots}");
        assert!(!slots.contains(B), "{slots}");

        finish(&app, OWNER, B, Outcome::Failed).await;
        let slots = next(&mut watch).await;
        assert!(
            slots.contains(&format!(r#"id="chat-row-status-{B}""#)),
            "{slots}"
        );
        assert!(slots.contains(r#"data-status="failed""#), "{slots}");
        assert!(!slots.contains(A), "{slots}");
    }

    #[tokio::test]
    async fn only_the_owners_chats_are_followed() {
        let directory = tempfile::tempdir().unwrap();
        let app = app(&directory);
        let mut watch = Watch::new(app.clone(), OWNER.into(), now(), Vec::new());
        app.config
            .chat_store
            .create(&chat(A, OTHER, now()))
            .await
            .unwrap();
        watch.drain();
        assert!(watch.dirty.is_empty());
        // Another visitor naming a chat id reads nothing of it.
        watch.dirty.insert(A.into());
        assert_eq!(watch.read_dirty().await, Ok(String::new()));
        assert!(watch.shown.is_empty());
    }

    #[tokio::test]
    async fn a_reconnect_resends_only_rows_changed_after_its_cursor() {
        let directory = tempfile::tempdir().unwrap();
        let app = app(&directory);
        let mut old = chat(A, OWNER, 100);
        old.pending = None;
        old.requests[0].outcome = Outcome::Failed;
        app.config.chat_store.create(&old).await.unwrap();
        app.config
            .chat_store
            .create(&chat(B, OWNER, now()))
            .await
            .unwrap();
        let mut watch = Watch::new(app.clone(), OWNER.into(), 1_000, Vec::new());
        assert!(watch.catch_up);
        let slots = next(&mut watch).await;
        assert!(
            slots.contains(&format!(r#"id="chat-row-status-{B}""#)),
            "{slots}"
        );
        assert!(slots.contains(r#"data-status="working""#), "{slots}");
        assert!(!slots.contains(A), "{slots}");
        assert_eq!(watch.shown.get(A), Some(&Some(ChatStatus::Failed)));
        assert!(watch.cursor >= now() - 2);
    }
}
