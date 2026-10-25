//! One chat's chat2 room membership: the viewer sink (remote rows → the
//! session doc), durable local publication (own commands/queue rows → the
//! outbox → the room), debounced snapshot+cursor persistence, and the join
//! loop that NEVER gives up (the legacy one-shot join left a chat dark until
//! relaunch).
//!
//! Shape follows the engine's `EngineChatSink` / `ChatPersistence`, minus
//! host duties (checkpoint posting, command execution).

use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use futures::future::BoxFuture;
use loro::VersionVector;
use openagents_doc::SessionDoc;
use openagents_sync::chat_client::{ChatTransport, RowImportOutcome};
use openagents_sync::{
    ChatClient, ChatDocSink, ChatEvent, CheckpointFetcher, DocsStore, SyncError, UrlProvider,
};
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

use super::{Bearer, JOIN_RETRY_BASE, JOIN_RETRY_CAP, urls};
use crate::lock;

/// chat2 doc epoch stamped on every snapshot (pre-chat2 epochs are ignored).
pub(crate) const CHAT2_DOC_EPOCH: u32 = 2;
/// Snapshot flush cadence while rows stream in (a small chat).
const SAVE_INTERVAL: Duration = Duration::from_secs(1);
/// Longest wait between saves of a very large chat.
const SAVE_INTERVAL_MAX: Duration = Duration::from_secs(60);
/// Snapshot bytes that add a second to the wait between saves.
const SAVE_BYTES_PER_SECOND: usize = 2 * 1024 * 1024;

/// How long to wait between saves of a chat whose last snapshot was
/// `snapshot_len` bytes. Every save exports and rewrites the whole snapshot,
/// so a 90 MB chat saved each second wrote gigabytes and held hundreds of
/// megabytes in flight (build 1017's out-of-memory kills).
pub(crate) fn save_interval(snapshot_len: usize) -> Duration {
    let seconds = (snapshot_len / SAVE_BYTES_PER_SECOND) as u64;
    Duration::from_secs(seconds).clamp(SAVE_INTERVAL, SAVE_INTERVAL_MAX)
}
/// How often the live-poll task checks whether a catch-up is due.
const LIVE_POLL_TICK: Duration = Duration::from_millis(250);
/// A catch-up that never completes (gap repair, redial) is retried after this.
const LIVE_POLL_STALE: Duration = Duration::from_secs(10);
/// Live-poll cadence while a send waits on the host or a turn runs.
pub(crate) const LIVE_POLL_ACTIVE: Duration = Duration::from_secs(2);
/// Live-poll cadence while the chat is on screen but quiet.
pub(crate) const LIVE_POLL_IDLE: Duration = Duration::from_secs(10);

/// Loaded local state for a chat (instant open).
pub(crate) struct LocalDoc {
    pub doc: SessionDoc,
    pub cursor: u64,
    /// A snapshot or outbox existed — content is on screen immediately.
    pub had_content: bool,
}

/// Restore a chat doc from disk: the chat2 snapshot (epoch ≥ 2 only — a
/// retired s2 lineage is never imported) plus any unacknowledged own writes.
pub(crate) fn load_local(store: &DocsStore, chat_id: &str) -> LocalDoc {
    let raw = loro::LoroDoc::new();
    let mut cursor = 0;
    let mut had_content = false;
    match store.load_snapshot_with_cursor(chat_id) {
        Ok(Some((bytes, stored_cursor, epoch))) if epoch >= CHAT2_DOC_EPOCH => {
            match raw.import(&bytes) {
                Ok(_) => {
                    had_content = true;
                    if store.snapshot_cursor_verified(chat_id).unwrap_or(false) {
                        cursor = stored_cursor;
                    }
                }
                Err(err) => {
                    tracing::warn!(chat = %chat_id, error = %err, "chat snapshot unreadable; starting fresh")
                }
            }
        }
        Ok(_) => {}
        Err(err) => tracing::warn!(chat = %chat_id, error = %err, "chat snapshot load failed"),
    }
    if let Ok(pending) = store.pending_chat_updates(chat_id) {
        for (_, bytes) in pending {
            if raw.import(&bytes).is_ok() {
                had_content = true;
            }
        }
    }
    LocalDoc {
        doc: SessionDoc::from_doc(raw),
        cursor,
        had_content,
    }
}

