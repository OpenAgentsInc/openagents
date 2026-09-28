//! The Chats surface: saved Claude and Codex chats from every computer this
//! phone paired for reading, in one list, and a reader for one chat.
//!
//! Each computer pairs with a `coder-pair:` invitation from `coder pair`
//! (the read-only SESS observer in `coder-connect`). A host grant from the
//! Computers surface never admits a history read, so chats need their own
//! pairing. Reads run in the background; the host polls with `snapshot`.

use base64::Engine;
use coder_computers::cache::Cache;
use coder_connect::{Client, ConnectionCode, Observation, Query, RelayPolicy};
use coder_history::{CatalogRequest, Chat, Harness, TranscriptRequest};
use rust_native::input::InputRequest;
use rust_native::style::{Color, Space, Style, TextWeight};
use rust_native::{Activation, Axis, Element, Node, TextRole, ValidatedView, View};
use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use tokio::runtime::Handle;

const OBSERVE_LIMIT: Duration = Duration::from_secs(20);
/// Catalog pages read per computer, of up to 32 chats each.
const CATALOG_PAGES: usize = 4;
/// Transcript pages read per chat, of up to 32 KiB each.
const TRANSCRIPT_PAGES: usize = 64;
/// Messages shown for one chat: the newest ones.
const SHOWN_MESSAGES: usize = 120;
/// Text shown per message.
const MESSAGE_BYTES: usize = 2_000;
const SHOWN_CHATS: usize = 200;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Intent {
    AddComputer,
    Refresh,
    Open { computer: String, source: String },
    Back,
    Forget { computer: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Purpose {
    Pair,
    Name,
}

#[derive(Clone, Serialize, Deserialize)]
struct Saved {
    code: ConnectionCode,
    label: String,
}

enum Status {
    Loading,
    Ready,
    Failed(String),
}

struct Computer {
    saved: Saved,
    client: Result<Arc<Client>, String>,
    status: Status,
    chats: Vec<Chat>,
}

struct Message {
    role: String,
    text: String,
    markdown: bool,
}

struct Reading {
    computer: String,
    chat: Chat,
    messages: Vec<Message>,
    loading: bool,
    error: Option<String>,
}

#[derive(Default)]
struct State {
    computers: Vec<Computer>,
    reading: Option<Reading>,
    pairing: bool,
    notice: Option<String>,
    /// A computer just paired: ask for its name next.
    named: Option<String>,
    /// The saved pairings changed off the app thread.
    dirty: bool,
}

pub struct Chats {
    runtime: Handle,
    secret: SecretKey,
    store: Result<Cache, String>,
    state: Arc<Mutex<State>>,
    instance: String,
    revision: u64,
    current: Option<ValidatedView<Intent>>,
    input: Option<InputRequest<Purpose>>,
    tokens: u64,
}

fn lock(state: &Mutex<State>) -> MutexGuard<'_, State> {
    state.lock().unwrap_or_else(|poison| poison.into_inner())
}

impl Chats {
    pub fn new(
        runtime: Handle,
        secret: SecretKey,
        store: Result<Cache, String>,
        instance: String,
    ) -> Self {
        let saved: Vec<Saved> = match &store {
            Ok(cache) => cache.read("chats").ok().flatten().unwrap_or_default(),
            Err(_) => vec![],
        };
        let mut state = State::default();
        if let Err(error) = &store {
            state.notice = Some(format!("Chats can't be saved: {error}"));
        }
        state.computers = saved
            .into_iter()
            .map(|saved| Computer {
                client: client(&saved.code, secret),
                saved,
                status: Status::Loading,
                chats: vec![],
            })
            .collect();
        let mut chats = Self {
            runtime,
            secret,
            store,
            state: Arc::new(Mutex::new(state)),
            instance,
            revision: 0,
            current: None,
            input: None,
            tokens: 0,
        };
        chats.refresh();
        chats
    }

    pub fn input(&self) -> Option<&InputRequest<Purpose>> {
        self.input.as_ref()
    }

    /// The node the reader should keep in view.
    pub fn follow(&self) -> Option<&'static str> {
        lock(&self.state).reading.as_ref().map(|_| "chat-end")
    }

    pub fn loading(&self) -> bool {
        let state = lock(&self.state);
        state.pairing
            || state
                .computers
                .iter()
                .any(|c| matches!(c.status, Status::Loading))
            || state.reading.as_ref().is_some_and(|r| r.loading)
    }

    /// Read every computer's catalog again.
    pub fn refresh(&mut self) {
        let jobs: Vec<(String, Arc<Client>)> = {
            let mut state = lock(&self.state);
            state
                .computers
                .iter_mut()
                .filter_map(|computer| {
                    let client = computer.client.as_ref().ok()?.clone();
                    computer.status = Status::Loading;
                    Some((computer.saved.code.host.clone(), client))
                })
                .collect()
        };
        for (host, client) in jobs {
            let state = self.state.clone();
            self.runtime.spawn(async move {
                let result = catalog(&client).await;
                let mut state = lock(&state);
                if let Some(computer) = state
                    .computers
                    .iter_mut()
                    .find(|c| c.saved.code.host == host)
                {
                    match result {
                        Ok(chats) => {
                            computer.chats = chats;
                            computer.status = Status::Ready;
                        }
                        Err(error) => computer.status = Status::Failed(error),
                    }
                }
            });
        }
    }

    pub fn activate(&mut self, event: &Activation) {
        let Some(intent) = self
            .current
            .as_ref()
            .and_then(|view| view.activate(event).ok())
            .cloned()
        else {
            return;
        };
        match intent {
            Intent::AddComputer => self.ask(Purpose::Pair),
            Intent::Refresh => self.refresh(),
            Intent::Back => lock(&self.state).reading = None,
            Intent::Forget { computer } => {
                lock(&self.state)
                    .computers
                    .retain(|c| c.saved.code.host != computer);
                self.save();
            }
            Intent::Open { computer, source } => self.open(computer, source),
        }
    }

    fn ask(&mut self, purpose: Purpose) {
        self.tokens += 1;
        let (label, prompt, scan, max_bytes) = match purpose {
            Purpose::Pair => (
                "Chat invitation",
                "On the computer, run `coder pair` and keep it running. Scan its QR code or paste its coder-pair: string.",
                true,
                16 * 1024,
            ),
            Purpose::Name => (
                "Computer name",
                "Name this computer so you can tell its chats apart.",
                false,
                64,
            ),
        };
        self.input = Some(InputRequest {
            token: format!("chats-input-{}", self.tokens),
            purpose,
            label: label.into(),
            prompt: prompt.into(),
            scan,
            secret: false,
            max_bytes,
        });
    }

    pub fn cancel(&mut self, token: &str) {
        if self
            .input
            .as_ref()
            .is_some_and(|input| input.token == token)
        {
            self.input = None;
            lock(&self.state).named = None;
        }
    }

    pub fn submit(&mut self, token: &str, value: &str) {
        let Some(input) = self.input.take() else {
            return;
        };
        if input.accept(token, value).is_err() {
            self.input = Some(input);
            return;
        }
        match input.purpose {
            Purpose::Pair => self.pair(value.trim().to_owned(), None),
            Purpose::Name => {
                let name: String = value
                    .trim()
                    .chars()
                    .filter(|c| !c.is_control())
                    .take(64)
                    .collect();
                let mut state = lock(&self.state);
                if let Some(host) = state.named.take()
                    && !name.is_empty()
                    && let Some(computer) = state
                        .computers
                        .iter_mut()
                        .find(|c| c.saved.code.host == host)
                {
                    computer.saved.label = name;
                }
                drop(state);
                self.save();
            }
        }
    }

    /// Pair from a `coder-pair:` string. With `label`, the computer is named
    /// and no name is asked for.
    pub fn pair(&mut self, text: String, label: Option<String>) {
        let secret = self.secret;
        let state = self.state.clone();
        {
            let mut state = lock(&state);
            state.pairing = true;
            state.notice = None;
        }
        let handle = self.runtime.clone();
        self.runtime.spawn(async move {
            let code = if text.starts_with("coder-pair:") {
                coder_connect::pairing::redeem(&text, &secret, RelayPolicy::Production)
                    .await
                    .map_err(|error| error.to_string())
            } else {
                ConnectionCode::parse(text.as_bytes())
                    .map_err(|_| "That isn't a chat invitation. Run `coder pair` on the computer and scan its code.".to_string())
            };
            let mut guard = lock(&state);
            guard.pairing = false;
            let code = match code {
                Ok(code) => code,
                Err(error) => {
                    guard.notice = Some(format!("Pairing failed: {error}"));
                    return;
                }
            };
            let host = code.host.clone();
            let number = guard.computers.len() + 1;
            let named = label.is_some();
            let label = label.unwrap_or_else(|| {
                guard
                    .computers
                    .iter()
                    .find(|c| c.saved.code.host == host)
                    .map_or_else(|| format!("Computer {number}"), |c| c.saved.label.clone())
            });
            guard.computers.retain(|c| c.saved.code.host != host);
            let client = client(&code, secret);
            guard.computers.push(Computer {
                saved: Saved { code, label },
                client: client.clone(),
                status: Status::Loading,
                chats: vec![],
            });
            guard.named = (!named).then(|| host.clone());
            guard.dirty = true;
            drop(guard);
            if let Ok(client) = client {
                let state = state.clone();
                handle.spawn(async move {
                    let result = catalog(&client).await;
                    let mut state = lock(&state);
                    if let Some(computer) = state.computers.iter_mut().find(|c| c.saved.code.host == host) {
                        match result {
                            Ok(chats) => {
                                computer.chats = chats;
                                computer.status = Status::Ready;
                            }
                            Err(error) => computer.status = Status::Failed(error),
                        }
                    }
                });
            }
        });
    }

    /// Called on each packet: save a new pairing and ask for its name.
    pub fn settle(&mut self) {
        let (wants_name, dirty) = {
            let mut state = lock(&self.state);
            (state.named.is_some(), std::mem::take(&mut state.dirty))
        };
        if dirty {
            self.save();
        }
        if wants_name && self.input.is_none() {
            self.ask(Purpose::Name);
        }
    }

    fn save(&mut self) {
        let saved: Vec<Saved> = lock(&self.state)
            .computers
            .iter()
            .map(|c| c.saved.clone())
            .collect();
        if let Ok(cache) = &self.store
            && let Err(error) = cache.write("chats", &saved)
        {
            lock(&self.state).notice = Some(format!("Chats can't be saved: {error}"));
        }
    }

    fn open(&mut self, computer: String, source: String) {
        let found = {
            let state = lock(&self.state);
            state
                .computers
                .iter()
                .find(|c| c.saved.code.host == computer)
                .and_then(|c| {
                    let client = c.client.as_ref().ok()?.clone();
                    let chat = c
                        .chats
                        .iter()
                        .find(|chat| chat.source_id.as_deref() == Some(&source))?
                        .clone();
                    Some((client, chat))
                })
        };
        let Some((client, chat)) = found else { return };
        lock(&self.state).reading = Some(Reading {
            computer: computer.clone(),
            chat,
            messages: vec![],
            loading: true,
            error: None,
        });
        let state = self.state.clone();
        self.runtime.spawn(async move {
            let mut cursor = None;
            for _ in 0..TRANSCRIPT_PAGES {
                let request = TranscriptRequest {
                    source_id: source.clone(),
                    cursor: cursor.take(),
                    max_bytes: coder_history::MAX_PAGE_BYTES,
                };
                let page = match observe(&client, Query::Page(request)).await {
                    Ok(Observation::Page(page)) => page,
                    Ok(_) => {
                        let error = "The computer answered with the wrong page.".to_string();
                        set_error(&state, &computer, &source, error);
                        return;
                    }
                    Err(error) => {
                        set_error(&state, &computer, &source, error);
                        return;
                    }
                };
                let messages = messages(&page.chunks);
                let more = page.has_more;
                cursor = Some(page.next);
                let mut guard = lock(&state);
                let Some(reading) = guard.reading.as_mut().filter(|r| {
                    r.computer == computer && r.chat.source_id.as_deref() == Some(&source)
                }) else {
                    return;
                };
                reading.messages.extend(messages);
                let excess = reading.messages.len().saturating_sub(SHOWN_MESSAGES);
                reading.messages.drain(..excess);
                if !more {
                    reading.loading = false;
                    return;
                }
            }
            if let Some(reading) = lock(&state).reading.as_mut() {
                reading.loading = false;
            }
        });
    }

    pub fn render(&mut self) -> Option<serde_json::Value> {
        self.revision += 1;
        let root = {
            let state = lock(&self.state);
            match &state.reading {
                Some(reading) => reader(reading),
                None => catalog_view(&state),
            }
        };
        let view = View::new(self.instance.clone(), self.revision, root)
            .validate()
            .ok()?;
        let value = serde_json::to_value(view.view()).ok();
        self.current = Some(view);
        value
    }
}

