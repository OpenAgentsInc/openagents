//! Relay reads and the sealed-job exchange (feature `net`): what the
//! gateway route and the `oa-att` CLI do on the wire. Neither opens a
//! sealed event; they carry and collect them.

use std::time::{Duration, Instant};

use nostr::att::{ENDPOINT_KIND, HEAD_KIND};
use nostr::decision::{FEEDBACK_KIND, RESULT_KIND};
use nostr::domain::Event;
use nostr_transport::Connection;
use secp256k1::SecretKey;
use serde_json::{Value, json};

use crate::Records;

/// The longest a relay connection lives (the transport's bound).
pub const LIFETIME: Duration = Duration::from_secs(115);

/// Records fetched from the relay, with how long the fetch took.
#[derive(Debug, Clone)]
pub struct Fetched {
    pub records: Records,
    pub ms: u64,
}

async fn connect(relay: &str, secret: &SecretKey, lifetime: Duration) -> Result<Connection, String> {
    Connection::connect(relay, secret, lifetime)
        .await
        .map(|c| c.with_frame_budget(4096))
        .map_err(|e| format!("relay {relay}: {e}"))
}

async fn query(conn: &mut Connection, sub: &str, filter: Value) -> Result<Vec<Event>, String> {
    conn.send(json!(["REQ", sub, filter])).await?;
    let mut events = Vec::new();
    loop {
        let frame = conn.next().await?;
        match frame[0].as_str() {
            Some("EVENT") if frame[1] == sub => {
                if let Ok(event) = serde_json::from_value::<Event>(frame[2].clone()) {
                    events.push(event);
                }
            }
            Some("EOSE") if frame[1] == sub => break,
            Some("CLOSED") if frame[1] == sub => {
                return Err(format!("the relay closed the query: {}", frame[2]));
            }
            _ => {}
        }
    }
    conn.send(json!(["CLOSE", sub])).await?;
    Ok(events)
}

/// Publish `event` on `conn` and wait for the relay's `OK`.
///
/// # Errors
///
/// When the relay refuses it.
pub async fn publish_on(conn: &mut Connection, event: &Event) -> Result<(), String> {
    conn.send(json!(["EVENT", event])).await?;
    loop {
        let frame = conn.next().await?;
        if frame[0] == "OK" && frame[1] == event.id.as_str() {
            return if frame[2] == true {
                Ok(())
            } else {
                Err(format!("the relay refused kind {}: {}", event.kind, frame[3]))
            };
        }
    }
}

/// Publish events, authenticating as `secret`.
///
/// # Errors
///
/// When the relay is unreachable or refuses one.
pub async fn publish(relay: &str, secret: &SecretKey, events: &[Event]) -> Result<(), String> {
    let mut conn = connect(relay, secret, Duration::from_secs(30)).await?;
    for event in events {
        publish_on(&mut conn, event).await?;
    }
    let _ = conn.close().await;
    Ok(())
}

/// The newest admitted-workload records: the publisher's head for
/// `workload`, the newest current endpoint under it, the release it runs,
/// and that endpoint key's beacon.
///
/// # Errors
///
/// When the relay is unreachable or a record is missing.
pub async fn fetch(
    relay: &str,
    secret: &SecretKey,
    publisher: &str,
    workload: &str,
    now: u64,
) -> Result<Fetched, String> {
    let started = Instant::now();
    let mut conn = connect(relay, secret, Duration::from_secs(30)).await?;
    let heads = query(
        &mut conn,
        "head",
        json!({"kinds": [HEAD_KIND], "authors": [publisher], "#d": [workload]}),
    )
    .await?;
    let head = heads
        .into_iter()
        .max_by_key(|e| e.created_at)
        .ok_or("no release head is published for this workload")?;
    let address = format!("{HEAD_KIND}:{publisher}:{workload}");
    let endpoints = query(
        &mut conn,
        "endpoints",
        json!({"kinds": [ENDPOINT_KIND], "#a": [address], "since": now.saturating_sub(3_600)}),
    )
    .await?;
    let endpoint = endpoints
        .into_iter()
        .filter(|e| {
            e.tag_values("expiration")
                .next()
                .and_then(|t| t.parse::<u64>().ok())
                .is_some_and(|t| t > now)
        })
        .max_by_key(|e| e.created_at)
        .ok_or("no attested endpoint is running right now")?;
    let release_id = endpoint
        .tag_values("e")
        .next()
        .ok_or("the endpoint names no release")?
        .to_string();
    let release = query(&mut conn, "release", json!({"ids": [release_id]}))
        .await?
        .into_iter()
        .next()
        .ok_or("the endpoint's release is not on the relay")?;
    let beacon = query(
        &mut conn,
        "beacon",
        json!({"kinds": [nostr::pylon::BEACON_KIND], "authors": [endpoint.pubkey]}),
    )
    .await?
    .into_iter()
    .max_by_key(|e| e.created_at);
    let _ = conn.close().await;
    Ok(Fetched {
        records: Records {
            release,
            head,
            endpoint,
            beacon,
        },
        ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
    })
}

/// What [`exchange`] reports as it happens.
#[derive(Debug, Clone)]
pub enum Exchanged {
    /// The relay accepted the request after this many milliseconds.
    Accepted(u64),
    /// A signed answer from the worker the request names.
    Answer(Event),
}

/// Publish a sealed request and collect the worker's answers to it, each
/// handed to `on_event` as it arrives, until a result arrives or `wait`
/// passes. Returns the milliseconds the relay took to accept the request.
///
/// # Errors
///
/// When the relay is unreachable or refuses the request.
pub async fn exchange(
    relay: &str,
    secret: &SecretKey,
    request: &Event,
    wait: Duration,
    mut on_event: impl FnMut(Exchanged),
) -> Result<u64, String> {
    let wait = wait.min(LIFETIME - Duration::from_secs(5));
    let mut conn = connect(relay, secret, wait + Duration::from_secs(5)).await?;
    conn.send(json!(["REQ", "answers", {"kinds": [RESULT_KIND, FEEDBACK_KIND], "#e": [request.id]}]))
        .await?;
    let sent = Instant::now();
    conn.send(json!(["EVENT", request])).await?;
    let mut accepted_ms = None;
    let deadline = Instant::now() + wait;
    while Instant::now() < deadline {
        let Ok(frame) = conn.next().await else { break };
        match frame[0].as_str() {
            Some("OK") if frame[1] == request.id.as_str() => {
                if frame[2] != true {
                    return Err(format!("the relay refused the request: {}", frame[3]));
                }
                let ms = u64::try_from(sent.elapsed().as_millis()).unwrap_or(0);
                accepted_ms = Some(ms);
                on_event(Exchanged::Accepted(ms));
            }
            Some("EVENT") if frame[1] == "answers" => {
                if let Ok(event) = serde_json::from_value::<Event>(frame[2].clone())
                    && event.validate_crypto().is_ok()
                    && event.pubkey == request.tag_values("p").next().unwrap_or_default()
                {
                    let done = event.kind == RESULT_KIND;
                    on_event(Exchanged::Answer(event));
                    if done {
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    let _ = conn.close().await;
    accepted_ms.ok_or_else(|| "the relay never acknowledged the request".into())
}
