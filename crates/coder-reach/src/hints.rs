//! Reachability hints: host-signed endpoints a device may try.
//!
//! A hint grants nothing and proves nothing. Only a completed direct-channel
//! handshake, or an authenticated relay exchange, proves a route. Selection
//! never offers a loopback endpoint to a client on another machine.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use nostr::domain::Event;
use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{Refusal, Result, artifact, fail, parse_pubkey, pubkey, relay_url, requires};

/// Schema of a hint set.
pub const SCHEMA: &str = "openagents.reach-hints.v1";
/// Most hints one set carries.
pub const MAX_HINTS: usize = 16;
/// Largest address, in bytes.
pub const MAX_ADDRESS_BYTES: usize = 512;
/// Longest a hint set may claim validity, in seconds.
pub const MAX_LIFETIME: u64 = 24 * 60 * 60;

/// Where an endpoint is reachable from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Class {
    Loopback,
    Lan,
    Tailnet,
    Public,
    Relay,
}

/// How a client speaks to the endpoint.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Transport {
    /// A direct channel over TCP; the address is `host:port`.
    Tcp,
    /// A direct channel over WebSocket; the address is a `ws` or `wss` URL.
    Websocket,
    /// Relay-carried control; the address is a relay URL.
    Nostr,
}

/// What the host last observed about the endpoint. Advisory only.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Reachable,
    Unreachable,
    Unknown,
}

/// One endpoint hint.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hint {
    pub class: Class,
    pub transport: Transport,
    pub address: String,
    pub status: Status,
    pub observed_at: u64,
}

/// A host-signed hint set, bound to one host generation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hints {
    pub v: String,
    pub requires: Vec<String>,
    pub host: String,
    pub generation: u64,
    pub issued_at: u64,
    pub expires_at: u64,
    pub hints: Vec<Hint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

/// Whether the client runs on the host's machine. A client claims
/// `SameMachine` only from local evidence, such as reaching the host through
/// a local socket its own user owns; a matching address is not evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Locality {
    SameMachine,
    OtherMachine,
}

impl Hint {
    /// Check the address against its transport and class.
    ///
    /// # Errors
    /// Refuses malformed addresses, credentials or queries in URLs, unspecified
    /// addresses, and a class that disagrees with the address or transport.
    pub fn validate(&self) -> Result<()> {
        if self.address.len() > MAX_ADDRESS_BYTES || !self.address.is_ascii() {
            return fail(Refusal::Malformed, "hint address");
        }
        if (self.class == Class::Relay) != (self.transport == Transport::Nostr) {
            return fail(
                Refusal::Malformed,
                "relay class requires the nostr transport",
            );
        }
        let loopback = match self.transport {
            Transport::Tcp => tcp_host(&self.address)?,
            Transport::Websocket => websocket_host(&self.address)?,
            Transport::Nostr => {
                relay_url(&self.address, true)?;
                websocket_host(&self.address)?
            }
        };
        if self.class == Class::Loopback && !loopback {
            return fail(
                Refusal::Malformed,
                "loopback class needs a loopback address",
            );
        }
        if matches!(self.class, Class::Lan | Class::Tailnet | Class::Public) && loopback {
            return fail(
                Refusal::Malformed,
                "a loopback address cannot use a shareable class",
            );
        }
        Ok(())
    }

    /// Whether the address names this machine.
    #[must_use]
    pub fn is_loopback(&self) -> bool {
        match self.transport {
            Transport::Tcp => tcp_host(&self.address).unwrap_or(true),
            Transport::Websocket | Transport::Nostr => {
                websocket_host(&self.address).unwrap_or(true)
            }
        }
    }

    /// Direct transports carry a direct channel; `nostr` is relay fallback.
    #[must_use]
    pub const fn is_direct(&self) -> bool {
        !matches!(self.transport, Transport::Nostr)
    }
}

impl Hints {
    /// Check the closed schema, bounds, and every hint.
    ///
    /// # Errors
    /// Refuses unknown versions or features, bad lifetimes, duplicates, and
    /// invalid hints.
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
        for (index, hint) in self.hints.iter().enumerate() {
            hint.validate()?;
            if hint.observed_at > self.issued_at {
                return fail(Refusal::Malformed, "hint observed after issue");
            }
            if self.hints[..index]
                .iter()
                .any(|old| old.transport == hint.transport && old.address == hint.address)
            {
                return fail(Refusal::Malformed, "duplicate hint");
            }
        }
        Ok(())
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

