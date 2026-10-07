//! NIP-AE agent engrams: an agent's private memory as `kind:30174` events.
//!
//! An engram is an addressable event that the agent signs and encrypts with
//! NIP-44 v2 under the conversation key `K_c` it shares with its owner, so
//! both parties decrypt every record. This module holds the pure codec:
//!
//! - [`Slug`] checks the slug grammar (`core` or `mem/...`, at most 255
//!   bytes).
//! - [`Pair::d_tag`] derives the blinded `d` tag:
//!   `lower_hex(HMAC-SHA256(K_c, "agent-memory/v1/d-tag" || 0x00 || slug))`.
//! - [`Body`] is the decrypted record. [`Body::parse`] rejects a duplicate
//!   member name anywhere in the JSON and ignores unknown fields.
//! - [`build_event`] encrypts and signs a body. [`validate_and_decrypt`]
//!   applies the five validity rules in order from either side of the pair.
//! - [`select_head`], [`list_heads`], and [`monotonic_created_at`] implement
//!   head selection, listing, and monotonic writes.
//! - [`wiki_links`] extracts `[[slug]]` references.
//!
//! The module does no I/O and draws no randomness. A caller supplies the
//! clock, the NIP-44 nonce, and the BIP-340 auxiliary bytes, and production
//! callers draw the last two from a CSPRNG.

use std::collections::BTreeMap;
use std::fmt;

use secp256k1::{Keypair, Secp256k1, SecretKey, XOnlyPublicKey};
use serde::Deserialize;
use serde::de::{self, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};

use crate::domain::hex::encode_lower_hex;
use crate::domain::{Event, Tag};
use crate::nip44;
use crate::nip44::primitives::hmac_sha256;

/// The event kind NIP-AE claims for agent engrams.
pub const ENGRAM_KIND: u16 = 30_174;

/// The reserved slug of the agent's core profile.
pub const CORE_SLUG: &str = "core";

/// The fixed domain prefix of the `d` tag derivation.
pub const D_TAG_DOMAIN: &[u8] = b"agent-memory/v1/d-tag";

/// The longest slug, in bytes.
pub const MAX_SLUG_BYTES: usize = 255;

/// The longest serialized body, in bytes: the NIP-44 plaintext limit.
pub const MAX_BODY_BYTES: usize = 65_535;

/// The NIP-31 `alt` text that [`build_event`] adds when asked.
pub const ALT_TEXT: &str = "encrypted agent memory record";

/// How far ahead of the clock a monotonic write may land before
/// [`monotonic_created_at`] treats the prior head as clock-poisoned.
pub const CLOCK_POISON_SECS: u64 = 3_600;

const MEMORY_PREFIX: &str = "mem/";
const MAX_SEGMENT_BYTES: usize = 64;
const DUPLICATE_MARKER: &str = "engram body repeats member name";

/// Why an engram operation failed.
///
/// The validity variants follow the order of NIP-AE's five rules, so the
/// first failing rule names the error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngramError {
    /// The string is not a valid slug.
    InvalidSlug(String),
    /// Rule 1: the event's kind is not `30174`.
    WrongKind(u16),
    /// Rule 1: the event's author is not the pair's agent.
    WrongAuthor,
    /// Rule 1: the event does not carry exactly one valued `d` tag.
    DTagCount,
    /// Rule 1: the event does not carry exactly one valued `p` tag.
    PTagCount,
    /// Rule 1: the `p` tag does not name the pair's owner.
    WrongOwner,
    /// Rule 2: the event's id or signature does not verify.
    Signature,
    /// Rule 3: the content does not decrypt under `K_c`.
    Decrypt(String),
    /// Rule 3: the plaintext is not a single JSON object.
    NotJsonObject(String),
    /// Rule 3: a JSON object in the body repeats a member name.
    DuplicateMember,
    /// Rule 4: the body's slug is missing or breaks the grammar.
    BodySlug(String),
    /// Rule 4: the body's slug does not re-derive to the event's `d` tag.
    DTagMismatch,
    /// Rule 5: the body's shape does not match the type its slug names.
    Shape(String),
    /// The serialized body exceeds [`MAX_BODY_BYTES`].
    BodyTooLarge(usize),
    /// An extra field reuses a member name the body type defines.
    ReservedField(String),
    /// The prior head lies so far ahead of the clock that a write would
    /// land more than the threshold in the future.
    ClockPoisoned {
        /// The prior head's `created_at`.
        head: u64,
        /// The caller's clock.
        now: u64,
    },
    /// The event could not be serialized or encrypted.
    Encode(String),
}

