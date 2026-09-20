use std::{
    collections::{HashMap, HashSet, VecDeque},
    io,
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use tokio::{
    net::{TcpListener, TcpStream},
    sync::{Semaphore, watch},
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
        AdmissionOutcome, AdmissionRejection, NotificationListener, Store, StoreError,
        StoreNotification,
    },
};

use super::{
    GatewayConfig, GatewayError,
    auth::{AuthState, make_challenge, read_process_secret},
    db::{DbPool, DbProtocolConfig},
    management::{is_management_request, serve_management},
    media::{MediaStorage, STALE_RESERVATION_AGE, is_media_request, serve_media},
    rate::{ConnectionPermit, RateLimiter},
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
const MAX_NOTIFICATION_GAP: usize = 4_096;

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
        let mut notifications =
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
                            fail_process(&expiration_current, &expiration_shutdown);
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
                                    fail_process(&expiration_current, &expiration_shutdown);
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
                                    fail_process(&import_current, &import_shutdown);
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
        background.push(tokio::spawn(async move {
            let mut durable_cursor = initial_ingest_seq;
            loop {
                tokio::select! {
                    changed = notify_stop.changed() => {
                        if changed.is_err() || *notify_stop.borrow() {
                            break;
                        }
                    }
                    notification = notifications.recv_notification() => {
                        let Some(notification) = notification else {
                            fail_process(&notify_current, &notify_shutdown);
                            break;
                        };
                        let now = unix_now();
                        let published = match notification {
                            StoreNotification::Stored(ingest_seq) => {
                                if ingest_seq <= durable_cursor {
                                    continue;
                                }
                                let Some(gap) = ingest_seq
                                    .checked_sub(durable_cursor)
                                    .and_then(|gap| usize::try_from(gap).ok())
                                else {
                                    fail_process(&notify_current, &notify_shutdown);
                                    break;
                                };
                                if gap > MAX_NOTIFICATION_GAP {
                                    fail_process(&notify_current, &notify_shutdown);
                                    break;
                                }
                                let catch_up = match notify_db
                                    .catch_up(
                                        durable_cursor,
                                        ingest_seq,
                                        now,
                                        MAX_NOTIFICATION_GAP + 1,
                                    )
                                    .await
                                {
                                    Ok(catch_up)
                                        if catch_up.latest >= ingest_seq
                                            && catch_up.events.len() <= MAX_NOTIFICATION_GAP =>
                                    {
                                        catch_up
                                    }
                                    Ok(_) | Err(_) => {
                                        fail_process(&notify_current, &notify_shutdown);
                                        break;
                                    }
                                };
                                let mut failed = false;
                                for stored in catch_up.events {
                                    if notify_hub
                                        .publish(
                                            PublishedEvent {
                                                event: Arc::new(stored.event),
                                                ingest_seq: Some(stored.ingest_seq),
                                            },
                                            now,
                                        )
                                        .await
                                        .is_err()
                                    {
                                        failed = true;
                                        break;
                                    }
                                }
                                if failed {
                                    fail_process(&notify_current, &notify_shutdown);
                                    break;
                                }
                                durable_cursor = ingest_seq;
                                None
                            }
                            StoreNotification::Ephemeral(event) => Some(PublishedEvent {
                                event: Arc::new(event),
                                ingest_seq: None,
                            }),
                        };
                        if let Some(published) = published
                            && notify_hub.publish(published, now).await.is_err() {
                                fail_process(&notify_current, &notify_shutdown);
                                break;
                            }
                    }
                }
            }
        }));

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
                            fail_process(&self.state.current, &self.shutdown);
                            break;
                        }
                    };
                    let Ok(slot) = Arc::clone(&connection_slots).try_acquire_owned() else {
                        drop(stream);
                        continue;
                    };
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
                "Postgres notification or worker state became non-current".to_owned(),
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
    let ip = effective_ip(&head, peer.ip(), state.config.trust_proxy);
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
        let icon = state.db.workspace_icon().await?;
        let nip11 = wire::nip11_json_with_icon(&state.config, &state.policy, icon.as_deref());
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
    if event.kind == PUSH_LEASE_KIND {
        // This relay deliberately has no external push service. Without an
        // advertised executor key it cannot decrypt and bind a lease, so the
        // NIP-PL handler fails closed instead of persisting unusable state.
        pending.push_back(ok_message(
            &event.id,
            false,
            "restricted: push executor is not configured or advertised",
        ));
        return Ok(());
    }

    if let Some(rejection) = event_key_rate_rejection(context, &event) {
        let reason = match rejection {
            EventKeyRateRejection::Event => "rate-limited: event rate exceeded",
            EventKeyRateRejection::GiftWrapRecipient => GIFT_WRAP_RECIPIENT_RATE_EXCEEDED,
        };
        pending.push_back(ok_message(&event.id, false, reason));
        return Ok(());
    }
    admit_event(context, event, pending, virtual_owner).await
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
                    "restricted: no self, admin, or owner consent path accepts this request",
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
                "duplicate: archive state already current"
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
                "duplicate: visibility state already current"
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
            "invalid: agent observer timestamp is outside the five-minute freshness window",
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
    admit_event(context, event, pending, virtual_owner).await
}

