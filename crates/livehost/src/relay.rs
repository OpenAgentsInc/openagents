//! A minimal NIP-01 relay client: publish an event, query stored events,
//! and follow a live subscription.

use std::time::Duration;

use anyhow::{Context, Result, bail};
use futures_util::{SinkExt, StreamExt};
use nostr::domain::Event;
use serde_json::{Value, json};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

pub struct Relay {
    socket: Socket,
    next: u64,
}

const WAIT: Duration = Duration::from_secs(10);

impl Relay {
    pub async fn connect(url: &str) -> Result<Self> {
        let (socket, _) = tokio::time::timeout(WAIT, connect_async(url))
            .await
            .context("connecting to the relay timed out")?
            .with_context(|| format!("connecting to {url}"))?;
        Ok(Relay { socket, next: 0 })
    }

    async fn send(&mut self, value: Value) -> Result<()> {
        self.socket
            .send(Message::Text(value.to_string().into()))
            .await?;
        Ok(())
    }

    async fn receive(&mut self) -> Result<Value> {
        loop {
            let message = tokio::time::timeout(WAIT, self.socket.next())
                .await
                .context("the relay stopped answering")?
                .context("the relay closed the connection")??;
            if let Message::Text(text) = message {
                return Ok(serde_json::from_str(&text)?);
            }
        }
    }

    /// Returns the stored events that match `filter`.
    pub async fn query(&mut self, filter: Value) -> Result<Vec<Event>> {
        self.next += 1;
        let id = format!("q{}", self.next);
        self.send(json!(["REQ", id, filter])).await?;
        let mut events = Vec::new();
        loop {
            let message = self.receive().await?;
            match (message[0].as_str(), message[1].as_str()) {
                (Some("EVENT"), Some(sub)) if sub == id => {
                    events.push(serde_json::from_value(message[2].clone())?);
                }
                (Some("EOSE"), Some(sub)) if sub == id => break,
                (Some("CLOSED"), Some(sub)) if sub == id => {
                    bail!("the relay closed the query: {}", message[2]);
                }
                _ => {}
            }
        }
        self.send(json!(["CLOSE", id])).await?;
        Ok(events)
    }

    /// Publishes an event and waits for the relay to accept it.
    pub async fn publish(&mut self, event: &Event) -> Result<()> {
        self.send(json!(["EVENT", event])).await?;
        loop {
            let message = self.receive().await?;
            if message[0] == "OK" && message[1] == event.id.as_str() {
                if message[2] == true {
                    return Ok(());
                }
                bail!("the relay refused the pointer: {}", message[3]);
            }
        }
    }
}

/// Follows `filters` on the relay and sends each event to `events`,
/// reconnecting after a dropped connection. Returns when the receiver is
/// gone.
pub async fn follow(url: String, filters: Vec<Value>, events: mpsc::Sender<Event>) {
    let mut delay = Duration::from_millis(100);
    loop {
        match follow_once(&url, &filters, &events).await {
            Ok(()) => return,
            Err(error) => eprintln!("livehost: relay {url}: {error:#}; reconnecting"),
        }
        tokio::time::sleep(delay).await;
        delay = (delay * 2).min(Duration::from_secs(5));
    }
}

async fn follow_once(url: &str, filters: &[Value], events: &mpsc::Sender<Event>) -> Result<()> {
    let (mut socket, _) = connect_async(url).await?;
    let mut request = vec![json!("REQ"), json!("pointers")];
    request.extend(filters.iter().cloned());
    socket
        .send(Message::Text(Value::Array(request).to_string().into()))
        .await?;
    while let Some(message) = socket.next().await {
        let Message::Text(text) = message? else {
            continue;
        };
        let value: Value = serde_json::from_str(&text)?;
        match value[0].as_str() {
            Some("EVENT") => match serde_json::from_value::<Event>(value[2].clone()) {
                Ok(event) => {
                    if events.send(event).await.is_err() {
                        return Ok(());
                    }
                }
                Err(error) => eprintln!("livehost: malformed event from the relay: {error}"),
            },
            Some("CLOSED") => bail!("the relay closed the subscription: {}", value[2]),
            Some("NOTICE") => eprintln!("livehost: relay notice: {}", value[1]),
            _ => {}
        }
    }
    bail!("the relay closed the connection")
}