/// Debounced snapshot + cursor writer (cursor sampled BEFORE export, both
/// committed in one transaction).
pub(crate) struct Persister {
    doc: Weak<SessionDoc>,
    store: Arc<DocsStore>,
    chat_id: String,
    cursor: AtomicU64,
    generation: AtomicU64,
    saved: AtomicU64,
    urgent: AtomicBool,
    /// Bytes of the last snapshot written (paces the next save).
    last_len: AtomicUsize,
    write: Mutex<()>,
    wake: Arc<Notify>,
}

impl Persister {
    pub(crate) fn new(
        doc: &Arc<SessionDoc>,
        store: Arc<DocsStore>,
        chat_id: &str,
        cursor: u64,
    ) -> Arc<Self> {
        Arc::new(Self {
            doc: Arc::downgrade(doc),
            store,
            chat_id: chat_id.to_owned(),
            cursor: AtomicU64::new(cursor),
            generation: AtomicU64::new(0),
            saved: AtomicU64::new(0),
            urgent: AtomicBool::new(false),
            last_len: AtomicUsize::new(0),
            write: Mutex::new(()),
            wake: Arc::new(Notify::new()),
        })
    }

    pub(crate) fn start(self: &Arc<Self>, cancel: CancellationToken) {
        let weak = Arc::downgrade(self);
        let wake = self.wake.clone();
        crate::runtime::shared().spawn(async move {
            let mut last_save: Option<tokio::time::Instant> = None;
            loop {
                let notified = wake.notified();
                tokio::select! {
                    _ = cancel.cancelled() => {
                        if let Some(this) = weak.upgrade() {
                            let _ = tokio::task::spawn_blocking(move || this.flush()).await;
                        }
                        return;
                    }
                    _ = notified => {}
                }
                let Some(this) = weak.upgrade() else { return };
                let urgent = this.urgent.swap(false, Ordering::AcqRel);
                let pace = save_interval(this.last_len.load(Ordering::Acquire));
                drop(this);
                // A small chat saves an urgent change at once and others after
                // a second, as before; a large one waits out its pace either way.
                let wait = if urgent {
                    let since = last_save.map_or(pace, |at| at.elapsed());
                    pace.saturating_sub(SAVE_INTERVAL).saturating_sub(since)
                } else {
                    pace
                };
                if !wait.is_zero() {
                    tokio::select! {
                        _ = cancel.cancelled() => {
                            if let Some(this) = weak.upgrade() {
                                let _ = tokio::task::spawn_blocking(move || this.flush()).await;
                            }
                            return;
                        }
                        _ = tokio::time::sleep(wait) => {}
                    }
                }
                let Some(again) = weak.upgrade() else { return };
                let _ = tokio::task::spawn_blocking(move || again.flush()).await;
                last_save = Some(tokio::time::Instant::now());
            }
        });
    }

    pub(crate) fn cursor(&self) -> u64 {
        self.cursor.load(Ordering::Acquire)
    }

    pub(crate) fn applied(&self, cursor: u64, urgent: bool) {
        self.cursor.fetch_max(cursor, Ordering::AcqRel);
        self.dirty(urgent);
    }

    pub(crate) fn reset_cursor(&self, cursor: u64) {
        self.cursor.store(cursor, Ordering::Release);
        self.dirty(true);
    }

    pub(crate) fn dirty(&self, urgent: bool) {
        self.generation.fetch_add(1, Ordering::AcqRel);
        if urgent {
            self.urgent.store(true, Ordering::Release);
        }
        self.wake.notify_one();
    }

    /// Write now if anything changed since the last save (blocking I/O).
    pub(crate) fn flush(&self) {
        let _serial = lock(&self.write);
        let generation = self.generation.load(Ordering::Acquire);
        if generation == self.saved.load(Ordering::Acquire) {
            return;
        }
        let Some(doc) = self.doc.upgrade() else {
            return;
        };
        let cursor = self.cursor();
        let bytes = match doc.export_snapshot() {
            Ok(bytes) => {
                self.last_len.store(bytes.len(), Ordering::Release);
                bytes
            }
            Err(err) => {
                tracing::warn!(chat = %self.chat_id, error = %err, "chat snapshot export failed");
                return;
            }
        };
        match self.store.save_verified_snapshot_with_cursor(
            &self.chat_id,
            &bytes,
            cursor,
            CHAT2_DOC_EPOCH,
        ) {
            Ok(()) => self.saved.store(generation, Ordering::Release),
            Err(err) => {
                tracing::warn!(chat = %self.chat_id, error = %err, "chat snapshot save failed")
            }
        }
    }
}

