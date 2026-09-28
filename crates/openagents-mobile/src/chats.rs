//! The Chats surface: saved Claude and Codex chats from every computer this
//! phone paired for reading, in one list, and a reader for one chat.
//!
//! Each computer pairs with a `coder-pair:` invitation from `coder pair`
//! (the read-only SESS observer in `coder-connect`). A host grant from the
//! Computers surface never admits a history read, so chats need their own
//! pairing. Reads run in the background; the host polls with `snapshot`.

use crate::conversation::Conversation;
use coder_computers::cache::Cache;
use coder_connect::{Client, ConnectionCode, Observation, Query, RelayPolicy};
use coder_history::{CatalogRequest, Chat, Harness};
use rust_native::input::InputRequest;
use rust_native::style::{Color, Space, Style, TextWeight};
use rust_native::{Activation, Axis, Element, Node, TextRole, ValidatedView, View};
use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use tokio::runtime::Handle;

const OBSERVE_LIMIT: Duration = Duration::from_secs(20);
/// Catalog pages read per computer, of up to 32 chats each, newest first.
const CATALOG_PAGES: usize = 8;
/// Stop reading a computer's catalog once this many chats would show.
const CATALOG_WANTED: usize = 60;
const SHOWN_CHATS: usize = 200;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Intent {
    AddComputer,
    Refresh,
    Open { computer: String, source: String },
    Back,
    Earlier,
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
    /// The Computers host key of the same machine, when tailnet admission
    /// paired it: the Coder tab reads that host's task chats here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    host: Option<String>,
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

#[derive(Default)]
struct State {
    computers: Vec<Computer>,
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
    reading: Option<Conversation>,
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
            reading: None,
        };
        chats.refresh();
        chats
    }

    pub fn input(&self) -> Option<&InputRequest<Purpose>> {
        self.input.as_ref()
    }

    pub fn loading(&self) -> bool {
        let state = lock(&self.state);
        state.pairing
            || state
                .computers
                .iter()
                .any(|c| matches!(c.status, Status::Loading))
            || self.reading.as_ref().is_some_and(Conversation::loading)
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
            Intent::Back => self.reading = None,
            Intent::Earlier => {
                if let Some(reading) = &self.reading {
                    reading.earlier();
                }
            }
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
            Purpose::Pair => self.pair(value.trim().to_owned(), None, None),
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
    /// and no name is asked for; `host` links it to the machine's Computers
    /// host key.
    pub fn pair(&mut self, text: String, label: Option<String>, linked: Option<String>) {
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
                saved: Saved {
                    code,
                    label,
                    host: linked,
                },
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
        if let Some((client, chat)) = found {
            self.reading = Some(Conversation::open(self.runtime.clone(), client, chat));
        }
    }

    /// The saved chat of Coder task `task` on the machine whose Computers
    /// host key is `host`: its observer computer, client, and newest chat.
    pub fn coder_chat(&self, host: &str, task: &str) -> Option<(String, Arc<Client>, Chat)> {
        let state = lock(&self.state);
        let computer = state
            .computers
            .iter()
            .find(|c| c.saved.host.as_deref() == Some(host))?;
        let client = computer.client.as_ref().ok()?.clone();
        let chat = computer
            .chats
            .iter()
            .filter(|chat| {
                chat.harness == Harness::Coder && chat.native_id.as_deref() == Some(task)
            })
            .max_by(|a, b| a.updated_at.cmp(&b.updated_at))?
            .clone();
        Some((computer.saved.code.host.clone(), client, chat))
    }

    /// Read the catalog of the machine whose Computers host key is `host`
    /// again, as a new task's chat appears.
    pub fn refresh_linked(&mut self, host: &str) {
        let job = {
            let mut state = lock(&self.state);
            state
                .computers
                .iter_mut()
                .find(|c| {
                    c.saved.host.as_deref() == Some(host) && !matches!(c.status, Status::Loading)
                })
                .and_then(|computer| {
                    let client = computer.client.as_ref().ok()?.clone();
                    computer.status = Status::Loading;
                    Some((computer.saved.code.host.clone(), client))
                })
        };
        let Some((observer, client)) = job else {
            return;
        };
        let state = self.state.clone();
        self.runtime.spawn(async move {
            let result = catalog(&client).await;
            let mut state = lock(&state);
            if let Some(computer) = state
                .computers
                .iter_mut()
                .find(|c| c.saved.code.host == observer)
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

    pub fn runtime(&self) -> Handle {
        self.runtime.clone()
    }

    pub fn render(&mut self) -> Option<serde_json::Value> {
        self.revision += 1;
        let view = loop {
            let root = match &self.reading {
                Some(reading) => reader(reading),
                None => catalog_view(&lock(&self.state)),
            };
            match View::new(self.instance.clone(), self.revision, root).validate() {
                Ok(view) => break view,
                // A long chat can outgrow one view; keep its newest half.
                Err(_) if self.reading.is_some() => {
                    self.reading.as_ref()?.shrink();
                    if self.reading.as_ref()?.is_empty() {
                        return None;
                    }
                }
                Err(_) => return None,
            }
        };
        let value = serde_json::to_value(view.view()).ok();
        self.current = Some(view);
        value
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

/// The computer's newest chats, until enough would show. The host lists
/// newest first; a chat that moved between pages is kept once.
async fn catalog(client: &Client) -> Result<Vec<Chat>, String> {
    let mut chats: Vec<Chat> = vec![];
    let mut cursor = None;
    for _ in 0..CATALOG_PAGES {
        let request = CatalogRequest {
            cursor: cursor.take(),
            limit: coder_history::MAX_CATALOG_PAGE,
        };
        let page = match observe(client, Query::Catalog(request)).await {
            Ok(Observation::Catalog(page)) => page,
            Ok(_) => return Err("The computer answered with the wrong page.".into()),
            // A later page can fail when the chat list changed; keep what
            // arrived.
            Err(_) if !chats.is_empty() => break,
            Err(error) => return Err(error),
        };
        for chat in page.entries {
            if !chats.iter().any(|known| known.id == chat.id) {
                chats.push(chat);
            }
        }
        if chats.iter().filter(|chat| shown(chat)).count() >= CATALOG_WANTED {
            break;
        }
        match page.next {
            Some(next) => cursor = Some(next),
            None => break,
        }
    }
    Ok(chats)
}

/// Whether the list shows a chat: not archived, not a subagent's, and
/// readable.
fn shown(chat: &Chat) -> bool {
    !chat.archived && !chat.subagent && chat.source_id.is_some()
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
        .filter(|(_, chat)| shown(chat))
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
                Harness::Coder => "Coder",
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

fn reader(reading: &Conversation) -> Node<Intent> {
    let harness = match reading.chat.harness {
        Harness::Codex => "Codex",
        Harness::Claude => "Claude",
        Harness::Coder => "Coder",
    };
    page(vec![
        row(
            "chat-header",
            vec![
                button("back", "Chats", Intent::Back),
                status("chat-harness", harness),
            ],
        ),
        heading("chat-title", &reading.chat.title),
        reading.transcript("chat", Intent::Earlier, None),
    ])
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