fn set_error(state: &Mutex<State>, computer: &str, source: &str, error: String) {
    let mut guard = lock(state);
    if let Some(reading) = guard
        .reading
        .as_mut()
        .filter(|r| r.computer == computer && r.chat.source_id.as_deref() == Some(source))
    {
        reading.loading = false;
        reading.error = Some(error);
    }
}

fn client(code: &ConnectionCode, secret: SecretKey) -> Result<Arc<Client>, String> {
    Client::new_with_policy(code.clone(), secret, RelayPolicy::Production)
        .map(Arc::new)
        .map_err(|error| error.to_string())
}

async fn observe(client: &Client, query: Query) -> Result<Observation, String> {
    tokio::time::timeout(OBSERVE_LIMIT, client.observe(query))
        .await
        .map_err(|_| {
            "The computer did not answer. Keep `coder pair` running and refresh.".to_string()
        })?
        .map_err(|error| error.to_string())
}

async fn catalog(client: &Client) -> Result<Vec<Chat>, String> {
    let mut chats = vec![];
    let mut cursor = None;
    for _ in 0..CATALOG_PAGES {
        let request = CatalogRequest {
            cursor: cursor.take(),
            limit: coder_history::MAX_CATALOG_PAGE,
        };
        let Observation::Catalog(page) = observe(client, Query::Catalog(request)).await? else {
            return Err("The computer answered with the wrong page.".into());
        };
        chats.extend(page.entries);
        match page.next {
            Some(next) => cursor = Some(next),
            None => break,
        }
    }
    Ok(chats)
}

