//! Filesystem notifications behind a direct connection's nudges.
//!
//! One process-wide watcher follows every history root that some direct
//! connection watches, recursively, and each root once however many
//! connections watch it. A change under a root marks each subscribed
//! connection's [`Inbox`] and wakes it; the connection then looks at what it
//! watches. The inbox only accumulates flags, so a burst of changes costs one
//! look, and nothing is lost while a connection is busy.
//!
//! Only changes that can change a chat or the chat list count: a `.jsonl`
//! file (every harness's transcripts and title indexes), Coder's
//! `archive.json`, a folder appearing, leaving, or moving in, or a change the
//! platform could not name. Reads never count, so a host reading a chat
//! cannot nudge itself.
//!
//! A folder counts because a platform may not report what is already in it:
//! inotify, on Linux, watches a new folder only once it sees it made, so a
//! chat written into a new day's folder at once is never reported.

use notify::event::{CreateKind, EventKind, ModifyKind, RemoveKind};
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use tokio::sync::Notify;

/// What changed under a connection's roots since it last looked.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Touched {
    /// A chat file or the Coder archive changed.
    pub any: bool,
    /// A chat file appeared, left, or was renamed, or a title index or the
    /// Coder archive changed: the chat list itself changed.
    pub listing: bool,
}

/// A connection's side of the watcher.
#[derive(Default)]
pub(super) struct Inbox {
    touched: Mutex<Touched>,
    wake: Notify,
}

impl Inbox {
    /// Take what changed and clear it.
    pub fn take(&self) -> Touched {
        std::mem::take(&mut *lock(&self.touched))
    }

    /// Wait until something changed.
    pub async fn changed(&self) {
        self.wake.notified().await;
    }

    fn touch(&self, listing: bool) {
        {
            let mut touched = lock(&self.touched);
            touched.any = true;
            touched.listing |= listing;
        }
        self.wake.notify_one();
    }
}

/// Roots one connection watches. Dropping it stops watching roots no other
/// connection watches.
pub(super) struct Subscription {
    id: u64,
    /// The roots as the connection named them.
    roots: Vec<PathBuf>,
    /// The canonical roots this subscription counts in [`Watching`].
    counted: Vec<PathBuf>,
    /// Whether the platform watches every root; when it does not, the
    /// connection falls back to looking often.
    pub complete: bool,
}

impl Subscription {
    pub fn roots(&self) -> &[PathBuf] {
        &self.roots
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        lock(&routes().subscribers).remove(&self.id);
        let mut watching = lock(watching());
        let Watching { watcher, watched } = &mut *watching;
        for canonical in &self.counted {
            if let Some(count) = watched.get_mut(canonical) {
                *count -= 1;
                if *count == 0 {
                    watched.remove(canonical);
                    if let Some(watcher) = watcher {
                        let _ = watcher.unwatch(canonical);
                    }
                }
            }
        }
    }
}

struct Subscriber {
    /// Each root as given and as the platform names it.
    roots: Vec<(PathBuf, PathBuf)>,
    inbox: Weak<Inbox>,
}

/// The platform watcher. Only subscribing and unsubscribing lock it; a
/// platform may wait for its event thread while it changes what it watches,
/// so the event handler never takes this lock.
#[derive(Default)]
struct Watching {
    watcher: Option<RecommendedWatcher>,
    /// Canonical roots and how many subscriptions watch each.
    watched: HashMap<PathBuf, usize>,
}

/// Where the event handler sends changes. Held only briefly.
#[derive(Default)]
struct Routes {
    subscribers: Mutex<HashMap<u64, Subscriber>>,
    /// Chat files already announced as new, so a platform that reports a
    /// file's creation again with a later write announces it once.
    announced: Mutex<HashSet<PathBuf>>,
    next: std::sync::atomic::AtomicU64,
}

/// The most created paths remembered before the record starts over.
const ANNOUNCED: usize = 4096;

fn watching() -> &'static Mutex<Watching> {
    static WATCHING: OnceLock<Mutex<Watching>> = OnceLock::new();
    WATCHING.get_or_init(Mutex::default)
}

