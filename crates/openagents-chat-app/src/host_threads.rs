//! The chat threads each paired computer keeps, read and continued from
//! this device.
//!
//! A computer's host keeps its own threads (`<host root>/basic-chats`): the
//! ones started in its desktop app or with `openagents chat` there. This
//! device lists them beside its own with NIP-HOST `thread.list`, reads the
//! open one with `thread.read`, sends a follow-up with `thread.send`, stops
//! a reply with `thread.stop`, and starts Coder with `thread.run`, all on
//! the computer's current host link (iroh or the relay) under the device's
//! grant: `observe` to read, `operate` to send, stop, or run. The host appends
//! the follow-up and asks OpenAgents for the reply, so the desktop and
//! `openagents chat read` show it too, and this device reads the reply as
//! it streams by reading the thread again while the host answers.
//!
//! A stop control shows only once the computer has shown it can stop: when
//! a thread opens, this device asks it to stop the reply to a send ID it
//! just minted, which no message holds and so changes nothing. A computer
//! that answers is one that stops; an older one, or a grant without
//! `operate`, refuses, and then no stop control shows at all, never one
//! that does nothing.
//!
//! Run Coder shows when the reply carried that offer and the thread has not
//! started Coder. Tapping it sends `thread.run`. The phone does not ask
//! whether the computer can start Coder until that tap. An older computer
//! refuses the operation, and then the chip goes away. A follow-up chip
//! sends its label with `thread.send`.
//!
//! Each computer's list, and each thread's turns as last read, are kept in
//! the app's encrypted store ([`HostThreads::with_cache`]), the way Coder
//! chats are: a relaunch shows them at once, marked with when they were
//! read, and they open with the computer off. A read that answers always
//! replaces the kept copy. A follow-up waits in a durable outbox under the
//! send ID minted when it was typed; it goes whenever the computer answers
//! again, from the open thread or with the computer's next list read, and
//! leaves the outbox only once the computer accepted or refused it.
//! `thread.send` is idempotent per send ID, so a resend after a crash or a
//! relaunch never appends twice. Nothing here touches this device's own
//! threads, which need no computer. Cached bytes never prove current
//! access: every send, stop, and run still goes through the computer under
//! the device's grant.
//!
//! A thread that delegated Coder work names the task
//! ([`ThreadCoder`](coder_host::access::thread::ThreadCoder)); the Coder
//! tab opens that task's chat through the computer's history observer, the
//! same read path that carries a task's events. A Coder run `openagents
//! chat` started on the computer names the computer itself when its host
//! serves that run's task store; otherwise the page says it ran outside the
//! host ([`ThreadOutside`](coder_host::access::thread::ThreadOutside)) and
//! the phone offers no Coder control for it.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use coder_computers::cache::Cache;
use coder_computers::live::Terminals;
use coder_host::access::Code;
use coder_host::access::protocol::{Operation, Outcome};
use coder_host::access::thread::{ThreadPage, ThreadRole, ThreadRow, ThreadTurn};
use openagents_chat::basic_coder::{Role, Turn};
use serde::{Deserialize, Serialize};

/// How often each computer's list is read again.
pub const LIST_EVERY: Duration = Duration::from_secs(10);
/// How long after a computer said it keeps no threads before it is asked
/// again: an older host, or one with no chat store.
const NOT_SERVED: Duration = Duration::from_secs(10 * 60);
/// How often the open thread is read while a reply streams or a follow-up
/// waits.
const STREAMING: Duration = Duration::from_millis(300);
/// How often the open thread is read otherwise.
const SETTLED: Duration = Duration::from_secs(3);
/// How often a follow-up the computer did not answer is sent again, with
/// the same send ID.
const RESEND: Duration = Duration::from_secs(2);
/// How long a follow-up keeps trying before it says the computer is not
/// answering. It stays and goes again when the thread is opened next.
const SEND_PATIENCE: Duration = Duration::from_secs(30);

/// The key each computer's kept list is stored under.
const LISTS: &str = "host-thread-lists";
/// The key the follow-up outbox is stored under.
const OUTBOX: &str = "host-thread-outbox";
/// The key of the list of kept threads, least recently read first.
const INDEX: &str = "host-thread-index";
/// The prefix of each kept thread's key; the host key and thread ID follow.
const PAGE: &str = "ht-";
/// The most threads kept on disk.
pub const MAX_KEPT: usize = 32;
/// The most plaintext one kept thread may take; older turns go first.
const MAX_KEPT_BYTES: usize = 160 * 1024;
/// The most follow-ups waiting at once.
pub const MAX_QUEUED: usize = 64;

/// A computer's list as kept.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct KeptList {
    label: String,
    rows: Vec<ThreadRow>,
    /// Unix seconds when the computer last answered the list.
    read_at: u64,
}

/// A thread as last read, kept.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct KeptThread {
    /// Unix seconds when the computer last answered a read.
    read_at: u64,
    /// The last page read, without its turns, its reply, or `busy`.
    page: ThreadPage,
    /// Every turn read, oldest first, from `start`.
    start: u64,
    turns: Vec<ThreadTurn>,
}

/// A follow-up waiting for its computer, under the send ID minted when it
/// was typed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Queued {
    pub host: String,
    pub thread: String,
    pub request: String,
    pub text: String,
    /// Unix seconds when it was typed.
    pub queued_at: u64,
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

/// The key a thread is kept under, when its host and ID make one.
fn page_key(host: &str, thread: &str) -> Option<String> {
    let fits = |id: &str| !id.is_empty() && id.bytes().all(|b| b.is_ascii_alphanumeric());
    (fits(host) && fits(thread) && host.len() <= 96 && thread.len() <= 64)
        .then(|| format!("{PAGE}{host}-{thread}"))
}

/// Why a computer did not answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The computer keeps no threads for this device, or predates them.
    NotServed,
    /// The computer refused, in words to show.
    Refused(String),
    /// The computer could not be reached; try again soon.
    Failed,
}

/// How this device reaches a computer's threads.
pub trait Link: Send + Sync {
    /// `thread.list` on `host`.
    fn list(&self, host: &str) -> Result<Vec<ThreadRow>, Refusal>;
    /// `thread.read` on `host`: the newest turns, or those before `before`.
    fn read(&self, host: &str, thread: &str, before: Option<u64>) -> Result<ThreadPage, Refusal>;
    /// `thread.send` on `host` under the send ID `request`.
    fn send(&self, host: &str, thread: &str, request: &str, text: &str) -> Result<(), Refusal>;
    /// `thread.stop` on `host`: stop the reply to the message whose send
    /// ID is `request`. A computer that predates it refuses as not served.
    fn stop(&self, host: &str, thread: &str, request: Option<&str>) -> Result<(), Refusal>;
    /// `thread.run` on `host`: start Coder for the thread. Returns the task
    /// ID. A computer that predates it refuses as not served.
    fn run(&self, host: &str, thread: &str) -> Result<String, Refusal>;
}

/// The live link: the Computers service's current link to each host.
pub struct Live {
    terminals: Terminals,
    handle: tokio::runtime::Handle,
}

impl Live {
    pub fn new(terminals: Terminals, handle: tokio::runtime::Handle) -> Self {
        Self { terminals, handle }
    }

    fn call(&self, host: &str, op: Operation) -> Result<Outcome, Refusal> {
        let link = (self.terminals.links(host))().map_err(|_| Refusal::Failed)?;
        match self.handle.block_on(link.call(op)) {
            Ok(outcome) => Ok(outcome),
            Err(coder_host::Error::Access(error)) => Err(match error.code {
                Code::Unsupported | Code::Malformed | Code::Unavailable => Refusal::NotServed,
                Code::MissingRight | Code::Forbidden => {
                    Refusal::Refused("This phone may not do that on this computer.".into())
                }
                Code::Conflict => Refusal::Refused(
                    "The computer is still answering this thread. Try again when it finishes."
                        .into(),
                ),
                Code::Revoked | Code::Stale | Code::Expired => {
                    Refusal::Refused("This phone is no longer paired with the computer.".into())
                }
                _ => Refusal::Failed,
            }),
            Err(_) => Err(Refusal::Failed),
        }
    }
}

