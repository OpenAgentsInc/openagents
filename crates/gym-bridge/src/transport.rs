//! Bounded authenticated private-artifact transport; no subscription on construction.
use crate::{ErrorCode, RelayPolicy, Result, error};
use nostr::domain::Event;
use nostr_transport::Connection;
use secp256k1::SecretKey;
use serde_json::json;
use std::time::{Duration, Instant};

pub use coder_connect::transport::Receiver;
fn transport(_: String) -> crate::Error {
    error(
        ErrorCode::Transport,
        "Gym relay exchange unavailable; launch delivery may be unknown",
    )
}
pub struct Session {
    socket: Connection,
    started: Instant,
    exchanges: usize,
}
impl Session {
    pub async fn connect(relay: &str, secret: &SecretKey, policy: RelayPolicy) -> Result<Self> {
        policy.validate(relay)?;
        Ok(Self {
            socket: Connection::connect(relay, secret, Duration::from_secs(90))
                .await
                .map_err(transport)?,
            started: Instant::now(),
            exchanges: 0,
        })
    }
    pub fn reusable(&self) -> bool {
        self.exchanges < 16 && self.started.elapsed() < Duration::from_secs(75)
    }
    pub async fn exchange(
        &mut self,
        event: &Event,
        mailbox: &str,
        host: &str,
        client: &str,
    ) -> Result<Event> {
        self.socket.send(json!(["REQ",mailbox,{"kinds":[3188],"authors":[host],"#p":[client],"#h":[mailbox],"limit":0}])).await.map_err(transport)?;
        loop {
            let frame = self.socket.next().await.map_err(transport)?;
            if frame[0] == "CLOSED" && frame[1] == mailbox {
                return Err(transport(String::new()));
            }
            if frame[0] == "EVENT" && frame[1] == mailbox {
                return Err(error(
                    ErrorCode::Forbidden,
                    "Gym reply arrived before the request",
                ));
            }
            if frame[0] == "EOSE" && frame[1] == mailbox {
                break;
            }
        }
        self.socket
            .send(json!(["EVENT", event]))
            .await
            .map_err(transport)?;
        let mut acknowledged = false;
        let mut response = None::<Event>;
        loop {
            let frame = self.socket.next().await.map_err(transport)?;
            if frame[0] == "CLOSED" && frame[1] == mailbox {
                return Err(transport(String::new()));
            }
            if frame[0] == "OK" && frame[1] == event.id {
                if frame[2] != true {
                    return Err(transport(String::new()));
                }
                acknowledged = true;
            }
            if frame[0] == "EVENT" && frame[1] == mailbox {
                let reply: Event = serde_json::from_value(frame[2].clone())
                    .map_err(|_| error(ErrorCode::Malformed, "invalid Gym relay event"))?;
                nostr::private_artifact::admit(&reply)
                    .map_err(|_| error(ErrorCode::Forbidden, "invalid private Gym event"))?;
                if reply.pubkey != host
                    || reply.tag_values("p").collect::<Vec<_>>() != [client]
                    || reply.tag_values("h").collect::<Vec<_>>() != [mailbox]
                    || response.as_ref().is_some_and(|r| r.id != reply.id)
                {
                    return Err(error(
                        ErrorCode::Forbidden,
                        "Gym relay supplied a foreign or conflicting reply",
                    ));
                }
                response = Some(reply);
            }
            if acknowledged && let Some(reply) = response {
                self.socket
                    .send(json!(["CLOSE", mailbox]))
                    .await
                    .map_err(transport)?;
                self.exchanges += 1;
                return Ok(reply);
            }
        }
    }
}