// Full checkpoint merges expand historical operations. Room actors run on
// separate blocking workers; keep those transient allocations from overlapping.
static CHECKPOINT_IMPORT: Mutex<()> = Mutex::new(());

/// Returns whether the checkpoint needed a merge. Inspecting a modern snapshot
/// reads its version metadata without decoding every historical string. Loro's
/// normal import into a nonempty doc eagerly decodes that history even when all
/// operations are already present (the build 1027 OOM stack).
fn import_checkpoint(doc: &loro::LoroDoc, bytes: &[u8]) -> Result<bool, String> {
    let _serial = lock(&CHECKPOINT_IMPORT);
    let meta = loro::LoroDoc::decode_import_blob_meta(bytes, true).ok();
    if let Some(meta) = &meta {
        if !meta.partial_end_vv.is_empty() && doc.state_vv().includes_vv(&meta.partial_end_vv) {
            return Ok(false);
        }
    }
    let updates;
    let bytes = if meta.is_some_and(|m| m.mode == loro::EncodedBlobMode::Snapshot)
        && !doc.oplog_vv().is_empty()
    {
        // Loading a snapshot into an empty doc keeps old change blocks lazy.
        // Export only the missing span before merging into the live doc;
        // importing the entire snapshot directly calls visit_all_changes and
        // allocates every historical string, including already-known deletes.
        let snapshot = loro::LoroDoc::new();
        snapshot.import(bytes).map_err(|err| err.to_string())?;
        updates = snapshot
            .export(loro::ExportMode::updates(&doc.oplog_vv()))
            .map_err(|err| err.to_string())?;
        updates.as_slice()
    } else {
        bytes
    };
    let status = doc.import(bytes).map_err(|err| err.to_string())?;
    if status.pending.is_some() {
        return Err("checkpoint is missing causal dependencies".into());
    }
    Ok(true)
}

/// Remote bytes → the session doc; outbox hooks → the docs store.
pub(crate) struct ViewerSink {
    pub(crate) doc: Weak<SessionDoc>,
    pub(crate) store: Arc<DocsStore>,
    pub(crate) chat_id: String,
    pub(crate) persister: Arc<Persister>,
    /// Coalesced "the doc changed remotely" wake (republish).
    pub(crate) on_applied: Arc<dyn Fn() + Send + Sync>,
}

impl ViewerSink {
    fn import(&self, bytes: &[u8], cursor: u64, urgent: bool) -> RowImportOutcome {
        let Some(doc) = self.doc.upgrade() else {
            return RowImportOutcome::Applied;
        };
        match doc.doc().import(bytes) {
            Ok(status) if status.pending.is_some() => RowImportOutcome::PendingDependencies,
            Ok(_) => {
                self.persister.applied(cursor, urgent);
                (self.on_applied)();
                RowImportOutcome::Applied
            }
            Err(err) => {
                // A poison row must not wedge the room: skip it, keep going.
                tracing::warn!(chat = %self.chat_id, error = %err, "chat row import failed; skipping");
                self.persister.applied(cursor, urgent);
                RowImportOutcome::Applied
            }
        }
    }
}

impl ChatDocSink for ViewerSink {
    fn cursor_is_verified(&self) -> bool {
        self.store
            .snapshot_cursor_verified(&self.chat_id)
            .unwrap_or(false)
    }

    fn reset_cursor(&self, cursor: u64) {
        self.persister.reset_cursor(cursor);
    }

    fn pending_updates(&self) -> Result<Vec<(String, Vec<u8>)>, String> {
        let rejected: std::collections::HashSet<String> = self
            .store
            .rejected_chat_updates(&self.chat_id)
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        Ok(self
            .store
            .pending_chat_updates(&self.chat_id)
            .map_err(|e| e.to_string())?
            .into_iter()
            .filter(|(id, bytes)| {
                !rejected.contains(id)
                    && bytes.len() <= openagents_sync::chat_client::MAX_PUSH_BYTES
            })
            .collect())
    }

    fn persist_update(&self, batch_id: &str, bytes: &[u8]) -> Result<(), String> {
        self.store
            .enqueue_chat_update(&self.chat_id, batch_id, bytes)
            .map_err(|e| e.to_string())
    }

    fn acknowledge_update(&self, batch_id: &str) -> Result<(), String> {
        self.store
            .acknowledge_chat_update(&self.chat_id, batch_id)
            .map_err(|e| e.to_string())
    }

