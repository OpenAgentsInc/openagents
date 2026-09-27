//! The push gateway service.
//!
//! Two listeners with separate route sets:
//!
//! - The **delivery** listener serves `POST /v1/deliveries/apns` and
//!   `POST /v1/deliveries/fcm` for the relay's PL executor. The NIP-98
//!   signer must be a configured relay key. Keep it on loopback or a private
//!   link; the relay speaks plain HTTP to it.
//! - The **registration** listener serves the installation and delegation
//!   routes for devices, behind a TLS proxy. The NIP-98 signer is the
//!   installation owner, the same key that authors the device's lease.
//!
//! A delivery resolves its opaque grant to a held native token, then sends
//! the registered wake constant. Outcomes are idempotent by
//! `(relay_pubkey, request_id)`: a terminal outcome is recorded and replayed
//! without a second send; a transient outcome is released so the relay's
//! bounded retry reaches the provider again.

pub mod apns;
pub mod config;
pub mod fcm;
pub mod jwt;
pub mod store;

use std::{
    collections::{BTreeMap, HashMap},
    net::SocketAddr,
    sync::{Arc, Mutex as StdMutex},
};

use axum::{
    Router,
    body::Bytes,
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, Method, StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde_json::{Value, json};
use tokio::sync::{Mutex, watch};

use crate::{
    nip98,
    wire::{
        self, DelegationRequest, DelegationResponse, DelegationRevokeRequest, DeliveryRequest,
        InstallationRequest, InstallationResponse, InstallationRevokeRequest, RotateRequest,
        Transport,
    },
};
pub use config::{ApnsConfig, Config, FcmConfig, Limits, Secret};
use store::{Delegation, Finished, Installation, Store};

/// How far ahead a delegation's `not_before` may be, in seconds.
const NOT_BEFORE_SKEW: u64 = 300;
/// How long an in-flight reservation blocks a duplicate, in seconds.
const IN_FLIGHT_SECONDS: u64 = 60;
/// Extra retention for a finished outcome past its request expiry.
const FINISHED_MARGIN_SECONDS: u64 = 3_600;
/// Delay suggested to a relay whose duplicate is still in flight.
const IN_FLIGHT_RETRY_SECONDS: u64 = 5;
const GRANT_PREFIX: &str = "pg1_";

/// A provider's classification of one send.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The provider accepted the wake.
    Accepted,
    /// The token is permanently invalid.
    InvalidEndpoint {
        /// When the provider says it became invalid, in Unix seconds.
        invalid_at: Option<u64>,
    },
    /// A transient failure; retry, no sooner than `after` seconds if given.
    Retry {
        /// Provider-requested delay.
        after: Option<u64>,
    },
    /// Credentials or topic are wrong; retrying the same request may work
    /// once an operator fixes them.
    ConfigurationFault,
    /// The provider refused the request permanently.
    RequestFault,
}

/// An HTTP answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    /// Status code.
    pub status: u16,
    /// JSON body.
    pub body: Value,
}

impl Reply {
    fn new(status: u16, body: Value) -> Self {
        Self { status, body }
    }
    fn error(status: u16, error: &str) -> Self {
        Self::new(status, json!({ "error": error }))
    }
}

/// Which listener a request arrived on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Listener {
    /// The relay's private delivery listener.
    Delivery,
    /// The device registration listener.
    Registration,
}

/// The gateway's state and senders.
pub struct Service {
    relay_pubkeys: Vec<String>,
    limits: Limits,
    profiles: BTreeMap<String, Transport>,
    store: Mutex<Store>,
    burned: StdMutex<HashMap<String, u64>>,
    in_flight: StdMutex<HashMap<String, u64>>,
    apns: Option<apns::Apns>,
    fcm: Option<fcm::Fcm>,
}

