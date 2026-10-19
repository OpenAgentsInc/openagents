//! The coordinator receives presence, catalogs and SDP only. Reconnects obtain
//! fresh credentials and clear the previous connection's remote routes/peers.
use crate::{
    catalog::Catalog,
    peer::{OutgoingSignal, Peers, Signal},
};
use futures::{SinkExt, StreamExt};
use openagents_proto::PreviewService;
use serde::Deserialize;
use std::{sync::Arc, time::Duration};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};
use tokio_util::sync::CancellationToken;
#[async_trait::async_trait]
pub trait TokenSource: Send + Sync {
    async fn token(&self) -> anyhow::Result<String>;
}
pub struct Config {
    pub edge_url: String,
    pub org_id: String,
    pub tokens: Arc<dyn TokenSource>,
}
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum Incoming {
    Catalog {
        device: String,
        services: Vec<PreviewService>,
    },
    Gone {
        device: String,
    },
    Signal {
        from: String,
        signal: Signal,
    },
}
pub async fn run(
    config: Config,
    catalog: Catalog,
    peers: Peers,
    mut outgoing: mpsc::Receiver<OutgoingSignal>,
    stop: CancellationToken,
) {
    let mut backoff = Backoff::default();
    loop {
        let mut established = false;
        let connected = connect(&config, &catalog, &peers, &mut outgoing, &mut established);
        let mut missing = false;
        tokio::select! { _ = stop.cancelled() => break, result = connected => {
            if let Err(error) = result {
                missing = route_missing(&error);
                tracing::debug!(%error, missing, "preview coordinator disconnected");
            }
        } }
        catalog.clear_remote();
        peers.clear().await;
        while outgoing.try_recv().is_ok() {} // signaling from the old lease is stale
        let delay = backoff.next(established, missing, jitter_seed());
        tokio::select! { _ = stop.cancelled() => break, _ = tokio::time::sleep(delay) => {} }
    }
    catalog.clear_remote();
    peers.clear().await;
}
/// The first retry delay after a failed or closed coordinator connection.
const RETRY_MIN: Duration = Duration::from_secs(3);
/// The longest delay between retries of a coordinator that keeps failing.
const RETRY_MAX: Duration = Duration::from_secs(5 * 60);
/// The delay after the server answers the upgrade with 404 or 410: the
/// coordinator route isn't served there, so a fast retry only adds load (#12126).
const RETRY_MISSING: Duration = Duration::from_secs(30 * 60);

/// Reconnect delays: doubling from [`RETRY_MIN`] to [`RETRY_MAX`] while
/// attempts fail, reset once a connection is established, and held at
/// [`RETRY_MISSING`] while the route is missing. Each delay carries up to 25%
/// jitter so devices don't retry in step.
#[derive(Default)]
struct Backoff {
    failures: u32,
}

impl Backoff {
    fn next(&mut self, established: bool, missing: bool, seed: u64) -> Duration {
        if established && !missing {
            self.failures = 0;
        }
        let base = if missing {
            RETRY_MISSING
        } else {
            let doubled = RETRY_MIN.saturating_mul(1u32 << self.failures.min(16));
            doubled.min(RETRY_MAX)
        };
        self.failures = self.failures.saturating_add(1);
        let quarter = base.as_millis() as u64 / 4;
        let extra = if quarter == 0 {
            0
        } else {
            seed % (quarter + 1)
        };
        base + Duration::from_millis(extra)
    }
}

fn jitter_seed() -> u64 {
    let bytes = uuid::Uuid::new_v4().into_bytes();
    u64::from_le_bytes(bytes[..8].try_into().expect("eight bytes"))
}

/// Whether the error is the server saying it has no coordinator route.
fn route_missing(error: &anyhow::Error) -> bool {
    use tokio_tungstenite::tungstenite::{Error, http::StatusCode};
    matches!(
        error.downcast_ref::<Error>(),
        Some(Error::Http(response))
            if matches!(response.status(), StatusCode::NOT_FOUND | StatusCode::GONE)
    )
}