impl Link for Live {
    fn list(&self, host: &str) -> Result<Vec<ThreadRow>, Refusal> {
        match self.call(host, Operation::ListThreads {})? {
            Outcome::Threads { threads } => Ok(threads),
            _ => Err(Refusal::Failed),
        }
    }

    fn read(&self, host: &str, thread: &str, before: Option<u64>) -> Result<ThreadPage, Refusal> {
        match self.call(
            host,
            Operation::ReadThread {
                thread: thread.to_owned(),
                before,
            },
        )? {
            Outcome::Thread { thread } => Ok(*thread),
            _ => Err(Refusal::Failed),
        }
    }

    fn send(&self, host: &str, thread: &str, request: &str, text: &str) -> Result<(), Refusal> {
        match self.call(
            host,
            Operation::SendThread {
                thread: thread.to_owned(),
                request: request.to_owned(),
                text: text.to_owned(),
            },
        )? {
            Outcome::Dispatched { .. } => Ok(()),
            _ => Err(Refusal::Failed),
        }
    }

    fn stop(&self, host: &str, thread: &str, request: Option<&str>) -> Result<(), Refusal> {
        match self.call(
            host,
            Operation::StopThread {
                thread: thread.to_owned(),
                request: request.map(str::to_owned),
            },
        )? {
            Outcome::Dispatched { .. } => Ok(()),
            _ => Err(Refusal::Failed),
        }
    }

    fn run(&self, host: &str, thread: &str) -> Result<String, Refusal> {
        let link = (self.terminals.links(host))().map_err(|_| Refusal::Failed)?;
        match self.handle.block_on(link.call(Operation::RunThread {
            thread: thread.to_owned(),
        })) {
            Ok(Outcome::Dispatched { receipt }) => Ok(receipt.reference),
            Ok(_) => Err(Refusal::Failed),
            Err(coder_host::Error::Access(error)) => Err(match error.code {
                Code::Unsupported | Code::Malformed => Refusal::NotServed,
                Code::Conflict => {
                    Refusal::Refused("Wait for a complete saved reply before running Coder.".into())
                }
                Code::Forbidden => Refusal::Refused("This reply can no longer start Coder.".into()),
                Code::MissingRight => {
                    Refusal::Refused("This phone may not do that on this computer.".into())
                }
                _ => Refusal::Refused("Coder could not start on this computer.".into()),
            }),
            Err(_) => Err(Refusal::Failed),
        }
    }
}

/// One computer's threads as last read.
#[derive(Clone, Debug)]
pub struct Listed {
    pub label: String,
    pub rows: Vec<ThreadRow>,
    /// Unix seconds when the computer last answered the list.
    pub read_at: u64,
    /// The computer answered since launch and its last read did not fail;
    /// otherwise these rows are the copy kept from `read_at`.
    pub fresh: bool,
}

/// A follow-up on its way to the computer.
#[derive(Clone, Debug)]
struct Sending {
    request: String,
    text: String,
    since: Instant,
    /// The computer accepted it; it shows once a read carries it.
    accepted: bool,
}

/// The thread open on this device.
#[derive(Clone, Debug)]
struct Opened {
    host: String,
    thread: String,
    /// Each open gets its own generation, so a reader for a thread closed
    /// since stops, and its late page is dropped.
    generation: u64,
    page: Option<ThreadPage>,
    /// Every turn read so far, in order: the newest page and any earlier
    /// pages read with **Load earlier**.
    turns: Vec<ThreadTurn>,
    /// The index of the first of `turns`.
    turns_start: u64,
    loading_earlier: bool,
    sending: Option<Sending>,
    /// The computer stops replies for this device: unknown until it
    /// answers.
    can_stop: Option<bool>,
    /// A stop is on its way.
    stopping: bool,
    /// This device stopped a reply in this thread since it opened.
    stopped_here: bool,
    /// `thread.run` is on its way.
    running: bool,
    /// The computer can start Coder from this phone. Unknown until a run
    /// is refused as not served, which hides the chip.
    can_run: Option<bool>,
    error: Option<String>,
    /// The turns show the kept copy read at this time; no read answered
    /// since the thread opened.
    kept_at: Option<u64>,
    /// What was last written to the store for this thread.
    kept: Option<KeptThread>,
}

impl Opened {
    /// This thread as it would be kept: settled turns only.
    fn keepable(&self, read_at: u64) -> Option<KeptThread> {
        let page = self.page.as_ref()?;
        Some(KeptThread {
            read_at,
            page: ThreadPage {
                turns: Vec::new(),
                busy: false,
                partial: String::new(),
                ..page.clone()
            },
            start: self.turns_start,
            turns: self.turns.clone(),
        })
    }

    /// The message whose reply is streaming, by its send ID (`None` for
    /// one sent without an ID): the follow-up this device sent, once the
    /// computer accepted it, else the last turn while the thread answers.
    fn answering(&self) -> Option<Option<String>> {
        if let Some(sending) = &self.sending {
            return sending.accepted.then(|| Some(sending.request.clone()));
        }
        let page = self.page.as_ref().filter(|page| page.busy)?;
        let last = page.turns.last()?;
        (last.role == ThreadRole::User).then(|| last.request.clone())
    }
}

#[derive(Default)]
struct Inner {
    /// Where lists, threads, and the outbox are kept; `None` keeps them
    /// only while the app runs.
    cache: Option<Arc<Cache>>,
    /// Kept threads' keys, least recently read first.
    index: Vec<String>,
    /// Follow-ups waiting for their computers, oldest first.
    outbox: Vec<Queued>,
    /// Send IDs being sent now, so two readers never send one at once.
    delivering: BTreeSet<String>,
    lists: BTreeMap<String, Listed>,
    /// Whether each computer stops replies, once it answered a stop.
    stops: BTreeMap<String, bool>,
    listing: BTreeSet<String>,
    next: BTreeMap<String, Instant>,
    open: Option<Opened>,
    generation: u64,
    revision: u64,
}

/// The computers' threads and the one open here. Reads run on their own
/// threads and ring `wake` when what shows changed.
#[derive(Clone)]
pub struct HostThreads {
    inner: Arc<Mutex<Inner>>,
    wake: Arc<dyn Fn() + Send + Sync>,
}

impl Default for HostThreads {
    fn default() -> Self {
        Self::new(Arc::new(|| {}))
    }
}

/// A view of the open thread, for drawing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shown {
    pub host: String,
    pub thread: String,
    pub title: String,
    /// Every loaded turn, oldest first, with a follow-up still on its way
    /// shown as the last message.
    pub turns: Vec<Turn>,
    /// The index of the first loaded turn.
    pub start: u64,
    pub busy: bool,
    pub partial: String,
    pub failure: Option<String>,
    pub error: Option<String>,
    /// The first page has not arrived yet.
    pub loading: bool,
    /// Earlier turns are loading.
    pub loading_earlier: bool,
    pub coder: Option<coder_host::access::thread::ThreadCoder>,
    /// Coder work the thread ran on the computer outside its host, which
    /// this phone can neither open nor stop.
    pub outside: Option<coder_host::access::thread::ThreadOutside>,
    /// A reply streams and the computer can stop it for this device: the
    /// stop control shows only then.
    pub stoppable: bool,
    /// This device stopped a reply here since the thread opened.
    pub stopped_here: bool,
    /// Run Coder can still be offered. An older computer sets this false.
    pub runnable: bool,
    /// The turns are the copy this device kept, read at this Unix time; the
    /// computer has not answered since the thread opened.
    pub kept_at: Option<u64>,
    /// A follow-up waits for the computer to accept it.
    pub queued: bool,
}

