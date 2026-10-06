//! The direct channel: a mutually authenticated, encrypted, sequenced byte
//! stream between one enrolled device and one host.
//!
//! The client proves its device key, the host proves its host key, both bind
//! fresh nonces and ephemeral keys, and the transcript binds the device's
//! grant ID, grant epoch, and the host generation. Either side refuses a
//! mismatch. Relay-carried control remains the fallback when no direct route
//! works. The handshake runs over any ordered byte stream; this crate tests it
//! over TCP and over WebSocket through [`crate::websocket::WebSocket`].
//!
//! Wire frame: `u32` big-endian length of the rest, a `u8` frame kind, a `u64`
//! big-endian sequence number, and the body. Each direction numbers its frames
//! from zero. A frame longer than [`MAX_FRAME_BYTES`] refuses before any body
//! bytes are read.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use base64::{Engine, engine::general_purpose::STANDARD};
use nostr::{contracts, nip44};
use secp256k1::{Keypair, Secp256k1, SecretKey, XOnlyPublicKey, schnorr};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::json;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::{
    Error, Refusal, Result, fail, hex, new_id, parse_pubkey, pubkey, random_bytes, random_id,
    requires, unhex32,
};

/// Largest frame, counting the kind, sequence number, and body.
pub const MAX_FRAME_BYTES: usize = 65_536;
/// Largest application payload in one data frame.
pub const MAX_DATA_BYTES: usize = 16_384;
/// Largest plaintext handshake message.
pub const MAX_HANDSHAKE_BYTES: usize = 4_096;
/// Frame header after the length prefix: kind and sequence number.
pub const HEADER_BYTES: usize = 9;
/// Largest distance between the client's hello time and the host clock.
pub const MAX_CLOCK_SKEW: u64 = 120;
/// Most client nonces the host remembers.
pub const REPLAY_CAPACITY: usize = 4_096;
/// Detail of a refusal the host sent before proving its key. Treat its code
/// as advisory: anyone on the path could have sent it.
pub const UNAUTHENTICATED: &str = "host refused before proving its key";
/// Detail of the client's refusal of a host that proved its key at another
/// generation than the presence the client read: the host restarted and
/// the presence is not yet its new one.
pub const GENERATION_DIFFERS: &str = "host generation differs from presence";

const HELLO: &str = "openagents.reach-hello.v1";
const HOST_PROOF: &str = "openagents.reach-host-proof.v1";
const CLIENT_PROOF: &str = "openagents.reach-client-proof.v1";
const REFUSAL: &str = "openagents.reach-refusal.v1";
const VERDICT: &str = "openagents.reach-verdict.v1";
const TRANSCRIPT_LABEL: &[u8] = b"openagents.reach-channel.v1\0";
const HOST_SIGN_LABEL: &[u8] = b"openagents.reach-host-proof.v1\0";
const CLIENT_SIGN_LABEL: &[u8] = b"openagents.reach-client-proof.v1\0";
const C2H_LABEL: &[u8] = b"openagents.reach-c2h.v1\0";
const H2C_LABEL: &[u8] = b"openagents.reach-h2c.v1\0";

/// Frame kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum FrameKind {
    ClientHello = 1,
    HostProof = 2,
    ClientProof = 3,
    Verdict = 4,
    Refusal = 5,
    Data = 16,
    Close = 17,
}

impl FrameKind {
    fn parse(value: u8) -> Result<Self> {
        Ok(match value {
            1 => Self::ClientHello,
            2 => Self::HostProof,
            3 => Self::ClientProof,
            4 => Self::Verdict,
            5 => Self::Refusal,
            16 => Self::Data,
            17 => Self::Close,
            _ => return fail(Refusal::Malformed, "unknown frame kind"),
        })
    }
}

/// One decoded frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub kind: FrameKind,
    pub seq: u64,
    pub body: Vec<u8>,
}

