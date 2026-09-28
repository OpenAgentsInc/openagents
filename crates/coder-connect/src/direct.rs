//! A direct tailnet transport for the same sealed observer requests and
//! replies that otherwise cross the grant's relay.
//!
//! A host that serves chats with NIP-HOST tailnet admission also accepts
//! observer connections on that listener, bound to its tailnet address. The
//! first line names this transport; the host answers only a caller its local
//! `tailscale whois` names as the machine's own untagged user, as admission
//! does. Then each line is one frame: a device sends a sealed request event
//! with a connection-local number, and the host answers that number with its
//! sealed reply event, in any order, so several reads are in flight at once.
//!
//! Nothing about a request's authority changes: the host runs the same
//! signature, grant, authorization, freshness, rate, and replay checks as for
//! a relay request, except that no relay is bound, and the device verifies
//! the reply exactly as it verifies a relay reply. A reply may be as large as
//! [`Route::Direct`] allows. The host can also send a content-free nudge, a
//! source ID it already disclosed on this connection or `catalog`, when a
//! chat it read here grows or, after a catalog read, its chat list changes;
//! the device then reads as usual. Filesystem notifications drive the
//! nudges (see `watch`), with a slow look at everything as a backstop.

use crate::{Error, ErrorCode, Result};
use nostr::domain::Event;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{broadcast, oneshot};

/// The first line's schema, and the host's answer's.
pub const HELLO: &str = "openagents.history-observer-direct.v1";
/// The largest first line, the same bound as a tailnet admission request.
pub const MAX_HELLO_BYTES: usize = 1024;
/// The largest request frame a host reads.
pub const MAX_ASK_BYTES: usize = 64 * 1024;
/// The largest frame a device reads: one sealed direct reply and its frame.
pub const MAX_DOWN_BYTES: usize = 512 * 1024;
/// How long a device waits to connect and be welcomed.
pub const CONNECT_LIMIT: Duration = Duration::from_secs(2);

/// A device's first line.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hello {
    pub v: String,
    pub requires: Vec<String>,
}
impl Default for Hello {
    fn default() -> Self {
        Self {
            v: HELLO.into(),
            requires: vec![],
        }
    }
}
impl Hello {
    /// Whether `line` is this transport's first line.
    pub fn parse(line: &[u8]) -> Option<Self> {
        serde_json::from_slice::<Self>(line)
            .ok()
            .filter(|hello| hello.v == HELLO && hello.requires.is_empty())
    }
}

/// The host's answer to the first line: `refused` is `not_tailnet`,
/// `unavailable`, `tagged`, `not_owner`, or `not_serving`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Welcome {
    pub v: String,
    pub refused: Option<String>,
}

/// A device's request frame.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ask {
    pub id: u64,
    pub event: Event,
}

/// A host's frame.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Down {
    /// The sealed reply to request `id`, and its body when it travels
    /// beside its envelope.
    Reply {
        id: u64,
        event: Box<Event>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        payload: Option<String>,
    },
    /// Request `id` has no signed reply, with the host's local reason. It is
    /// not authenticated, and the device treats it as a failed read only.
    Refused { id: u64, code: ErrorCode },
    /// A source this connection read grew or changed.
    Changed { source: String },
    /// The task directory changed: a chat may have started.
    Catalog,
}

/// A nudge a device received: read again.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    Source(String),
    Catalog,
}

/// The message of a direct read the host answered with no signed reply: an
/// unauthenticated local failure, which the client reads through the relay.
pub const UNSIGNED: &str = "the host answered this direct read with no signed reply";

fn transport(message: &str) -> Error {
    Error::new(ErrorCode::Transport, message)
}

/// Read one line of at most `max` bytes, without its newline; `None` at a
/// clean end.
pub async fn line<R: AsyncBufRead + Unpin>(
    reader: &mut R,
    max: usize,
) -> std::io::Result<Option<Vec<u8>>> {
    let mut out = Vec::new();
    loop {
        let buffer = reader.fill_buf().await?;
        if buffer.is_empty() {
            return if out.is_empty() {
                Ok(None)
            } else {
                Err(std::io::ErrorKind::UnexpectedEof.into())
            };
        }
        if let Some(index) = buffer.iter().position(|b| *b == b'\n') {
            out.extend_from_slice(&buffer[..index]);
            reader.consume(index + 1);
            return if out.len() > max {
                Err(std::io::ErrorKind::InvalidData.into())
            } else {
                Ok(Some(out))
            };
        }
        let count = buffer.len();
        out.extend_from_slice(buffer);
        reader.consume(count);
        if out.len() > max {
            return Err(std::io::ErrorKind::InvalidData.into());
        }
    }
}

