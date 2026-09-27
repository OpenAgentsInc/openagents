//! The device side of NIP-PL wakes.
//!
//! A device does four things, all with its own Nostr key:
//!
//! 1. Registers its native token (APNs device token or FCM registration
//!    token) with the gateway, which returns an installation handle.
//! 2. Asks the gateway for a delivery capability (`endpoint_grant`) that only
//!    the relay's signing key can present.
//! 3. Publishes a kind 30350 lease to the relay whose encrypted `endpoint`
//!    is that capability, never the native token.
//! 4. Renews the lease before it expires, rotates the token when the
//!    platform changes it, and revokes with a higher-generation tombstone.
//!
//! [`Enrollment`] runs these steps from persisted state; the lower-level
//! functions are public for callers that manage state themselves.

use std::time::Duration;

use nostr::{
    domain::{Event, RelaySigner, Tag},
    nip44::{conversation_key, encrypt},
};
use secp256k1::{Keypair, Secp256k1, SecretKey, XOnlyPublicKey, rand::RngCore as _};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

use crate::wire::{
    self, DelegationRequest, DelegationResponse, DelegationRevokeRequest, InstallationRequest,
    InstallationResponse, InstallationRevokeRequest, RotateRequest, Transport,
};

/// The push lease kind.
pub const LEASE_KIND: u16 = 30_350;
/// Lease lifetime a device asks for, capped by the relay's `max_lease_ttl`.
pub const DEFAULT_LEASE_SECONDS: u64 = 7 * 86_400;
/// Installation lifetime a device asks for.
pub const DEFAULT_INSTALLATION_SECONDS: u64 = 60 * 86_400;
/// Clock skew the relay allows around `expiration`.
const SKEW_SECONDS: u64 = nostr::push_lease::ALLOWED_SKEW;
const RELAY_OPERATION_SECONDS: u64 = 20;

/// A client failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientError {
    /// The gateway answered with a refusal.
    Refused {
        /// HTTP status.
        status: u16,
        /// The closed error or status value.
        error: String,
    },
    /// The relay refused the lease with this `OK` message.
    RelayRefused(String),
    /// The network or a peer failed.
    Transport(String),
    /// A peer's answer or local input is not valid.
    Invalid(String),
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Refused { status, error } => {
                write!(
                    formatter,
                    "the push gateway refused the request ({status} {error})"
                )
            }
            Self::RelayRefused(message) => {
                write!(formatter, "the relay refused the lease: {message}")
            }
            Self::Transport(message) => {
                write!(formatter, "push setup could not connect: {message}")
            }
            Self::Invalid(message) => write!(formatter, "push setup failed: {message}"),
        }
    }
}

impl std::error::Error for ClientError {}

type Result<T> = std::result::Result<T, ClientError>;

/// A connection to the gateway's registration routes.
#[derive(Clone)]
pub struct GatewayClient {
    base: String,
    http: reqwest::Client,
}

