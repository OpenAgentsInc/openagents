//! Finite NIP-42 connections. Signed application replies, not relay ACKs, carry data.
use crate::{Error, ErrorCode, Result, client::Pending, protocol::*, unix_time};
use nostr::domain::Event;
use nostr_transport::Connection;
use secp256k1::SecretKey;
use serde_json::json;
use std::time::Duration;

fn transport(_: String) -> Error {
    Error::new(
        ErrorCode::Transport,
        "authenticated relay exchange is unavailable",
    )
}
fn event(value: serde_json::Value) -> Result<Event> {
    let event: Event = serde_json::from_value(value)
        .map_err(|_| Error::new(ErrorCode::Malformed, "relay returned an invalid event"))?;
    nostr::private_artifact::admit(&event).map_err(|_| {
        Error::new(
            ErrorCode::Forbidden,
            "relay returned an invalid private declaration",
        )
    })?;
    Ok(event)
}

pub async fn exchange(
    relay: &str,
    secret: &SecretKey,
    pending: &Pending,
    host: &str,
    policy: RelayPolicy,
) -> Result<Event> {
    tokio::time::timeout(Duration::from_secs(8), async {
        let mut session = Session::connect(relay, secret, policy).await?;
        session.exchange(pending, host, &pubkey(secret)).await
    })
    .await
    .map_err(|_| transport(String::new()))?
}

/// A serial client socket with a fixed lifetime and aggregate frame budget.
/// The caller must drop it after any interrupted or unsuccessful exchange.
pub struct Session {
    socket: Connection,
    started: std::time::Instant,
    exchanges: usize,
}
impl Session {
    pub async fn connect(relay: &str, secret: &SecretKey, policy: RelayPolicy) -> Result<Self> {
        policy.validate(relay)?;
        let started = std::time::Instant::now();
        let socket = Connection::connect(relay, secret, Duration::from_secs(90))
            .await
            .map_err(transport)?;
        Ok(Self {
            socket,
            started,
            exchanges: 0,
        })
    }
    pub fn reusable(&self) -> bool {
        self.exchanges < 24 && self.started.elapsed() < Duration::from_secs(75)
    }
    pub async fn exchange(&mut self, pending: &Pending, host: &str, own: &str) -> Result<Event> {
        let subscription = &pending.request.request;
        self.socket.send(json!(["REQ",subscription,{"kinds":[3188],"authors":[host],"#p":[own],"#h":[pending.request.request],"limit":0}])).await.map_err(transport)?;
        loop {
            let frame = self.socket.next().await.map_err(transport)?;
            if frame[0] == "CLOSED" && frame[1] == *subscription {
                return Err(transport(String::new()));
            }
            if frame[0] == "EVENT" && frame[1] == *subscription {
                // No read has been published on this subscription yet. A
                // replayed old reply cannot establish current host admission.
                return Err(Error::new(
                    ErrorCode::Forbidden,
                    "reply arrived before this exchange was admitted",
                ));
            }
            if frame[0] == "EOSE" && frame[1] == *subscription {
                break;
            }
        }
        fresh(
            pending.request.issued_at,
            pending.request.expires_at,
            unix_time()?,
        )?;
        self.socket
            .send(json!(["EVENT", pending.event]))
            .await
            .map_err(transport)?;
        let mut acknowledged = false;
        let mut response = None;
        loop {
            let frame = self.socket.next().await.map_err(transport)?;
            if frame[0] == "CLOSED" && frame[1] == *subscription {
                return Err(transport(String::new()));
            }
            if frame[0] == "OK" && frame[1] == pending.event.id {
                if frame[2] != true {
                    return Err(transport(String::new()));
                }
                acknowledged = true;
            }
            if frame[0] == "EVENT" && frame[1] == *subscription {
                let received = check_reply(frame[2].clone(), host, own, &pending.request.request)?;
                if response
                    .as_ref()
                    .is_some_and(|old: &Event| old.id != received.id)
                {
                    return Err(Error::new(
                        ErrorCode::Conflict,
                        "relay supplied conflicting replies",
                    ));
                }
                response = Some(received);
            }
            if acknowledged && let Some(reply) = response {
                self.socket
                    .send(json!(["CLOSE", subscription]))
                    .await
                    .map_err(transport)?;
                self.exchanges += 1;
                return Ok(reply);
            }
        }
    }
}
fn check_reply(
    value: serde_json::Value,
    host: &str,
    recipient: &str,
    mailbox: &str,
) -> Result<Event> {
    let event = event(value)?;
    if event.pubkey != host
        || event.tag_values("p").collect::<Vec<_>>() != [recipient]
        || event.tag_values("h").collect::<Vec<_>>() != [mailbox]
    {
        return Err(Error::new(
            ErrorCode::Forbidden,
            "relay reply identity differs",
        ));
    }
    Ok(event)
}

/// A finite host subscription, renewed by the CLI with bounded backoff.
pub struct Receiver {
    socket: Connection,
    host: String,
}
impl Receiver {
    pub async fn connect(relay: &str, secret: &SecretKey, policy: RelayPolicy) -> Result<Self> {
        policy.validate(relay)?;
        let host = pubkey(secret);
        let mut socket = Connection::connect(relay, secret, Duration::from_secs(90))
            .await
            .map_err(transport)?;
        socket.send(json!(["REQ","history-input",{"kinds":[3188],"#p":[host],"since":unix_time()?.saturating_sub(60),"limit":128}])).await.map_err(transport)?;
        Ok(Self { socket, host })
    }
    pub async fn next_request(&mut self) -> Result<Event> {
        loop {
            let frame = self.socket.next().await.map_err(transport)?;
            if frame[0] == "CLOSED" {
                return Err(transport(String::new()));
            }
            if frame[0] == "EVENT" && frame[1] == "history-input" {
                let event = event(frame[2].clone())?;
                if event.tag_values("p").collect::<Vec<_>>() != [self.host.as_str()] {
                    return Err(Error::new(
                        ErrorCode::Forbidden,
                        "request targets another host",
                    ));
                }
                return Ok(event);
            }
        }
    }
    pub async fn publish(&mut self, event: &Event) -> Result<()> {
        if event.pubkey != self.host {
            return Err(Error::new(
                ErrorCode::Forbidden,
                "reply signer differs from host",
            ));
        }
        self.socket
            .send(json!(["EVENT", event]))
            .await
            .map_err(transport)?;
        // ACKs and interleaved requests stay in the socket and are read by the
        // next iteration. The client independently needs the signed reply.
        Ok(())
    }
}
