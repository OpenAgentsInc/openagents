//! HPKE request carriage: seal, carry (signed or gift-wrapped), open.
use crate::keys::{Admitted, WorkerKeys};
use crate::stream::{ClientStream, StreamHeader, WorkerStream};
use crate::{Kem, Result, SUITE, b64, hex, unb64, unhex32};
use hpke::{
    Deserializable as _, OpModeR, OpModeS, Serializable as _, aead::AesGcm256, kdf::HkdfSha256,
};
use nostr::domain::{Event, Rumor, Tag};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use zeroize::Zeroizing;

pub const REQUEST_V: &str = "openagents.att-sealed-request.v1";
/// The CJ decision request kind carries the sealed request.
pub const REQUEST_KIND: u16 = nostr::decision::REQUEST_KIND;
/// The largest plaintext request.
pub const MAX_REQUEST_BYTES: usize = 48 * 1024;
/// How far ahead a request may set its expiry.
pub const MAX_REQUEST_TTL: u64 = 600;
const INFO: &[u8] = b"openagents.att.v1/request\0";
const AAD: &[u8] = b"openagents.att.v1/request-aad\0";

/// Everything the request binds, in the clear and authenticated.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Header {
    pub v: String,
    pub suite: String,
    /// The endpoint address the client verified.
    pub endpoint: String,
    /// The release the client verified.
    pub release: String,
    /// The endpoint's Nostr key.
    pub recipient: String,
    /// The HPKE public key sealed to, hex.
    pub key: String,
    /// A fresh 32-byte request ID, hex.
    pub request: String,
    /// The caller's Nostr key; output chunks are addressed to it.
    pub reply_to: String,
    pub expires_at: u64,
    /// The most output bytes the caller accepts.
    pub max_output_bytes: u64,
    /// The HPKE encapsulated key, base64.
    pub enc: String,
}

/// A sealed request: the header and the ciphertext.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SealedRequest {
    #[serde(flatten)]
    pub header: Header,
    pub ct: String,
}

fn aad(header: &Header) -> Result<Vec<u8>> {
    let mut out = AAD.to_vec();
    out.extend(serde_json::to_vec(header).map_err(|_| "the header does not encode")?);
    Ok(out)
}

fn info(binding: &[u8; 32]) -> Vec<u8> {
    let mut out = INFO.to_vec();
    out.extend_from_slice(binding);
    out
}

/// What the caller fixes for one request.
pub struct Ask<'a> {
    pub request: [u8; 32],
    pub reply_to: &'a str,
    pub expires_at: u64,
    pub max_output_bytes: u64,
}

/// Seal `plaintext` to an admitted key. Returns the request and the
/// client's stream reader for the answer.
///
/// # Errors
///
/// When the plaintext is too large or the key is not a key.
pub fn seal<R: hpke::rand_core::RngCore + hpke::rand_core::CryptoRng>(
    key: &Admitted,
    ask: &Ask<'_>,
    plaintext: &[u8],
    now: u64,
    random: &mut R,
) -> Result<(SealedRequest, ClientStream)> {
    if plaintext.len() > MAX_REQUEST_BYTES {
        return Err("the request is too large".into());
    }
    if ask.expires_at <= now || ask.expires_at > now + MAX_REQUEST_TTL {
        return Err("the request expiry is out of range".into());
    }
    let public = <Kem as hpke::Kem>::PublicKey::from_bytes(&key.hpke)
        .map_err(|_| "the HPKE key is not a key")?;
    let (enc, mut ctx) = hpke::setup_sender::<AesGcm256, HkdfSha256, Kem, _>(
        &OpModeS::Base,
        &public,
        &info(&key.binding),
        random,
    )
    .map_err(|_| "the request could not be sealed")?;
    let header = Header {
        v: REQUEST_V.into(),
        suite: SUITE.into(),
        endpoint: key.address.clone(),
        release: key.release.clone(),
        recipient: key.endpoint.clone(),
        key: hex(&key.hpke),
        request: hex(&ask.request),
        reply_to: ask.reply_to.into(),
        expires_at: ask.expires_at,
        max_output_bytes: ask.max_output_bytes,
        enc: b64(&enc.to_bytes()),
    };
    let ct = ctx
        .seal(plaintext, &aad(&header)?)
        .map_err(|_| "the request could not be sealed")?;
    let stream = ClientStream::new(
        StreamHeader::of(&header),
        crate::stream::derive(&|label, out| ctx.export(label, out))?,
    );
    Ok((
        SealedRequest {
            header,
            ct: b64(&ct),
        },
        stream,
    ))
}