impl GatewayClient {
    /// A client for the registration base URL, such as
    /// `https://push.example.com`.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError::Invalid`] for a URL that is not `http(s)://`
    /// or has a query, fragment, or credentials.
    pub fn new(base_url: &str) -> Result<Self> {
        let base = base_url.trim_end_matches('/');
        if !(base.starts_with("https://") || base.starts_with("http://"))
            || base.contains(['?', '#', '@'])
        {
            return Err(ClientError::Invalid(
                "the push gateway URL must be an http(s) URL without a query or credentials".into(),
            ));
        }
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(20))
            .build()
            .map_err(|error| ClientError::Transport(error.to_string()))?;
        Ok(Self {
            base: base.to_owned(),
            http,
        })
    }

    async fn post<T: DeserializeOwned>(
        &self,
        secret: &SecretKey,
        path: &str,
        body: &impl Serialize,
    ) -> Result<T> {
        let url = format!("{}{path}", self.base);
        let body =
            serde_json::to_vec(body).map_err(|error| ClientError::Invalid(error.to_string()))?;
        let authorization = crate::nip98::sign(secret, &url, &body, crate::unix_now())
            .map_err(ClientError::Invalid)?;
        let response = self
            .http
            .post(&url)
            .header("content-type", "application/json")
            .header("authorization", authorization)
            .body(body)
            .send()
            .await
            .map_err(|error| ClientError::Transport(error.to_string()))?;
        let status = response.status().as_u16();
        let value: Value = response.json().await.unwrap_or_default();
        if !(200..300).contains(&status) {
            let error = value["error"]
                .as_str()
                .or_else(|| value["status"].as_str())
                .unwrap_or("unexpected_response")
                .to_owned();
            return Err(ClientError::Refused { status, error });
        }
        serde_json::from_value(value).map_err(|_| {
            ClientError::Invalid("the push gateway answer is not the expected shape".into())
        })
    }

    /// Register a native token.
    ///
    /// # Errors
    ///
    /// Returns the gateway's refusal or a transport failure.
    pub async fn register(
        &self,
        secret: &SecretKey,
        app_profile: &str,
        token: &str,
        expires_at: u64,
    ) -> Result<InstallationResponse> {
        self.post(
            secret,
            wire::INSTALLATIONS_PATH,
            &InstallationRequest {
                v: wire::WIRE_VERSION,
                app_profile: app_profile.to_owned(),
                endpoint: token.to_owned(),
                expires_at,
            },
        )
        .await
    }

    /// Replace the native token; returns the new epoch.
    ///
    /// # Errors
    ///
    /// Returns the gateway's refusal or a transport failure.
    pub async fn rotate(
        &self,
        secret: &SecretKey,
        installation_handle: &str,
        endpoint_epoch: u64,
        token: &str,
    ) -> Result<u64> {
        #[derive(Deserialize)]
        struct Rotated {
            endpoint_epoch: u64,
        }
        let rotated: Rotated = self
            .post(
                secret,
                wire::ROTATE_PATH,
                &RotateRequest {
                    v: wire::WIRE_VERSION,
                    installation_handle: installation_handle.to_owned(),
                    endpoint_epoch,
                    new_endpoint_epoch: endpoint_epoch + 1,
                    endpoint: token.to_owned(),
                },
            )
            .await?;
        Ok(rotated.endpoint_epoch)
    }

    /// Obtain a delivery capability for one relay.
    ///
    /// # Errors
    ///
    /// Returns the gateway's refusal or a transport failure.
    pub async fn delegate(
        &self,
        secret: &SecretKey,
        request: &DelegationRequest,
    ) -> Result<String> {
        let response: DelegationResponse =
            self.post(secret, wire::DELEGATIONS_PATH, request).await?;
        Ok(response.endpoint_grant)
    }

    /// End the capability for one relay.
    ///
    /// # Errors
    ///
    /// Returns the gateway's refusal or a transport failure.
    pub async fn revoke_delegation(
        &self,
        secret: &SecretKey,
        request: &DelegationRevokeRequest,
    ) -> Result<()> {
        let _: Value = self
            .post(secret, wire::DELEGATION_REVOKE_PATH, request)
            .await?;
        Ok(())
    }

    /// Forget the token and every capability.
    ///
    /// # Errors
    ///
    /// Returns the gateway's refusal or a transport failure.
    pub async fn revoke_installation(
        &self,
        secret: &SecretKey,
        request: &InstallationRevokeRequest,
    ) -> Result<()> {
        let _: Value = self
            .post(secret, wire::INSTALLATION_REVOKE_PATH, request)
            .await?;
        Ok(())
    }
}

/// What a relay advertises for push, read from NIP-11.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelayPush {
    /// The descriptor's canonical origin, copied into the lease verbatim.
    pub origin: String,
    /// The current executor key ID, sent in the `exec` tag.
    pub key_id: String,
    /// The executor encryption key the lease content is encrypted to.
    pub executor_pubkey: String,
    /// The relay signing key that posts deliveries (NIP-11 `self`).
    pub relay_pubkey: String,
    /// Advertised application profiles and their transports.
    pub app_profiles: Vec<(String, String)>,
    /// Kinds a lease may name.
    pub push_kinds: Vec<u64>,
    /// Longest lease lifetime.
    pub max_lease_ttl: u64,
}

