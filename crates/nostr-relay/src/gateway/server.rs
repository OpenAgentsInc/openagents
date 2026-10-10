use std::{
    collections::{HashMap, HashSet, VecDeque},
    io,
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use tokio::{
    net::{TcpListener, TcpStream},
    sync::{Semaphore, mpsc, watch},
    task::{JoinHandle, JoinSet},
    time::timeout,
};
use tokio_tungstenite::tungstenite::{
    Message,
    error::Error as WebSocketError,
    protocol::{CloseFrame, frame::coding::CloseCode},
};

use crate::{
    domain::{
        AGENT_ENGRAM_KIND, AGENT_OBSERVER_KIND, AGENT_TURN_METRIC_KIND, AgentObserverDirection,
        DM_HIDE_KIND, DM_OPEN_KIND, DM_VISIBILITY_KIND, EVENT_REMINDER_KIND, Event, EventClass,
        Filter, IDENTITY_ARCHIVE_REQUEST_KIND, IDENTITY_UNARCHIVE_REQUEST_KIND, PUSH_LEASE_KIND,
        RELAY_ONLY_BLOCK_KINDS, WORKSPACE_PROFILE_KIND, agent_observer_route,
        agent_turn_metric_owner, dm_visibility_channel, parse_identity_archive_request,
        validate_block_ingest, verify_agent_auth_attestation, verify_owner_binding, workspace_icon,
    },
    store::{
        AdmissionOutcome, AdmissionRejection, NotificationListener, PushLeaseWrite, Store,
        StoreError, StoreNotification,
    },
};

use super::{
    GatewayConfig, GatewayError,
    auth::{AuthState, make_challenge, read_process_secret},
    db::{CatchUpResult, DbPool, DbProtocolConfig},
    management::{is_management_request, serve_management},
    media::{MediaStorage, STALE_RESERVATION_AGE, is_media_request, serve_media},
    push::{self, PushExecutor},
    query::{is_query_request, serve_query},
    rate::{ConnectionPermit, RateLimiter, WorldRefusal},
    socket::{
        ServerWebSocket, effective_ip, is_websocket_upgrade, read_http_head, serve_http,
        websocket_handshake,
    },
    subscription::{ConnectionId, HubHandle, PublishedEvent},
    wire::{
        self, ClientMessage, closed_message, count_message, notice_message, ok_message,
        parse_client_message,
    },
};

const MAX_PROCESS_CONNECTIONS: usize = 4_096;
const NOTIFICATION_QUEUE_CAPACITY: usize = 2_048;
const HUB_COMMAND_CAPACITY: usize = 2_048;
const MAX_DB_QUEUED_JOBS: usize = 256;
/// The most sequences one catch-up read covers.
const MAX_NOTIFICATION_GAP: usize = 4_096;
/// Backoff for replacing a lost notification listener and for retrying a
/// failed catch-up read.
const LISTENER_RETRY_MIN: Duration = Duration::from_millis(100);
const LISTENER_RETRY_MAX: Duration = Duration::from_secs(5);
const LISTENER_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Locally committed events waiting for the delivery task. When it is full
/// an event waits for its database notification instead.
const COMMITTED_CAPACITY: usize = 1_024;

pub const GIFT_WRAP_RECIPIENT_RATE_EXCEEDED: &str =
    "rate-limited: gift-wrap recipient rate exceeded";

pub struct Gateway {
    listener: TcpListener,
    local_addr: SocketAddr,
    state: Arc<ServerState>,
    shutdown: watch::Sender<bool>,
    shutdown_receiver: watch::Receiver<bool>,
    background: Vec<JoinHandle<()>>,
}

#[derive(Clone)]
pub struct ShutdownHandle {
    sender: watch::Sender<bool>,
}

struct ServerState {
    config: Arc<GatewayConfig>,
    db: DbPool,
    hub: HubHandle,
    rate: RateLimiter,
    challenge_secret: [u8; 32],
    next_connection_id: AtomicU64,
    policy: crate::store::RelayPolicy,
    current: Arc<AtomicBool>,
    shutdown: watch::Sender<bool>,
    media: Option<MediaStorage>,
    /// Shortens the push worker's wait after an admission.
    push_wake: Arc<tokio::sync::Notify>,
    /// Events this process committed, for delivery without a catch-up read.
    committed: mpsc::Sender<PublishedEvent>,
}

enum Wake {
    Notification(StoreNotification),
    ListenerLost,
    Reconnected(Result<(NotificationListener, i64), String>),
    Committed(PublishedEvent),
    Read(Result<CatchUpResult, StoreError>),
}

/// A replacement listener being connected, after its backoff delay, with
/// the latest sequence read on it once `LISTEN` took effect.
type PendingListener = std::pin::Pin<
    Box<dyn std::future::Future<Output = Result<(NotificationListener, i64), String>> + Send>,
>;

fn reconnect_listener(database_url: String, delay: Duration) -> PendingListener {
    Box::pin(async move {
        tokio::time::sleep(delay).await;
        let connect = async {
            let listener =
                NotificationListener::connect(&database_url, NOTIFICATION_QUEUE_CAPACITY).await?;
            let latest = listener.latest_ingest_seq().await?;
            Ok::<_, StoreError>((listener, latest))
        };
        match timeout(LISTENER_CONNECT_TIMEOUT, connect).await {
            Ok(Ok(connected)) => Ok(connected),
            Ok(Err(error)) => Err(error.to_string()),
            Err(_) => Err("connecting timed out".to_owned()),
        }
    })
}

fn next_read_retry(previous: Duration) -> Duration {
    if previous.is_zero() {
        LISTENER_RETRY_MIN
    } else {
        (previous * 2).min(LISTENER_RETRY_MAX)
    }
}

/// A catch-up read in flight, through `through`.
struct PendingCatchUp {
    through: i64,
    read: std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<CatchUpResult, StoreError>> + Send>,
    >,
}

/// Which stored events the process may deliver, so that every subscriber
/// sees them in `ingest_seq` order and none is skipped.
///
/// Durable admissions commit in sequence order, so once an admission
/// commits, every smaller sequence is already committed or will never be.
/// An event this process committed right after the last delivered one can
/// therefore go out from memory. Any other sequence a notification or a
/// commit names is read back from the database with the gap before it.
#[derive(Debug)]
struct DurableSequence {
    delivered: i64,
    wanted: i64,
}

impl DurableSequence {
    fn new(cursor: i64) -> Self {
        Self {
            delivered: cursor,
            wanted: cursor,
        }
    }

    /// Another admission, here or in another process, committed `ingest_seq`.
    fn notified(&mut self, ingest_seq: i64) {
        self.wanted = self.wanted.max(ingest_seq);
    }

    /// This process committed `ingest_seq`. True means deliver it now.
    fn committed(&mut self, ingest_seq: i64) -> bool {
        if self.delivered.checked_add(1) == Some(ingest_seq) {
            self.delivered = ingest_seq;
            self.wanted = self.wanted.max(ingest_seq);
            true
        } else {
            self.notified(ingest_seq);
            false
        }
    }

    /// The range to read next, after `.0` through `.1`, if any. A range
    /// holds at most [`MAX_NOTIFICATION_GAP`] sequences, so a long outage
    /// is read back in bounded steps.
    fn read_needed(&self) -> Option<(i64, i64)> {
        let step = i64::try_from(MAX_NOTIFICATION_GAP).unwrap_or(i64::MAX);
        (self.wanted > self.delivered).then(|| {
            (
                self.delivered,
                self.wanted.min(self.delivered.saturating_add(step)),
            )
        })
    }

    /// A read through `through`, whose database held sequences up to
    /// `latest`, returned `events` in sequence order. Returns those not yet
    /// delivered, and false when `through` names a sequence the database
    /// does not hold: the notifications that asked for it can't be trusted,
    /// so nothing past `latest` is wanted until a new listener says so.
    fn read(
        &mut self,
        through: i64,
        latest: i64,
        events: Vec<crate::store::StoredEvent>,
    ) -> (Vec<crate::store::StoredEvent>, bool) {
        let after = self.delivered;
        let covered = through.min(latest.max(after));
        self.delivered = self.delivered.max(covered);
        let consistent = latest >= through;
        if !consistent {
            self.wanted = self.delivered;
        }
        let events = events
            .into_iter()
            .filter(|stored| stored.ingest_seq > after && stored.ingest_seq <= covered)
            .collect();
        (events, consistent)
    }
}

struct ConnectionContext {
    connection_id: ConnectionId,
    ip: std::net::IpAddr,
    state: Arc<ServerState>,
    auth: Option<AuthState>,
    active_subscriptions: HashSet<String>,
    cancellations: HashMap<String, watch::Sender<bool>>,
    generation: u64,
    query_tasks: JoinSet<()>,
    /// NIP-77 sessions. The id namespace is not the `REQ` namespace.
    negentropy: HashMap<String, Vec<nostr::negentropy::Item>>,
}

impl Gateway {
    /// Validate configuration, migrate and verify Postgres, create fixed
    /// workers and notification state, and only then bind the network socket.
    pub async fn start(config: GatewayConfig) -> Result<Self, GatewayError> {
        config.validate()?;
        let media = match &config.media {
            Some(media) => Some(MediaStorage::prepare(media).await?),
            None => None,
        };
        let challenge_secret = read_process_secret()?;
        let (mut migration_store, _) = Store::connect_with_report(&config.database_url).await?;
        if config.import_nostr_effect {
            let mut total = crate::store::LegacyImportReport::default();
            loop {
                let report = migration_store
                    .import_nostr_effect_events(unix_now(), config.relay_signer.as_ref())
                    .await?;
                let done = report.is_empty();
                total.merge(&report);
                if done {
                    break;
                }
            }
            let retry = migration_store
                .retry_rejected_nostr_effect_events(unix_now(), config.relay_signer.as_ref())
                .await?;
            total.merge(&retry);
            print_legacy_import_report("startup", &total);
        }
        let policy = migration_store.relay_policy().await?;
        let notifications =
            NotificationListener::connect(&config.database_url, NOTIFICATION_QUEUE_CAPACITY)
                .await?;
        // LISTEN is current before the cursor is sampled. Notifications at or
        // below this boundary can be ignored because no client socket is bound
        // yet; later jumps are caught up through the durable sequence.
        let initial_ingest_seq = migration_store.latest_ingest_seq().await?;
        drop(migration_store);
        let (shutdown, shutdown_receiver) = watch::channel(false);
        let current = Arc::new(AtomicBool::new(true));
        let queue_capacity = MAX_DB_QUEUED_JOBS.div_ceil(config.db_connections);
        let (db, mut background) = DbPool::start(
            &config.database_url,
            config.db_connections,
            queue_capacity,
            shutdown.clone(),
            shutdown_receiver.clone(),
            Arc::clone(&current),
            DbProtocolConfig {
                relay_signer: config.relay_signer.clone(),
            },
        )
        .await?;

        let expiration_store = Store::connect_verified(&config.database_url).await?;
        let expiration_shutdown = shutdown.clone();
        let expiration_current = Arc::clone(&current);
        let mut expiration_stop = shutdown_receiver.clone();
        let expiration_interval = config.expiration_sweep;
        let expiration_media = media.clone();
        background.push(tokio::spawn(async move {
            let mut interval = tokio::time::interval(expiration_interval);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tokio::select! {
                    changed = expiration_stop.changed() => {
                        if changed.is_err() || *expiration_stop.borrow() {
                            break;
                        }
                    }
                    _ = interval.tick() => {
                        let now = unix_now();
                        if expiration_store.delete_expired(now).await.is_err()
                            || !expiration_store.is_current()
                        {
                            fail_process(&expiration_current, &expiration_shutdown, "the expiration sweep failed");
                            break;
                        }
                        if let Some(storage) = &expiration_media {
                            let cutoff = now.saturating_sub(STALE_RESERVATION_AGE.as_secs());
                            match expiration_store.release_stale_media_reservations(cutoff).await {
                                Ok(records) => {
                                    for record in &records {
                                        let _ = storage.remove_unpublished_blob(record).await;
                                    }
                                }
                                Err(_) => {
                                    fail_process(&expiration_current, &expiration_shutdown, "releasing stale media reservations failed");
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }));
        if config.import_nostr_effect {
            let mut import_store = Store::connect_verified(&config.database_url).await?;
            let import_signer = config.relay_signer.clone();
            let import_shutdown = shutdown.clone();
            let import_current = Arc::clone(&current);
            let mut import_stop = shutdown_receiver.clone();
            let import_interval = config.legacy_import_sweep;
            background.push(tokio::spawn(async move {
                let mut interval = tokio::time::interval(import_interval);
                interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                // Startup already drained the source; do not immediately run
                // a redundant first interval tick.
                interval.tick().await;
                loop {
                    tokio::select! {
                        changed = import_stop.changed() => {
                            if changed.is_err() || *import_stop.borrow() {
                                break;
                            }
                        }
                        _ = interval.tick() => {
                            match import_store
                                .import_nostr_effect_events(unix_now(), import_signer.as_ref())
                                .await
                            {
                                Ok(report) => {
                                    if !report.is_empty() {
                                        print_legacy_import_report("tail", &report);
                                    }
                                }
                                Err(_) => {
                                    fail_process(&import_current, &import_shutdown, "the nostr-effect import sweep failed");
                                    break;
                                }
                            }
                        }
                    }
                }
            }));
        }
        let (hub, hub_task) = HubHandle::start(
            HUB_COMMAND_CAPACITY,
            (config.limits.send_queue_capacity.saturating_sub(1) / 2).max(1),
            config.limits.max_frame_bytes,
            shutdown_receiver.clone(),
        );
        background.push(hub_task);

        let notify_db = db.clone();
        let notify_hub = hub.clone();
        let notify_shutdown = shutdown.clone();
        let notify_current = Arc::clone(&current);
        let mut notify_stop = shutdown_receiver.clone();
        let notify_database_url = config.database_url.clone();
        let (committed, mut committed_receiver) =
            mpsc::channel::<PublishedEvent>(COMMITTED_CAPACITY);
        background.push(tokio::spawn(async move {
            let mut sequence = DurableSequence::new(initial_ingest_seq);
            let mut reading: Option<PendingCatchUp> = None;
            let mut read_retry = Duration::ZERO;
            // The listener, or the reconnection that will replace it. A lost
            // listener never stops the relay: local commits keep going out
            // from memory, and the replacement's first read catches up by
            // sequence on everything committed meanwhile (#9947).
            let mut listener = Some(notifications);
            let mut reconnecting: Option<PendingListener> = None;
            let mut listener_retry = LISTENER_RETRY_MIN;
            loop {
                if reading.is_none()
                    && let Some((after, through)) = sequence.read_needed()
                {
                    let db = notify_db.clone();
                    let delay = read_retry;
                    reading = Some(PendingCatchUp {
                        through,
                        read: Box::pin(async move {
                            if !delay.is_zero() {
                                tokio::time::sleep(delay).await;
                            }
                            db.catch_up(after, through, unix_now(), MAX_NOTIFICATION_GAP + 1)
                                .await
                        }),
                    });
                }
                let wake = tokio::select! {
                    changed = notify_stop.changed() => {
                        if changed.is_err() || *notify_stop.borrow() {
                            break;
                        }
                        continue;
                    }
                    notification = async {
                        match listener.as_mut() {
                            Some(listener) => listener.recv_notification().await,
                            None => std::future::pending().await,
                        }
                    }, if listener.is_some() => match notification {
                        Some(notification) => Wake::Notification(notification),
                        None => Wake::ListenerLost,
                    },
                    connected = async {
                        match reconnecting.as_mut() {
                            Some(pending) => pending.as_mut().await,
                            None => std::future::pending().await,
                        }
                    }, if reconnecting.is_some() => Wake::Reconnected(connected),
                    Some(local) = committed_receiver.recv() => Wake::Committed(local),
                    result = async {
                        match reading.as_mut() {
                            Some(pending) => pending.read.as_mut().await,
                            None => std::future::pending().await,
                        }
                    }, if reading.is_some() => Wake::Read(result),
                };
                let now = unix_now();
                let mut publish = Vec::new();
                match wake {
                    Wake::Notification(StoreNotification::Stored(ingest_seq)) => {
                        sequence.notified(ingest_seq);
                    }
                    Wake::Notification(StoreNotification::Ephemeral(event)) => {
                        publish.push(PublishedEvent {
                            event: Arc::new(event),
                            ingest_seq: None,
                        });
                    }
                    Wake::ListenerLost => {
                        let reason = listener
                            .take()
                            .and_then(|lost| lost.fault())
                            .unwrap_or("the listener stopped");
                        log_warning(
                            "the relay lost its Postgres notification listener; reconnecting",
                            reason,
                        );
                        reconnecting = Some(reconnect_listener(
                            notify_database_url.clone(),
                            listener_retry,
                        ));
                    }
                    Wake::Reconnected(Ok((restored, latest))) => {
                        reconnecting = None;
                        listener_retry = LISTENER_RETRY_MIN;
                        // Everything committed before LISTEN took effect is
                        // at or below `latest`; the read it starts delivers
                        // what was missed, once, in order.
                        sequence.notified(latest);
                        listener = Some(restored);
                        log_info("the relay's Postgres notification listener is back", latest);
                    }
                    Wake::Reconnected(Err(reason)) => {
                        listener_retry = (listener_retry * 2).min(LISTENER_RETRY_MAX);
                        log_warning(
                            "the relay could not reconnect its Postgres notification listener; retrying",
                            &reason,
                        );
                        reconnecting = Some(reconnect_listener(
                            notify_database_url.clone(),
                            listener_retry,
                        ));
                    }
                    Wake::Committed(local) => {
                        if let Some(ingest_seq) = local.ingest_seq
                            && sequence.committed(ingest_seq)
                        {
                            publish.push(local);
                            // The pending read covered nothing new.
                            if reading
                                .as_ref()
                                .is_some_and(|pending| pending.through <= ingest_seq)
                            {
                                reading = None;
                            }
                        }
                    }
                    Wake::Read(result) => {
                        let through = reading.take().map_or(0, |pending| pending.through);
                        match result {
                            Ok(catch_up) if catch_up.events.len() <= MAX_NOTIFICATION_GAP => {
                                read_retry = Duration::ZERO;
                                let (events, consistent) =
                                    sequence.read(through, catch_up.latest, catch_up.events);
                                publish.extend(events.into_iter().map(|stored| PublishedEvent {
                                    event: Arc::new(stored.event),
                                    ingest_seq: Some(stored.ingest_seq),
                                }));
                                if !consistent && listener.is_some() {
                                    // A notification named a sequence the
                                    // database does not hold. Trust nothing
                                    // it sent: replace it and resynchronize.
                                    listener = None;
                                    log_warning(
                                        "the relay's Postgres notification listener named a sequence the database does not hold; reconnecting",
                                        &format!("read through {through}, latest {}", catch_up.latest),
                                    );
                                    reconnecting = Some(reconnect_listener(
                                        notify_database_url.clone(),
                                        LISTENER_RETRY_MIN,
                                    ));
                                }
                            }
                            Ok(catch_up) => {
                                read_retry = next_read_retry(read_retry);
                                log_warning(
                                    "a notification catch-up read returned more rows than its range; retrying",
                                    &catch_up.events.len().to_string(),
                                );
                            }
                            Err(error) => {
                                read_retry = next_read_retry(read_retry);
                                log_warning(
                                    "a notification catch-up read failed; retrying",
                                    &error.to_string(),
                                );
                            }
                        }
                    }
                }
                let mut failed = false;
                for published in publish {
                    if notify_hub.publish(published, now).await.is_err() {
                        failed = true;
                        break;
                    }
                }
                if failed {
                    fail_process(
                        &notify_current,
                        &notify_shutdown,
                        "the subscription hub stopped",
                    );
                    break;
                }
            }
        }));

        let push_wake = Arc::new(tokio::sync::Notify::new());
        if let Some(executor) = config.push.clone() {
            background.push(
                spawn_push_worker(
                    &config.database_url,
                    executor,
                    Arc::clone(&push_wake),
                    shutdown.clone(),
                    shutdown_receiver.clone(),
                    Arc::clone(&current),
                )
                .await?,
            );
        }

        let listener = match TcpListener::bind(config.bind_addr).await {
            Ok(listener) => listener,
            Err(error) => {
                let _ = shutdown.send(true);
                return Err(error.into());
            }
        };
        let local_addr = listener.local_addr()?;
        let state = Arc::new(ServerState {
            rate: RateLimiter::new(config.limits.clone()),
            config: Arc::new(config),
            db,
            hub,
            challenge_secret,
            next_connection_id: AtomicU64::new(1),
            policy,
            current,
            shutdown: shutdown.clone(),
            media,
            push_wake,
            committed,
        });
        Ok(Self {
            listener,
            local_addr,
            state,
            shutdown,
            shutdown_receiver,
            background,
        })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    pub fn shutdown_handle(&self) -> ShutdownHandle {
        ShutdownHandle {
            sender: self.shutdown.clone(),
        }
    }

    pub async fn run(mut self) -> Result<(), GatewayError> {
        let connection_slots = Arc::new(Semaphore::new(MAX_PROCESS_CONNECTIONS));
        let mut connections = JoinSet::new();
        loop {
            tokio::select! {
                changed = self.shutdown_receiver.changed() => {
                    if changed.is_err() || *self.shutdown_receiver.borrow() {
                        break;
                    }
                }
                accepted = self.listener.accept() => {
                    let (stream, peer) = match accepted {
                        Ok(accepted) => accepted,
                        Err(_) => {
                            fail_process(&self.state.current, &self.shutdown, "accepting a connection failed");
                            break;
                        }
                    };
                    let Ok(slot) = Arc::clone(&connection_slots).try_acquire_owned() else {
                        drop(stream);
                        continue;
                    };
                    // Frames are small and written one at a time; do not let
                    // Nagle's algorithm hold one back for an earlier ACK.
                    let _ = stream.set_nodelay(true);
                    let state = Arc::clone(&self.state);
                    connections.spawn(async move {
                        let _slot = slot;
                        let _ = handle_socket(stream, peer, state).await;
                    });
                }
                completed = connections.join_next(), if !connections.is_empty() => {
                    let _ = completed;
                }
            }
        }

        let failed = !self.state.current.load(Ordering::Acquire);
        self.state.current.store(false, Ordering::Release);
        let _ = self.shutdown.send(true);
        let grace = self.state.config.shutdown_grace;
        if timeout(grace, async {
            while connections.join_next().await.is_some() {}
        })
        .await
        .is_err()
        {
            connections.abort_all();
            while connections.join_next().await.is_some() {}
        }
        for mut task in self.background.drain(..) {
            if timeout(grace, &mut task).await.is_err() {
                task.abort();
                let _ = task.await;
            }
        }
        if failed {
            Err(GatewayError::Internal(
                "a database worker or background task failed; the line before this one says which"
                    .to_owned(),
            ))
        } else {
            Ok(())
        }
    }
}

fn print_legacy_import_report(phase: &str, report: &crate::store::LegacyImportReport) {
    println!(
        "{}",
        serde_json::json!({
            "level": "info",
            "message": "nostr-effect import sweep",
            "phase": phase,
            "scanned": report.scanned,
            "stored": report.stored,
            "duplicate": report.duplicate,
            "ephemeral": report.ephemeral,
            "expired": report.expired,
            "rejected": report.rejected,
            "rejection_reasons": report.rejection_reasons,
        })
    );
}

/// Logs an admitted ephemeral event at `debug` level: the kind, the ID,
/// the author, the `e` and `p` tags, and the content's length. The
/// content itself is never logged; NIP-CJ carries it encrypted.
fn print_ephemeral_admitted(event: &Event) {
    let tagged = |name: &str| -> Vec<&str> {
        event
            .tags
            .iter()
            .filter(|tag| tag.name() == Some(name))
            .filter_map(|tag| tag.value())
            .collect()
    };
    println!(
        "{}",
        serde_json::json!({
            "level": "debug",
            "message": "ephemeral event admitted",
            "kind": event.kind,
            "id": event.id,
            "pubkey": event.pubkey,
            "e": tagged("e"),
            "p": tagged("p"),
            "content_bytes": event.content.len(),
        })
    );
}

impl ShutdownHandle {
    pub fn shutdown(&self) {
        let _ = self.sender.send(true);
    }
}

async fn handle_socket(
    mut stream: TcpStream,
    peer: SocketAddr,
    state: Arc<ServerState>,
) -> Result<(), GatewayError> {
    let (request_bytes, head) = read_http_head(&mut stream).await?;
    let ip = effective_ip(
        &head,
        peer.ip(),
        state.config.trust_proxy,
        state.config.trusted_proxy_hops,
    );
    let Some(connection_permit) = state.rate.connect(ip) else {
        return Ok(());
    };
    if !is_websocket_upgrade(&head) {
        let _connection_permit = connection_permit;
        if is_media_request(&head)
            && let Some(media) = &state.media
        {
            return serve_media(
                stream,
                &head,
                &state.config,
                media,
                &state.db,
                &state.rate,
                ip,
            )
            .await;
        }
        if is_management_request(&head) {
            return serve_management(stream, &head, &state.config, &state.db).await;
        }
        if is_query_request(&head) {
            return serve_query(stream, &head, &state.config, &state.db).await;
        }
        let icon = state.db.workspace_icon().await?;
        let nip11 = wire::nip11_json_for_host(
            &state.config,
            &state.policy,
            icon.as_deref(),
            head.header("host"),
        );
        return serve_http(stream, &head, &nip11, state.current.load(Ordering::Acquire)).await;
    }
    let websocket =
        websocket_handshake(stream, request_bytes, state.config.limits.max_frame_bytes).await?;
    handle_websocket(websocket, ip, connection_permit, state).await
}

async fn handle_websocket(
    mut websocket: ServerWebSocket,
    ip: std::net::IpAddr,
    _connection_permit: ConnectionPermit,
    state: Arc<ServerState>,
) -> Result<(), GatewayError> {
    let connection_id = state.next_connection_id.fetch_add(1, Ordering::Relaxed);
    let channels = state
        .hub
        .add_connection(connection_id, state.config.limits.send_queue_capacity)
        .await?;
    let mut outbound = channels.outbound;
    let mut close = channels.close;
    let mut shutdown = state.shutdown.subscribe();
    let auth = state.config.relay_url.as_ref().map(|relay_url| {
        AuthState::new(
            make_challenge(&state.challenge_secret, connection_id, ip),
            relay_url.clone(),
        )
    });
    let mut context = ConnectionContext {
        connection_id,
        ip,
        state: Arc::clone(&state),
        auth,
        active_subscriptions: HashSet::new(),
        cancellations: HashMap::new(),
        generation: 0,
        query_tasks: JoinSet::new(),
        negentropy: HashMap::new(),
    };
    let mut pending = VecDeque::new();
    if let Some(auth) = &context.auth {
        pending.push_back(wire::auth_message(auth.challenge()));
    }
    let mut write_pending = false;

    'connection: loop {
        while context.query_tasks.try_join_next().is_some() {}
        let mut progressed = false;
        if let Some(message) = pending.pop_front() {
            queue_websocket_text(&mut websocket, message)?;
            write_pending = true;
            progressed = true;
        } else if let Ok(message) = outbound.try_recv() {
            queue_websocket_text(&mut websocket, message)?;
            write_pending = true;
            progressed = true;
        }
        if write_pending {
            match websocket.flush() {
                Ok(()) => write_pending = false,
                Err(WebSocketError::Io(error)) if error.kind() == io::ErrorKind::WouldBlock => {}
                Err(WebSocketError::ConnectionClosed | WebSocketError::AlreadyClosed) => {
                    break 'connection;
                }
                Err(error) => return Err(error.into()),
            }
        }

        match websocket.read() {
            Ok(Message::Text(text)) => {
                progressed = true;
                handle_client_text(&mut context, text.as_str(), &mut pending).await?;
            }
            Ok(Message::Binary(_)) => {
                progressed = true;
                pending.push_back(notice_message("invalid: binary messages are not supported"));
            }
            Ok(Message::Ping(_) | Message::Pong(_)) => {
                progressed = true;
                write_pending = true;
            }
            Ok(Message::Close(_)) => {
                break 'connection;
            }
            Ok(Message::Frame(_)) => {}
            Err(WebSocketError::Io(error)) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(WebSocketError::ConnectionClosed | WebSocketError::AlreadyClosed) => {
                break 'connection;
            }
            Err(WebSocketError::Capacity(_)) => {
                let _ = websocket.close(Some(CloseFrame {
                    code: CloseCode::Size,
                    reason: "message too large".into(),
                }));
                break 'connection;
            }
            Err(_) => break 'connection,
        }
        if progressed {
            continue;
        }

        enum Wake {
            Socket,
            Outbound(Option<String>),
            Stop,
        }
        let wake = {
            let socket = websocket.get_ref().stream();
            tokio::select! {
                _ = socket.readable() => Wake::Socket,
                _ = socket.writable(), if write_pending => Wake::Socket,
                message = outbound.recv() => Wake::Outbound(message),
                changed = close.changed() => {
                    let _ = changed;
                    Wake::Stop
                }
                changed = shutdown.changed() => {
                    let _ = changed;
                    Wake::Stop
                }
            }
        };
        match wake {
            Wake::Socket => {}
            Wake::Outbound(Some(message)) => pending.push_back(message),
            Wake::Outbound(None) | Wake::Stop => break 'connection,
        }
    }

    for cancellation in context.cancellations.values() {
        let _ = cancellation.send(true);
    }
    context.query_tasks.abort_all();
    while context.query_tasks.join_next().await.is_some() {}
    context
        .state
        .hub
        .remove_connection(context.connection_id)
        .await;
    let _ = websocket.close(Some(CloseFrame {
        code: CloseCode::Away,
        reason: "connection closing".into(),
    }));
    let _ = websocket.flush();
    Ok(())
}

async fn handle_client_text(
    context: &mut ConnectionContext,
    text: &str,
    pending: &mut VecDeque<String>,
) -> Result<(), GatewayError> {
    let message = match parse_client_message(text) {
        Ok(message) => message,
        Err(error) => {
            if let Some(event_id) = error.event_id {
                pending.push_back(ok_message(
                    &event_id,
                    false,
                    &format!("invalid: {}", error.reason),
                ));
            } else if let Some(subscription_id) = error.subscription_id {
                pending.push_back(closed_message(
                    &subscription_id,
                    &format!("invalid: {}", error.reason),
                ));
            } else {
                pending.push_back(notice_message(&format!("invalid: {}", error.reason)));
            }
            return Ok(());
        }
    };
    match message {
        ClientMessage::Auth(event) => handle_auth(context, event, pending).await,
        ClientMessage::Event(event) => handle_event(context, event, pending).await,
        ClientMessage::Req {
            subscription_id,
            filters,
        } => handle_req(context, subscription_id, filters, pending).await,
        ClientMessage::Count { query_id, filters } => {
            handle_count(context, query_id, filters, pending).await
        }
        ClientMessage::Close { subscription_id } => {
            if let Some(cancellation) = context.cancellations.remove(&subscription_id) {
                let _ = cancellation.send(true);
            }
            context.active_subscriptions.remove(&subscription_id);
            context
                .state
                .hub
                .remove(context.connection_id, subscription_id)
                .await;
            Ok(())
        }
        ClientMessage::NegOpen {
            subscription_id,
            filter,
            message,
        } => handle_neg_open(context, subscription_id, filter, message, pending).await,
        ClientMessage::NegMsg {
            subscription_id,
            message,
        } => {
            handle_neg_msg(context, subscription_id, message, pending);
            Ok(())
        }
        ClientMessage::NegClose { subscription_id } => {
            context.negentropy.remove(&subscription_id);
            Ok(())
        }
    }
}

const SYNC_LIMIT: usize = 4_096;

async fn handle_neg_open(
    context: &mut ConnectionContext,
    subscription_id: String,
    filter: Filter,
    message: Vec<u8>,
    pending: &mut VecDeque<String>,
) -> Result<(), GatewayError> {
    if let Err(reason) = admit_read(context) {
        pending.push_back(wire::neg_err(&subscription_id, reason));
        return Ok(());
    }
    if !context.negentropy.contains_key(&subscription_id)
        && context.negentropy.len() >= context.state.config.limits.max_subscriptions
    {
        pending.push_back(wire::neg_err(
            &subscription_id,
            "closed: too many sync subscriptions",
        ));
        return Ok(());
    }
    let filters = match validate_and_clamp_filters(vec![filter], &context.state.config) {
        Ok(filters) => filters,
        Err(reason) => {
            pending.push_back(wire::neg_err(&subscription_id, &reason));
            return Ok(());
        }
    };
    let read_pubkeys = context
        .auth
        .as_ref()
        .map(AuthState::authenticated_pubkeys)
        .unwrap_or_default();
    let (_cancel, cancel) = watch::channel(false);
    let stored = match context
        .state
        .db
        .history(filters, unix_now(), SYNC_LIMIT + 1, cancel, read_pubkeys)
        .await
    {
        Ok(stored) => stored,
        Err(error) => {
            pending.push_back(wire::neg_err(&subscription_id, &format!("error: {error}")));
            return Ok(());
        }
    };
    if stored.events.len() > SYNC_LIMIT {
        pending.push_back(wire::neg_err(
            &subscription_id,
            &format!("blocked: this query is too big: {SYNC_LIMIT}"),
        ));
        return Ok(());
    }
    if !stored.complete {
        // Reconciling a truncated set would tell the client it holds
        // everything the relay has; refuse instead.
        pending.push_back(wire::neg_err(&subscription_id, "blocked: incomplete"));
        return Ok(());
    }
    let mut items = Vec::with_capacity(stored.events.len());
    for stored in stored.events {
        let Some(id) = decode_id(&stored.event.id) else {
            continue;
        };
        items.push(nostr::negentropy::Item {
            timestamp: stored.event.created_at,
            id,
        });
    }
    nostr::negentropy::prepare(&mut items);
    context
        .negentropy
        .insert(subscription_id.clone(), items.clone());
    reply_sync(&subscription_id, &items, &message, pending);
    Ok(())
}

fn handle_neg_msg(
    context: &mut ConnectionContext,
    subscription_id: String,
    message: Vec<u8>,
    pending: &mut VecDeque<String>,
) {
    let Some(items) = context.negentropy.get(&subscription_id).cloned() else {
        pending.push_back(wire::neg_err(
            &subscription_id,
            "closed: sync subscription is not open",
        ));
        return;
    };
    reply_sync(&subscription_id, &items, &message, pending);
}

fn reply_sync(
    subscription_id: &str,
    items: &[nostr::negentropy::Item],
    message: &[u8],
    pending: &mut VecDeque<String>,
) {
    match nostr::negentropy::respond(items, message) {
        Ok(frame) => pending.push_back(wire::neg_msg(subscription_id, &frame)),
        Err(nostr::negentropy::SyncError::UnsupportedVersion { supported }) => {
            pending.push_back(wire::neg_msg(subscription_id, &[supported]));
        }
        Err(nostr::negentropy::SyncError::Malformed) => {
            pending.push_back(wire::neg_err(
                subscription_id,
                "invalid: the sync frame is malformed",
            ));
        }
    }
}

fn decode_id(text: &str) -> Option<[u8; 32]> {
    if text.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    let bytes = text.as_bytes();
    for index in 0..32 {
        let high = hex_nibble(bytes[index * 2])?;
        let low = hex_nibble(bytes[index * 2 + 1])?;
        out[index] = (high << 4) | low;
    }
    Some(out)
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

async fn handle_auth(
    context: &mut ConnectionContext,
    event: Event,
    pending: &mut VecDeque<String>,
) -> Result<(), GatewayError> {
    if let Err(error) = event.validate_structure() {
        pending.push_back(ok_message(
            &bounded(&event.id, 64),
            false,
            &format!("invalid: {error}"),
        ));
        return Ok(());
    }
    if !context.state.rate.event_from_ip(context.ip)
        || !context.state.rate.event_from_pubkey(&event.pubkey)
    {
        pending.push_back(ok_message(
            &event.id,
            false,
            "rate-limited: authentication event rate exceeded",
        ));
        return Ok(());
    }
    let Some(auth) = &context.auth else {
        pending.push_back(ok_message(
            &event.id,
            false,
            "restricted: NIP-42 is not configured on this relay",
        ));
        return Ok(());
    };
    if let Err(reason) = auth.verify(&event, unix_now()) {
        pending.push_back(ok_message(&event.id, false, &reason));
        return Ok(());
    }

    let identity = context
        .state
        .db
        .identity_status(event.pubkey.clone())
        .await?;
    let virtual_owner = if identity.closed_membership && !identity.direct_member {
        let attestation = match verify_agent_auth_attestation(&event) {
            Ok(Some(attestation)) => attestation,
            Ok(None) => {
                pending.push_back(ok_message(
                    &event.id,
                    false,
                    "restricted: agent authentication requires an owner attestation",
                ));
                return Ok(());
            }
            Err(reason) => {
                pending.push_back(ok_message(
                    &event.id,
                    false,
                    &format!("restricted: {reason}"),
                ));
                return Ok(());
            }
        };
        if !context
            .state
            .db
            .materialize_agent_owner(event.pubkey.clone(), attestation.owner_pubkey.clone(), true)
            .await?
        {
            pending.push_back(ok_message(
                &event.id,
                false,
                "restricted: owner is not an active member or conflicts with the agent's main owner",
            ));
            return Ok(());
        }
        Some(attestation.owner_pubkey)
    } else {
        // Direct members and open relays use ordinary NIP-42 authentication.
        // A valid optional NIP-OA tag still mints the main owner relation; a
        // malformed optional tag cannot invalidate direct authentication.
        if let Ok(Some(attestation)) = verify_agent_auth_attestation(&event) {
            context
                .state
                .db
                .materialize_agent_owner(event.pubkey.clone(), attestation.owner_pubkey, false)
                .await?;
        }
        None
    };

    let auth = context
        .auth
        .as_mut()
        .expect("authentication state was checked above");
    if let Some(owner) = virtual_owner {
        auth.accept_virtual(event.pubkey.clone(), owner);
    } else {
        auth.accept_direct(event.pubkey.clone());
    }
    pending.push_back(ok_message(&event.id, true, ""));
    Ok(())
}

async fn handle_event(
    context: &mut ConnectionContext,
    event: Event,
    pending: &mut VecDeque<String>,
) -> Result<(), GatewayError> {
    if let Err(error) = event.validate_structure() {
        pending.push_back(ok_message(
            &bounded(&event.id, 64),
            false,
            &format!("invalid: {error}"),
        ));
        return Ok(());
    }
    if let Err(rejection) = event_preflight(&context.state.rate, context.ip, &event) {
        let reason = match rejection {
            EventPreflightRejection::IpRate => "rate-limited: event rate exceeded".to_owned(),
            EventPreflightRejection::InvalidCrypto(error) => format!("invalid: {error}"),
        };
        pending.push_back(ok_message(&event.id, false, &reason));
        return Ok(());
    }
    if context.state.config.auth_required
        && !context
            .auth
            .as_ref()
            .is_some_and(AuthState::is_authenticated)
    {
        pending.push_back(ok_message(
            &event.id,
            false,
            "auth-required: authenticate before publishing",
        ));
        return Ok(());
    }
    let identity = context
        .state
        .db
        .identity_status(event.pubkey.clone())
        .await?;
    if identity.closed_membership
        && !context
            .auth
            .as_ref()
            .is_some_and(|auth| auth.is_authenticated_as(&event.pubkey))
    {
        pending.push_back(ok_message(
            &event.id,
            false,
            "auth-required: closed-relay events require authentication by their author",
        ));
        return Ok(());
    }
    if RELAY_ONLY_BLOCK_KINDS.contains(&event.kind) {
        pending.push_back(ok_message(
            &event.id,
            false,
            "restricted: this Block NIP kind is relay-authored only",
        ));
        return Ok(());
    }
    if matches!(event.kind, 78 | 30_078)
        && !context
            .auth
            .as_ref()
            .is_some_and(|auth| auth.is_authenticated_as(&event.pubkey))
    {
        pending.push_back(ok_message(
            &event.id,
            false,
            "auth-required: app data requires authentication by its exact author",
        ));
        return Ok(());
    }
    if nostr::profile::requires_author_auth(&event)
        && !context
            .auth
            .as_ref()
            .is_some_and(|auth| auth.is_authenticated_as(&event.pubkey))
    {
        pending.push_back(ok_message(
            &event.id,
            false,
            "auth-required: this OpenAgents record requires authentication by its author",
        ));
        return Ok(());
    }
    if event.is_protected()
        && !context
            .auth
            .as_ref()
            .is_some_and(|auth| auth.is_authenticated_as(&event.pubkey))
    {
        pending.push_back(ok_message(
            &event.id,
            false,
            "auth-required: protected event may only be published by its authenticated author",
        ));
        return Ok(());
    }
    if event.embeds_protected_repost() {
        pending.push_back(ok_message(
            &event.id,
            false,
            "invalid: repost must not embed a protected event",
        ));
        return Ok(());
    }

    if event.kind == WORKSPACE_PROFILE_KIND {
        return handle_workspace_profile(context, event, pending).await;
    }
    if matches!(
        event.kind,
        IDENTITY_ARCHIVE_REQUEST_KIND | IDENTITY_UNARCHIVE_REQUEST_KIND
    ) {
        return handle_identity_archive(context, event, pending).await;
    }
    if matches!(event.kind, DM_HIDE_KIND | DM_OPEN_KIND) {
        return handle_dm_visibility(context, event, pending).await;
    }

    if event.kind == AGENT_OBSERVER_KIND {
        return handle_agent_observer_event(context, event, pending).await;
    }

    let virtual_owner = context
        .auth
        .as_ref()
        .and_then(|auth| auth.virtual_owner_for(&event.pubkey))
        .map(str::to_owned);

    if event.kind == AGENT_TURN_METRIC_KIND {
        if !context
            .auth
            .as_ref()
            .is_some_and(|auth| auth.is_authenticated_as(&event.pubkey))
        {
            pending.push_back(ok_message(
                &event.id,
                false,
                "auth-required: agent turn metrics require agent authentication",
            ));
            return Ok(());
        }
        if let Err(error) = event.validate_crypto() {
            pending.push_back(ok_message(&event.id, false, &format!("invalid: {error}")));
            return Ok(());
        }
        let owner = match agent_turn_metric_owner(&event) {
            Ok(owner) => owner,
            Err(reason) => {
                pending.push_back(ok_message(&event.id, false, &format!("invalid: {reason}")));
                return Ok(());
            }
        };
        if !context
            .state
            .db
            .is_agent_owner(event.pubkey.clone(), owner)
            .await?
        {
            pending.push_back(ok_message(
                &event.id,
                false,
                "restricted: turn metric owner is not the authenticated main owner of this agent",
            ));
            return Ok(());
        }
    }

    if event.kind == PUSH_LEASE_KIND {
        if !context
            .auth
            .as_ref()
            .is_some_and(|auth| auth.is_authenticated_as(&event.pubkey))
        {
            pending.push_back(ok_message(
                &event.id,
                false,
                "auth-required: push leases require author authentication",
            ));
            return Ok(());
        }
        if let Err(error) = event.validate_crypto() {
            pending.push_back(ok_message(&event.id, false, &format!("invalid: {error}")));
            return Ok(());
        }
    }
    if let Err(reason) = validate_block_ingest(&event, unix_now()) {
        pending.push_back(ok_message(&event.id, false, &format!("invalid: {reason}")));
        return Ok(());
    }
    let mut lease = None;
    if event.kind == PUSH_LEASE_KIND {
        let Some(executor) = context.state.config.push.as_ref() else {
            pending.push_back(ok_message(
                &event.id,
                false,
                "restricted: this relay does not send push notifications",
            ));
            return Ok(());
        };
        match push::lease_write(executor, &event, unix_now()) {
            Ok(write) => lease = Some(write),
            Err(reason) => {
                pending.push_back(ok_message(&event.id, false, &format!("invalid: {reason}")));
                return Ok(());
            }
        }
    }

    if let Some(rejection) = event_key_rate_rejection(context, &event) {
        let reason = match rejection {
            EventKeyRateRejection::Event => "rate-limited: event rate exceeded",
            EventKeyRateRejection::GiftWrapRecipient => GIFT_WRAP_RECIPIENT_RATE_EXCEEDED,
            EventKeyRateRejection::WorldFull => WORLD_FULL,
            EventKeyRateRejection::WorldBudget => WORLD_BUDGET_EXCEEDED,
        };
        pending.push_back(ok_message(&event.id, false, reason));
        return Ok(());
    }
    admit_event(context, event, pending, virtual_owner, lease).await
}

async fn handle_workspace_profile(
    context: &mut ConnectionContext,
    event: Event,
    pending: &mut VecDeque<String>,
) -> Result<(), GatewayError> {
    if !context
        .auth
        .as_ref()
        .is_some_and(|auth| auth.is_directly_authenticated_as(&event.pubkey))
    {
        pending.push_back(ok_message(
            &event.id,
            false,
            "auth-required: workspace profile commands require direct relay-owner authentication",
        ));
        return Ok(());
    }
    if context.state.config.management_pubkey.as_deref() != Some(&event.pubkey) {
        pending.push_back(ok_message(
            &event.id,
            false,
            "restricted: workspace profile commands require the relay owner",
        ));
        return Ok(());
    }
    if event.created_at.abs_diff(unix_now()) > 120 {
        pending.push_back(ok_message(
            &event.id,
            false,
            "invalid: workspace profile command is outside the 120-second freshness window",
        ));
        return Ok(());
    }
    if let Err(error) = event.validate_crypto() {
        pending.push_back(ok_message(&event.id, false, &format!("invalid: {error}")));
        return Ok(());
    }
    let icon = match workspace_icon(&event) {
        Ok(icon) => icon,
        Err(reason) => {
            pending.push_back(ok_message(&event.id, false, &format!("invalid: {reason}")));
            return Ok(());
        }
    };
    if event_key_rate_rejection(context, &event).is_some() {
        pending.push_back(ok_message(
            &event.id,
            false,
            "rate-limited: event rate exceeded",
        ));
        return Ok(());
    }
    match context
        .state
        .db
        .set_workspace_icon(event.clone(), icon)
        .await
    {
        Ok(changed) => pending.push_back(ok_message(
            &event.id,
            true,
            if changed {
                ""
            } else {
                "duplicate: already processed"
            },
        )),
        Err(error) => {
            pending.push_back(ok_message(&event.id, false, &store_error_response(&error)))
        }
    }
    Ok(())
}

async fn handle_identity_archive(
    context: &mut ConnectionContext,
    event: Event,
    pending: &mut VecDeque<String>,
) -> Result<(), GatewayError> {
    if !context
        .auth
        .as_ref()
        .is_some_and(|auth| auth.is_authenticated_as(&event.pubkey))
    {
        pending.push_back(ok_message(
            &event.id,
            false,
            "auth-required: identity archive requests require actor authentication",
        ));
        return Ok(());
    }
    if context.state.config.relay_signer.is_none() {
        pending.push_back(ok_message(
            &event.id,
            false,
            "error: identity archival requires a configured relay identity",
        ));
        return Ok(());
    }
    if let Err(error) = event.validate_crypto() {
        pending.push_back(ok_message(&event.id, false, &format!("invalid: {error}")));
        return Ok(());
    }
    let now = unix_now();
    let request = match parse_identity_archive_request(&event, now) {
        Ok(request) => request,
        Err(reason) => {
            pending.push_back(ok_message(&event.id, false, &format!("invalid: {reason}")));
            return Ok(());
        }
    };
    if event_key_rate_rejection(context, &event).is_some() {
        pending.push_back(ok_message(
            &event.id,
            false,
            "rate-limited: event rate exceeded",
        ));
        return Ok(());
    }
    let consent = if request.target == event.pubkey {
        "self"
    } else if context.state.config.management_pubkey.as_deref() == Some(&event.pubkey)
        && context
            .auth
            .as_ref()
            .is_some_and(|auth| auth.is_directly_authenticated_as(&event.pubkey))
    {
        "admin"
    } else {
        let binding = match verify_owner_binding(&event, &request.target) {
            Ok(Some(binding)) => binding,
            Ok(None) => {
                pending.push_back(ok_message(
                    &event.id,
                    false,
                    "restricted: only the identity itself, its owner, or a relay administrator can make this request",
                ));
                return Ok(());
            }
            Err(reason) => {
                pending.push_back(ok_message(&event.id, false, &format!("invalid: {reason}")));
                return Ok(());
            }
        };
        if binding.owner_pubkey != event.pubkey
            || !context
                .state
                .db
                .materialize_agent_owner(request.target.clone(), event.pubkey.clone(), false)
                .await?
        {
            pending.push_back(ok_message(
                &event.id,
                false,
                "restricted: owner credential conflicts with the agent's main owner",
            ));
            return Ok(());
        }
        "owner"
    };
    match context
        .state
        .db
        .process_identity_archive(event.clone(), request, consent.to_owned(), now)
        .await
    {
        Ok(changed) => pending.push_back(ok_message(
            &event.id,
            true,
            if changed {
                ""
            } else {
                "duplicate: the identity is already in this archive state"
            },
        )),
        Err(error) => {
            pending.push_back(ok_message(&event.id, false, &store_error_response(&error)))
        }
    }
    Ok(())
}

async fn handle_dm_visibility(
    context: &mut ConnectionContext,
    event: Event,
    pending: &mut VecDeque<String>,
) -> Result<(), GatewayError> {
    if !context
        .auth
        .as_ref()
        .is_some_and(|auth| auth.is_authenticated_as(&event.pubkey))
    {
        pending.push_back(ok_message(
            &event.id,
            false,
            "auth-required: DM visibility commands require actor authentication",
        ));
        return Ok(());
    }
    if context.state.config.relay_signer.is_none() {
        pending.push_back(ok_message(
            &event.id,
            false,
            "error: DM visibility requires a configured relay identity",
        ));
        return Ok(());
    }
    if let Err(error) = event.validate_crypto() {
        pending.push_back(ok_message(&event.id, false, &format!("invalid: {error}")));
        return Ok(());
    }
    let channel = match dm_visibility_channel(&event) {
        Ok(channel) => channel.to_owned(),
        Err(reason) => {
            pending.push_back(ok_message(&event.id, false, &format!("invalid: {reason}")));
            return Ok(());
        }
    };
    if event_key_rate_rejection(context, &event).is_some() {
        pending.push_back(ok_message(
            &event.id,
            false,
            "rate-limited: event rate exceeded",
        ));
        return Ok(());
    }
    let hidden = event.kind == DM_HIDE_KIND;
    match context
        .state
        .db
        .process_dm_visibility(event.clone(), channel, hidden, unix_now())
        .await
    {
        Ok(changed) => pending.push_back(ok_message(
            &event.id,
            true,
            if changed {
                ""
            } else {
                "duplicate: the visibility is already set to this state"
            },
        )),
        Err(StoreError::Management(reason)) => pending.push_back(ok_message(
            &event.id,
            false,
            &format!("restricted: {}", bounded(&reason, 512)),
        )),
        Err(error) => {
            pending.push_back(ok_message(&event.id, false, &store_error_response(&error)))
        }
    }
    Ok(())
}

async fn handle_agent_observer_event(
    context: &mut ConnectionContext,
    event: Event,
    pending: &mut VecDeque<String>,
) -> Result<(), GatewayError> {
    if !context
        .auth
        .as_ref()
        .is_some_and(|auth| auth.is_authenticated_as(&event.pubkey))
    {
        pending.push_back(ok_message(
            &event.id,
            false,
            "auth-required: agent observer frames require sender authentication",
        ));
        return Ok(());
    }
    if let Err(error) = event.validate_crypto() {
        pending.push_back(ok_message(&event.id, false, &format!("invalid: {error}")));
        return Ok(());
    }
    if event.created_at.abs_diff(unix_now()) > 300 {
        pending.push_back(ok_message(
            &event.id,
            false,
            "invalid: the agent observer frame's timestamp is more than five minutes from the relay's clock",
        ));
        return Ok(());
    }
    let route = match agent_observer_route(&event) {
        Ok(Some(route)) => route,
        Ok(None) => {
            pending.push_back(ok_message(&event.id, true, ""));
            return Ok(());
        }
        Err(reason) => {
            pending.push_back(ok_message(&event.id, false, &format!("invalid: {reason}")));
            return Ok(());
        }
    };
    if !context
        .state
        .db
        .is_agent_owner(route.agent_pubkey.clone(), route.owner_pubkey.clone())
        .await?
    {
        pending.push_back(ok_message(
            &event.id,
            false,
            "restricted: observer frame is not authorized for this agent owner",
        ));
        return Ok(());
    }
    if !context.state.rate.observer_from_ip(context.ip)
        || !context.state.rate.observer_from_agent(&route.agent_pubkey)
    {
        pending.push_back(ok_message(
            &event.id,
            false,
            "rate-limited: agent observer frame rate exceeded",
        ));
        return Ok(());
    }
    let virtual_owner = (route.direction == AgentObserverDirection::Telemetry)
        .then(|| {
            context
                .auth
                .as_ref()
                .and_then(|auth| auth.virtual_owner_for(&event.pubkey))
                .map(str::to_owned)
        })
        .flatten();
    admit_event(context, event, pending, virtual_owner, None).await
}

async fn admit_event(
    context: &mut ConnectionContext,
    event: Event,
    pending: &mut VecDeque<String>,
    virtual_owner: Option<String>,
    lease: Option<PushLeaseWrite>,
) -> Result<(), GatewayError> {
    let event_bytes = serde_json::to_vec(&event)
        .map_err(|error| GatewayError::Internal(format!("event serialization: {error}")))?;
    if event_bytes.len() > context.state.config.limits.max_frame_bytes {
        pending.push_back(ok_message(
            &event.id,
            false,
            "invalid: event exceeds the configured byte limit",
        ));
        return Ok(());
    }
    let event_id = event.id.clone();
    let ephemeral = (event.class() == EventClass::Ephemeral).then(|| Arc::new(event.clone()));
    // A stored event outside a group is delivered from memory once its
    // admission commits. A group admission may also write relay-signed
    // events, so it is delivered from the database in sequence order.
    let local = (event.class() != EventClass::Ephemeral
        && !crate::store::writes_group_state(&event))
    .then(|| Arc::new(event.clone()));
    let admission_now = unix_now();
    let is_lease = lease.is_some();
    let admission = match lease {
        Some(lease) => {
            context
                .state
                .db
                .admit_push_lease(event, admission_now, lease)
                .await
        }
        None => {
            context
                .state
                .db
                .admit(event, admission_now, virtual_owner)
                .await
        }
    };
    match admission {
        Ok(outcome) => {
            if matches!(&outcome, AdmissionOutcome::Ephemeral)
                && let Some(event) = ephemeral
            {
                if context.state.config.log_level == "debug" {
                    print_ephemeral_admitted(&event);
                }
                let published = PublishedEvent {
                    event,
                    ingest_seq: None,
                };
                if context
                    .state
                    .hub
                    .publish(published, admission_now)
                    .await
                    .is_err()
                {
                    fail_process(
                        &context.state.current,
                        &context.state.shutdown,
                        "the subscription hub stopped",
                    );
                }
            }
            if let AdmissionOutcome::Stored { ingest_seq } = outcome
                && let Some(event) = local
            {
                // Full means the delivery task is behind; the event's
                // notification then delivers it.
                let _ = context.state.committed.try_send(PublishedEvent {
                    event,
                    ingest_seq: Some(ingest_seq),
                });
            }
            let stored = matches!(outcome, AdmissionOutcome::Stored { .. });
            let (accepted, reason) = if is_lease
                && matches!(
                    outcome,
                    AdmissionOutcome::Rejected(AdmissionRejection::Superseded)
                ) {
                (false, "invalid: stale replacement".to_owned())
            } else {
                admission_response(outcome)
            };
            if stored && !is_lease && context.state.config.push.is_some() {
                context.state.push_wake.notify_one();
            }
            pending.push_back(ok_message(&event_id, accepted, &reason));
        }
        Err(error) => {
            pending.push_back(ok_message(&event_id, false, &store_error_response(&error)));
        }
    }
    Ok(())
}

/// Shared admission for every read that queries stored history (`REQ` and
/// NIP-77 `NEG-OPEN`): the auth-required gate, then the per-IP `REQ` rate.
/// The refusal is the machine-readable reason to send back.
fn admit_read(context: &ConnectionContext) -> Result<(), &'static str> {
    read_admission(
        context.state.config.auth_required,
        context.auth.as_ref(),
        &context.state.rate,
        context.ip,
    )
}

fn read_admission(
    auth_required: bool,
    auth: Option<&AuthState>,
    rate: &RateLimiter,
    ip: std::net::IpAddr,
) -> Result<(), &'static str> {
    if auth_required && !auth.is_some_and(AuthState::is_authenticated) {
        return Err("auth-required: authenticate before subscribing");
    }
    if !rate.req_from_ip(ip) {
        return Err("rate-limited: REQ rate exceeded");
    }
    Ok(())
}

async fn handle_req(
    context: &mut ConnectionContext,
    subscription_id: String,
    filters: Vec<Filter>,
    pending: &mut VecDeque<String>,
) -> Result<(), GatewayError> {
    if let Err(reason) = admit_read(context) {
        pending.push_back(closed_message(&subscription_id, reason));
        return Ok(());
    }
    if subscription_id.is_empty() || subscription_id.chars().count() > 64 {
        pending.push_back(closed_message(
            &subscription_id,
            "invalid: subscription id must contain 1 to 64 characters",
        ));
        return Ok(());
    }
    if filters.len() > context.state.config.limits.max_filters {
        pending.push_back(closed_message(
            &subscription_id,
            "restricted: too many filters",
        ));
        return Ok(());
    }
    if !context.active_subscriptions.contains(&subscription_id)
        && context.active_subscriptions.len() >= context.state.config.limits.max_subscriptions
    {
        pending.push_back(closed_message(
            &subscription_id,
            "restricted: too many active subscriptions",
        ));
        return Ok(());
    }
    let filters = match validate_and_clamp_filters(filters, &context.state.config) {
        Ok(filters) => filters,
        Err(reason) => {
            pending.push_back(closed_message(&subscription_id, &reason));
            return Ok(());
        }
    };
    let read_pubkeys = context
        .auth
        .as_ref()
        .map(AuthState::authenticated_pubkeys)
        .unwrap_or_default();
    if let Some(reason) = owner_scoped_filter_denial(&filters, &read_pubkeys) {
        pending.push_back(closed_message(&subscription_id, reason));
        return Ok(());
    }
    if context.query_tasks.len()
        >= context
            .state
            .config
            .limits
            .max_subscriptions
            .saturating_mul(2)
    {
        pending.push_back(closed_message(
            &subscription_id,
            "rate-limited: too many historical queries in flight",
        ));
        return Ok(());
    }

    context.generation = context.generation.wrapping_add(1).max(1);
    let generation = context.generation;
    let previous_cancellation = context.cancellations.remove(&subscription_id);
    if !context
        .state
        .hub
        .register_for(
            context.connection_id,
            subscription_id.clone(),
            generation,
            filters.clone(),
            context
                .auth
                .as_ref()
                .map(AuthState::authenticated_pubkeys)
                .unwrap_or_default()
                .into_iter()
                .collect(),
        )
        .await?
    {
        return Err(GatewayError::Internal(
            "connection disappeared while registering subscription".to_owned(),
        ));
    }
    if let Some(cancellation) = previous_cancellation {
        let _ = cancellation.send(true);
    }
    context.active_subscriptions.insert(subscription_id.clone());
    let (cancel, cancel_receiver) = watch::channel(false);
    context
        .cancellations
        .insert(subscription_id.clone(), cancel);
    let db = context.state.db.clone();
    let hub = context.state.hub.clone();
    let connection_id = context.connection_id;
    let max_results = context.state.config.limits.history_limit();
    context.query_tasks.spawn(async move {
        match db
            .history(
                filters,
                unix_now(),
                max_results,
                cancel_receiver,
                read_pubkeys,
            )
            .await
        {
            Ok(history) => {
                hub.history_ready(
                    connection_id,
                    subscription_id,
                    generation,
                    history.high_water,
                    history.events,
                    history.complete,
                )
                .await;
            }
            Err(StoreError::QueryCancelled) => {}
            Err(_) => {
                hub.close_subscription(
                    connection_id,
                    subscription_id,
                    "error: historical query failed".to_owned(),
                )
                .await;
            }
        }
    });
    Ok(())
}

async fn handle_count(
    context: &mut ConnectionContext,
    query_id: String,
    filters: Vec<Filter>,
    pending: &mut VecDeque<String>,
) -> Result<(), GatewayError> {
    if !context.state.rate.req_from_ip(context.ip) {
        pending.push_back(closed_message(
            &query_id,
            "rate-limited: COUNT rate exceeded",
        ));
        return Ok(());
    }
    if filters.len() > context.state.config.limits.max_filters {
        pending.push_back(closed_message(&query_id, "restricted: too many filters"));
        return Ok(());
    }
    let filters = match validate_and_clamp_filters(filters, &context.state.config) {
        Ok(filters) => filters,
        Err(reason) => {
            pending.push_back(closed_message(&query_id, &reason));
            return Ok(());
        }
    };
    let read_pubkeys = context
        .auth
        .as_ref()
        .map(AuthState::authenticated_pubkeys)
        .unwrap_or_default();
    if let Some(reason) = owner_scoped_filter_denial(&filters, &read_pubkeys) {
        pending.push_back(closed_message(&query_id, reason));
        return Ok(());
    }
    if filters.iter().any(|filter| {
        filter
            .kinds
            .as_ref()
            .is_some_and(|kinds| kinds.contains(&1_059))
    }) && read_pubkeys.is_empty()
    {
        pending.push_back(closed_message(
            &query_id,
            "auth-required: cannot count gift wraps without recipient authentication",
        ));
        return Ok(());
    }
    match context
        .state
        .db
        .count(
            filters,
            unix_now(),
            context.state.config.limits.max_query_cost,
            read_pubkeys,
        )
        .await
    {
        Ok(Some(count)) => pending.push_back(count_message(&query_id, count)),
        Ok(None) => pending.push_back(closed_message(
            &query_id,
            "restricted: this count would cost more than the relay's query limit allows; narrow the filters",
        )),
        Err(_) => pending.push_back(closed_message(&query_id, "error: count query failed")),
    }
    Ok(())
}

fn owner_scoped_filter_denial(filters: &[Filter], read_pubkeys: &[String]) -> Option<&'static str> {
    for filter in filters {
        let explicitly_private = filter.kinds.as_ref().is_some_and(|kinds| {
            kinds.iter().any(|kind| {
                matches!(
                    *kind,
                    1_059
                        | AGENT_OBSERVER_KIND
                        | AGENT_TURN_METRIC_KIND
                        | AGENT_ENGRAM_KIND
                        | EVENT_REMINDER_KIND
                        | PUSH_LEASE_KIND
                        | DM_VISIBILITY_KIND
                )
            })
        });
        if !explicitly_private {
            continue;
        }
        if read_pubkeys.is_empty() {
            if filter
                .kinds
                .as_ref()
                .is_some_and(|kinds| kinds.contains(&1_059))
            {
                return Some("auth-required: gift-wrap reads require recipient authentication");
            }
            return Some("auth-required: private Block NIP reads require authentication");
        }
        let kinds = filter.kinds.as_deref().unwrap_or_default();
        let p_scoped = filter.tags.get("p").is_some_and(|values| {
            !values.is_empty()
                && values
                    .iter()
                    .all(|value| read_pubkeys.iter().any(|pubkey| pubkey == value))
        });
        let author_scoped = filter.authors.as_ref().is_some_and(|values| {
            !values.is_empty()
                && values
                    .iter()
                    .all(|value| read_pubkeys.iter().any(|pubkey| pubkey == value))
        });
        if kinds.contains(&1_059) && !p_scoped {
            return Some(
                "restricted: to read gift wraps, filter #p to your own authenticated public key",
            );
        }
        if kinds.iter().any(|kind| {
            matches!(
                *kind,
                AGENT_OBSERVER_KIND | AGENT_TURN_METRIC_KIND | DM_VISIBILITY_KIND
            )
        }) && !p_scoped
        {
            return Some(
                "restricted: to read these private events, filter #p to your own authenticated public key",
            );
        }
        if kinds
            .iter()
            .any(|kind| matches!(*kind, EVENT_REMINDER_KIND | PUSH_LEASE_KIND))
            && !author_scoped
        {
            return Some(
                "restricted: to read these private events, filter authors to your own authenticated public key",
            );
        }
        if kinds.contains(&AGENT_ENGRAM_KIND) && !p_scoped && !author_scoped {
            return Some(
                "restricted: to read agent engrams, filter authors to the agent or #p to its owner, using your authenticated public key",
            );
        }
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EventKeyRateRejection {
    Event,
    GiftWrapRecipient,
    WorldFull,
    WorldBudget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum EventPreflightRejection {
    IpRate,
    InvalidCrypto(crate::domain::DomainError),
}

/// The refusal of a pose-lane event from a new key while its world holds
/// its population cap. It is `rate-limited:` so the publisher backs off and
/// tries again later, when a present player may have left.
const WORLD_FULL: &str = "rate-limited: world is full; try again when a player leaves";
/// The refusal of a pose-lane event once its world spent this second's budget.
const WORLD_BUDGET_EXCEEDED: &str = "rate-limited: world pose budget exceeded";

/// Whether `kind` travels in the pose lane: NIP-MV frames and gestures,
/// ephemeral events a moving player publishes several times a second, which
/// the relay counts by the second instead of against the per-minute budget.
fn is_pose_lane(kind: u16) -> bool {
    kind == nostr::kinds::MV_FRAME || kind == nostr::kinds::MV_GESTURE
}

fn event_preflight(
    rate: &RateLimiter,
    ip: std::net::IpAddr,
    event: &Event,
) -> Result<(), EventPreflightRejection> {
    let allowed = if is_pose_lane(event.kind) {
        rate.pose_from_ip(ip)
    } else {
        rate.event_from_ip(ip)
    };
    if !allowed {
        return Err(EventPreflightRejection::IpRate);
    }
    event
        .validate_crypto()
        .map_err(EventPreflightRejection::InvalidCrypto)
}

fn event_key_rate_rejection(
    context: &ConnectionContext,
    event: &Event,
) -> Option<EventKeyRateRejection> {
    let virtual_owner = context
        .auth
        .as_ref()
        .and_then(|auth| auth.virtual_owner_for(&event.pubkey));
    event_key_rate_rejection_for(&context.state.rate, event, virtual_owner)
}

fn event_key_rate_rejection_for(
    rate: &RateLimiter,
    event: &Event,
    virtual_owner: Option<&str>,
) -> Option<EventKeyRateRejection> {
    let allowed = if is_pose_lane(event.kind) {
        rate.pose_from_pubkey(&event.pubkey)
            && virtual_owner.is_none_or(|owner| rate.pose_from_pubkey(owner))
    } else {
        rate.event_from_pubkey(&event.pubkey)
            && virtual_owner.is_none_or(|owner| rate.event_from_pubkey(owner))
    };
    if !allowed {
        return Some(EventKeyRateRejection::Event);
    }
    if is_pose_lane(event.kind)
        && let Some(world) = event.tag_values("w").next()
    {
        match rate.pose_in_world(world, &event.pubkey) {
            Ok(()) => {}
            Err(WorldRefusal::Full) => return Some(EventKeyRateRejection::WorldFull),
            Err(WorldRefusal::Budget) => return Some(EventKeyRateRejection::WorldBudget),
        }
    }
    if event.kind == 1_059
        && !event
            .gift_wrap_recipient()
            .is_some_and(|recipient| rate.gift_wrap_for_recipient(recipient))
    {
        return Some(EventKeyRateRejection::GiftWrapRecipient);
    }
    None
}

fn validate_and_clamp_filters(
    mut filters: Vec<Filter>,
    config: &GatewayConfig,
) -> Result<Vec<Filter>, String> {
    let mut total_cost = 0_usize;
    for filter in &mut filters {
        filter
            .validate()
            .map_err(|error| format!("invalid: {error}"))?;
        if filter.ids.as_ref().is_some_and(Vec::is_empty)
            || filter.authors.as_ref().is_some_and(Vec::is_empty)
            || filter.kinds.as_ref().is_some_and(Vec::is_empty)
            || filter.tags.values().any(Vec::is_empty)
        {
            return Err("invalid: filter arrays must not be empty".to_owned());
        }
        for (name, values) in &filter.tags {
            if matches!(name.as_str(), "e" | "p")
                && values.iter().any(|value| {
                    value.len() != 64
                        || !value
                            .as_bytes()
                            .iter()
                            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
                })
            {
                return Err(format!(
                    "invalid: #{name} values must be 64 lowercase hexadecimal characters"
                ));
            }
        }
        let limit = filter
            .limit
            .unwrap_or(config.limits.max_limit)
            .min(config.limits.max_limit);
        filter.limit = Some(limit);
        let selector_values = filter.ids.as_ref().map_or(0, Vec::len)
            + filter.authors.as_ref().map_or(0, Vec::len)
            + filter.kinds.as_ref().map_or(0, Vec::len)
            + filter.tags.values().map(Vec::len).sum::<usize>();
        let factor = if filter.search.is_some() {
            100
        } else if filter.ids.is_some() {
            1
        } else if filter.authors.is_some() && filter.kinds.is_some() {
            4
        } else if !filter.tags.is_empty() {
            10
        } else if filter.authors.is_some() || filter.kinds.is_some() {
            20
        } else if filter.since.is_some() || filter.until.is_some() {
            50
        } else {
            100
        };
        let cost = limit
            .saturating_mul(factor)
            .saturating_add(selector_values.saturating_mul(factor));
        total_cost = total_cost.saturating_add(cost);
        if total_cost > config.limits.max_query_cost {
            return Err("restricted: query cost exceeds the configured limit".to_owned());
        }
    }
    Ok(filters)
}

fn admission_response(outcome: AdmissionOutcome) -> (bool, String) {
    match outcome {
        AdmissionOutcome::Stored { .. } | AdmissionOutcome::Ephemeral => (true, String::new()),
        AdmissionOutcome::Duplicate => (true, "duplicate: already have this event".to_owned()),
        AdmissionOutcome::Rejected(rejection) => match rejection {
            AdmissionRejection::BlockedPubkey(reason) | AdmissionRejection::BlockedKind(reason) => {
                (false, format!("blocked: {}", bounded(&reason, 512)))
            }
            AdmissionRejection::PubkeyNotAllowed
            | AdmissionRejection::KindNotAllowed
            | AdmissionRejection::NotMember => (
                false,
                "restricted: event is not allowed by relay policy".to_owned(),
            ),
            AdmissionRejection::ContentTooLarge { .. } => {
                (false, "invalid: event content is too large".to_owned())
            }
            AdmissionRejection::TooManyTags { .. } => {
                (false, "invalid: event has too many tags".to_owned())
            }
            AdmissionRejection::TimestampTooFarInFuture { .. }
            | AdmissionRejection::TimestampTooOld { .. } => (
                false,
                "invalid: the event's created_at timestamp is too far in the past or the future for this relay".to_owned(),
            ),
            AdmissionRejection::AuthEvent => (
                false,
                "invalid: authentication events cannot be published".to_owned(),
            ),
            AdmissionRejection::Deleted => (
                false,
                "blocked: event is covered by a deletion request".to_owned(),
            ),
            AdmissionRejection::Superseded => (
                true,
                "duplicate: newer replaceable event already stored".to_owned(),
            ),
            AdmissionRejection::GroupNotFound => {
                (false, "restricted: group does not exist".to_owned())
            }
            AdmissionRejection::GroupUnauthorized => (
                false,
                "restricted: only a group member or administrator can do this".to_owned(),
            ),
            AdmissionRejection::GroupClosed => (false, "restricted: group is closed".to_owned()),
            AdmissionRejection::GroupAlreadyMember => {
                (false, "duplicate: you are already a member of this group".to_owned())
            }
            AdmissionRejection::GroupUnsupportedKind => (
                false,
                "restricted: event kind is not supported by this group".to_owned(),
            ),
            AdmissionRejection::GroupPreviousUnknown => (
                false,
                "invalid: the event's previous tag names an event that is not in this relay's recent group history".to_owned(),
            ),
            AdmissionRejection::GroupSigningUnavailable => (
                false,
                "error: relay group signing key is unavailable".to_owned(),
            ),
            AdmissionRejection::GroupHierarchy(reason) => (false, format!("invalid: {reason}")),
            AdmissionRejection::PushLease(reason) => (false, format!("invalid: {reason}")),
        },
    }
}

fn store_error_response(error: &StoreError) -> String {
    match error {
        StoreError::Domain(reason) => format!("invalid: {}", bounded(&reason.to_string(), 512)),
        StoreError::TimestampOutOfRange { .. }
        | StoreError::InvalidLimit(_)
        | StoreError::Serialization(_)
        | StoreError::EphemeralTooLarge(_) => {
            format!("invalid: {}", bounded(&error.to_string(), 512))
        }
        StoreError::QueryCancelled => {
            "error: the relay cancelled storing this event; try again".to_owned()
        }
        _ => "error: the relay's storage is unavailable; try again later".to_owned(),
    }
}

fn bounded(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn queue_websocket_text(
    websocket: &mut ServerWebSocket,
    message: String,
) -> Result<(), GatewayError> {
    match websocket.write(Message::text(message)) {
        Ok(()) => Ok(()),
        Err(WebSocketError::Io(error)) if error.kind() == io::ErrorKind::WouldBlock => Ok(()),
        Err(error) => Err(error.into()),
    }
}

/// Start the NIP-PL worker on its own verified connection. A lost
/// connection fails the process, like the other background stores.
async fn spawn_push_worker(
    database_url: &str,
    executor: PushExecutor,
    wake: Arc<tokio::sync::Notify>,
    shutdown: watch::Sender<bool>,
    stop: watch::Receiver<bool>,
    current: Arc<AtomicBool>,
) -> Result<JoinHandle<()>, GatewayError> {
    let mut store = Store::connect_verified(database_url).await?;
    // The first pass fixes a new origin's cursor before the socket binds,
    // so no event admitted through this process precedes it.
    push::match_events(&mut store, &executor, unix_now()).await?;
    let entropy = read_process_secret()?;
    let worker = entropy[..16]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Ok(tokio::spawn(async move {
        if push::run_worker(store, executor, worker, wake, stop)
            .await
            .is_err()
        {
            fail_process(&current, &shutdown, "the push worker failed");
        }
    }))
}

fn fail_process(current: &AtomicBool, shutdown: &watch::Sender<bool>, reason: &str) {
    log_failure("the relay is stopping", reason);
    current.store(false, Ordering::Release);
    let _ = shutdown.send(true);
}

/// One JSON error line naming what stopped the relay, before it exits.
pub(super) fn log_failure(message: &str, reason: &str) {
    log_line(serde_json::json!({"level": "error", "message": message, "reason": reason}));
}

fn log_warning(message: &str, reason: &str) {
    log_line(serde_json::json!({"level": "warn", "message": message, "reason": reason}));
}

fn log_info(message: &str, latest_ingest_seq: i64) {
    log_line(serde_json::json!({
        "level": "info",
        "message": message,
        "latest_ingest_seq": latest_ingest_seq,
    }));
}

/// Writes to stderr and ignores a closed stream: a log line never stops
/// the relay.
fn log_line(line: serde_json::Value) {
    use std::io::Write;
    let _ = writeln!(io::stderr().lock(), "{line}");
}

pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use crate::{
        domain::{Event, Filter},
        gateway::GatewayConfig,
        store::StoredEvent,
    };

    use super::{
        DurableSequence, EventKeyRateRejection, MAX_NOTIFICATION_GAP, event_key_rate_rejection_for,
        read_admission, validate_and_clamp_filters,
    };
    use crate::gateway::auth::AuthState;
    use crate::gateway::{GatewayLimits, rate::RateLimiter};

    fn frame(pubkey: &str, world: Option<&str>) -> Event {
        Event {
            id: "0".repeat(64),
            pubkey: pubkey.to_owned(),
            created_at: 0,
            kind: nostr::kinds::MV_FRAME,
            tags: world
                .map(|world| vec![crate::domain::Tag(vec!["w".to_owned(), world.to_owned()])])
                .unwrap_or_default(),
            content: String::new(),
            sig: "0".repeat(128),
        }
    }

    #[test]
    fn read_admission_refuses_anonymous_reads_on_an_auth_required_relay() {
        let rate = RateLimiter::new(GatewayLimits::default());
        let ip = "203.0.113.5".parse().unwrap();
        let mut auth = AuthState::new("challenge".to_owned(), "ws://relay.test".to_owned());
        assert_eq!(
            read_admission(true, None, &rate, ip),
            Err("auth-required: authenticate before subscribing")
        );
        assert_eq!(
            read_admission(true, Some(&auth), &rate, ip),
            Err("auth-required: authenticate before subscribing")
        );
        auth.accept_direct("a".repeat(64));
        assert_eq!(read_admission(true, Some(&auth), &rate, ip), Ok(()));
        assert_eq!(read_admission(false, None, &rate, ip), Ok(()));
    }

    #[test]
    fn read_admission_spends_the_shared_req_budget() {
        // REQ and NEG-OPEN both admit through `read_admission`, so one
        // per-IP budget covers both and NEG-OPEN cannot bypass it.
        let rate = RateLimiter::new(GatewayLimits {
            req_per_minute_ip: 2,
            ..GatewayLimits::default()
        });
        let ip = "203.0.113.6".parse().unwrap();
        assert_eq!(read_admission(false, None, &rate, ip), Ok(()));
        assert_eq!(read_admission(false, None, &rate, ip), Ok(()));
        assert_eq!(
            read_admission(false, None, &rate, ip),
            Err("rate-limited: REQ rate exceeded")
        );
    }

    #[test]
    fn a_world_at_its_cap_refuses_a_new_players_frames() {
        let rate = RateLimiter::new(GatewayLimits {
            world_population_cap: 1,
            ..GatewayLimits::default()
        });
        let first = "a".repeat(64);
        let second = "b".repeat(64);
        assert_eq!(
            event_key_rate_rejection_for(&rate, &frame(&first, Some("verse-bare")), None),
            None
        );
        assert_eq!(
            event_key_rate_rejection_for(&rate, &frame(&second, Some("verse-bare")), None),
            Some(EventKeyRateRejection::WorldFull)
        );
        // The same key in another world, and a frame naming no world, pass.
        assert_eq!(
            event_key_rate_rejection_for(&rate, &frame(&second, Some("verse-ruins")), None),
            None
        );
        assert_eq!(
            event_key_rate_rejection_for(&rate, &frame(&second, None), None),
            None
        );
        assert_eq!(rate.world_population("verse-bare"), 1);
    }

    fn stored(ingest_seq: i64) -> StoredEvent {
        StoredEvent {
            event: Event {
                id: format!("{ingest_seq:064x}"),
                pubkey: "0".repeat(64),
                created_at: 0,
                kind: 1,
                tags: Vec::new(),
                content: String::new(),
                sig: "0".repeat(128),
            },
            ingest_seq,
        }
    }

    fn sequences((events, consistent): (Vec<StoredEvent>, bool)) -> Vec<i64> {
        assert!(consistent);
        events.into_iter().map(|stored| stored.ingest_seq).collect()
    }

    #[test]
    fn a_commit_right_after_the_last_delivered_goes_out_from_memory() {
        let mut sequence = DurableSequence::new(10);
        assert!(sequence.committed(11));
        assert_eq!(sequence.read_needed(), None);
        // Its own notification, arriving later, asks for nothing.
        sequence.notified(11);
        assert_eq!(sequence.read_needed(), None);
        assert!(sequence.committed(12));
        assert!(!sequence.committed(12), "a sequence is delivered once");
    }

    #[test]
    fn a_commit_past_a_gap_waits_for_the_read_of_the_gap() {
        let mut sequence = DurableSequence::new(10);
        // Another process committed 11; this one committed 12.
        assert!(!sequence.committed(12));
        assert_eq!(sequence.read_needed(), Some((10, 12)));
        let delivered = sequences(sequence.read(12, 12, vec![stored(11), stored(12)]));
        assert_eq!(delivered, [11, 12]);
        assert_eq!(sequence.read_needed(), None);
        assert!(sequence.committed(13));
    }

    #[test]
    fn a_read_delivers_only_what_memory_did_not() {
        let mut sequence = DurableSequence::new(10);
        // A notification for 11 starts a read before the commit arrives.
        sequence.notified(11);
        assert_eq!(sequence.read_needed(), Some((10, 11)));
        sequence.notified(13);
        assert!(sequence.committed(11));
        // The read through 11 was already covered; the next asks for 12..13.
        assert_eq!(sequence.read_needed(), Some((11, 13)));
        // A read that started at 10 still delivers nothing twice.
        let delivered = sequences(sequence.read(13, 13, vec![stored(11), stored(12), stored(13)]));
        assert_eq!(delivered, [12, 13]);
        assert!(!sequence.committed(13));
        assert_eq!(sequence.read_needed(), None);
    }

    #[test]
    fn a_long_outage_is_read_back_in_bounded_steps() {
        let step = i64::try_from(MAX_NOTIFICATION_GAP).unwrap();
        let mut sequence = DurableSequence::new(10);
        // A replacement listener reports far more than one read covers.
        sequence.notified(10 + 2 * step + 5);
        assert_eq!(sequence.read_needed(), Some((10, 10 + step)));
        let (events, consistent) = sequence.read(10 + step, 10 + 2 * step + 5, vec![stored(11)]);
        assert!(consistent);
        assert_eq!(events.len(), 1);
        assert_eq!(sequence.read_needed(), Some((10 + step, 10 + 2 * step)));
        let _ = sequence.read(10 + 2 * step, 10 + 2 * step + 5, Vec::new());
        assert_eq!(
            sequence.read_needed(),
            Some((10 + 2 * step, 10 + 2 * step + 5))
        );
        let _ = sequence.read(10 + 2 * step + 5, 10 + 2 * step + 5, Vec::new());
        assert_eq!(sequence.read_needed(), None);
    }

    #[test]
    fn a_notification_past_the_database_is_dropped_without_skipping_real_events() {
        let mut sequence = DurableSequence::new(10);
        sequence.notified(10_000);
        let (after, through) = sequence.read_needed().unwrap();
        assert_eq!(
            (after, through),
            (10, 10 + i64::try_from(MAX_NOTIFICATION_GAP).unwrap())
        );
        // The database holds only 11 and 12; a row committed after the
        // read sampled `latest` is not delivered by it.
        let (events, consistent) =
            sequence.read(through, 12, vec![stored(11), stored(12), stored(13)]);
        assert!(!consistent);
        assert_eq!(
            events
                .iter()
                .map(|stored| stored.ingest_seq)
                .collect::<Vec<_>>(),
            [11, 12]
        );
        assert_eq!(sequence.read_needed(), None);
        // The next real commit, 13, still goes out exactly once.
        assert!(sequence.committed(13));
        assert!(!sequence.committed(13));
    }

    #[test]
    fn req_limits_reject_empty_arrays_and_expensive_queries_and_clamp_limits() {
        let mut config = GatewayConfig::new(
            "host=/tmp dbname=test".to_owned(),
            "127.0.0.1:0".parse().unwrap(),
        );
        config.limits.max_limit = 10;
        config.limits.max_query_cost = 1_000;

        let empty = Filter {
            ids: Some(Vec::new()),
            ..Filter::default()
        };
        assert!(validate_and_clamp_filters(vec![empty], &config).is_err());

        let expensive = vec![Filter::default(), Filter::default()];
        assert!(validate_and_clamp_filters(expensive, &config).is_err());

        let bounded = Filter {
            ids: Some(vec!["a".repeat(64)]),
            limit: Some(1_000),
            ..Filter::default()
        };
        let filters = validate_and_clamp_filters(vec![bounded], &config).unwrap();
        assert_eq!(filters[0].limit, Some(10));
    }
}