impl Inner {
    fn save_lists(&self) {
        let Some(cache) = &self.cache else { return };
        let kept: BTreeMap<&String, KeptList> = self
            .lists
            .iter()
            .map(|(host, listed)| {
                (
                    host,
                    KeptList {
                        label: listed.label.clone(),
                        rows: listed.rows.clone(),
                        read_at: listed.read_at,
                    },
                )
            })
            .collect();
        let _ = cache.write(LISTS, &kept);
    }

    fn save_outbox(&self) -> bool {
        self.cache
            .as_ref()
            .is_none_or(|cache| cache.write(OUTBOX, &self.outbox).is_ok())
    }

    /// Keep `kept` as `host`'s `thread`, trimmed to fit, newest read last.
    fn keep_thread(&mut self, host: &str, thread: &str, mut kept: KeptThread) {
        let (Some(cache), Some(key)) = (self.cache.clone(), page_key(host, thread)) else {
            return;
        };
        while kept.turns.len() > 1
            && serde_json::to_vec(&kept).map_or(0, |bytes| bytes.len()) > MAX_KEPT_BYTES
        {
            let drop = kept.turns.len().div_ceil(4);
            kept.turns.drain(..drop);
            kept.start += drop as u64;
        }
        if cache.write(&key, &kept).is_err() {
            return;
        }
        self.index.retain(|kept| kept != &key);
        self.index.push(key);
        while self.index.len() > MAX_KEPT {
            let old = self.index.remove(0);
            let _ = cache.erase(&old);
        }
        let _ = cache.write(INDEX, &self.index);
    }

    fn kept_thread(&self, host: &str, thread: &str) -> Option<KeptThread> {
        let key = page_key(host, thread)?;
        self.cache.as_ref()?.read(&key).ok().flatten()
    }
}

impl HostThreads {
    pub fn new(wake: Arc<dyn Fn() + Send + Sync>) -> Self {
        Self {
            inner: Arc::default(),
            wake,
        }
    }

    /// Keep lists, threads, and waiting follow-ups in `cache`, and show
    /// what it kept at once: each list marked with when it was read.
    #[must_use]
    pub fn with_cache(self, cache: Option<Cache>) -> Self {
        {
            let mut inner = self.lock();
            let cache = cache.map(Arc::new);
            if let Some(cache) = &cache {
                let lists: BTreeMap<String, KeptList> =
                    cache.read(LISTS).ok().flatten().unwrap_or_default();
                inner.lists = lists
                    .into_iter()
                    .map(|(host, kept)| {
                        (
                            host,
                            Listed {
                                label: kept.label,
                                rows: kept.rows,
                                read_at: kept.read_at,
                                fresh: false,
                            },
                        )
                    })
                    .collect();
                inner.outbox = cache.read(OUTBOX).ok().flatten().unwrap_or_default();
                inner.index = cache.read(INDEX).ok().flatten().unwrap_or_default();
            }
            inner.cache = cache;
            self.changed(&mut inner);
        }
        self
    }