impl RelayPush {
    /// The transport the relay advertises for `app_profile`.
    #[must_use]
    pub fn transport(&self, app_profile: &str) -> Option<Transport> {
        self.app_profiles
            .iter()
            .find(|(id, _)| id == app_profile)
            .and_then(|(_, transport)| Transport::parse(transport))
    }
}

/// Parse a NIP-11 document's push support.
///
/// # Errors
///
/// Returns [`ClientError::Invalid`] when the relay does not advertise
/// `nip-pl`, its descriptor is not valid, or it names no signing key.
pub fn parse_relay_push(nip11: &Value) -> Result<RelayPush> {
    let invalid = |reason: &str| ClientError::Invalid(reason.to_owned());
    if !nip11["supported_extensions"]
        .as_array()
        .is_some_and(|extensions| extensions.iter().any(|value| value == "nip-pl"))
    {
        return Err(invalid("the relay does not offer push wakes"));
    }
    let push = &nip11["push"];
    let keys = push["keys"]
        .as_array()
        .ok_or_else(|| invalid("no executor keys"))?;
    let current = keys
        .iter()
        .filter(|key| key["current"] == true)
        .collect::<Vec<_>>();
    let [current] = current.as_slice() else {
        return Err(invalid("the descriptor must mark exactly one current key"));
    };
    let executor_pubkey = current["pubkey"].as_str().unwrap_or_default().to_owned();
    let relay_pubkey = nip11["self"].as_str().unwrap_or_default().to_owned();
    if !wire::valid_pubkey(&executor_pubkey) {
        return Err(invalid("the executor key is not valid"));
    }
    if !wire::valid_pubkey(&relay_pubkey) {
        return Err(invalid("the relay does not publish its signing key"));
    }
    Ok(RelayPush {
        origin: push["origin"]
            .as_str()
            .ok_or_else(|| invalid("no origin"))?
            .to_owned(),
        key_id: current["id"]
            .as_str()
            .ok_or_else(|| invalid("no key ID"))?
            .to_owned(),
        executor_pubkey,
        relay_pubkey,
        app_profiles: push["app_profiles"]
            .as_array()
            .map(|profiles| {
                profiles
                    .iter()
                    .filter_map(|profile| {
                        Some((
                            profile["id"].as_str()?.to_owned(),
                            profile["transport"].as_str()?.to_owned(),
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default(),
        push_kinds: push["push_kinds"]
            .as_array()
            .map(|kinds| kinds.iter().filter_map(Value::as_u64).collect())
            .unwrap_or_default(),
        max_lease_ttl: push["limitation"]["max_lease_ttl"]
            .as_u64()
            .unwrap_or(nostr::push_lease::DEFAULT_MAX_LEASE_TTL),
    })
}

/// Read a relay's NIP-11 document over HTTP(S).
///
/// # Errors
///
/// Returns a transport failure or [`parse_relay_push`]'s refusal.
pub async fn discover(relay_url: &str) -> Result<RelayPush> {
    let http_url = if let Some(rest) = relay_url.strip_prefix("wss://") {
        format!("https://{rest}")
    } else if let Some(rest) = relay_url.strip_prefix("ws://") {
        format!("http://{rest}")
    } else {
        return Err(ClientError::Invalid(
            "the relay URL must be ws:// or wss://".into(),
        ));
    };
    let response = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|error| ClientError::Transport(error.to_string()))?
        .get(http_url)
        .header("accept", "application/nostr+json")
        .send()
        .await
        .map_err(|error| ClientError::Transport(error.to_string()))?;
    let document: Value = response
        .json()
        .await
        .map_err(|_| ClientError::Invalid("the relay information document is not JSON".into()))?;
    parse_relay_push(&document)
}

/// The NIP-WS wake subscription: activity summaries addressed to `device`.
#[must_use]
pub fn summary_subscriptions(device_pubkey: &str) -> Value {
    json!([{ "filter": { "kinds": [3188], "#p": [device_pubkey] }, "class": "default" }])
}

/// What an active lease says.
#[derive(Debug, Clone)]
pub struct LeaseSpec<'a> {
    /// The per-origin installation ID (`d`).
    pub d: &'a str,
    /// Strictly increasing per lease address.
    pub generation: u64,
    /// The application profile.
    pub app_profile: &'a str,
    /// The profile's transport.
    pub transport: Transport,
    /// The gateway capability, carried as the lease `endpoint`.
    pub endpoint: &'a str,
    /// Subscription objects.
    pub subscriptions: &'a Value,
    /// Public expiration.
    pub expiration: u64,
}

/// Build a signed active lease encrypted to the executor.
///
/// # Errors
///
/// Returns [`ClientError::Invalid`] when encryption or signing fails.
pub fn lease_event(
    secret: &SecretKey,
    relay: &RelayPush,
    spec: &LeaseSpec<'_>,
    now: u64,
) -> Result<Event> {
    let plaintext = json!({
        "v": 1,
        "origin": relay.origin,
        "app_profile": spec.app_profile,
        "transport": spec.transport.as_str(),
        "endpoint": spec.endpoint,
        "generation": spec.generation,
        "active": true,
        "subscriptions": spec.subscriptions,
    });
    sealed_lease(secret, relay, spec.d, spec.expiration, &plaintext, now)
}

/// Build a signed revocation tombstone.
///
/// # Errors
///
/// Returns [`ClientError::Invalid`] when encryption or signing fails.
pub fn revocation_event(
    secret: &SecretKey,
    relay: &RelayPush,
    d: &str,
    generation: u64,
    expiration: u64,
    now: u64,
) -> Result<Event> {
    let plaintext =
        json!({ "v": 1, "origin": relay.origin, "generation": generation, "active": false });
    sealed_lease(secret, relay, d, expiration, &plaintext, now)
}

fn sealed_lease(
    secret: &SecretKey,
    relay: &RelayPush,
    d: &str,
    expiration: u64,
    plaintext: &Value,
    now: u64,
) -> Result<Event> {
    let executor = XOnlyPublicKey::from_byte_array(
        decode_hex32(&relay.executor_pubkey)
            .ok_or_else(|| ClientError::Invalid("executor key".into()))?,
    )
    .map_err(|_| ClientError::Invalid("the executor key is not a curve point".into()))?;
    let mut nonce = [0_u8; 32];
    secp256k1::rand::rng().fill_bytes(&mut nonce);
    let content = encrypt(
        &plaintext.to_string(),
        &conversation_key(secret, &executor),
        nonce,
    )
    .map_err(ClientError::Invalid)?;
    let signer = RelaySigner::from_secret_hex(&secret.display_secret().to_string())
        .map_err(|error| ClientError::Invalid(error.to_string()))?;
    Ok(signer.sign(
        now,
        LEASE_KIND,
        vec![
            Tag::new(vec!["d".into(), d.to_owned()]),
            Tag::new(vec!["expiration".into(), expiration.to_string()]),
            Tag::new(vec!["exec".into(), relay.key_id.clone()]),
            Tag::new(vec!["alt".into(), "Push lease".into()]),
        ],
        content,
    ))
}

fn decode_hex32(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 {
        return None;
    }
    let mut out = [0_u8; 32];
    for (index, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(value.get(index * 2..index * 2 + 2)?, 16).ok()?;
    }
    Some(out)
}

/// Publish `event` over an authenticated connection and wait for `OK`.
///
/// # Errors
///
/// Returns [`ClientError::RelayRefused`] with the relay's message, or a
/// transport failure.
pub async fn publish(relay_url: &str, secret: &SecretKey, event: &Event) -> Result<()> {
    let mut connection = nostr_transport::Connection::connect(
        relay_url,
        secret,
        Duration::from_secs(RELAY_OPERATION_SECONDS),
    )
    .await
    .map_err(ClientError::Transport)?;
    connection
        .send(json!(["EVENT", event]))
        .await
        .map_err(ClientError::Transport)?;
    loop {
        let message = connection.next().await.map_err(ClientError::Transport)?;
        if message[0] == "OK" && message[1] == event.id.as_str() {
            let _ = connection.close().await;
            return if message[2] == true {
                Ok(())
            } else {
                Err(ClientError::RelayRefused(
                    message[3].as_str().unwrap_or_default().to_owned(),
                ))
            };
        }
    }
}

/// The gateway installation a device holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeldInstallation {
    /// The installation handle.
    pub handle: String,
    /// Current endpoint epoch.
    pub epoch: u64,
    /// SHA-256 of the native token, to notice rotation without keeping it.
    pub token_digest: String,
    /// When the installation lapses.
    pub expires_at: u64,
}

/// The lease a device last published.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublishedLease {
    /// Its generation.
    pub generation: u64,
    /// Its public expiration.
    pub expiration: u64,
}

/// What [`Enrollment::sync`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncOutcome {
    /// A new lease generation is live.
    Published {
        /// The generation.
        generation: u64,
    },
    /// The live lease does not need renewal yet.
    Current,
}

/// One device's push enrollment with one relay, persisted by the caller.
///
/// Every state change that consumes a generation is saved through the
/// caller's `save` before anything is sent, so a crash never reuses a
/// generation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Enrollment {
    /// The relay WebSocket URL.
    pub relay_url: String,
    /// The gateway registration base URL.
    pub gateway_url: String,
    /// The application profile.
    pub app_profile: String,
    /// The per-origin installation ID (`d`), 128 bits of randomness.
    pub d: String,
    /// Highest generation used for the lease and the delegation.
    pub generation: u64,
    /// Subscriptions the lease carries.
    pub subscriptions: Value,
    /// Requested lease lifetime, capped by the relay.
    pub lease_seconds: u64,
    /// The gateway installation, once registered.
    pub installation: Option<HeldInstallation>,
    /// The live lease, if any.
    pub lease: Option<PublishedLease>,
}

