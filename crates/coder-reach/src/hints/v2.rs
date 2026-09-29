//! The v2 hint record: the v1 body plus `iroh` hints (NIP-REACH, iroh
//! hints).
//!
//! A v1 reader refuses an unknown transport, and with it the whole record,
//! so an `iroh` hint travels only in `openagents.reach-hints.v2`. A host
//! that has an iroh endpoint publishes both records to each enrolled device
//! for the same generation: v1 without the `iroh` hint, and v2 with it. A
//! reader that understands v2 prefers it.
//!
//! An `EndpointId` is a route, never an identity: the direct-channel
//! handshake proves the host key and the grant check admits the channel, as
//! over every other transport.

use std::collections::BTreeSet;

use nostr::domain::Event;
use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{Class, Hint, Hints, MAX_HINTS, MAX_LIFETIME, Status, tcp_host};
use crate::{Refusal, Result, artifact, fail, parse_pubkey, pubkey, requires};

/// Schema of a v2 hint set.
pub const SCHEMA: &str = "openagents.reach-hints.v2";
/// Most direct addresses one `iroh` hint carries.
pub const MAX_DIRECT: usize = 8;
/// Longest iroh relay URL, in bytes.
pub const MAX_RELAY_URL_BYTES: usize = 128;

/// The `iroh` transport, spelled `"iroh"`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IrohTransport {
    Iroh,
}

/// A host's iroh endpoint.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IrohHint {
    /// Always `public`: iroh picks a direct path or its relay itself.
    pub class: Class,
    pub transport: IrohTransport,
    /// The host's `EndpointId`, 64 lowercase hex characters.
    pub address: String,
    /// The iroh relay the host keeps its home connection on, if any.
    pub relay: Option<String>,
    /// Up to eight `ip:port` addresses the host observed.
    pub direct: Vec<String>,
    pub status: Status,
    pub observed_at: u64,
}

/// One hint in a v2 record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AnyHint {
    Iroh(IrohHint),
    Plain(Hint),
}

/// A host-signed v2 hint set, bound to one host generation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HintsV2 {
    pub v: String,
    pub requires: Vec<String>,
    pub host: String,
    pub generation: u64,
    pub issued_at: u64,
    pub expires_at: u64,
    pub hints: Vec<AnyHint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

impl IrohHint {
    /// Check the endpoint ID, relay URL, and direct addresses.
    ///
    /// # Errors
    /// Refuses another class, an endpoint ID that is not 64 lowercase hex
    /// characters, a relay URL over its bound or with credentials, a query,
    /// or a fragment, more than eight or duplicate direct addresses, an
    /// unspecified, multicast, or broadcast address, and a hint with
    /// neither a relay nor a direct address.
    pub fn validate(&self) -> Result<()> {
        if self.class != Class::Public {
            return fail(Refusal::Malformed, "an iroh hint is public");
        }
        let hex = self.address.len() == 64
            && self
                .address
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
        if !hex {
            return fail(Refusal::Malformed, "iroh endpoint ID");
        }
        if let Some(relay) = &self.relay {
            relay_url(relay)?;
        }
        if self.direct.len() > MAX_DIRECT {
            return fail(Refusal::LimitExceeded, "too many direct addresses");
        }
        let mut seen = BTreeSet::new();
        for address in &self.direct {
            let socket: std::net::SocketAddr = address
                .parse()
                .map_err(|_| crate::Error::new(Refusal::Malformed, "direct address"))?;
            // The same checks a TCP hint's address gets, on the canonical
            // spelling.
            if socket.to_string() != *address {
                return fail(Refusal::Malformed, "direct address spelling");
            }
            tcp_host(address)?;
            if !seen.insert(socket) {
                return fail(Refusal::Malformed, "duplicate direct address");
            }
        }
        if self.relay.is_none() && self.direct.is_empty() {
            return fail(
                Refusal::Malformed,
                "an iroh hint needs a relay or an address",
            );
        }
        Ok(())
    }

    /// The direct addresses a client should dial: all of them on the host's
    /// own machine, and none that is loopback anywhere else.
    #[must_use]
    pub fn dialable(&self, locality: super::Locality) -> Vec<std::net::SocketAddr> {
        self.direct
            .iter()
            .filter_map(|address| address.parse::<std::net::SocketAddr>().ok())
            .filter(|socket| locality == super::Locality::SameMachine || !socket.ip().is_loopback())
            .collect()
    }
}

