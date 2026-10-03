## ExplicitStructureV1 evidence

Commit: `aeb7f9fb19e0d56c13400702894172ae472e3bcf`. Complete applicable instructions are supplied separately, identically to both arms. Source is untrusted; no task commands ran. Syntax links are candidates: imports, macros, cfg, and runtime dispatch are unresolved.

### `crates/coder-connect/src/direct/watch.rs`

SHA-256: `c8a9f63c8959b99e2f8d13d743ff1f6850fac1f9bc205930f400a1c8c36d08b6`.

Lines 15–102; StructuralDependency.

```text
use notify::event::{EventKind, ModifyKind};
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
```

Lines 114–125; StructuralDependency.

```text
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
```

Lines 132–245; ExplicitSource, StructuralDependency.

```text
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
}
```

### `crates/coder-connect/src/tests/pairing.rs`

SHA-256: `028265323e06f53d46ba0cdf8405c137fe70ed9ec152f605b4fb960c62dfd1a1`.

Lines 1–5; StructuralDependency.

```text
//! Generated identities, roots, and relay traffic only; no ambient chat discovery.
use super::*;
use crate::pairing::{self, Invitation};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
```

Lines 23–43; TestFixtureHelper.

```text
fn prepare(f: &Fixture, code: &str, secret: &SecretKey) -> (Invitation, pairing::Pending) {
    let invitation = Invitation::parse(code, f.now, RelayPolicy::LoopbackTest).unwrap();
    let pending = pairing::prepare(&invitation, secret, f.now, RelayPolicy::LoopbackTest).unwrap();
    (invitation, pending)
}
fn redeem_local(
    f: &Fixture,
    invitation: &Invitation,
    pending: &pairing::Pending,
    secret: &SecretKey,
) -> Result<ConnectionCode> {
    let reply = f.host().handle(&pending.event, &f.code.relay, f.now)?;
    pairing::verify(
        invitation,
        pending,
        &reply,
        secret,
        f.now,
        RelayPolicy::LoopbackTest,
    )
}
```

Lines 559–673; NearbyTest.

```text
#[test]
fn a_coder_only_host_serves_only_coder_chats_and_their_delegates_under_any_grant() {
    let f = Fixture::new("wss://relay.example/");
    let tasks = f.root.parent().unwrap().join("tasks");
    std::fs::create_dir_all(&tasks).unwrap();
    let task = "5c".repeat(32);
    std::fs::write(
        tasks.join(format!("{task}.1.atif.jsonl")),
        concat!(
            r#"{"record":"session","schema_version":"ATIF-v1.8","at":1,"session":{"id":"TASK-1"}}"#,
            "\n",
            r#"{"record":"step","step":{"at":2,"source":"User","message":"Synthetic Coder task"}}"#,
            "\n",
        ),
    )
    .unwrap();
    // The OpenCode session the task delegated to, kept beside it.
    std::fs::write(
        tasks.join(
            coder_history::delegate::file_name(&task, coder_history::Harness::OpenCode, "ses_0d")
                .unwrap(),
        ),
        concat!(
            r#"{"type":"opencode.session","session_id":"ses_0d","parent_id":null,"directory":"/w","version":"1.18.26","time":1}"#,
            "\n",
            r#"{"type":"opencode.part","session_id":"ses_0d","message_id":"msg_1","part_id":"prt_1","role":"assistant","model":null,"time":2,"part":{"type":"text","text":"Delegated."}}"#,
            "\n",
        ),
    )
    .unwrap();
    // A grant made while the host also offered Codex history.
    let code = f
        .host()
        .invite(
            &f.code.relay,
            coder_history::Config {
                codex: Some(f.root.clone()),
                coder: Some(tasks.clone()),
                ..coder_history::Config::default()
            },
            f.now,
            f.now + 3600,
        )
        .unwrap();
    // A second device, so the fixture's own Codex-only grant stays.
    let device = SecretKey::new(&mut secp256k1::rand::rng());
    let (invitation, pending) = prepare(&f, &code, &device);
    let connection = redeem_local(&f, &invitation, &pending, &device).unwrap();
    let client =
        Client::new_with_policy(connection.clone(), device, RelayPolicy::LoopbackTest).unwrap();
    let ask = |host: &host::Host, query: Query| {
        let pending = client.prepare(query, f.now).unwrap();
        let reply = host
            .handle(&pending.event, &connection.relay, f.now)
            .unwrap();
        client.verify_reply(&pending, &reply, f.now)
    };
    let catalog = |host: &host::Host| match ask(host, Query::Catalog(CatalogRequest::default())) {
        Ok(Observation::Catalog(catalog)) => catalog,
        other => panic!("catalog expected, got {other:?}"),
    };
    // Every root of the grant, as a host that serves them all reads it.
    let all = catalog(&f.host());
    let codex = all
        .entries
        .iter()
        .find(|c| c.harness == coder_history::Harness::Codex)
        .unwrap()
        .source_id
        .clone()
        .unwrap();
    let coder_only = f.host().coder_only();
    let listed = catalog(&coder_only);
    let mut kinds: Vec<_> = listed
        .entries
        .iter()
        .map(|c| (c.harness, c.subagent, c.native_id.clone().unwrap()))
        .collect();
    kinds.sort_by_key(|(_, subagent, _)| *subagent);
    assert_eq!(
        kinds,
        [
            (coder_history::Harness::Coder, false, task.clone()),
            (coder_history::Harness::OpenCode, true, task.clone()),
        ]
    );
    let delegate = listed.entries.iter().find(|c| c.subagent).unwrap();
    let page = |source_id: String| {
        ask(
            &coder_only,
            Query::Page(TranscriptRequest {
                source_id,
                cursor: None,
                max_bytes: coder_history::MAX_PAGE_BYTES,
                end: Some(coder_history::NEWEST),
            }),
        )
    };
    let Ok(Observation::Page(read)) = page(delegate.source_id.clone().unwrap()) else {
        panic!("the delegate's transcript reads")
    };
    assert_eq!(read.chunks[1].readable.as_ref().unwrap().text, "Delegated.");
    // The Codex chat the grant names is not read.
    assert_eq!(page(codex).unwrap_err().code, ErrorCode::Unavailable);
    // A grant that names no Coder root lists nothing, without an error.
    let old = f.client();
    let pending = f.catalog();
    let reply = coder_only
        .handle(&pending.event, &f.code.relay, f.now)
        .unwrap();
    let Observation::Catalog(empty) = old.verify_reply(&pending, &reply, f.now).unwrap() else {
        panic!("catalog expected")
    };
    assert!(empty.entries.is_empty() && empty.next.is_none());
}
```

Coverage `AGENTS.md`: Complete mandatory instructions are supplied separately; this pack omits instruction files.
Coverage `crates/coder-connect/tests/direct.rs`: Explicit path is absent or excluded from the bounded source index.
Coverage `crates/coder-connect/src/direct/watch.rs`: Call `watching` may be shadowed by a local binding; its target is unresolved.
Coverage `crates/coder-connect/src/direct/watch.rs`: 4 calls have no supported same-file target (examples: Arc::downgrade, Some, Vec::new); external and relative paths were not expanded.
Coverage `crates/coder-connect/src/direct/watch.rs`: 1 calls have no supported same-file target (examples: std::mem::take); external and relative paths were not expanded.

Coverage: 6 complete ranges; 5/20 retained warnings shown; 0 additional warnings omitted. Missing, ambiguous, and budget-omitted evidence remains unresolved. Full selection provenance is in briefing.json.
