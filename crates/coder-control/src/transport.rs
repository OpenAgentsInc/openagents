//! Finite authenticated exchanges; reconnect retries preserve exact signed bytes.
//! Transport checks signed routing. The host/client still validates every body.
use crate::{Reply, Result, client};
use nostr::domain::Event;
use nostr::execution::{FEEDBACK_KIND, REQUEST_KIND, RESULT_KIND};
use nostr_transport::Connection;
use secp256k1::SecretKey;
use serde_json::{Value, json};
use std::time::Duration;

fn addressed(event: &Event, kind: u16, recipient: &str) -> Result<()> {
    event.validate_crypto().map_err(|e| e.to_string())?;
    if event.kind != kind || event.tag_values("p").collect::<Vec<_>>() != [recipient] {
        return Err("control relay event kind or recipient differs".into());
    }
    Ok(())
}
fn response(event: &Event, request: &Event, authority: &str, recipient: &str) -> Result<()> {
    addressed(event, RESULT_KIND, recipient)?;
    if event.pubkey != authority
        || event.tag_values("e").collect::<Vec<_>>() != [request.id.as_str()]
    {
        return Err("control result signer or request differs".into());
    }
    Ok(())
}
/// Subscribe before sending one immutable CJ request. A transport error is
/// unknown delivery, not permission to mint another logical command.
pub async fn exchange(
    url: &str,
    secret: &SecretKey,
    request: &Event,
    authority: &str,
) -> Result<Event> {
    addressed(request, REQUEST_KIND, authority)?;
    let principal = client::pubkey(secret);
    if request.pubkey != principal {
        return Err("request signer differs from client identity".into());
    }
    let mut socket = Connection::connect(url, secret, Duration::from_secs(30)).await?;
    socket.send(json!(["REQ","control-reply",{"kinds":[RESULT_KIND,FEEDBACK_KIND],"authors":[authority],"#e":[request.id],"#p":[principal],"limit":0}])).await?;
    socket.send(json!(["EVENT", request])).await?;
    let mut acknowledged = false;
    let mut result = None;
    loop {
        let value = socket.next().await?;
        match value[0].as_str() {
            Some("OK") if value[1] == request.id => {
                if value[2] != true {
                    return Err("relay refused the exact control request".into());
                }
                acknowledged = true;
            }
            Some("CLOSED") if value[1] == "control-reply" => {
                return Err("relay closed the control result subscription".into());
            }
            Some("EVENT") if value[1] == "control-reply" => {
                let event: Event =
                    serde_json::from_value(value[2].clone()).map_err(|e| e.to_string())?;
                if event.kind == RESULT_KIND {
                    response(&event, request, authority, &principal)?;
                    if result
                        .as_ref()
                        .is_some_and(|old: &Event| old.id != event.id)
                    {
                        return Err("conflicting terminal control results".into());
                    }
                    result = Some(event);
                }
            }
            _ => {}
        }
        if acknowledged && let Some(event) = result {
            socket.close().await?;
            return Ok(event);
        }
    }
}

/// A bounded, one-request host subscription. Creating it does not admit work.
pub struct Receiver {
    socket: Connection,
    authority: String,
}
impl Receiver {
    /// Wait for EOSE so a caller can expose readiness without a timing sleep.
    pub async fn connect(url: &str, secret: &SecretKey) -> Result<Self> {
        let authority = client::pubkey(secret);
        let mut socket = Connection::connect(url, secret, Duration::from_secs(120)).await?;
        socket
            .send(
                json!(["REQ","control-input",{"kinds":[REQUEST_KIND],"#p":[authority],"limit":0}]),
            )
            .await?;
        loop {
            let value = socket.next().await?;
            if value[0] == "EOSE" && value[1] == "control-input" {
                break;
            }
            if value[0] == "CLOSED" {
                return Err("relay refused the control input subscription".into());
            }
            // A request before readiness has unknown delivery; the client can
            // retry its original packet once this finite host is ready.
            if value[0] == "EVENT" {
                return Err("control request arrived before subscription readiness".into());
            }
        }
        Ok(Self { socket, authority })
    }
    pub async fn receive(&mut self) -> Result<Event> {
        loop {
            let value = self.socket.next().await?;
            if value[0] == "CLOSED" && value[1] == "control-input" {
                return Err("control input subscription closed".into());
            }
            if value[0] == "EVENT" && value[1] == "control-input" {
                let event: Event =
                    serde_json::from_value(value[2].clone()).map_err(|e| e.to_string())?;
                addressed(&event, REQUEST_KIND, &self.authority)?;
                return Ok(event);
            }
        }
    }
    /// Publish already retained host output, then confirm the exact event IDs.
    /// Caller publishes referenced private artifacts before invoking this method.
    pub async fn reply(mut self, request: &Event, reply: &Reply) -> Result<()> {
        response(&reply.result, request, &self.authority, &request.pubkey)?;
        addressed(&reply.feedback, FEEDBACK_KIND, &request.pubkey)?;
        if reply.feedback.pubkey != self.authority
            || reply.feedback.tag_values("e").collect::<Vec<_>>() != [request.id.as_str()]
        {
            return Err("control feedback routing differs".into());
        }
        for event in [&reply.feedback, &reply.result] {
            self.socket.send(json!(["EVENT", event])).await?;
            loop {
                let value: Value = self.socket.next().await?;
                if value[0] == "OK" && value[1] == event.id {
                    if value[2] != true {
                        return Err("relay refused retained control output".into());
                    }
                    break;
                }
            }
        }
        self.socket.close().await
    }
}