/// The request as a plain signed event from the caller (`p` = endpoint).
///
/// # Errors
///
/// When the request does not encode.
pub fn event(
    caller: &nostr::domain::RelaySigner,
    sealed: &SealedRequest,
    now: u64,
) -> Result<Event> {
    let content = serde_json::to_string(sealed).map_err(|_| "the request does not encode")?;
    Ok(caller.sign(now, REQUEST_KIND, tags(sealed), content))
}

fn tags(sealed: &SealedRequest) -> Vec<Tag> {
    vec![
        Tag::new(vec!["p".into(), sealed.header.recipient.clone()]),
        Tag::new(vec![
            "expiration".into(),
            sealed.header.expires_at.to_string(),
        ]),
    ]
}

/// The request gift-wrapped (NIP-59 `1059`) to the endpoint key, so relays
/// see the endpoint and the size but not the caller. `nonces` seed the
/// seal and wrap encryption; `wrapper` is a one-use key.
///
/// # Errors
///
/// When a key is not a key or the wrap fails.
pub fn gift_wrap(
    caller: &secp256k1::SecretKey,
    wrapper: &secp256k1::SecretKey,
    sealed: &SealedRequest,
    now: u64,
    nonces: [[u8; 32]; 2],
) -> Result<Event> {
    let author = secp256k1::Keypair::from_secret_key(&secp256k1::Secp256k1::new(), caller)
        .x_only_public_key()
        .0
        .to_string();
    if author != sealed.header.reply_to {
        return Err("the wrap author is not the caller the request names".into());
    }
    let content = serde_json::to_string(sealed).map_err(|_| "the request does not encode")?;
    let rumor =
        Rumor::new(&author, now, REQUEST_KIND, tags(sealed), content).map_err(|e| e.to_string())?;
    let recipient: secp256k1::XOnlyPublicKey = sealed
        .header
        .recipient
        .parse()
        .map_err(|_| "the endpoint key is not a key")?;
    let seal_time = nostr::domain::randomized_timestamp(now, u64::from(nonces[0][0]))
        .map_err(|e| e.to_string())?;
    let seal = nostr::domain::seal_rumor(
        &rumor,
        caller,
        &recipient,
        seal_time,
        nonces[0],
        Some(sealed.header.expires_at),
    )
    .map_err(|e| e.to_string())?;
    nostr::domain::wrap_seal(&seal, wrapper, &recipient, now, nonces[1], 1_059)
        .map_err(|e| e.to_string())
}

/// Who the worker is.
pub struct Identity<'a> {
    pub address: &'a str,
    pub release: &'a str,
    pub endpoint: &'a str,
    pub binding: [u8; 32],
    /// The most output this worker will produce for one request.
    pub max_output_bytes: u64,
}

/// Request IDs this worker has opened, until they expire.
#[derive(Default, Debug)]
pub struct ReplayGuard {
    seen: BTreeMap<String, u64>,
}

impl ReplayGuard {
    fn check(&mut self, request: &str, expires_at: u64, now: u64) -> Result<()> {
        self.seen.retain(|_, until| *until > now);
        if self.seen.contains_key(request) {
            return Err("this request was already opened".into());
        }
        self.seen.insert(request.to_string(), expires_at);
        Ok(())
    }
}