impl Enrollment {
    /// A new enrollment with a fresh random `d` and the NIP-WS summary
    /// subscription for `device_pubkey`.
    #[must_use]
    pub fn new(relay_url: &str, gateway_url: &str, app_profile: &str, device_pubkey: &str) -> Self {
        let mut d = [0_u8; 16];
        secp256k1::rand::rng().fill_bytes(&mut d);
        Self {
            relay_url: relay_url.to_owned(),
            gateway_url: gateway_url.to_owned(),
            app_profile: app_profile.to_owned(),
            d: crate::hex(&d),
            generation: 0,
            subscriptions: summary_subscriptions(device_pubkey),
            lease_seconds: DEFAULT_LEASE_SECONDS,
            installation: None,
            lease: None,
        }
    }

    /// Make sure a lease that wakes `token` is live: register or rotate the
    /// token, then publish a new generation when none is live or the live
    /// one is in its last third.
    ///
    /// # Errors
    ///
    /// Returns the first refusal or failure; saved state stays consistent.
    pub async fn sync(
        &mut self,
        secret: &SecretKey,
        token: &str,
        now: u64,
        save: &mut dyn FnMut(&Self) -> std::result::Result<(), String>,
    ) -> Result<SyncOutcome> {
        let relay = discover(&self.relay_url).await?;
        let transport = relay.transport(&self.app_profile).ok_or_else(|| {
            ClientError::Invalid("the relay does not serve this app profile".into())
        })?;
        if !transport.valid_token(token) {
            return Err(ClientError::Invalid(
                "the native push token is not valid".into(),
            ));
        }
        let gateway = GatewayClient::new(&self.gateway_url)?;
        let digest = crate::sha256_hex(token.as_bytes());
        let lease_seconds = self.lease_seconds.min(relay.max_lease_ttl).max(3_600);
        let mut changed = false;
        match self.installation.clone() {
            Some(held) if held.expires_at > now + lease_seconds => {
                if held.token_digest != digest {
                    let epoch = gateway
                        .rotate(secret, &held.handle, held.epoch, token)
                        .await?;
                    self.installation = Some(HeldInstallation {
                        epoch,
                        token_digest: digest,
                        ..held
                    });
                    changed = true;
                    save(self).map_err(ClientError::Invalid)?;
                }
            }
            _ => {
                let registered = gateway
                    .register(
                        secret,
                        &self.app_profile,
                        token,
                        now + DEFAULT_INSTALLATION_SECONDS,
                    )
                    .await?;
                self.installation = Some(HeldInstallation {
                    handle: registered.installation_handle,
                    epoch: registered.endpoint_epoch,
                    token_digest: digest,
                    expires_at: registered.expires_at,
                });
                changed = true;
                save(self).map_err(ClientError::Invalid)?;
            }
        }
        let due = self
            .lease
            .as_ref()
            .is_none_or(|lease| lease.expiration <= now + lease_seconds / 3);
        if !changed && !due {
            return Ok(SyncOutcome::Current);
        }
        let held = self
            .installation
            .clone()
            .ok_or_else(|| ClientError::Invalid("no installation".into()))?;
        self.generation += 1;
        save(self).map_err(ClientError::Invalid)?;
        let expiration = now + lease_seconds;
        let grant = gateway
            .delegate(
                secret,
                &DelegationRequest {
                    v: wire::WIRE_VERSION,
                    installation_handle: held.handle.clone(),
                    endpoint_epoch: held.epoch,
                    generation: self.generation,
                    relay_pubkey: relay.relay_pubkey.clone(),
                    not_before: now.saturating_sub(60),
                    expires_at: expiration + SKEW_SECONDS,
                },
            )
            .await?;
        let event = lease_event(
            secret,
            &relay,
            &LeaseSpec {
                d: &self.d,
                generation: self.generation,
                app_profile: &self.app_profile,
                transport,
                endpoint: &grant,
                subscriptions: &self.subscriptions,
                expiration,
            },
            now,
        )?;
        publish(&self.relay_url, secret, &event).await?;
        if let Some(installation) = self.installation.as_mut() {
            installation.expires_at = installation.expires_at.max(expiration + SKEW_SECONDS);
        }
        self.lease = Some(PublishedLease {
            generation: self.generation,
            expiration,
        });
        save(self).map_err(ClientError::Invalid)?;
        Ok(SyncOutcome::Published {
            generation: self.generation,
        })
    }

