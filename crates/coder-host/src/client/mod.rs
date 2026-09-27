//! The device side: find a host, prove a route, and operate it.
//!
//! - [`fetch_directory`] reads the owner's host directory with the owner
//!   key. [`publish_directory`] is the owner's action that adds a host.
//! - [`fetch_reach`] reads the host's presence and hints sealed to this
//!   device, and [`fetch_summaries`] its activity summaries.
//! - [`Link`] is one proven route: a direct channel or relay fallback. It
//!   carries NIP-HOST operations and NIP-TERM requests over either.
//! - [`Connector`] implements the `coder-link` connector, so a `Registry`
//!   decides when to connect and each attempt tries direct routes, over TCP
//!   or WebSocket, before the relay.

use std::collections::BTreeMap;
use std::time::Duration;

use coder_access::{Access, RelayPolicy};
use coder_reach::directory::Directory;
use coder_reach::hints::Hints;
use coder_reach::presence::{Freshness, Presence, Received};
use nostr::activity_summary::{self, ActivitySummary};
use nostr::domain::Event;
use nostr_transport::Connection;
use secp256k1::SecretKey;
use serde_json::{Value, json};

use crate::mailbox::{self, Stream};
use crate::{Error, Result, unix_time};

mod connector;
mod link;
mod order;
mod websocket;

pub use connector::{Connector, Reports};
pub use link::{Link, Route};
pub use order::Ordered;
pub use websocket::Stream as WebSocketStream;

/// How long one relay fetch may take.
const FETCH_TIMEOUT: Duration = Duration::from_secs(10);
/// The most events one fetch reads.
const FETCH_LIMIT: usize = 200;

/// An enrolled device: its key and its saved access record for one host.
pub struct Device {
    pub(crate) secret: SecretKey,
    pub(crate) access: Access,
    pub(crate) client: coder_access::Client,
    pub(crate) policy: RelayPolicy,
}

impl std::fmt::Debug for Device {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The secret key stays out of debug output.
        f.debug_struct("Device")
            .field("device", &self.key())
            .field("host", &self.host())
            .finish_non_exhaustive()
    }
}

impl Device {
    /// A device from its access record.
    ///
    /// # Errors
    /// Refuses a record that belongs to another key or has expired.
    pub fn new(access: Access, secret: SecretKey, policy: RelayPolicy) -> Result<Self> {
        let client = coder_access::Client::device(access.clone(), secret, policy)?;
        Ok(Self {
            secret,
            access,
            client,
            policy,
        })
    }

    /// This device's public key.
    #[must_use]
    pub fn key(&self) -> String {
        coder_reach::pubkey(&self.secret)
    }

    /// The host this device enrolled with.
    #[must_use]
    pub fn host(&self) -> &str {
        &self.access.grant.host
    }

    /// The relay the grant names.
    #[must_use]
    pub fn relay(&self) -> &str {
        &self.access.grant.relay
    }

    /// The grant ID.
    #[must_use]
    pub fn grant(&self) -> &str {
        &self.access.grant.grant
    }

    /// The grant's epoch.
    #[must_use]
    pub fn epoch(&self) -> u64 {
        self.access.grant.epoch
    }

    /// The saved access record.
    #[must_use]
    pub fn access(&self) -> &Access {
        &self.access
    }
}

/// A host's presence and hints, as this device read them.
#[derive(Clone, Debug)]
pub struct Reach {
    pub presence: Received,
    pub hints: Hints,
}

/// Read the owner's current host directory. Only the owner key can.
///
/// # Errors
/// Reports transport failures and conflicting revisions.
pub async fn fetch_directory(
    relay: &str,
    owner: &SecretKey,
    policy: RelayPolicy,
) -> Result<Option<Directory>> {
    let key = coder_reach::pubkey(owner);
    let filter = json!({"kinds": [3188], "authors": [key], "#p": [key]});
    let events = fetch(relay, owner, policy, filter).await?;
    let directories: Vec<Directory> = events
        .iter()
        .filter_map(|event| Directory::open(event, owner).ok())
        .collect();
    Ok(Directory::current(&directories)?.cloned())
}

/// Publish a directory revision, sealed by the owner to the owner.
///
/// # Errors
/// Refuses a key that is not the directory's owner, and reports transport
/// failures.
pub async fn publish_directory(
    relay: &str,
    owner: &SecretKey,
    directory: &Directory,
    mailbox: &str,
    retain_until: u64,
    policy: RelayPolicy,
) -> Result<()> {
    policy
        .validate(relay)
        .map_err(|_| Error::Config("the relay policy refuses this relay".into()))?;
    let event = directory.seal(owner, mailbox, retain_until)?;
    nostr_transport::artifacts::publish(relay, owner, &event)
        .await
        .map_err(Error::Transport)
}