impl Service {
    /// Build the service: open the state and load the provider credentials.
    ///
    /// # Errors
    ///
    /// Returns a reason when the state or a credential cannot be loaded.
    pub fn new(config: &Config) -> Result<Self, String> {
        let mut profiles = BTreeMap::new();
        let apns = match &config.apns {
            Some(apns_config) => {
                profiles.insert(apns_config.app_profile.clone(), Transport::Apns);
                Some(apns::Apns::new(
                    apns_config,
                    config.limits.max_wake_expiration_seconds,
                )?)
            }
            None => None,
        };
        let fcm = match &config.fcm {
            Some(fcm_config) => {
                profiles.insert(fcm_config.app_profile.clone(), Transport::Fcm);
                Some(fcm::Fcm::new(
                    fcm_config,
                    config.limits.max_wake_expiration_seconds,
                )?)
            }
            None => None,
        };
        Ok(Self {
            relay_pubkeys: config.relay_pubkeys.clone(),
            limits: config.limits,
            profiles,
            store: Mutex::new(Store::open(&config.state_dir, &config.state_key)?),
            burned: StdMutex::new(HashMap::new()),
            in_flight: StdMutex::new(HashMap::new()),
            apns,
            fcm,
        })
    }

    /// Answer one `POST` on `listener`.
    pub async fn handle(
        &self,
        listener: Listener,
        path: &str,
        authorization: Option<&str>,
        body: &[u8],
        now: u64,
    ) -> Reply {
        match (listener, path) {
            (Listener::Delivery, wire::APNS_DELIVERY_PATH) => {
                self.deliver(Transport::Apns, path, authorization, body, now)
                    .await
            }
            (Listener::Delivery, wire::FCM_DELIVERY_PATH) => {
                self.deliver(Transport::Fcm, path, authorization, body, now)
                    .await
            }
            (
                Listener::Registration,
                wire::INSTALLATIONS_PATH
                | wire::ROTATE_PATH
                | wire::INSTALLATION_REVOKE_PATH
                | wire::DELEGATIONS_PATH
                | wire::DELEGATION_REVOKE_PATH,
            ) => self.register(path, authorization, body, now).await,
            _ => Reply::error(404, "not_found"),
        }
    }

    /// Verify NIP-98 and burn its event ID. `None` is `401 invalid_auth`.
    fn authorize(
        &self,
        authorization: Option<&str>,
        path: &str,
        body: &[u8],
        now: u64,
    ) -> Option<nip98::Verified> {
        let verified = nip98::verify(authorization?, path, body, now).ok()?;
        self.burn(&verified, now).then_some(verified)
    }

    /// Burn an event ID; false when it was already used.
    fn burn(&self, verified: &nip98::Verified, now: u64) -> bool {
        let mut burned = self
            .burned
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        burned.retain(|_, until| *until > now);
        if burned.contains_key(&verified.event_id) {
            return false;
        }
        burned.insert(
            verified.event_id.clone(),
            verified.created_at + nip98::WINDOW_SECONDS + 1,
        );
        true
    }