impl fmt::Display for EngramError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSlug(reason) => write!(f, "invalid engram slug: {reason}"),
            Self::WrongKind(kind) => write!(f, "engram kind is {kind}, not 30174"),
            Self::WrongAuthor => f.write_str("engram author is not the agent"),
            Self::DTagCount => f.write_str("engram must carry exactly one d tag"),
            Self::PTagCount => f.write_str("engram must carry exactly one p tag"),
            Self::WrongOwner => f.write_str("engram p tag does not name the owner"),
            Self::Signature => f.write_str("engram id or signature does not verify"),
            Self::Decrypt(reason) => write!(f, "engram content does not decrypt: {reason}"),
            Self::NotJsonObject(reason) => write!(f, "engram body is not a JSON object: {reason}"),
            Self::DuplicateMember => f.write_str("engram body repeats a member name"),
            Self::BodySlug(reason) => write!(f, "engram body slug is invalid: {reason}"),
            Self::DTagMismatch => f.write_str("engram body slug does not derive the d tag"),
            Self::Shape(reason) => write!(f, "engram body has the wrong shape: {reason}"),
            Self::BodyTooLarge(len) => {
                write!(
                    f,
                    "engram body is {len} bytes; the limit is {MAX_BODY_BYTES}"
                )
            }
            Self::ReservedField(name) => {
                write!(f, "engram extra field {name:?} is defined by the body type")
            }
            Self::ClockPoisoned { head, now } => write!(
                f,
                "engram head created_at {head} is implausibly ahead of the clock {now}"
            ),
            Self::Encode(reason) => write!(f, "engram cannot be encoded: {reason}"),
        }
    }
}

impl std::error::Error for EngramError {}

/// A slug that satisfies the NIP-AE grammar: `core`, or `mem/` followed by
/// one or more `/`-separated segments of `[a-z0-9][a-z0-9_-]{0,63}`, with
/// the whole slug at most [`MAX_SLUG_BYTES`] bytes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Slug(String);

impl Slug {
    /// Parses `text` as a slug.
    ///
    /// # Errors
    ///
    /// Returns [`EngramError::InvalidSlug`] when `text` breaks the grammar.
    pub fn parse(text: &str) -> Result<Self, EngramError> {
        validate_slug(text).map(|()| Self(text.to_owned()))
    }

    /// Returns the reserved `core` slug.
    #[must_use]
    pub fn core() -> Self {
        Self(CORE_SLUG.to_owned())
    }

    /// Returns the slug text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns whether this is the `core` slug.
    #[must_use]
    pub fn is_core(&self) -> bool {
        self.0 == CORE_SLUG
    }
}

impl fmt::Display for Slug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Checks `text` against the slug grammar.
///
/// # Errors
///
/// Returns [`EngramError::InvalidSlug`] with the first rule `text` breaks.
pub fn validate_slug(text: &str) -> Result<(), EngramError> {
    if text == CORE_SLUG {
        return Ok(());
    }
    if text.len() > MAX_SLUG_BYTES {
        return Err(EngramError::InvalidSlug(format!(
            "a slug is at most {MAX_SLUG_BYTES} bytes"
        )));
    }
    let Some(path) = text.strip_prefix(MEMORY_PREFIX) else {
        return Err(EngramError::InvalidSlug(
            "a slug is core or starts with mem/".to_owned(),
        ));
    };
    for segment in path.split('/') {
        let bytes = segment.as_bytes();
        let Some((first, rest)) = bytes.split_first() else {
            return Err(EngramError::InvalidSlug(
                "a slug segment is not empty".to_owned(),
            ));
        };
        if bytes.len() > MAX_SEGMENT_BYTES {
            return Err(EngramError::InvalidSlug(format!(
                "a slug segment is at most {MAX_SEGMENT_BYTES} bytes"
            )));
        }
        if !is_lower_alphanumeric(*first) {
            return Err(EngramError::InvalidSlug(
                "a slug segment starts with a-z or 0-9".to_owned(),
            ));
        }
        if !rest
            .iter()
            .all(|byte| is_lower_alphanumeric(*byte) || matches!(byte, b'_' | b'-'))
        {
            return Err(EngramError::InvalidSlug(
                "a slug segment holds only a-z, 0-9, _, and -".to_owned(),
            ));
        }
    }
    Ok(())
}

