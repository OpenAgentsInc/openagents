//! Per-chunk sealed output. Each chunk is its own AES-256-GCM ciphertext
//! under a key exported from the request's HPKE context, with the stream
//! position, finality and the request's bindings as associated data, in an
//! event signed by the endpoint key. A reader verifies every chunk on its
//! own and refuses gaps, reordering, duplicates, foreign chunks, output
//! past the limit, chunks after the last, and unknown stop outcomes.
use crate::request::Header;
use crate::{Result, b64, unb64};
use aes_gcm::{
    Aes256Gcm, KeyInit, Nonce,
    aead::{Aead, Payload},
};
use nostr::domain::{Event, RelaySigner, Tag};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

pub const CHUNK_V: &str = "openagents.att-chunk.v1";
/// Chunks ride CJ execution progress events.
pub const CHUNK_KIND: u16 = 27_020;
/// The most text one chunk carries.
pub const MAX_CHUNK_BYTES: usize = 16 * 1024;
const EXPORT: &[u8] = b"openagents.att.v1/stream\0";
const AAD: &[u8] = b"openagents.att.v1/chunk\0";

/// How a stream ended. A closed set: anything else refuses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stop {
    /// The model finished.
    Complete,
    /// The output limit was reached.
    Length,
    /// The worker refused the job.
    Refused,
    /// The job was cancelled.
    Cancelled,
}

/// One chunk on the wire.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Chunk {
    pub v: String,
    pub request: String,
    pub seq: u32,
    pub last: bool,
    pub ct: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Inner {
    text: String,
    stop: Option<serde_json::Value>,
}

/// The request fields every chunk binds.
#[derive(Clone, Debug)]
pub struct StreamHeader {
    endpoint: String,
    release: String,
    recipient: String,
    request: String,
    reply_to: String,
    expires_at: u64,
    max_output_bytes: u64,
}

impl StreamHeader {
    pub(crate) fn of(h: &Header) -> Self {
        Self {
            endpoint: h.endpoint.clone(),
            release: h.release.clone(),
            recipient: h.recipient.clone(),
            request: h.request.clone(),
            reply_to: h.reply_to.clone(),
            expires_at: h.expires_at,
            max_output_bytes: h.max_output_bytes,
        }
    }

    fn aad(&self, seq: u32, last: bool) -> Vec<u8> {
        let mut out = AAD.to_vec();
        for field in [
            &self.endpoint,
            &self.release,
            &self.recipient,
            &self.request,
            &self.reply_to,
        ] {
            out.extend_from_slice(field.as_bytes());
            out.push(0);
        }
        out.extend_from_slice(&seq.to_be_bytes());
        out.push(u8::from(last));
        out
    }
}

pub(crate) struct Keys {
    key: Zeroizing<[u8; 32]>,
    base: [u8; 12],
}

/// Export the stream key and base nonce from the request's HPKE context.
pub(crate) fn derive(
    export: &dyn Fn(&[u8], &mut [u8]) -> std::result::Result<(), hpke::HpkeError>,
) -> Result<Keys> {
    let mut out = Zeroizing::new([0u8; 44]);
    export(EXPORT, out.as_mut()).map_err(|_| "the stream key could not be derived")?;
    let mut key = Zeroizing::new([0u8; 32]);
    key.copy_from_slice(&out[..32]);
    let mut base = [0u8; 12];
    base.copy_from_slice(&out[32..]);
    Ok(Keys { key, base })
}

impl Keys {
    fn nonce(&self, seq: u32) -> [u8; 12] {
        let mut nonce = self.base;
        for (i, b) in seq.to_be_bytes().iter().enumerate() {
            nonce[8 + i] ^= b;
        }
        nonce
    }
    fn cipher(&self) -> Aes256Gcm {
        Aes256Gcm::new(self.key.as_ref().into())
    }
}

/// The worker's side: produces chunks in order, within the limit, and
/// nothing after the last.
pub struct WorkerStream {
    header: StreamHeader,
    keys: Keys,
    seq: u32,
    bytes: u64,
    done: bool,
}

impl std::fmt::Debug for WorkerStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "WorkerStream(request {}, seq {}, key redacted)",
            self.header.request, self.seq
        )
    }
}

impl WorkerStream {
    pub(crate) fn new(header: StreamHeader, keys: Keys) -> Self {
        Self {
            header,
            keys,
            seq: 0,
            bytes: 0,
            done: false,
        }
    }

    /// Seal the next chunk of output. `stop` ends the stream.
    ///
    /// # Errors
    ///
    /// After the last chunk, past the output limit, or for an oversized
    /// chunk.
    pub fn chunk(&mut self, text: &str, stop: Option<Stop>) -> Result<Chunk> {
        if self.done {
            return Err("the stream has already ended".into());
        }
        if text.len() > MAX_CHUNK_BYTES {
            return Err("a chunk is too large".into());
        }
        let total = self.bytes + text.len() as u64;
        if total > self.header.max_output_bytes {
            return Err("the output limit would be exceeded; end with a length stop".into());
        }
        let last = stop.is_some();
        let inner = Inner {
            text: text.into(),
            stop: stop.map(|s| serde_json::to_value(s).unwrap_or_default()),
        };
        let plaintext =
            Zeroizing::new(serde_json::to_vec(&inner).map_err(|_| "the chunk does not encode")?);
        let ct = self.seal(self.seq, last, &plaintext)?;
        let chunk = Chunk {
            v: CHUNK_V.into(),
            request: self.header.request.clone(),
            seq: self.seq,
            last,
            ct: b64(&ct),
        };
        self.seq += 1;
        self.bytes = total;
        self.done = last;
        Ok(chunk)
    }