async fn admit_event(
    context: &mut ConnectionContext,
    event: Event,
    pending: &mut VecDeque<String>,
    virtual_owner: Option<String>,
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
    let admission_now = unix_now();
    match context
        .state
        .db
        .admit(event, admission_now, virtual_owner)
        .await
    {
        Ok(outcome) => {
            if matches!(&outcome, AdmissionOutcome::Ephemeral)
                && let Some(event) = ephemeral
                && context
                    .state
                    .hub
                    .publish(
                        PublishedEvent {
                            event,
                            ingest_seq: None,
                        },
                        admission_now,
                    )
                    .await
                    .is_err()
            {
                fail_process(&context.state.current, &context.state.shutdown);
            }
            let (accepted, reason) = admission_response(outcome);
            pending.push_back(ok_message(&event_id, accepted, &reason));
        }
        Err(error) => {
            pending.push_back(ok_message(&event_id, false, &store_error_response(&error)));
        }
    }
    Ok(())
}

async fn handle_req(
    context: &mut ConnectionContext,
    subscription_id: String,
    filters: Vec<Filter>,
    pending: &mut VecDeque<String>,
) -> Result<(), GatewayError> {
    if context.state.config.auth_required
        && !context
            .auth
            .as_ref()
            .is_some_and(AuthState::is_authenticated)
    {
        pending.push_back(closed_message(
            &subscription_id,
            "auth-required: authenticate before subscribing",
        ));
        return Ok(());
    }
    if !context.state.rate.req_from_ip(context.ip) {
        pending.push_back(closed_message(
            &subscription_id,
            "rate-limited: REQ rate exceeded",
        ));
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
    let max_results = context.state.config.limits.max_limit.min(
        (context
            .state
            .config
            .limits
            .send_queue_capacity
            .saturating_sub(1)
            / 2)
        .max(1),
    );
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
            "restricted: count exceeds the configured query bound",
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
            return Some("restricted: gift-wrap reads must be scoped to #p self");
        }
        if kinds.iter().any(|kind| {
            matches!(
                *kind,
                AGENT_OBSERVER_KIND | AGENT_TURN_METRIC_KIND | DM_VISIBILITY_KIND
            )
        }) && !p_scoped
        {
            return Some("restricted: recipient-private reads must be scoped to #p self");
        }
        if kinds
            .iter()
            .any(|kind| matches!(*kind, EVENT_REMINDER_KIND | PUSH_LEASE_KIND))
            && !author_scoped
        {
            return Some("restricted: author-private reads must be scoped to authors self");
        }
        if kinds.contains(&AGENT_ENGRAM_KIND) && !p_scoped && !author_scoped {
            return Some("restricted: engram reads must be scoped to agent author or #p owner");
        }
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EventKeyRateRejection {
    Event,
    GiftWrapRecipient,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum EventPreflightRejection {
    IpRate,
    InvalidCrypto(crate::domain::DomainError),
}

fn event_preflight(
    rate: &RateLimiter,
    ip: std::net::IpAddr,
    event: &Event,
) -> Result<(), EventPreflightRejection> {
    if !rate.event_from_ip(ip) {
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
    if !rate.event_from_pubkey(&event.pubkey)
        || virtual_owner.is_some_and(|owner| !rate.event_from_pubkey(owner))
    {
        return Some(EventKeyRateRejection::Event);
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
                "invalid: event timestamp is outside relay bounds".to_owned(),
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
                "restricted: group membership or administrator role required".to_owned(),
            ),
            AdmissionRejection::GroupClosed => (false, "restricted: group is closed".to_owned()),
            AdmissionRejection::GroupAlreadyMember => {
                (false, "duplicate: already a group member".to_owned())
            }
            AdmissionRejection::GroupUnsupportedKind => (
                false,
                "restricted: event kind is not supported by this group".to_owned(),
            ),
            AdmissionRejection::GroupPreviousUnknown => (
                false,
                "invalid: group previous reference is not in recent relay history".to_owned(),
            ),
            AdmissionRejection::GroupSigningUnavailable => (
                false,
                "error: relay group signing key is unavailable".to_owned(),
            ),
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
        StoreError::QueryCancelled => "error: admission was cancelled".to_owned(),
        _ => "error: storage unavailable".to_owned(),
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

fn fail_process(current: &AtomicBool, shutdown: &watch::Sender<bool>) {
    current.store(false, Ordering::Release);
    let _ = shutdown.send(true);
}

pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use crate::{domain::Filter, gateway::GatewayConfig};

    use super::validate_and_clamp_filters;

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
