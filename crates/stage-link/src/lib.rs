//! Direct stage-to-stage links for a model group (#12399, W12).
//!
//! A model group runs its layers as an ordered chain of stages. Stage `i`
//! sends stage-state messages (W9's `StageMessage` bytes) straight to stage
//! `i + 1` over one [`Sender`]; the next stage reads them from a
//! [`Receiver`]. The group's head keeps the control plane (admission,
//! sampling settings, cancellation, generation fencing, and readiness) and
//! is no longer on the path of every hop.
//!
//! A link is one TCP connection, one way, for one hop at one group
//! generation:
//!
//! - **Authenticated.** The head gives every stage a fresh 32-byte [`Key`]
//!   for each generation. The two ends prove they hold it with a
//!   challenge-response over both sides' nonces, and each frame carries an
//!   HMAC-SHA256 over its sequence number and contents under a session key
//!   derived from that exchange. A frame that is replayed, reordered,
//!   altered, or sent with an old generation's key is refused. The link
//!   doesn't encrypt: it is meant for machines inside one VPC and zone.
//! - **Bounded.** A receiver refuses a frame longer than its [`Limits`]
//!   before reading its body. A sender holds at most [`Limits::queue`]
//!   frames; when the next stage stops reading and the queue is full,
//!   [`Sender::send`] waits up to [`Limits::send_timeout`] and then fails
//!   with [`LinkError::Backpressure`].
//! - **Fenced.** The handshake names the group, the generation, and the
//!   hop (`from`, `to`). A listener accepts only the hop and generation it
//!   expects, and a receiver refuses any frame whose generation differs
//!   from its link's.
//!
//! # Wire format (version 1)
//!
//! Integers are big-endian. `MAC(k, ...)` is HMAC-SHA256 over the
//! concatenated parts.
//!
//! ```text
//! client hello  "OASL" 01 01 generation:u64 from:u16 to:u16 n:u8 group[n] nonce_c[32]
//! server hello  "OASL" 01 02 nonce_s[32] MAC(key, "oasl/1 server", client_hello, nonce_s)
//! client finish "OASL" 01 03 MAC(key, "oasl/1 client", client_hello, nonce_s)
//! accepted      "OASL" 01 04
//! refused       "OASL" 01 05 code:u8
//!
//! session = MAC(key, "oasl/1 session", client_hello, nonce_s)
//!
//! frame  len:u32 body[len - 32] MAC(session, body)
//! body   seq:u64 kind:u8 generation:u64 attempt:u32 step:u64
//!        n:u8 request_id[n] m:u32 meta[m] s:u32 state[s]
//! ```
//!
//! `seq` starts at zero and grows by one for every frame. `kind` is 1 for
//! stage state and 2 for a cancellation travelling down the chain. `meta`
//! is opaque to the link (the head's per-step settings, as the stage
//! protocol defines them); `state` holds the encoded `StageMessage`.

use std::fmt;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, ToSocketAddrs};
use tokio::sync::{Semaphore, mpsc};

/// The four bytes every handshake message starts with.
pub const MAGIC: [u8; 4] = *b"OASL";
/// The wire version this crate speaks.
pub const VERSION: u8 = 1;
/// The bytes of a frame that aren't the request ID, meta, or state: the
/// length prefix, the fixed body fields, and the MAC.
pub const FRAME_OVERHEAD: usize = 4 + BODY_FIXED + MAC_LEN;

const BODY_FIXED: usize = 8 + 1 + 8 + 4 + 8 + 1 + 4 + 4;
const MAC_LEN: usize = 32;
const NONCE_LEN: usize = 32;

const CLIENT_HELLO: u8 = 1;
const SERVER_HELLO: u8 = 2;
const CLIENT_FINISH: u8 = 3;
const ACCEPTED: u8 = 4;
const REFUSED: u8 = 5;

/// A group generation's link key: 32 random bytes the head gives every
/// stage. Its `Debug` output never shows the bytes.
#[derive(Clone, PartialEq, Eq)]
pub struct Key([u8; 32]);

impl Key {
    /// A fresh random key from the operating system.
    ///
    /// # Errors
    ///
    /// When the system's random source can't be read.
    pub fn generate() -> std::io::Result<Self> {
        let mut bytes = [0u8; 32];
        random(&mut bytes)?;
        Ok(Self(bytes))
    }