async fn connect(
    config: &Config,
    catalog: &Catalog,
    peers: &Peers,
    outgoing: &mut mpsc::Receiver<OutgoingSignal>,
    established: &mut bool,
) -> anyhow::Result<()> {
    let token = config.tokens.token().await?;
    let mut url = reqwest::Url::parse(&config.edge_url)?;
    let scheme = if url.scheme() == "https" { "wss" } else { "ws" };
    url.set_scheme(scheme)
        .map_err(|_| anyhow::anyhow!("invalid preview coordinator URL"))?;
    url.set_path(&format!("/preview/{}/ws", config.org_id));
    url.query_pairs_mut()
        .clear()
        .append_pair("device", catalog.device_id());
    let mut request = url.as_str().into_client_request()?;
    request
        .headers_mut()
        .insert("authorization", format!("Bearer {token}").parse()?);
    let mut websocket_config = tokio_tungstenite::tungstenite::protocol::WebSocketConfig::default();
    websocket_config.max_message_size = Some(1024 * 1024);
    websocket_config.max_frame_size = Some(1024 * 1024);
    let (socket, _) = tokio::time::timeout(
        Duration::from_secs(10),
        tokio_tungstenite::connect_async_with_config(request, Some(websocket_config), false),
    )
    .await??;
    *established = true;
    let (mut write, mut read) = socket.split();
    let mut changes = catalog.subscribe();
    let mut advertised = catalog.local_services();
    write
        .send(Message::Text(
            serde_json::json!({"type":"catalog", "services":advertised}).to_string(),
        ))
        .await?;
    let mut ping = tokio::time::interval(Duration::from_secs(10));
    let mut last_message = tokio::time::Instant::now();
    // SDP gathering must not block the socket heartbeat or other devices.
    let mut negotiations = tokio::task::JoinSet::new();
    loop {
        tokio::select! {
            _ = ping.tick() => {
                anyhow::ensure!(last_message.elapsed() < Duration::from_secs(40), "preview coordinator lease expired");
                write.send(Message::Text("ping".into())).await?;
            }
            _ = changes.changed() => {
                let next = catalog.local_services();
                if next != advertised { write.send(Message::Text(serde_json::json!({"type":"catalog", "services":next}).to_string())).await?; advertised = next; }
            }
            Some(message) = outgoing.recv() => {
                write.send(Message::Text(serde_json::json!({"type":"signal", "to":message.to, "signal":message.signal}).to_string())).await?;
            }
            Some(_) = negotiations.join_next(), if !negotiations.is_empty() => {}
            message = read.next() => {
                let message = message.ok_or_else(|| anyhow::anyhow!("preview coordinator closed"))??;
                last_message = tokio::time::Instant::now();
                match message {
                    Message::Text(text) if text == "pong" => {},
                    Message::Text(text) => {
                        anyhow::ensure!(text.len() <= 1024*1024, "oversized preview coordinator message");
                        match serde_json::from_str::<Incoming>(&text)? {
                            Incoming::Catalog { device, services } => catalog.set_remote(&device, services)?,
                            Incoming::Gone { device } => { catalog.remove_remote(&device); peers.remove(&device).await; }
                            Incoming::Signal { from, signal } => {
                                anyhow::ensure!(negotiations.len() < 16, "too many concurrent preview negotiations");
                                let peers = peers.clone();
                                negotiations.spawn(async move { if let Err(error) = peers.signal(&from, signal).await { tracing::debug!(%error, "preview negotiation failed"); } });
                            }
                        }
                    }
                    Message::Ping(bytes) => write.send(Message::Pong(bytes)).await?,
                    Message::Close(_) => anyhow::bail!("preview coordinator closed"),
                    Message::Binary(_) => anyhow::bail!("application traffic is not allowed on preview signaling"),
                    _ => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failures_double_to_the_cap_with_bounded_jitter() {
        let mut backoff = Backoff::default();
        let delays: Vec<_> = (0..10).map(|_| backoff.next(false, false, 0)).collect();
        assert_eq!(delays[0], RETRY_MIN);
        assert_eq!(delays[1], RETRY_MIN * 2);
        assert_eq!(delays[2], RETRY_MIN * 4);
        assert_eq!(*delays.last().unwrap(), RETRY_MAX);
        let jittered = Backoff::default().next(false, false, u64::MAX);
        assert!(jittered >= RETRY_MIN && jittered <= RETRY_MIN + RETRY_MIN / 4);
    }

    #[test]
    fn an_established_session_resets_the_backoff() {
        let mut backoff = Backoff::default();
        for _ in 0..6 {
            backoff.next(false, false, 0);
        }
        assert_eq!(backoff.next(true, false, 0), RETRY_MIN);
    }

    #[test]
    fn a_missing_route_waits_long() {
        let mut backoff = Backoff::default();
        assert_eq!(backoff.next(false, true, 0), RETRY_MISSING);
        assert!(backoff.next(false, true, u64::MAX) <= RETRY_MISSING + RETRY_MISSING / 4);
    }

    #[test]
    fn upgrade_404_and_410_count_as_a_missing_route() {
        use tokio_tungstenite::tungstenite::{Error, http::Response};
        let status = |code: u16| -> anyhow::Error {
            Error::Http(Response::builder().status(code).body(None).unwrap()).into()
        };
        assert!(route_missing(&status(404)));
        assert!(route_missing(&status(410)));
        assert!(!route_missing(&status(503)));
        assert!(!route_missing(&anyhow::anyhow!(
            "preview coordinator closed"
        )));
    }
}