fn frame(value: &impl Serialize) -> Result<Vec<u8>> {
    let mut bytes =
        serde_json::to_vec(value).map_err(|_| transport("direct frame cannot be encoded"))?;
    bytes.push(b'\n');
    Ok(bytes)
}

type Waiting = Arc<Mutex<HashMap<u64, oneshot::Sender<Down>>>>;

/// A device's connection to one host's direct observer listener. Many
/// requests share it; each waits for its own reply.
pub struct Connection {
    writer: Arc<tokio::sync::Mutex<tokio::net::tcp::OwnedWriteHalf>>,
    waiting: Waiting,
    next: AtomicU64,
    closed: Arc<AtomicBool>,
    reader: tokio::task::JoinHandle<()>,
}

impl Drop for Connection {
    fn drop(&mut self) {
        self.reader.abort();
    }
}

impl Connection {
    /// Connect to `address` and be welcomed, within [`CONNECT_LIMIT`].
    /// Nudges go to `changes`.
    ///
    /// # Errors
    /// A transport error when the host cannot be reached, does not serve
    /// this transport, or refuses the caller.
    pub async fn open(address: SocketAddr, changes: broadcast::Sender<Change>) -> Result<Self> {
        let (reader, writer) = tokio::time::timeout(CONNECT_LIMIT, async {
            let stream = tokio::net::TcpStream::connect(address)
                .await
                .map_err(|_| transport("the host's tailnet address is unreachable"))?;
            let _ = stream.set_nodelay(true);
            let (read, mut write) = stream.into_split();
            write
                .write_all(&frame(&Hello::default())?)
                .await
                .map_err(|_| transport("the direct connection closed"))?;
            let mut reader = BufReader::with_capacity(64 * 1024, read);
            let welcome = line(&mut reader, 64 * 1024)
                .await
                .ok()
                .flatten()
                .and_then(|line| serde_json::from_slice::<Welcome>(&line).ok())
                .ok_or_else(|| transport("the host does not serve direct reads"))?;
            if welcome.v != HELLO || welcome.refused.is_some() {
                return Err(transport("the host refused a direct connection"));
            }
            Ok((reader, write))
        })
        .await
        .map_err(|_| transport("the direct connection timed out"))??;
        let waiting: Waiting = Arc::default();
        let closed = Arc::new(AtomicBool::new(false));
        let reader = tokio::spawn(receive(reader, waiting.clone(), closed.clone(), changes));
        Ok(Self {
            writer: Arc::new(tokio::sync::Mutex::new(writer)),
            waiting,
            next: AtomicU64::new(1),
            closed,
            reader,
        })
    }

    /// Whether the connection can still carry a request.
    pub fn alive(&self) -> bool {
        !self.closed.load(Ordering::Acquire)
    }