/// Readable messages from one page. A record whose chunks are all on the
/// page is projected in full; one split across pages shows its preview.
fn messages(chunks: &[coder_history::RecordChunk]) -> Vec<Message> {
    let mut out = vec![];
    let mut index = 0;
    while index < chunks.len() {
        let start = index;
        let record = chunks[start].record_offset;
        while index < chunks.len() && chunks[index].record_offset == record {
            index += 1;
        }
        let group = &chunks[start..index];
        let whole = group[0].offset == record && group.last().is_some_and(|c| c.complete);
        let full = whole
            .then(|| {
                let mut bytes = vec![];
                for chunk in group {
                    bytes.extend(
                        base64::engine::general_purpose::STANDARD
                            .decode(&chunk.raw_base64)
                            .ok()?,
                    );
                }
                coder_history::readable_record_full(&bytes)
            })
            .flatten();
        let Some(readable) = full.or_else(|| group.iter().rev().find_map(|c| c.readable.clone()))
        else {
            continue;
        };
        if readable.unknown || readable.text.trim().is_empty() {
            continue;
        }
        let role = readable
            .role
            .clone()
            .unwrap_or_else(|| readable.kind.clone());
        let markdown = matches!(readable.role.as_deref(), Some("user" | "assistant"));
        let mut text = readable.text.trim().to_owned();
        if text.len() > MESSAGE_BYTES {
            let mut end = MESSAGE_BYTES;
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            text.truncate(end);
            text.push('…');
        }
        let role = match readable.tool_name {
            Some(name) if !markdown => format!("{role} · {name}"),
            _ => role,
        };
        out.push(Message {
            role,
            text,
            markdown,
        });
    }
    out
}

