//! Closed observer messages inside original signed private artifact envelopes.
use crate::{Error, ErrorCode, Result, fail};
use coder_history::{CatalogPage, CatalogRequest, TranscriptPage, TranscriptRequest};
use nostr::{contracts, domain::Event};
use secp256k1::{SecretKey, XOnlyPublicKey, rand::RngCore};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::json;
use std::str::FromStr;

pub const MAX_BODY: usize = 128 * 1024;
/// A body on a direct tailnet connection. A sealed envelope's inline body is
/// bounded by NIP-44's 64 KiB standard plaintext, so a direct reply's body
/// travels beside its sealed envelope (see [`seal_detached`]), encrypted with
/// NIP-44's 256 KiB client bound and pinned by the envelope's digest.
pub const MAX_DIRECT_BODY: usize = 248 * 1024;
pub const MAX_GRANT_LIFETIME: u64 = 30 * 24 * 60 * 60;
pub const MAX_REQUEST_LIFETIME: u64 = 60;
pub const GRANT: &str = "openagents.history-observer-grant.v1";
pub const CONNECTION: &str = "openagents.history-observer-connection.v1";
pub const REQUEST: &str = "openagents.history-observer-request.v1";
pub const REPLY: &str = "openagents.history-observer-reply.v1";

/// How a sealed request and its reply travel. The request, its grant, its
/// authorization, and every host check are the same on both; only the
/// bounds of one reply differ.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    /// Through the grant's relay.
    Relay,
    /// On a direct connection to the host's tailnet address.
    Direct,
}
impl Route {
    pub fn limits(self) -> coder_history::Limits {
        match self {
            Self::Relay => coder_history::Limits::RELAY,
            Self::Direct => coder_history::Limits::DIRECT,
        }
    }
    pub fn body(self) -> usize {
        match self {
            Self::Relay => MAX_BODY,
            Self::Direct => MAX_DIRECT_BODY,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelayPolicy {
    Production,
    LoopbackTest,
}
impl RelayPolicy {
    pub fn validate(self, relay: &str) -> Result<()> {
        let url = url::Url::parse(relay)
            .map_err(|_| Error::new(ErrorCode::Malformed, "invalid relay URL"))?;
        let local = match url.host() {
            Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
            Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
            _ => false,
        };
        if relay.len() > 2048
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
            || url.query().is_some()
            || url.host().is_none()
            || !relay.is_ascii()
            || relay
                .bytes()
                .any(|b| b.is_ascii_control() || b.is_ascii_whitespace())
            || !(url.scheme() == "wss"
                || (self == Self::LoopbackTest && url.scheme() == "ws" && local))
        {
            return fail(
                ErrorCode::Forbidden,
                "relay requires an exact credential-free wss URL; ws is loopback-test only",
            );
        }
        Ok(())
    }
}

/// The most source collections one grant names. Before OpenCode was a
/// source it was three; a device built before then refuses a grant with
/// more.
pub const MAX_SOURCES: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    Codex,
    Claude,
    /// Coder task transcripts, `*.atif.jsonl` in Coder's task directory.
    Coder,
    /// OpenCode sessions, as the host mirrors them from OpenCode's database
    /// (`coder_history::opencode`).
    #[serde(rename = "opencode")]
    OpenCode,
    /// Devin CLI sessions, as the host mirrors them from Devin's session
    /// store (`coder_history::devin`).
    Devin,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceScope {
    pub id: String,
    pub label: String,
    pub kind: SourceKind,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grant {
    pub v: String,
    pub requires: Vec<String>,
    pub grant: String,
    pub host: String,
    pub client: String,
    pub relay: String,
    pub sources: Vec<SourceScope>,
    pub issued_at: u64,
    pub expires_at: u64,
}
impl Grant {
    pub fn validate(&self, policy: RelayPolicy) -> Result<()> {
        schema(&self.v, GRANT, &self.requires)?;
        identity(&self.grant)?;
        public(&self.host)?;
        public(&self.client)?;
        if self.host == self.client {
            return fail(ErrorCode::Forbidden, "host and client keys must differ");
        }
        policy.validate(&self.relay)?;
        window(self.issued_at, self.expires_at, MAX_GRANT_LIFETIME)?;
        if self.sources.is_empty() || self.sources.len() > MAX_SOURCES {
            return fail(
                ErrorCode::Bounds,
                "grant needs one to eight explicit source collections",
            );
        }
        for (i, source) in self.sources.iter().enumerate() {
            identity(&source.id)?;
            if source.label.is_empty()
                || source.label.len() > 128
                || source.label.chars().any(char::is_control)
                || self.sources[..i]
                    .iter()
                    .any(|old| old.id == source.id || old.kind == source.kind)
            {
                return fail(
                    ErrorCode::Malformed,
                    "source identities, labels, and kinds must be bounded and unique",
                );
            }
        }
        Ok(())
    }
}

/// Public bootstrap data. Authentic transfer of this code is a local pairing step.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectionCode {
    pub v: String,
    pub requires: Vec<String>,
    pub host: String,
    pub client: String,
    pub relay: String,
    pub grant: String,
    pub sources: Vec<SourceScope>,
    pub expires_at: u64,
    pub authorization: Event,
}
impl ConnectionCode {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        decode(bytes)
    }
    /// Verify pinned identity and lifetime offline. This cannot check revocation.
    pub fn verify(&self, secret: &SecretKey, now: u64, policy: RelayPolicy) -> Result<Grant> {
        schema(&self.v, CONNECTION, &self.requires)?;
        if self.client != pubkey(secret) {
            return fail(
                ErrorCode::Forbidden,
                "connection belongs to another client key",
            );
        }
        let grant: Grant = open(&self.authorization, secret, &self.host, &self.client, GRANT)?;
        grant.validate(policy)?;
        if grant.host != self.host
            || grant.client != self.client
            || grant.relay != self.relay
            || grant.grant != self.grant
            || grant.sources != self.sources
            || grant.expires_at != self.expires_at
            || self.authorization.tag_values("h").collect::<Vec<_>>() != [self.grant.as_str()]
        {
            return fail(
                ErrorCode::Forbidden,
                "connection and original signed grant differ",
            );
        }
        fresh(grant.issued_at, grant.expires_at, now)?;
        Ok(grant)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "request",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Query {
    Catalog(CatalogRequest),
    Page(TranscriptRequest),
}
impl Query {
    pub fn validate(&self) -> Result<()> {
        self.validate_for(Route::Relay)
    }
    /// Check the query against the bounds of the route it travels.
    pub fn validate_for(&self, route: Route) -> Result<()> {
        let limits = route.limits();
        if encoded(self)?.len() > 8192 {
            return fail(ErrorCode::Bounds, "query exceeds its byte bound");
        }
        match self {
            Self::Catalog(q) if q.limit == 0 || q.limit > limits.catalog_page => {
                fail(ErrorCode::Bounds, "catalog limit exceeds the reader bound")
            }
            Self::Page(q)
                if q.max_bytes == 0
                    || q.max_bytes > limits.page_bytes
                    || q.source_id.is_empty()
                    || q.source_id.len() > 128 =>
            {
                fail(
                    ErrorCode::Bounds,
                    "transcript query exceeds its source or byte bound",
                )
            }
            _ => Ok(()),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "page",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Observation {
    Catalog(CatalogPage),
    Page(TranscriptPage),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub v: String,
    pub requires: Vec<String>,
    pub request: String,
    pub grant: String,
    pub authorization: String,
    pub issued_at: u64,
    pub expires_at: u64,
    pub query: Query,
}
impl Request {
    pub fn validate(&self) -> Result<()> {
        self.validate_for(Route::Relay)
    }
    pub fn validate_for(&self, route: Route) -> Result<()> {
        schema(&self.v, REQUEST, &self.requires)?;
        for id in [&self.request, &self.grant, &self.authorization] {
            identity(id)?;
        }
        window(self.issued_at, self.expires_at, MAX_REQUEST_LIFETIME)?;
        self.query.validate_for(route)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReplyResult {
    Ok { observation: Box<Observation> },
    Refused { code: ErrorCode },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reply {
    pub v: String,
    pub requires: Vec<String>,
    pub request: String,
    pub request_event: String,
    pub grant: String,
    pub issued_at: u64,
    pub expires_at: u64,
    pub result: ReplyResult,
}

pub fn pubkey(secret: &SecretKey) -> String {
    secret
        .x_only_public_key(&secp256k1::Secp256k1::new())
        .0
        .to_string()
}
pub fn random_id() -> String {
    random_bytes().iter().map(|b| format!("{b:02x}")).collect()
}
fn random_bytes() -> [u8; 32] {
    let mut bytes = [0; 32];
    secp256k1::rand::rng().fill_bytes(&mut bytes);
    bytes
}
pub fn identity(id: &str) -> Result<()> {
    if id.len() != 64
        || !id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return fail(
            ErrorCode::Malformed,
            "identity must be lower-case 32-byte hex",
        );
    }
    Ok(())
}
pub fn public(id: &str) -> Result<()> {
    identity(id)?;
    XOnlyPublicKey::from_str(id)
        .map(|_| ())
        .map_err(|_| Error::new(ErrorCode::Malformed, "invalid public key"))
}
pub fn schema(actual: &str, expected: &str, requires: &[String]) -> Result<()> {
    if actual != expected || !requires.is_empty() {
        return fail(
            ErrorCode::Unsupported,
            "unsupported observer schema or feature",
        );
    }
    Ok(())
}
pub fn window(issued: u64, expires: u64, max: u64) -> Result<()> {
    if issued >= expires || expires > 9_007_199_254_740_991 || expires - issued > max {
        return fail(ErrorCode::Malformed, "invalid observer lifetime");
    }
    Ok(())
}
pub fn fresh(issued: u64, expires: u64, now: u64) -> Result<()> {
    if issued > now || expires <= now {
        return fail(ErrorCode::Expired, "observer artifact is not current");
    }
    Ok(())
}
pub fn encoded(value: &impl Serialize) -> Result<Vec<u8>> {
    encoded_within(value, MAX_BODY)
}
/// Canonical bytes of an observer body of at most `max` bytes.
pub fn encoded_within(value: &impl Serialize, max: usize) -> Result<Vec<u8>> {
    let value = serde_json::to_value(value)
        .map_err(|_| Error::new(ErrorCode::Malformed, "observer serialization failed"))?;
    let bytes = contracts::jcs(&value).map_err(|_| {
        Error::new(
            ErrorCode::Malformed,
            "observer canonical serialization failed",
        )
    })?;
    if bytes.len() > max {
        return fail(ErrorCode::Bounds, "observer body exceeds its byte bound");
    }
    Ok(bytes)
}
pub fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    decode_within(bytes, MAX_BODY)
}
pub fn decode_within<T: DeserializeOwned>(bytes: &[u8], max: usize) -> Result<T> {
    let value = contracts::parse_strict_bounded(bytes, max)
        .map_err(|_| Error::new(ErrorCode::Malformed, "invalid bounded observer JSON"))?;
    serde_json::from_value(value).map_err(|_| {
        Error::new(
            ErrorCode::Malformed,
            "observer fields do not match the supported schema",
        )
    })
}
pub fn seal(
    value: &impl Serialize,
    schema: &str,
    secret: &SecretKey,
    recipient: &str,
    mailbox: &str,
    issued: u64,
    expires: u64,
) -> Result<Event> {
    seal_within(
        value,
        schema,
        secret,
        recipient,
        (mailbox, issued, expires),
        MAX_BODY,
    )
}
/// [`seal`] a body of at most `max` bytes.
pub fn seal_within(
    value: &impl Serialize,
    schema: &str,
    secret: &SecretKey,
    recipient: &str,
    (mailbox, issued, expires): (&str, u64, u64),
    max: usize,
) -> Result<Event> {
    let bytes = encoded_within(value, max)?;
    let inline: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|_| Error::new(ErrorCode::Malformed, "observer body"))?;
    let body = json!({"v":"openagents.artifact-envelope.v1","requires":[],"artifact":{"digest":contracts::digest_bytes(&bytes),"size":bytes.len(),"media_type":"application/json","schema":schema},"inline":inline,"issued_at":issued,"retain_until":expires});
    let recipient = XOnlyPublicKey::from_str(recipient)
        .map_err(|_| Error::new(ErrorCode::Malformed, "invalid recipient"))?;
    nostr::private_artifact::seal(&body, secret, &recipient, mailbox, issued, random_bytes())
        .map_err(|_| Error::new(ErrorCode::Malformed, "observer envelope cannot be sealed"))
}
/// Seal a body that travels beside its envelope: the signed envelope names
/// the body's digest and size with no inline copy, and the body is NIP-44
/// encrypted to the same recipient under the same conversation key, up to
/// NIP-44's 256 KiB client plaintext. Returns the envelope event and the
/// encrypted body.
pub fn seal_detached(
    value: &impl Serialize,
    schema: &str,
    secret: &SecretKey,
    recipient: &str,
    (mailbox, issued, expires): (&str, u64, u64),
    max: usize,
) -> Result<(Event, String)> {
    let bytes = encoded_within(value, max)?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| Error::new(ErrorCode::Malformed, "observer body"))?;
    let body = json!({"v":"openagents.artifact-envelope.v1","requires":[],"artifact":{"digest":contracts::digest_bytes(&bytes),"size":bytes.len(),"media_type":"application/json","schema":schema},"inline":null,"issued_at":issued,"retain_until":expires});
    let peer = XOnlyPublicKey::from_str(recipient)
        .map_err(|_| Error::new(ErrorCode::Malformed, "invalid recipient"))?;
    let event =
        nostr::private_artifact::seal(&body, secret, &peer, mailbox, issued, random_bytes())
            .map_err(|_| Error::new(ErrorCode::Malformed, "observer envelope cannot be sealed"))?;
    let key = nostr::nip44::conversation_key(secret, &peer);
    let payload = nostr::nip44::encrypt(text, &key, random_bytes())
        .map_err(|_| Error::new(ErrorCode::Bounds, "observer body cannot be encrypted"))?;
    Ok((event, payload))
}
/// Open a body sealed by [`seal_detached`]: the envelope's signature,
/// signer, recipient, schema, and lifetimes are checked as by [`open`], and
/// the decrypted body must be exactly the bytes the envelope names.
pub fn open_detached<T: DeserializeOwned>(
    event: &Event,
    payload: &str,
    secret: &SecretKey,
    (signer, recipient): (&str, &str),
    schema: &str,
    max: usize,
) -> Result<T> {
    // NIP-44 pads a plaintext by up to an eighth and adds 65 bytes before
    // base64; decryption bounds it again and the plaintext is checked below.
    if event.content.len() > 400 * 1024 || payload.len() > 4 * (max + max / 8 + 128) / 3 + 64 {
        return fail(
            ErrorCode::Bounds,
            "observer detached body exceeds its bound",
        );
    }
    let opened = nostr::private_artifact::open(event, secret).map_err(|_| {
        Error::new(
            ErrorCode::Forbidden,
            "invalid observer signature, encryption, or recipient",
        )
    })?;
    if opened.signer() != signer
        || opened.recipient() != recipient
        || opened.artifact().schema.as_deref() != Some(schema)
        || opened.artifact().media_type != "application/json"
        || event.created_at != opened.body().issued_at
        || opened.inline_bytes().is_some()
    {
        return fail(
            ErrorCode::Forbidden,
            "observer signer, recipient, schema, or issue time differs",
        );
    }
    let own = pubkey(secret);
    let peer = if own == signer { recipient } else { signer };
    let peer = XOnlyPublicKey::from_str(peer)
        .map_err(|_| Error::new(ErrorCode::Malformed, "invalid observer peer"))?;
    let key = nostr::nip44::conversation_key(secret, &peer);
    let text = nostr::nip44::decrypt(payload, &key)
        .map_err(|_| Error::new(ErrorCode::Forbidden, "observer detached body encryption"))?;
    if text.len() > max {
        return fail(ErrorCode::Bounds, "observer body exceeds its byte bound");
    }
    opened.check_external(text.as_bytes()).map_err(|_| {
        Error::new(
            ErrorCode::Forbidden,
            "observer body differs from the bytes its envelope names",
        )
    })?;
    let value: serde_json::Value = decode_within(text.as_bytes(), max)?;
    if value["issued_at"].as_u64() != Some(opened.body().issued_at)
        || value["expires_at"].as_u64() != Some(opened.body().retain_until)
    {
        return fail(
            ErrorCode::Forbidden,
            "observer body and envelope lifetimes differ",
        );
    }
    serde_json::from_value(value).map_err(|_| {
        Error::new(
            ErrorCode::Malformed,
            "observer fields do not match the supported schema",
        )
    })
}
pub fn open<T: DeserializeOwned>(
    event: &Event,
    secret: &SecretKey,
    signer: &str,
    recipient: &str,
    schema: &str,
) -> Result<T> {
    open_within(event, secret, (signer, recipient), schema, MAX_BODY)
}
/// [`open`] a body of at most `max` bytes.
pub fn open_within<T: DeserializeOwned>(
    event: &Event,
    secret: &SecretKey,
    (signer, recipient): (&str, &str),
    schema: &str,
    max: usize,
) -> Result<T> {
    if event.content.len() > 400 * 1024 {
        return fail(
            ErrorCode::Bounds,
            "observer encrypted event exceeds its byte bound",
        );
    }
    let opened = nostr::private_artifact::open(event, secret).map_err(|_| {
        Error::new(
            ErrorCode::Forbidden,
            "invalid observer signature, encryption, or recipient",
        )
    })?;
    if opened.signer() != signer
        || opened.recipient() != recipient
        || opened.artifact().schema.as_deref() != Some(schema)
        || opened.artifact().media_type != "application/json"
        || event.created_at != opened.body().issued_at
    {
        return fail(
            ErrorCode::Forbidden,
            "observer signer, recipient, schema, or issue time differs",
        );
    }
    let value: serde_json::Value = decode_within(
        opened.inline_bytes().ok_or_else(|| {
            Error::new(ErrorCode::Unavailable, "inline observer bytes unavailable")
        })?,
        max,
    )?;
    if value["issued_at"].as_u64() != Some(opened.body().issued_at)
        || value["expires_at"].as_u64() != Some(opened.body().retain_until)
    {
        return fail(
            ErrorCode::Forbidden,
            "observer body and envelope lifetimes differ",
        );
    }
    serde_json::from_value(value).map_err(|_| {
        Error::new(
            ErrorCode::Malformed,
            "observer fields do not match the supported schema",
        )
    })
}