/// A request the worker opened: the plaintext (zeroed on drop), the carrier
/// event the chunks answer, and the stream writer.
pub struct Opened {
    pub plaintext: Zeroizing<Vec<u8>>,
    pub carrier: String,
    pub stream: WorkerStream,
}

impl std::fmt::Debug for Opened {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Opened(carrier {}, plaintext redacted)", self.carrier)
    }
}

/// Take the request out of its carrier: a signed `REQUEST_KIND` event from
/// the caller, or a gift wrap to the endpoint key.
///
/// # Errors
///
/// When the carrier is not a request to this endpoint from the caller it
/// names.
pub fn unwrap(
    event: &Event,
    endpoint_secret: &secp256k1::SecretKey,
) -> Result<(SealedRequest, String)> {
    let (sealed, author, id): (SealedRequest, String, String) = if event.kind == 1_059 {
        let rumor = nostr::domain::open_wrap(event, endpoint_secret).map_err(|e| e.to_string())?;
        if rumor.kind != REQUEST_KIND {
            return Err("the gift wrap does not carry a sealed request".into());
        }
        let sealed = serde_json::from_str(&rumor.content)
            .map_err(|_| "the sealed request does not parse")?;
        (sealed, rumor.pubkey.clone(), rumor.id.clone())
    } else if event.kind == REQUEST_KIND {
        event
            .validate_crypto()
            .map_err(|_| "the request signature does not verify")?;
        let sealed = serde_json::from_str(&event.content)
            .map_err(|_| "the sealed request does not parse")?;
        (sealed, event.pubkey.clone(), event.id.clone())
    } else {
        return Err("not a sealed request".into());
    };
    if sealed.header.reply_to != author {
        return Err("the request names another caller than its author".into());
    }
    Ok((sealed, id))
}

/// Open a sealed request inside the worker.
///
/// # Errors
///
/// When it is for another endpoint, release, or key; expired; a replay;
/// asks for more output than the worker allows; or does not decrypt.
pub fn open(
    sealed: &SealedRequest,
    carrier: &str,
    me: &Identity<'_>,
    keys: &WorkerKeys,
    replay: &mut ReplayGuard,
    now: u64,
) -> Result<Opened> {
    let h = &sealed.header;
    if h.v != REQUEST_V || h.suite != SUITE {
        return Err("an unknown request version or suite".into());
    }
    if h.endpoint != me.address || h.recipient != me.endpoint {
        return Err("the request is for another endpoint".into());
    }
    if h.release != me.release {
        return Err("the request is for another release".into());
    }
    if h.expires_at <= now || h.expires_at > now + MAX_REQUEST_TTL {
        return Err("the request has expired".into());
    }
    if h.max_output_bytes == 0 || h.max_output_bytes > me.max_output_bytes {
        return Err("the request asks for more output than this endpoint allows".into());
    }
    unhex32(&h.request)?;
    let pair = keys.find(&unhex32(&h.key)?, now)?;
    let enc_bytes = unb64(&h.enc)?;
    let enc = <Kem as hpke::Kem>::EncappedKey::from_bytes(&enc_bytes)
        .map_err(|_| "the encapsulated key is malformed")?;
    let private = <Kem as hpke::Kem>::PrivateKey::from_bytes(pair.private())
        .map_err(|_| "the worker key is malformed")?;
    let mut ctx = hpke::setup_receiver::<AesGcm256, HkdfSha256, Kem>(
        &OpModeR::Base,
        &private,
        &enc,
        &info(&me.binding),
    )
    .map_err(|_| "the request does not open")?;
    let plaintext = Zeroizing::new(
        ctx.open(&unb64(&sealed.ct)?, &aad(h)?)
            .map_err(|_| "the request does not open")?,
    );
    replay.check(&h.request, h.expires_at, now)?;
    let stream = WorkerStream::new(
        StreamHeader::of(h),
        crate::stream::derive(&|label, out| ctx.export(label, out))?,
    );
    Ok(Opened {
        plaintext,
        carrier: carrier.to_string(),
        stream,
    })
}