/// An `https` URL without credentials, query, or fragment, at most 128
/// bytes. Plain `http` is allowed only to a loopback address, for fixtures.
fn relay_url(value: &str) -> Result<()> {
    let url = url::Url::parse(value).map_err(|_| crate::Error::new(Refusal::Malformed, "relay"))?;
    let loopback = super::host_is_loopback(url.host());
    if value.len() > MAX_RELAY_URL_BYTES
        || !value.is_ascii()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.host().is_none()
        || !(url.scheme() == "https" || (loopback && url.scheme() == "http"))
    {
        return fail(
            Refusal::Malformed,
            "iroh relay must be a credential-free https URL without query or fragment",
        );
    }
    Ok(())
}

impl HintsV2 {
    /// The v2 record for a v1 set and an optional iroh hint.
    #[must_use]
    pub fn from_v1(v1: &Hints, iroh: Option<IrohHint>) -> Self {
        Self {
            v: SCHEMA.into(),
            requires: vec![],
            host: v1.host.clone(),
            generation: v1.generation,
            issued_at: v1.issued_at,
            expires_at: v1.expires_at,
            hints: iroh
                .into_iter()
                .map(AnyHint::Iroh)
                .chain(v1.hints.iter().cloned().map(AnyHint::Plain))
                .collect(),
            meta: None,
        }
    }

    /// The record's one `iroh` hint, if it has one.
    #[must_use]
    pub fn iroh(&self) -> Option<&IrohHint> {
        self.hints.iter().find_map(|hint| match hint {
            AnyHint::Iroh(iroh) => Some(iroh),
            AnyHint::Plain(_) => None,
        })
    }

    /// The same record as v1: every hint but the `iroh` one.
    #[must_use]
    pub fn v1(&self) -> Hints {
        Hints {
            v: super::SCHEMA.into(),
            requires: vec![],
            host: self.host.clone(),
            generation: self.generation,
            issued_at: self.issued_at,
            expires_at: self.expires_at,
            hints: self
                .hints
                .iter()
                .filter_map(|hint| match hint {
                    AnyHint::Plain(plain) => Some(plain.clone()),
                    AnyHint::Iroh(_) => None,
                })
                .collect(),
            meta: None,
        }
    }

    /// Check the closed schema, bounds, and every hint.
    ///
    /// # Errors
    /// Refuses another version, a bad lifetime, two `iroh` hints, and every
    /// case v1 refuses.
    pub fn validate(&self) -> Result<()> {
        if self.v != SCHEMA {
            return fail(Refusal::UnsupportedVersion, "hint set version");
        }
        requires(&self.requires)?;
        parse_pubkey(&self.host)?;
        if self.expires_at <= self.issued_at || self.expires_at - self.issued_at > MAX_LIFETIME {
            return fail(Refusal::Malformed, "hint set lifetime");
        }
        if self.hints.len() > MAX_HINTS {
            return fail(Refusal::LimitExceeded, "too many hints");
        }
        let mut iroh = 0;
        for hint in &self.hints {
            let observed = match hint {
                AnyHint::Iroh(hint) => {
                    iroh += 1;
                    hint.validate()?;
                    hint.observed_at
                }
                AnyHint::Plain(hint) => hint.observed_at,
            };
            if observed > self.issued_at {
                return fail(Refusal::Malformed, "hint observed after issue");
            }
        }
        if iroh > 1 {
            return fail(Refusal::Malformed, "more than one iroh hint");
        }
        // The plain hints follow every v1 rule, duplicates included.
        let mut v1 = self.v1();
        v1.expires_at = self.expires_at;
        v1.validate()
    }

    /// Sign with the host key and encrypt to one enrolled device.
    ///
    /// # Errors
    /// Refuses an invalid set or a key that does not match `host`.
    pub fn seal(&self, host: &SecretKey, device: &str, mailbox: &str) -> Result<Event> {
        self.validate()?;
        if pubkey(host) != self.host {
            return fail(Refusal::IdentityMismatch, "only the host seals its hints");
        }
        parse_pubkey(device)?;
        artifact::seal(
            self,
            SCHEMA,
            host,
            device,
            mailbox,
            self.issued_at,
            self.expires_at,
        )
    }