fn is_lower_alphanumeric(byte: u8) -> bool {
    byte.is_ascii_lowercase() || byte.is_ascii_digit()
}

/// One agent-owner pair, seen from either side, with its conversation key.
///
/// `K_c` is symmetric, so [`Pair::for_agent`] and [`Pair::for_owner`] build
/// equal pairs for the same two keys.
#[derive(Clone, PartialEq, Eq)]
pub struct Pair {
    agent: XOnlyPublicKey,
    owner: XOnlyPublicKey,
    conversation_key: [u8; 32],
}

impl Pair {
    /// Builds the pair from the agent's secret key and the owner's public key.
    #[must_use]
    pub fn for_agent(agent_secret: &SecretKey, owner: &XOnlyPublicKey) -> Self {
        Self {
            agent: x_only(agent_secret),
            owner: *owner,
            conversation_key: nip44::conversation_key(agent_secret, owner),
        }
    }

    /// Builds the pair from the owner's secret key and the agent's public key.
    #[must_use]
    pub fn for_owner(owner_secret: &SecretKey, agent: &XOnlyPublicKey) -> Self {
        Self {
            agent: *agent,
            owner: x_only(owner_secret),
            conversation_key: nip44::conversation_key(owner_secret, agent),
        }
    }

    /// Returns the agent's public key.
    #[must_use]
    pub fn agent(&self) -> &XOnlyPublicKey {
        &self.agent
    }

    /// Returns the owner's public key.
    #[must_use]
    pub fn owner(&self) -> &XOnlyPublicKey {
        &self.owner
    }

    /// Returns the NIP-44 conversation key `K_c`.
    #[must_use]
    pub fn conversation_key(&self) -> &[u8; 32] {
        &self.conversation_key
    }

    /// Derives the `d` tag that addresses `slug` for this pair.
    #[must_use]
    pub fn d_tag(&self, slug: &Slug) -> String {
        d_tag(&self.conversation_key, slug)
    }
}

impl fmt::Debug for Pair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Pair")
            .field("agent", &self.agent.to_string())
            .field("owner", &self.owner.to_string())
            .field("conversation_key", &"<redacted>")
            .finish()
    }
}

fn x_only(secret: &SecretKey) -> XOnlyPublicKey {
    Keypair::from_secret_key(&Secp256k1::signing_only(), secret)
        .x_only_public_key()
        .0
}

/// Derives the `d` tag for `slug` under the conversation key `K_c`.
#[must_use]
pub fn d_tag(conversation_key: &[u8; 32], slug: &Slug) -> String {
    let mut message = Vec::with_capacity(D_TAG_DOMAIN.len() + 1 + slug.0.len());
    message.extend_from_slice(D_TAG_DOMAIN);
    message.push(0);
    message.extend_from_slice(slug.0.as_bytes());
    encode_lower_hex(&hmac_sha256(conversation_key, &message))
}

/// A decrypted engram body.
///
/// `extra` holds fields beyond those NIP-AE defines. Readers keep them;
/// they never affect validity.
#[derive(Debug, Clone, PartialEq)]
pub enum Body {
    /// The `core` record: the agent's identity, rules, and goals.
    Core {
        /// The free-form profile text.
        profile: String,
        /// Fields beyond `slug` and `profile`.
        extra: Map<String, Value>,
    },
    /// One memory entry, or its tombstone when `value` is `None`.
    Memory {
        /// The entry's `mem/...` slug.
        slug: Slug,
        /// The entry's text; `None` is a tombstone.
        value: Option<String>,
        /// Fields beyond `slug` and `value`.
        extra: Map<String, Value>,
    },
}

impl Body {
    /// Builds a `core` body.
    #[must_use]
    pub fn core(profile: impl Into<String>) -> Self {
        Self::Core {
            profile: profile.into(),
            extra: Map::new(),
        }
    }