/// Write one frame.
///
/// # Errors
/// Refuses frames over [`MAX_FRAME_BYTES`] and reports write failures as
/// `unavailable`.
pub async fn write_frame<W: AsyncWrite + Unpin>(
    writer: &mut W,
    kind: FrameKind,
    seq: u64,
    body: &[u8],
) -> Result<()> {
    let len = HEADER_BYTES + body.len();
    if len > MAX_FRAME_BYTES {
        return fail(Refusal::LimitExceeded, "frame exceeds the maximum size");
    }
    let mut bytes = Vec::with_capacity(4 + len);
    bytes.extend_from_slice(&u32::try_from(len).unwrap_or(u32::MAX).to_be_bytes());
    bytes.push(kind as u8);
    bytes.extend_from_slice(&seq.to_be_bytes());
    bytes.extend_from_slice(body);
    writer
        .write_all(&bytes)
        .await
        .map_err(|e| io_refusal(&e, "write failed"))?;
    writer
        .flush()
        .await
        .map_err(|e| io_refusal(&e, "flush failed"))
}

/// Read one frame, checking its length before reading the body.
///
/// # Errors
/// Refuses oversized, undersized, or unknown frames; reports read failures
/// and end of stream as `unavailable`.
pub async fn read_frame<R: AsyncRead + Unpin>(reader: &mut R) -> Result<Frame> {
    let mut prefix = [0; 4];
    reader
        .read_exact(&mut prefix)
        .await
        .map_err(|e| io_refusal(&e, "stream closed"))?;
    let len = u32::from_be_bytes(prefix) as usize;
    if len > MAX_FRAME_BYTES {
        return fail(Refusal::LimitExceeded, "frame exceeds the maximum size");
    }
    if len < HEADER_BYTES {
        return fail(Refusal::Malformed, "frame shorter than its header");
    }
    let mut bytes = vec![0; len];
    reader
        .read_exact(&mut bytes)
        .await
        .map_err(|e| io_refusal(&e, "stream closed"))?;
    let kind = FrameKind::parse(bytes[0])?;
    let mut seq = [0; 8];
    seq.copy_from_slice(&bytes[1..HEADER_BYTES]);
    Ok(Frame {
        kind,
        seq: u64::from_be_bytes(seq),
        body: bytes.split_off(HEADER_BYTES),
    })
}

/// The refusal a transport attached to an I/O error, such as a WebSocket
/// message that breaks the mapping, or `unavailable` with `detail`.
fn io_refusal(error: &std::io::Error, detail: &'static str) -> Error {
    error
        .get_ref()
        .and_then(|inner| inner.downcast_ref::<Error>())
        .cloned()
        .unwrap_or(Error::new(Refusal::Unavailable, detail))
}

/// The client's opening message.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClientHello {
    pub v: String,
    pub requires: Vec<String>,
    /// The device key.
    pub client: String,
    /// The host key the client expects.
    pub host: String,
    /// The device's current grant ID on this host.
    pub grant: String,
    /// The grant's current revocation epoch.
    pub epoch: u64,
    /// The host generation the client expects, from fresh presence.
    pub generation: u64,
    pub nonce: String,
    /// The client's ephemeral x-only key for this channel.
    pub ephemeral: String,
    pub issued_at: u64,
}

/// The host's signed answer to a hello.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostProof {
    pub v: String,
    pub requires: Vec<String>,
    pub nonce: String,
    pub ephemeral: String,
    /// The host's actual generation.
    pub generation: u64,
    /// BIP-340 signature by the host key over the host proof message.
    pub signature: String,
}