    async fn deliver(
        &self,
        transport: Transport,
        path: &str,
        authorization: Option<&str>,
        body: &[u8],
        now: u64,
    ) -> Reply {
        let Some(auth) =
            authorization.and_then(|header| nip98::verify(header, path, body, now).ok())
        else {
            return Reply::error(401, "invalid_auth");
        };
        if !self.relay_pubkeys.contains(&auth.pubkey) {
            return Reply::error(401, "invalid_auth");
        }
        let Ok(request) = wire::parse_closed::<DeliveryRequest>(body) else {
            return Reply::error(400, "invalid_request");
        };
        if !wire::valid_uuid(&request.request_id)
            || request.endpoint_grant.is_empty()
            || request.endpoint_grant.len() > wire::MAX_GRANT_BYTES
        {
            return Reply::error(400, "invalid_request");
        }
        let reservation = format!("{}:{}", auth.pubkey, request.request_id);

        let mut store = self.store.lock().await;
        // A finished request replays its outcome without a send, even when
        // the relay's signer produced the same authorization again.
        if let Some(finished) = store.data.finished.get(&reservation) {
            return Reply::new(finished.status, finished.body.clone());
        }
        // Otherwise every authorization admits at most one attempt.
        if !self.burn(&auth, now) {
            return Reply::error(404, "invalid_grant");
        }
        {
            let mut in_flight = self
                .in_flight
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            in_flight.retain(|_, since| now.saturating_sub(*since) < IN_FLIGHT_SECONDS);
            if in_flight.contains_key(&reservation) {
                return Reply::new(
                    503,
                    json!({"status": "retry", "retry_after_seconds": IN_FLIGHT_RETRY_SECONDS}),
                );
            }
        }
        let grant_digest = store.digest(&["grant", &request.endpoint_grant]);
        let Some(delegation) = store.data.delegations.get(&grant_digest).cloned() else {
            return Reply::error(404, "invalid_grant");
        };
        let Some(installation) = store
            .data
            .installations
            .get(&delegation.installation_handle)
            .cloned()
        else {
            return Reply::error(404, "invalid_grant");
        };
        if delegation.relay_pubkey != auth.pubkey
            || installation.transport != transport
            || !installation.live(now)
            || installation.endpoint_epoch != delegation.endpoint_epoch
            || installation.generations.get(&auth.pubkey) != Some(&delegation.generation)
            || delegation.not_before > now
            || request.expires_at < now
            || request.expires_at > delegation.expires_at
        {
            return Reply::error(404, "invalid_grant");
        }
        let retain_until = request.expires_at.max(now) + FINISHED_MARGIN_SECONDS;
        if let Some(invalid_at) = installation.invalid_at {
            let reply = invalid_endpoint(delegation.generation, Some(invalid_at));
            return finish(&mut store, reservation, reply, retain_until);
        }
        let Some(token) = store.unseal(&delegation.installation_handle, &installation.token_sealed)
        else {
            return Reply::error(503, "temporarily_unavailable");
        };
        {
            let Some(entry) = store
                .data
                .installations
                .get_mut(&delegation.installation_handle)
            else {
                return Reply::error(404, "invalid_grant");
            };
            if now.saturating_sub(entry.window_start) >= 3_600 {
                entry.window_start = now;
                entry.window_count = 0;
            }
            if entry.window_count >= self.limits.wakes_per_hour {
                return Reply::error(429, "rate_limited");
            }
            // Charged once per admitted attempt and never refunded.
            entry.window_count += 1;
        }
        self.in_flight
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(reservation.clone(), now);
        drop(store);

        let outcome = match (transport, &self.apns, &self.fcm) {
            (Transport::Apns, Some(apns), _) => {
                apns.send(&token, &request.request_id, request.expires_at, now)
                    .await
            }
            (Transport::Fcm, _, Some(fcm)) => fcm.send(&token, request.expires_at, now).await,
            _ => Outcome::ConfigurationFault,
        };
        drop(token);

        let mut store = self.store.lock().await;
        self.in_flight
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&reservation);
        log_outcome(&request.request_id, transport, outcome);
        match outcome {
            Outcome::Accepted => finish(
                &mut store,
                reservation,
                Reply::new(200, json!({"status": "accepted"})),
                retain_until,
            ),
            Outcome::InvalidEndpoint { invalid_at } => {
                let marked = invalid_at.unwrap_or(now);
                if let Some(entry) = store
                    .data
                    .installations
                    .get_mut(&delegation.installation_handle)
                    && entry.endpoint_epoch == delegation.endpoint_epoch
                {
                    entry.invalid_at = Some(marked);
                }
                finish(
                    &mut store,
                    reservation,
                    invalid_endpoint(delegation.generation, invalid_at),
                    retain_until,
                )
            }
            Outcome::RequestFault => finish(
                &mut store,
                reservation,
                Reply::error(400, "invalid_request"),
                retain_until,
            ),
            Outcome::Retry { after } => {
                let _ = store.save();
                Reply::new(
                    503,
                    json!({"status": "retry", "retry_after_seconds": after.filter(|after| *after > 0)}),
                )
            }
            Outcome::ConfigurationFault => {
                let _ = store.save();
                Reply::error(503, "configuration_fault")
            }
        }
    }

    async fn register(
        &self,
        path: &str,
        authorization: Option<&str>,
        body: &[u8],
        now: u64,
    ) -> Reply {
        let Some(auth) = self.authorize(authorization, path, body, now) else {
            return Reply::error(401, "invalid_auth");
        };
        let owner = auth.pubkey;
        let mut store = self.store.lock().await;
        store.prune(now);
        let reply = match path {
            wire::INSTALLATIONS_PATH => match wire::parse_closed(body) {
                Ok(request) => self.create_installation(&mut store, &owner, request, now),
                Err(_) => Reply::error(400, "invalid_request"),
            },
            wire::ROTATE_PATH => match wire::parse_closed(body) {
                Ok(request) => rotate(&mut store, &owner, request, now),
                Err(_) => Reply::error(400, "invalid_request"),
            },
            wire::INSTALLATION_REVOKE_PATH => match wire::parse_closed(body) {
                Ok(request) => revoke_installation(&mut store, &owner, &request, now),
                Err(_) => Reply::error(400, "invalid_request"),
            },
            wire::DELEGATIONS_PATH => match wire::parse_closed(body) {
                Ok(request) => self.delegate(&mut store, &owner, &request, now),
                Err(_) => Reply::error(400, "invalid_request"),
            },
            wire::DELEGATION_REVOKE_PATH => match wire::parse_closed(body) {
                Ok(request) => revoke_delegation(&mut store, &owner, &request, now),
                Err(_) => Reply::error(400, "invalid_request"),
            },
            _ => Reply::error(404, "not_found"),
        };
        if reply.status < 300 && store.save().is_err() {
            return Reply::error(503, "temporarily_unavailable");
        }
        reply
    }

    fn create_installation(
        &self,
        store: &mut Store,
        owner: &str,
        request: InstallationRequest,
        now: u64,
    ) -> Reply {
        let Some(transport) = self.profiles.get(&request.app_profile).copied() else {
            return Reply::error(400, "invalid_request");
        };
        if !transport.valid_token(&request.endpoint)
            || request.expires_at <= now
            || request.expires_at > now + self.limits.max_installation_seconds
        {
            return Reply::error(400, "invalid_request");
        }
        let digest = store.digest(&["token", &request.app_profile, &request.endpoint]);
        let existing = store
            .data
            .installations
            .iter()
            .find(|(_, installation)| installation.live(now) && installation.token_digest == digest)
            .map(|(handle, installation)| (handle.clone(), installation.owner.clone()));
        if let Some((handle, holder)) = existing {
            if holder != owner {
                return Reply::error(409, "installation_conflict");
            }
            // The same owner registering the same token again recovers its
            // installation.
            let Some(installation) = store.data.installations.get_mut(&handle) else {
                return Reply::error(503, "temporarily_unavailable");
            };
            installation.expires_at = installation.expires_at.max(request.expires_at);
            return Reply::new(
                201,
                json!(InstallationResponse {
                    installation_handle: handle,
                    endpoint_epoch: installation.endpoint_epoch,
                    expires_at: installation.expires_at,
                }),
            );
        }
        let owned = store
            .data
            .installations
            .values()
            .filter(|installation| installation.owner == owner && installation.live(now))
            .count();
        if owned >= self.limits.max_installations_per_owner {
            return Reply::error(429, "rate_limited");
        }
        let handle = wire::random_uuid();
        let Ok(token_sealed) = store.seal(&handle, &request.endpoint) else {
            return Reply::error(503, "temporarily_unavailable");
        };
        store.data.installations.insert(
            handle.clone(),
            Installation {
                owner: owner.to_owned(),
                app_profile: request.app_profile,
                transport,
                endpoint_epoch: 1,
                token_sealed,
                token_digest: digest,
                expires_at: request.expires_at,
                revoked: false,
                invalid_at: None,
                generations: BTreeMap::new(),
                window_start: now,
                window_count: 0,
            },
        );
        Reply::new(
            201,
            json!(InstallationResponse {
                installation_handle: handle,
                endpoint_epoch: 1,
                expires_at: request.expires_at,
            }),
        )
    }

    fn delegate(
        &self,
        store: &mut Store,
        owner: &str,
        request: &DelegationRequest,
        now: u64,
    ) -> Reply {
        if !wire::valid_pubkey(&request.relay_pubkey)
            || !self.relay_pubkeys.contains(&request.relay_pubkey)
            || request.generation == 0
            || request.not_before > now + NOT_BEFORE_SKEW
            || request.not_before >= request.expires_at
            || request.expires_at <= now
            || request.expires_at > now + self.limits.max_grant_seconds
        {
            return Reply::error(400, "invalid_request");
        }
        let Some(installation) = owned(store, owner, &request.installation_handle, now) else {
            return Reply::error(404, "not_authorized");
        };
        if installation.endpoint_epoch != request.endpoint_epoch {
            return Reply::error(404, "not_authorized");
        }
        if installation
            .generations
            .get(&request.relay_pubkey)
            .is_some_and(|last| *last >= request.generation)
        {
            return Reply::error(400, "invalid_request");
        }
        let mut secret = [0_u8; 32];
        if store.random(&mut secret).is_err() {
            return Reply::error(503, "temporarily_unavailable");
        }
        let grant = format!("{GRANT_PREFIX}{}", URL_SAFE_NO_PAD.encode(secret));
        let digest = store.digest(&["grant", &grant]);
        // A newer delegation supersedes the older capability for this relay.
        store.data.delegations.retain(|_, delegation| {
            delegation.installation_handle != request.installation_handle
                || delegation.relay_pubkey != request.relay_pubkey
        });
        store.data.delegations.insert(
            digest,
            Delegation {
                installation_handle: request.installation_handle.clone(),
                relay_pubkey: request.relay_pubkey.clone(),
                generation: request.generation,
                endpoint_epoch: request.endpoint_epoch,
                not_before: request.not_before,
                expires_at: request.expires_at,
            },
        );
        if let Some(installation) = store
            .data
            .installations
            .get_mut(&request.installation_handle)
        {
            installation
                .generations
                .insert(request.relay_pubkey.clone(), request.generation);
            installation.expires_at = installation.expires_at.max(request.expires_at);
        }
        Reply::new(
            201,
            json!(DelegationResponse {
                endpoint_grant: grant
            }),
        )
    }
}