fn routes() -> &'static Routes {
    static ROUTES: OnceLock<Routes> = OnceLock::new();
    ROUTES.get_or_init(Routes::default)
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poison| poison.into_inner())
}

/// Watch `roots` for `inbox` until the subscription is dropped.
pub(super) fn subscribe(roots: Vec<PathBuf>, inbox: &Arc<Inbox>) -> Subscription {
    let id = routes()
        .next
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let named: Vec<(PathBuf, PathBuf)> = roots
        .iter()
        .filter_map(|root| Some((root.clone(), root.canonicalize().ok()?)))
        .collect();
    // Route first, so a change made while the platform starts is not lost.
    lock(&routes().subscribers).insert(
        id,
        Subscriber {
            roots: named.clone(),
            inbox: Arc::downgrade(inbox),
        },
    );
    let mut watching = lock(watching());
    if watching.watcher.is_none() {
        watching.watcher = notify::recommended_watcher(|result: notify::Result<Event>| {
            if let Ok(event) = result {
                deliver(&event);
            }
        })
        .ok();
    }
    let mut complete = watching.watcher.is_some() && named.len() == roots.len();
    let mut counted = Vec::new();
    for (_, canonical) in named {
        if let Some(count) = watching.watched.get_mut(&canonical) {
            *count += 1;
            counted.push(canonical);
            continue;
        }
        let started = watching
            .watcher
            .as_mut()
            .is_some_and(|w| w.watch(&canonical, RecursiveMode::Recursive).is_ok());
        if started {
            watching.watched.insert(canonical.clone(), 1);
            counted.push(canonical);
        } else {
            complete = false;
        }
    }
    Subscription {
        id,
        roots,
        counted,
        complete,
    }
}

/// Whether a path names a file whose change can change a chat or the list.
fn relevant(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == "jsonl")
        || path.file_name().is_some_and(|n| n == "archive.json")
}

fn deliver(event: &Event) {
    if matches!(event.kind, EventKind::Access(_)) {
        return;
    }
    let structural = matches!(
        event.kind,
        EventKind::Create(_) | EventKind::Remove(_) | EventKind::Modify(ModifyKind::Name(_))
    );
    let routes = routes();
    if event.need_rescan() || event.paths.is_empty() {
        // The platform lost track: every connection looks at everything.
        for subscriber in lock(&routes.subscribers).values() {
            if let Some(inbox) = subscriber.inbox.upgrade() {
                inbox.touch(true);
            }
        }
        return;
    }
    // A folder appeared, left, or moved in: the chats in it may never be
    // reported, so the chat list changed.
    let folder = matches!(
        event.kind,
        EventKind::Create(CreateKind::Folder) | EventKind::Remove(RemoveKind::Folder)
    );
    for path in event.paths.iter().filter(|path| {
        !relevant(path)
            && (folder
                || (matches!(
                    event.kind,
                    EventKind::Create(_) | EventKind::Modify(ModifyKind::Name(_))
                ) && path.is_dir()))
    }) {
        touch(routes, path, true);
    }
    for path in event.paths.iter().filter(|path| relevant(path)) {
        let name = path.file_name().unwrap_or_default();
        let listing = match event.kind {
            EventKind::Create(_) => {
                let mut announced = lock(&routes.announced);
                if announced.len() >= ANNOUNCED {
                    announced.clear();
                }
                announced.insert(path.clone())
            }
            EventKind::Remove(_) => {
                lock(&routes.announced).remove(path);
                true
            }
            _ => structural || name == "session_index.jsonl" || name == "archive.json",
        };
        touch(routes, path, listing);
    }
}

/// Mark every connection that watches a root holding `path`.
fn touch(routes: &Routes, path: &Path, listing: bool) {
    for subscriber in lock(&routes.subscribers).values() {
        if subscriber
            .roots
            .iter()
            .any(|(given, canonical)| path.starts_with(canonical) || path.starts_with(given))
            && let Some(inbox) = subscriber.inbox.upgrade()
        {
            inbox.touch(listing);
        }
    }
}