    fn reject_update(&self, batch_id: &str) -> Result<(), String> {
        self.store
            .reject_chat_update(&self.chat_id, batch_id)
            .map_err(|e| e.to_string())
    }

    fn apply_row(&self, bytes: &[u8], cursor: u64) -> RowImportOutcome {
        self.import(bytes, cursor, false)
    }

    fn apply_checkpoint(&self, bytes: &[u8], cursor: u64) -> Result<(), String> {
        let Some(doc) = self.doc.upgrade() else {
            return Ok(());
        };
        import_checkpoint(doc.doc(), bytes)?;
        self.persister.applied(cursor, true);
        (self.on_applied)();
        Ok(())
    }

    fn contains_frontier(&self, frontier: &[u8]) -> bool {
        let Some(doc) = self.doc.upgrade() else {
            return false;
        };
        // An undecodable or EMPTY frontier is "not contained": fetch the
        // checkpoint (an empty-VV shortcut once left readers blank).
        match VersionVector::decode(frontier) {
            Ok(vv) if !vv.is_empty() => doc.doc().oplog_vv().includes_vv(&vv),
            _ => false,
        }
    }

    fn advance_cursor(&self, cursor: u64) {
        self.persister.applied(cursor, true);
    }
}

/// Fresh `?token=` per dial.
pub(crate) struct ChatUrl {
    pub(crate) bearer: Bearer,
    pub(crate) edge: String,
    pub(crate) chat_id: String,
    pub(crate) device_id: String,
    pub(crate) host_device_id: String,
}

impl UrlProvider for ChatUrl {
    fn request(&self) -> BoxFuture<'static, Result<(String, Vec<(String, String)>), SyncError>> {
        let url = self.url();
        let bearer = self.bearer.clone();
        let host = self.host_device_id.clone();
        Box::pin(async move { bearer.proved_socket(&host, &url.await?).await })
    }

    fn url(&self) -> BoxFuture<'static, Result<String, SyncError>> {
        let bearer = self.bearer.clone();
        let host = self.host_device_id.clone();
        let (edge, chat, device) = (
            self.edge.clone(),
            self.chat_id.clone(),
            self.device_id.clone(),
        );
        Box::pin(async move {
            let token = bearer.get().await?;
            Ok(format!(
                "{}&hostDevice={}",
                urls::chat_ws(&edge, &chat, &token, &device),
                urls::encode(&host)
            ))
        })
    }
}

/// `GET /chat2/{id}/checkpoint` with Range resume (engine
/// `EdgeCheckpointFetcher`), plus the HTTPS rows pull/push transport.
pub(crate) struct ChatHttp {
    pub(crate) bearer: Bearer,
    pub(crate) edge: String,
    pub(crate) chat_id: String,
    pub(crate) device_id: String,
    pub(crate) host_device_id: String,
}

fn http_err(err: reqwest::Error) -> SyncError {
    SyncError::WebSocket(err.to_string())
}

impl CheckpointFetcher for ChatHttp {
    fn fetch(&self) -> BoxFuture<'static, Result<Vec<u8>, SyncError>> {
        let bearer = self.bearer.clone();
        let host = self.host_device_id.clone();
        let url = urls::chat_checkpoint(&self.edge, &self.chat_id);
        Box::pin(async move {
            let mut got: Vec<u8> = Vec::new();
            let mut seen_seq: Option<String> = None;
            let mut last_failure = None;
            for _attempt in 0..4 {
                let _permit = openagents_sync::budget::shared()
                    .http(openagents_sync::budget::Priority::Interactive)
                    .await?;
                let token = bearer.get().await?;
                let proofs = bearer.device_proofs(&host, &url, "GET", b"").await?;
                let mut request = crate::auth::http()
                    .get(&url)
                    .bearer_auth(&token)
                    .timeout(Duration::from_secs(300));
                for (key, value) in proofs {
                    request = request.header(key, value);
                }
                if !got.is_empty() {
                    request = request.header("range", format!("bytes={}-", got.len()));
                }
                let response = match request.send().await {
                    Ok(response) => response,
                    Err(err) => {
                        last_failure = Some(err.to_string());
                        continue;
                    }
                };
                let seq = response
                    .headers()
                    .get("x-chat2-checkpoint-seq")
                    .and_then(|v| v.to_str().ok())
                    .map(str::to_owned);
                if seq.is_some() && seen_seq.is_some() && seq != seen_seq {
                    // A new checkpoint committed between attempts: restart.
                    got.clear();
                    seen_seq = seq;
                    continue;
                }
                if seq.is_some() {
                    seen_seq = seq;
                }
                match response.status().as_u16() {
                    200 => got.clear(),
                    206 => {}
                    404 => return Err(SyncError::Protocol("no checkpoint".into())),
                    code => return Err(SyncError::Protocol(format!("checkpoint HTTP {code}"))),
                }
                let mut response = response;
                loop {
                    match response.chunk().await {
                        Ok(Some(chunk)) => got.extend_from_slice(&chunk),
                        Ok(None) => return Ok(got),
                        Err(err) => {
                            last_failure = Some(err.to_string());
                            break;
                        }
                    }
                }
            }
            Err(SyncError::Protocol(format!(
                "checkpoint fetch exhausted resume attempts: {}",
                last_failure.unwrap_or_default()
            )))
        })
    }
}

