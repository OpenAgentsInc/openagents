//! The chat threads each paired computer keeps, read and continued from
//! this device.
//!
//! A computer's host keeps its own threads (`<host root>/basic-chats`): the
//! ones started in its desktop app or with `openagents chat` there. This
//! device lists them beside its own with NIP-HOST `thread.list`, reads the
//! open one with `thread.read`, and sends a follow-up with `thread.send`,
//! all on the computer's current host link (iroh or the relay) under the
//! device's grant: `observe` to read, `operate` to send. The host appends
//! the follow-up and asks OpenAgents for the reply, so the desktop and
//! `openagents chat read` show it too, and this device reads the reply as
//! it streams by reading the thread again while the host answers.
//!
//! Nothing here is kept across a relaunch, and nothing here touches this
//! device's own threads, which need no computer: when a computer is
//! offline its threads keep their last rows until the app closes.
//!
//! A thread that delegated Coder work names the task
//! ([`ThreadCoder`](coder_host::access::thread::ThreadCoder)); the Coder
//! tab opens that task's chat through the computer's history observer, the
//! same read path that carries a task's events.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use coder_computers::live::Terminals;
use coder_host::access::Code;
use coder_host::access::protocol::{Operation, Outcome};
use coder_host::access::thread::{ThreadPage, ThreadRole, ThreadRow, ThreadTurn};
use openagents_chat::basic_coder::{Role, Turn};

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
}

/// One computer's threads as last read.
#[derive(Clone, Debug)]
pub struct Listed {
    pub label: String,
    pub rows: Vec<ThreadRow>,
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
    error: Option<String>,
}

#[derive(Default)]
struct Inner {
    lists: BTreeMap<String, Listed>,
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
}

impl HostThreads {
    pub fn new(wake: Arc<dyn Fn() + Send + Sync>) -> Self {
        Self {
            inner: Arc::default(),
            wake,
        }
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
                let wait = match answer {
                    Ok(rows) => {
                        let listed = Listed { label, rows };
                        let same = inner.lists.get(&host).is_some_and(|old| {
                            old.rows == listed.rows && old.label == listed.label
                        });
                        inner.lists.insert(host.clone(), listed);
                        if !same {
                            threads.changed(&mut inner);
                        }
                        LIST_EVERY
                    }
                    Err(Refusal::NotServed) => {
                        if inner.lists.remove(&host).is_some() {
                            threads.changed(&mut inner);
                        }
                        NOT_SERVED
                    }
                    Err(_) => LIST_EVERY,
                };
                inner.next.insert(host, Instant::now() + wait);
            });
        }
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
            inner.open = Some(Opened {
                host: host.to_owned(),
                thread: thread.to_owned(),
                generation,
                page: None,
                turns: Vec::new(),
                turns_start: 0,
                loading_earlier: false,
                sending: None,
                error: None,
            });
            self.changed(&mut inner);
            generation
        };
        let threads = self.clone();
        let (host, thread) = (host.to_owned(), thread.to_owned());
        std::thread::spawn(move || threads.follow(&host, &thread, generation, &*link));
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
        let Some(open) = inner.open.as_mut() else {
            return false;
        };
        if open.sending.is_some() || open.page.as_ref().is_some_and(|page| page.busy) {
            return false;
        }
        open.sending = Some(Sending {
            request: uuid::Uuid::new_v4().simple().to_string(),
            text: text.to_owned(),
            since: Instant::now(),
            accepted: false,
        });
        open.error = None;
        self.changed(&mut inner);
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
                let answer = link.send(host, thread, &sending.request, &sending.text);
                let mut inner = self.lock();
                let Some(open) = inner.open.as_mut().filter(|o| o.generation == generation) else {
                    return;
                };
                match answer {
                    Ok(()) => {
                        if let Some(held) = open.sending.as_mut() {
                            held.accepted = true;
                        }
                    }
                    Err(Refusal::Failed) if sending.since.elapsed() < SEND_PATIENCE => {
                        drop(inner);
                        std::thread::sleep(RESEND);
                        continue;
                    }
                    Err(refusal) => {
                        open.sending = None;
                        open.error = Some(words(&refusal));
                    }
                }
                self.changed(&mut inner);
            }
            let answer = link.read(host, thread, None);
            let pause = {
                let mut inner = self.lock();
                let Some(open) = inner.open.as_mut().filter(|o| o.generation == generation) else {
                    return;
                };
                let mut changed = false;
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
                        if open.page.as_ref() != Some(&page) || open.error.is_some() {
                            open.page = Some(page);
                            open.error = None;
                            changed = true;
                        }
                    }
                    Err(refusal) => {
                        let error = Some(words(&refusal));
                        if open.error != error {
                            open.error = error;
                            changed = true;
                        }
                    }
                }
                let quick =
                    open.sending.is_some() || open.page.as_ref().is_some_and(|page| page.busy);
                if changed {
                    self.changed(&mut inner);
                }
                if quick { STREAMING } else { SETTLED }
            };
            std::thread::sleep(pause);
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
    out
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
    }

    #[derive(Default)]
    struct FakeState {
        turns: Vec<ThreadTurn>,
        streaming: Option<usize>,
    }

    fn user(text: &str, request: Option<&str>) -> ThreadTurn {
        ThreadTurn {
            role: ThreadRole::User,
            text: text.into(),
            at: Some(1),
            stopped: false,
            model: None,
            request: request.map(str::to_owned),
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
                if *step == 3 {
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
                coder: None,
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
}
