//! Portable client. It never reads desktop roots or acquires an engine credential.
use crate::direct::{self, Change};
use crate::{Error, ErrorCode, Result, fail, protocol::*, transport, unix_time};
use base64::{Engine, engine::general_purpose::STANDARD};
use nostr::domain::Event;
use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub struct Client {
    code: ConnectionCode,
    secret: SecretKey,
    policy: RelayPolicy,
    /// The standing relay link every relay read shares while it lasts.
    link: std::sync::Mutex<Option<Arc<transport::Link>>>,
    linking: tokio::sync::Mutex<()>,
    relay_slots: tokio::sync::Semaphore,
    direct: std::sync::Mutex<DirectState>,
    connecting: tokio::sync::Mutex<()>,
    changes: tokio::sync::broadcast::Sender<Change>,
}

/// Relay exchanges one client runs at once.
const RELAY_IN_FLIGHT: usize = 8;
/// How long a relay read waits for its reply.
const RELAY_LIMIT: Duration = Duration::from_secs(8);
/// How long a direct read waits for its reply.
const DIRECT_LIMIT: Duration = Duration::from_secs(8);
/// After a direct connection fails, reads use the relay for this long.
const DIRECT_RETRY: Duration = Duration::from_secs(30);

#[derive(Default)]
struct DirectState {
    address: Option<SocketAddr>,
    connection: Option<Arc<direct::Connection>>,
    retry_at: Option<Instant>,
}

/// Retaining this exact packet permits a bounded retry without a new read ID.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pending {
    pub request: Request,
    pub event: Event,
}

