//! `openagents pylon verify`: a pass/fail checklist for a running pylon.
//!
//! 1. The engine answers `/health` on loopback.
//! 2. This pylon's beacon is visible on the relay from a fresh connection
//!    (a new throwaway key, so nothing cached on this pylon's own
//!    connection counts).
//! 3. The owner link, when there is one, verifies for this pylon key.
//! 4. The trusted checkers' latest verdicts on this pylon are not a fail.
//! 5. This computer's clock is within 30 s of the relay's (its HTTP `Date`).
//! 6. Nothing answers on the Psionic ports beyond loopback.
//!
//! An item that cannot be judged (no owner link, no verdicts yet, a relay
//! that sends no `Date`) is `skip`, which does not fail the checklist.

use std::collections::BTreeSet;
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use nostr::domain::MintedOwnerAttestation;
use nostr::pylon::{Freshness, Status, Verdict, freshness};
use serde::Serialize;

use crate::identity::Identity;
use crate::relay;

/// Clock skew against the relay above this fails.
pub const MAX_SKEW_SECS: u64 = 30;
/// How far back verdicts are read.
pub const VERDICT_WINDOW_SECS: u64 = 7 * 24 * 3600;
/// The text lane's default loopback port.
pub const TEXT_PORT: u16 = 18080;

/// An item's result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Pass,
    Fail,
    Skip,
}

/// One checklist line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Item {
    /// `engine`, `beacon`, `owner_link`, `checks`, `clock`, `loopback`.
    pub id: &'static str,
    pub state: State,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<String>,
}

impl Item {
    fn new(id: &'static str, state: State, detail: impl Into<String>) -> Self {
        Self {
            id,
            state,
            detail: detail.into(),
            fix: None,
        }
    }

    fn fix(mut self, fix: &str) -> Self {
        self.fix = Some(fix.into());
        self
    }
}

/// The whole checklist.
#[derive(Debug, Clone, Serialize)]
pub struct Checklist {
    /// No item failed.
    pub pass: bool,
    pub items: Vec<Item>,
}

/// What `verify` checks against.
pub struct Inputs<'a> {
    pub relay: &'a str,
    /// The decision engine, e.g. `http://127.0.0.1:18096`.
    pub engine: &'a str,
    /// This pylon's key.
    pub pylon: &'a Identity,
    pub owner: Option<&'a MintedOwnerAttestation>,
    pub checkers: &'a BTreeSet<String>,
    /// Loopback ports that must not answer beyond loopback.
    pub ports: Vec<u16>,
    /// This computer's non-loopback addresses; `None` reads them.
    pub addresses: Option<Vec<IpAddr>>,
    pub now: u64,
}

/// Run every check.
pub async fn run(inputs: Inputs<'_>) -> Checklist {
    let mut items = vec![engine(inputs.engine).await];
    items.extend(on_relay(&inputs).await);
    items.push(owner_link(inputs.pylon, inputs.owner));
    items.push(clock(inputs.relay, inputs.now).await);
    let addresses = inputs.addresses.clone().unwrap_or_else(local_addresses);
    items.push(loopback(&addresses, &inputs.ports).await);
    let order = [
        "engine",
        "beacon",
        "owner_link",
        "checks",
        "clock",
        "loopback",
    ];
    items.sort_by_key(|item| order.iter().position(|id| *id == item.id));
    Checklist {
        pass: items.iter().all(|i| i.state != State::Fail),
        items,
    }
}

async fn engine(url: &str) -> Item {
    let health = format!("{}/health", url.trim_end_matches('/'));
    let answered = match reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
    {
        Ok(client) => client
            .get(&health)
            .send()
            .await
            .is_ok_and(|r| r.status().is_success()),
        Err(_) => false,
    };
    if answered {
        Item::new(
            "engine",
            State::Pass,
            format!("the engine answers at {url}"),
        )
    } else {
        Item::new(
            "engine",
            State::Fail,
            format!("the engine does not answer at {health}"),
        )
        .fix("openagents pylon start --allow-any")
    }
}