    /// The follow-ups waiting for their computers, oldest first.
    pub fn queued(&self) -> Vec<Queued> {
        self.lock().outbox.clone()
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn changed(&self, inner: &mut Inner) {
        inner.revision = inner.revision.wrapping_add(1);
        (self.wake)();
    }

    /// Changes since launch; a view built at another revision is stale.
    pub fn revision(&self) -> u64 {
        self.lock().revision
    }

    /// Read the list of each of `hosts` (host key, label) that is due, in
    /// the background; forget computers no longer paired for reading.
    pub fn poll(&self, hosts: Vec<(String, String)>, link: &Arc<dyn Link>) {
        let now = Instant::now();
        let due: Vec<(String, String)> = {
            let mut inner = self.lock();
            let keep: BTreeSet<&String> = hosts.iter().map(|(host, _)| host).collect();
            let before = inner.lists.len();
            inner.lists.retain(|host, _| keep.contains(host));
            if inner.lists.len() != before {
                inner.save_lists();
                self.changed(&mut inner);
            }
            let due: Vec<(String, String)> = hosts
                .into_iter()
                .filter(|(host, _)| {
                    !inner.listing.contains(host)
                        && inner.next.get(host).is_none_or(|at| now >= *at)
                })
                .collect();
            for (host, _) in &due {
                inner.listing.insert(host.clone());
            }
            due
        };
        for (host, label) in due {
            let threads = self.clone();
            let link = link.clone();
            std::thread::spawn(move || {
                let answer = link.list(&host);
                let mut inner = threads.lock();
                inner.listing.remove(&host);
                let reached = answer.is_ok();
                let wait = match answer {
                    Ok(rows) => {
                        let old = inner.lists.get(&host);
                        let same = old
                            .is_some_and(|old| old.fresh && old.rows == rows && old.label == label);
                        // The kept copy is rewritten only when it changed
                        // or is a minute old, so an idle list is not
                        // written every read.
                        let now = unix_now();
                        let save = !old.is_some_and(|old| {
                            old.rows == rows
                                && old.label == label
                                && now.saturating_sub(old.read_at) < 60
                        });
                        let read_at = if save {
                            now
                        } else {
                            old.map_or(now, |old| old.read_at)
                        };
                        inner.lists.insert(
                            host.clone(),
                            Listed {
                                label,
                                rows,
                                read_at,
                                fresh: true,
                            },
                        );
                        if save {
                            inner.save_lists();
                        }
                        if !same {
                            threads.changed(&mut inner);
                        }
                        LIST_EVERY
                    }
                    Err(Refusal::NotServed) => {
                        if inner.lists.remove(&host).is_some() {
                            inner.save_lists();
                            threads.changed(&mut inner);
                        }
                        NOT_SERVED
                    }
                    Err(_) => {
                        // Unreachable: the rows stay, marked with when
                        // they were read.
                        if let Some(listed) = inner.lists.get_mut(&host)
                            && listed.fresh
                        {
                            listed.fresh = false;
                            threads.changed(&mut inner);
                        }
                        LIST_EVERY
                    }
                };
                inner.next.insert(host.clone(), Instant::now() + wait);
                let waiting: Vec<Queued> = if reached {
                    inner
                        .outbox
                        .iter()
                        .filter(|queued| queued.host == host)
                        .cloned()
                        .collect()
                } else {
                    Vec::new()
                };
                drop(inner);
                // The computer answers again: follow-ups that waited for it
                // go now, each under its own send ID.
                for queued in waiting {
                    threads.deliver(&queued, &*link);
                }
            });
        }
    }

    /// Send one waiting follow-up, unless another reader is sending it
    /// now (`None`). An answer or a refusal takes it out of the outbox; a
    /// computer not reached leaves it there for the next try.
    fn deliver(&self, queued: &Queued, link: &dyn Link) -> Option<Result<(), Refusal>> {
        {
            let mut inner = self.lock();
            if !inner
                .outbox
                .iter()
                .any(|waiting| waiting.request == queued.request)
                || !inner.delivering.insert(queued.request.clone())
            {
                return None;
            }
        }
        let answer = link.send(&queued.host, &queued.thread, &queued.request, &queued.text);
        let mut inner = self.lock();
        inner.delivering.remove(&queued.request);
        if !matches!(answer, Err(Refusal::Failed)) {
            inner
                .outbox
                .retain(|waiting| waiting.request != queued.request);
            inner.save_outbox();
        }
        if let Some(open) = inner.open.as_mut()
            && open
                .sending
                .as_ref()
                .is_some_and(|sending| sending.request == queued.request)
        {
            match &answer {
                Ok(()) => {
                    if let Some(sending) = open.sending.as_mut() {
                        sending.accepted = true;
                    }
                }
                Err(Refusal::Failed) => {}
                Err(refusal) => {
                    open.sending = None;
                    open.error = Some(words(refusal));
                }
            }
            self.changed(&mut inner);
        }
        Some(answer)
    }

    /// When `host`'s list was last read, while its rows are the kept copy:
    /// the computer has not answered since launch, or stopped answering.
    pub fn kept_at(&self, host: &str) -> Option<u64> {
        self.lock()
            .lists
            .get(host)
            .filter(|listed| !listed.fresh)
            .map(|listed| listed.read_at)
    }

    /// Every computer's threads: host key, computer label, and row.
    pub fn rows(&self) -> Vec<(String, String, ThreadRow)> {
        self.lock()
            .lists
            .iter()
            .flat_map(|(host, listed)| {
                listed
                    .rows
                    .iter()
                    .map(|row| (host.clone(), listed.label.clone(), row.clone()))
            })
            .collect()
    }

    /// The label of a computer whose threads were listed.
    pub fn label(&self, host: &str) -> Option<String> {
        self.lock()
            .lists
            .get(host)
            .map(|listed| listed.label.clone())
    }

    /// Open `thread` on `host` and keep reading it until it closes.
    pub fn open(&self, host: &str, thread: &str, link: Arc<dyn Link>) {
        let generation = {
            let mut inner = self.lock();
            inner.generation += 1;
            let generation = inner.generation;
            let can_stop = inner.stops.get(host).copied();
            // The kept copy shows at once; the first read replaces it.
            let kept = inner.kept_thread(host, thread);
            // A follow-up still waiting for this thread shows as the last
            // message and goes when the computer answers.
            let sending = inner
                .outbox
                .iter()
                .find(|queued| queued.host == host && queued.thread == thread)
                .map(|queued| Sending {
                    request: queued.request.clone(),
                    text: queued.text.clone(),
                    since: Instant::now(),
                    accepted: false,
                });
            inner.open = Some(Opened {
                host: host.to_owned(),
                thread: thread.to_owned(),
                generation,
                page: kept.as_ref().map(|kept| kept.page.clone()),
                turns: kept
                    .as_ref()
                    .map(|kept| kept.turns.clone())
                    .unwrap_or_default(),
                turns_start: kept.as_ref().map_or(0, |kept| kept.start),
                loading_earlier: false,
                sending,
                can_stop,
                stopping: false,
                stopped_here: false,
                running: false,
                can_run: None,
                error: None,
                kept_at: kept.as_ref().map(|kept| kept.read_at),
                kept,
            });
            self.changed(&mut inner);
            (generation, can_stop.is_none())
        };
        let (generation, probe) = generation;
        if probe {
            let threads = self.clone();
            let (host, thread, link) = (host.to_owned(), thread.to_owned(), link.clone());
            std::thread::spawn(move || threads.probe(&host, &thread, generation, &*link));
        }
        let threads = self.clone();
        let (host, thread) = (host.to_owned(), thread.to_owned());
        std::thread::spawn(move || threads.follow(&host, &thread, generation, &*link));
    }

    /// Learn whether `host` stops replies: a stop under a send ID minted
    /// here, which no message holds, changes nothing on a computer that
    /// stops and is refused by one that does not.
    fn probe(&self, host: &str, thread: &str, generation: u64, link: &dyn Link) {
        let fresh = uuid::Uuid::new_v4().simple().to_string();
        let answer = link.stop(host, thread, Some(&fresh));
        let known = match answer {
            Ok(()) => Some(true),
            // An older computer: it will not stop until it is updated.
            Err(Refusal::NotServed) => Some(false),
            // No `operate`, or unreachable: no stop now; ask again next
            // time a thread opens.
            Err(_) => None,
        };
        let mut inner = self.lock();
        if let Some(known) = known {
            inner.stops.insert(host.to_owned(), known);
        }
        if let Some(open) = inner.open.as_mut().filter(|o| o.generation == generation) {
            open.can_stop = Some(known.unwrap_or(false));
            self.changed(&mut inner);
        }
    }

    /// Stop the reply streaming into the open thread, through its
    /// computer. False when there is nothing this device can stop: no
    /// reply streams, the computer cannot stop, or a stop is on its way.
    pub fn stop(&self, link: Arc<dyn Link>) -> bool {
        let (host, thread, request, generation) = {
            let mut inner = self.lock();
            let Some(open) = inner.open.as_mut() else {
                return false;
            };
            if open.stopping || open.can_stop != Some(true) {
                return false;
            }
            let Some(request) = open.answering() else {
                return false;
            };
            open.stopping = true;
            open.error = None;
            let found = (
                open.host.clone(),
                open.thread.clone(),
                request,
                open.generation,
            );
            self.changed(&mut inner);
            found
        };
        let threads = self.clone();
        std::thread::spawn(move || {
            let since = Instant::now();
            let answer = loop {
                match link.stop(&host, &thread, request.as_deref()) {
                    Err(Refusal::Failed) if since.elapsed() < SEND_PATIENCE => {
                        std::thread::sleep(RESEND);
                    }
                    answer => break answer,
                }
            };
            let mut inner = threads.lock();
            let Some(open) = inner.open.as_mut().filter(|o| o.generation == generation) else {
                return;
            };
            open.stopping = false;
            match answer {
                Ok(()) => open.stopped_here = true,
                Err(refusal) => open.error = Some(words(&refusal)),
            }
            threads.changed(&mut inner);
        });
        true
    }

    /// Close the open thread; its reader stops.
    pub fn close(&self) {
        let mut inner = self.lock();
        if inner.open.take().is_some() {
            self.changed(&mut inner);
        }
    }

    /// The open thread's host and ID.
    pub fn opened(&self) -> Option<(String, String)> {
        self.lock()
            .open
            .as_ref()
            .map(|open| (open.host.clone(), open.thread.clone()))
    }

    /// Whether the open thread is changing on its own: a reply streams, a
    /// follow-up waits, or the first page has not arrived.
    pub fn live(&self) -> bool {
        self.lock().open.as_ref().is_some_and(|open| {
            open.sending.is_some()
                || open.loading_earlier
                || open.page.as_ref().is_none_or(|page| page.busy)
        })
    }

    /// Send `text` to the open thread through its computer. False when no
    /// thread is open, a follow-up is still on its way, or the computer is
    /// answering.
    pub fn send(&self, text: &str) -> bool {
        let text = text.trim();
        if text.is_empty() || text.len() > coder_host::access::thread::MAX_MESSAGE {
            return false;
        }
        let mut inner = self.lock();
        let Some(open) = inner.open.as_ref() else {
            return false;
        };
        if open.sending.is_some() || open.page.as_ref().is_some_and(|page| page.busy) {
            return false;
        }
        if inner.outbox.len() >= MAX_QUEUED {
            return false;
        }
        // The send ID is minted once and kept with the text before anything
        // goes, so every try, across relaunches, carries the same one.
        let queued = Queued {
            host: open.host.clone(),
            thread: open.thread.clone(),
            request: uuid::Uuid::new_v4().simple().to_string(),
            text: text.to_owned(),
            queued_at: unix_now(),
        };
        inner.outbox.push(queued.clone());
        if !inner.save_outbox() {
            inner.outbox.pop();
            return false;
        }
        let Some(open) = inner.open.as_mut() else {
            return false;
        };
        open.sending = Some(Sending {
            request: queued.request,
            text: queued.text,
            since: Instant::now(),
            accepted: false,
        });
        open.error = None;
        self.changed(&mut inner);
        true
    }

    /// Start Coder for the open thread through its computer. False when no
    /// thread is open, a run is on its way, the computer cannot start
    /// Coder, a reply is streaming, or the thread already started Coder.
    pub fn run(&self, link: Arc<dyn Link>) -> bool {
        let (host, thread, generation) = {
            let mut inner = self.lock();
            let Some(open) = inner.open.as_mut() else {
                return false;
            };
            if open.running || open.can_run == Some(false) {
                return false;
            }
            if open
                .page
                .as_ref()
                .is_some_and(|page| page.busy || page.coder.is_some())
            {
                return false;
            }
            open.running = true;
            open.error = None;
            let found = (open.host.clone(), open.thread.clone(), open.generation);
            self.changed(&mut inner);
            found
        };
        let threads = self.clone();
        std::thread::spawn(move || {
            let answer = link.run(&host, &thread);
            let mut inner = threads.lock();
            let Some(open) = inner
                .open
                .as_mut()
                .filter(|open| open.generation == generation)
            else {
                return;
            };
            open.running = false;
            match answer {
                Ok(_) => {}
                Err(Refusal::NotServed) => {
                    open.can_run = Some(false);
                    open.error = Some("This computer can't start Coder from the phone yet.".into());
                }
                Err(refusal) => open.error = Some(words(&refusal)),
            }
            threads.changed(&mut inner);
        });
        true
    }

    /// Read the turns before the loaded ones.
    pub fn earlier(&self, link: Arc<dyn Link>) {
        let (host, thread, before, generation) = {
            let mut inner = self.lock();
            let Some(open) = inner.open.as_mut() else {
                return;
            };
            if open.page.is_none() {
                return;
            }
            let before = open.turns_start;
            if before == 0 || open.loading_earlier {
                return;
            }
            open.loading_earlier = true;
            let found = (
                open.host.clone(),
                open.thread.clone(),
                before,
                open.generation,
            );
            self.changed(&mut inner);
            found
        };
        let threads = self.clone();
        std::thread::spawn(move || {
            let answer = link.read(&host, &thread, Some(before));
            let mut inner = threads.lock();
            let Some(open) = inner.open.as_mut().filter(|o| o.generation == generation) else {
                return;
            };
            open.loading_earlier = false;
            match answer {
                // Only a page that ends where the read turns start joins them.
                Ok(page)
                    if page.start + page.turns.len() as u64 == before
                        && open.turns_start == before =>
                {
                    let mut turns = page.turns;
                    turns.append(&mut open.turns);
                    open.turns = turns;
                    open.turns_start = page.start;
                    if open.kept_at.is_none() && open.page.as_ref().is_some_and(|p| !p.busy) {
                        threads.keep_open(&mut inner, generation);
                    }
                }
                Ok(_) => {}
                Err(refusal) => open.error = Some(words(&refusal)),
            }
            threads.changed(&mut inner);
        });
    }

    /// The open thread, for drawing.
    pub fn shown(&self) -> Option<Shown> {
        let inner = self.lock();
        let open = inner.open.as_ref()?;
        let page = open.page.as_ref();
        let mut turns: Vec<Turn> = open.turns.iter().map(turn).collect();
        if let Some(sending) = &open.sending {
            let mut echo = Turn::user(sending.text.clone());
            echo.request = Some(sending.request.clone());
            turns.push(echo);
        }
        let start = open.turns_start;
        Some(Shown {
            host: open.host.clone(),
            thread: open.thread.clone(),
            title: page.map_or_else(String::new, |page| page.title.clone()),
            turns,
            start,
            busy: page.is_some_and(|page| page.busy) || open.sending.is_some(),
            partial: page.map_or_else(String::new, |page| page.partial.clone()),
            failure: page.and_then(|page| page.failure.clone()),
            error: open.error.clone(),
            loading: page.is_none() && open.error.is_none(),
            loading_earlier: open.loading_earlier,
            coder: page.and_then(|page| page.coder.clone()),
            outside: page.and_then(|page| page.outside.clone()),
            stoppable: open.can_stop == Some(true) && !open.stopping && open.answering().is_some(),
            stopped_here: open.stopped_here,
            runnable: open.can_run != Some(false),
            kept_at: open.kept_at,
            queued: open
                .sending
                .as_ref()
                .is_some_and(|sending| !sending.accepted),
        })
    }

    /// Read the open thread until it closes or another opens, and deliver
    /// its follow-up.
    fn follow(&self, host: &str, thread: &str, generation: u64, link: &dyn Link) {
        loop {
            // Deliver a follow-up first, so the next read carries it.
            let sending = {
                let inner = self.lock();
                match inner.open.as_ref() {
                    Some(open) if open.generation == generation => {
                        open.sending.clone().filter(|sending| !sending.accepted)
                    }
                    _ => return,
                }
            };
            if let Some(sending) = sending {
                let queued = Queued {
                    host: host.to_owned(),
                    thread: thread.to_owned(),
                    request: sending.request.clone(),
                    text: sending.text.clone(),
                    queued_at: 0,
                };
                if matches!(self.deliver(&queued, link), Some(Err(Refusal::Failed))) {
                    if sending.since.elapsed() < SEND_PATIENCE {
                        std::thread::sleep(RESEND);
                        continue;
                    }
                    // It stays in the outbox, shown as waiting, and goes
                    // when the computer answers again.
                    let mut inner = self.lock();
                    let Some(open) = inner.open.as_mut().filter(|o| o.generation == generation)
                    else {
                        return;
                    };
                    let error = Some(QUEUED.to_owned());
                    if open.error != error {
                        open.error = error;
                        self.changed(&mut inner);
                    }
                }
            }
            let answer = link.read(host, thread, None);
            let pause = {
                let mut inner = self.lock();
                let Some(open) = inner.open.as_mut().filter(|o| o.generation == generation) else {
                    return;
                };
                let mut changed = false;
                let mut answered = false;
                match answer {
                    Ok(page) => {
                        // The newest page replaces the turns it covers and
                        // keeps earlier ones that still join it; after a
                        // gap it starts over from the page.
                        let end = open.turns_start + open.turns.len() as u64;
                        if open.page.is_some()
                            && page.start >= open.turns_start
                            && page.start <= end
                        {
                            open.turns
                                .truncate((page.start - open.turns_start) as usize);
                        } else {
                            open.turns.clear();
                            open.turns_start = page.start;
                        }
                        open.turns.extend(page.turns.iter().cloned());
                        let arrived = open.sending.as_ref().is_some_and(|sending| {
                            sending.accepted
                                && page
                                    .turns
                                    .iter()
                                    .any(|turn| turn.request.as_deref() == Some(&sending.request))
                        });
                        if arrived {
                            open.sending = None;
                            changed = true;
                        }
                        if open.page.as_ref() != Some(&page)
                            || open.error.is_some()
                            || open.kept_at.is_some()
                        {
                            open.page = Some(page);
                            open.error = None;
                            open.kept_at = None;
                            changed = true;
                        }
                        answered = true;
                    }
                    Err(refusal) => {
                        let queued = open.sending.as_ref().is_some_and(|s| !s.accepted)
                            && open.error.as_deref() == Some(QUEUED);
                        let error = Some(words(&refusal));
                        if !queued && open.error != error {
                            open.error = error;
                            changed = true;
                        }
                    }
                }
                let quick = open.error.is_none()
                    && (open.sending.is_some() || open.page.as_ref().is_some_and(|page| page.busy));
                // A settled thread is kept once its turns changed.
                let settled = open.error.is_none()
                    && open.kept_at.is_none()
                    && open.page.as_ref().is_some_and(|page| !page.busy);
                // The computer answers again: its kept list is read again
                // at the next poll rather than on its timer.
                if answered && inner.lists.get(host).is_some_and(|listed| !listed.fresh) {
                    inner.next.remove(host);
                }
                if settled {
                    self.keep_open(&mut inner, generation);
                }
                if changed {
                    self.changed(&mut inner);
                }
                if quick { STREAMING } else { SETTLED }
            };
            std::thread::sleep(pause);
        }
    }
}

/// What the open thread says while its follow-up waits for the computer.
const QUEUED: &str =
    "The computer isn't answering. Your message waits on this phone and goes when it's back.";

impl HostThreads {
    /// Keep the open thread of `generation` when its settled turns changed
    /// since they were last kept.
    fn keep_open(&self, inner: &mut Inner, generation: u64) {
        let Some(open) = inner.open.as_ref().filter(|o| o.generation == generation) else {
            return;
        };
        let now = unix_now();
        let Some(kept) = open.keepable(now) else {
            return;
        };
        // Unchanged turns are written again only once a minute, so the
        // kept copy's time stays near the last read without a write per
        // read.
        let same = open.kept.as_ref().is_some_and(|old| {
            old.page == kept.page
                && old.start == kept.start
                && old.turns == kept.turns
                && now.saturating_sub(old.read_at) < 60
        });
        if same {
            return;
        }
        let (host, thread) = (open.host.clone(), open.thread.clone());
        inner.keep_thread(&host, &thread, kept.clone());
        if let Some(open) = inner.open.as_mut() {
            open.kept = Some(kept);
        }
    }
}

/// Words for a refusal.
fn words(refusal: &Refusal) -> String {
    match refusal {
        Refusal::NotServed => "This computer doesn't share its chats with this phone.".into(),
        Refusal::Refused(words) => words.clone(),
        Refusal::Failed => "Couldn't reach the computer. Trying again…".into(),
    }
}

/// A thread's turn as this device's chat views draw one.
pub fn turn(turn: &ThreadTurn) -> Turn {
    let mut out = match turn.role {
        ThreadRole::User => Turn::user(turn.text.clone()),
        ThreadRole::Assistant => Turn::assistant(turn.text.clone(), None),
    };
    debug_assert!(matches!(out.role, Role::User | Role::Assistant));
    out.at = turn.at;
    out.stopped = turn.stopped;
    out.model = turn.model.clone();
    out.request = turn.request.clone();
    out.meta = meta_of(&turn.extras);
    out
}

/// Offers, cards, and follow-ups the page carried, read again. Anything
/// the phone's own tables refuse is dropped. The typed judgment stays off
/// the page, so it stays unset here.
fn meta_of(
    extras: &coder_host::access::thread::ThreadExtras,
) -> Option<openagents_chat::router::Meta> {
    if extras.is_empty() {
        return None;
    }
    let mut meta = openagents_chat::router::Meta::default();
    for value in &extras.offers {
        if let Some(offer) = openagents_chat::router::Offer::parse(value)
            && meta.offers.len() < coder_host::access::thread::MAX_OFFERS
            && !meta.offers.contains(&offer)
        {
            // The computer's own prediction of who runs Coder there rides
            // on its Run Coder offer, as a typed value or not at all.
            if offer == openagents_chat::router::Offer::RunCoder {
                meta.runner = serde_json::from_value(value["runner"].clone()).ok();
                meta.engine = openagents_chat::router::engine_of(value);
            }
            meta.offers.push(offer);
        }
    }
    for followup in &extras.followups {
        let label = followup.label.trim();
        let count = label.chars().count();
        if !(1..=coder_host::access::thread::MAX_FOLLOWUP_CHARS).contains(&count)
            || label.chars().any(char::is_control)
            || meta.followups.len() >= coder_host::access::thread::MAX_FOLLOWUPS
            || meta.followups.iter().any(|kept| kept.label == label)
        {
            continue;
        }
        meta.followups.push(openagents_chat::router::Followup {
            answer: followup.answer.clone().filter(|answer| answer_tag(answer)),
            label: label.to_owned(),
        });
    }
    for card in &extras.cards {
        meta.carded(card);
    }
    (!meta.offers.is_empty() || !meta.followups.is_empty() || !meta.cards.is_empty())
        .then_some(meta)
}

fn answer_tag(text: &str) -> bool {
    (1..=96).contains(&text.len())
        && text.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._@-:".contains(&byte)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A computer's chat service in memory: sends append, and a reply
    /// streams over three reads before it finishes.
    #[derive(Default)]
    struct Fake {
        state: Mutex<FakeState>,
        sends: AtomicUsize,
        offline: std::sync::atomic::AtomicBool,
        /// Replies stream until stopped.
        slow: std::sync::atomic::AtomicBool,
        /// A computer from before `thread.stop`.
        old: std::sync::atomic::AtomicBool,
        stops: AtomicUsize,
        runs: AtomicUsize,
        /// Every `thread.send` that reached the computer.
        calls: AtomicUsize,
    }

    #[derive(Default)]
    struct FakeState {
        turns: Vec<ThreadTurn>,
        streaming: Option<usize>,
        coder: Option<coder_host::access::thread::ThreadCoder>,
        outside: Option<coder_host::access::thread::ThreadOutside>,
    }

    fn user(text: &str, request: Option<&str>) -> ThreadTurn {
        ThreadTurn {
            role: ThreadRole::User,
            text: text.into(),
            at: Some(1),
            stopped: false,
            model: None,
            request: request.map(str::to_owned),
            extras: coder_host::access::thread::ThreadExtras::default(),
        }
    }

    const THREAD: &str = "0123456789abcdef0123456789abcdef";

    impl Link for Fake {
        fn list(&self, _host: &str) -> Result<Vec<ThreadRow>, Refusal> {
            if self.offline.load(Ordering::SeqCst) {
                return Err(Refusal::Failed);
            }
            Ok(vec![ThreadRow {
                thread: THREAD.into(),
                title: "Rain".into(),
                started: 1,
                updated: 2,
                pinned: false,
                coder: None,
            }])
        }
        fn read(
            &self,
            _host: &str,
            thread: &str,
            before: Option<u64>,
        ) -> Result<ThreadPage, Refusal> {
            if self.offline.load(Ordering::SeqCst) {
                return Err(Refusal::Failed);
            }
            let mut state = self.state.lock().unwrap();
            let mut partial = String::new();
            if let Some(step) = state.streaming.as_mut() {
                *step += 1;
                partial = "Snow ".repeat(*step);
                if *step == 3 && !self.slow.load(Ordering::SeqCst) {
                    state.streaming = None;
                    state.turns.push(ThreadTurn {
                        role: ThreadRole::Assistant,
                        ..user("Snow falls.", None)
                    });
                    partial.clear();
                }
            }
            let end = before.map_or(state.turns.len(), |b| b as usize);
            let start = end.saturating_sub(2);
            Ok(ThreadPage {
                thread: thread.into(),
                title: "Rain".into(),
                start: start as u64,
                total: state.turns.len() as u64,
                turns: state.turns[start..end].to_vec(),
                busy: state.streaming.is_some(),
                partial,
                failure: None,
                coder: state.coder.clone(),
                outside: state.outside.clone(),
            })
        }
        fn send(
            &self,
            _host: &str,
            _thread: &str,
            request: &str,
            text: &str,
        ) -> Result<(), Refusal> {
            if self.offline.load(Ordering::SeqCst) {
                return Err(Refusal::Failed);
            }
            self.calls.fetch_add(1, Ordering::SeqCst);
            let mut state = self.state.lock().unwrap();
            if !state
                .turns
                .iter()
                .any(|t| t.request.as_deref() == Some(request))
            {
                self.sends.fetch_add(1, Ordering::SeqCst);
                state.turns.push(user(text, Some(request)));
                state.streaming = Some(0);
            }
            Ok(())
        }
        fn stop(&self, _host: &str, _thread: &str, request: Option<&str>) -> Result<(), Refusal> {
            self.stops.fetch_add(1, Ordering::SeqCst);
            if self.old.load(Ordering::SeqCst) {
                return Err(Refusal::NotServed);
            }
            let mut state = self.state.lock().unwrap();
            let answering = state.turns.last().and_then(|t| t.request.clone());
            if let Some(step) = state.streaming
                && answering.as_deref() == request
            {
                state.streaming = None;
                state.turns.push(ThreadTurn {
                    role: ThreadRole::Assistant,
                    stopped: true,
                    ..user(&"Snow ".repeat(step), None)
                });
            }
            Ok(())
        }

        fn run(&self, host: &str, _thread: &str) -> Result<String, Refusal> {
            self.runs.fetch_add(1, Ordering::SeqCst);
            if self.old.load(Ordering::SeqCst) {
                return Err(Refusal::NotServed);
            }
            let task = "ab".repeat(32);
            self.state.lock().unwrap().coder = Some(coder_host::access::thread::ThreadCoder {
                host: host.to_owned(),
                task: task.clone(),
                project: Some("checkout".into()),
                at: Some(1),
            });
            Ok(task)
        }
    }

    fn until(what: &str, mut done: impl FnMut() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !done() {
            assert!(Instant::now() < deadline, "{what}");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn a_listed_thread_opens_and_a_follow_up_streams_its_reply() {
        let fake = Arc::new(Fake::default());
        {
            let mut state = fake.state.lock().unwrap();
            for n in 0..4 {
                state.turns.push(user(&format!("Message {n}"), None));
            }
        }
        let link: Arc<dyn Link> = fake.clone();
        let threads = HostThreads::default();
        threads.poll(vec![("host".into(), "Studio Mac".into())], &link);
        until("the list arrives", || !threads.rows().is_empty());
        let rows = threads.rows();
        assert_eq!(
            (rows[0].0.as_str(), rows[0].1.as_str()),
            ("host", "Studio Mac")
        );
        assert_eq!(rows[0].2.title, "Rain");

        threads.open("host", THREAD, link.clone());
        until("the first page arrives", || {
            threads.shown().is_some_and(|shown| !shown.loading)
        });
        let shown = threads.shown().unwrap();
        assert_eq!((shown.start, shown.turns.len()), (2, 2));
        threads.earlier(link.clone());
        until("earlier turns arrive", || {
            threads.shown().is_some_and(|shown| shown.start == 0)
        });
        assert_eq!(threads.shown().unwrap().turns[0].text, "Message 0");

        assert!(threads.send("And in the snow?"));
        // The message shows at once, before the computer answers.
        let echo = threads.shown().unwrap();
        assert_eq!(echo.turns.last().unwrap().text, "And in the snow?");
        assert!(echo.busy && threads.live());
        assert!(!threads.send("Another"), "one follow-up at a time");
        let mut streamed = false;
        until("the reply finishes", || {
            let shown = threads.shown().unwrap();
            streamed |= !shown.partial.is_empty();
            !shown.busy && shown.turns.last().unwrap().text == "Snow falls."
        });
        assert!(streamed, "the reply streamed");
        let shown = threads.shown().unwrap();
        let texts: Vec<_> = shown.turns.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(
            texts,
            [
                "Message 0",
                "Message 1",
                "Message 2",
                "Message 3",
                "And in the snow?",
                "Snow falls."
            ]
        );
        assert_eq!(fake.sends.load(Ordering::SeqCst), 1);
        assert!(!threads.live());
        threads.close();
        assert!(threads.shown().is_none());
    }

    #[test]
    fn a_follow_up_to_an_unreachable_computer_goes_again_with_its_send_id() {
        let fake = Arc::new(Fake::default());
        fake.offline.store(true, Ordering::SeqCst);
        let link: Arc<dyn Link> = fake.clone();
        let threads = HostThreads::default();
        threads.open("host", THREAD, link);
        assert!(threads.send("Hello"));
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(fake.sends.load(Ordering::SeqCst), 0);
        fake.offline.store(false, Ordering::SeqCst);
        until("the follow-up lands once", || {
            threads
                .shown()
                .is_some_and(|shown| shown.turns.last().is_some_and(|t| t.text == "Snow falls."))
        });
        assert_eq!(fake.sends.load(Ordering::SeqCst), 1);
    }

    fn cache(dir: &std::path::Path) -> Option<Cache> {
        let secret = secp256k1::SecretKey::from_byte_array([9; 32]).unwrap();
        Some(Cache::open(dir, &secret).unwrap())
    }

    fn kept(dir: &std::path::Path) -> bool {
        std::fs::read_dir(dir).unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_str()
                .is_some_and(|name| name.starts_with(PAGE))
        })
    }

    #[test]
    fn a_relaunch_with_the_computer_off_lists_and_opens_kept_threads() {
        let temp = tempfile::tempdir().unwrap();
        let fake = Arc::new(Fake::default());
        {
            let mut state = fake.state.lock().unwrap();
            for n in 0..4 {
                state.turns.push(user(&format!("Message {n}"), None));
            }
        }
        let link: Arc<dyn Link> = fake.clone();
        {
            let threads = HostThreads::default().with_cache(cache(temp.path()));
            threads.poll(vec![("host".into(), "Studio Mac".into())], &link);
            until("the list arrives", || !threads.rows().is_empty());
            assert_eq!(threads.kept_at("host"), None, "a fresh list");
            threads.open("host", THREAD, link.clone());
            until("the first page arrives", || {
                threads.shown().is_some_and(|shown| !shown.loading)
            });
            threads.earlier(link.clone());
            until("earlier turns arrive and are kept", || {
                threads.shown().is_some_and(|shown| shown.start == 0) && kept(temp.path())
            });
            // Kept after the join, with every turn.
            until("the joined turns are kept", || {
                threads
                    .lock()
                    .open
                    .as_ref()
                    .and_then(|open| open.kept.as_ref())
                    .is_some_and(|kept| kept.start == 0 && kept.turns.len() == 4)
            });
            threads.close();
        }
        // Relaunch with the computer off: the list shows at once, marked
        // with when it was read, and the thread opens from the kept copy.
        fake.offline.store(true, Ordering::SeqCst);
        let threads = HostThreads::default().with_cache(cache(temp.path()));
        let rows = threads.rows();
        assert_eq!(rows.len(), 1);
        assert_eq!(
            (rows[0].1.as_str(), rows[0].2.title.as_str()),
            ("Studio Mac", "Rain")
        );
        let read_at = threads.kept_at("host").expect("marked as kept");
        assert!(unix_now().saturating_sub(read_at) < 60);
        threads.poll(vec![("host".into(), "Studio Mac".into())], &link);
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(
            threads.rows().len(),
            1,
            "an unreachable computer keeps its rows"
        );
        assert!(threads.kept_at("host").is_some());
        threads.open("host", THREAD, link.clone());
        let shown = threads.shown().unwrap();
        assert!(!shown.loading && !shown.busy);
        assert!(shown.kept_at.is_some_and(|at| at.abs_diff(read_at) < 60));
        assert_eq!(shown.title, "Rain");
        let texts: Vec<_> = shown.turns.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, ["Message 0", "Message 1", "Message 2", "Message 3"]);
        until("the computer is not reached", || {
            threads.shown().is_some_and(|shown| shown.error.is_some())
        });
        assert_eq!(threads.shown().unwrap().turns.len(), 4, "the copy stays");
        // The computer answers again: the read replaces the copy.
        fake.offline.store(false, Ordering::SeqCst);
        until("a read replaces the copy", || {
            threads
                .shown()
                .is_some_and(|shown| shown.kept_at.is_none() && shown.error.is_none())
        });
        until("the list is fresh again", || {
            threads.poll(vec![("host".into(), "Studio Mac".into())], &link);
            threads.kept_at("host").is_none()
        });
    }