impl ChatTransport for ChatHttp {
    fn fetch_rows(&self, after: u64) -> BoxFuture<'static, Result<Vec<u8>, SyncError>> {
        let bearer = self.bearer.clone();
        let host = self.host_device_id.clone();
        let url = urls::chat_rows(&self.edge, &self.chat_id, after, &self.device_id);
        Box::pin(async move {
            let _permit = openagents_sync::budget::shared()
                .http(openagents_sync::budget::Priority::Interactive)
                .await?;
            let token = bearer.get().await?;
            let proofs = bearer.device_proofs(&host, &url, "GET", b"").await?;
            let mut request = crate::auth::http()
                .get(url)
                .bearer_auth(token)
                .timeout(Duration::from_secs(120));
            for (key, value) in proofs {
                request = request.header(key, value);
            }
            let response = request.send().await.map_err(http_err)?;
            if !response.status().is_success() {
                return Err(SyncError::Protocol(format!(
                    "chat pull http {}",
                    response.status()
                )));
            }
            Ok(response.bytes().await.map_err(http_err)?.to_vec())
        })
    }

    fn push(
        &self,
        batch_id: String,
        bytes: Vec<u8>,
    ) -> BoxFuture<'static, Result<String, SyncError>> {
        let bearer = self.bearer.clone();
        let host = self.host_device_id.clone();
        let url = format!(
            "{}&hostDevice={}",
            urls::chat_push(&self.edge, &self.chat_id, &batch_id, &self.device_id),
            urls::encode(&self.host_device_id)
        );
        Box::pin(async move {
            let _permit = openagents_sync::budget::shared()
                .http(openagents_sync::budget::Priority::Interactive)
                .await?;
            let token = bearer.get().await?;
            let proofs = bearer.device_proofs(&host, &url, "POST", &bytes).await?;
            let mut request = crate::auth::http().post(url).bearer_auth(token).body(bytes);
            for (key, value) in proofs {
                request = request.header(key, value);
            }
            let response = request.send().await.map_err(http_err)?;
            if !response.status().is_success() {
                return Err(SyncError::Protocol(format!(
                    "chat push http {}",
                    response.status()
                )));
            }
            response.text().await.map_err(http_err)
        })
    }
}

/// Room connection posture (feeds connectivity + the composer strip).
#[derive(Default)]
pub(crate) struct RoomStatus {
    pub connected: AtomicBool,
    /// A server state has been applied at least once (hydration).
    pub caught_up: AtomicBool,
    /// Next scheduled join attempt while the first join keeps failing.
    pub retry_at_ms: AtomicI64,
    pub last_failure: Mutex<Option<String>>,
}

/// One live chat room. Dropping it stops the client and flushes the doc.
pub(crate) struct Room {
    pub(crate) chat_id: String,
    slot: Arc<Mutex<Slot>>,
    pub(crate) status: Arc<RoomStatus>,
    pub(crate) persister: Arc<Persister>,
    cancel: CancellationToken,
    /// Live-poll cadence in milliseconds (0 = off); see [`Room::set_live_poll`].
    live_poll_ms: Arc<AtomicU64>,
    _local_updates: loro::Subscription,
}

#[derive(Default)]
struct Slot {
    client: Option<ChatClient>,
    /// Own updates committed before the client existed (also durable in the
    /// store's outbox; handed to the client on join — duplicates are
    /// idempotent: batch-id dedupe + Loro re-import no-op).
    unsent: Vec<(String, Vec<u8>)>,
}