fn owned<'a>(store: &'a Store, owner: &str, handle: &str, now: u64) -> Option<&'a Installation> {
    store
        .data
        .installations
        .get(handle)
        .filter(|installation| installation.owner == owner && installation.live(now))
}

fn rotate(store: &mut Store, owner: &str, request: RotateRequest, now: u64) -> Reply {
    let Some(installation) = owned(store, owner, &request.installation_handle, now) else {
        return Reply::error(404, "not_authorized");
    };
    if request.endpoint_epoch != installation.endpoint_epoch
        || request.endpoint_epoch.checked_add(1) != Some(request.new_endpoint_epoch)
        || !installation.transport.valid_token(&request.endpoint)
    {
        return Reply::error(400, "invalid_request");
    }
    let digest = store.digest(&["token", &installation.app_profile, &request.endpoint]);
    if store.data.installations.iter().any(|(handle, other)| {
        handle != &request.installation_handle && other.live(now) && other.token_digest == digest
    }) {
        return Reply::error(409, "installation_conflict");
    }
    let Ok(sealed) = store.seal(&request.installation_handle, &request.endpoint) else {
        return Reply::error(503, "temporarily_unavailable");
    };
    let handle = request.installation_handle.clone();
    // Every capability sealed to the old epoch ends here.
    store
        .data
        .delegations
        .retain(|_, delegation| delegation.installation_handle != handle);
    let Some(installation) = store.data.installations.get_mut(&handle) else {
        return Reply::error(404, "not_authorized");
    };
    installation.token_sealed = sealed;
    installation.token_digest = digest;
    installation.endpoint_epoch = request.new_endpoint_epoch;
    installation.invalid_at = None;
    Reply::new(
        200,
        json!({"status": "rotated", "endpoint_epoch": request.new_endpoint_epoch}),
    )
}