/// The client's signature over the same transcript.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClientProof {
    pub v: String,
    pub requires: Vec<String>,
    pub signature: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RefusalBody {
    v: String,
    code: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Verdict {
    v: String,
    code: Option<String>,
}

impl ClientHello {
    fn validate(&self) -> Result<()> {
        if self.v != HELLO {
            return fail(Refusal::UnsupportedVersion, "hello version");
        }
        requires(&self.requires)?;
        parse_pubkey(&self.client)?;
        parse_pubkey(&self.host)?;
        parse_pubkey(&self.ephemeral)?;
        random_id(&self.grant)?;
        random_id(&self.nonce)?;
        if self.client == self.host {
            return fail(Refusal::Malformed, "client and host keys must differ");
        }
        Ok(())
    }
}

/// The channel transcript digest both signatures cover. It binds both keys,
/// both nonces, both ephemeral keys, the grant ID and epoch, the generation
/// the client expected, the host's actual generation, and the frame bound.
///
/// # Errors
/// Refuses values that cannot be canonicalized.
pub fn transcript(hello: &ClientHello, proof: &HostProof) -> Result<[u8; 32]> {
    let value = json!({
        "client": hello.client,
        "host": hello.host,
        "grant": hello.grant,
        "epoch": hello.epoch,
        "expected_generation": hello.generation,
        "client_nonce": hello.nonce,
        "client_ephemeral": hello.ephemeral,
        "issued_at": hello.issued_at,
        "host_nonce": proof.nonce,
        "host_ephemeral": proof.ephemeral,
        "generation": proof.generation,
        "max_frame": MAX_FRAME_BYTES,
    });
    let bytes = contracts::jcs(&value).map_err(|_| Error::new(Refusal::Malformed, "transcript"))?;
    Ok(labeled(TRANSCRIPT_LABEL, &[&bytes]))
}

fn labeled(label: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(label);
    for part in parts {
        hash.update(part);
    }
    hash.finalize().into()
}

/// Sign the role-specific proof message for a transcript.
fn sign(label: &[u8], transcript: &[u8; 32], secret: &SecretKey) -> String {
    let message = labeled(label, &[transcript]);
    let keypair = Keypair::from_secret_key(&Secp256k1::new(), secret);
    Secp256k1::signing_only()
        .sign_schnorr_no_aux_rand(&message, &keypair)
        .to_string()
}

fn verify(label: &[u8], transcript: &[u8; 32], signature: &str, key: &str) -> Result<()> {
    let bad = || Error::new(Refusal::IdentityMismatch, "proof signature");
    if signature.len() != 128 {
        return Err(bad());
    }
    let mut bytes = [0; 64];
    let (a, b) = signature.split_at(64);
    bytes[..32].copy_from_slice(&unhex32(a).map_err(|_| bad())?);
    bytes[32..].copy_from_slice(&unhex32(b).map_err(|_| bad())?);
    let signature = schnorr::Signature::from_byte_array(bytes);
    let key = parse_pubkey(key)?;
    let message = labeled(label, &[transcript]);
    Secp256k1::verification_only()
        .verify_schnorr(&signature, &message, &key)
        .map_err(|_| bad())
}

fn ephemeral() -> SecretKey {
    loop {
        if let Ok(secret) = SecretKey::from_byte_array(random_bytes()) {
            return secret;
        }
    }
}

fn session_keys(
    ephemeral: &SecretKey,
    peer_ephemeral: &str,
    transcript: &[u8; 32],
) -> Result<([u8; 32], [u8; 32])> {
    let peer: XOnlyPublicKey = parse_pubkey(peer_ephemeral)?;
    let shared = nip44::conversation_key(ephemeral, &peer);
    Ok((
        labeled(C2H_LABEL, &[&shared, transcript]),
        labeled(H2C_LABEL, &[&shared, transcript]),
    ))
}

fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    let value =
        serde_json::to_value(value).map_err(|_| Error::new(Refusal::Malformed, "message"))?;
    contracts::jcs(&value).map_err(|_| Error::new(Refusal::Malformed, "message"))
}

fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    let value = contracts::parse_strict_bounded(bytes, MAX_HANDSHAKE_BYTES)
        .map_err(|_| Error::new(Refusal::Malformed, "handshake JSON"))?;
    serde_json::from_value(value).map_err(|_| Error::new(Refusal::Malformed, "handshake fields"))
}

