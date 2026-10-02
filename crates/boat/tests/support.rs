#![allow(dead_code)]
use std::{collections::BTreeMap, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

#[derive(Debug)]
pub struct Request {
    pub method: String,
    pub target: String,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
}

/// One canned HTTP response.
pub struct Reply {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Reply {
    pub fn new(status: u16, headers: &[(&str, &str)], body: &[u8]) -> Self {
        Self {
            status,
            headers: headers
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            body: body.to_vec(),
        }
    }

    fn bytes(&self) -> Vec<u8> {
        let mut response = format!(
            "HTTP/1.1 {} Test\r\nContent-Length: {}\r\nConnection: close\r\n",
            self.status,
            self.body.len()
        );
        for (name, value) in &self.headers {
            response.push_str(&format!("{name}: {value}\r\n"));
        }
        response.push_str("\r\n");
        let mut response = response.into_bytes();
        response.extend_from_slice(&self.body);
        response
    }
}

async fn handle(mut socket: TcpStream, reply: &Reply) -> Request {
    let mut bytes = Vec::new();
    let split = loop {
        let mut chunk = [0; 4096];
        let len = socket.read(&mut chunk).await.expect("read");
        assert!(len > 0);
        bytes.extend_from_slice(&chunk[..len]);
        if let Some(at) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
            break at;
        }
    };
    let header = String::from_utf8(bytes[..split].to_vec()).expect("header");
    let mut lines = header.split("\r\n");
    let mut first = lines.next().expect("request line").split_whitespace();
    let method = first.next().expect("method").into();
    let target = first.next().expect("target").into();
    let headers: BTreeMap<String, String> = lines
        .map(|line| {
            let (key, value) = line.split_once(':').expect("header pair");
            (key.to_lowercase(), value.trim().into())
        })
        .collect();
    let length = headers
        .get("content-length")
        .map(|s| s.parse::<usize>().expect("length"))
        .unwrap_or(0);
    while bytes.len() < split + 4 + length {
        let mut chunk = [0; 4096];
        let len = socket.read(&mut chunk).await.expect("body");
        assert!(len > 0);
        bytes.extend_from_slice(&chunk[..len]);
    }
    socket.write_all(&reply.bytes()).await.expect("respond");
    Request {
        method,
        target,
        headers,
        body: bytes[split + 4..].to_vec(),
    }
}

async fn listen() -> (TcpListener, String) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("listen");
    let base = format!("http://{}/api/v1", listener.local_addr().expect("address"));
    (listener, base)
}

pub fn fast_retries() -> boat::RetryPolicy {
    boat::RetryPolicy {
        max_retries: 2,
        base_delay: Duration::from_millis(1),
        max_delay: Duration::from_secs(1),
    }
}

pub fn builder(base: String) -> boat::ClientBuilder {
    boat::Client::builder(boat::ApiKey::new("test-secret").expect("key"))
        .base_url(base)
        .retry(fast_retries())
}

/// Answer exactly one request.
pub async fn serve(
    status: u16,
    headers: &[(&str, &str)],
    body: &[u8],
) -> (boat::Client, tokio::task::JoinHandle<Request>) {
    let (listener, base) = listen().await;
    let reply = Reply::new(status, headers, body);
    let job = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.expect("accept");
        handle(socket, &reply).await
    });
    (builder(base).build().expect("client"), job)
}

/// Answer requests in order, one reply each. The job yields the requests it
/// saw once the client is dropped or the listener stops being used; await it
/// with a timeout via [`collect`].
pub async fn serve_sequence(
    replies: Vec<Reply>,
    configure: impl FnOnce(boat::ClientBuilder) -> boat::ClientBuilder,
) -> (boat::Client, tokio::task::JoinHandle<Vec<Request>>) {
    let (listener, base) = listen().await;
    let job = tokio::spawn(async move {
        let mut seen = Vec::new();
        for reply in &replies {
            match tokio::time::timeout(Duration::from_millis(500), listener.accept()).await {
                Ok(Ok((socket, _))) => seen.push(handle(socket, reply).await),
                _ => break,
            }
        }
        seen
    });
    (configure(builder(base)).build().expect("client"), job)
}