    #[test]
    fn a_follow_up_queued_offline_goes_once_when_the_computer_is_back() {
        let temp = tempfile::tempdir().unwrap();
        let fake = Arc::new(Fake::default());
        fake.state.lock().unwrap().turns.push(user("Hello", None));
        let link: Arc<dyn Link> = fake.clone();
        {
            let threads = HostThreads::default().with_cache(cache(temp.path()));
            threads.poll(vec![("host".into(), "Studio Mac".into())], &link);
            threads.open("host", THREAD, link.clone());
            until("the thread is kept", || {
                kept(temp.path()) && !threads.rows().is_empty()
            });
            threads.close();
        }
        fake.offline.store(true, Ordering::SeqCst);
        let request = {
            let threads = HostThreads::default().with_cache(cache(temp.path()));
            threads.open("host", THREAD, link.clone());
            assert!(threads.send("And the snow?"));
            let shown = threads.shown().unwrap();
            assert!(shown.queued);
            assert_eq!(shown.turns.last().unwrap().text, "And the snow?");
            // It waits in the outbox, on disk, with its send ID.
            let queued = threads.queued();
            assert_eq!(queued.len(), 1);
            assert_eq!(queued[0].text, "And the snow?");
            assert_eq!(queued[0].request.len(), 32);
            std::thread::sleep(Duration::from_millis(100));
            threads.close();
            queued[0].request.clone()
        };
        assert_eq!(fake.sends.load(Ordering::SeqCst), 0);
        // Relaunch; the follow-up is still waiting, under the same ID, and
        // shows in its thread before the computer answers.
        let threads = HostThreads::default().with_cache(cache(temp.path()));
        assert_eq!(threads.queued()[0].request, request);
        threads.open("host", THREAD, link.clone());
        let shown = threads.shown().unwrap();
        assert!(shown.queued && shown.kept_at.is_some());
        assert_eq!(
            shown.turns.last().unwrap().request.as_deref(),
            Some(request.as_str())
        );
        threads.close();
        // The computer is back: its next list read delivers it, once.
        fake.offline.store(false, Ordering::SeqCst);
        threads.poll(vec![("host".into(), "Studio Mac".into())], &link);
        until("the follow-up is delivered", || threads.queued().is_empty());
        assert_eq!(fake.sends.load(Ordering::SeqCst), 1);
        {
            let state = fake.state.lock().unwrap();
            let sent: Vec<_> = state
                .turns
                .iter()
                .filter(|turn| turn.request.as_deref() == Some(request.as_str()))
                .collect();
            assert_eq!(sent.len(), 1);
            assert_eq!(sent[0].text, "And the snow?");
        }
        // The outbox left the disk too: another relaunch sends nothing.
        assert!(
            HostThreads::default()
                .with_cache(cache(temp.path()))
                .queued()
                .is_empty()
        );
        threads.open("host", THREAD, link.clone());
        until("the reply shows", || {
            threads
                .shown()
                .is_some_and(|shown| shown.turns.last().is_some_and(|t| t.text == "Snow falls."))
        });
        assert_eq!(fake.sends.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn a_follow_up_the_computer_took_before_a_crash_is_not_appended_twice() {
        let temp = tempfile::tempdir().unwrap();
        let fake = Arc::new(Fake::default());
        let link: Arc<dyn Link> = fake.clone();
        fake.offline.store(true, Ordering::SeqCst);
        let queued = {
            let threads = HostThreads::default().with_cache(cache(temp.path()));
            threads.open("host", THREAD, link.clone());
            assert!(threads.send("Once"));
            threads.close();
            threads.queued().remove(0)
        };
        // The computer took it, and the app died before it heard back.
        fake.offline.store(false, Ordering::SeqCst);
        link.send("host", THREAD, &queued.request, &queued.text)
            .unwrap();
        assert_eq!(fake.sends.load(Ordering::SeqCst), 1);
        let threads = HostThreads::default().with_cache(cache(temp.path()));
        threads.poll(vec![("host".into(), "Studio Mac".into())], &link);
        until("the outbox empties", || threads.queued().is_empty());
        assert_eq!(fake.sends.load(Ordering::SeqCst), 1, "the send ID held");
        assert_eq!(
            fake.calls.load(Ordering::SeqCst),
            2,
            "sent again, appended once"
        );
    }

    #[test]
    fn a_streaming_reply_stops_through_the_computer_and_keeps_its_partial() {
        let fake = Arc::new(Fake::default());
        fake.slow.store(true, Ordering::SeqCst);
        let link: Arc<dyn Link> = fake.clone();
        let threads = HostThreads::default();
        threads.open("host", THREAD, link.clone());
        until("the computer says it can stop", || {
            threads.lock().stops.get("host") == Some(&true)
        });
        // The probe changed nothing.
        assert!(fake.state.lock().unwrap().turns.is_empty());
        assert!(!threads.shown().unwrap().stoppable, "nothing streams yet");
        assert!(!threads.stop(link.clone()));
        assert!(threads.send("Tell me about snow"));
        until("the reply streams and can stop", || {
            threads
                .shown()
                .is_some_and(|shown| shown.stoppable && !shown.partial.is_empty())
        });
        assert!(threads.stop(link.clone()));
        until("the stopped reply shows", || {
            threads.shown().is_some_and(|shown| {
                !shown.busy && shown.turns.last().is_some_and(|turn| turn.stopped)
            })
        });
        let shown = threads.shown().unwrap();
        assert!(shown.stopped_here && !shown.stoppable);
        assert!(shown.turns.last().unwrap().text.starts_with("Snow"));
        // One probe and one stop.
        assert_eq!(fake.stops.load(Ordering::SeqCst), 2);
        // Another thread opened on the same computer asks no probe again.
        threads.open("host", THREAD, link);
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(fake.stops.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn an_older_computer_shows_no_stop_control() {
        let fake = Arc::new(Fake::default());
        fake.slow.store(true, Ordering::SeqCst);
        fake.old.store(true, Ordering::SeqCst);
        let link: Arc<dyn Link> = fake.clone();
        let threads = HostThreads::default();
        threads.open("host", THREAD, link.clone());
        until("the computer refused the probe", || {
            threads.lock().stops.get("host") == Some(&false)
        });
        assert!(threads.send("Tell me about snow"));
        until("the reply streams", || {
            threads
                .shown()
                .is_some_and(|shown| shown.busy && !shown.partial.is_empty())
        });
        let shown = threads.shown().unwrap();
        assert!(!shown.stoppable, "no stop control on an older computer");
        assert!(!threads.stop(link));
        assert_eq!(fake.stops.load(Ordering::SeqCst), 1, "only the probe");
    }

    #[test]
    fn a_host_threads_offer_is_read_back_and_run_starts_coder() {
        let fake = Arc::new(Fake::default());
        {
            let mut state = fake.state.lock().unwrap();
            state.turns.push(user("offer coder a haiku", None));
            let mut reply = user("Rain on the roof.", None);
            reply.role = ThreadRole::Assistant;
            reply.extras = coder_host::access::thread::ThreadExtras {
                offers: vec![serde_json::json!({"offer": "run_coder", "runner": {
                    "state": "runs", "provider": "claude", "model": "claude-opus-5-5",
                    "passed": [{"provider": "codex", "why": "near_limit", "used_percent": 92}]
                }})],
                followups: vec![coder_host::access::thread::ThreadFollowup {
                    answer: None,
                    label: "Say it shorter".into(),
                }],
                cards: vec![serde_json::json!({
                    "v": 2, "requires": [], "type": "card", "card": "news",
                    "items": [{
                        "title": "Rain", "line": "On the roof.",
                        "event": null, "path": "notes/rain"
                    }]
                })],
            };
            state.turns.push(reply);
        }
        let link: Arc<dyn Link> = fake.clone();
        let threads = HostThreads::default();
        threads.open("host", THREAD, link.clone());
        until("the reply arrives", || {
            threads.shown().is_some_and(|shown| {
                !shown.loading && shown.turns.last().is_some_and(|turn| turn.meta.is_some())
            })
        });
        let shown = threads.shown().unwrap();
        let meta = shown.turns.last().unwrap().meta.as_ref().unwrap();
        assert!(
            meta.offers
                .contains(&openagents_chat::router::Offer::RunCoder)
        );
        assert_eq!(
            meta.runner.as_ref().map(|runner| runner.text()).as_deref(),
            Some("Claude Code will do this.")
        );
        assert_eq!(meta.followups[0].label, "Say it shorter");
        assert_eq!(meta.cards[0]["card"], "news");
        assert!(meta.judgment.is_none() && meta.tier.is_none());
        assert!(shown.runnable);
        assert!(threads.run(link));
        until("Coder is linked", || {
            threads.shown().is_some_and(|shown| shown.coder.is_some())
        });
        assert_eq!(fake.runs.load(Ordering::SeqCst), 1);
        assert_eq!(
            threads.shown().unwrap().coder.unwrap().task,
            "ab".repeat(32)
        );
    }
}