    #[must_use]
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Read 64 hexadecimal digits.
    ///
    /// # Errors
    ///
    /// When `text` isn't 64 hexadecimal digits.
    pub fn from_hex(text: &str) -> Result<Self, String> {
        let text = text.trim();
        if text.len() != 64 || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("a link key is 64 hexadecimal digits".into());
        }
        let mut bytes = [0u8; 32];
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&text[2 * i..2 * i + 2], 16)
                .map_err(|_| "a link key is 64 hexadecimal digits".to_string())?;
        }
        Ok(Self(bytes))
    }

    #[must_use]
    pub fn to_hex(&self) -> String {
        hex(&self.0)
    }
}

impl fmt::Debug for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Key(..)")
    }
}

#[cfg(unix)]
fn random(bytes: &mut [u8]) -> std::io::Result<()> {
    use std::io::Read;
    std::fs::File::open("/dev/urandom")?.read_exact(bytes)
}

#[cfg(not(unix))]
fn random(_: &mut [u8]) -> std::io::Result<()> {
    Err(std::io::Error::other("stage links need a Unix host"))
}

fn hex(bytes: &[u8]) -> String {
    use fmt::Write;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// One hop of one group at one generation: stage `from` sends to `to`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hop {
    /// The group's identity both ends agree on, such as its manifest
    /// digest; at most 255 bytes.
    pub group: String,
    pub generation: u64,
    pub from: u16,
    pub to: u16,
}

/// A link's bounds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Limits {
    /// The largest `state` section a frame may carry.
    pub max_state: u32,
    /// The largest `meta` section a frame may carry.
    pub max_meta: u32,
    /// Frames a sender holds while the next stage isn't reading.
    pub queue: usize,
    /// How long [`Sender::send`] waits for room, and how long one frame
    /// may take to write, before the link fails.
    pub send_timeout: Duration,
    /// How long either end waits for each handshake message.
    pub handshake_timeout: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_state: 64 << 20,
            max_meta: 64 << 10,
            queue: 4,
            send_timeout: Duration::from_secs(10),
            handshake_timeout: Duration::from_secs(5),
        }
    }
}

impl Limits {
    fn max_frame(&self) -> usize {
        BODY_FIXED + 255 + self.max_meta as usize + self.max_state as usize + MAC_LEN
    }
}

/// What a frame carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Stage state for the next stage.
    State,
    /// The request was cancelled; pass it on and stop.
    Cancel,
}

impl Kind {
    fn byte(self) -> u8 {
        match self {
            Self::State => 1,
            Self::Cancel => 2,
        }
    }

    fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            1 => Some(Self::State),
            2 => Some(Self::Cancel),
            _ => None,
        }
    }
}

/// One message from a stage to the next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub kind: Kind,
    pub generation: u64,
    /// At most 255 bytes.
    pub request_id: String,
    pub attempt: u32,
    pub step: u64,
    /// Opaque per-step settings from the head.
    pub meta: Vec<u8>,
    /// The encoded stage-state message.
    pub state: Vec<u8>,
}

impl Frame {
    /// The bytes this frame takes on the wire, MAC and length included.
    #[must_use]
    pub fn wire_len(&self) -> usize {
        FRAME_OVERHEAD + self.request_id.len() + self.meta.len() + self.state.len()
    }
}

/// Why a handshake was refused. The code travels in the refusal message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// The peer speaks another wire version.
    Version,
    /// The hop names another group.
    Group,
    /// The hop names another generation.
    Generation,
    /// The hop names other stages.
    Hop,
    /// The peer didn't prove it holds the key.
    Auth,
    /// The receiving stage already has a link for this hop.
    Busy,
}

impl Refusal {
    fn code(self) -> u8 {
        match self {
            Self::Version => 1,
            Self::Group => 2,
            Self::Generation => 3,
            Self::Hop => 4,
            Self::Auth => 5,
            Self::Busy => 6,
        }
    }