    /// Builds a memory body that holds `value` at `slug`.
    ///
    /// # Errors
    ///
    /// Returns [`EngramError::InvalidSlug`] when `slug` is `core`.
    pub fn memory(slug: Slug, value: impl Into<String>) -> Result<Self, EngramError> {
        Self::memory_value(slug, Some(value.into()))
    }

    /// Builds a tombstone that marks `slug` absent.
    ///
    /// # Errors
    ///
    /// Returns [`EngramError::InvalidSlug`] when `slug` is `core`.
    pub fn tombstone(slug: Slug) -> Result<Self, EngramError> {
        Self::memory_value(slug, None)
    }

    fn memory_value(slug: Slug, value: Option<String>) -> Result<Self, EngramError> {
        if slug.is_core() {
            return Err(EngramError::InvalidSlug(
                "a memory body needs a mem/ slug".to_owned(),
            ));
        }
        Ok(Self::Memory {
            slug,
            value,
            extra: Map::new(),
        })
    }

    /// Adds an extra field, serialized after the defined ones.
    ///
    /// # Errors
    ///
    /// Returns [`EngramError::ReservedField`] when `name` is a member name
    /// the body type defines.
    pub fn with_extra(mut self, name: &str, value: Value) -> Result<Self, EngramError> {
        if self.reserved().contains(&name) {
            return Err(EngramError::ReservedField(name.to_owned()));
        }
        match &mut self {
            Self::Core { extra, .. } | Self::Memory { extra, .. } => {
                extra.insert(name.to_owned(), value);
            }
        }
        Ok(self)
    }

    fn reserved(&self) -> [&'static str; 2] {
        match self {
            Self::Core { .. } => ["slug", "profile"],
            Self::Memory { .. } => ["slug", "value"],
        }
    }

    /// Returns the body's slug.
    #[must_use]
    pub fn slug(&self) -> Slug {
        match self {
            Self::Core { .. } => Slug::core(),
            Self::Memory { slug, .. } => slug.clone(),
        }
    }

    /// Returns whether the body is a memory tombstone.
    #[must_use]
    pub fn is_tombstone(&self) -> bool {
        matches!(self, Self::Memory { value: None, .. })
    }

    /// Returns the extra fields.
    #[must_use]
    pub fn extra(&self) -> &Map<String, Value> {
        match self {
            Self::Core { extra, .. } | Self::Memory { extra, .. } => extra,
        }
    }

    /// Returns the `[[slug]]` references in the body's text: `profile` for
    /// `core`, `value` for a memory.
    #[must_use]
    pub fn links(&self) -> Vec<Slug> {
        match self {
            Self::Core { profile, .. } => wiki_links(profile),
            Self::Memory { value, .. } => value.as_deref().map(wiki_links).unwrap_or_default(),
        }
    }

    /// Serializes the body as compact JSON: `slug` first, then `profile` or
    /// `value`, then the extra fields.
    #[must_use]
    pub fn to_json(&self) -> String {
        let mut out = String::from("{\"slug\":");
        match self {
            Self::Core { profile, extra } => {
                out.push_str(&json_string(CORE_SLUG));
                out.push_str(",\"profile\":");
                out.push_str(&json_string(profile));
                push_extra(&mut out, extra);
            }
            Self::Memory { slug, value, extra } => {
                out.push_str(&json_string(slug.as_str()));
                out.push_str(",\"value\":");
                match value {
                    Some(value) => out.push_str(&json_string(value)),
                    None => out.push_str("null"),
                }
                push_extra(&mut out, extra);
            }
        }
        out.push('}');
        out
    }