pub(crate) fn seal_frame(
    key: &[u8; 32],
    kind: FrameKind,
    seq: u64,
    data: &[u8],
) -> Result<Vec<u8>> {
    let mut plain = Vec::with_capacity(HEADER_BYTES + data.len());
    plain.push(kind as u8);
    plain.extend_from_slice(&seq.to_be_bytes());
    plain.extend_from_slice(data);
    nip44::encrypt(&STANDARD.encode(plain), key, random_bytes())
        .map(String::into_bytes)
        .map_err(|_| Error::new(Refusal::LimitExceeded, "frame encryption"))
}

pub(crate) fn open_frame(key: &[u8; 32], frame: &Frame) -> Result<Vec<u8>> {
    let bad = || Error::new(Refusal::IdentityMismatch, "frame authentication");
    let text = std::str::from_utf8(&frame.body).map_err(|_| bad())?;
    let plain = nip44::decrypt(text, key).map_err(|_| bad())?;
    let plain = STANDARD.decode(plain).map_err(|_| bad())?;
    if plain.len() < HEADER_BYTES
        || plain[0] != frame.kind as u8
        || plain[1..HEADER_BYTES] != frame.seq.to_be_bytes()
    {
        return Err(bad());
    }
    Ok(plain[HEADER_BYTES..].to_vec())
}

/// Why a host refuses a device's grant. The host's grant store answers it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GrantRefusal {
    /// No grant with this ID for this device.
    Unknown,
    /// The grant is revoked.
    Revoked,
    /// The grant exists but its current epoch differs.
    EpochMismatch,
    /// The grant expired.
    Expired,
}

impl GrantRefusal {
    const fn code(self) -> Refusal {
        match self {
            Self::Unknown => Refusal::NotAdmitted,
            Self::Revoked => Refusal::Revoked,
            Self::EpochMismatch | Self::Expired => Refusal::Stale,
        }
    }
}

/// The host's grant store, seen through the one question a channel asks.
/// Opening a channel grants nothing; each operation checks its own right.
pub trait GrantCheck: Send + Sync {
    /// Whether `grant` at `epoch` is `device`'s current, unrevoked grant.
    ///
    /// # Errors
    /// Returns the reason the grant cannot open a channel.
    fn check(
        &self,
        device: &str,
        grant: &str,
        epoch: u64,
        now: u64,
    ) -> std::result::Result<(), GrantRefusal>;
}

/// What an open channel is bound to. The host rechecks the grant and
/// generation before each operation and closes the channel when either
/// changes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binding {
    pub client: String,
    pub host: String,
    pub grant: String,
    pub epoch: u64,
    pub generation: u64,
    pub transcript: [u8; 32],
}

/// An open direct channel.
pub struct Channel<S> {
    stream: S,
    send_key: [u8; 32],
    recv_key: [u8; 32],
    send_seq: u64,
    recv_seq: u64,
    binding: Binding,
    closed: bool,
}