    fn from_code(code: u8) -> Self {
        match code {
            1 => Self::Version,
            2 => Self::Group,
            3 => Self::Generation,
            4 => Self::Hop,
            6 => Self::Busy,
            _ => Self::Auth,
        }
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Version => "another link version",
            Self::Group => "another group",
            Self::Generation => "another group generation",
            Self::Hop => "another pair of stages",
            Self::Auth => "the wrong link key",
            Self::Busy => "a hop that is already linked",
        })
    }
}

/// Why a link failed or refused a frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkError {
    Io(String),
    /// The other end refused the handshake.
    Refused(Refusal),
    /// A handshake message didn't arrive in time.
    Timeout,
    /// The other end closed the link between frames.
    Closed,
    /// The next stage stopped reading and the sender's queue stayed full.
    Backpressure {
        queued: usize,
    },
    /// A frame, or one of its sections, is longer than the limits allow.
    TooLarge {
        len: usize,
        limit: usize,
    },
    /// A frame's MAC doesn't match: altered, or from another session.
    Mac,
    /// A frame arrived out of sequence: replayed, dropped, or reordered.
    Sequence {
        got: u64,
        expected: u64,
    },
    /// A frame names another generation than its link's.
    Generation {
        got: u64,
        expected: u64,
    },
    Malformed(String),
}

impl fmt::Display for LinkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "the link failed: {error}"),
            Self::Refused(why) => write!(f, "the other stage refused the link: {why}"),
            Self::Timeout => write!(f, "the other stage didn't answer the link in time"),
            Self::Closed => write!(f, "the other stage closed the link"),
            Self::Backpressure { queued } => write!(
                f,
                "the next stage isn't keeping up: {queued} frames are waiting"
            ),
            Self::TooLarge { len, limit } => {
                write!(f, "a frame of {len} bytes is over the {limit}-byte limit")
            }
            Self::Mac => write!(f, "a frame failed its authentication check"),
            Self::Sequence { got, expected } => {
                write!(f, "frame {got} arrived where frame {expected} was due")
            }
            Self::Generation { got, expected } => write!(
                f,
                "a frame from generation {got} arrived on a generation {expected} link"
            ),
            Self::Malformed(why) => write!(f, "an unreadable frame: {why}"),
        }
    }
}

impl std::error::Error for LinkError {}

impl From<std::io::Error> for LinkError {
    fn from(error: std::io::Error) -> Self {
        if error.kind() == std::io::ErrorKind::UnexpectedEof {
            Self::Closed
        } else {
            Self::Io(error.to_string())
        }
    }
}