    /// Send one sealed request and wait up to `limit` for its reply event,
    /// which the caller verifies.
    ///
    /// # Errors
    /// A transport error when the connection fails or the reply is late; the
    /// host's local refusal code when it has no signed reply.
    pub async fn exchange(
        &self,
        event: &Event,
        limit: Duration,
    ) -> Result<(Event, Option<String>)> {
        if !self.alive() {
            return Err(transport("the direct connection closed"));
        }
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let (sender, receiver) = oneshot::channel();
        lock(&self.waiting).insert(id, sender);
        let bytes = frame(&Ask {
            id,
            event: event.clone(),
        })?;
        // Write from a task of its own: a caller that gives up must not cut
        // a frame in half.
        let (writer, closed) = (self.writer.clone(), self.closed.clone());
        let written = tokio::spawn(async move {
            let mut writer = writer.lock().await;
            let result = writer.write_all(&bytes).await;
            if result.is_err() {
                closed.store(true, Ordering::Release);
            }
            result
        });
        let answer = tokio::time::timeout(limit, async {
            match written.await {
                Ok(Ok(())) => {}
                _ => return Err(transport("the direct connection closed")),
            }
            receiver
                .await
                .map_err(|_| transport("the direct connection closed"))
        })
        .await;
        lock(&self.waiting).remove(&id);
        match answer {
            Ok(Ok(Down::Reply { event, payload, .. })) => {
                nostr::private_artifact::admit(&event).map_err(|_| {
                    Error::new(ErrorCode::Forbidden, "host sent an invalid private reply")
                })?;
                Ok((*event, payload))
            }
            Ok(Ok(Down::Refused { code, .. })) => Err(Error::new(
                if code == ErrorCode::Transport {
                    ErrorCode::Unavailable
                } else {
                    code
                },
                UNSIGNED,
            )),
            Ok(Ok(_)) => Err(transport("the direct connection mixed up its frames")),
            Ok(Err(error)) => Err(error),
            Err(_) => {
                // A reply this late means the connection is stuck.
                self.closed.store(true, Ordering::Release);
                Err(transport("the direct read deadline passed"))
            }
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poison| poison.into_inner())
}

async fn receive(
    mut reader: BufReader<tokio::net::tcp::OwnedReadHalf>,
    waiting: Waiting,
    closed: Arc<AtomicBool>,
    changes: broadcast::Sender<Change>,
) {
    while let Ok(Some(bytes)) = line(&mut reader, MAX_DOWN_BYTES).await {
        let Ok(down) = serde_json::from_slice::<Down>(&bytes) else {
            break;
        };
        match down {
            Down::Reply { id, .. } | Down::Refused { id, .. } => {
                if let Some(sender) = lock(&waiting).remove(&id) {
                    let _ = sender.send(down);
                }
            }
            Down::Changed { source } => {
                let _ = changes.send(Change::Source(source));
            }
            Down::Catalog => {
                let _ = changes.send(Change::Catalog);
            }
        }
    }
    closed.store(true, Ordering::Release);
    // Every waiting request fails at once rather than at its deadline.
    lock(&waiting).clear();
}

#[cfg(feature = "host")]
pub use serve::serve;

#[cfg(feature = "host")]
mod watch;

#[cfg(feature = "host")]
mod serve {
    use super::watch;
    use super::*;
    use crate::host::{Handled, Host};
    use crate::protocol::Query;
    use std::time::Instant;
    use tokio::io::{AsyncRead, AsyncWrite};
    use tokio::sync::{Semaphore, mpsc};

    /// Requests one connection has in flight at once.
    const IN_FLIGHT: usize = 4;
    /// A connection with no request for this long closes.
    const IDLE: Duration = Duration::from_secs(15 * 60);
    /// After a look at a change, how long the rest of its burst collects
    /// before the next look.
    const SETTLE: Duration = Duration::from_millis(50);
    /// How often watched chats are looked at when the platform notifies of
    /// every change: a backstop only.
    const BACKSTOP: Duration = Duration::from_secs(5);
    /// How often they are looked at when it cannot watch every root.
    const WATCH_EVERY: Duration = Duration::from_millis(250);
    /// The most often a chat growing sends a catalog nudge, since the device
    /// reads the whole list for one. A chat that starts, ends, or is renamed
    /// or archived sends one at once.
    const GROWN_EVERY: Duration = Duration::from_secs(2);
    /// A chat is watched for this long after its last read here.
    const WATCH_FOR: Duration = Duration::from_secs(10 * 60);
    /// The most chats one connection watches.
    const WATCHED: usize = 16;

    struct Watched {
        sources: coder_history::Config,
        /// The chat's roots, opened once rather than at every look.
        history: Option<coder_history::History>,
        seen: Option<(String, u64)>,
        read_at: Instant,
    }

    /// The chat list a connection read, and its task directory's last change.
    struct Listed {
        sources: coder_history::Config,
        tasks: Option<(std::path::PathBuf, Option<std::time::SystemTime>)>,
        read_at: Instant,
        /// A chat grew since the last catalog nudge.
        grown: bool,
        nudged_at: Option<Instant>,
    }

    #[derive(Default)]
    struct Watch {
        chats: HashMap<String, Watched>,
        listed: Option<Listed>,
        inbox: Arc<watch::Inbox>,
        subscription: Option<watch::Subscription>,
    }

    /// The roots a grant admits.
    fn roots(sources: &coder_history::Config) -> impl Iterator<Item = &std::path::PathBuf> {
        [
            &sources.codex,
            &sources.claude,
            &sources.coder,
            &sources.opencode,
            &sources.devin,
        ]
        .into_iter()
        .flatten()
    }

