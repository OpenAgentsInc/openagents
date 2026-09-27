//! This machine as a device of other hosts.
//!
//! A linked computer is also a client: its device key and grants live in the
//! same owner-only store the Computers screens use on the desktop and in the
//! terminal, `~/.openagents/coder-computers` (`device.key` and
//! `computers.json`). Joining redeems an invitation into that store; the
//! check proves each route to each joined host.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use coder_access::protocol::{Operation, Outcome};
use coder_access::{Code, RelayPolicy, Right};
use coder_computers::live::{FileStore, Saved, SavedHost, Store, load_or_create_key};
use coder_host::client::{Device, Link, WebSocketTls, connect_websocket, fetch_reach};
use coder_reach::hints::{Class, Locality, Transport, select};
use secp256k1::SecretKey;
use tokio::net::TcpStream;

use crate::{Error, Result};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(8);
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

/// `~/.openagents/coder-computers`.
///
/// # Errors
/// Reports an unset `HOME`.
pub fn default_dir() -> Result<PathBuf> {
    crate::home(".openagents/coder-computers")
}

/// This machine's device key and saved hosts.
pub struct Client {
    secret: SecretKey,
    store: FileStore,
    saved: Saved,
    policy: RelayPolicy,
    locality: Locality,
}

impl Client {
    /// Open the store, creating the device key on first use.
    ///
    /// # Errors
    /// Reports an unreadable key or record.
    pub fn open(dir: &Path, policy: RelayPolicy) -> Result<Self> {
        let secret = load_or_create_key(dir).map_err(Error::new)?;
        let mut store = FileStore::open(dir).map_err(Error::new)?;
        let saved = store.load().map_err(Error::new)?.unwrap_or_default();
        Ok(Self {
            secret,
            store,
            saved,
            policy,
            locality: Locality::OtherMachine,
        })
    }

    /// Where the hosts run relative to this client. Claim
    /// `Locality::SameMachine` only from local evidence, such as a test on
    /// one machine; it admits loopback hints.
    #[must_use]
    pub fn with_locality(mut self, locality: Locality) -> Self {
        self.locality = locality;
        self
    }

    /// This device's public key.
    #[must_use]
    pub fn key(&self) -> String {
        coder_reach::pubkey(&self.secret)
    }

    /// The hosts this device joined.
    #[must_use]
    pub fn hosts(&self) -> &[SavedHost] {
        &self.saved.hosts
    }

    /// Redeem `invitation` and save the grant under `label`, replacing an
    /// earlier grant for the same host. Returns the host key. The host must
    /// be serving the invitation's relay.
    ///
    /// # Errors
    /// Reports a refused, expired, or used invitation, and a failed save.
    pub async fn join(&mut self, invitation: &str, label: &str) -> Result<String> {
        let access = coder_access::client::redeem(invitation.trim(), &self.secret, self.policy)
            .await
            .map_err(|error| Error::new(format!("the invitation was refused: {error}")))?;
        let host = access.grant.host.clone();
        self.saved
            .hosts
            .retain(|saved| saved.access.grant.host != host);
        self.saved.hosts.push(SavedHost {
            access,
            label: label.to_owned(),
            enabled: true,
            revoked: false,
            ssh: None,
            delisted: false,
        });
        self.store.save(&self.saved).map_err(Error::new)?;
        Ok(host)
    }

    /// Prove the routes to every enabled, unrevoked host this device joined.
    pub async fn check(&self, which: Which) -> Vec<Checked> {
        let mut results = Vec::new();
        for saved in self.saved.hosts.iter().filter(|h| h.enabled && !h.revoked) {
            results.extend(check_host(saved, self.secret, self.policy, self.locality, which).await);
        }
        results
    }
}

/// Which routes to prove.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Which {
    Direct,
    Relay,
    Both,
}

/// One route's result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checked {
    /// The host's label on this device.
    pub label: String,
    pub host: String,
    /// `direct` or `relay`.
    pub kind: &'static str,
    /// The hint address or relay URL.
    pub route: String,
    pub ok: bool,
    pub detail: String,
    pub millis: u128,
}