impl<S> std::fmt::Debug for Channel<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Keys stay out of debug output.
        f.debug_struct("Channel")
            .field("binding", &self.binding)
            .field("send_seq", &self.send_seq)
            .field("recv_seq", &self.recv_seq)
            .field("closed", &self.closed)
            .finish_non_exhaustive()
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin> Channel<S> {
    /// The identities, grant, and generation this channel is bound to.
    #[must_use]
    pub fn binding(&self) -> &Binding {
        &self.binding
    }

    /// Send one data frame.
    ///
    /// # Errors
    /// Refuses payloads over [`MAX_DATA_BYTES`] or a closed channel.
    pub async fn send(&mut self, data: &[u8]) -> Result<()> {
        if self.closed {
            return fail(Refusal::Unavailable, "channel closed");
        }
        if data.len() > MAX_DATA_BYTES {
            return fail(
                Refusal::LimitExceeded,
                "data exceeds the frame payload bound",
            );
        }
        self.write(FrameKind::Data, data).await
    }

    /// Receive the next data frame; `None` after the peer closes.
    ///
    /// # Errors
    /// Refuses out-of-order, unauthenticated, oversized, or unexpected frames
    /// and closes the channel.
    pub async fn recv(&mut self) -> Result<Option<Vec<u8>>> {
        if self.closed {
            return Ok(None);
        }
        let result = self.read().await;
        match result {
            Ok((FrameKind::Data, data)) => Ok(Some(data)),
            Ok((FrameKind::Close, _)) => {
                self.closed = true;
                Ok(None)
            }
            Ok(_) => {
                self.closed = true;
                fail(Refusal::Malformed, "unexpected frame kind")
            }
            Err(error) => {
                self.closed = true;
                Err(error)
            }
        }
    }

    /// Split the channel into a reader and a writer, so one task can wait
    /// for the peer's frames while another sends. Each half keeps its own
    /// direction's key and sequence number.
    #[must_use]
    pub fn into_split(
        self,
    ) -> (
        crate::split::ChannelReader<tokio::io::ReadHalf<S>>,
        crate::split::ChannelWriter<tokio::io::WriteHalf<S>>,
    ) {
        let (read, write) = tokio::io::split(self.stream);
        (
            crate::split::ChannelReader::new(
                read,
                self.recv_key,
                self.recv_seq,
                self.binding,
                self.closed,
            ),
            crate::split::ChannelWriter::new(write, self.send_key, self.send_seq, self.closed),
        )
    }

    /// Send a close frame and stop.
    ///
    /// # Errors
    /// Reports a failed write.
    pub async fn close(mut self) -> Result<()> {
        if self.closed {
            return Ok(());
        }
        self.closed = true;
        self.write(FrameKind::Close, &[]).await?;
        self.stream
            .shutdown()
            .await
            .map_err(|_| Error::new(Refusal::Unavailable, "shutdown failed"))
    }

    async fn write(&mut self, kind: FrameKind, data: &[u8]) -> Result<()> {
        let body = seal_frame(&self.send_key, kind, self.send_seq, data)?;
        write_frame(&mut self.stream, kind, self.send_seq, &body).await?;
        self.send_seq += 1;
        Ok(())
    }

    async fn read(&mut self) -> Result<(FrameKind, Vec<u8>)> {
        let frame = read_frame(&mut self.stream).await?;
        if frame.seq != self.recv_seq {
            return fail(Refusal::Malformed, "frame out of sequence");
        }
        let data = open_frame(&self.recv_key, &frame)?;
        self.recv_seq += 1;
        Ok((frame.kind, data))
    }
}

/// A device's side of the handshake.
#[derive(Clone)]
pub struct ClientConfig {
    /// The device key.
    pub device: SecretKey,
    /// The host key the client expects, from the owner directory.
    pub host: String,
    /// The device's current grant ID and epoch.
    pub grant: String,
    pub epoch: u64,
    /// The host generation from fresh presence.
    pub generation: u64,
    pub timeout: Duration,
}

/// Open a direct channel as the client.
///
/// # Errors
/// Refuses a host that proves another key or generation, a host refusal, a
/// timeout, or any malformed frame. A refusal carrying [`UNAUTHENTICATED`] came
/// before the host proved its key.
pub async fn connect<S: AsyncRead + AsyncWrite + Unpin>(
    stream: S,
    config: &ClientConfig,
    now: u64,
) -> Result<Channel<S>> {
    #[cfg(not(target_arch = "wasm32"))]
    return tokio::time::timeout(config.timeout, client_handshake(stream, config, now))
        .await
        .map_err(|_| Error::new(Refusal::Unavailable, "handshake timed out"))?;
    #[cfg(target_arch = "wasm32")]
    {
        use futures_util::future::{Either, select};
        let millis = config.timeout.as_millis().min(u32::MAX as u128).max(1) as u32;
        match select(
            Box::pin(client_handshake(stream, config, now)),
            Box::pin(gloo_timers::future::TimeoutFuture::new(millis)),
        )
        .await
        {
            Either::Left((result, _)) => result,
            Either::Right(_) => Err(Error::new(Refusal::Unavailable, "handshake timed out")),
        }
    }
}

