use std::{
    collections::HashMap,
    io::{self, Cursor, Read, Write},
    net::IpAddr,
    time::Duration,
};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::timeout,
};
use tokio_tungstenite::tungstenite::{
    ServerHandshake, WebSocket,
    handshake::{HandshakeError, server::NoCallback},
    protocol::WebSocketConfig,
};

use super::GatewayError;

const MAX_HTTP_HEAD_BYTES: usize = 16_384;
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

pub struct HttpHead {
    pub method: String,
    pub path: String,
    pub(crate) headers: HashMap<String, String>,
}

impl HttpHead {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .get(&name.to_ascii_lowercase())
            .map(String::as_str)
    }
}

pub struct SocketIo {
    stream: TcpStream,
    prefix: Cursor<Vec<u8>>,
}

pub type ServerWebSocket = WebSocket<SocketIo>;

pub async fn read_http_head(stream: &mut TcpStream) -> Result<(Vec<u8>, HttpHead), GatewayError> {
    timeout(HANDSHAKE_TIMEOUT, read_http_head_inner(stream))
        .await
        .map_err(|_| GatewayError::Io(io::Error::new(io::ErrorKind::TimedOut, "HTTP handshake")))?
}

async fn read_http_head_inner(stream: &mut TcpStream) -> Result<(Vec<u8>, HttpHead), GatewayError> {
    let mut bytes = Vec::with_capacity(1_024);
    let mut byte = [0_u8; 1];
    while bytes.len() < MAX_HTTP_HEAD_BYTES {
        let read = stream.read(&mut byte).await?;
        if read == 0 {
            return Err(GatewayError::Io(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "connection closed during HTTP handshake",
            )));
        }
        bytes.push(byte[0]);
        if bytes.ends_with(b"\r\n\r\n") {
            return Ok((bytes.clone(), parse_http_head(&bytes)?));
        }
    }
    Err(GatewayError::Io(io::Error::new(
        io::ErrorKind::InvalidData,
        "HTTP headers exceed 16384 bytes",
    )))
}

fn parse_http_head(bytes: &[u8]) -> Result<HttpHead, GatewayError> {
    let text = std::str::from_utf8(bytes).map_err(|_| {
        GatewayError::Io(io::Error::new(
            io::ErrorKind::InvalidData,
            "HTTP headers are not UTF-8",
        ))
    })?;
    let mut lines = text.split("\r\n");
    let mut request = lines
        .next()
        .ok_or_else(|| invalid_http("missing request line"))?
        .split_whitespace();
    let method = request
        .next()
        .ok_or_else(|| invalid_http("missing method"))?
        .to_owned();
    let path = request
        .next()
        .ok_or_else(|| invalid_http("missing path"))?
        .to_owned();
    let version = request
        .next()
        .ok_or_else(|| invalid_http("missing version"))?;
    if request.next().is_some() || !matches!(version, "HTTP/1.0" | "HTTP/1.1") {
        return Err(invalid_http("invalid request line"));
    }
    let mut headers = HashMap::new();
    for line in lines.take_while(|line| !line.is_empty()) {
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| invalid_http("malformed header"))?;
        let name = name.trim().to_ascii_lowercase();
        if name.is_empty()
            || !name
                .as_bytes()
                .iter()
                .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(byte))
        {
            return Err(invalid_http("invalid header name"));
        }
        let value = value.trim();
        headers
            .entry(name)
            .and_modify(|current: &mut String| {
                current.push_str(", ");
                current.push_str(value);
            })
            .or_insert_with(|| value.to_owned());
    }
    Ok(HttpHead {
        method,
        path,
        headers,
    })
}

pub fn is_websocket_upgrade(head: &HttpHead) -> bool {
    head.method == "GET"
        && head
            .headers
            .get("upgrade")
            .is_some_and(|value| value.eq_ignore_ascii_case("websocket"))
        && head.headers.get("connection").is_some_and(|value| {
            value
                .split(',')
                .any(|token| token.trim().eq_ignore_ascii_case("upgrade"))
        })
}

pub fn effective_ip(head: &HttpHead, peer: IpAddr, trust_proxy: bool) -> IpAddr {
    if !trust_proxy {
        return peer;
    }
    head.headers
        .get("x-forwarded-for")
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .and_then(|value| value.parse().ok())
        .or_else(|| {
            head.headers
                .get("x-real-ip")
                .and_then(|value| value.trim().parse().ok())
        })
        .unwrap_or(peer)
}