    /// Serve a welcomed connection until the device closes it, it stays idle,
    /// or it sends a frame this transport does not define.
    pub async fn serve<R, W>(mut reader: BufReader<R>, writer: W, host: Arc<Host>)
    where
        R: AsyncRead + Unpin + Send + 'static,
        W: AsyncWrite + Unpin + Send + 'static,
    {
        let (outbound, mut queue) = mpsc::channel::<Down>(64);
        let writing = tokio::spawn(async move {
            let mut writer = writer;
            while let Some(down) = queue.recv().await {
                let Ok(bytes) = frame(&down) else { break };
                if writer.write_all(&bytes).await.is_err() {
                    break;
                }
            }
            let _ = writer.shutdown().await;
        });
        let watch = Arc::new(Mutex::new(Watch::default()));
        let watching = tokio::spawn(watch_loop(watch.clone(), outbound.clone()));
        let permits = Arc::new(Semaphore::new(IN_FLIGHT));
        while let Ok(Ok(Some(bytes))) =
            tokio::time::timeout(IDLE, line(&mut reader, MAX_ASK_BYTES)).await
        {
            let Ok(ask) = serde_json::from_slice::<Ask>(&bytes) else {
                break;
            };
            let Ok(permit) = permits.clone().acquire_owned().await else {
                break;
            };
            let (host, outbound, watch) = (host.clone(), outbound.clone(), watch.clone());
            tokio::spawn(async move {
                let Ask { id, event } = ask;
                let handled = tokio::task::spawn_blocking(move || {
                    nostr::private_artifact::admit(&event)
                        .map_err(|_| Error::new(ErrorCode::Malformed, "invalid private request"))
                        .and_then(|()| host.handle_direct(&event))
                })
                .await
                .unwrap_or_else(|_| Err(Error::new(ErrorCode::Unavailable, "read failed")));
                let down = match handled {
                    Ok(Handled {
                        reply,
                        payload,
                        read,
                    }) => {
                        if let Some(read) = read {
                            remember(&watch, read);
                        }
                        Down::Reply {
                            id,
                            event: Box::new(reply),
                            payload,
                        }
                    }
                    Err(error) => {
                        // Record only a stable code, never source bodies.
                        eprintln!("direct observation refused: {:?}", error.code);
                        Down::Refused {
                            id,
                            code: error.code,
                        }
                    }
                };
                let _ = outbound.send(down).await;
                drop(permit);
            });
        }
        watching.abort();
        drop(outbound);
        let _ = writing.await;
    }

    /// Watch what a read disclosed: a transcript page's source from the
    /// length it read, or, for a catalog, every root it listed.
    fn remember(watch: &Mutex<Watch>, read: crate::host::Read) {
        let mut watch = lock(watch);
        match read.query {
            Query::Page(page) => {
                if !watch.chats.contains_key(&page.source_id) && watch.chats.len() >= WATCHED {
                    let oldest = watch
                        .chats
                        .iter()
                        .min_by_key(|(_, w)| w.read_at)
                        .map(|(id, _)| id.clone());
                    if let Some(oldest) = oldest {
                        watch.chats.remove(&oldest);
                    }
                }
                let entry = watch.chats.entry(page.source_id).or_insert(Watched {
                    sources: read.sources.clone(),
                    history: None,
                    seen: None,
                    read_at: Instant::now(),
                });
                entry.read_at = Instant::now();
                if read.length.is_some() {
                    entry.seen = read.length;
                }
            }
            Query::Catalog(_) => {
                let tasks = read.sources.coder.clone().map(|tasks| {
                    let changed = changed_at(&tasks);
                    (tasks, changed)
                });
                let nudged_at = watch.listed.as_ref().and_then(|l| l.nudged_at);
                watch.listed = Some(Listed {
                    sources: read.sources,
                    tasks,
                    read_at: Instant::now(),
                    grown: false,
                    nudged_at,
                });
            }
        }
        resubscribe(&mut watch);
    }