async fn check_host(
    saved: &SavedHost,
    secret: SecretKey,
    policy: RelayPolicy,
    locality: Locality,
    which: Which,
) -> Vec<Checked> {
    let host = saved.access.grant.host.clone();
    let checked = |kind, route: String, ok, detail: String, started: Instant| Checked {
        label: saved.label.clone(),
        host: host.clone(),
        kind,
        route,
        ok,
        detail,
        millis: started.elapsed().as_millis(),
    };
    let started = Instant::now();
    let device = match Device::new(saved.access.clone(), secret, policy) {
        Ok(device) => Arc::new(device),
        Err(error) => {
            return vec![checked(
                "grant",
                String::new(),
                false,
                error.to_string(),
                started,
            )];
        }
    };
    let relay = device.relay().to_owned();
    let reach = match fetch_reach(&device, &relay).await {
        Ok(reach) => reach,
        Err(error) => {
            return vec![checked(
                "presence",
                relay,
                false,
                format!("no fresh presence: {error}"),
                started,
            )];
        }
    };
    let mut results = Vec::new();
    let generation = reach.presence.presence.generation;
    if which != Which::Relay {
        let now = coder_host::unix_time().unwrap_or(0);
        let hints = select(&reach.hints, locality, generation, now)
            .map(|hints| hints.into_iter().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        let direct: Vec<_> = hints
            .into_iter()
            .filter(|hint| hint.class != Class::Relay && hint.transport != Transport::Nostr)
            .collect();
        if direct.is_empty() {
            results.push(checked(
                "direct",
                String::new(),
                false,
                "the host advertises no direct route to another machine".into(),
                Instant::now(),
            ));
        }
        for hint in direct {
            let started = Instant::now();
            let outcome = direct_ping(&device, hint.transport, &hint.address, generation).await;
            let class = format!("{:?}", hint.class).to_lowercase();
            results.push(match outcome {
                Ok(()) => checked(
                    "direct",
                    hint.address.clone(),
                    true,
                    format!("{class} channel proved both keys and answered a ping"),
                    started,
                ),
                Err(why) => checked("direct", hint.address.clone(), false, why, started),
            });
        }
    }
    if which != Which::Direct {
        let started = Instant::now();
        results.push(match relay_call(&device, &relay).await {
            Ok(detail) => checked("relay", relay.clone(), true, detail, started),
            Err(why) => checked("relay", relay.clone(), false, why, started),
        });
    }
    results
}

async fn direct_ping(
    device: &Arc<Device>,
    transport: Transport,
    address: &str,
    generation: u64,
) -> std::result::Result<(), String> {
    let link = match transport {
        Transport::Tcp => {
            let stream = tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(address))
                .await
                .map_err(|_| "connect timed out".to_owned())?
                .map_err(|error| format!("connect failed: {error}"))?;
            Link::direct(
                device.clone(),
                stream,
                address.to_owned(),
                generation,
                HANDSHAKE_TIMEOUT,
            )
            .await
        }
        Transport::Websocket => {
            let stream = connect_websocket(address, &WebSocketTls::webpki(), CONNECT_TIMEOUT)
                .await
                .map_err(|error| format!("WebSocket failed: {error}"))?;
            Link::direct(
                device.clone(),
                stream,
                address.to_owned(),
                generation,
                HANDSHAKE_TIMEOUT,
            )
            .await
        }
        Transport::Nostr => return Err("not a direct route".into()),
    }
    .map_err(|error| format!("handshake failed: {error}"))?;
    let pinged = link
        .ping()
        .await
        .map_err(|error| format!("ping failed: {error}"));
    link.shutdown();
    pinged
}

/// One NIP-HOST operation over the relay alone: `device.list` when the grant
/// holds `access_read`. The host's signed reply is the proof.
async fn relay_call(device: &Arc<Device>, relay: &str) -> std::result::Result<String, String> {
    if !device.access().grant.rights.contains(Right::AccessRead) {
        return Ok(
            "fresh presence arrived through the relay; the grant has no access_read for a round trip"
                .into(),
        );
    }
    let link = Link::relay(device.clone(), relay.to_owned());
    match link.call(Operation::ListDevices {}).await {
        Ok(Outcome::Devices { devices }) => Ok(format!(
            "device.list answered through the relay with {} devices",
            devices.len()
        )),
        Ok(_) => Ok("the host answered through the relay".into()),
        Err(coder_host::Error::Access(error)) if error.code != Code::Transport => Err(format!(
            "the host refused through the relay: {:?}",
            error.code
        )),
        Err(error) => Err(format!("no answer through the relay: {error}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_store_has_a_private_key_and_no_hosts() {
        let dir = tempfile::tempdir().unwrap();
        let client = Client::open(dir.path(), RelayPolicy::Production).unwrap();
        assert!(client.hosts().is_empty());
        let again = Client::open(dir.path(), RelayPolicy::Production).unwrap();
        assert_eq!(client.key(), again.key());
    }
}