    /// Parses a decrypted body under rules 3 to 5, without the `d` check.
    ///
    /// # Errors
    ///
    /// Returns the error of the first rule `text` breaks: not one JSON
    /// object, a repeated member name at any depth, a missing or invalid
    /// slug, or a shape that does not match the slug's type.
    pub fn parse(text: &str) -> Result<Self, EngramError> {
        let mut object = parse_strict_object(text)?;
        let slug = match object.remove("slug") {
            Some(Value::String(slug)) => {
                Slug::parse(&slug).map_err(|error| EngramError::BodySlug(error.to_string()))?
            }
            Some(_) => return Err(EngramError::BodySlug("slug is not a string".to_owned())),
            None => return Err(EngramError::BodySlug("slug is missing".to_owned())),
        };
        if slug.is_core() {
            let profile = match object.remove("profile") {
                Some(Value::String(profile)) => profile,
                Some(_) => return Err(EngramError::Shape("profile is not a string".to_owned())),
                None => return Err(EngramError::Shape("core body has no profile".to_owned())),
            };
            Ok(Self::Core {
                profile,
                extra: object,
            })
        } else {
            let value = match object.remove("value") {
                Some(Value::String(value)) => Some(value),
                Some(Value::Null) => None,
                Some(_) => {
                    return Err(EngramError::Shape(
                        "value is neither a string nor null".to_owned(),
                    ));
                }
                None => return Err(EngramError::Shape("memory body has no value".to_owned())),
            };
            Ok(Self::Memory {
                slug,
                value,
                extra: object,
            })
        }
    }
}

fn json_string(text: &str) -> String {
    Value::String(text.to_owned()).to_string()
}

fn push_extra(out: &mut String, extra: &Map<String, Value>) {
    for (name, value) in extra {
        out.push(',');
        out.push_str(&json_string(name));
        out.push(':');
        out.push_str(&value.to_string());
    }
}

/// Parses `text` as one JSON object, refusing a repeated member name in any
/// object at any depth.
fn parse_strict_object(text: &str) -> Result<Map<String, Value>, EngramError> {
    match serde_json::from_str::<StrictValue>(text) {
        Ok(StrictValue(Value::Object(object))) => Ok(object),
        Ok(_) => Err(EngramError::NotJsonObject(
            "the body is not an object".to_owned(),
        )),
        Err(error) if error.to_string().contains(DUPLICATE_MARKER) => {
            Err(EngramError::DuplicateMember)
        }
        Err(error) => Err(EngramError::NotJsonObject(error.to_string())),
    }
}

/// A JSON value whose objects never repeat a member name.
struct StrictValue(Value);

impl<'de> Deserialize<'de> for StrictValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(StrictVisitor).map(Self)
    }
}

struct StrictVisitor;

impl<'de> Visitor<'de> for StrictVisitor {
    type Value = Value;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a JSON value")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Value, E> {
        Ok(Value::Bool(value))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Value, E> {
        Ok(Value::Number(value.into()))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Value, E> {
        Ok(Value::Number(value.into()))
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Value, E> {
        Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("a JSON number is not finite"))
    }

    fn visit_str<E>(self, value: &str) -> Result<Value, E> {
        Ok(Value::String(value.to_owned()))
    }

    fn visit_string<E>(self, value: String) -> Result<Value, E> {
        Ok(Value::String(value))
    }

    fn visit_unit<E>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
        let mut items = Vec::new();
        while let Some(StrictValue(item)) = seq.next_element()? {
            items.push(item);
        }
        Ok(Value::Array(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        let mut object = Map::new();
        while let Some(name) = map.next_key::<String>()? {
            if object.contains_key(&name) {
                return Err(de::Error::custom(DUPLICATE_MARKER));
            }
            let StrictValue(value) = map.next_value()?;
            object.insert(name, value);
        }
        Ok(Value::Object(object))
    }
}

/// The caller-supplied inputs of one engram event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventParams {
    /// The event's `created_at`; see [`monotonic_created_at`].
    pub created_at: u64,
    /// The 32-byte NIP-44 nonce, drawn from a CSPRNG.
    pub nonce: [u8; 32],
    /// The 32 BIP-340 auxiliary bytes, drawn from a CSPRNG.
    pub aux: [u8; 32],
    /// Whether to add the NIP-31 `["alt", ALT_TEXT]` tag.
    pub alt: bool,
}

/// Encrypts `body` under the pair's `K_c` and signs it with the agent's key.
///
/// The event carries `["d", d]`, `["p", owner]`, and, when
/// [`EventParams::alt`] is set, `["alt", ALT_TEXT]`.
///
/// # Errors
///
/// Returns [`EngramError::BodyTooLarge`] when the serialized body exceeds
/// [`MAX_BODY_BYTES`], or [`EngramError::Encode`] when encryption or
/// serialization fails.
pub fn build_event(
    agent_secret: &SecretKey,
    owner: &XOnlyPublicKey,
    body: &Body,
    params: &EventParams,
) -> Result<Event, EngramError> {
    let pair = Pair::for_agent(agent_secret, owner);
    let plaintext = body.to_json();
    if plaintext.len() > MAX_BODY_BYTES {
        return Err(EngramError::BodyTooLarge(plaintext.len()));
    }
    let content = nip44::encrypt(&plaintext, &pair.conversation_key, params.nonce)
        .map_err(EngramError::Encode)?;
    let mut tags = vec![
        Tag::new(vec!["d".to_owned(), pair.d_tag(&body.slug())]),
        Tag::new(vec!["p".to_owned(), owner.to_string()]),
    ];
    if params.alt {
        tags.push(Tag::new(vec!["alt".to_owned(), ALT_TEXT.to_owned()]));
    }
    sign_event(agent_secret, params.created_at, tags, content, &params.aux)
}

fn sign_event(
    secret: &SecretKey,
    created_at: u64,
    tags: Vec<Tag>,
    content: String,
    aux: &[u8; 32],
) -> Result<Event, EngramError> {
    let secp = Secp256k1::signing_only();
    let keypair = Keypair::from_secret_key(&secp, secret);
    let mut event = Event {
        id: String::new(),
        pubkey: keypair.x_only_public_key().0.to_string(),
        created_at,
        kind: ENGRAM_KIND,
        tags,
        content,
        sig: String::new(),
    };
    let id = event
        .computed_id_bytes()
        .map_err(|error| EngramError::Encode(error.to_string()))?;
    event.id = encode_lower_hex(&id);
    event.sig = secp
        .sign_schnorr_with_aux_rand(&id, &keypair, aux)
        .to_string();
    Ok(event)
}

/// A valid engram: its event's identity and its decrypted body.
#[derive(Debug, Clone, PartialEq)]
pub struct Engram {
    /// The event id, 64 lowercase hex characters.
    pub id: String,
    /// The event's `created_at`.
    pub created_at: u64,
    /// The event's `d` tag.
    pub d: String,
    /// The decrypted body.
    pub body: Body,
}

impl Engram {
    /// Returns the body's slug.
    #[must_use]
    pub fn slug(&self) -> Slug {
        self.body.slug()
    }