impl Client {
    pub fn new(code: ConnectionCode, secret: SecretKey) -> Result<Self> {
        Self::new_with_policy(code, secret, RelayPolicy::Production)
    }
    pub fn new_with_policy(
        code: ConnectionCode,
        secret: SecretKey,
        policy: RelayPolicy,
    ) -> Result<Self> {
        code.verify(&secret, unix_time()?, policy)?;
        Ok(Self {
            code,
            secret,
            policy,
            link: std::sync::Mutex::new(None),
            linking: tokio::sync::Mutex::new(()),
            relay_slots: tokio::sync::Semaphore::new(RELAY_IN_FLIGHT),
            direct: std::sync::Mutex::new(DirectState::default()),
            connecting: tokio::sync::Mutex::new(()),
            changes: tokio::sync::broadcast::channel(64).0,
        })
    }
    pub fn connection(&self) -> &ConnectionCode {
        &self.code
    }
    /// Read over a direct connection to the host's tailnet listener at
    /// `address` when it answers, and through the relay otherwise; `None`
    /// reads only through the relay.
    pub fn set_direct(&self, address: Option<SocketAddr>) {
        let mut state = lock(&self.direct);
        if state.address != address {
            *state = DirectState {
                address,
                ..DirectState::default()
            };
        }
    }
    /// The route the next read tries first.
    pub fn route(&self) -> Route {
        let state = lock(&self.direct);
        match (state.address, state.retry_at) {
            (Some(_), None) => Route::Direct,
            (Some(_), Some(at)) if at <= Instant::now() => Route::Direct,
            _ => Route::Relay,
        }
    }
    /// Nudges a direct connection receives: read again.
    pub fn changes(&self) -> tokio::sync::broadcast::Receiver<Change> {
        self.changes.subscribe()
    }
    pub fn prepare(&self, query: Query, now: u64) -> Result<Pending> {
        self.prepare_for(query, now, Route::Relay)
    }
    /// A sealed request within `route`'s bounds.
    pub fn prepare_for(&self, query: Query, now: u64, route: Route) -> Result<Pending> {
        self.code.verify(&self.secret, now, self.policy)?;
        query.validate_for(route)?;
        let request = Request {
            v: REQUEST.into(),
            requires: vec![],
            request: random_id(),
            grant: self.code.grant.clone(),
            authorization: self.code.authorization.id.clone(),
            issued_at: now,
            expires_at: self
                .code
                .expires_at
                .min(now.saturating_add(MAX_REQUEST_LIFETIME)),
            query,
        };
        request.validate_for(route)?;
        let event = seal(
            &request,
            REQUEST,
            &self.secret,
            &self.code.host,
            &request.request,
            now,
            request.expires_at,
        )?;
        Ok(Pending { request, event })
    }
    pub async fn observe(&self, query: Query) -> Result<Observation> {
        self.observe_with(move |_| query.clone()).await
    }
    /// Read the query `make` builds for the route it travels: directly when
    /// the host's tailnet listener answers, else through the relay, where a
    /// failed direct read is tried again.
    pub async fn observe_with(&self, make: impl Fn(Route) -> Query) -> Result<Observation> {
        if self.route() == Route::Direct {
            match self.observe_direct(make(Route::Direct)).await {
                Err(error) if error.code == ErrorCode::Transport => self.direct_failed(),
                // No signed reply: read this one through the relay.
                Err(error) if error.message == direct::UNSIGNED => {}
                result => return result,
            }
        }
        let pending = self.prepare(make(Route::Relay), unix_time()?)?;
        self.send(&pending).await
    }
    async fn observe_direct(&self, query: Query) -> Result<Observation> {
        let connection = self.direct_connection().await?;
        let pending = self.prepare_for(query, unix_time()?, Route::Direct)?;
        let (event, payload) = connection.exchange(&pending.event, DIRECT_LIMIT).await?;
        match payload {
            Some(payload) => self.verify_detached(&pending, &event, &payload, unix_time()?),
            None => self.verify_reply_via(&pending, &event, unix_time()?, Route::Direct),
        }
    }
    async fn direct_connection(&self) -> Result<Arc<direct::Connection>> {
        let current = || {
            let state = lock(&self.direct);
            state.connection.clone().filter(|c| c.alive())
        };
        if let Some(connection) = current() {
            return Ok(connection);
        }
        let _one = self.connecting.lock().await;
        if let Some(connection) = current() {
            return Ok(connection);
        }
        let address = lock(&self.direct)
            .address
            .ok_or_else(|| Error::new(ErrorCode::Transport, "no direct address"))?;
        let connection = Arc::new(direct::Connection::open(address, self.changes.clone()).await?);
        let mut state = lock(&self.direct);
        if state.address == Some(address) {
            state.connection = Some(connection.clone());
            state.retry_at = None;
        }
        Ok(connection)
    }
    /// Open, ahead of the next read, the connections it will use: the
    /// direct connection when the host has a direct address, and the relay
    /// link, which is the fallback either way. Call it when the app comes to
    /// the foreground. A failure is left for the read itself to meet.
    pub async fn warm(&self) {
        let direct = async {
            if self.route() == Route::Direct && self.direct_connection().await.is_err() {
                self.direct_failed();
            }
        };
        let relay = async {
            let _ = self.relay_link().await;
        };
        tokio::join!(direct, relay);
    }
    async fn relay_link(&self) -> Result<Arc<transport::Link>> {
        let current = || lock(&self.link).clone().filter(|link| link.reusable());
        if let Some(link) = current() {
            return Ok(link);
        }
        let _one = self.linking.lock().await;
        if let Some(link) = current() {
            return Ok(link);
        }
        let link = Arc::new(
            transport::Link::connect(&self.code.relay, &self.secret, self.policy, &self.code.host)
                .await?,
        );
        *lock(&self.link) = Some(link.clone());
        Ok(link)
    }
    fn direct_failed(&self) {
        let mut state = lock(&self.direct);
        state.connection = None;
        state.retry_at = Some(Instant::now() + DIRECT_RETRY);
    }
    pub async fn send(&self, pending: &Pending) -> Result<Observation> {
        self.check_pending(pending, unix_time()?, Route::Relay)?;
        let _slot = self
            .relay_slots
            .acquire()
            .await
            .map_err(|_| Error::new(ErrorCode::Transport, "client is closing"))?;
        self.check_pending(pending, unix_time()?, Route::Relay)?;
        let response = tokio::time::timeout(RELAY_LIMIT, async {
            let link = self.relay_link().await?;
            let response = link.exchange(pending).await;
            if response
                .as_ref()
                .is_err_and(|e| e.code == ErrorCode::Transport)
            {
                // The next read opens a new link.
                let mut current = lock(&self.link);
                if current.as_ref().is_some_and(|c| Arc::ptr_eq(c, &link)) {
                    *current = None;
                }
            }
            response
        })
        .await
        .map_err(|_| Error::new(ErrorCode::Transport, "observation deadline exceeded"))??;
        self.verify_reply(pending, &response, unix_time()?)
    }
    fn check_pending(&self, pending: &Pending, now: u64, route: Route) -> Result<()> {
        self.code.verify(&self.secret, now, self.policy)?;
        pending.request.validate_for(route)?;
        fresh(pending.request.issued_at, pending.request.expires_at, now)?;
        let original: Request = open(
            &pending.event,
            &self.secret,
            &self.code.client,
            &self.code.host,
            REQUEST,
        )?;
        if encoded(&original)? != encoded(&pending.request)?
            || original.grant != self.code.grant
            || original.authorization != self.code.authorization.id
            || original.expires_at > self.code.expires_at
            || pending.event.tag_values("h").collect::<Vec<_>>() != [original.request.as_str()]
        {
            return fail(
                ErrorCode::Forbidden,
                "pending packet differs from the exact connection request",
            );
        }
        Ok(())
    }
    pub fn verify_reply(&self, pending: &Pending, event: &Event, now: u64) -> Result<Observation> {
        self.verify_reply_via(pending, event, now, Route::Relay)
    }
    /// Verify a reply that travelled `route`: the same checks on either,
    /// within that route's bounds.
    pub fn verify_reply_via(
        &self,
        pending: &Pending,
        event: &Event,
        now: u64,
        route: Route,
    ) -> Result<Observation> {
        self.check_pending(pending, now, route)?;
        let reply: Reply = open_within(
            event,
            &self.secret,
            (&self.code.host, &self.code.client),
            REPLY,
            route.body(),
        )?;
        self.check_reply(pending, event, reply, now, route)
    }
    /// Verify a direct reply whose body travelled beside its sealed envelope
    /// ([`seal_detached`]): the envelope pins the body's digest, and every
    /// check of [`Client::verify_reply_via`] applies.
    pub fn verify_detached(
        &self,
        pending: &Pending,
        event: &Event,
        payload: &str,
        now: u64,
    ) -> Result<Observation> {
        self.check_pending(pending, now, Route::Direct)?;
        let reply: Reply = open_detached(
            event,
            payload,
            &self.secret,
            (&self.code.host, &self.code.client),
            REPLY,
            Route::Direct.body(),
        )?;
        self.check_reply(pending, event, reply, now, Route::Direct)
    }
    fn check_reply(
        &self,
        pending: &Pending,
        event: &Event,
        reply: Reply,
        now: u64,
        route: Route,
    ) -> Result<Observation> {
        schema(&reply.v, REPLY, &reply.requires)?;
        window(reply.issued_at, reply.expires_at, MAX_REQUEST_LIFETIME)?;
        fresh(reply.issued_at, reply.expires_at, now)?;
        if reply.request != pending.request.request
            || reply.request_event != pending.event.id
            || reply.grant != self.code.grant
            || reply.issued_at < pending.request.issued_at
            || reply.expires_at > pending.request.expires_at
            || event.tag_values("h").collect::<Vec<_>>() != [reply.request.as_str()]
        {
            return fail(
                ErrorCode::Forbidden,
                "reply does not bind the exact pending request",
            );
        }
        match reply.result {
            ReplyResult::Refused {
                code: ErrorCode::Transport,
            } => fail(
                ErrorCode::Malformed,
                "remote transport refusal is not a domain result",
            ),
            ReplyResult::Refused { code } => Err(Error::new(code, "host refused this observation")),
            ReplyResult::Ok { observation } => {
                check_observation_within(&pending.request.query, &observation, route.limits())?;
                Ok(*observation)
            }
        }
    }
}