/// The beacon and the verdicts, read over one fresh connection.
async fn on_relay(inputs: &Inputs<'_>) -> Vec<Item> {
    let reader = Identity::generate();
    let mut conn = match relay::connect(inputs.relay, &reader, Duration::from_secs(30)).await {
        Ok(conn) => conn,
        Err(e) => {
            return vec![
                Item::new(
                    "beacon",
                    State::Fail,
                    format!("cannot reach the relay: {e}"),
                )
                .fix("check this computer's internet connection"),
                Item::new("checks", State::Skip, "the relay could not be read"),
            ];
        }
    };
    let pubkey = inputs.pylon.pubkey().to_string();
    let beacon = match crate::client::beacons(&mut conn, Some(std::slice::from_ref(&pubkey))).await
    {
        Ok(book) => {
            let newest = book
                .iter()
                .map(|(_, b)| b.clone())
                .max_by_key(|b| b.observed_at);
            beacon_item(newest.as_ref(), inputs.now)
        }
        Err(e) => Item::new(
            "beacon",
            State::Fail,
            format!("the relay query failed: {e}"),
        ),
    };
    let checks = match crate::check::fetch(
        &mut conn,
        inputs.checkers,
        inputs.now.saturating_sub(VERDICT_WINDOW_SECS),
    )
    .await
    {
        Ok((labels, receipts)) => {
            let verdicts = crate::check::Verdicts::new(&labels, &receipts, inputs.checkers);
            let mine: Vec<Verdict> = verdicts
                .checks
                .iter()
                .filter(|c| c.provider == pubkey)
                .map(|c| c.verdict)
                .collect();
            checks_item(&mine)
        }
        Err(e) => Item::new("checks", State::Skip, format!("verdicts unreadable: {e}")),
    };
    let _ = conn.close().await;
    vec![beacon, checks]
}

/// Judge the newest beacon seen from outside.
#[must_use]
pub fn beacon_item(newest: Option<&nostr::pylon::Beacon>, now: u64) -> Item {
    let Some(beacon) = newest else {
        return Item::new(
            "beacon",
            State::Fail,
            "no beacon from this pylon is on the relay",
        )
        .fix("openagents pylon start --allow-any");
    };
    match (freshness(beacon, now), beacon.status) {
        (Freshness::Fresh, Status::Offline) => Item::new(
            "beacon",
            State::Fail,
            "the relay shows this pylon as offline",
        )
        .fix("openagents pylon start --allow-any"),
        (Freshness::Fresh, Status::Draining) => Item::new(
            "beacon",
            State::Pass,
            format!(
                "visible on the relay as paused ({}s old)",
                now.saturating_sub(beacon.observed_at)
            ),
        ),
        (Freshness::Fresh, _) => Item::new(
            "beacon",
            State::Pass,
            format!(
                "visible on the relay as serving ({}s old)",
                now.saturating_sub(beacon.observed_at)
            ),
        ),
        (Freshness::Future, _) => Item::new(
            "beacon",
            State::Fail,
            "the newest beacon is dated in the future; check this computer's clock",
        ),
        (_, _) => Item::new(
            "beacon",
            State::Fail,
            format!(
                "the newest beacon is stale ({}s old)",
                now.saturating_sub(beacon.observed_at)
            ),
        )
        .fix("openagents pylon start --allow-any"),
    }
}

/// Judge this pylon's counted verdicts, oldest first.
#[must_use]
pub fn checks_item(verdicts: &[Verdict]) -> Item {
    let Some(last) = verdicts.last() else {
        return Item::new(
            "checks",
            State::Skip,
            "no trusted checker has checked this pylon in the last 7 days",
        );
    };
    let count = |v: Verdict| verdicts.iter().filter(|x| **x == v).count();
    let tally = format!(
        "{} passed, {} failed, {} inconclusive in the last 7 days",
        count(Verdict::Pass),
        count(Verdict::Fail),
        count(Verdict::Inconclusive)
    );
    if *last == Verdict::Fail {
        Item::new(
            "checks",
            State::Fail,
            format!("the latest check failed; {tally}"),
        )
        .fix("openagents pylon selfcheck")
    } else {
        Item::new("checks", State::Pass, tally)
    }
}

/// Whether the owner link fits this pylon key.
#[must_use]
pub fn owner_link(pylon: &Identity, owner: Option<&MintedOwnerAttestation>) -> Item {
    match owner {
        None => Item::new("owner_link", State::Skip, "not linked to an owner")
            .fix("openagents pylon link --owner-secret FILE"),
        Some(credential) => match crate::identity::check_owner(pylon, credential) {
            Ok(()) => Item::new(
                "owner_link",
                State::Pass,
                format!(
                    "linked to {}",
                    crate::identity::npub(&credential.owner_pubkey)
                ),
            ),
            Err(e) => Item::new("owner_link", State::Fail, e)
                .fix("openagents pylon link --owner-secret FILE"),
        },
    }
}