    /// Returns whether the body is a memory tombstone.
    #[must_use]
    pub fn is_tombstone(&self) -> bool {
        self.body.is_tombstone()
    }
}

/// Validates `event` for `pair` and returns its decrypted body.
///
/// The rules apply in NIP-AE's order, and the signature verifies before
/// any decryption:
///
/// 1. Kind `30174`, the agent as author, exactly one `d` tag, and exactly
///    one `p` tag that names the owner.
/// 2. The event id and signature verify.
/// 3. The content decrypts under `K_c` to one JSON object that repeats no
///    member name at any depth.
/// 4. The body's slug follows the grammar and re-derives to the `d` tag.
/// 5. The body's shape matches the type its slug names.
///
/// # Errors
///
/// Returns the error of the first rule `event` breaks.
pub fn validate_and_decrypt(event: &Event, pair: &Pair) -> Result<Engram, EngramError> {
    if event.kind != ENGRAM_KIND {
        return Err(EngramError::WrongKind(event.kind));
    }
    if event.pubkey != pair.agent.to_string() {
        return Err(EngramError::WrongAuthor);
    }
    let d = single_tag(event, "d").ok_or(EngramError::DTagCount)?;
    let p = single_tag(event, "p").ok_or(EngramError::PTagCount)?;
    if p != pair.owner.to_string() {
        return Err(EngramError::WrongOwner);
    }
    event
        .validate_crypto()
        .map_err(|_| EngramError::Signature)?;
    let plaintext =
        nip44::decrypt(&event.content, &pair.conversation_key).map_err(EngramError::Decrypt)?;
    if plaintext.len() > MAX_BODY_BYTES {
        return Err(EngramError::Decrypt(format!(
            "the plaintext exceeds {MAX_BODY_BYTES} bytes"
        )));
    }
    let body = Body::parse(&plaintext)?;
    if pair.d_tag(&body.slug()) != d {
        return Err(EngramError::DTagMismatch);
    }
    Ok(Engram {
        id: event.id.clone(),
        created_at: event.created_at,
        d: d.to_owned(),
        body,
    })
}

fn single_tag<'a>(event: &'a Event, name: &str) -> Option<&'a str> {
    let mut values = event.tags.iter().filter(|tag| tag.name() == Some(name));
    let first = values.next()?;
    if values.next().is_some() {
        return None;
    }
    first.value()
}