pub(crate) struct RoomDeps {
    pub bearer: Bearer,
    pub edge: String,
    pub device_id: String,
    pub host_device_id: String,
    pub store: Arc<DocsStore>,
    /// Remote change landed (schedule a coalesced session refresh).
    pub on_applied: Arc<dyn Fn() + Send + Sync>,
    /// Connection posture changed (connectivity recompute).
    pub on_status: Arc<dyn Fn() + Send + Sync>,
    /// The client's lifetime: sign-out stops the room even while a view
    /// still holds its session.
    pub cancel: CancellationToken,
}

impl Room {
    /// Start publishing local writes and joining the room. `cursor` is the
    /// verified cursor from [`load_local`].
    pub(crate) fn start(
        doc: &Arc<SessionDoc>,
        chat_id: &str,
        cursor: u64,
        deps: RoomDeps,
    ) -> Arc<Self> {
        let persister = Persister::new(doc, deps.store.clone(), chat_id, cursor);
        let cancel = deps.cancel.child_token();
        persister.start(cancel.clone());
        let slot = Arc::new(Mutex::new(Slot::default()));

        // Own writes (commands, queue rows): durable outbox first, then the
        // live client (or the pre-join buffer).
        let publish_slot = slot.clone();
        let store = deps.store.clone();
        let publish_chat = chat_id.to_owned();
        let publish_persister = persister.clone();
        let local_updates = doc
            .doc()
            .subscribe_local_update(Box::new(move |bytes: &Vec<u8>| {
                let batch_id = crate::new_id();
                let mut slot = lock(&publish_slot);
                if let Err(err) = store.enqueue_chat_update(&publish_chat, &batch_id, bytes) {
                    tracing::warn!(chat = %publish_chat, error = %err, "outbox enqueue failed");
                }
                match &slot.client {
                    Some(client) => client.enqueue_batch(batch_id, bytes.clone()),
                    None => slot.unsent.push((batch_id, bytes.clone())),
                }
                publish_persister.dirty(false);
                true
            }));

        let room = Arc::new(Self {
            chat_id: chat_id.to_owned(),
            slot,
            status: Arc::new(RoomStatus::default()),
            persister,
            cancel,
            live_poll_ms: Arc::new(AtomicU64::new(0)),
            _local_updates: local_updates,
        });
        room.spawn_join(doc, deps);
        room.spawn_live_poll();
        room
    }

    /// Ask the room for rows on a cadence while someone needs this chat live
    /// (`None` stops it). Rooms on the sync server fan out only within one
    /// server instance (#12120), so a host connected to another instance
    /// never reaches this socket live: without a pull, its rows waited for
    /// our next push's ACK gap or the 15-minute quiet probe (#12132).
    pub(crate) fn set_live_poll(&self, every: Option<Duration>) {
        let ms = every.map_or(0, |d| d.as_millis().max(1) as u64);
        self.live_poll_ms.store(ms, Ordering::Release);
    }