/// HMAC-SHA256 over the concatenation of `parts`.
#[must_use]
pub fn hmac(key: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let mut block = [0u8; 64];
    if key.len() > 64 {
        block[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        block[..key.len()].copy_from_slice(key);
    }
    let mut inner = Sha256::new();
    inner.update(block.map(|b| b ^ 0x36));
    for part in parts {
        inner.update(part);
    }
    let mut outer = Sha256::new();
    outer.update(block.map(|b| b ^ 0x5c));
    outer.update(inner.finalize());
    outer.finalize().into()
}

fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn header(kind: u8) -> Vec<u8> {
    let mut bytes = MAGIC.to_vec();
    bytes.extend([VERSION, kind]);
    bytes
}

fn client_hello(hop: &Hop, nonce: &[u8; NONCE_LEN]) -> Result<Vec<u8>, LinkError> {
    let group = hop.group.as_bytes();
    let n = u8::try_from(group.len()).map_err(|_| LinkError::TooLarge {
        len: group.len(),
        limit: 255,
    })?;
    let mut bytes = header(CLIENT_HELLO);
    bytes.extend(hop.generation.to_be_bytes());
    bytes.extend(hop.from.to_be_bytes());
    bytes.extend(hop.to.to_be_bytes());
    bytes.push(n);
    bytes.extend(group);
    bytes.extend(nonce);
    Ok(bytes)
}

async fn timed<T>(
    limit: Duration,
    work: impl Future<Output = Result<T, LinkError>>,
) -> Result<T, LinkError> {
    tokio::time::timeout(limit, work)
        .await
        .map_err(|_| LinkError::Timeout)?
}

/// Read a handshake header and check its magic, version, and kind.
async fn read_header(stream: &mut TcpStream, want: u8) -> Result<(), LinkError> {
    let mut head = [0u8; 6];
    stream.read_exact(&mut head).await?;
    if head[..4] != MAGIC {
        return Err(LinkError::Malformed("not a stage link".into()));
    }
    if head[4] != VERSION {
        return Err(LinkError::Refused(Refusal::Version));
    }
    if head[5] == REFUSED && want != CLIENT_HELLO {
        let code = stream.read_u8().await?;
        return Err(LinkError::Refused(Refusal::from_code(code)));
    }
    if head[5] != want {
        return Err(LinkError::Malformed(format!(
            "handshake message {} where {want} was due",
            head[5]
        )));
    }
    Ok(())
}

async fn refuse(stream: &mut TcpStream, why: Refusal) {
    let mut bytes = header(REFUSED);
    bytes.push(why.code());
    let _ = stream.write_all(&bytes).await;
    let _ = stream.shutdown().await;
}

/// Open a link to the next stage at `address` for `hop`.
///
/// # Errors
///
/// When the connection fails, the other end refuses or doesn't answer in
/// time, or it can't prove it holds `key`.
pub async fn connect(
    address: impl ToSocketAddrs,
    key: &Key,
    hop: &Hop,
    limits: &Limits,
) -> Result<Sender, LinkError> {
    let wait = limits.handshake_timeout;
    let mut stream = timed(wait, async {
        TcpStream::connect(address).await.map_err(LinkError::from)
    })
    .await?;
    let _ = stream.set_nodelay(true);
    let mut nonce = [0u8; NONCE_LEN];
    random(&mut nonce)?;
    let hello = client_hello(hop, &nonce)?;
    stream.write_all(&hello).await?;
    let (nonce_s, mac_s) = timed(wait, async {
        read_header(&mut stream, SERVER_HELLO).await?;
        let mut nonce_s = [0u8; NONCE_LEN];
        let mut mac = [0u8; MAC_LEN];
        stream.read_exact(&mut nonce_s).await?;
        stream.read_exact(&mut mac).await?;
        Ok((nonce_s, mac))
    })
    .await?;
    if !same(&mac_s, &hmac(&key.0, &[b"oasl/1 server", &hello, &nonce_s])) {
        refuse(&mut stream, Refusal::Auth).await;
        return Err(LinkError::Refused(Refusal::Auth));
    }
    let mut finish = header(CLIENT_FINISH);
    finish.extend(hmac(&key.0, &[b"oasl/1 client", &hello, &nonce_s]));
    stream.write_all(&finish).await?;
    timed(wait, read_header(&mut stream, ACCEPTED)).await?;
    let session = hmac(&key.0, &[b"oasl/1 session", &hello, &nonce_s]);
    Ok(Sender::start(stream, session, hop.clone(), limits.clone()))
}

/// A stage's inbound link listener.
pub struct Listener {
    inner: TcpListener,
}

impl Listener {
    /// Listen on `address`.
    ///
    /// # Errors
    ///
    /// When the address can't be bound.
    pub async fn bind(address: impl ToSocketAddrs) -> std::io::Result<Self> {
        Ok(Self {
            inner: TcpListener::bind(address).await?,
        })
    }

    /// The address it listens on.
    ///
    /// # Errors
    ///
    /// When the socket has no local address.
    pub fn local_addr(&self) -> std::io::Result<std::net::SocketAddr> {
        self.inner.local_addr()
    }

    /// Wait for the next connection and read the hop it claims.
    ///
    /// # Errors
    ///
    /// When accepting fails, or the peer sends no readable hello in time.
    /// A failed peer never stops the listener; call again.
    pub async fn accept(&self, limits: &Limits) -> Result<Offer, LinkError> {
        let (mut stream, _) = self.inner.accept().await?;
        let _ = stream.set_nodelay(true);
        let claimed = timed(limits.handshake_timeout, async {
            let version = {
                let mut head = [0u8; 6];
                stream.read_exact(&mut head).await?;
                if head[..4] != MAGIC {
                    return Err(LinkError::Malformed("not a stage link".into()));
                }
                if head[5] != CLIENT_HELLO {
                    return Err(LinkError::Malformed("not a client hello".into()));
                }
                head
            };
            if version[4] != VERSION {
                return Err(LinkError::Refused(Refusal::Version));
            }
            let generation = stream.read_u64().await?;
            let from = stream.read_u16().await?;
            let to = stream.read_u16().await?;
            let n = stream.read_u8().await?;
            let mut group = vec![0u8; usize::from(n)];
            stream.read_exact(&mut group).await?;
            let mut nonce = [0u8; NONCE_LEN];
            stream.read_exact(&mut nonce).await?;
            let group = String::from_utf8(group)
                .map_err(|_| LinkError::Malformed("a group name that isn't text".into()))?;
            let hop = Hop {
                group,
                generation,
                from,
                to,
            };
            let hello = client_hello(&hop, &nonce)?;
            Ok((hop, hello))
        })
        .await;
        match claimed {
            Ok((hop, hello)) => Ok(Offer { stream, hop, hello }),
            Err(error) => {
                if let LinkError::Refused(why) = error {
                    // Another wire version: say so before closing.
                    refuse(&mut stream, why).await;
                }
                Err(error)
            }
        }
    }
}

/// A connection that has claimed a hop and waits to be accepted or
/// refused.
pub struct Offer {
    stream: TcpStream,
    hop: Hop,
    hello: Vec<u8>,
}

impl Offer {
    /// The hop the other end claims.
    #[must_use]
    pub fn hop(&self) -> &Hop {
        &self.hop
    }

    /// Accept the link when its hop is `expected` and the other end proves
    /// it holds `key`; otherwise refuse it and say why.
    ///
    /// # Errors
    ///
    /// The refusal, or a failure of the handshake.
    pub async fn accept(
        mut self,
        key: &Key,
        expected: &Hop,
        limits: &Limits,
    ) -> Result<Receiver, LinkError> {
        let why = if self.hop.group != expected.group {
            Some(Refusal::Group)
        } else if self.hop.generation != expected.generation {
            Some(Refusal::Generation)
        } else if (self.hop.from, self.hop.to) != (expected.from, expected.to) {
            Some(Refusal::Hop)
        } else {
            None
        };
        if let Some(why) = why {
            refuse(&mut self.stream, why).await;
            return Err(LinkError::Refused(why));
        }
        let mut nonce = [0u8; NONCE_LEN];
        random(&mut nonce)?;
        let mut reply = header(SERVER_HELLO);
        reply.extend(nonce);
        reply.extend(hmac(&key.0, &[b"oasl/1 server", &self.hello, &nonce]));
        self.stream.write_all(&reply).await?;
        let stream = &mut self.stream;
        let mac = timed(limits.handshake_timeout, async {
            read_header(stream, CLIENT_FINISH).await?;
            let mut mac = [0u8; MAC_LEN];
            stream.read_exact(&mut mac).await?;
            Ok(mac)
        })
        .await?;
        if !same(
            &mac,
            &hmac(&key.0, &[b"oasl/1 client", &self.hello, &nonce]),
        ) {
            refuse(&mut self.stream, Refusal::Auth).await;
            return Err(LinkError::Refused(Refusal::Auth));
        }
        self.stream.write_all(&header(ACCEPTED)).await?;
        Ok(Receiver {
            session: hmac(&key.0, &[b"oasl/1 session", &self.hello, &nonce]),
            stream: self.stream,
            hop: self.hop,
            limits: limits.clone(),
            seq: 0,
            bytes: 0,
        })
    }

    /// Refuse the link.
    pub async fn refuse(mut self, why: Refusal) {
        refuse(&mut self.stream, why).await;
    }
}

/// Encode `frame` as sequence `seq` under `session`.
///
/// # Errors
///
/// When a section is over `limits`.
pub fn encode(
    frame: &Frame,
    seq: u64,
    session: &[u8; 32],
    limits: &Limits,
) -> Result<Vec<u8>, LinkError> {
    let request = frame.request_id.as_bytes();
    let n = u8::try_from(request.len()).map_err(|_| LinkError::TooLarge {
        len: request.len(),
        limit: 255,
    })?;
    for (len, limit) in [
        (frame.meta.len(), limits.max_meta),
        (frame.state.len(), limits.max_state),
    ] {
        if len > limit as usize {
            return Err(LinkError::TooLarge {
                len,
                limit: limit as usize,
            });
        }
    }
    let body_len = BODY_FIXED + request.len() + frame.meta.len() + frame.state.len();
    let mut bytes = Vec::with_capacity(4 + body_len + MAC_LEN);
    bytes.extend(
        u32::try_from(body_len + MAC_LEN)
            .unwrap_or(u32::MAX)
            .to_be_bytes(),
    );
    bytes.extend(seq.to_be_bytes());
    bytes.push(frame.kind.byte());
    bytes.extend(frame.generation.to_be_bytes());
    bytes.extend(frame.attempt.to_be_bytes());
    bytes.extend(frame.step.to_be_bytes());
    bytes.push(n);
    bytes.extend(request);
    bytes.extend(
        u32::try_from(frame.meta.len())
            .unwrap_or(u32::MAX)
            .to_be_bytes(),
    );
    bytes.extend(&frame.meta);
    bytes.extend(
        u32::try_from(frame.state.len())
            .unwrap_or(u32::MAX)
            .to_be_bytes(),
    );
    bytes.extend(&frame.state);
    let mac = hmac(session, &[&bytes[4..]]);
    bytes.extend(mac);
    Ok(bytes)
}

/// Read one frame's body (MAC already checked) into a frame.
fn parse(body: &[u8]) -> Result<(u64, Frame), LinkError> {
    struct Cursor<'a>(&'a [u8]);
    impl<'a> Cursor<'a> {
        fn take(&mut self, n: usize) -> Result<&'a [u8], LinkError> {
            if self.0.len() < n {
                return Err(LinkError::Malformed("a frame ends early".into()));
            }
            let (head, rest) = self.0.split_at(n);
            self.0 = rest;
            Ok(head)
        }
        fn u64(&mut self) -> Result<u64, LinkError> {
            Ok(u64::from_be_bytes(
                self.take(8)?.try_into().unwrap_or([0; 8]),
            ))
        }
        fn u32(&mut self) -> Result<u32, LinkError> {
            Ok(u32::from_be_bytes(
                self.take(4)?.try_into().unwrap_or([0; 4]),
            ))
        }
    }
    let mut c = Cursor(body);
    let seq = c.u64()?;
    let kind = c.take(1)?[0];
    let kind = Kind::from_byte(kind)
        .ok_or_else(|| LinkError::Malformed(format!("unknown frame kind {kind}")))?;
    let generation = c.u64()?;
    let attempt = c.u32()?;
    let step = c.u64()?;
    let n = usize::from(c.take(1)?[0]);
    let request_id = String::from_utf8(c.take(n)?.to_vec())
        .map_err(|_| LinkError::Malformed("a request ID that isn't text".into()))?;
    let m = c.u32()? as usize;
    let meta = c.take(m)?.to_vec();
    let s = c.u32()? as usize;
    let state = c.take(s)?.to_vec();
    if !c.0.is_empty() {
        return Err(LinkError::Malformed("bytes after the state section".into()));
    }
    Ok((
        seq,
        Frame {
            kind,
            generation,
            request_id,
            attempt,
            step,
            meta,
            state,
        },
    ))
}