/// Selects the head among valid engrams that share one `d` tag: the
/// greatest `created_at`, with ties going to the lowest event id.
#[must_use]
pub fn select_head<'a, I>(engrams: I) -> Option<&'a Engram>
where
    I: IntoIterator<Item = &'a Engram>,
{
    engrams.into_iter().reduce(|head, candidate| {
        if precedes(candidate, head) {
            candidate
        } else {
            head
        }
    })
}

fn precedes(candidate: &Engram, head: &Engram) -> bool {
    candidate.created_at > head.created_at
        || (candidate.created_at == head.created_at && candidate.id < head.id)
}

/// Returns the `created_at` for a write over a head created at `head`:
/// `max(now, head + 1)`, or `now` when no head exists.
///
/// # Errors
///
/// Returns [`EngramError::ClockPoisoned`] when the result lies more than
/// [`CLOCK_POISON_SECS`] after `now`. Surface that as a conflict rather than
/// publish it.
pub fn monotonic_created_at(now: u64, head: Option<u64>) -> Result<u64, EngramError> {
    monotonic_created_at_within(now, head, CLOCK_POISON_SECS)
}

/// Same as [`monotonic_created_at`], with the poison threshold `max_ahead`
/// in seconds.
///
/// # Errors
///
/// Returns [`EngramError::ClockPoisoned`] when the result lies more than
/// `max_ahead` seconds after `now`.
pub fn monotonic_created_at_within(
    now: u64,
    head: Option<u64>,
    max_ahead: u64,
) -> Result<u64, EngramError> {
    let Some(head) = head else {
        return Ok(now);
    };
    let created_at = now.max(head.saturating_add(1));
    if created_at - now > max_ahead || head == u64::MAX {
        return Err(EngramError::ClockPoisoned { head, now });
    }
    Ok(created_at)
}

/// One live memory entry in a listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadEntry {
    /// The entry's slug.
    pub slug: Slug,
    /// The head event's id.
    pub event_id: String,
    /// The head event's `created_at`.
    pub created_at: u64,
}

/// Lists the live memory entries among valid engrams.
///
/// The engrams group by `d` tag, each group keeps its head, tombstones drop
/// out, and `core` is omitted. The entries come back sorted by slug.
#[must_use]
pub fn list_heads(engrams: &[Engram]) -> Vec<HeadEntry> {
    let mut heads: BTreeMap<&str, &Engram> = BTreeMap::new();
    for engram in engrams {
        heads
            .entry(engram.d.as_str())
            .and_modify(|head| {
                if precedes(engram, head) {
                    *head = engram;
                }
            })
            .or_insert(engram);
    }
    let mut entries = heads
        .into_values()
        .filter(|head| !head.is_tombstone())
        .filter_map(|head| match &head.body {
            Body::Memory { slug, .. } => Some(HeadEntry {
                slug: slug.clone(),
                event_id: head.id.clone(),
                created_at: head.created_at,
            }),
            Body::Core { .. } => None,
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| left.slug.cmp(&right.slug));
    entries
}

/// Extracts the `[[slug]]` references from `text`, in order of first
/// appearance and without repeats.
///
/// A reference is a literal `[[`, the nearest following `]]`, and a valid
/// slug between them. Bare slugs without brackets are not references.
#[must_use]
pub fn wiki_links(text: &str) -> Vec<Slug> {
    let mut links: Vec<Slug> = Vec::new();
    let mut search = 0;
    while let Some(offset) = text[search..].find("[[") {
        let open = search + offset;
        let inner_start = open + 2;
        if let Some(length) = text[inner_start..].find("]]") {
            if let Ok(slug) = Slug::parse(&text[inner_start..inner_start + length])
                && !links.contains(&slug)
            {
                links.push(slug);
            }
        } else {
            break;
        }
        // Step one byte past the first bracket so `[[[mem/a]]` still finds
        // `[[mem/a]]`; `[` is ASCII, so the next index is a char boundary.
        search = open + 1;
    }
    links
}

#[cfg(test)]
#[path = "engram_tests.rs"]
mod tests;