pub async fn serve_http(
    mut stream: TcpStream,
    head: &HttpHead,
    nip11: &str,
    current: bool,
) -> Result<(), GatewayError> {
    if head.method == "OPTIONS" {
        return write_http(&mut stream, 204, "No Content", "text/plain", "").await;
    }
    if head.method != "GET" {
        return write_http(
            &mut stream,
            405,
            "Method Not Allowed",
            "text/plain; charset=utf-8",
            "method not allowed\n",
        )
        .await;
    }
    if head.path == "/health" {
        let (status, reason, body) = if current {
            (200, "OK", "{\"status\":\"ok\"}")
        } else {
            (503, "Service Unavailable", "{\"status\":\"unavailable\"}")
        };
        return write_http(&mut stream, status, reason, "application/json", body).await;
    }
    if head.headers.get("accept").is_some_and(|value| {
        value
            .split(',')
            .any(|token| token.trim().eq_ignore_ascii_case("application/nostr+json"))
    }) {
        return write_http(&mut stream, 200, "OK", "application/nostr+json", nip11).await;
    }
    write_http(
        &mut stream,
        426,
        "Upgrade Required",
        "text/plain; charset=utf-8",
        "nostr-relay is a Nostr WebSocket relay.\n",
    )
    .await
}

pub async fn read_http_body(
    stream: &mut TcpStream,
    head: &HttpHead,
    max_bytes: usize,
) -> Result<Vec<u8>, GatewayError> {
    if head.header("transfer-encoding").is_some() {
        return Err(invalid_http("chunked request bodies are not supported"));
    }
    let length = head
        .header("content-length")
        .ok_or_else(|| invalid_http("Content-Length is required"))?
        .parse::<usize>()
        .map_err(|_| invalid_http("invalid Content-Length"))?;
    if length > max_bytes {
        return Err(invalid_http("request body exceeds configured limit"));
    }
    let mut body = vec![0_u8; length];
    timeout(HANDSHAKE_TIMEOUT, stream.read_exact(&mut body))
        .await
        .map_err(|_| GatewayError::Io(io::Error::new(io::ErrorKind::TimedOut, "HTTP body")))??;
    Ok(body)
}

pub async fn write_http(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    content_type: &str,
    body: &str,
) -> Result<(), GatewayError> {
    write_http_bytes(stream, status, reason, content_type, body.as_bytes(), &[]).await
}

pub async fn write_http_bytes(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    content_type: &str,
    body: &[u8],
    extra_headers: &[(&str, String)],
) -> Result<(), GatewayError> {
    write_http_head(
        stream,
        status,
        reason,
        content_type,
        body.len() as u64,
        extra_headers,
    )
    .await?;
    stream.write_all(body).await?;
    stream.shutdown().await?;
    Ok(())
}

pub async fn write_http_head(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    content_type: &str,
    content_length: u64,
    extra_headers: &[(&str, String)],
) -> Result<(), GatewayError> {
    let mut response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {content_length}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: Accept, Authorization, Content-Type, Range, X-SHA-256\r\nAccess-Control-Expose-Headers: Content-Length, Content-Range, Location, X-SHA-256\r\nAccess-Control-Allow-Methods: GET, HEAD, PUT, POST, DELETE, OPTIONS\r\nConnection: close\r\n"
    );
    for (name, value) in extra_headers {
        response.push_str(name);
        response.push_str(": ");
        response.push_str(value);
        response.push_str("\r\n");
    }
    response.push_str("\r\n");
    stream.write_all(response.as_bytes()).await?;
    Ok(())
}

pub async fn shutdown_http(stream: &mut TcpStream) -> Result<(), GatewayError> {
    stream.shutdown().await?;
    Ok(())
}

pub async fn websocket_handshake(
    stream: TcpStream,
    request_head: Vec<u8>,
    max_message_bytes: usize,
) -> Result<ServerWebSocket, GatewayError> {
    let io = SocketIo {
        stream,
        prefix: Cursor::new(request_head),
    };
    let config = WebSocketConfig::default()
        .read_buffer_size(4_096)
        .write_buffer_size(0)
        .max_write_buffer_size(max_message_bytes.saturating_mul(2).saturating_add(4_096))
        .max_message_size(Some(max_message_bytes))
        .max_frame_size(Some(max_message_bytes));
    let mut handshake = ServerHandshake::start(io, NoCallback, Some(config));
    loop {
        match handshake.handshake() {
            Ok(websocket) => return Ok(websocket),
            Err(HandshakeError::Interrupted(next)) => {
                handshake = next;
                handshake.get_ref().get_ref().stream.writable().await?;
            }
            Err(HandshakeError::Failure(error)) => return Err(error.into()),
        }
    }
}

impl SocketIo {
    pub fn stream(&self) -> &TcpStream {
        &self.stream
    }
}

impl Read for SocketIo {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.prefix.position() < self.prefix.get_ref().len() as u64 {
            return Read::read(&mut self.prefix, buffer);
        }
        self.stream.try_read(buffer)
    }
}

impl Write for SocketIo {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.stream.try_write(buffer)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn invalid_http(reason: &str) -> GatewayError {
    GatewayError::Io(io::Error::new(io::ErrorKind::InvalidData, reason))
}