/// The receiving end of a link.
pub struct Receiver {
    stream: TcpStream,
    session: [u8; 32],
    hop: Hop,
    limits: Limits,
    seq: u64,
    bytes: u64,
}

impl Receiver {
    /// The hop this link carries.
    #[must_use]
    pub fn hop(&self) -> &Hop {
        &self.hop
    }

    /// Bytes read so far, frames' length prefixes and MACs included.
    #[must_use]
    pub fn bytes(&self) -> u64 {
        self.bytes
    }

    /// Wait for the next frame.
    ///
    /// # Errors
    ///
    /// [`LinkError::Closed`] when the sender closed the link between
    /// frames; otherwise why the frame was refused. Any error ends the
    /// link: the stream's position is no longer trustworthy.
    pub async fn recv(&mut self) -> Result<Frame, LinkError> {
        let len = match self.stream.read_u32().await {
            Ok(len) => len as usize,
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => {
                return Err(LinkError::Closed);
            }
            Err(error) => return Err(error.into()),
        };
        let limit = self.limits.max_frame();
        if len > limit {
            return Err(LinkError::TooLarge { len, limit });
        }
        if len < BODY_FIXED + MAC_LEN {
            return Err(LinkError::Malformed(
                "a frame shorter than its header".into(),
            ));
        }
        let mut bytes = vec![0u8; len];
        self.stream.read_exact(&mut bytes).await?;
        let (body, mac) = bytes.split_at(len - MAC_LEN);
        if !same(mac, &hmac(&self.session, &[body])) {
            return Err(LinkError::Mac);
        }
        let (seq, frame) = parse(body)?;
        if seq != self.seq {
            return Err(LinkError::Sequence {
                got: seq,
                expected: self.seq,
            });
        }
        if frame.generation != self.hop.generation {
            return Err(LinkError::Generation {
                got: frame.generation,
                expected: self.hop.generation,
            });
        }
        if frame.meta.len() > self.limits.max_meta as usize
            || frame.state.len() > self.limits.max_state as usize
        {
            return Err(LinkError::TooLarge {
                len: frame.meta.len().max(frame.state.len()),
                limit: self.limits.max_state.max(self.limits.max_meta) as usize,
            });
        }
        self.seq += 1;
        self.bytes += 4 + len as u64;
        Ok(frame)
    }
}

