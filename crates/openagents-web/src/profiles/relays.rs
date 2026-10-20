//! Reading and writing public profile events on relays, for `/@{name}`.
//!
//! A read opens a short public connection to each relay (no NIP-42), sends
//! one `REQ`, keeps the events that arrive before `EOSE` or the deadline,
//! and drops any whose id or signature doesn't check. Relays a person
//! lists (kind 10002) are read too, but only `wss://` relays whose host
//! resolves to public addresses, so a profile can't point this server at
//! its own network.

use std::net::IpAddr;
use std::time::Duration;

use nostr::domain::Event;
use nostr_transport::Connection;
use serde_json::{Value, json};

/// How long one relay read may take.
pub(crate) const READ_WAIT: Duration = Duration::from_millis(2_500);
/// How long one publish may take.
pub(crate) const PUBLISH_WAIT: Duration = Duration::from_secs(5);
/// The most events one read keeps.
const EVENTS_MAX: usize = 500;

/// Whether `ip` is an address on the public internet.
pub(crate) fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let [a, b, ..] = v4.octets();
            !(v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_documentation()
                || v4.is_unspecified()
                || v4.is_multicast()
                || a == 0
                || (a == 100 && (64..128).contains(&b))
                || (a == 192 && b == 0)
                || (a == 198 && (18..20).contains(&b))
                || a >= 240)
        }
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return public_ip(IpAddr::V4(v4));
            }
            let first = v6.segments()[0];
            !(v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || (first & 0xfe00) == 0xfc00
                || (first & 0xffc0) == 0xfe80
                || first == 0x2001 && v6.segments()[1] == 0x0db8)
        }
    }
}

/// The host and port of a `wss://` or `https://` URL.
pub(crate) fn host_port(url: &str, scheme: &str, default_port: u16) -> Option<(String, u16)> {
    let rest = url.strip_prefix(scheme)?;
    let authority = rest.split(['/', '?', '#']).next()?;
    if authority.is_empty() || authority.contains('@') {
        return None;
    }
    if let Some(v6) = authority.strip_prefix('[') {
        let (host, after) = v6.split_once(']')?;
        let port = match after.strip_prefix(':') {
            Some(port) => port.parse().ok()?,
            None if after.is_empty() => default_port,
            None => return None,
        };
        return Some((host.to_string(), port));
    }
    match authority.rsplit_once(':') {
        Some((host, port)) => Some((host.to_ascii_lowercase(), port.parse().ok()?)),
        None => Some((authority.to_ascii_lowercase(), default_port)),
    }
}

/// Whether every address `host` resolves to is public.
pub(crate) async fn resolves_public(host: &str, port: u16) -> bool {
    match tokio::time::timeout(
        Duration::from_secs(2),
        tokio::net::lookup_host((host, port)),
    )
    .await
    {
        Ok(Ok(addresses)) => {
            let addresses: Vec<_> = addresses.collect();
            !addresses.is_empty() && addresses.iter().all(|a| public_ip(a.ip()))
        }
        _ => false,
    }
}

/// Whether this server may read a relay a profile listed.
pub(crate) async fn listed_relay_allowed(url: &str) -> bool {
    match host_port(url, "wss://", 443) {
        Some((host, port)) => resolves_public(&host, port).await,
        None => false,
    }
}

/// The events `relay` holds for `filter`, with valid ids and signatures.
/// A relay that can't be reached answers nothing.
pub(crate) async fn read(relay: &str, filter: Value, wait: Duration) -> Vec<Event> {
    let Ok(mut connection) = Connection::connect_open(relay, wait).await else {
        return Vec::new();
    };
    let mut connection_events = Vec::new();
    let subscription = "profile";
    if connection
        .send(json!(["REQ", subscription, filter]))
        .await
        .is_err()
    {
        return Vec::new();
    }
    while connection_events.len() < EVENTS_MAX {
        let Ok(frame) = connection.next().await else {
            break;
        };
        match frame.get(0).and_then(Value::as_str) {
            Some("EVENT") if frame.get(1) == Some(&json!(subscription)) => {
                if let Some(event) = frame
                    .get(2)
                    .and_then(|e| serde_json::from_value::<Event>(e.clone()).ok())
                    && event.validate_crypto().is_ok()
                {
                    connection_events.push(event);
                }
            }
            Some("EOSE" | "CLOSED") => break,
            _ => {}
        }
    }
    let _ = connection.send(json!(["CLOSE", subscription])).await;
    let _ = connection.close().await;
    connection_events
}

/// Publish `event` to `relay`. `Ok(())` once the relay said `OK true`.
pub(crate) async fn publish(relay: &str, event: &Event) -> Result<(), String> {
    let mut connection = Connection::connect_open(relay, PUBLISH_WAIT).await?;
    connection.send(json!(["EVENT", event])).await?;
    loop {
        let frame = connection.next().await?;
        match frame.get(0).and_then(Value::as_str) {
            Some("OK") if frame.get(1) == Some(&json!(event.id)) => {
                let _ = connection.close().await;
                return if frame.get(2) == Some(&json!(true)) {
                    Ok(())
                } else {
                    Err(frame
                        .get(3)
                        .and_then(Value::as_str)
                        .unwrap_or("refused")
                        .to_string())
                };
            }
            // A relay that asks for NIP-42 still takes public profiles on
            // many setups; this server never signs for a person, so it
            // keeps waiting for the answer to the event.
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_public_addresses_pass() {
        for private in [
            "127.0.0.1",
            "10.128.0.46",
            "192.168.1.1",
            "172.16.0.1",
            "169.254.169.254",
            "100.64.0.1",
            "0.0.0.0",
            "::1",
            "fd00::1",
            "fe80::1",
            "::ffff:127.0.0.1",
        ] {
            assert!(!public_ip(private.parse().unwrap()), "{private}");
        }
        for public in ["1.1.1.1", "8.8.8.8", "2606:4700::1111"] {
            assert!(public_ip(public.parse().unwrap()), "{public}");
        }
    }

    #[test]
    fn hosts_and_ports_are_read_strictly() {
        assert_eq!(
            host_port("wss://Relay.Example.com/x", "wss://", 443),
            Some(("relay.example.com".into(), 443))
        );
        assert_eq!(
            host_port("wss://r.test:7447", "wss://", 443),
            Some(("r.test".into(), 7447))
        );
        assert_eq!(
            host_port("wss://[::1]:80", "wss://", 443),
            Some(("::1".into(), 80))
        );
        assert_eq!(host_port("wss://user@r.test", "wss://", 443), None);
        assert_eq!(host_port("ws://r.test", "wss://", 443), None);
        assert_eq!(host_port("wss://", "wss://", 443), None);
    }

    #[tokio::test]
    async fn local_relays_a_profile_lists_are_not_read() {
        assert!(!listed_relay_allowed("wss://127.0.0.1:7447").await);
        assert!(!listed_relay_allowed("wss://localhost").await);
        assert!(!listed_relay_allowed("ws://relay.example.com").await);
    }
}
