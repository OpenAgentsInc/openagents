//! Guild channels: a synchronous Nostr client for one relay.
//!
//! Each enrolled agent holds one websocket to the episode's relay,
//! answers its NIP-42 challenge as its own derived key, and speaks in
//! its guild's NIP-29 group with C7 `kind:9` chat events carrying the
//! `h` tag. Membership is the relay's admission, not a flag here: a
//! member of another guild publishing to this channel is refused with
//! `restricted:`, which is exactly the evidence the run records.
//!
//! The client is deliberately small — one socket, one outstanding
//! request pattern at a time, bounded reads. It is a participant, not a
//! relay implementation.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use nostr::domain::{Event, RelaySigner, Tag};
use serde_json::{Value, json};
use tungstenite::{Message, WebSocket, stream::MaybeTlsStream};

use crate::error::{Error, Result};

/// Kind-9 chat: the C7 message kind a guild channel carries.
pub const CHAT_KIND: u16 = 9;
/// The NIP-42 authentication kind.
const AUTH_KIND: u16 = 22_242;
/// The NIP-98 HTTP-auth kind the management endpoint wants.
const HTTP_AUTH_KIND: u16 = 27_235;
/// How long any single wait may block.
const IO_WAIT: Duration = Duration::from_secs(20);

/// Unix seconds now.
#[must_use]
pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or(0)
}

/// One websocket to the relay.
pub struct Channel {
    socket: WebSocket<MaybeTlsStream<TcpStream>>,
    url: String,
    /// A counter for subscription ids.
    subs: u64,
}

/// What the relay said about a published event.
#[derive(Clone, Debug)]
pub struct Verdict {
    /// Whether the relay accepted the event.
    pub accepted: bool,
    /// The relay's message — empty on a plain accept, `restricted: …`
    /// or similar on a refusal.
    pub message: String,
}

impl Channel {
    /// Opens a websocket to `url` and answers the relay's NIP-42
    /// challenge as `signer`, when the relay issues one.
    ///
    /// # Errors
    ///
    /// The handshake must succeed and a challenge, if sent, must be
    /// answered inside [`IO_WAIT`].
    pub fn connect(url: &str, signer: &RelaySigner) -> Result<Self> {
        let (mut socket, _response) = tungstenite::connect(url)
            .map_err(|error| Error::relay(format!("connect {url}: {error}")))?;
        set_timeouts(&mut socket);
        let mut channel = Channel {
            socket,
            url: url.to_string(),
            subs: 0,
        };
        channel.authenticate(signer)?;
        Ok(channel)
    }

    /// NIP-42: if the relay challenges, sign kind `22242` and wait for
    /// its `OK`. An open relay may never challenge; any other first
    /// frame ends the wait.
    fn authenticate(&mut self, signer: &RelaySigner) -> Result<()> {
        let deadline = Instant::now() + IO_WAIT;
        loop {
            let Some(value) = self.read_frame(deadline)? else {
                return Ok(());
            };
            match value[0].as_str().unwrap_or_default() {
                "AUTH" => {
                    let Some(challenge) = value[1].as_str() else {
                        return Err(Error::relay("malformed AUTH challenge"));
                    };
                    let event = signer.sign(
                        unix_now(),
                        AUTH_KIND,
                        vec![
                            Tag::new(vec!["relay".into(), self.url.clone()]),
                            Tag::new(vec!["challenge".into(), challenge.to_string()]),
                        ],
                        String::new(),
                    );
                    let auth_id = event.id.clone();
                    self.send(&json!(["AUTH", event]))?;
                    let verdict = self.wait_verdict(&auth_id, deadline)?;
                    if !verdict.accepted {
                        return Err(Error::relay(format!("NIP-42 refused: {}", verdict.message)));
                    }
                    return Ok(());
                }
                "NOTICE" => continue,
                _ => return Ok(()),
            }
        }
    }

    /// Publishes `event` and waits for the relay's `OK` verdict.
    ///
    /// # Errors
    ///
    /// The verdict must arrive inside [`IO_WAIT`]. A refusal is a
    /// `Verdict`, not an error — the caller decides what it means.
    pub fn publish(&mut self, event: &Event) -> Result<Verdict> {
        let id = event.id.clone();
        self.send(&json!(["EVENT", event]))?;
        self.wait_verdict(&id, Instant::now() + IO_WAIT)
    }

    /// A guild chat message: kind 9, `h`-tagged to `group`, signed by
    /// the agent whose channel this is.
    pub fn chat(&mut self, signer: &RelaySigner, group: &str, text: &str) -> Result<Verdict> {
        let event = signer.sign(
            unix_now(),
            CHAT_KIND,
            vec![Tag::new(vec!["h".into(), group.to_string()])],
            text.to_string(),
        );
        self.publish(&event)
    }