#[derive(Default)]
struct Shared {
    failed: Mutex<Option<LinkError>>,
    queued: AtomicUsize,
    peak: AtomicUsize,
    bytes: AtomicU64,
    frames: AtomicU64,
}

impl Shared {
    fn fail(&self, error: LinkError) {
        let mut failed = self.failed.lock().unwrap_or_else(|e| e.into_inner());
        failed.get_or_insert(error);
    }

    fn failed(&self) -> Option<LinkError> {
        self.failed
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}

/// The sending end of a link, with a bounded queue in front of the socket.
pub struct Sender {
    queue: mpsc::UnboundedSender<Vec<u8>>,
    /// One permit per frame the queue may hold, the one being written
    /// included.
    room: Arc<Semaphore>,
    shared: Arc<Shared>,
    session: [u8; 32],
    hop: Hop,
    limits: Limits,
    seq: u64,
    writer: Option<tokio::task::JoinHandle<()>>,
}

impl Sender {
    fn start(stream: TcpStream, session: [u8; 32], hop: Hop, limits: Limits) -> Self {
        let (queue, mut frames) = mpsc::unbounded_channel::<Vec<u8>>();
        let room = Arc::new(Semaphore::new(limits.queue.max(1)));
        let freed = room.clone();
        let shared = Arc::new(Shared::default());
        let state = shared.clone();
        let timeout = limits.send_timeout;
        let writer = tokio::spawn(async move {
            let mut stream = stream;
            while let Some(bytes) = frames.recv().await {
                let wrote = tokio::time::timeout(timeout, stream.write_all(&bytes)).await;
                state.queued.fetch_sub(1, Ordering::SeqCst);
                freed.add_permits(1);
                match wrote {
                    Ok(Ok(())) => {
                        state.bytes.fetch_add(bytes.len() as u64, Ordering::SeqCst);
                        state.frames.fetch_add(1, Ordering::SeqCst);
                    }
                    Ok(Err(error)) => return state.fail(error.into()),
                    Err(_) => {
                        return state.fail(LinkError::Backpressure {
                            queued: state.queued.load(Ordering::SeqCst) + 1,
                        });
                    }
                }
            }
            let _ = stream.shutdown().await;
        });
        Self {
            queue,
            room,
            shared,
            session,
            hop,
            limits,
            seq: 0,
            writer: Some(writer),
        }
    }