/// Read the newest presence and hints the host sealed to this device.
///
/// # Errors
/// Reports transport failures, and `stale` when the host published nothing
/// current.
pub async fn fetch_reach(device: &Device, relay: &str) -> Result<Reach> {
    let host = device.host();
    let presence_box = mailbox::mailbox(&device.secret, host, Stream::Presence)?;
    let hints_box = mailbox::mailbox(&device.secret, host, Stream::Hints)?;
    let me = device.key();
    let filter = json!({
        "kinds": [3188], "authors": [host], "#p": [me], "#h": [presence_box, hints_box]
    });
    let events = fetch(relay, &device.secret, device.policy, filter).await?;
    let now = unix_time()?;
    let owner = &device.access.grant.owner;
    let presence = events
        .iter()
        .filter_map(|event| Presence::open(event, &device.secret, host, owner).ok())
        .max_by_key(|p| (p.generation, p.observed_at))
        .ok_or_else(|| stale("the host published no presence to this device"))?;
    let received = Received {
        presence,
        received_at: now,
    };
    received.judge(now, Freshness::default())?;
    let hints = events
        .iter()
        .filter_map(|event| Hints::open(event, &device.secret, host).ok())
        .filter(|h| h.generation == received.presence.generation && h.expires_at > now)
        .max_by_key(|h| h.issued_at)
        .ok_or_else(|| stale("the host published no current hints to this device"))?;
    Ok(Reach {
        presence: received,
        hints,
    })
}

/// Read the activity summaries the host sealed to this device and keep the
/// highest sequence per subject. A summary is a pointer, not evidence.
///
/// # Errors
/// Reports transport failures.
pub async fn fetch_summaries(device: &Device, relay: &str) -> Result<Vec<ActivitySummary>> {
    let host = device.host();
    let mailbox = mailbox::mailbox(&device.secret, host, Stream::Summaries)?;
    let filter = json!({
        "kinds": [3188], "authors": [host], "#p": [device.key()], "#h": [mailbox]
    });
    let events = fetch(relay, &device.secret, device.policy, filter).await?;
    let mut latest: BTreeMap<String, ActivitySummary> = BTreeMap::new();
    for event in &events {
        let Ok(summary) = activity_summary::open(event, &device.secret, host) else {
            continue;
        };
        match latest.get(&summary.subject) {
            Some(held) if !activity_summary::supersedes(held, &summary).unwrap_or(false) => {}
            _ => {
                latest.insert(summary.subject.clone(), summary);
            }
        }
    }
    Ok(latest.into_values().collect())
}

fn stale(detail: &'static str) -> Error {
    Error::Reach(coder_reach::Error::new(coder_reach::Refusal::Stale, detail))
}

/// Read every stored event that matches `filter`, up to the end of stored
/// events.
async fn fetch(
    relay: &str,
    secret: &SecretKey,
    policy: RelayPolicy,
    filter: Value,
) -> Result<Vec<Event>> {
    policy
        .validate(relay)
        .map_err(|_| Error::Config("the relay policy refuses this relay".into()))?;
    tokio::time::timeout(FETCH_TIMEOUT, async {
        let mut socket = Connection::connect(relay, secret, FETCH_TIMEOUT)
            .await
            .map_err(Error::Transport)?;
        let id = coder_reach::new_id();
        socket
            .send(json!(["REQ", id, filter]))
            .await
            .map_err(Error::Transport)?;
        let mut events = Vec::new();
        loop {
            let frame = socket.next().await.map_err(Error::Transport)?;
            if frame[1] != id.as_str() {
                continue;
            }
            match frame[0].as_str() {
                Some("EOSE") => break,
                Some("CLOSED") => return Err(Error::Transport("the relay closed a fetch".into())),
                Some("EVENT") if events.len() < FETCH_LIMIT => {
                    if let Ok(event) = serde_json::from_value::<Event>(frame[2].clone()) {
                        events.push(event);
                    }
                }
                _ => {}
            }
        }
        let _ = socket.close().await;
        Ok(events)
    })
    .await
    .map_err(|_| Error::Transport("the relay fetch timed out".into()))?
}