async fn client_handshake<S: AsyncRead + AsyncWrite + Unpin>(
    mut stream: S,
    config: &ClientConfig,
    now: u64,
) -> Result<Channel<S>> {
    parse_pubkey(&config.host)?;
    random_id(&config.grant)?;
    let eph = ephemeral();
    let hello = ClientHello {
        v: HELLO.into(),
        requires: vec![],
        client: pubkey(&config.device),
        host: config.host.clone(),
        grant: config.grant.clone(),
        epoch: config.epoch,
        generation: config.generation,
        nonce: new_id(),
        ephemeral: pubkey(&eph),
        issued_at: now,
    };
    hello.validate()?;
    write_frame(&mut stream, FrameKind::ClientHello, 0, &encode(&hello)?).await?;

    let frame = read_frame(&mut stream).await?;
    if frame.seq != 0 {
        return fail(Refusal::Malformed, "frame out of sequence");
    }
    let proof: HostProof = match frame.kind {
        FrameKind::HostProof => decode(&frame.body)?,
        FrameKind::Refusal => {
            let body: RefusalBody = decode(&frame.body)?;
            let code = Refusal::parse(&body.code).unwrap_or(Refusal::Malformed);
            return Err(Error::new(code, UNAUTHENTICATED));
        }
        _ => return fail(Refusal::Malformed, "expected a host proof"),
    };
    if proof.v != HOST_PROOF {
        return fail(Refusal::UnsupportedVersion, "host proof version");
    }
    requires(&proof.requires)?;
    random_id(&proof.nonce)?;
    let digest = transcript(&hello, &proof)?;
    verify(HOST_SIGN_LABEL, &digest, &proof.signature, &config.host)?;
    if proof.generation != config.generation {
        return fail(Refusal::Stale, GENERATION_DIFFERS);
    }
    let (c2h, h2c) = session_keys(&eph, &proof.ephemeral, &digest)?;
    let client_proof = ClientProof {
        v: CLIENT_PROOF.into(),
        requires: vec![],
        signature: sign(CLIENT_SIGN_LABEL, &digest, &config.device),
    };
    write_frame(
        &mut stream,
        FrameKind::ClientProof,
        1,
        &encode(&client_proof)?,
    )
    .await?;

    let frame = read_frame(&mut stream).await?;
    if frame.kind != FrameKind::Verdict || frame.seq != 1 {
        return fail(Refusal::Malformed, "expected a verdict");
    }
    let verdict: Verdict = decode(&open_frame(&h2c, &frame)?)?;
    if verdict.v != VERDICT {
        return fail(Refusal::UnsupportedVersion, "verdict version");
    }
    if let Some(code) = verdict.code {
        let code = Refusal::parse(&code).unwrap_or(Refusal::Malformed);
        return Err(Error::new(code, "host refused the channel"));
    }
    Ok(Channel {
        stream,
        send_key: c2h,
        recv_key: h2c,
        send_seq: 2,
        recv_seq: 2,
        binding: Binding {
            client: hello.client,
            host: hello.host,
            grant: hello.grant,
            epoch: hello.epoch,
            generation: proof.generation,
            transcript: digest,
        },
        closed: false,
    })
}

/// A host's side of the handshake, shared across connections so it can
/// refuse replayed nonces.
pub struct Acceptor<G> {
    host: SecretKey,
    host_key: String,
    generation: u64,
    grants: G,
    timeout: Duration,
    seen: Mutex<HashMap<String, u64>>,
}

impl<G: GrantCheck> Acceptor<G> {
    #[must_use]
    pub fn new(host: SecretKey, generation: u64, grants: G, timeout: Duration) -> Self {
        Self {
            host_key: pubkey(&host),
            host,
            generation,
            grants,
            timeout,
            seen: Mutex::new(HashMap::new()),
        }
    }