    /// Open a hint set from the expected host.
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

/// Order the endpoints a client should try, best first.
///
/// Direct classes come before relay fallback. Within a class, `reachable`
/// comes before `unknown`, and `unreachable` hints are skipped. A client on
/// another machine never receives a loopback endpoint, in any class, so an
/// empty result means "no shareable route", never "try loopback".
///
/// # Errors
/// Refuses as `stale` when the set is expired or names another generation.
pub fn select(hints: &Hints, locality: Locality, generation: u64, now: u64) -> Result<Vec<&Hint>> {
    hints.validate()?;
    if hints.generation != generation {
        return fail(Refusal::Stale, "hints name another host generation");
    }
    if now >= hints.expires_at {
        return fail(Refusal::Stale, "hint set expired");
    }
    let mut chosen: Vec<&Hint> = hints
        .hints
        .iter()
        .filter(|hint| hint.status != Status::Unreachable)
        .filter(|hint| locality == Locality::SameMachine || !hint.is_loopback())
        .collect();
    chosen.sort_by_key(|hint| (hint.class, hint.status != Status::Reachable));
    Ok(chosen)
}

pub(crate) fn host_is_loopback(host: Option<url::Host<&str>>) -> bool {
    match host {
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        Some(url::Host::Domain(name)) => is_localhost_name(name),
        None => false,
    }
}

fn is_localhost_name(name: &str) -> bool {
    let name = name.trim_end_matches('.').to_ascii_lowercase();
    name == "localhost" || name.ends_with(".localhost")
}

fn check_ip(ip: IpAddr) -> Result<bool> {
    if ip.is_unspecified() || ip.is_multicast() {
        return fail(Refusal::Malformed, "unspecified or multicast address");
    }
    if let IpAddr::V4(v4) = ip
        && v4 == Ipv4Addr::BROADCAST
    {
        return fail(Refusal::Malformed, "broadcast address");
    }
    Ok(ip.is_loopback())
}

/// Parse `host:port`; returns whether the host is loopback.
fn tcp_host(address: &str) -> Result<bool> {
    let (host, port) = address
        .rsplit_once(':')
        .ok_or_else(|| crate::Error::new(Refusal::Malformed, "tcp address needs a port"))?;
    match port.parse::<u16>() {
        Ok(p) if p != 0 && !port.starts_with('0') => {}
        _ => return fail(Refusal::Malformed, "tcp port"),
    }
    if let Some(inner) = host.strip_prefix('[').and_then(|h| h.strip_suffix(']')) {
        let ip: Ipv6Addr = inner
            .parse()
            .map_err(|_| crate::Error::new(Refusal::Malformed, "IPv6 address"))?;
        return check_ip(IpAddr::V6(ip));
    }
    if let Ok(ip) = host.parse::<Ipv4Addr>() {
        return check_ip(IpAddr::V4(ip));
    }
    dns_name(host)?;
    Ok(is_localhost_name(host))
}

fn websocket_host(address: &str) -> Result<bool> {
    let url = url::Url::parse(address).map_err(|_| crate::Error::new(Refusal::Malformed, "URL"))?;
    if !matches!(url.scheme(), "ws" | "wss")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return fail(
            Refusal::Malformed,
            "websocket hint must be a credential-free ws or wss URL without query",
        );
    }
    match url.host() {
        Some(url::Host::Ipv4(ip)) => check_ip(IpAddr::V4(ip)),
        Some(url::Host::Ipv6(ip)) => check_ip(IpAddr::V6(ip)),
        Some(url::Host::Domain(name)) => Ok(is_localhost_name(name)),
        None => fail(Refusal::Malformed, "URL host"),
    }
}