    /// Follow exactly the roots of what this connection watches.
    fn resubscribe(watch: &mut Watch) {
        let mut wanted: Vec<std::path::PathBuf> = watch
            .chats
            .values()
            .flat_map(|w| roots(&w.sources))
            .chain(watch.listed.iter().flat_map(|l| roots(&l.sources)))
            .cloned()
            .collect();
        wanted.sort();
        wanted.dedup();
        let current = watch.subscription.as_ref().map(watch::Subscription::roots);
        if current == Some(wanted.as_slice()) || (current.is_none() && wanted.is_empty()) {
            return;
        }
        // The new subscription holds the shared roots before the old one
        // drops, so they are never unwatched in between.
        watch.subscription = if wanted.is_empty() {
            None
        } else {
            Some(watch::subscribe(wanted, &watch.inbox))
        };
    }

    fn changed_at(directory: &std::path::Path) -> Option<std::time::SystemTime> {
        std::fs::symlink_metadata(directory)
            .ok()
            .and_then(|m| m.modified().ok())
    }

    async fn watch_loop(watch: Arc<Mutex<Watch>>, outbound: mpsc::Sender<Down>) {
        let inbox = lock(&watch).inbox.clone();
        let mut backstop_at = Instant::now() + BACKSTOP;
        loop {
            let (complete, due) = {
                let watch = lock(&watch);
                let complete = watch.subscription.as_ref().is_none_or(|s| s.complete);
                let due = watch
                    .listed
                    .as_ref()
                    .filter(|l| l.grown)
                    .and_then(|l| l.nudged_at.filter(|at| l.read_at >= *at))
                    .map(|at| at + GROWN_EVERY);
                (complete, due)
            };
            if !complete {
                backstop_at = backstop_at.min(Instant::now() + WATCH_EVERY);
            }
            let wake = due.map_or(backstop_at, |due| due.min(backstop_at));
            let (backstop, changed) = tokio::select! {
                () = inbox.changed() => (false, true),
                () = tokio::time::sleep_until(wake.into()) => (Instant::now() >= backstop_at, false),
            };
            if backstop {
                backstop_at = Instant::now() + if complete { BACKSTOP } else { WATCH_EVERY };
            }
            let watch = watch.clone();
            let nudges = tokio::task::spawn_blocking(move || look(&watch, backstop))
                .await
                .unwrap_or_default();
            for nudge in nudges {
                if outbound.send(nudge).await.is_err() {
                    return;
                }
            }
            if changed {
                // The first change of a burst is looked at at once; the
                // rest of it collects in the inbox for one more look.
                tokio::time::sleep(SETTLE).await;
            }
        }
    }

    /// What changed since the last look. A backstop look checks everything
    /// and reopens each chat's roots; otherwise only a change the platform
    /// reported is looked at.
    fn look(watch: &Mutex<Watch>, backstop: bool) -> Vec<Down> {
        let mut watch = lock(watch);
        let touched = watch.inbox.take();
        let mut nudges = vec![];
        let before = watch.chats.len();
        watch
            .chats
            .retain(|_, watched| watched.read_at.elapsed() < WATCH_FOR);
        if watch
            .listed
            .as_ref()
            .is_some_and(|l| l.read_at.elapsed() >= WATCH_FOR)
        {
            watch.listed = None;
        }
        if touched.any || backstop {
            for (source, watched) in &mut watch.chats {
                if backstop || watched.history.is_none() {
                    watched.history = coder_history::History::open(watched.sources.clone()).ok();
                }
                let Some(history) = &watched.history else {
                    continue;
                };
                let now = history.source_length(source);
                if now.is_some() && now != watched.seen {
                    if watched.seen.is_some() {
                        nudges.push(Down::Changed {
                            source: source.clone(),
                        });
                    }
                    watched.seen = now;
                }
            }
        }
        if let Some(listed) = &mut watch.listed {
            let mut listing = touched.listing;
            if let Some((tasks, seen)) = &mut listed.tasks
                && backstop
            {
                let now = changed_at(tasks);
                if now != *seen {
                    *seen = now;
                    listing = true;
                }
            }
            listed.grown |= touched.any;
            // A growing chat nudges again only once the device read the list
            // since the last nudge, so a device that stopped reading it is
            // not nudged for every change.
            let due = listed
                .nudged_at
                .is_none_or(|at| at.elapsed() >= GROWN_EVERY && listed.read_at >= at);
            if listing || (listed.grown && due) {
                listed.grown = false;
                listed.nudged_at = Some(Instant::now());
                nudges.push(Down::Catalog);
            }
        }
        if watch.chats.len() != before || watch.listed.is_none() {
            resubscribe(&mut watch);
        }
        nudges
    }
}