    /// The hop this link carries.
    #[must_use]
    pub fn hop(&self) -> &Hop {
        &self.hop
    }

    /// Frames waiting to be written, the one being written included.
    #[must_use]
    pub fn queued(&self) -> usize {
        self.shared.queued.load(Ordering::SeqCst)
    }

    /// The most frames that ever waited at once; never more than
    /// [`Limits::queue`].
    #[must_use]
    pub fn peak(&self) -> usize {
        self.shared.peak.load(Ordering::SeqCst)
    }

    /// Bytes written so far, length prefixes and MACs included.
    #[must_use]
    pub fn bytes(&self) -> u64 {
        self.shared.bytes.load(Ordering::SeqCst)
    }

    /// Frames written so far.
    #[must_use]
    pub fn frames(&self) -> u64 {
        self.shared.frames.load(Ordering::SeqCst)
    }

    /// Why the link failed, once it has.
    #[must_use]
    pub fn failed(&self) -> Option<LinkError> {
        self.shared.failed()
    }

    /// Queue `frame` for the next stage and return its wire size. Waits up
    /// to [`Limits::send_timeout`] while the queue is full.
    ///
    /// # Errors
    ///
    /// When the frame is over the limits or names another generation, when
    /// the queue stays full ([`LinkError::Backpressure`]), or when the link
    /// already failed. After `Backpressure` the link is failed too: the
    /// group must not wait on a stage that stopped reading.
    pub async fn send(&mut self, frame: &Frame) -> Result<usize, LinkError> {
        if let Some(error) = self.shared.failed() {
            return Err(error);
        }
        if frame.generation != self.hop.generation {
            return Err(LinkError::Generation {
                got: frame.generation,
                expected: self.hop.generation,
            });
        }
        let bytes = encode(frame, self.seq, &self.session, &self.limits)?;
        let len = bytes.len();
        match tokio::time::timeout(self.limits.send_timeout, self.room.acquire()).await {
            Ok(Ok(permit)) => permit.forget(),
            Ok(Err(_)) => {
                return Err(self.shared.failed().unwrap_or(LinkError::Closed));
            }
            Err(_) => {
                let error = LinkError::Backpressure {
                    queued: self.queued(),
                };
                self.shared.fail(error.clone());
                return Err(error);
            }
        }
        let queued = self.shared.queued.fetch_add(1, Ordering::SeqCst) + 1;
        self.shared.peak.fetch_max(queued, Ordering::SeqCst);
        if self.queue.send(bytes).is_err() {
            return Err(self.shared.failed().unwrap_or(LinkError::Closed));
        }
        self.seq += 1;
        Ok(len)
    }