    fn spawn_live_poll(self: &Arc<Self>) {
        let weak = Arc::downgrade(self);
        let cancel = self.cancel.clone();
        let every = self.live_poll_ms.clone();
        crate::runtime::shared().spawn(async move {
            // (ticket, sent at) of the last catch-up this task requested.
            let mut last: Option<(u64, tokio::time::Instant)> = None;
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => return,
                    _ = tokio::time::sleep(LIVE_POLL_TICK) => {}
                }
                let ms = every.load(Ordering::Acquire);
                if ms == 0 {
                    continue;
                }
                let Some(room) = weak.upgrade() else { return };
                let slot = lock(&room.slot);
                let Some(client) = slot.client.as_ref() else {
                    continue;
                };
                // A dropped socket has its own HTTP pull on every backoff.
                if !client.stats().connected {
                    continue;
                }
                let now = tokio::time::Instant::now();
                let due = match last {
                    None => true,
                    Some((ticket, at)) => {
                        let elapsed = now.duration_since(at);
                        elapsed >= Duration::from_millis(ms)
                            && (client.catch_up_completed(ticket) || elapsed >= LIVE_POLL_STALE)
                    }
                };
                if due {
                    last = Some((client.request_catch_up(), now));
                }
            }
        });
    }

    fn spawn_join(self: &Arc<Self>, doc: &Arc<SessionDoc>, deps: RoomDeps) {
        let sink = Arc::new(ViewerSink {
            doc: Arc::downgrade(doc),
            store: deps.store.clone(),
            chat_id: self.chat_id.clone(),
            persister: self.persister.clone(),
            on_applied: deps.on_applied.clone(),
        });
        let url: Arc<dyn UrlProvider> = Arc::new(ChatUrl {
            bearer: deps.bearer.clone(),
            edge: deps.edge.clone(),
            chat_id: self.chat_id.clone(),
            device_id: deps.device_id.clone(),
            host_device_id: deps.host_device_id.clone(),
        });
        let http = Arc::new(ChatHttp {
            bearer: deps.bearer.clone(),
            edge: deps.edge.clone(),
            chat_id: self.chat_id.clone(),
            device_id: deps.device_id.clone(),
            host_device_id: deps.host_device_id.clone(),
        });
        let weak = Arc::downgrade(self);
        let cancel = self.cancel.clone();
        let device_id = deps.device_id.clone();
        let on_status = deps.on_status.clone();
        let on_applied = deps.on_applied.clone();
        crate::runtime::shared().spawn(async move {
            let mut backoff = JOIN_RETRY_BASE;
            let client = loop {
                let Some(room) = weak.upgrade() else { return };
                let cursor = room.persister.cursor();
                let status = room.status.clone();
                drop(room);
                let attempt = ChatClient::connect_via_transport(
                    url.clone(),
                    sink.clone(),
                    http.clone(),
                    &device_id,
                    cursor,
                    http.clone(),
                );
                let result = tokio::select! {
                    _ = cancel.cancelled() => return,
                    result = tokio::time::timeout(Duration::from_secs(60), attempt) => result,
                };
                match result {
                    Ok(Ok(client)) => {
                        status.retry_at_ms.store(0, Ordering::Release);
                        break client;
                    }
                    Ok(Err(err)) => *lock(&status.last_failure) = Some(err.to_string()),
                    Err(_) => *lock(&status.last_failure) = Some("join timed out".into()),
                }
                status.retry_at_ms.store(
                    crate::now_ms() + backoff.as_millis() as i64,
                    Ordering::Release,
                );
                on_status();
                if !super::wait_backoff(&cancel, backoff).await {
                    return;
                }
                backoff = (backoff * 2).min(JOIN_RETRY_CAP);
            };
            let mut events = client.events();
            {
                let Some(room) = weak.upgrade() else { return };
                let mut slot = lock(&room.slot);
                for (batch_id, bytes) in std::mem::take(&mut slot.unsent) {
                    client.enqueue_batch(batch_id, bytes);
                }
                slot.client = Some(client);
            }
            on_status();
            loop {
                let event = tokio::select! {
                    _ = cancel.cancelled() => return,
                    event = events.recv() => event,
                };
                let Some(room) = weak.upgrade() else { return };
                match event {
                    Ok(ChatEvent::Connected) => {
                        room.status.connected.store(true, Ordering::Release);
                        *lock(&room.status.last_failure) = None;
                        on_status();
                    }
                    Ok(ChatEvent::CaughtUp { .. }) => {
                        room.status.connected.store(true, Ordering::Release);
                        room.status.caught_up.store(true, Ordering::Release);
                        on_applied();
                        on_status();
                    }
                    Ok(ChatEvent::Disconnected) => {
                        room.status.connected.store(false, Ordering::Release);
                        on_status();
                    }
                    Ok(ChatEvent::Applied) => on_applied(),
                    Ok(_) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => on_applied(),
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
                }
            }
        });
    }

    /// Liveness hint (view attached / foreground): probe or redial now.
    pub(crate) fn kick(&self) {
        if let Some(client) = &lock(&self.slot).client {
            if client.stats().connected {
                client.probe();
            } else {
                client.redial();
            }
        }
    }

    /// Fresh socket (retry delivery).
    pub(crate) fn redial(&self) {
        if let Some(client) = &lock(&self.slot).client {
            client.redial();
        }
    }

    pub(crate) fn connected(&self) -> bool {
        lock(&self.slot)
            .client
            .as_ref()
            .is_some_and(|c| c.stats().connected)
    }

    /// Has unacknowledged own writes (never evict / keep the room).
    pub(crate) fn has_pending_pushes(&self) -> bool {
        let slot = lock(&self.slot);
        !slot.unsent.is_empty()
            || slot
                .client
                .as_ref()
                .is_some_and(|c| c.stats().pending_pushes > 0)
    }

    pub(crate) fn retry_at_ms(&self) -> Option<i64> {
        let at = self.status.retry_at_ms.load(Ordering::Acquire);
        (at > 0).then_some(at)
    }

    pub(crate) fn flush(&self) {
        self.persister.flush();
    }
}

