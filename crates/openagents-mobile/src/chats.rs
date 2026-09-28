//! The Chats surface: saved Claude and Codex chats from every computer this
//! phone paired for reading, in one list, and a reader for one chat.
//!
//! Each computer pairs with a `coder-pair:` invitation from `coder pair`
//! (the read-only SESS observer in `coder-connect`). A host grant from the
//! Computers surface never admits a history read, so chats need their own
//! pairing. Reads run in the background; the host polls with `snapshot`.

use crate::conversation::Conversation;
use crate::transcripts::Transcripts;
use coder_computers::cache::Cache;
use coder_connect::direct::Change;
use coder_connect::protocol::Route;
use coder_connect::{Client, ConnectionCode, Observation, Query, RelayPolicy};
use coder_history::{CatalogCursor, CatalogPage, CatalogRequest, Chat, Harness};
use rust_native::input::InputRequest;
use rust_native::layout::source;
use rust_native::style::{Color, Space, Style, TextWeight};
use rust_native::{Activation, Axis, Element, Node, TextRole, ValidatedView, View};
use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use tokio::runtime::Handle;
use tokio::sync::broadcast::error::RecvError;

const OBSERVE_LIMIT: Duration = Duration::from_secs(20);
/// Catalog pages read per computer, of up to 32 chats each, newest first.
const CATALOG_PAGES: usize = 8;
/// Stop reading a computer's catalog once this many chats would show.
const CATALOG_WANTED: usize = 60;
const SHOWN_CHATS: usize = 200;
/// The prefix of each computer's kept chat list; its observer key follows.
const CATALOG_KEY: &str = "chats-catalog-";
/// The most chats kept per computer across a relaunch, newest first.
const KEPT_CHATS: usize = 160;
/// The most plaintext one kept chat list may take.
const KEPT_BYTES: usize = 150 * 1024;
/// Keep an open chat's transcript at most this often while it changes.
const KEEP_EVERY: u64 = 5;

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

/// What became of a read of a catalog's first page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Head {
    Running,
    Read,
    /// It failed or was never started; ask again.
    Failed,
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
    /// The machine's tailnet listener, where chats read directly when it
    /// answers; the relay otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    direct: Option<std::net::SocketAddr>,
}

enum Status {
    /// Nothing to show yet: the first read is running.
    Loading,
    /// The chats kept from the last read show while the computer is read
    /// again.
    Refreshing,
    Ready,
    Failed(String),
}

struct Computer {
    saved: Saved,
    client: Result<Arc<Client>, String>,
    /// Reads the newest chats when the computer says its task list changed.
    watch: Option<tokio::task::AbortHandle>,
    status: Status,
    chats: Vec<Chat>,
    /// Reads of the catalog's first page started and finished, and whether
    /// one is running.
    heads: u64,
    heads_done: u64,
    head_running: bool,
    /// Changes with each catalog read started; an older read stops at its
    /// next page.
    generation: u64,
}

impl Computer {
    fn new(
        saved: Saved,
        client: Result<Arc<Client>, String>,
        watch: Option<tokio::task::AbortHandle>,
        chats: Vec<Chat>,
    ) -> Self {
        Self {
            saved,
            client,
            watch,
            status: if chats.is_empty() {
                Status::Loading
            } else {
                Status::Refreshing
            },
            chats,
            heads: 0,
            heads_done: 0,
            head_running: false,
            generation: 0,
        }
    }

    /// Start a catalog read: the chats kept show while it runs.
    fn start(&mut self) -> u64 {
        self.generation += 1;
        self.status = if self.chats.is_empty() {
            Status::Loading
        } else {
            Status::Refreshing
        };
        self.generation
    }
}

