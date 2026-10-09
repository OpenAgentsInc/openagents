//! Coder's chats on the computers: each computer's read-only history
//! observer (the SESS observer in `coder-connect`), the Coder task chats it
//! lists, kept across a relaunch, and the client the Coder tab reads each
//! task's transcript through.
//!
//! A computer pairs for reading through a `coder-pair:` invitation linked
//! to the machine's Computers host key: the iroh enroll reply's, tailnet
//! admission's, or one the phone asks for with `chats.invite` on any other
//! path and before a chat grant ends ([`crate::chat_invites`]); a host
//! grant never admits a history read by itself. Reads run in the
//! background; the host polls with `snapshot`.
//!
//! The phone keeps only Coder task chats and the sessions they delegated.
//! A computer serves only those; an older one that still lists its Claude
//! Code, Codex, OpenCode, or Devin sessions in the same catalog has them
//! ignored here, so the phone neither shows nor keeps them. A session a
//! Coder task delegated to OpenCode or Devin is listed as the task's
//! subagent (`coder_history::delegate`); it shows inside the Coder chat
//! that delegated it, never as a chat of its own.

use coder_computers::cache::Cache;
use coder_connect::direct::Change;
use coder_connect::protocol::Route;
use coder_connect::{Client, ConnectionCode, Observation, Query, RelayPolicy};
use coder_history::{CatalogCursor, CatalogPage, CatalogRequest, Chat, Harness};
use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use tokio::runtime::Handle;
use tokio::sync::broadcast::error::RecvError;

const OBSERVE_LIMIT: Duration = Duration::from_secs(20);
/// Catalog pages read per computer, newest first.
const CATALOG_PAGES: usize = 8;
/// Stop reading a computer's catalog once this many Coder chats are known.
const CATALOG_WANTED: usize = 60;
/// The prefix of each computer's kept chat list; its observer key follows.
const CATALOG_KEY: &str = "chats-catalog-";
/// The most chats kept per computer across a relaunch, newest first.
const KEPT_CHATS: usize = 160;
/// The most plaintext one kept chat list may take.
const KEPT_BYTES: usize = 150 * 1024;
/// The least time between two warm-ups of the computers' connections.
const WARM_EVERY: Duration = Duration::from_secs(15);

/// What became of a read of a catalog's first page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Head {
    Running,
    Read,
    /// It failed or was never started; ask again.
    Failed,
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

struct Computer {
    saved: Saved,
    client: Result<Arc<Client>, String>,
    /// Reads the newest chats when the computer says its task list changed.
    watch: Option<tokio::task::AbortHandle>,
    /// A whole catalog read finished.
    ready: bool,
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
            ready: false,
            chats,
            heads: 0,
            heads_done: 0,
            head_running: false,
            generation: 0,
        }
    }

    /// Start a catalog read: the chats kept stay while it runs.
    fn start(&mut self) -> u64 {
        self.generation += 1;
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
    /// When the computers' connections were last warmed.
    warmed: Option<std::time::Instant>,
}

fn lock(state: &Mutex<State>) -> MutexGuard<'_, State> {
    state.lock().unwrap_or_else(|poison| poison.into_inner())
}