impl Drop for Room {
    fn drop(&mut self) {
        self.cancel.cancel();
        if let Some(client) = lock(&self.slot).client.take() {
            crate::runtime::shared().spawn(client.shutdown());
        }
        self.persister.flush();
    }
}

#[cfg(test)]
mod save_pace_tests {
    use super::*;

    #[test]
    fn a_small_chat_saves_each_second_and_a_huge_one_paces_its_rewrites() {
        assert_eq!(save_interval(0), SAVE_INTERVAL);
        assert_eq!(save_interval(512 * 1024), SAVE_INTERVAL);
        assert_eq!(save_interval(20 * 1024 * 1024), Duration::from_secs(10));
        // The owner's 90 MB chat (build 1017): at most one rewrite every 45 s.
        assert_eq!(save_interval(90 * 1024 * 1024), Duration::from_secs(45));
        assert_eq!(save_interval(usize::MAX), SAVE_INTERVAL_MAX);
    }

    #[test]
    fn repeated_checkpoint_skips_history_expansion_and_preserves_local_edits() {
        let host = loro::LoroDoc::new();
        host.set_peer_id(1).unwrap();
        // A short visible transcript with substantial deleted history: the
        // nonempty snapshot merge must not re-expand this on every reconnect.
        let text = host.get_text("transcript");
        text.insert(0, &"historical message\n".repeat(100_000))
            .unwrap();
        host.commit();
        text.delete(0, text.len_unicode()).unwrap();
        text.insert(0, "visible message").unwrap();
        host.commit();
        let bytes = host.export(loro::ExportMode::Snapshot).unwrap();
        let viewer = loro::LoroDoc::new();
        assert!(import_checkpoint(&viewer, &bytes).unwrap());
        viewer.set_peer_id(2).unwrap();
        viewer.get_map("local").insert("draft", "keep me").unwrap();
        viewer.commit();
        let before = viewer.oplog_vv();
        for _ in 0..3 {
            assert!(!import_checkpoint(&viewer, &bytes).unwrap());
        }
        assert_eq!(viewer.oplog_vv(), before);
        assert_eq!(viewer.get_text("transcript").to_string(), "visible message");
        assert!(viewer.get_map("local").get("draft").is_some());

        text.insert(text.len_unicode(), " newer").unwrap();
        host.commit();
        let newer = host.export(loro::ExportMode::Snapshot).unwrap();
        assert!(import_checkpoint(&viewer, &newer).unwrap());
        assert_eq!(
            viewer.get_text("transcript").to_string(),
            "visible message newer"
        );
        assert!(viewer.get_map("local").get("draft").is_some());
    }

    #[test]
    fn invalid_checkpoint_does_not_advance_persisted_cursor() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(DocsStore::open(dir.path()).unwrap());
        let doc = Arc::new(SessionDoc::from_doc(loro::LoroDoc::new()));
        let persister = Persister::new(&doc, store.clone(), "chat", 7);
        let sink = ViewerSink {
            doc: Arc::downgrade(&doc),
            store,
            chat_id: "chat".into(),
            persister: persister.clone(),
            on_applied: Arc::new(|| {}),
        };
        assert!(sink.apply_checkpoint(b"invalid", 20).is_err());
        assert_eq!(persister.cursor(), 7);
        assert_eq!(persister.generation.load(Ordering::Acquire), 0);

        let host = loro::LoroDoc::new();
        host.get_text("transcript").insert(0, "first").unwrap();
        host.commit();
        let base = host.oplog_vv();
        host.get_text("transcript").insert(5, " second").unwrap();
        host.commit();
        let missing_base = host.export(loro::ExportMode::updates(&base)).unwrap();
        assert!(sink.apply_checkpoint(&missing_base, 20).is_err());
        assert_eq!(persister.cursor(), 7);
        assert_eq!(persister.generation.load(Ordering::Acquire), 0);
        let complete = host.export(loro::ExportMode::Snapshot).unwrap();
        sink.apply_checkpoint(&complete, 20).unwrap();
        assert_eq!(persister.cursor(), 20);
        assert_eq!(doc.doc().get_text("transcript").to_string(), "first second");
    }
}