    /// Write every queued frame, then close the link.
    ///
    /// # Errors
    ///
    /// When the link failed before every frame was written.
    pub async fn close(mut self) -> Result<(), LinkError> {
        let (queue, _) = mpsc::unbounded_channel();
        drop(std::mem::replace(&mut self.queue, queue));
        if let Some(writer) = self.writer.take() {
            let _ = writer.await;
        }
        match self.shared.failed() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

impl Drop for Sender {
    /// Dropping a sender abandons its queued frames and closes the
    /// connection at once, as a fence requires; [`Sender::close`] writes
    /// them first.
    fn drop(&mut self) {
        if let Some(writer) = self.writer.take() {
            writer.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hmac_matches_rfc_4231() {
        // Test cases 1 and 2.
        let mac = hmac(&[0x0b; 20], &[b"Hi There"]);
        assert_eq!(
            hex(&mac),
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
        let mac = hmac(b"Jefe", &[b"what do ya want ", b"for nothing?"]);
        assert_eq!(
            hex(&mac),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
        // Test case 6: a key longer than the block.
        let mac = hmac(
            &[0xaa; 131],
            &[b"Test Using Larger Than Block-Size Key - Hash Key First"],
        );
        assert_eq!(
            hex(&mac),
            "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54"
        );
    }

    #[test]
    fn keys_read_and_write_hex_and_hide_in_debug() {
        let key = Key::generate().unwrap();
        assert_eq!(Key::from_hex(&key.to_hex()).unwrap(), key);
        assert_eq!(format!("{key:?}"), "Key(..)");
        assert!(Key::from_hex("00").is_err());
        assert!(Key::from_hex(&"g".repeat(64)).is_err());
    }

    #[test]
    fn frames_encode_and_parse_with_their_wire_size() {
        let frame = Frame {
            kind: Kind::State,
            generation: 3,
            request_id: "17".into(),
            attempt: 1,
            step: 9,
            meta: b"{}".to_vec(),
            state: vec![7; 1000],
        };
        let session = [5; 32];
        let bytes = encode(&frame, 4, &session, &Limits::default()).unwrap();
        assert_eq!(bytes.len(), frame.wire_len());
        let body = &bytes[4..bytes.len() - MAC_LEN];
        assert_eq!(&bytes[bytes.len() - MAC_LEN..], hmac(&session, &[body]));
        assert_eq!(parse(body).unwrap(), (4, frame));
    }
}