impl Drop for Computer {
    fn drop(&mut self) {
        if let Some(watch) = &self.watch {
            watch.abort();
        }
    }
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
    /// Computers whose chat list changed since it was last kept.
    changed: std::collections::BTreeSet<String>,
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
    /// The host's transcript layout reads a chat's rows from Rust.
    pulled: bool,
    /// Chats' transcripts as last shown, so a chat opens at once.
    transcripts: Transcripts,
    /// The open chat's key in `transcripts`, and the version and time it
    /// was last kept.
    kept: Option<(String, u64, u64)>,
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
        let shared = Arc::new(Mutex::new(State::default()));
        // Each computer's chats as last read show at once; the reads below
        // bring them up to date.
        state.computers = saved
            .into_iter()
            .map(|saved| {
                let client = client(&saved, secret);
                let kept = store
                    .as_ref()
                    .ok()
                    .and_then(|cache| kept(cache, &saved.code.host))
                    .unwrap_or_default();
                Computer::new(
                    saved.clone(),
                    client.clone(),
                    watch(&runtime, &shared, &saved, &client),
                    kept,
                )
            })
            .collect();
        *lock(&shared) = state;
        let mut chats = Self {
            runtime,
            secret,
            store,
            state: shared,
            instance,
            revision: 0,
            current: None,
            input: None,
            tokens: 0,
            reading: None,
            pulled: false,
            transcripts: Transcripts::chats(None),
            kept: None,
        };
        chats.refresh();
        chats
    }

    pub fn input(&self) -> Option<&InputRequest<Purpose>> {
        self.input.as_ref()
    }

    /// Keep each opened chat's transcript in `transcripts`, which survives
    /// a relaunch.
    pub fn with_transcripts(mut self, transcripts: Transcripts) -> Self {
        self.transcripts = transcripts;
        self
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
        let jobs: Vec<(String, u64, Arc<Client>)> = {
            let mut state = lock(&self.state);
            state
                .computers
                .iter_mut()
                .filter_map(|computer| {
                    let client = computer.client.as_ref().ok()?.clone();
                    let generation = computer.start();
                    Some((computer.saved.code.host.clone(), generation, client))
                })
                .collect()
        };
        for (host, generation, client) in jobs {
            let state = self.state.clone();
            self.runtime.spawn(async move {
                read_catalog(&state, &host, generation, |after| {
                    catalog_page(&client, after)
                })
                .await;
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
            Intent::Back => {
                self.keep(true);
                self.reading = None;
                self.kept = None;
            }
            Intent::Earlier => {
                if let Some(reading) = &self.reading {
                    reading.earlier();
                }
            }
            Intent::Forget { computer } => {
                lock(&self.state)
                    .computers
                    .retain(|c| c.saved.code.host != computer);
                if let Ok(cache) = &self.store {
                    let _ = cache.erase(&format!("{CATALOG_KEY}{computer}"));
                }
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
            Purpose::Pair => self.pair(value.trim().to_owned(), None, None, None),
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
    pub fn pair(
        &mut self,
        text: String,
        label: Option<String>,
        linked: Option<String>,
        direct: Option<std::net::SocketAddr>,
    ) {
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
            // Paired again: its chats keep showing while they are read.
            let known = guard
                .computers
                .iter_mut()
                .find(|c| c.saved.code.host == host)
                .map(|c| std::mem::take(&mut c.chats))
                .unwrap_or_default();
            guard.computers.retain(|c| c.saved.code.host != host);
            let saved = Saved {
                code,
                label,
                host: linked,
                direct,
            };
            let client = client(&saved, secret);
            let mut computer = Computer::new(
                saved.clone(),
                client.clone(),
                watch(&handle, &state, &saved, &client),
                known,
            );
            let generation = computer.start();
            guard.computers.push(computer);
            guard.named = (!named).then(|| host.clone());
            guard.dirty = true;
            drop(guard);
            if let Ok(client) = client {
                let state = state.clone();
                handle.spawn(async move {
                    read_catalog(&state, &host, generation, |after| catalog_page(&client, after))
                        .await;
                });
            }
        });
    }

    /// Called on each packet: save a new pairing and each chat list that
    /// changed, and ask for a new pairing's name.
    pub fn settle(&mut self) {
        let (wants_name, dirty, lists) = {
            let mut state = lock(&self.state);
            let changed = std::mem::take(&mut state.changed);
            let lists: Vec<(String, Vec<Chat>)> = state
                .computers
                .iter()
                .filter(|c| changed.contains(&c.saved.code.host))
                .map(|c| (c.saved.code.host.clone(), c.chats.clone()))
                .collect();
            (
                state.named.is_some(),
                std::mem::take(&mut state.dirty),
                lists,
            )
        };
        if dirty {
            self.save();
        }
        if let Ok(cache) = &self.store {
            for (observer, chats) in lists {
                keep(cache, &observer, chats);
            }
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
            // The chat as last shown, at once, while its computer is read
            // for what changed since.
            let key = transcript_key(&computer, &chat.id);
            let cached = self
                .transcripts
                .get(&key)
                .filter(|cached| cached.chat.id == chat.id);
            self.reading = Some(Conversation::resume(
                self.runtime.clone(),
                client,
                chat,
                cached,
            ));
            self.kept = Some((key, 0, 0));
        }
    }

    /// Keep the open chat's transcript for its next opening: when `now`, or
    /// when it changed and was last kept a while ago.
    fn keep(&mut self, now: bool) {
        let (Some(reading), Some((key, version, at))) = (&self.reading, self.kept.as_mut()) else {
            return;
        };
        let current = reading.version();
        let time = unix_now();
        if current == *version || !now && time.saturating_sub(*at) < KEEP_EVERY {
            return;
        }
        if let Some(cached) = reading.cached() {
            *version = current;
            *at = time;
            let key = key.clone();
            self.transcripts.put(&key, cached);
        }
    }

    /// Whether the computer whose Computers host key is `host` tells this
    /// phone when its chat list changes: it reads over a direct connection
    /// that has read its catalog, so a timed read of its newest chats can
    /// wait longer.
    pub fn nudged(&self, host: &str) -> bool {
        lock(&self.state).computers.iter().any(|c| {
            c.saved.host.as_deref() == Some(host)
                && c.client
                    .as_ref()
                    .is_ok_and(|client| client.route() == Route::Direct)
                && (c.heads_done > 0 || matches!(c.status, Status::Ready))
        })
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

    /// The observer client of the machine whose Computers host key is
    /// `host`, whether or not its catalog has been read.
    pub fn coder_client(&self, host: &str) -> Option<Arc<Client>> {
        lock(&self.state)
            .computers
            .iter()
            .find(|c| c.saved.host.as_deref() == Some(host))?
            .client
            .as_ref()
            .ok()
            .cloned()
    }

    /// Read the chats of the machine whose Computers host key is `host`
    /// directly at its tailnet listener `address` when it answers.
    pub fn set_direct(&mut self, host: &str, address: std::net::SocketAddr) {
        let mut state = lock(&self.state);
        let mut changed = false;
        for computer in state
            .computers
            .iter_mut()
            .filter(|c| c.saved.host.as_deref() == Some(host))
        {
            if let Ok(client) = &computer.client {
                client.set_direct(Some(address));
            }
            if computer.saved.direct != Some(address) {
                computer.saved.direct = Some(address);
                changed = true;
            }
        }
        state.dirty |= changed;
    }

    /// Read the first page of the catalog of the machine whose Computers
    /// host key is `host`, its newest chats, and merge it into the list, as
    /// a Coder task's next turn appears. Returns the read's number, or
    /// `None` when none started because one is running or no pairing is
    /// linked; [`Chats::head_read`] says when it finished.
    pub fn refresh_head(&mut self, host: &str) -> Option<u64> {
        let (observer, client, round) = {
            let mut state = lock(&self.state);
            let computer = state
                .computers
                .iter_mut()
                .find(|c| c.saved.host.as_deref() == Some(host) && !c.head_running)?;
            let client = computer.client.as_ref().ok()?.clone();
            computer.head_running = true;
            computer.heads += 1;
            (computer.saved.code.host.clone(), client, computer.heads)
        };
        let state = self.state.clone();
        self.runtime.spawn(async move {
            let result = observe(&client, &|route| {
                Query::Catalog(CatalogRequest {
                    cursor: None,
                    limit: head_limit(route),
                })
            })
            .await;
            let mut state = lock(&state);
            let Some(computer) = state
                .computers
                .iter_mut()
                .find(|c| c.saved.code.host == observer)
            else {
                return;
            };
            computer.head_running = false;
            if let Ok(Observation::Catalog(page)) = result {
                merge(&mut computer.chats, &page.entries);
                computer.heads_done = computer.heads_done.max(round);
                state.changed.insert(observer);
            }
        });
        Some(round)
    }

    /// What became of first-page read `round` of the machine whose
    /// Computers host key is `host`.
    pub fn head(&self, host: &str, round: u64) -> Head {
        let state = lock(&self.state);
        let Some(computer) = state
            .computers
            .iter()
            .find(|c| c.saved.host.as_deref() == Some(host))
        else {
            return Head::Failed;
        };
        if computer.heads_done >= round {
            Head::Read
        } else if computer.head_running && computer.heads == round {
            Head::Running
        } else {
            Head::Failed
        }
    }

    /// Whether a current chat pairing is linked to the machine whose
    /// Computers host key is `host`. A pairing made before the link, or one
    /// near its end, needs tailnet admission again.
    pub fn linked(&self, host: &str, until: u64) -> bool {
        lock(&self.state)
            .computers
            .iter()
            .any(|c| c.saved.host.as_deref() == Some(host) && c.saved.code.expires_at > until)
    }

    pub fn runtime(&self) -> Handle {
        self.runtime.clone()
    }

    /// Publish a chat's rows for the host's transcript layout instead of
    /// listing them in the view, so a long chat never outgrows one view.
    pub fn with_pulled_transcripts(mut self, pulled: bool) -> Self {
        self.pulled = pulled;
        self
    }

    pub fn render(&mut self) -> Option<serde_json::Value> {
        self.keep(false);
        self.revision += 1;
        let view = loop {
            let mut root = match &self.reading {
                Some(reading) => reader(reading),
                None => catalog_view(&lock(&self.state)),
            };
            let detached = !self.pulled || source::detach(&mut root, &self.instance).is_ok();
            match View::new(self.instance.clone(), self.revision, root)
                .validate()
                .ok()
                .filter(|_| detached)
            {
                Some(view) => break view,
                // A long chat can outgrow one view; keep its newest half.
                None if self.reading.is_some() => {
                    if !self.reading.as_ref()?.shrink() {
                        return None;
                    }
                }
                None => return None,
            }
        };
        let value = serde_json::to_value(view.view()).ok();
        self.current = Some(view);
        value
    }
}

fn client(saved: &Saved, secret: SecretKey) -> Result<Arc<Client>, String> {
    let client = Client::new_with_policy(saved.code.clone(), secret, RelayPolicy::Production)
        .map_err(|error| error.to_string())?;
    client.set_direct(saved.direct);
    Ok(Arc::new(client))
}

/// The newest chats one read asks for: a relay page, or on a direct
/// connection a page large enough for the whole list at once.
fn head_limit(route: Route) -> u16 {
    route.limits().catalog_page
}

/// Follow the computer's nudges: when its task list changes, read its
/// newest chats and merge them, as a Coder task's next turn appears.
fn watch(
    runtime: &Handle,
    state: &Arc<Mutex<State>>,
    saved: &Saved,
    client: &Result<Arc<Client>, String>,
) -> Option<tokio::task::AbortHandle> {
    let client = client.as_ref().ok()?.clone();
    let mut changes = client.changes();
    let state = Arc::downgrade(state);
    let observer = saved.code.host.clone();
    let task = runtime.spawn(async move {
        loop {
            match changes.recv().await {
                Ok(Change::Catalog) => {}
                Ok(Change::Source(_)) | Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => return,
            }
            let result = observe(&client, &|route| {
                Query::Catalog(CatalogRequest {
                    cursor: None,
                    limit: head_limit(route),
                })
            })
            .await;
            let Some(state) = state.upgrade() else { return };
            let mut state = lock(&state);
            if let (Ok(Observation::Catalog(page)), Some(computer)) = (
                result,
                state
                    .computers
                    .iter_mut()
                    .find(|c| c.saved.code.host == observer),
            ) {
                merge(&mut computer.chats, &page.entries);
                state.changed.insert(observer.clone());
            }
        }
    });
    Some(task.abort_handle())
}

async fn observe(
    client: &Client,
    make: &(dyn Fn(Route) -> Query + Sync),
) -> Result<Observation, String> {
    tokio::time::timeout(OBSERVE_LIMIT, client.observe_with(make))
        .await
        .map_err(|_| {
            "The computer did not answer. Keep `coder pair` running and refresh.".to_string()
        })?
        .map_err(|error| error.to_string())
}

/// Read the catalog of computer `observer` with `fetch` into its list: each
/// page shows as it arrives, over the chats kept from the last read, and a
/// read `generation` no longer names stops at its next page.
async fn read_catalog<F, Fut>(state: &Arc<Mutex<State>>, observer: &str, generation: u64, fetch: F)
where
    F: FnMut(Option<CatalogCursor>) -> Fut,
    Fut: std::future::Future<Output = Result<CatalogPage, String>>,
{
    let current = |state: &mut State| -> Option<usize> {
        state
            .computers
            .iter()
            .position(|c| c.saved.code.host == observer && c.generation == generation)
    };
    let mut merged = 0;
    let result = catalog_pages(fetch, |chats| {
        let mut state = lock(state);
        let Some(index) = current(&mut state) else {
            return false;
        };
        let computer = &mut state.computers[index];
        merge(&mut computer.chats, &chats[merged..]);
        merged = chats.len();
        if matches!(computer.status, Status::Loading) && !computer.chats.is_empty() {
            computer.status = Status::Refreshing;
        }
        state.changed.insert(observer.to_owned());
        true
    })
    .await;
    let mut state = lock(state);
    let Some(index) = current(&mut state) else {
        return;
    };
    let computer = &mut state.computers[index];
    match result {
        Ok((fresh, complete)) => {
            prune(&mut computer.chats, &fresh, complete);
            computer.status = Status::Ready;
            state.changed.insert(observer.to_owned());
        }
        Err(error) => computer.status = Status::Failed(error),
    }
}

/// Merge `fresh` chats into `list`: a chat already listed is replaced in
/// place, a new one is added. The view orders them.
fn merge(list: &mut Vec<Chat>, fresh: &[Chat]) {
    for chat in fresh {
        match list.iter_mut().find(|known| known.id == chat.id) {
            Some(known) => known.clone_from(chat),
            None => list.push(chat.clone()),
        }
    }
}

/// After a whole read of `fresh` chats, drop the kept chats the computer no
/// longer lists: those within the span the read covered, or every one when
/// it read the whole catalog. A chat newer than the read, as a nudge brings
/// while it runs, stays.
fn prune(list: &mut Vec<Chat>, fresh: &[Chat], complete: bool) {
    let newest = fresh
        .iter()
        .map(|c| c.updated_at.as_deref())
        .max()
        .flatten();
    let oldest = fresh
        .iter()
        .map(|c| c.updated_at.as_deref())
        .min()
        .flatten();
    list.retain(|chat| {
        fresh.iter().any(|known| known.id == chat.id)
            || chat.updated_at.as_deref() > newest
            || !complete && chat.updated_at.as_deref() < oldest
    });
}

/// Newest first, then by ID, so rows keep their places across reads.
fn order(a: &Chat, b: &Chat) -> std::cmp::Ordering {
    b.updated_at
        .cmp(&a.updated_at)
        .then_with(|| a.id.cmp(&b.id))
}

/// The chats kept for computer `observer`.
fn kept(cache: &Cache, observer: &str) -> Option<Vec<Chat>> {
    cache
        .read(&format!("{CATALOG_KEY}{observer}"))
        .ok()
        .flatten()
}

/// Keep computer `observer`'s newest chats for the next launch. An empty
/// list is not kept, so a relaunch never shows a computer as empty while a
/// read that found nothing is still possible to redo.
fn keep(cache: &Cache, observer: &str, mut chats: Vec<Chat>) {
    if chats.is_empty() {
        return;
    }
    chats.sort_by(order);
    chats.truncate(KEPT_CHATS);
    while serde_json::to_vec(&chats).map_or(0, |bytes| bytes.len()) > KEPT_BYTES {
        chats.truncate(chats.len() * 3 / 4);
    }
    let _ = cache.write(&format!("{CATALOG_KEY}{observer}"), &chats);
}

/// The store key of chat `id` of computer `observer`: a 64-bit FNV-1a
/// digest in hex, since a chat ID may hold any character. A kept
/// transcript names its chat, which is checked on reading.
fn transcript_key(observer: &str, id: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in observer.bytes().chain(*b"\n").chain(id.bytes()) {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}")
}

fn unix_now() -> u64 {
    now()
}

/// One catalog page after `after`.
async fn catalog_page(
    client: &Client,
    after: Option<CatalogCursor>,
) -> Result<CatalogPage, String> {
    let make = |route: Route| {
        Query::Catalog(CatalogRequest {
            cursor: after.clone(),
            limit: head_limit(route),
        })
    };
    match observe(client, &make).await? {
        Observation::Catalog(page) => Ok(page),
        _ => Err("The computer answered with the wrong page.".into()),
    }
}

/// Read catalog pages with `fetch`, newest first, until enough chats would
/// show. `arrived` sees the chats read so far after each page and returns
/// whether to read on.
async fn catalog_pages<F, Fut>(
    mut fetch: F,
    mut arrived: impl FnMut(&[Chat]) -> bool,
) -> Result<(Vec<Chat>, bool), String>
where
    F: FnMut(Option<CatalogCursor>) -> Fut,
    Fut: std::future::Future<Output = Result<CatalogPage, String>>,
{
    let mut chats: Vec<Chat> = vec![];
    let mut cursor = None;
    let mut complete = false;
    for _ in 0..CATALOG_PAGES {
        let page = match fetch(cursor.take()).await {
            Ok(page) => page,
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
        if !arrived(&chats) || chats.iter().filter(|chat| shown(chat)).count() >= CATALOG_WANTED {
            break;
        }
        match page.next {
            Some(next) => cursor = Some(next),
            None => {
                complete = true;
                break;
            }
        }
    }
    Ok((chats, complete))
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
            (_, Status::Ready | Status::Refreshing) => match computer.chats.len() {
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
    all.sort_by(|a, b| order(a.1, b.1));
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
                Harness::OpenCode => "OpenCode",
                Harness::Devin => "Devin",
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
        Harness::OpenCode => "OpenCode",
        Harness::Devin => "Devin",
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
        reading.transcript("chat", Intent::Earlier, &[], None),
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
            icon: None,
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
mod speed {
    //! Time to first rows, against a fake computer whose every catalog page
    //! takes [`PAGE`] under a paused clock, so the numbers are exact.
    use super::*;
    use coder_history::SourceStatus;
    use std::time::Duration;

    /// One catalog page's round trip, as over the relay.
    const PAGE: Duration = Duration::from_millis(250);
    const OBSERVER: &str = "observer";

    /// Chat `n`: newest first; three in four are subagents' and do not show.
    fn chat(n: usize) -> Chat {
        Chat {
            id: format!("chat-{n}"),
            harness: Harness::Claude,
            native_id: None,
            title: format!("Chat {n}"),
            title_truncated: false,
            updated_at: Some(format!("2026-09-{:02}T00:00:00Z", 28 - n / 20)),
            archived: false,
            subagent: !n.is_multiple_of(4),
            source_id: Some(format!("source-{n}")),
            status: SourceStatus::Available,
        }
    }

    async fn fake(after: Option<CatalogCursor>) -> Result<CatalogPage, String> {
        tokio::time::sleep(PAGE).await;
        let start = after.map_or(0, |c| c.after.parse::<usize>().unwrap());
        let end = start + 32;
        Ok(CatalogPage {
            snapshot: "s".into(),
            entries: (start..end).map(chat).collect(),
            next: (end < 32 * 10).then(|| CatalogCursor {
                snapshot: "s".into(),
                after: end.to_string(),
            }),
            notices: vec![],
        })
    }

    /// A pairing of this device's key `secret` with a computer whose relay
    /// no test reaches, signed as a host signs one.
    pub(crate) fn code(secret: &SecretKey) -> ConnectionCode {
        use coder_connect::protocol::{CONNECTION, GRANT, pubkey, random_id, seal};
        use coder_connect::{Grant, SourceKind, SourceScope};
        let host = SecretKey::from_byte_array([0x42; 32]).unwrap();
        let now = now();
        let expires_at = now + 7 * 24 * 60 * 60;
        let grant = Grant {
            v: GRANT.into(),
            requires: vec![],
            grant: random_id(),
            host: pubkey(&host),
            client: pubkey(secret),
            relay: "wss://relay.invalid".into(),
            sources: vec![SourceScope {
                id: random_id(),
                label: "Claude".into(),
                kind: SourceKind::Claude,
            }],
            issued_at: now,
            expires_at,
        };
        let authorization = seal(
            &grant,
            GRANT,
            &host,
            &grant.client,
            &grant.grant,
            now,
            expires_at,
        )
        .unwrap();
        ConnectionCode {
            v: CONNECTION.into(),
            requires: vec![],
            host: grant.host.clone(),
            client: grant.client.clone(),
            relay: grant.relay.clone(),
            grant: grant.grant.clone(),
            sources: grant.sources.clone(),
            expires_at,
            authorization,
        }
    }

    fn computer() -> Computer {
        let mut code = code(&SecretKey::from_byte_array([0x11; 32]).unwrap());
        code.host = OBSERVER.into();
        Computer::new(
            Saved {
                code,
                label: "Desk".into(),
                host: None,
                direct: None,
            },
            Err("no client in tests".into()),
            None,
            vec![],
        )
    }

    fn visible(state: &Arc<Mutex<State>>) -> usize {
        lock(state).computers[0]
            .chats
            .iter()
            .filter(|chat| shown(chat))
            .count()
    }

    /// A computer's first catalog page shows as soon as it arrives; later
    /// pages follow in the background. Before, its chats showed only after
    /// every page it read: eight sequential pages here, 2.01 s.
    #[test]
    fn the_first_catalog_page_shows_before_the_rest_arrive() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .start_paused(true)
            .build()
            .unwrap();
        runtime.block_on(async {
            let state = Arc::new(Mutex::new(State::default()));
            lock(&state).computers.push(computer());
            let started = tokio::time::Instant::now();
            let reading = tokio::spawn({
                let state = state.clone();
                async move { read_catalog(&state, OBSERVER, 0, fake).await }
            });
            while visible(&state) == 0 {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            let first = started.elapsed();
            reading.await.unwrap();
            let all = started.elapsed();
            eprintln!("chats list: first rows after {first:?}, every page after {all:?}");
            assert!(first <= PAGE + Duration::from_millis(10), "{first:?}");
            assert!(all >= PAGE * 8, "{all:?}");
            assert_eq!(visible(&state), 64);
            assert!(matches!(lock(&state).computers[0].status, Status::Ready));
        });
    }

    /// A read a newer one replaced stops at its next page and changes
    /// nothing after.
    #[test]
    fn a_replaced_read_stops() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .start_paused(true)
            .build()
            .unwrap();
        runtime.block_on(async {
            let state = Arc::new(Mutex::new(State::default()));
            lock(&state).computers.push(computer());
            let reading = tokio::spawn({
                let state = state.clone();
                async move { read_catalog(&state, OBSERVER, 0, fake).await }
            });
            tokio::time::sleep(PAGE + Duration::from_millis(1)).await;
            let shown = visible(&state);
            assert_eq!(shown, 8);
            lock(&state).computers[0].start();
            reading.await.unwrap();
            assert_eq!(visible(&state), shown);
            assert!(matches!(
                lock(&state).computers[0].status,
                Status::Refreshing
            ));
        });
    }

    /// A read merges by ID without moving rows, keeps a chat newer than the
    /// read, and drops one the computer no longer lists.
    #[test]
    fn a_read_merges_in_place_and_drops_what_vanished() {
        let mut list = vec![chat(0), chat(4), chat(8)];
        let mut renamed = chat(4);
        renamed.title = "Renamed".into();
        let mut newer = chat(100);
        newer.updated_at = Some("2026-09-29T00:00:00Z".into());
        list.push(newer.clone());
        merge(&mut list, &[renamed.clone(), chat(0), chat(12)]);
        assert_eq!(list[1].title, "Renamed");
        prune(&mut list, &[renamed, chat(0), chat(12)], true);
        let ids: Vec<&str> = list.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["chat-0", "chat-4", "chat-100", "chat-12"]);
        let mut sorted = list.clone();
        sorted.sort_by(order);
        let ids: Vec<&str> = sorted.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["chat-100", "chat-0", "chat-12", "chat-4"]);
    }

    fn render(chats: &mut Chats) -> String {
        serde_json::to_string(&chats.render().unwrap()).unwrap()
    }

    fn listed(view: &str) -> usize {
        view.matches("\"key\":\"chat-").count()
            - usize::from(view.contains("\"key\":\"chat-list\""))
    }

    /// On relaunch the Chats list paints the chats kept from the last read
    /// in its first view, before any computer answers. Before, the first
    /// view showed "Loading chats…" and no rows until every catalog page
    /// arrived.
    #[test]
    fn a_relaunch_paints_the_kept_chats_list_at_once() {
        let dir = tempfile::tempdir().unwrap();
        let secret = SecretKey::from_byte_array([0x11; 32]).unwrap();
        // A runtime that is never driven: no read reaches a computer.
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let store = || Cache::open(&dir.path().join("chats"), &secret);
        let saved = vec![Saved {
            code: code(&secret),
            label: "Desk".into(),
            host: None,
            direct: None,
        }];
        store().unwrap().write("chats", &saved).unwrap();
        let observer = saved[0].code.host.clone();

        let mut first = Chats::new(runtime.handle().clone(), secret, store(), "chats:t".into());
        let view = render(&mut first);
        assert!(view.contains("Loading chats…"));
        assert_eq!(listed(&view), 0);
        assert!(first.loading());
        // The computer's read arrives.
        {
            let mut state = lock(&first.state);
            let computer = &mut state.computers[0];
            merge(&mut computer.chats, &(0..32).map(chat).collect::<Vec<_>>());
            computer.status = Status::Ready;
            state.changed.insert(observer.clone());
        }
        first.settle();
        drop(first);

        let started = std::time::Instant::now();
        let mut again = Chats::new(runtime.handle().clone(), secret, store(), "chats:t".into());
        let view = render(&mut again);
        let elapsed = started.elapsed();
        eprintln!(
            "chats list on relaunch: {} rows in the first view, {elapsed:?}",
            listed(&view)
        );
        assert!(!view.contains("Loading chats…"), "{view}");
        assert!(view.contains("8 chats"), "{view}");
        assert_eq!(listed(&view), 8);
        // The read runs quietly behind the kept rows.
        assert!(!again.loading());
    }

    /// Opening a chat again, after a relaunch, shows its transcript as last
    /// shown in the first view, then reads what is newer. Before, it showed
    /// no rows until the computer answered.
    #[test]
    fn a_reopened_chat_paints_its_kept_transcript_at_once() {
        use crate::conversation::{Cached, CachedRow, Entry};
        let dir = tempfile::tempdir().unwrap();
        let secret = SecretKey::from_byte_array([0x11; 32]).unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let store = || Cache::open(&dir.path().join("chats"), &secret);
        let transcripts =
            || Transcripts::chats(Cache::open(&dir.path().join("chats-transcripts"), &secret).ok());
        let saved = vec![Saved {
            code: code(&secret),
            label: "Desk".into(),
            host: None,
            direct: None,
        }];
        store().unwrap().write("chats", &saved).unwrap();
        let observer = saved[0].code.host.clone();
        let opened = chat(0);
        keep(&store().unwrap(), &observer, vec![opened.clone()]);
        transcripts().put(
            &transcript_key(&observer, &opened.id),
            Cached {
                chat: opened.clone(),
                sources: vec![opened.source_id.clone().unwrap()],
                rows: (0..3)
                    .map(|n| CachedRow {
                        segment: 0,
                        offset: n * 10,
                        end: n * 10 + 10,
                        part: 0,
                        entry: Entry::Message {
                            role: rust_native::MessageRole::Assistant,
                            text: format!("Kept reply {n}"),
                        },
                    })
                    .collect(),
                previous: None,
                through: 30,
            },
        );

        let mut chats = Chats::new(runtime.handle().clone(), secret, store(), "chats:t".into())
            .with_transcripts(transcripts());
        render(&mut chats);
        let started = std::time::Instant::now();
        chats.open(observer.clone(), opened.source_id.clone().unwrap());
        let view = render(&mut chats);
        eprintln!(
            "chat reopened: kept transcript in the first view, {:?}",
            started.elapsed()
        );
        for n in 0..3 {
            assert!(view.contains(&format!("Kept reply {n}")), "{view}");
        }
        // A chat never kept opens empty and reads as before.
        let mut fresh = Chats::new(runtime.handle().clone(), secret, store(), "chats:u".into());
        fresh.open(observer, opened.source_id.clone().unwrap());
        assert!(!render(&mut fresh).contains("Kept reply"));
    }
}