    /// The host key this acceptor proves.
    #[must_use]
    pub fn host_key(&self) -> &str {
        &self.host_key
    }

    /// Accept one direct channel.
    ///
    /// # Errors
    /// Refuses a hello for another host, a stale or replayed hello, a bad
    /// client proof, a generation mismatch, or a grant the store refuses.
    pub async fn accept<S: AsyncRead + AsyncWrite + Unpin>(
        &self,
        stream: S,
        now: u64,
    ) -> Result<Channel<S>> {
        tokio::time::timeout(self.timeout, self.handshake(stream, now))
            .await
            .map_err(|_| Error::new(Refusal::Unavailable, "handshake timed out"))?
    }

    /// Decide the verdict after the client proved its key: the generation
    /// must match and the grant store must admit the grant at its epoch.
    ///
    /// # Errors
    /// Returns the refusal the host sends in its verdict.
    pub fn admit(&self, hello: &ClientHello, now: u64) -> Result<()> {
        if hello.generation != self.generation {
            return fail(Refusal::Stale, "host generation differs");
        }
        self.grants
            .check(&hello.client, &hello.grant, hello.epoch, now)
            .map_err(|refusal| Error::new(refusal.code(), "grant refused"))
    }

    fn remember(&self, nonce: &str, now: u64) -> Result<()> {
        let mut seen = self
            .seen
            .lock()
            .map_err(|_| Error::new(Refusal::Unavailable, "replay cache"))?;
        seen.retain(|_, at| at.saturating_add(2 * MAX_CLOCK_SKEW) >= now);
        if seen.contains_key(nonce) {
            return fail(Refusal::Replayed, "handshake nonce already used");
        }
        if seen.len() >= REPLAY_CAPACITY {
            return fail(Refusal::LimitExceeded, "too many recent handshakes");
        }
        seen.insert(nonce.to_owned(), now);
        Ok(())
    }

    async fn refuse<S: AsyncWrite + Unpin, T>(&self, stream: &mut S, error: Error) -> Result<T> {
        let body = encode(&RefusalBody {
            v: REFUSAL.into(),
            code: error.code.as_str().into(),
        })?;
        let _ = write_frame(stream, FrameKind::Refusal, 0, &body).await;
        Err(error)
    }

