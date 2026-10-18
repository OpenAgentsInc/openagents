#![allow(dead_code)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use archil::{Archil, ClientOptions};
use serde_json::{Value, json};
use wiremock::{MockServer, Request, Respond, ResponseTemplate};

/// A client pointed at the mock server for both the control plane and S3,
/// polling quickly so wait loops finish fast.
pub fn client(server: &MockServer) -> Archil {
    Archil::new(
        ClientOptions::new()
            .api_key("test-key")
            .region("aws-us-east-1")
            .base_url(server.uri())
            .s3_base_url(server.uri())
            .poll_interval(Duration::from_millis(10)),
    )
    .expect("client")
}

pub fn ok(data: Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({ "success": true, "data": data }))
}

pub fn status(code: u16, data: Value) -> ResponseTemplate {
    ResponseTemplate::new(code).set_body_json(json!({ "success": true, "data": data }))
}

pub fn err(code: u16, message: &str, error_code: Option<&str>) -> ResponseTemplate {
    let mut body = json!({ "success": false, "error": message });
    if let Some(c) = error_code {
        body["code"] = c.into();
    }
    ResponseTemplate::new(code).set_body_json(body)
}

/// Replays responses in order; the last one repeats.
pub struct Seq {
    responses: Vec<ResponseTemplate>,
    next: AtomicUsize,
    pub hits: Arc<AtomicUsize>,
}

impl Seq {
    pub fn new(responses: Vec<ResponseTemplate>) -> Self {
        Self {
            responses,
            next: AtomicUsize::new(0),
            hits: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl Respond for Seq {
    fn respond(&self, _: &Request) -> ResponseTemplate {
        self.hits.fetch_add(1, Ordering::SeqCst);
        let i = self
            .next
            .fetch_add(1, Ordering::SeqCst)
            .min(self.responses.len() - 1);
        self.responses[i].clone()
    }
}

/// Records every request body it sees and answers with a fixed response.
pub struct Capture {
    pub bodies: Arc<Mutex<Vec<Value>>>,
    pub response: ResponseTemplate,
}

impl Capture {
    pub fn new(response: ResponseTemplate) -> (Self, Arc<Mutex<Vec<Value>>>) {
        let bodies = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                bodies: bodies.clone(),
                response,
            },
            bodies,
        )
    }
}

impl Respond for Capture {
    fn respond(&self, req: &Request) -> ResponseTemplate {
        let v = serde_json::from_slice(&req.body).unwrap_or(Value::Null);
        self.bodies.lock().unwrap().push(v);
        self.response.clone()
    }
}

pub fn disk_json(id: &str, name: &str) -> Value {
    json!({
        "id": id, "name": name, "organization": "acme", "status": "available",
        "provider": "aws", "region": "aws-us-east-1", "createdAt": "2026-01-01T00:00:00Z",
        "sandboxDisk": false
    })
}

pub fn sandbox_json(id: &str, status: &str) -> Value {
    json!({
        "sandbox_id": id, "name": "sb", "status": status, "vcpu_count": 1, "mem_size_mib": 2048,
        "max_ttl_seconds": 86400, "idle_ttl_seconds": 0, "max_concurrent_execs": 8,
        "base_image": "ubuntu", "enable_service_ingress": false,
        "created_at": "2026-01-01T00:00:00Z", "last_active_at": "2026-01-01T00:00:00Z"
    })
}

// ------------------------------------------------------------------ fake process runtime

pub mod ws {
    use std::future::Future;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use futures_util::{SinkExt, StreamExt};
    use serde_json::{Value, json};
    use tokio::net::{TcpListener, TcpStream};
    use tokio_tungstenite::WebSocketStream;
    use tokio_tungstenite::tungstenite::Message;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer};

    pub type Ws = WebSocketStream<TcpStream>;

    /// Listen for WebSocket connections; `handler(socket, connection_index)` runs per connection.
    pub async fn runtime<F, Fut>(handler: F) -> String
    where
        F: Fn(Ws, usize) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}/ws", listener.local_addr().unwrap());
        let handler = Arc::new(handler);
        let count = Arc::new(AtomicUsize::new(0));
        tokio::spawn(async move {
            loop {
                let Ok((tcp, _)) = listener.accept().await else {
                    return;
                };
                let handler = handler.clone();
                let index = count.fetch_add(1, Ordering::SeqCst);
                tokio::spawn(async move {
                    if let Ok(ws) = tokio_tungstenite::accept_async(tcp).await {
                        handler(ws, index).await;
                    }
                });
            }
        });
        url
    }

    /// Mount `POST /connections` for the given sandbox id, answering with each URL in turn
    /// (the last repeats).
    pub async fn mount_connections(
        server: &MockServer,
        sid: &str,
        urls: Vec<String>,
    ) -> Arc<AtomicUsize> {
        let responses = urls
            .into_iter()
            .map(|u| super::ok(json!({"url": u, "expires_at": "2099-01-01T00:00:00Z"})))
            .collect();
        let seq = super::Seq::new(responses);
        let hits = seq.hits.clone();
        Mock::given(method("POST"))
            .and(path(format!("/api/sandboxes/{sid}/connections")))
            .respond_with(seq)
            .mount(server)
            .await;
        hits
    }

    /// Next text message parsed as JSON; panics if the peer closes first.
    pub async fn recv_json(ws: &mut Ws) -> Value {
        loop {
            match ws.next().await {
                Some(Ok(Message::Text(t))) => return serde_json::from_str(&t).unwrap(),
                Some(Ok(Message::Ping(_) | Message::Pong(_))) => continue,
                other => panic!("expected a text frame, got {other:?}"),
            }
        }
    }

    /// Next binary message.
    pub async fn recv_bin(ws: &mut Ws) -> Vec<u8> {
        loop {
            match ws.next().await {
                Some(Ok(Message::Binary(b))) => return b.to_vec(),
                Some(Ok(Message::Ping(_) | Message::Pong(_))) => continue,
                other => panic!("expected a binary frame, got {other:?}"),
            }
        }
    }

    pub async fn send_json(ws: &mut Ws, v: Value) {
        ws.send(Message::Text(v.to_string().into())).await.unwrap();
    }

    /// Output frame: 1 = stdout, 2 = stderr; big-endian u64 offset; payload.
    pub async fn send_out(ws: &mut Ws, stream: u8, offset: u64, data: &[u8]) {
        let mut frame = vec![stream];
        frame.extend_from_slice(&offset.to_be_bytes());
        frame.extend_from_slice(data);
        ws.send(Message::Binary(frame.into())).await.unwrap();
    }

    pub async fn started(ws: &mut Ws, id: &str) {
        send_json(ws, json!({"type": "started", "process_id": id})).await;
    }

    pub async fn exit(ws: &mut Ws, status: &str, cursor: u64, code: Option<i64>) {
        send_json(
            ws,
            json!({"type": "exit", "status": status, "cursor": cursor, "exit_code": code}),
        )
        .await;
    }
}
