//! Spend wakes: the host's note to a phone that a payment request waits.
//!
//! The phone reads spend requests with NIP-HOST `spend.list`, which it sends
//! only while the app runs. A spend wake is how a request reaches a phone
//! that is not looking: it is a private `3188` artifact from the host to the
//! device whose grant the request draws on, on the spend-wake mailbox the
//! two derive ([`crate::mailbox::Stream::SpendWakes`]), stored by the relay
//! for [`LIFETIME`]. A relay whose NIP-PL executor holds the phone's push
//! lease (`kind 3188`, `#p` the device) wakes the phone for it; the wake
//! body is the fixed transport constant, never the artifact.
//!
//! A wake carries no request, amount, payee, or task, and grants nothing.
//! It says only that this host has something for this device. The phone
//! answers by sending `spend.list`, which the host answers as usual; a
//! forged or replayed wake costs one `spend.list`.

use nostr::domain::Event;
use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};

use crate::mailbox::{self, Stream};
use crate::{Error, Result};

/// The wake artifact's schema.
pub const SCHEMA: &str = "openagents.spend-wake.v1";
/// How long a wake stays meaningful: a spend request's longest life.
pub const LIFETIME: u64 = coder_access::spend::MAX_REQUEST_LIFETIME;

/// A host's wake to one device.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Wake {
    pub v: String,
    pub requires: Vec<String>,
    /// The host that has requests waiting.
    pub host: String,
    /// The device whose grant they draw on.
    pub device: String,
    pub issued_at: u64,
    pub expires_at: u64,
}

impl Wake {
    /// A wake from `secret`'s key (the host) to `device`, dated `now`.
    #[must_use]
    pub fn new(secret: &SecretKey, device: &str, now: u64) -> Self {
        Self {
            v: SCHEMA.into(),
            requires: vec![],
            host: coder_reach::pubkey(secret),
            device: device.into(),
            issued_at: now,
            expires_at: now + LIFETIME,
        }
    }

    /// Seal it to the device on the spend-wake mailbox.
    ///
    /// # Errors
    /// Refuses a device that is not an x-only key.
    pub fn seal(&self, secret: &SecretKey) -> Result<Event> {
        let mailbox = mailbox::mailbox(secret, &self.device, Stream::SpendWakes)?;
        coder_connect::protocol::seal(
            self,
            SCHEMA,
            secret,
            &self.device,
            &mailbox,
            self.issued_at,
            self.expires_at,
        )
        .map_err(|_| Error::Config("the spend wake cannot be sealed".into()))
    }

    /// Open a wake addressed to the device whose key is `secret`, and check
    /// that its signer is the host it names, it names this device, it sits
    /// on the pair's mailbox, and it is current at `now`.
    ///
    /// # Errors
    /// Refuses anything else.
    pub fn open(event: &Event, secret: &SecretKey, now: u64) -> Result<Self> {
        let device = coder_reach::pubkey(secret);
        let refused = || Error::Config("not a current spend wake for this device".into());
        let wake: Self =
            coder_connect::protocol::open(event, secret, &event.pubkey, &device, SCHEMA)
                .map_err(|_| refused())?;
        let mailbox = mailbox::mailbox(secret, &event.pubkey, Stream::SpendWakes)?;
        let current = wake.issued_at <= now + 300 && now < wake.expires_at;
        if wake.v != SCHEMA
            || !wake.requires.is_empty()
            || wake.device != device
            || wake.host != event.pubkey
            || wake.expires_at != wake.issued_at + LIFETIME
            || event.tag_values("h").collect::<Vec<_>>() != [mailbox.as_str()]
            || !current
        {
            return Err(refused());
        }
        Ok(wake)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(n: u8) -> SecretKey {
        SecretKey::from_byte_array([n; 32]).unwrap()
    }

    #[test]
    fn a_spend_wake_names_only_the_pair_and_opens_only_for_its_device() {
        let (host, device, other) = (key(3), key(4), key(5));
        let device_key = coder_reach::pubkey(&device);
        let now = 1_790_000_000;
        let event = Wake::new(&host, &device_key, now).seal(&host).unwrap();
        // The push lease matches `kind 3188` with `#p` the device.
        assert_eq!(event.kind, 3188);
        assert_eq!(
            event.tag_values("p").collect::<Vec<_>>(),
            [device_key.as_str()]
        );
        let opened = Wake::open(&event, &device, now + 60).unwrap();
        assert_eq!(opened.host, coder_reach::pubkey(&host));
        // Nothing about the request crosses the wire.
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
        // Another device cannot open it, and it lapses with a request's life.
        assert!(Wake::open(&event, &other, now).is_err());
        assert!(Wake::open(&event, &device, now + LIFETIME).is_err());
        // A host cannot wake in another host's name.
        let mut forged = Wake::new(&host, &device_key, now);
        forged.host = coder_reach::pubkey(&other);
        let forged = forged.seal(&host).unwrap();
        assert!(Wake::open(&forged, &device, now).is_err());
        // The mailbox is not the nudge mailbox the device sends on.
        assert_ne!(
            mailbox::mailbox(&host, &device_key, Stream::SpendWakes).unwrap(),
            mailbox::mailbox(&host, &device_key, Stream::Nudges).unwrap()
        );
    }
}