    async fn handshake<S: AsyncRead + AsyncWrite + Unpin>(
        &self,
        mut stream: S,
        now: u64,
    ) -> Result<Channel<S>> {
        let frame = read_frame(&mut stream).await?;
        if frame.kind != FrameKind::ClientHello || frame.seq != 0 {
            return self
                .refuse(
                    &mut stream,
                    Error::new(Refusal::Malformed, "expected a hello"),
                )
                .await;
        }
        if frame.body.len() > MAX_HANDSHAKE_BYTES {
            return self
                .refuse(
                    &mut stream,
                    Error::new(Refusal::LimitExceeded, "hello too large"),
                )
                .await;
        }
        let hello: ClientHello = match decode(&frame.body).and_then(|h: ClientHello| {
            h.validate()?;
            Ok(h)
        }) {
            Ok(hello) => hello,
            Err(error) => return self.refuse(&mut stream, error).await,
        };
        if hello.host != self.host_key {
            return self
                .refuse(
                    &mut stream,
                    Error::new(Refusal::IdentityMismatch, "hello names another host"),
                )
                .await;
        }
        if hello.issued_at.abs_diff(now) > MAX_CLOCK_SKEW {
            return self
                .refuse(
                    &mut stream,
                    Error::new(Refusal::Stale, "hello outside the clock window"),
                )
                .await;
        }
        if let Err(error) = self.remember(&hello.nonce, now) {
            return self.refuse(&mut stream, error).await;
        }

        let eph = ephemeral();
        let mut proof = HostProof {
            v: HOST_PROOF.into(),
            requires: vec![],
            nonce: new_id(),
            ephemeral: pubkey(&eph),
            generation: self.generation,
            signature: String::new(),
        };
        let digest = transcript(&hello, &proof)?;
        proof.signature = sign(HOST_SIGN_LABEL, &digest, &self.host);
        write_frame(&mut stream, FrameKind::HostProof, 0, &encode(&proof)?).await?;

        let frame = read_frame(&mut stream).await?;
        if frame.kind != FrameKind::ClientProof || frame.seq != 1 {
            return fail(Refusal::Malformed, "expected a client proof");
        }
        let client_proof: ClientProof = decode(&frame.body)?;
        if client_proof.v != CLIENT_PROOF {
            return fail(Refusal::UnsupportedVersion, "client proof version");
        }
        requires(&client_proof.requires)?;
        verify(
            CLIENT_SIGN_LABEL,
            &digest,
            &client_proof.signature,
            &hello.client,
        )?;

        let (c2h, h2c) = session_keys(&eph, &hello.ephemeral, &digest)?;
        let decision = self.admit(&hello, now);
        let verdict = Verdict {
            v: VERDICT.into(),
            code: decision.as_ref().err().map(|e| e.code.as_str().to_owned()),
        };
        let body = seal_frame(&h2c, FrameKind::Verdict, 1, &encode(&verdict)?)?;
        write_frame(&mut stream, FrameKind::Verdict, 1, &body).await?;
        decision?;
        Ok(Channel {
            stream,
            send_key: h2c,
            recv_key: c2h,
            send_seq: 2,
            recv_seq: 2,
            binding: Binding {
                client: hello.client,
                host: hello.host,
                grant: hello.grant,
                epoch: hello.epoch,
                generation: self.generation,
                transcript: digest,
            },
            closed: false,
        })
    }
}

/// Hex form of a transcript digest, for logs that must not carry content.
#[must_use]
pub fn transcript_id(binding: &Binding) -> String {
    hex(&binding.transcript)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn frame_reader_refuses_oversized_and_short_frames() {
        let (mut a, mut b) = tokio::io::duplex(1024);
        a.write_all(&(MAX_FRAME_BYTES as u32 + 1).to_be_bytes())
            .await
            .unwrap();
        assert_eq!(
            read_frame(&mut b).await.unwrap_err().code,
            Refusal::LimitExceeded
        );
        let (mut a, mut b) = tokio::io::duplex(1024);
        a.write_all(&3u32.to_be_bytes()).await.unwrap();
        assert_eq!(
            read_frame(&mut b).await.unwrap_err().code,
            Refusal::Malformed
        );
        let (mut a, _b) = tokio::io::duplex(1024);
        let big = vec![0; MAX_FRAME_BYTES];
        assert_eq!(
            write_frame(&mut a, FrameKind::Data, 0, &big)
                .await
                .unwrap_err()
                .code,
            Refusal::LimitExceeded
        );
    }

    #[test]
    fn sealed_frames_bind_kind_and_sequence() {
        let key = [7; 32];
        let body = seal_frame(&key, FrameKind::Data, 5, b"hello").unwrap();
        let frame = Frame {
            kind: FrameKind::Data,
            seq: 5,
            body: body.clone(),
        };
        assert_eq!(open_frame(&key, &frame).unwrap(), b"hello");
        let moved = Frame {
            seq: 6,
            ..frame.clone()
        };
        assert!(open_frame(&key, &moved).is_err());
        let retyped = Frame {
            kind: FrameKind::Close,
            ..frame.clone()
        };
        assert!(open_frame(&key, &retyped).is_err());
        assert!(open_frame(&[8; 32], &frame).is_err());
        // The largest payload fits in one frame.
        let full = seal_frame(&key, FrameKind::Data, 0, &[0xff; MAX_DATA_BYTES]).unwrap();
        assert!(HEADER_BYTES + full.len() <= MAX_FRAME_BYTES);
    }
}