async fn clock(relay: &str, now: u64) -> Item {
    let url = relay
        .replacen("wss://", "https://", 1)
        .replacen("ws://", "http://", 1);
    let date = match reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
    {
        Ok(client) => client
            .get(&url)
            .header("accept", "application/nostr+json")
            .send()
            .await
            .ok()
            .and_then(|r| {
                r.headers()
                    .get(reqwest::header::DATE)
                    .and_then(|v| v.to_str().ok())
                    .and_then(parse_http_date)
            }),
        Err(_) => None,
    };
    clock_item(date, now)
}

/// Judge the skew between the relay's `Date` and this computer's clock.
#[must_use]
pub fn clock_item(relay_time: Option<u64>, now: u64) -> Item {
    let Some(at) = relay_time else {
        return Item::new("clock", State::Skip, "the relay sent no time to compare");
    };
    let skew = now.abs_diff(at);
    if skew < MAX_SKEW_SECS {
        Item::new("clock", State::Pass, format!("within {skew}s of the relay"))
    } else {
        Item::new(
            "clock",
            State::Fail,
            format!("{skew}s off the relay's clock; beacons over 30s off are refused"),
        )
        .fix("turn on automatic time in this computer's settings")
    }
}

/// An HTTP date (`Sun, 19 Oct 2026 12:00:00 GMT`) as Unix seconds.
#[must_use]
pub fn parse_http_date(text: &str) -> Option<u64> {
    let mut words = text.split_whitespace().skip(1);
    let day: u64 = words.next()?.parse().ok()?;
    let month = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ]
    .iter()
    .position(|m| Some(*m) == words.clone().next())? as u64
        + 1;
    words.next();
    let year: i64 = words.next()?.parse().ok()?;
    let mut clock = words.next()?.split(':').map(str::parse::<u64>);
    let (h, m, s) = (
        clock.next()?.ok()?,
        clock.next()?.ok()?,
        clock.next()?.ok()?,
    );
    if words.next()? != "GMT" || !(1..=31).contains(&day) || h > 23 || m > 59 || s > 60 {
        return None;
    }
    // Days from 1970-01-01 (Howard Hinnant's days_from_civil).
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    u64::try_from(days * 86_400)
        .ok()
        .map(|d| d + h * 3600 + m * 60 + s)
}

/// This computer's outward-facing addresses (the one a default route
/// would use; no packet is sent).
#[must_use]
pub fn local_addresses() -> Vec<IpAddr> {
    let mut out = Vec::new();
    for (bind, probe) in [("0.0.0.0:0", "192.0.2.1:9"), ("[::]:0", "[2001:db8::1]:9")] {
        if let Ok(socket) = std::net::UdpSocket::bind(bind)
            && socket.connect(probe).is_ok()
            && let Ok(addr) = socket.local_addr()
            && !addr.ip().is_loopback()
            && !addr.ip().is_unspecified()
        {
            out.push(addr.ip());
        }
    }
    out
}

