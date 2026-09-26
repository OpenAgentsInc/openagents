//! Bounded authenticated transport for exact private Nostr artifacts.
//! The caller admits the relay and recipient before using these helpers.
use serde_json::{Value, json};

type Result<T> = std::result::Result<T, String>;
use futures_util::{SinkExt, StreamExt};
use nostr::domain::{Event, RelaySigner, Tag};
use secp256k1::SecretKey;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::net::TcpStream;
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async_with_config,
    tungstenite::{Message, protocol::WebSocketConfig},
};

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;
fn now() -> Result<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .map_err(|e| e.to_string())
}
async fn next(socket: &mut Socket) -> Result<Value> {
    for _ in 0..32 {
        match socket.next().await {
            Some(Ok(Message::Text(text))) => {
                return nostr::contracts::parse_strict_bounded(text.as_bytes(), 1024 * 1024)
                    .map_err(|e| e.to_string());
            }
            Some(Ok(Message::Ping(bytes))) => socket
                .send(Message::Pong(bytes))
                .await
                .map_err(|e| e.to_string())?,
            _ => {
                return Err("private artifact relay disconnected before its acknowledgment".into());
            }
        }
    }
    Err("private artifact relay exceeded its control-frame bound".into())
}
async fn send(socket: &mut Socket, value: Value) -> Result<()> {
    socket
        .send(Message::Text(value.to_string().into()))
        .await
        .map_err(|e| e.to_string())
}
async fn connect(url: &str, secret: &SecretKey) -> Result<Socket> {
    let config = WebSocketConfig::default().max_message_size(Some(1024 * 1024));
    let (mut socket, _) = connect_async_with_config(url, Some(config), false)
        .await
        .map_err(|e| e.to_string())?;
    let challenge = next(&mut socket).await?;
    if challenge[0] != "AUTH"
        || challenge.as_array().is_none_or(|a| a.len() != 2)
        || challenge[1]
            .as_str()
            .is_none_or(|s| s.is_empty() || s.len() > 4096)
    {
        return Err(
            "private artifact relay did not issue a bounded authentication challenge".into(),
        );
    }
    let signer = RelaySigner::from_secret_hex(&secret.display_secret().to_string())
        .map_err(|e| e.to_string())?;
    let event = signer.sign(
        now()?,
        22242,
        vec![
            Tag::new(vec!["relay".into(), url.into()]),
            Tag::new(vec![
                "challenge".into(),
                challenge[1].as_str().ok_or("relay challenge")?.into(),
            ]),
        ],
        String::new(),
    );
    send(&mut socket, json!(["AUTH", event])).await?;
    let acknowledgment = next(&mut socket).await?;
    if acknowledgment[0] != "OK" || acknowledgment[1] != event.id || acknowledgment[2] != true {
        return Err("private artifact relay authentication was not acknowledged".into());
    }
    Ok(socket)
}
/// Publish one exact signed declaration. Retrying its event ID cannot mint a
/// second logical artifact; the receiver still applies durable deduplication.
pub async fn publish(url: &str, secret: &SecretKey, event: &Event) -> Result<()> {
    tokio::time::timeout(Duration::from_secs(10), async {
        nostr::private_artifact::admit(event).map_err(|e| e.to_string())?;
        let mut socket = connect(url, secret).await?;
        send(&mut socket, json!(["EVENT", event])).await?;
        for _ in 0..32 {
            let result = next(&mut socket).await?;
            if result[0] == "OK" && result[1] == event.id {
                if result[2] != true {
                    return Err("private artifact relay refused the declaration".into());
                }
                socket.close(None).await.map_err(|e| e.to_string())?;
                return Ok(());
            }
        }
        Err("artifact publication acknowledgment unavailable".into())
    })
    .await
    .map_err(|_| "artifact publication deadline exceeded".to_owned())?
}
/// Fetch an exact retained declaration after reconnect. A missing event is
/// unavailable evidence, never permission to replace a job or assume it failed.
pub async fn fetch(url: &str, secret: &SecretKey, event_id: &str) -> Result<Event> {
    tokio::time::timeout(Duration::from_secs(10), async {
        if event_id.len() != 64 || !event_id.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("invalid private event identity".into());
        }
        let mut socket = connect(url, secret).await?;
        send(
            &mut socket,
            json!(["REQ","private-artifact-exact",{"ids":[event_id],"kinds":[3188],"limit":1}]),
        )
        .await?;
        for _ in 0..32 {
            let value = next(&mut socket).await?;
            if value[0] == "EVENT" && value[1] == "private-artifact-exact" {
                let event: Event =
                    serde_json::from_value(value[2].clone()).map_err(|e| e.to_string())?;
                if event.id != event_id {
                    return Err("private artifact relay returned another event".into());
                }
                nostr::private_artifact::open(&event, secret).map_err(|e| e.to_string())?;
                socket.close(None).await.map_err(|e| e.to_string())?;
                return Ok(event);
            }
            if value[0] == "EOSE" || value[0] == "CLOSED" {
                return Err("retained private declaration is unavailable to this reader".into());
            }
        }
        Err("artifact retrieval exceeded its message bound".into())
    })
    .await
    .map_err(|_| "artifact retrieval deadline exceeded".to_owned())?
}