    /// Revoke the lease with a higher-generation tombstone, then ask the
    /// gateway to forget the token. The tombstone is what stops wakes; the
    /// gateway calls are best effort once it is accepted.
    ///
    /// # Errors
    ///
    /// Returns the relay's refusal or a transport failure.
    pub async fn revoke(
        &mut self,
        secret: &SecretKey,
        now: u64,
        save: &mut dyn FnMut(&Self) -> std::result::Result<(), String>,
    ) -> Result<()> {
        let relay = discover(&self.relay_url).await?;
        if self.lease.is_some() {
            self.generation += 1;
            save(self).map_err(ClientError::Invalid)?;
            let expiration = now + self.lease_seconds.min(relay.max_lease_ttl).max(3_600);
            let event =
                revocation_event(secret, &relay, &self.d, self.generation, expiration, now)?;
            publish(&self.relay_url, secret, &event).await?;
            self.lease = None;
            save(self).map_err(ClientError::Invalid)?;
        }
        if let Some(held) = self.installation.take() {
            if let Ok(gateway) = GatewayClient::new(&self.gateway_url) {
                let _ = gateway
                    .revoke_installation(
                        secret,
                        &InstallationRevokeRequest {
                            v: wire::WIRE_VERSION,
                            installation_handle: held.handle,
                            endpoint_epoch: held.epoch,
                        },
                    )
                    .await;
            }
            save(self).map_err(ClientError::Invalid)?;
        }
        Ok(())
    }
}