fn dns_name(name: &str) -> Result<()> {
    let trimmed = name.strip_suffix('.').unwrap_or(name);
    let ok = !trimmed.is_empty()
        && trimmed.len() <= 253
        && trimmed.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        });
    if ok {
        Ok(())
    } else {
        fail(Refusal::Malformed, "DNS name")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::new_id;
    use crate::presence::tests::key;

    fn hint(class: Class, transport: Transport, address: &str, status: Status) -> Hint {
        Hint {
            class,
            transport,
            address: address.into(),
            status,
            observed_at: 100,
        }
    }

    fn set(hints: Vec<Hint>) -> Hints {
        Hints {
            v: SCHEMA.into(),
            requires: vec![],
            host: pubkey(&key(2)),
            generation: 7,
            issued_at: 100,
            expires_at: 1000,
            hints,
            meta: None,
        }
    }

    #[test]
    fn selection_with_shareable_endpoints() {
        let hints = set(vec![
            hint(
                Class::Relay,
                Transport::Nostr,
                "wss://relay.example",
                Status::Reachable,
            ),
            hint(
                Class::Public,
                Transport::Tcp,
                "203.0.113.5:4100",
                Status::Unknown,
            ),
            hint(
                Class::Loopback,
                Transport::Tcp,
                "127.0.0.1:4100",
                Status::Reachable,
            ),
            hint(
                Class::Tailnet,
                Transport::Tcp,
                "100.101.102.103:4100",
                Status::Reachable,
            ),
            hint(
                Class::Lan,
                Transport::Tcp,
                "192.168.1.20:4100",
                Status::Unknown,
            ),
            hint(
                Class::Lan,
                Transport::Websocket,
                "ws://192.168.1.20:4101/reach",
                Status::Reachable,
            ),
            hint(
                Class::Public,
                Transport::Tcp,
                "198.51.100.9:4100",
                Status::Unreachable,
            ),
        ]);
        let remote = select(&hints, Locality::OtherMachine, 7, 500).unwrap();
        let addresses: Vec<&str> = remote.iter().map(|h| h.address.as_str()).collect();
        assert_eq!(
            addresses,
            vec![
                "ws://192.168.1.20:4101/reach",
                "192.168.1.20:4100",
                "100.101.102.103:4100",
                "203.0.113.5:4100",
                "wss://relay.example",
            ]
        );
        assert!(remote.iter().all(|h| !h.is_loopback()));
        let local = select(&hints, Locality::SameMachine, 7, 500).unwrap();
        assert_eq!(local[0].address, "127.0.0.1:4100");
        assert_eq!(local.last().unwrap().class, Class::Relay);
    }

    #[test]
    fn selection_without_shareable_endpoints_never_falls_back_to_loopback() {
        let only_loopback = set(vec![
            hint(
                Class::Loopback,
                Transport::Tcp,
                "127.0.0.1:4100",
                Status::Reachable,
            ),
            hint(
                Class::Loopback,
                Transport::Websocket,
                "ws://localhost:4101",
                Status::Reachable,
            ),
        ]);
        assert!(
            select(&only_loopback, Locality::OtherMachine, 7, 500)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            select(&only_loopback, Locality::SameMachine, 7, 500)
                .unwrap()
                .len(),
            2
        );
        // A loopback relay is also withheld from another machine; only the
        // shareable relay remains.
        let with_relays = set(vec![
            hint(
                Class::Loopback,
                Transport::Tcp,
                "127.0.0.1:4100",
                Status::Reachable,
            ),
            hint(
                Class::Relay,
                Transport::Nostr,
                "ws://127.0.0.1:7000",
                Status::Reachable,
            ),
            hint(
                Class::Relay,
                Transport::Nostr,
                "wss://relay.example",
                Status::Unknown,
            ),
        ]);
        let remote = select(&with_relays, Locality::OtherMachine, 7, 500).unwrap();
        assert_eq!(remote.len(), 1);
        assert_eq!(remote[0].address, "wss://relay.example");
    }

    #[test]
    fn stale_hints_refuse() {
        let hints = set(vec![hint(
            Class::Lan,
            Transport::Tcp,
            "10.0.0.2:4100",
            Status::Unknown,
        )]);
        assert_eq!(
            select(&hints, Locality::OtherMachine, 8, 500)
                .unwrap_err()
                .code,
            Refusal::Stale
        );
        assert_eq!(
            select(&hints, Locality::OtherMachine, 7, 1000)
                .unwrap_err()
                .code,
            Refusal::Stale
        );
    }

    #[test]
    fn mislabeled_and_unsafe_hints_refuse() {
        for bad in [
            hint(
                Class::Lan,
                Transport::Tcp,
                "127.0.0.1:4100",
                Status::Unknown,
            ),
            hint(Class::Public, Transport::Tcp, "[::1]:4100", Status::Unknown),
            hint(
                Class::Lan,
                Transport::Tcp,
                "localhost:4100",
                Status::Unknown,
            ),
            hint(
                Class::Loopback,
                Transport::Tcp,
                "192.168.1.2:4100",
                Status::Unknown,
            ),
            hint(Class::Lan, Transport::Tcp, "0.0.0.0:4100", Status::Unknown),
            hint(Class::Lan, Transport::Tcp, "10.0.0.2", Status::Unknown),
            hint(Class::Lan, Transport::Tcp, "10.0.0.2:0", Status::Unknown),
            hint(
                Class::Lan,
                Transport::Websocket,
                "ws://10.0.0.2:1/?token=secret",
                Status::Unknown,
            ),
            hint(
                Class::Lan,
                Transport::Websocket,
                "ws://user:pw@10.0.0.2:1/",
                Status::Unknown,
            ),
            hint(
                Class::Relay,
                Transport::Tcp,
                "10.0.0.2:4100",
                Status::Unknown,
            ),
            hint(
                Class::Lan,
                Transport::Nostr,
                "wss://relay.example",
                Status::Unknown,
            ),
            hint(
                Class::Relay,
                Transport::Nostr,
                "ws://relay.example",
                Status::Unknown,
            ),
        ] {
            assert!(bad.validate().is_err(), "{bad:?}");
        }
        hint(
            Class::Lan,
            Transport::Tcp,
            "[fd00::2]:4100",
            Status::Unknown,
        )
        .validate()
        .unwrap();
        hint(
            Class::Tailnet,
            Transport::Tcp,
            "desk.tailnet-name.example:4100",
            Status::Unknown,
        )
        .validate()
        .unwrap();
    }

    #[test]
    fn hint_round_trip() {
        let hints = set(vec![hint(
            Class::Lan,
            Transport::Tcp,
            "10.0.0.2:4100",
            Status::Reachable,
        )]);
        let device = key(9);
        let event = hints.seal(&key(2), &pubkey(&device), &new_id()).unwrap();
        assert_eq!(
            Hints::open(&event, &device, &pubkey(&key(2))).unwrap(),
            hints
        );
        assert!(Hints::open(&event, &device, &pubkey(&key(3))).is_err());
        assert!(hints.seal(&key(3), &pubkey(&device), &new_id()).is_err());
    }
}