fn revoke_installation(
    store: &mut Store,
    owner: &str,
    request: &InstallationRevokeRequest,
    now: u64,
) -> Reply {
    let Some(installation) = owned(store, owner, &request.installation_handle, now) else {
        return Reply::error(404, "not_authorized");
    };
    if installation.endpoint_epoch != request.endpoint_epoch {
        return Reply::error(404, "not_authorized");
    }
    let handle = request.installation_handle.clone();
    store
        .data
        .delegations
        .retain(|_, delegation| delegation.installation_handle != handle);
    if let Some(installation) = store.data.installations.get_mut(&handle) {
        installation.revoked = true;
        installation.token_sealed.clear();
    }
    Reply::new(200, json!({"status": "revoked"}))
}

fn revoke_delegation(
    store: &mut Store,
    owner: &str,
    request: &DelegationRevokeRequest,
    now: u64,
) -> Reply {
    let Some(installation) = owned(store, owner, &request.installation_handle, now) else {
        return Reply::error(404, "not_authorized");
    };
    if installation.generations.get(&request.relay_pubkey) != Some(&request.generation) {
        return Reply::error(404, "not_authorized");
    }
    store.data.delegations.retain(|_, delegation| {
        delegation.installation_handle != request.installation_handle
            || delegation.relay_pubkey != request.relay_pubkey
    });
    Reply::new(200, json!({"status": "revoked"}))
}

