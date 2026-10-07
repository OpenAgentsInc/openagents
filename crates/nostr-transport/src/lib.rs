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
    Connector, MaybeTlsStream, WebSocketStream, connect_async_tls_with_config,
    tungstenite::{Message, protocol::WebSocketConfig},
};

pub mod artifacts;
pub type Result<T> = std::result::Result<T, String>;
const MAX_BYTES: usize = 1024 * 1024;

fn tls_connector() -> Result<Connector> {
    // Select this socket's provider explicitly. Cargo feature unification can
    // otherwise enable both providers and make Rustls's automatic choice panic.
    // Do not change another library's process-wide cryptography configuration.
    let roots = rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config = rustls::ClientConfig::builder_with_provider(std::sync::Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .map_err(|_| "relay TLS protocol configuration is unavailable")?
    .with_root_certificates(roots)
    .with_no_client_auth();
    Ok(Connector::Rustls(std::sync::Arc::new(config)))
}

/// One connection has a fixed deadline and frame budget, including authentication.
/// Reconnecting does not implicitly resend any request or enlarge an operation.
pub struct Connection {
    socket: WebSocketStream<MaybeTlsStream<TcpStream>>,
    deadline: Instant,
    remaining: usize,
}
impl Connection {
    pub async fn connect(url: &str, secret: &SecretKey, lifetime: Duration) -> Result<Self> {
        Self::connect_as(url, secret, None, lifetime).await
    }
    /// Connect and authenticate like [`Connection::connect`], with a NIP-OA
    /// `auth` tag in the `kind:22242` event when `auth` is set, so an agent
    /// key whose owner is a relay member is admitted under NIP-AA. The tag
    /// must be the four-element `["auth", OWNER, CONDITIONS, SIG]`; it
    /// proves ownership and grants nothing beyond what the relay admits.
    pub async fn connect_as(
        url: &str,
        secret: &SecretKey,
        auth: Option<&Tag>,
        lifetime: Duration,
    ) -> Result<Self> {
        if auth.is_some_and(|tag| tag.0.len() != 4 || tag.name() != Some("auth")) {
            return Err("a NIP-AA credential is a four-element auth tag".into());
        }
        if lifetime.is_zero() || lifetime > Duration::from_secs(120) {
            return Err("relay connection lifetime must be within 120 seconds".into());
        }
        let deadline = Instant::now() + lifetime;
        let config = WebSocketConfig::default()
            .max_message_size(Some(MAX_BYTES))
            .max_frame_size(Some(MAX_BYTES));
        let (socket, _) = timeout_at(
            deadline,
            connect_async_tls_with_config(url, Some(config), false, Some(tls_connector()?)),
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
        let mut tags = vec![
            Tag::new(vec!["relay".into(), url.into()]),
            Tag::new(vec![
                "challenge".into(),
                challenge[1].as_str().ok_or("challenge")?.into(),
            ]),
        ];
        tags.extend(auth.cloned());
        let event = signer.sign(now, 22242, tags, String::new());
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
    /// Connect without NIP-42 authentication, for a public relay's reads
    /// and writes of public events. The same deadline and frame budget
    /// apply. A relay that sends an `AUTH` challenge anyway gets no answer;
    /// the caller skips that frame.
    pub async fn connect_open(url: &str, lifetime: Duration) -> Result<Self> {
        if lifetime.is_zero() || lifetime > Duration::from_secs(120) {
            return Err("relay connection lifetime must be within 120 seconds".into());
        }
        if !url.starts_with("wss://") {
            return Err("an unauthenticated relay connection must use wss".into());
        }
        let deadline = Instant::now() + lifetime;
        let config = WebSocketConfig::default()
            .max_message_size(Some(MAX_BYTES))
            .max_frame_size(Some(MAX_BYTES));
        let (socket, _) = timeout_at(
            deadline,
            connect_async_tls_with_config(url, Some(config), false, Some(tls_connector()?)),
        )
        .await
        .map_err(|_| "relay connection deadline exceeded")?
        .map_err(|e| e.to_string())?;
        Ok(Self {
            socket,
            deadline,
            remaining: 256,
        })
    }
    /// Replace the frame budget for the rest of this connection, at most
    /// 4,096 frames: a streamed answer reads one frame per delta event, more
    /// than the default 256 allows for a long one.
    #[must_use]
    pub fn with_frame_budget(mut self, frames: usize) -> Self {
        self.remaining = frames.min(4096);
        self
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A one-connection relay on loopback that issues a challenge, checks
    /// the AUTH event, and answers `OK`. Returns the AUTH event it saw.
    async fn loopback_relay() -> (String, tokio::task::JoinHandle<nostr::domain::Event>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}", listener.local_addr().unwrap());
        let handle = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut socket = tokio_tungstenite::accept_async(stream).await.unwrap();
            socket
                .send(Message::Text(
                    json!(["AUTH", "challenge-1"]).to_string().into(),
                ))
                .await
                .unwrap();
            let Some(Ok(Message::Text(text))) = socket.next().await else {
                panic!("no AUTH");
            };
            let frame: Value = serde_json::from_str(&text).unwrap();
            let event: nostr::domain::Event = serde_json::from_value(frame[1].clone()).unwrap();
            event.validate_crypto().unwrap();
            socket
                .send(Message::Text(
                    json!(["OK", event.id, true, ""]).to_string().into(),
                ))
                .await
                .unwrap();
            event
        });
        (url, handle)
    }

    #[tokio::test]
    async fn authentication_carries_a_nip_aa_auth_tag_when_given() {
        let secret = SecretKey::from_byte_array([3; 32]).unwrap();
        let tag = Tag::new(vec![
            "auth".into(),
            "ab".repeat(32),
            "created_at<2000000000".into(),
            "cd".repeat(64),
        ]);
        let (url, relay) = loopback_relay().await;
        let connection = Connection::connect_as(&url, &secret, Some(&tag), Duration::from_secs(5))
            .await
            .unwrap();
        drop(connection);
        let event = relay.await.unwrap();
        assert_eq!(event.kind, 22242);
        assert!(event.tags.contains(&tag));
        assert!(
            event
                .tags
                .iter()
                .any(|t| t.0 == ["challenge", "challenge-1"])
        );

        let (url, relay) = loopback_relay().await;
        drop(
            Connection::connect(&url, &secret, Duration::from_secs(5))
                .await
                .unwrap(),
        );
        let event = relay.await.unwrap();
        assert!(event.tags.iter().all(|t| t.name() != Some("auth")));

        let short = Tag::new(vec!["auth".into(), "ab".repeat(32)]);
        assert!(
            Connection::connect_as(
                "ws://127.0.0.1:9",
                &secret,
                Some(&short),
                Duration::from_secs(1)
            )
            .await
            .is_err()
        );
    }

    #[test]
    fn socket_tls_provider_does_not_depend_on_a_process_default() {
        assert!(matches!(
            super::tls_connector(),
            Ok(tokio_tungstenite::Connector::Rustls(_))
        ));
    }
}