/// Fail when any of `ports` accepts a connection on any of `addresses`.
pub async fn loopback(addresses: &[IpAddr], ports: &[u16]) -> Item {
    let mut open = Vec::new();
    for ip in addresses {
        for port in ports {
            let addr = SocketAddr::new(*ip, *port);
            if matches!(
                tokio::time::timeout(Duration::from_secs(1), tokio::net::TcpStream::connect(addr))
                    .await,
                Ok(Ok(_))
            ) {
                open.push(addr.to_string());
            }
        }
    }
    let ports_text = ports
        .iter()
        .map(u16::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    if !open.is_empty() {
        Item::new(
            "loopback",
            State::Fail,
            format!("the engine answers beyond loopback at {}", open.join(", ")),
        )
        .fix("restart the engine on 127.0.0.1 (openagents pylon start does)")
    } else if addresses.is_empty() {
        Item::new(
            "loopback",
            State::Pass,
            format!("no network address here; ports {ports_text} cannot be reached from outside"),
        )
    } else {
        Item::new(
            "loopback",
            State::Pass,
            format!("nothing answers on ports {ports_text} beyond loopback"),
        )
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::sync::Arc;

    use super::*;

    #[test]
    fn http_dates_parse() {
        assert_eq!(parse_http_date("Thu, 01 Jan 1970 00:00:00 GMT"), Some(0));
        assert_eq!(
            parse_http_date("Sun, 06 Nov 1994 08:49:37 GMT"),
            Some(784_111_777)
        );
        assert_eq!(
            parse_http_date("Mon, 19 Oct 2026 12:00:00 GMT"),
            Some(1_792_411_200)
        );
        assert_eq!(parse_http_date("yesterday"), None);
    }

    #[test]
    fn clock_skew_fails_at_30_seconds() {
        assert_eq!(clock_item(Some(1_000), 1_029).state, State::Pass);
        assert_eq!(clock_item(Some(1_030), 1_000).state, State::Fail);
        assert_eq!(clock_item(None, 1_000).state, State::Skip);
    }

    #[test]
    fn the_latest_verdict_decides() {
        assert_eq!(checks_item(&[]).state, State::Skip);
        assert_eq!(
            checks_item(&[Verdict::Fail, Verdict::Pass]).state,
            State::Pass
        );
        let failed = checks_item(&[Verdict::Pass, Verdict::Fail]);
        assert_eq!(failed.state, State::Fail);
        assert!(failed.detail.contains("1 passed, 1 failed"));
    }

    #[tokio::test]
    async fn a_port_open_beyond_loopback_fails() {
        let wide = tokio::net::TcpListener::bind("0.0.0.0:0").await.unwrap();
        let port = wide.local_addr().unwrap().port();
        // Stand in 127.0.0.1 for this computer's outward address.
        let here: IpAddr = "127.0.0.1".parse().unwrap();
        let item = loopback(&[here], &[port]).await;
        assert_eq!(item.state, State::Fail, "{}", item.detail);
        drop(wide);
        let narrow = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = narrow.local_addr().unwrap().port();
        let elsewhere: IpAddr = "192.0.2.1".parse().unwrap();
        assert_eq!(loopback(&[elsewhere], &[port]).await.state, State::Pass);
        assert_eq!(loopback(&[], &[port]).await.state, State::Pass);
    }

    /// A loopback HTTP server that answers every request with `status`.
    async fn http(status: &'static str) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                use tokio::io::{AsyncReadExt, AsyncWriteExt};
                let mut buf = [0u8; 1024];
                let _ = socket.read(&mut buf).await;
                let _ = socket
                    .write_all(
                        format!(
                            "HTTP/1.1 {status}\r\ncontent-length: 2\r\nconnection: close\r\n\r\nok"
                        )
                        .as_bytes(),
                    )
                    .await;
            }
        });
        url
    }

    #[tokio::test]
    async fn verify_against_a_fixture_relay() {
        let (relay_url, _hub) = crate::fixture::relay().await.unwrap();
        let engine = http("200 OK").await;
        let dir = tempfile::tempdir().unwrap();
        let pylon = Identity::generate();
        let checkers = BTreeSet::new();
        let inputs = |owner| Inputs {
            relay: &relay_url,
            engine: &engine,
            pylon: &pylon,
            owner,
            checkers: &checkers,
            ports: vec![1],
            addresses: Some(Vec::new()),
            now: crate::now(),
        };
        // Before the pylon publishes, its beacon is missing.
        let before = run(inputs(None)).await;
        assert!(!before.pass);
        let beacon = before.items.iter().find(|i| i.id == "beacon").unwrap();
        assert_eq!(beacon.state, State::Fail);
        // A running pylon publishes one.
        let provider = crate::provider::Provider::on(
            crate::provider::Config::new(&relay_url, "verify", dir.path().to_path_buf()),
            Identity::from_secret(*pylon.secret()).unwrap(),
            Arc::new(crate::engine::Echo),
            Arc::new(crate::lease::Dedicated),
        )
        .unwrap();
        let event =
            nostr::pylon::owned_beacon_event(pylon.signer(), &provider.beacon(None).await, None)
                .unwrap();
        let mut conn = relay::connect(&relay_url, &pylon, Duration::from_secs(10))
            .await
            .unwrap();
        relay::publish(&mut conn, &event).await.unwrap();
        let after = run(inputs(None)).await;
        let ids: Vec<_> = after.items.iter().map(|i| i.id).collect();
        assert_eq!(
            ids,
            [
                "engine",
                "beacon",
                "owner_link",
                "checks",
                "clock",
                "loopback"
            ]
        );
        let state = |id: &str| after.items.iter().find(|i| i.id == id).unwrap().state;
        assert_eq!(state("engine"), State::Pass);
        assert_eq!(state("beacon"), State::Pass);
        assert_eq!(state("owner_link"), State::Skip);
        assert_eq!(state("checks"), State::Skip);
        assert_eq!(state("loopback"), State::Pass);
        assert!(after.pass, "{:?}", after.items);
        // An engine that fails its health check fails the list.
        let sick = http("503 Service Unavailable").await;
        let failed = run(Inputs {
            engine: &sick,
            ..inputs(None)
        })
        .await;
        assert!(!failed.pass);
    }
}
