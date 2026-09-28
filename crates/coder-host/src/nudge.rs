//! Nudges: a device's stored note to its host that it has commands waiting.
//!
//! A NIP-HOST request lives 60 seconds, and a host that was asleep, offline,
//! or reconnecting misses requests sent meanwhile; the device keeps its
//! commands and tries again with the same command IDs, but only while it
//! runs. A nudge is how the host learns, when it comes back, that a device
//! is waiting. It is a private `3188` artifact from the device to the host
//! on the nudge mailbox the two derive ([`crate::mailbox::Stream::Nudges`]),
//! stored by the relay and valid for [`LIFETIME`].
//!
//! A nudge carries no command, text, or task, and grants nothing. The host
//! reads the nudges addressed to it whenever its relay subscription
//! connects, and the new ones as they arrive. For a nudge from a device
//! whose grant is current, it evaluates held commands and publishes its
//! presence and hints to that device at once, at most once per
//! [`MIN_INTERVAL`] per device. The device sees the fresh presence and sends
//! its waiting commands again.

use coder_access::RelayPolicy;
use nostr::domain::Event;
use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};

use crate::mailbox::{self, Stream};
use crate::{Error, Result};

/// The nudge artifact's schema.
pub const SCHEMA: &str = "openagents.host-nudge.v1";
/// How long a nudge stays meaningful: a device command's lifetime.
pub const LIFETIME: u64 = 24 * 60 * 60;
/// The least time between two answers to one device's nudges, in seconds.
pub const MIN_INTERVAL: u64 = 30;

/// A device's nudge to its host.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Nudge {
    pub v: String,
    pub requires: Vec<String>,
    /// The host the device nudges.
    pub host: String,
    /// The nudging device.
    pub device: String,
    pub issued_at: u64,
    pub expires_at: u64,
}

impl Nudge {
    /// A nudge from `secret`'s key to `host`, dated `now`.
    #[must_use]
    pub fn new(secret: &SecretKey, host: &str, now: u64) -> Self {
        Self {
            v: SCHEMA.into(),
            requires: vec![],
            host: host.into(),
            device: coder_reach::pubkey(secret),
            issued_at: now,
            expires_at: now + LIFETIME,
        }
    }

    /// Seal it to the host on the nudge mailbox.
    ///
    /// # Errors
    /// Refuses a host that is not an x-only key.
    pub fn seal(&self, secret: &SecretKey) -> Result<Event> {
        let mailbox = mailbox::mailbox(secret, &self.host, Stream::Nudges)?;
        coder_connect::protocol::seal(
            self,
            SCHEMA,
            secret,
            &self.host,
            &mailbox,
            self.issued_at,
            self.expires_at,
        )
        .map_err(|_| Error::Config("the nudge cannot be sealed".into()))
    }

    /// Open a nudge addressed to the host whose key is `secret`, and check
    /// that its signer is the device it names, it names this host, and it is
    /// current at `now`.
    ///
    /// # Errors
    /// Refuses anything else.
    pub fn open(event: &Event, secret: &SecretKey, now: u64) -> Result<Self> {
        let host = coder_reach::pubkey(secret);
        let refused = || Error::Config("not a current nudge for this host".into());
        let nudge: Self =
            coder_connect::protocol::open(event, secret, &event.pubkey, &host, SCHEMA)
                .map_err(|_| refused())?;
        let mailbox = mailbox::mailbox(secret, &event.pubkey, Stream::Nudges)?;
        let current = nudge.issued_at <= now + 300 && now < nudge.expires_at;
        if nudge.v != SCHEMA
            || !nudge.requires.is_empty()
            || nudge.host != host
            || nudge.device != event.pubkey
            || nudge.expires_at != nudge.issued_at + LIFETIME
            || event.tag_values("h").collect::<Vec<_>>() != [mailbox.as_str()]
            || !current
        {
            return Err(refused());
        }
        Ok(nudge)
    }
}

/// Publish a nudge from `secret`'s key to `host` on `relay`.
///
/// # Errors
/// Reports a relay the policy refuses and transport failures.
pub async fn send(
    relay: &str,
    secret: &SecretKey,
    host: &str,
    policy: RelayPolicy,
    now: u64,
) -> Result<Event> {
    policy
        .validate(relay)
        .map_err(|_| Error::Config("the relay policy refuses this relay".into()))?;
    let event = Nudge::new(secret, host, now).seal(secret)?;
    nostr_transport::artifacts::publish(relay, secret, &event)
        .await
        .map_err(Error::Transport)?;
    Ok(event)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(n: u8) -> SecretKey {
        SecretKey::from_byte_array([n; 32]).unwrap()
    }

    #[test]
    fn only_the_addressed_host_opens_a_current_nudge_from_its_signer() {
        let (host, device, other) = (key(3), key(4), key(5));
        let host_key = coder_reach::pubkey(&host);
        let now = 1_790_000_000;
        let event = Nudge::new(&device, &host_key, now).seal(&device).unwrap();
        let opened = Nudge::open(&event, &host, now + 60).unwrap();
        assert_eq!(opened.device, coder_reach::pubkey(&device));
        // Nothing but its own existence crosses the wire.
        let body = serde_json::to_value(&opened).unwrap();
        let mut fields: Vec<&str> = body
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        fields.sort_unstable();
        assert_eq!(
            fields,
            ["device", "expires_at", "host", "issued_at", "requires", "v"]
        );
        // Another host cannot open it, and it lapses with a command's life.
        assert!(Nudge::open(&event, &other, now).is_err());
        assert!(Nudge::open(&event, &host, now + LIFETIME).is_err());
        // A nudge sealed to another host does not open here.
        let elsewhere = Nudge::new(&device, &coder_reach::pubkey(&other), now)
            .seal(&device)
            .unwrap();
        assert!(Nudge::open(&elsewhere, &host, now).is_err());
        // A device cannot nudge in another device's name.
        let mut forged = Nudge::new(&device, &host_key, now);
        forged.device = coder_reach::pubkey(&other);
        let forged = forged.seal(&device).unwrap();
        assert!(Nudge::open(&forged, &host, now).is_err());
    }
}
