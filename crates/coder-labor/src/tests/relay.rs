//! Local Nostr fixture, not the production relay or a persistence benchmark.
use super::*;
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio_tungstenite::{
    accept_async_with_config,
    tungstenite::{Message, protocol::WebSocketConfig},
};

pub async fn start() -> (
    String,
    tokio::task::JoinHandle<()>,
    Arc<Mutex<BTreeMap<String, Event>>>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    let events = Arc::new(Mutex::new(BTreeMap::new()));
    let kept = events.clone();
    let address = url.clone();
    let task = tokio::spawn(async move {
        while let Ok((socket, _)) = listener.accept().await {
            let url = address.clone();
            let events = kept.clone();
            tokio::spawn(async move {
                let _ = serve(socket, &url, events).await;
            });
        }
    });
    (url, task, events)
}
async fn serve(
    stream: TcpStream,
    url: &str,
    events: Arc<Mutex<BTreeMap<String, Event>>>,
) -> Result<()> {
    let mut socket = accept_async_with_config(
        stream,
        Some(WebSocketConfig::default().max_message_size(Some(1024 * 1024))),
    )
    .await
    .map_err(|e| e.to_string())?;
    let challenge = secp256k1::rand::random::<u128>().to_string();
    socket
        .send(Message::Text(json!(["AUTH", challenge]).to_string().into()))
        .await
        .map_err(|e| e.to_string())?;
    let mut principal = None;
    while let Some(message) = socket.next().await {
        let text = match message.map_err(|e| e.to_string())? {
            Message::Text(text) => text,
            Message::Close(_) => return Ok(()),
            _ => return Err("unexpected fixture relay frame".into()),
        };
        let value: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
        let response = match value[0].as_str() {
            Some("AUTH") => {
                let event: Event =
                    serde_json::from_value(value[1].clone()).map_err(|e| e.to_string())?;
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                let good = event.kind == 22242
                    && event.validate_crypto().is_ok()
                    && event.content.is_empty()
                    && event.tags.len() == 2
                    && event.tag_values("relay").collect::<Vec<_>>() == [url]
                    && event.tag_values("challenge").collect::<Vec<_>>() == [challenge.as_str()]
                    && event.created_at.abs_diff(now) <= 60;
                if good {
                    principal = Some(event.pubkey.clone());
                }
                json!([
                    "OK",
                    event.id,
                    good,
                    if good {
                        ""
                    } else {
                        "auth-required: invalid proof"
                    }
                ])
            }
            Some("EVENT") => {
                let event: Event =
                    serde_json::from_value(value[1].clone()).map_err(|e| e.to_string())?;
                let good = principal.as_ref() == Some(&event.pubkey)
                    && private_artifact::admit(&event).is_ok();
                if good {
                    events
                        .lock()
                        .await
                        .entry(event.id.clone())
                        .or_insert(event.clone());
                }
                json!([
                    "OK",
                    event.id,
                    good,
                    if good {
                        ""
                    } else {
                        "auth-required: invalid author"
                    }
                ])
            }
            Some("REQ") => {
                let id = value[2]["ids"][0]
                    .as_str()
                    .ok_or("fixture exact event filter")?;
                let readers = principal.iter().cloned().collect();
                let candidate = events.lock().await.get(id).cloned();
                if let Some(event) = candidate.filter(|e| private_artifact::visible(e, &readers)) {
                    socket
                        .send(Message::Text(
                            json!(["EVENT", value[1], event]).to_string().into(),
                        ))
                        .await
                        .map_err(|e| e.to_string())?;
                }
                json!(["EOSE", value[1]])
            }
            _ => return Err("unsupported fixture relay command".into()),
        };
        socket
            .send(Message::Text(response.to_string().into()))
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