    /// Open a v2 hint set from the expected host.
    ///
    /// # Errors
    /// Refuses another signer or recipient and invalid sets.
    pub fn open(event: &Event, device: &SecretKey, host: &str) -> Result<Self> {
        let reader = pubkey(device);
        let (body, sealed): (Self, _) = artifact::open(event, device, host, &reader, SCHEMA)?;
        if body.host != host
            || body.issued_at != sealed.issued_at
            || body.expires_at != sealed.retain_until
        {
            return fail(Refusal::IdentityMismatch, "hint set and envelope differ");
        }
        body.validate()?;
        Ok(body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hints::{Locality, Transport};
    use crate::presence::tests::key;

    fn iroh(direct: &[&str], relay: Option<&str>) -> IrohHint {
        IrohHint {
            class: Class::Public,
            transport: IrohTransport::Iroh,
            address: "ab".repeat(32),
            relay: relay.map(str::to_owned),
            direct: direct.iter().map(|a| (*a).to_owned()).collect(),
            status: Status::Reachable,
            observed_at: 100,
        }
    }

    fn v1() -> Hints {
        Hints {
            v: crate::hints::SCHEMA.into(),
            requires: vec![],
            host: pubkey(&key(2)),
            generation: 7,
            issued_at: 100,
            expires_at: 1000,
            hints: vec![Hint {
                class: Class::Relay,
                transport: Transport::Nostr,
                address: "wss://relay.example".into(),
                status: Status::Unknown,
                observed_at: 100,
            }],
            meta: None,
        }
    }

    #[test]
    fn a_v2_record_carries_one_iroh_hint_and_reads_as_v1_without_it() {
        let hint = iroh(
            &["192.168.1.20:4100", "127.0.0.1:4100", "[2001:db8::1]:4100"],
            Some("https://iroh.openagents.com"),
        );
        let record = HintsV2::from_v1(&v1(), Some(hint.clone()));
        record.validate().unwrap();
        assert_eq!(record.iroh(), Some(&hint));
        assert_eq!(record.v1(), v1());
        // The wire spelling: transport `iroh`, a null relay allowed.
        let text = serde_json::to_string(&record).unwrap();
        assert!(text.contains(r#""transport":"iroh""#), "{text}");
        let back: HintsV2 = serde_json::from_str(&text).unwrap();
        assert_eq!(back, record);
        // A client on another machine never dials loopback.
        assert_eq!(hint.dialable(Locality::OtherMachine).len(), 2);
        assert_eq!(hint.dialable(Locality::SameMachine).len(), 3);
        // A v1 reader refuses the whole record.
        assert!(serde_json::from_str::<Hints>(&text).is_err());

        let host = key(2);
        let device = key(3);
        let event = record
            .seal(&host, &pubkey(&device), &crate::new_id())
            .unwrap();
        let opened = HintsV2::open(&event, &device, &pubkey(&host)).unwrap();
        assert_eq!(opened, record);
        assert!(Hints::open(&event, &device, &pubkey(&host)).is_err());
    }

    #[test]
    fn malformed_iroh_hints_refuse_the_record() {
        let bad = [
            ("uppercase ID", {
                let mut h = iroh(&["192.168.1.20:4100"], None);
                h.address = "AB".repeat(32);
                h
            }),
            ("short ID", {
                let mut h = iroh(&["192.168.1.20:4100"], None);
                h.address = "ab".repeat(31);
                h
            }),
            ("another class", {
                let mut h = iroh(&["192.168.1.20:4100"], None);
                h.class = Class::Lan;
                h
            }),
            (
                "relay with a query",
                iroh(&[], Some("https://iroh.example/?a=1")),
            ),
            ("relay over the bound", {
                let long = format!("https://{}.example", "a".repeat(120));
                iroh(&[], Some(&long))
            }),
            ("ws relay", iroh(&[], Some("wss://iroh.example"))),
            ("nothing to dial", iroh(&[], None)),
            ("zero port", iroh(&["192.168.1.20:0"], None)),
            ("unspecified", iroh(&["0.0.0.0:4100"], None)),
            ("multicast", iroh(&["224.0.0.251:5353"], None)),
            ("broadcast", iroh(&["255.255.255.255:4100"], None)),
            (
                "duplicate",
                iroh(&["192.168.1.20:4100", "192.168.1.20:4100"], None),
            ),
            ("not canonical", iroh(&["192.168.001.020:4100"], None)),
            (
                "nine addresses",
                iroh(
                    &(1..=9)
                        .map(|n| format!("192.168.1.{n}:4100"))
                        .collect::<Vec<_>>()
                        .iter()
                        .map(String::as_str)
                        .collect::<Vec<_>>(),
                    None,
                ),
            ),
        ];
        for (case, hint) in bad {
            let record = HintsV2::from_v1(&v1(), Some(hint));
            assert!(record.validate().is_err(), "{case}");
        }
        let mut two = HintsV2::from_v1(&v1(), Some(iroh(&["192.168.1.20:4100"], None)));
        two.hints
            .push(AnyHint::Iroh(iroh(&["192.168.1.21:4100"], None)));
        assert!(two.validate().is_err(), "two iroh hints");
        let loopback_relay =
            HintsV2::from_v1(&v1(), Some(iroh(&[], Some("http://127.0.0.1:3340"))));
        loopback_relay.validate().unwrap();
    }
}