/// The x-only public key of `secret`, as hexadecimal.
#[must_use]
pub fn pubkey_hex(secret: &SecretKey) -> String {
    Keypair::from_secret_key(&Secp256k1::new(), secret)
        .x_only_public_key()
        .0
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn relay_push(executor: &SecretKey) -> RelayPush {
        RelayPush {
            origin: "ws://relay.test".into(),
            key_id: "current".into(),
            executor_pubkey: pubkey_hex(executor),
            relay_pubkey: "b".repeat(64),
            app_profiles: vec![("app.test/ios".into(), "apns".into())],
            push_kinds: vec![3188],
            max_lease_ttl: 2_592_000,
        }
    }

    #[test]
    fn a_built_lease_is_accepted_by_the_executor_rules() {
        let device = SecretKey::from_byte_array([0x31; 32]).unwrap();
        let executor = SecretKey::from_byte_array([0x42; 32]).unwrap();
        let relay = relay_push(&executor);
        let subscriptions = summary_subscriptions(&pubkey_hex(&device));
        let now = 1_800_000_000;
        let event = lease_event(
            &device,
            &relay,
            &LeaseSpec {
                d: "00112233445566778899aabbccddeeff",
                generation: 1,
                app_profile: "app.test/ios",
                transport: Transport::Apns,
                endpoint: "pg1_grant",
                subscriptions: &subscriptions,
                expiration: now + 3_600,
            },
            now,
        )
        .unwrap();
        event.validate_crypto().unwrap();
        let author = XOnlyPublicKey::from_byte_array(decode_hex32(&event.pubkey).unwrap()).unwrap();
        let plaintext = nostr::push_lease::open_lease(&event.content, &executor, &author).unwrap();
        let descriptor = nostr::push_lease::PushDescriptor {
            origin: relay.origin.clone(),
            key_id: "current".into(),
            pubkey: relay.executor_pubkey.clone(),
            app_profile: "app.test/ios".into(),
            transport: "apns".into(),
            push_kinds: vec![3188],
            limits: nostr::push_lease::LeaseLimits::default(),
        };
        let accepted =
            nostr::push_lease::accept_lease(&event, &plaintext, now, &descriptor, None, &[], 0)
                .unwrap();
        assert!(accepted.active);
        assert_eq!(accepted.endpoint.as_deref(), Some("pg1_grant"));
        let tombstone = revocation_event(
            &device,
            &relay,
            "00112233445566778899aabbccddeeff",
            2,
            now + 60,
            now + 1,
        )
        .unwrap();
        let plaintext =
            nostr::push_lease::open_lease(&tombstone.content, &executor, &author).unwrap();
        let revoked = nostr::push_lease::accept_lease(
            &tombstone,
            &plaintext,
            now + 1,
            &descriptor,
            Some(&accepted),
            &[],
            0,
        )
        .unwrap();
        assert!(!revoked.active);
    }

    #[test]
    fn nip11_push_support_needs_the_extension_a_current_key_and_self() {
        let document = json!({
            "self": "b".repeat(64),
            "supported_extensions": ["nip-pl"],
            "push": {
                "origin": "wss://relay.example",
                "keys": [{"id": "current", "pubkey": "c".repeat(64), "current": true}],
                "app_profiles": [{"id": "app/ios", "transport": "apns"}],
                "push_kinds": [3188],
                "limitation": {"max_lease_ttl": 86_400}
            }
        });
        let relay = parse_relay_push(&document).unwrap();
        assert_eq!(relay.transport("app/ios"), Some(Transport::Apns));
        assert_eq!(relay.max_lease_ttl, 86_400);
        let mut no_self = document.clone();
        no_self.as_object_mut().unwrap().remove("self");
        assert!(parse_relay_push(&no_self).is_err());
        let mut no_extension = document;
        no_extension["supported_extensions"] = json!([]);
        assert!(parse_relay_push(&no_extension).is_err());
    }
}