const WHITE: Color = Color::rgb(255, 255, 255);
const GRAY: Color = Color::rgb(153, 153, 153);

fn catalog_view(state: &State) -> Node<Intent> {
    let mut children = vec![heading("chats-title", "Chats")];
    if let Some(notice) = &state.notice {
        children.push(status("chats-notice", notice));
    }
    if state.pairing {
        children.push(status("chats-pairing", "Pairing…"));
    }
    if state.computers.is_empty() {
        children.push(body(
            "chats-empty",
            "Read the Claude and Codex chats saved on your computers. On a computer, run `coder pair` and keep it running, then add it here.",
        ));
    }
    for (index, computer) in state.computers.iter().enumerate() {
        let expired = computer.saved.code.expires_at <= now();
        let line = match (&computer.client, &computer.status) {
            _ if expired => "Pairing expired. Add it again.".to_string(),
            (Err(error), _) => error.clone(),
            (_, Status::Loading) => "Loading chats…".to_string(),
            (_, Status::Ready) => match computer.chats.len() {
                1 => "1 chat".into(),
                n => format!("{n} chats"),
            },
            (_, Status::Failed(error)) => error.clone(),
        };
        children.push(row(
            &format!("computer-{index}"),
            vec![
                text(
                    &format!("computer-{index}-label"),
                    &computer.saved.label,
                    TextRole::Body,
                    WHITE,
                    true,
                ),
                status(&format!("computer-{index}-status"), &line),
                button(
                    &format!("computer-{index}-forget"),
                    "Forget",
                    Intent::Forget {
                        computer: computer.saved.code.host.clone(),
                    },
                ),
            ],
        ));
    }
    let mut all: Vec<(&Computer, &Chat)> = state
        .computers
        .iter()
        .flat_map(|computer| computer.chats.iter().map(move |chat| (computer, chat)))
        .filter(|(_, chat)| !chat.archived && chat.source_id.is_some())
        .collect();
    all.sort_by(|a, b| b.1.updated_at.cmp(&a.1.updated_at));
    let total = all.len();
    let rows: Vec<Node<Intent>> = all
        .into_iter()
        .take(SHOWN_CHATS)
        .enumerate()
        .map(|(index, (computer, chat))| {
            let harness = match chat.harness {
                Harness::Codex => "Codex",
                Harness::Claude => "Claude",
            };
            let mut detail = format!("{harness} · {}", computer.saved.label);
            if let Some(updated) = chat.updated_at.as_deref() {
                detail.push_str(" · ");
                detail.push_str(
                    &updated
                        .chars()
                        .take(16)
                        .collect::<String>()
                        .replace('T', " "),
                );
            }
            if chat.subagent {
                detail.push_str(" · subagent");
            }
            button(
                &format!("chat-{index}"),
                &format!("{}\n{detail}", chat.title),
                Intent::Open {
                    computer: computer.saved.code.host.clone(),
                    source: chat.source_id.clone().unwrap_or_default(),
                },
            )
        })
        .collect();
    if !rows.is_empty() {
        children.push(status(
            "chats-count",
            &if total > SHOWN_CHATS {
                format!("Newest {SHOWN_CHATS} of {total} chats")
            } else {
                format!("{total} chats")
            },
        ));
        children.push(Node {
            key: "chat-list".into(),
            style: Style::default(),
            element: Element::List {
                label: "Chats from your computers".into(),
                children: rows,
            },
        });
    }
    children.push(row(
        "chats-actions",
        vec![
            button("add-computer", "Add a computer", Intent::AddComputer),
            button("refresh", "Refresh", Intent::Refresh),
        ],
    ));
    page(children)
}