fn lock<T>(mutex: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poison| poison.into_inner())
}

/// Check a page against its request and the bounds of its route.
pub(crate) fn check_observation_within(
    query: &Query,
    observation: &Observation,
    limits: coder_history::Limits,
) -> Result<()> {
    let page_bytes = match observation {
        Observation::Catalog(page) => encoded_within(page, MAX_DIRECT_BODY)?,
        Observation::Page(page) => encoded_within(page, MAX_DIRECT_BODY)?,
    };
    if page_bytes.len() > limits.response_bytes {
        return fail(
            ErrorCode::Bounds,
            "history page exceeds the negotiated bound",
        );
    }
    match (query, observation) {
        (Query::Catalog(request), Observation::Catalog(page)) => {
            if page.entries.len() > usize::from(request.limit) || page.notices.len() > 256 {
                return fail(
                    ErrorCode::Bounds,
                    "catalog page exceeds the requested bound",
                );
            }
        }
        (Query::Page(request), Observation::Page(page)) => {
            if page.source_id != request.source_id
                || page.next.source_id != request.source_id
                || page.next.incarnation != page.incarnation
                || page.next.offset > page.snapshot_bytes
                || page.next.record_offset > page.next.offset
                || page.chunks.len() > limits.chunks
            {
                return fail(
                    ErrorCode::Malformed,
                    "transcript page identity or progress differs",
                );
            }
            // A backward read starts at the first whole record it returns,
            // or, when it returns none, where its cursor points.
            let backward = request.end.map(|end| {
                let first = page.chunks.first();
                (
                    end,
                    first.map_or(page.next.offset, |c| c.offset),
                    first.map_or(page.next.record_index, |c| c.index),
                )
            });
            if let Some((end, start, _)) = backward
                && (request.cursor.is_some()
                    || page
                        .chunks
                        .first()
                        .is_some_and(|c| c.offset != c.record_offset)
                    || page.chunks.last().is_some_and(|c| !c.complete)
                    || page.previous != (start > 0).then_some(start)
                    || page.next.offset > end)
            {
                return fail(
                    ErrorCode::Malformed,
                    "backward transcript page is not whole records before its end",
                );
            }
            if backward.is_none() && page.previous.is_some() {
                return fail(
                    ErrorCode::Malformed,
                    "a forward transcript page names an earlier page",
                );
            }
            let mut offset = match backward {
                Some((_, start, _)) => start,
                None => request.cursor.as_ref().map_or(0, |c| c.offset),
            };
            let mut record_offset = match backward {
                Some((_, start, _)) => start,
                None => request.cursor.as_ref().map_or(0, |c| c.record_offset),
            };
            let mut record_index = match backward {
                Some((_, _, index)) => index,
                None => request.cursor.as_ref().map_or(0, |c| c.record_index),
            };
            let start = offset;
            if request
                .cursor
                .as_ref()
                .is_some_and(|c| c.incarnation != page.incarnation)
            {
                return fail(ErrorCode::SourceChanged, "source incarnation changed");
            }
            for chunk in &page.chunks {
                if chunk.offset != offset
                    || chunk.end_offset <= chunk.offset
                    || chunk.end_offset > page.snapshot_bytes
                    || chunk.record_offset != record_offset
                    || chunk.index != record_index
                    || chunk.id
                        != coder_history::record_id(
                            &page.source_id,
                            &page.incarnation,
                            record_offset,
                        )
                    || chunk.raw_base64.len() > 4 * coder_history::MAX_CHUNK_BYTES / 3 + 4
                {
                    return fail(
                        ErrorCode::Malformed,
                        "transcript chunks are discontinuous or exceed their source cut",
                    );
                }
                let raw = STANDARD.decode(&chunk.raw_base64).map_err(|_| {
                    Error::new(
                        ErrorCode::Malformed,
                        "transcript chunk is not canonical base64",
                    )
                })?;
                if raw.len() as u64 != chunk.end_offset - chunk.offset
                    || raw.len() > coder_history::MAX_CHUNK_BYTES
                    || STANDARD.encode(&raw) != chunk.raw_base64
                    || chunk.complete != raw.ends_with(b"\n")
                    || raw[..raw.len().saturating_sub(1)].contains(&b'\n')
                {
                    return fail(
                        ErrorCode::Malformed,
                        "transcript bytes differ from their bounds or record completion",
                    );
                }
                offset = chunk.end_offset;
                if chunk.complete {
                    record_offset = offset;
                    record_index = record_index.checked_add(1).ok_or_else(|| {
                        Error::new(
                            ErrorCode::Bounds,
                            "transcript record index exceeds its bound",
                        )
                    })?;
                }
            }
            if page.next.offset != offset
                || page.next.record_offset != record_offset
                || page.next.record_index != record_index
                || page.pending_line != (record_offset < offset)
                || page.has_more != (offset < page.snapshot_bytes)
                || offset.saturating_sub(start) > u64::from(request.max_bytes)
            {
                return fail(
                    ErrorCode::Bounds,
                    "transcript cursor advances beyond returned bounded bytes",
                );
            }
        }
        _ => {
            return fail(
                ErrorCode::Malformed,
                "reply variant differs from requested observation",
            );
        }
    }
    Ok(())
}