    /// Seal raw inner bytes at a given position. Exposed to tests that
    /// forge chunks a well-behaved worker would never send.
    #[doc(hidden)]
    pub fn seal(&self, seq: u32, last: bool, plaintext: &[u8]) -> Result<Vec<u8>> {
        let aad = self.header.aad(seq, last);
        self.keys
            .cipher()
            .encrypt(
                Nonce::from_slice(&self.keys.nonce(seq)),
                Payload {
                    msg: plaintext,
                    aad: &aad,
                },
            )
            .map_err(|_| "the chunk could not be sealed".into())
    }

    /// The chunk as a CJ progress event signed by the endpoint key,
    /// answering `carrier` (the request event or rumor ID).
    ///
    /// # Errors
    ///
    /// When the chunk does not encode.
    pub fn event(
        &self,
        endpoint: &RelaySigner,
        carrier: &str,
        chunk: &Chunk,
        now: u64,
    ) -> Result<Event> {
        let content = serde_json::to_string(chunk).map_err(|_| "the chunk does not encode")?;
        let tags = vec![
            Tag::new(vec!["e".into(), carrier.into()]),
            Tag::new(vec!["p".into(), self.header.reply_to.clone()]),
        ];
        Ok(endpoint.sign(now, CHUNK_KIND, tags, content))
    }
}

/// One verified piece of output.
#[derive(Clone, PartialEq, Eq)]
pub struct Piece {
    pub seq: u32,
    pub text: Zeroizing<String>,
    pub stop: Option<Stop>,
}

impl std::fmt::Debug for Piece {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Piece(seq {}, stop {:?}, text redacted)",
            self.seq, self.stop
        )
    }
}

/// The client's side: verifies each chunk event on its own and in order.
pub struct ClientStream {
    header: StreamHeader,
    keys: Keys,
    carrier: Option<String>,
    next: u32,
    bytes: u64,
    stop: Option<Stop>,
}

impl std::fmt::Debug for ClientStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ClientStream(request {}, next {}, key redacted)",
            self.header.request, self.next
        )
    }
}

impl ClientStream {
    pub(crate) fn new(header: StreamHeader, keys: Keys) -> Self {
        Self {
            header,
            keys,
            carrier: None,
            next: 0,
            bytes: 0,
            stop: None,
        }
    }

    /// The ID of the event that carried the request (the signed event, or
    /// the rumor inside a gift wrap); chunks must answer it.
    pub fn answering(&mut self, carrier: &str) {
        self.carrier = Some(carrier.to_string());
    }

    /// Verify one chunk event and return its piece of output.
    ///
    /// # Errors
    ///
    /// When it is not this endpoint's chunk for this request, out of order,
    /// a duplicate, after the last chunk, past the limit, does not decrypt
    /// under this request's key, or names an unknown stop outcome.
    pub fn accept(&mut self, event: &Event, now: u64) -> Result<Piece> {
        if now > self.header.expires_at {
            return Err("the answer arrived after the request expired".into());
        }
        if event.kind != CHUNK_KIND || event.pubkey != self.header.recipient {
            return Err("the chunk is not from the attested endpoint".into());
        }
        event
            .validate_crypto()
            .map_err(|_| "the chunk signature does not verify")?;
        let carrier = self
            .carrier
            .as_deref()
            .ok_or("the request carrier is not set")?;
        if event.tag_values("e").next() != Some(carrier)
            || event.tag_values("p").next() != Some(self.header.reply_to.as_str())
        {
            return Err("the chunk answers another request".into());
        }
        let chunk: Chunk =
            serde_json::from_str(&event.content).map_err(|_| "the chunk does not parse")?;
        if chunk.v != CHUNK_V {
            return Err("an unknown chunk version".into());
        }
        if chunk.request != self.header.request {
            return Err("the chunk answers another request".into());
        }
        if self.stop.is_some() {
            return Err("a chunk arrived after the last one".into());
        }
        if chunk.seq < self.next {
            return Err("a chunk is repeated or out of order".into());
        }
        if chunk.seq > self.next {
            return Err("a chunk is missing".into());
        }
        let aad = self.header.aad(chunk.seq, chunk.last);
        let plaintext = Zeroizing::new(
            self.keys
                .cipher()
                .decrypt(
                    Nonce::from_slice(&self.keys.nonce(chunk.seq)),
                    Payload {
                        msg: &unb64(&chunk.ct)?,
                        aad: &aad,
                    },
                )
                .map_err(|_| "the chunk does not open under this request's key")?,
        );
        let inner: Inner =
            serde_json::from_slice(&plaintext).map_err(|_| "the chunk content does not parse")?;
        let text = Zeroizing::new(inner.text);
        let stop = match inner.stop {
            None => None,
            Some(value) => Some(
                serde_json::from_value::<Stop>(value)
                    .map_err(|_| "the chunk names an unknown stop outcome")?,
            ),
        };
        if stop.is_some() != chunk.last {
            return Err("the last chunk and the stop outcome disagree".into());
        }
        let total = self.bytes + text.len() as u64;
        if total > self.header.max_output_bytes {
            return Err("the output exceeds the requested limit".into());
        }
        self.bytes = total;
        self.next += 1;
        self.stop = stop;
        Ok(Piece {
            seq: chunk.seq,
            text,
            stop,
        })
    }

    /// The stream's outcome, once the last chunk arrived.
    ///
    /// # Errors
    ///
    /// While the stream has not ended.
    pub fn finish(&self) -> Result<Stop> {
        self.stop
            .ok_or_else(|| "the stream ended without a last chunk".into())
    }
}