fn invalid_endpoint(generation: u64, invalid_at: Option<u64>) -> Reply {
    Reply::new(
        410,
        json!({"status": "invalid_endpoint", "generation": generation, "invalid_at": invalid_at}),
    )
}

/// Record a terminal outcome for idempotent replay and persist it.
fn finish(store: &mut Store, reservation: String, reply: Reply, retain_until: u64) -> Reply {
    store.data.finished.insert(
        reservation,
        Finished {
            status: reply.status,
            body: reply.body.clone(),
            retain_until,
        },
    );
    if store.save().is_err() {
        return Reply::error(503, "temporarily_unavailable");
    }
    reply
}

fn log_outcome(request_id: &str, transport: Transport, outcome: Outcome) {
    let label = match outcome {
        Outcome::Accepted => "accepted",
        Outcome::InvalidEndpoint { .. } => "invalid_endpoint",
        Outcome::Retry { .. } => "retry",
        Outcome::ConfigurationFault => "configuration_fault",
        Outcome::RequestFault => "request_fault",
    };
    eprintln!(
        "push-gateway: delivery request_id={request_id} transport={} outcome={label}",
        transport.as_str()
    );
}

/// A running gateway.
pub struct Running {
    /// Bound delivery address.
    pub delivery_addr: SocketAddr,
    /// Bound registration address.
    pub registration_addr: SocketAddr,
    shutdown: watch::Sender<bool>,
    tasks: Vec<tokio::task::JoinHandle<()>>,
}

impl Running {
    /// Stop both listeners and wait for them.
    pub async fn stop(self) {
        let _ = self.shutdown.send(true);
        for task in self.tasks {
            let _ = task.await;
        }
    }
}

/// Bind both listeners and serve until [`Running::stop`].
///
/// # Errors
///
/// Returns a reason when the service cannot load or a listener cannot bind.
pub async fn start(config: Config) -> Result<Running, String> {
    let service = Arc::new(Service::new(&config)?);
    let (shutdown, _) = watch::channel(false);
    let mut tasks = Vec::new();
    let mut addresses = Vec::new();
    for (listener, address) in [
        (Listener::Delivery, config.delivery_addr),
        (Listener::Registration, config.registration_addr),
    ] {
        let socket = tokio::net::TcpListener::bind(address)
            .await
            .map_err(|error| format!("cannot bind {address}: {error}"))?;
        addresses.push(
            socket
                .local_addr()
                .map_err(|error| format!("cannot read the bound address: {error}"))?,
        );
        let router = Router::new()
            .fallback(route)
            .layer(DefaultBodyLimit::max(wire::MAX_BODY_BYTES))
            .with_state((Arc::clone(&service), listener));
        let mut stop = shutdown.subscribe();
        tasks.push(tokio::spawn(async move {
            let _ = axum::serve(socket, router)
                .with_graceful_shutdown(async move {
                    let _ = stop.wait_for(|stopped| *stopped).await;
                })
                .await;
        }));
    }
    Ok(Running {
        delivery_addr: addresses[0],
        registration_addr: addresses[1],
        shutdown,
        tasks,
    })
}

async fn route(
    State((service, listener)): State<(Arc<Service>, Listener)>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let reply = if method != Method::POST {
        Reply::error(405, "invalid_request")
    } else if uri.query().is_some()
        || !headers
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| {
                value
                    .split(';')
                    .next()
                    .is_some_and(|kind| kind.trim().eq_ignore_ascii_case("application/json"))
            })
    {
        Reply::error(400, "invalid_request")
    } else {
        let authorization = headers
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok());
        service
            .handle(
                listener,
                uri.path(),
                authorization,
                &body,
                crate::unix_now(),
            )
            .await
    };
    let status = StatusCode::from_u16(reply.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (status, axum::Json(reply.body)).into_response()
}