impl Chats {
    pub fn new(runtime: Handle, secret: SecretKey, store: Result<Cache, String>) -> Self {
        let saved: Vec<Saved> = match &store {
            Ok(cache) => cache.read("chats").ok().flatten().unwrap_or_default(),
            Err(_) => vec![],
        };
        let shared = Arc::new(Mutex::new(State::default()));
        // Each computer's Coder chats as last read show at once; the reads
        // below bring them up to date.
        let computers = saved
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
        lock(&shared).computers = computers;
        let mut chats = Self {
            runtime,
            secret,
            store,
            state: shared,
            warmed: None,
        };
        chats.refresh();
        chats
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

    /// Open every computer's connections now, ahead of the Coder tab's first
    /// read or send, so it pays no connection or relay authentication: as
    /// the app comes to the foreground and as the Coder tab shows. At most
    /// every few seconds.
    pub fn warm(&mut self) {
        let now = std::time::Instant::now();
        if self
            .warmed
            .is_some_and(|at| now.duration_since(at) < WARM_EVERY)
        {
            return;
        }
        self.warmed = Some(now);
        let clients: Vec<Arc<Client>> = lock(&self.state)
            .computers
            .iter()
            .filter_map(|computer| computer.client.as_ref().ok().cloned())
            .collect();
        for client in clients {
            self.runtime.spawn(async move { client.warm().await });
        }
    }

    /// Pair from a `coder-pair:` string that tailnet admission, the iroh
    /// enroll reply, or `chats.invite` handed over, named `label` and linked
    /// to the machine's Computers host key `linked`.
    pub fn pair(
        &mut self,
        text: String,
        label: String,
        linked: String,
        direct: Option<std::net::SocketAddr>,
    ) {
        let secret = self.secret;
        let state = self.state.clone();
        let handle = self.runtime.clone();
        self.runtime.spawn(async move {
            let code = if text.starts_with("coder-pair:") {
                coder_connect::pairing::redeem(&text, &secret, RelayPolicy::Production)
                    .await
                    .map_err(|error| error.to_string())
            } else {
                ConnectionCode::parse(text.as_bytes()).map_err(|_| "not a chat invitation".into())
            };
            let Ok(code) = code else { return };
            let host = code.host.clone();
            let mut guard = lock(&state);
            // Paired again: its chats keep showing while they are read, and a
            // renewal that names no direct address keeps the one it had.
            let (known, direct) = guard
                .computers
                .iter_mut()
                .find(|c| c.saved.code.host == host)
                .map(|c| (std::mem::take(&mut c.chats), direct.or(c.saved.direct)))
                .unwrap_or((Vec::new(), direct));
            guard.computers.retain(|c| c.saved.code.host != host);
            let saved = Saved {
                code,
                label,
                host: Some(linked),
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
            guard.dirty = true;
            drop(guard);
            if let Ok(client) = client {
                let state = state.clone();
                handle.spawn(async move {
                    read_catalog(&state, &host, generation, |after| {
                        catalog_page(&client, after)
                    })
                    .await;
                });
            }
        });
    }

    /// Called on each packet: save a new pairing and each chat list that
    /// changed.
    pub fn settle(&mut self) {
        let (dirty, lists) = {
            let mut state = lock(&self.state);
            let changed = std::mem::take(&mut state.changed);
            let lists: Vec<(String, Vec<Chat>)> = state
                .computers
                .iter()
                .filter(|c| changed.contains(&c.saved.code.host))
                .map(|c| (c.saved.code.host.clone(), c.chats.clone()))
                .collect();
            (std::mem::take(&mut state.dirty), lists)
        };
        if dirty {
            self.save();
        }
        if let Ok(cache) = &self.store {
            for (observer, chats) in lists {
                keep(cache, &observer, chats);
            }
        }
    }

    fn save(&mut self) {
        let saved: Vec<Saved> = lock(&self.state)
            .computers
            .iter()
            .map(|c| c.saved.clone())
            .collect();
        if let Ok(cache) = &self.store {
            let _ = cache.write("chats", &saved);
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
                && (c.heads_done > 0 || c.ready)
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
            .filter(|chat| coder(chat) && chat.native_id.as_deref() == Some(task))
            .max_by(|a, b| a.updated_at.cmp(&b.updated_at))?
            .clone();
        Some((computer.saved.code.host.clone(), client, chat))
    }

    /// The copy of `agent`'s session `session` that Coder task `task` on
    /// the machine whose Computers host key is `host` delegated to, with
    /// the client that reads it. The host names it `<Agent> session <id>`
    /// (`coder_history::delegate`).
    pub fn delegate_chat(
        &self,
        host: &str,
        task: &str,
        agent: Harness,
        session: &str,
    ) -> Option<(Arc<Client>, Chat)> {
        let title = format!("{} session {session}", agent_name(agent)?);
        let state = lock(&self.state);
        let computer = state
            .computers
            .iter()
            .find(|c| c.saved.host.as_deref() == Some(host))?;
        let client = computer.client.as_ref().ok()?.clone();
        let chat = computer
            .chats
            .iter()
            .find(|chat| {
                delegated(chat)
                    && chat.harness == agent
                    && chat.native_id.as_deref() == Some(task)
                    && chat.title == title
            })?
            .clone();
        Some((client, chat))
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
    /// linked; [`Chats::head`] says when it finished.
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
            // A failed read rings nothing: the Coder tab asks again on its
            // next packet, and a ring would make that at once, in a loop.
            if let Ok(Observation::Catalog(page)) = result {
                merge(&mut computer.chats, &page.entries);
                computer.heads_done = computer.heads_done.max(round);
                state.changed.insert(observer);
                drop(state);
                crate::wake::ring();
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
                drop(state);
                crate::wake::ring();
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
        .map_err(|_| "The computer did not answer.".to_string())?
        .map_err(|error| error.to_string())
}

/// Read the catalog of computer `observer` with `fetch` into its list: each
/// page merges as it arrives, over the chats kept from the last read, and a
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
        state.changed.insert(observer.to_owned());
        crate::wake::ring();
        true
    })
    .await;
    let mut state = lock(state);
    let Some(index) = current(&mut state) else {
        return;
    };
    let computer = &mut state.computers[index];
    if let Ok((fresh, complete)) = result {
        prune(&mut computer.chats, &fresh, complete);
        computer.ready = true;
        state.changed.insert(observer.to_owned());
        drop(state);
        crate::wake::ring();
    }
}

/// Whether a chat is a Coder task's own.
fn coder(chat: &Chat) -> bool {
    chat.harness == Harness::Coder
}

/// Whether a chat is a session a Coder task delegated to OpenCode or
/// Devin: the task's subagent, named by the task's ID.
fn delegated(chat: &Chat) -> bool {
    chat.subagent
        && matches!(chat.harness, Harness::OpenCode | Harness::Devin)
        && chat
            .native_id
            .as_deref()
            .is_some_and(|task| task.len() == 64 && task.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// Whether the phone keeps a chat: a Coder task's, or a session one
/// delegated. Every other harness's session an older computer lists is
/// ignored.
fn kept_chat(chat: &Chat) -> bool {
    coder(chat) || delegated(chat)
}

/// Merge the chats the phone keeps among `fresh` into `list`: a chat
/// already listed is replaced in place, a new one is added. The Coder tab
/// orders them.
fn merge(list: &mut Vec<Chat>, fresh: &[Chat]) {
    for chat in fresh.iter().filter(|chat| kept_chat(chat)) {
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

/// Newest first, then by ID.
fn order(a: &Chat, b: &Chat) -> std::cmp::Ordering {
    b.updated_at
        .cmp(&a.updated_at)
        .then_with(|| a.id.cmp(&b.id))
}

/// The Coder chats kept for computer `observer`. A list kept by an earlier
/// build may hold other harnesses' sessions; they are left out.
fn kept(cache: &Cache, observer: &str) -> Option<Vec<Chat>> {
    let mut chats: Vec<Chat> = cache
        .read(&format!("{CATALOG_KEY}{observer}"))
        .ok()
        .flatten()?;
    chats.retain(kept_chat);
    Some(chats)
}

/// Keep computer `observer`'s newest Coder chats for the next launch. An
/// empty list is not kept, so a relaunch never shows a computer as empty
/// while a read that found nothing is still possible to redo.
fn keep(cache: &Cache, observer: &str, mut chats: Vec<Chat>) {
    chats.retain(kept_chat);
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
        _ => Err("Couldn't load this chat. Try again.".into()),
    }
}

/// Read catalog pages with `fetch`, newest first, until enough Coder chats
/// are known. `arrived` sees the chats read so far after each page and
/// returns whether to read on.
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

/// The name the host gives a delegate agent in its sessions' titles.
pub fn agent_name(agent: Harness) -> Option<&'static str> {
    match agent {
        Harness::OpenCode => Some("OpenCode"),
        Harness::Devin => Some("Devin"),
        Harness::Codex | Harness::Claude | Harness::Coder => None,
    }
}

/// Whether a chat counts toward the Coder chats a read looks for: a Coder
/// task's, not archived.
fn shown(chat: &Chat) -> bool {
    coder(chat) && !chat.archived
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

    /// Chat `n`: newest first; one in four is a Coder task's, and the rest
    /// are other harnesses' sessions the phone ignores.
    fn chat(n: usize) -> Chat {
        let coder = n.is_multiple_of(4);
        Chat {
            id: format!("chat-{n}"),
            harness: if coder {
                Harness::Coder
            } else {
                Harness::Claude
            },
            native_id: coder.then(|| format!("task-{n}")),
            title: format!("Chat {n}"),
            title_truncated: false,
            updated_at: Some(format!("2026-09-{:02}T00:00:00Z", 28 - n / 20)),
            archived: false,
            subagent: false,
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
    pub fn code(secret: &SecretKey) -> ConnectionCode {
        use coder_connect::protocol::{CONNECTION, GRANT, pubkey, random_id, seal};
        use coder_connect::{Grant, SourceKind, SourceScope};
        let host = SecretKey::from_byte_array([0x42; 32]).unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
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
                label: "Coder".into(),
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
        lock(state).computers[0].chats.len()
    }

    /// A computer's first catalog page merges as soon as it arrives; later
    /// pages follow in the background. Only Coder task chats are kept.
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
            eprintln!("coder chats: first rows after {first:?}, every page after {all:?}");
            assert!(first <= PAGE + Duration::from_millis(10), "{first:?}");
            assert!(all >= PAGE * 8, "{all:?}");
            assert_eq!(visible(&state), 64);
            let state = lock(&state);
            assert!(state.computers[0].ready);
            assert!(state.computers[0].chats.iter().all(coder));
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
            assert!(!lock(&state).computers[0].ready);
        });
    }

    /// A read merges by ID without moving rows, keeps a chat newer than the
    /// read, drops one the computer no longer lists, and ignores every
    /// other harness's session.
    #[test]
    fn a_read_merges_in_place_and_drops_what_vanished() {
        let mut list = vec![chat(0), chat(4), chat(8)];
        let mut renamed = chat(4);
        renamed.title = "Renamed".into();
        let mut newer = chat(100);
        newer.updated_at = Some("2026-09-29T00:00:00Z".into());
        list.push(newer.clone());
        // A session a task delegated is kept for its chat; another
        // harness's own session is not.
        let task = "ab".repeat(32);
        let mut delegate = chat(13);
        delegate.id = "delegate-1".into();
        delegate.harness = Harness::OpenCode;
        delegate.subagent = true;
        delegate.native_id = Some(task.clone());
        delegate.title = "OpenCode session ses_1".into();
        let mut mirrored = chat(13);
        mirrored.harness = Harness::OpenCode;
        merge(&mut list, &[delegate.clone(), mirrored]);
        assert!(list.iter().any(|c| c.id == "delegate-1"));
        assert!(!list.iter().any(|c| c.id == "chat-13"));
        list.retain(|c| c.id != "delegate-1");
        merge(&mut list, &[renamed.clone(), chat(0), chat(12), chat(13)]);
        assert_eq!(list[1].title, "Renamed");
        prune(&mut list, &[renamed, chat(0), chat(12)], true);
        let ids: Vec<&str> = list.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["chat-0", "chat-4", "chat-100", "chat-12"]);
        let mut sorted = list.clone();
        sorted.sort_by(order);
        let ids: Vec<&str> = sorted.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["chat-100", "chat-0", "chat-12", "chat-4"]);
    }

    /// On relaunch a computer's Coder chats as last read are there at once,
    /// before it answers, and a list an earlier build kept with other
    /// harnesses' sessions keeps only the Coder ones.
    #[test]
    fn a_relaunch_has_the_kept_coder_chats_at_once() {
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
            host: Some("desk".into()),
            direct: None,
        }];
        store().unwrap().write("chats", &saved).unwrap();
        let observer = saved[0].code.host.clone();
        // As an earlier build kept it: every harness.
        let every: Vec<Chat> = (0..32).map(chat).collect();
        store()
            .unwrap()
            .write(&format!("{CATALOG_KEY}{observer}"), &every)
            .unwrap();

        let started = std::time::Instant::now();
        let chats = Chats::new(runtime.handle().clone(), secret, store());
        let (_, _, found) = chats.coder_chat("desk", "task-8").expect("kept");
        eprintln!("coder chats on relaunch: {:?}", started.elapsed());
        assert_eq!(found.id, "chat-8");
        let state = lock(&chats.state);
        assert_eq!(state.computers[0].chats.len(), 8);
        assert!(state.computers[0].chats.iter().all(coder));
    }
}