    /// Reads stored events matching `filter` until `EOSE`. Public group
    /// reads need no membership — the relay's visibility rules decide.
    ///
    /// # Errors
    ///
    /// `EOSE` must arrive inside [`IO_WAIT`]; a `CLOSED` answer is a
    /// relay refusal and returns as an error with its message.
    pub fn read(&mut self, filter: Value) -> Result<Vec<Event>> {
        self.subs += 1;
        let sub = format!("r{}", self.subs);
        self.send(&json!(["REQ", sub, filter]))?;
        let deadline = Instant::now() + IO_WAIT;
        let mut events = Vec::new();
        loop {
            let value = self
                .read_frame(deadline)?
                .ok_or_else(|| Error::relay("the socket closed before EOSE".to_string()))?;
            match value[0].as_str().unwrap_or_default() {
                "EVENT" if value[1].as_str() == Some(sub.as_str()) => {
                    if let Ok(event) = serde_json::from_value::<Event>(value[2].clone()) {
                        events.push(event);
                    }
                }
                "EOSE" if value[1].as_str() == Some(sub.as_str()) => break,
                "CLOSED" if value[1].as_str() == Some(sub.as_str()) => {
                    return Err(Error::relay(format!(
                        "subscription refused: {}",
                        value[2].as_str().unwrap_or_default()
                    )));
                }
                "NOTICE" => {}
                _ => {}
            }
            if Instant::now() >= deadline {
                return Err(Error::relay("no EOSE inside the wait".to_string()));
            }
        }
        self.send(&json!(["CLOSE", sub]))?;
        Ok(events)
    }

    /// Waits for `["OK", id, accepted, message]`.
    fn wait_verdict(&mut self, id: &str, deadline: Instant) -> Result<Verdict> {
        loop {
            let value = self
                .read_frame(deadline)?
                .ok_or_else(|| Error::relay("the socket closed before the OK".to_string()))?;
            if value[0].as_str() == Some("OK") && value[1].as_str() == Some(id) {
                return Ok(Verdict {
                    accepted: value[2].as_bool().unwrap_or(false),
                    message: value[3].as_str().unwrap_or_default().to_string(),
                });
            }
            if Instant::now() >= deadline {
                return Err(Error::relay(format!("no OK for {id} inside the wait")));
            }
        }
    }

    /// One inbound frame as JSON, or `None` when the socket closed.
    fn read_frame(&mut self, deadline: Instant) -> Result<Option<Value>> {
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(Error::relay("timed out waiting on the relay".to_string()));
            }
            match self.socket.read() {
                Ok(Message::Text(text)) => match serde_json::from_str::<Value>(&text) {
                    Ok(value) => return Ok(Some(value)),
                    Err(_) => continue,
                },
                Ok(Message::Ping(bytes)) => {
                    let _ = self.socket.send(Message::Pong(bytes));
                }
                Ok(_) => continue,
                Err(tungstenite::Error::Io(error))
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
                {
                    continue;
                }
                Err(_) => return Ok(None),
            }
        }
    }

    /// Writes one JSON message.
    fn send(&mut self, value: &Value) -> Result<()> {
        self.socket
            .send(Message::Text(value.to_string().into()))
            .map_err(|error| Error::relay(format!("send: {error}")))
    }
}

/// One NIP-86 management call: `POST` `application/nostr+json+rpc`
/// authenticated by a NIP-98 event `signer` holds. The body is
/// `{"method": …, "params": […]}`; the answer is the relay's JSON.
///
/// # Errors
///
/// The endpoint must answer inside [`IO_WAIT`]; a relay `error` field
/// surfaces as an error carrying its text.
pub fn manage(http_url: &str, signer: &RelaySigner, method: &str, params: Value) -> Result<Value> {
    let body = serde_json::to_vec(&json!({"method": method, "params": params}))
        .map_err(|error| Error::relay(format!("rpc body: {error}")))?;
    let payload_hash = {
        use sha2::Digest;
        sha2::Sha256::digest(&body)
    };
    let auth = signer.sign(
        unix_now(),
        HTTP_AUTH_KIND,
        vec![
            Tag::new(vec!["u".into(), format!("{http_url}/")]),
            Tag::new(vec!["method".into(), "POST".into()]),
            Tag::new(vec![
                "payload".into(),
                payload_hash.iter().map(|b| format!("{b:02x}")).collect(),
            ]),
        ],
        String::new(),
    );
    let header = format!(
        "Nostr {}",
        nostr::nip44::primitives::base64_encode(
            &serde_json::to_vec(&auth).map_err(|error| Error::relay(format!("auth: {error}")))?
        )
    );
    let address = http_url
        .strip_prefix("http://")
        .and_then(|rest| rest.split('/').next())
        .ok_or_else(|| Error::relay(format!("bad management url {http_url}")))?;
    let mut stream = TcpStream::connect(address)
        .map_err(|error| Error::relay(format!("management connect {address}: {error}")))?;
    let _ = stream.set_read_timeout(Some(IO_WAIT));
    let _ = stream.set_write_timeout(Some(IO_WAIT));
    let request = format!(
        "POST / HTTP/1.1\r\nHost: {address}\r\nContent-Type: application/nostr+json+rpc\r\nAuthorization: {header}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream
        .write_all(request.as_bytes())
        .and_then(|()| stream.write_all(&body))
        .map_err(|error| Error::relay(format!("management send: {error}")))?;
    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .map_err(|error| Error::relay(format!("management read: {error}")))?;
    let text = String::from_utf8_lossy(&response);
    let Some((_, body_text)) = text.split_once("\r\n\r\n") else {
        return Err(Error::relay(format!("bad management reply: {text}")));
    };
    let answer: Value = serde_json::from_str(body_text)
        .map_err(|error| Error::relay(format!("management reply: {error}")))?;
    if let Some(error) = answer.get("error").and_then(Value::as_str) {
        return Err(Error::relay(format!("management {method}: {error}")));
    }
    Ok(answer)
}

/// Read timeouts so a silent relay cannot hold the episode.
fn set_timeouts(socket: &mut WebSocket<MaybeTlsStream<TcpStream>>) {
    if let MaybeTlsStream::Plain(stream) = socket.get_mut() {
        let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
        let _ = stream.set_nodelay(true);
    }
}
