//! Bounded NIP-42 transport. Callers admit the relay and each operation first.
//! A socket acknowledgment never supplies task, disclosure, or spending authority.
use futures_util::{SinkExt, StreamExt};
use nostr::domain::{RelaySigner, Tag};
use secp256k1::SecretKey;
use serde_json::{Value, json};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::{
    net::TcpStream,
    time::{Instant, timeout_at},
};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async_with_config,
    tungstenite::{Message, protocol::WebSocketConfig},
};

pub mod artifacts;
pub type Result<T> = std::result::Result<T, String>;
const MAX_BYTES: usize = 1024 * 1024;

/// One connection has a fixed deadline and frame budget, including authentication.
/// Reconnecting does not implicitly resend any request or enlarge an operation.
pub struct Connection {
    socket: WebSocketStream<MaybeTlsStream<TcpStream>>,
    deadline: Instant,
    remaining: usize,
}
impl Connection {
    pub async fn connect(url: &str, secret: &SecretKey, lifetime: Duration) -> Result<Self> {
        if lifetime.is_zero() || lifetime > Duration::from_secs(120) {
            return Err("relay connection lifetime must be within 120 seconds".into());
        }
        let deadline = Instant::now() + lifetime;
        let config = WebSocketConfig::default()
            .max_message_size(Some(MAX_BYTES))
            .max_frame_size(Some(MAX_BYTES));
        let (socket, _) = timeout_at(
            deadline,
            connect_async_with_config(url, Some(config), false),
        )
        .await
        .map_err(|_| "relay connection deadline exceeded")?
        .map_err(|e| e.to_string())?;
        let mut connection = Self {
            socket,
            deadline,
            remaining: 256,
        };
        let challenge = connection.next().await?;
        if challenge.as_array().is_none_or(|a| a.len() != 2)
            || challenge[0] != "AUTH"
            || challenge[1]
                .as_str()
                .is_none_or(|s| s.is_empty() || s.len() > 4096)
        {
            return Err("relay did not issue a bounded authentication challenge".into());
        }
        let signer = RelaySigner::from_secret_hex(&secret.display_secret().to_string())
            .map_err(|e| e.to_string())?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_secs();
        let event = signer.sign(
            now,
            22242,
            vec![
                Tag::new(vec!["relay".into(), url.into()]),
                Tag::new(vec![
                    "challenge".into(),
                    challenge[1].as_str().ok_or("challenge")?.into(),
                ]),
            ],
            String::new(),
        );
        connection.send(json!(["AUTH", event])).await?;
        let ack = connection.next().await?;
        if ack.as_array().is_none_or(|a| a.len() != 4)
            || ack[0] != "OK"
            || ack[1] != event.id
            || ack[2] != true
        {
            return Err("relay authentication was not acknowledged".into());
        }
        Ok(connection)
    }
    pub async fn send(&mut self, value: Value) -> Result<()> {
        let text = value.to_string();
        if text.len() > MAX_BYTES {
            return Err("relay message exceeds its byte bound".into());
        }
        timeout_at(self.deadline, self.socket.send(Message::Text(text.into())))
            .await
            .map_err(|_| "relay send deadline exceeded")?
            .map_err(|e| e.to_string())
    }
    pub async fn next(&mut self) -> Result<Value> {
        loop {
            if self.remaining == 0 {
                return Err("relay exceeded its frame bound".into());
            }
            self.remaining -= 1;
            let frame = timeout_at(self.deadline, self.socket.next())
                .await
                .map_err(|_| "relay read deadline exceeded")?;
            match frame {
                Some(Ok(Message::Text(text))) => {
                    return nostr::contracts::parse_strict_bounded(text.as_bytes(), MAX_BYTES)
                        .map_err(|e| e.to_string());
                }
                Some(Ok(Message::Ping(bytes))) => {
                    timeout_at(self.deadline, self.socket.send(Message::Pong(bytes)))
                        .await
                        .map_err(|_| "relay pong deadline exceeded")?
                        .map_err(|e| e.to_string())?
                }
                Some(Ok(Message::Pong(_))) => {}
                _ => return Err("relay disconnected before completing the operation".into()),
            }
        }
    }
    pub async fn close(mut self) -> Result<()> {
        timeout_at(self.deadline, self.socket.close(None))
            .await
            .map_err(|_| "relay close deadline exceeded")?
            .map_err(|e| e.to_string())
    }
}