fn reader(reading: &Reading) -> Node<Intent> {
    let harness = match reading.chat.harness {
        Harness::Codex => "Codex",
        Harness::Claude => "Claude",
    };
    let mut header = vec![
        button("back", "Chats", Intent::Back),
        heading("chat-title", &reading.chat.title),
        status("chat-harness", harness),
    ];
    if let Some(error) = &reading.error {
        header.push(status("chat-error", error));
    }
    let mut rows: Vec<Node<Intent>> = reading
        .messages
        .iter()
        .enumerate()
        .map(|(index, message)| {
            stack(
                &format!("message-{index}"),
                vec![
                    status(&format!("message-{index}-role"), &message.role),
                    text(
                        &format!("message-{index}-text"),
                        &message.text,
                        if message.markdown {
                            TextRole::Markdown
                        } else {
                            TextRole::Code
                        },
                        WHITE,
                        false,
                    ),
                ],
            )
        })
        .collect();
    rows.push(status(
        "chat-end",
        if reading.loading {
            "Loading…"
        } else if reading.messages.is_empty() {
            "No readable messages."
        } else {
            "End of chat"
        },
    ));
    header.push(Node {
        key: "messages".into(),
        style: Style::default(),
        element: Element::List {
            label: "Messages".into(),
            children: rows,
        },
    });
    page(header)
}

fn page(children: Vec<Node<Intent>>) -> Node<Intent> {
    let mut node = stack("chats", children);
    node.style.gap = Some(Space::Sm);
    node.style.padding_top = Some(Space::Md);
    node.style.padding_end = Some(Space::Md);
    node.style.padding_start = Some(Space::Md);
    node
}

fn stack(key: &str, children: Vec<Node<Intent>>) -> Node<Intent> {
    Node {
        key: key.into(),
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
            intent,
        },
    }
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use coder_history::RecordChunk;

    fn chunk(record: u64, offset: u64, bytes: &[u8], complete: bool) -> RecordChunk {
        RecordChunk {
            id: format!("chunk-{offset}"),
            index: 0,
            record_offset: record,
            offset,
            end_offset: offset + bytes.len() as u64,
            raw_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
            complete,
            oversized: false,
            readable: None,
        }
    }

    #[test]
    fn a_record_split_across_chunks_is_read_in_full() {
        let user = br#"{"type":"user","message":{"role":"user","content":"Fix the build"}}
"#;
        let assistant = br#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Done: the linker flag was missing."}]}}
"#;
        let (first, second) = assistant.split_at(40);
        let offset = user.len() as u64;
        let chunks = vec![
            chunk(0, 0, user, true),
            chunk(offset, offset, first, false),
            chunk(offset, offset + 40, second, true),
        ];
        let messages = messages(&chunks);
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].text, "Fix the build");
        assert_eq!(messages[1].text, "Done: the linker flag was missing.");
        assert!(messages[1].markdown);
    }

    #[test]
    fn a_record_continued_from_an_earlier_page_uses_its_preview() {
        let mut tail = chunk(0, 10, b"tail of a record", true);
        tail.readable = Some(coder_history::Readable {
            kind: "message".into(),
            native_id: None,
            role: Some("assistant".into()),
            timestamp: None,
            tool_name: None,
            call_id: None,
            text: "preview".into(),
            text_truncated: true,
            unknown: false,
        });
        let messages = messages(&[tail]);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].text, "preview");
    }
}
