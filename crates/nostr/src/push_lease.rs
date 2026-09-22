//! NIP-PL push leases.
//!
//! A lease authorizes a wake. The wake body is the fixed APNs reconnect
//! constant and never carries event content, ids, or ciphertext. FCM and
//! UnifiedPush stay refused until a profile registers its own constant.

use secp256k1::{SecretKey, XOnlyPublicKey};

use crate::domain::{Event, Filter};
use crate::nip44::{conversation_key, decrypt};

/// The only conforming v1 application body.
pub const APNS_BODY: &str =
    r#"{"aps":{"alert":{"body":"Reconnect to your relay now"},"mutable-content":1}}"#;
/// Default maximum lease lifetime, in seconds.
pub const DEFAULT_MAX_LEASE_TTL: u64 = 2_592_000;
/// Clock skew allowed when checking expiration.
pub const ALLOWED_SKEW: u64 = 900;

/// Advertised executor limits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaseLimits {
    /// Maximum `expiration - now`.
    pub max_lease_ttl: u64,
    /// Active leases per pubkey at this origin.
    pub max_leases_per_pubkey: usize,
    /// Subscriptions in one lease.
    pub max_subscriptions_per_lease: usize,
    /// Kinds in one filter.
    pub max_kinds: usize,
    /// Authors in one filter.
    pub max_authors: usize,
    /// Channel values in one filter.
    pub max_h: usize,
    /// Values on one `#p` or `#e` selector.
    pub max_tag_values: usize,
    /// Ignore filters on one subscription.
    pub max_ignore: usize,
    /// Ciphertext byte limit.
    pub max_content_len: usize,
    /// Plaintext byte limit.
    pub max_plaintext_len: usize,
    /// Endpoint byte limit.
    pub max_endpoint_len: usize,
    /// Other string byte limit.
    pub max_string_len: usize,
}

impl Default for LeaseLimits {
    fn default() -> Self {
        Self {
            max_lease_ttl: DEFAULT_MAX_LEASE_TTL,
            max_leases_per_pubkey: 16,
            max_subscriptions_per_lease: 16,
            max_kinds: 16,
            max_authors: 20,
            max_h: 50,
            max_tag_values: 20,
            max_ignore: 8,
            max_content_len: 65_536,
            max_plaintext_len: 32_768,
            max_endpoint_len: 4_096,
            max_string_len: 512,
        }
    }
}

/// The descriptor a configured executor advertises.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PushDescriptor {
    /// Canonical origin copied into lease plaintext.
    pub origin: String,
    /// Current encryption key id.
    pub key_id: String,
    /// Current encryption pubkey, 64 lowercase hex characters.
    pub pubkey: String,
    /// Application profile id.
    pub app_profile: String,
    /// Transport of that profile. Only `apns` is conforming.
    pub transport: String,
    /// Kinds a lease may name.
    pub push_kinds: Vec<u16>,
    /// Limits.
    pub limits: LeaseLimits,
}

/// One accepted lease. Inactive leases do not match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedLease {
    /// Lease author.
    pub author: String,
    /// Installation id from the `d` tag.
    pub installation: String,
    /// Event id of the accepted replacement.
    pub event_id: String,
    /// NIP-01 `created_at`.
    pub created_at: u64,
    /// Public expiration.
    pub expiration: u64,
    /// Plaintext generation.
    pub generation: u64,
    /// False is a revocation tombstone.
    pub active: bool,
    /// Present on an active lease.
    pub endpoint: Option<String>,
    /// Subscriptions that may wake the installation.
    pub subscriptions: Vec<LeaseSubscription>,
}

/// A narrowed filter and its class.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaseSubscription {
    /// Filter the executor matches.
    pub filter: Filter,
    /// Ignore filters. Extra wakes remain correct if these are skipped.
    pub ignore: Vec<Filter>,
    /// Drop a match with more `p` tags than this.
    pub p_tags_max: Option<u64>,
}

/// Decrypt lease content addressed to `secret` by `author`.
///
/// # Errors
///
/// Returns an error when the payload is not NIP-44 ciphertext for this pair.
pub fn open_lease(
    payload: &str,
    secret: &SecretKey,
    author: &XOnlyPublicKey,
) -> Result<String, String> {
    decrypt(payload, &conversation_key(secret, author))
}

/// The application body for a transport. It does not read the event.
///
/// # Errors
///
/// Returns an error for a transport without a registered constant.
pub fn application_body(transport: &str) -> Result<&'static str, &'static str> {
    match transport {
        "apns" => Ok(APNS_BODY),
        "fcm" | "unifiedpush" => Err("transport has no registered reconnect constant"),
        _ => Err("unknown transport"),
    }
}

/// Validate a descriptor this process is willing to advertise.
///
/// # Errors
///
/// Returns a reason when a required constant or key is missing.
pub fn validate_descriptor(descriptor: &PushDescriptor) -> Result<(), &'static str> {
    if descriptor.origin.is_empty() || descriptor.key_id.is_empty() {
        return Err("descriptor");
    }
    if XOnlyPublicKey::from_byte_array(decode_hex32(&descriptor.pubkey).map_err(|_| "key")?)
        .is_err()
    {
        return Err("key");
    }
    if descriptor.transport != "apns" || descriptor.app_profile.is_empty() {
        return Err("transport");
    }
    if descriptor.push_kinds.is_empty()
        || descriptor.push_kinds.iter().any(|kind| {
            (20_000..30_000).contains(kind)
                || matches!(*kind, 39_005 | 39_006 | 30_622 | 8_002 | 8_003 | 13_535)
        })
    {
        return Err("push_kinds");
    }
    Ok(())
}

/// NIP-11 `push` object for a valid descriptor.
#[must_use]
pub fn descriptor_document(descriptor: &PushDescriptor) -> serde_json::Value {
    serde_json::json!({
        "origin": descriptor.origin,
        "keys": [{ "id": descriptor.key_id, "pubkey": descriptor.pubkey, "current": true }],
        "app_profiles": [{ "id": descriptor.app_profile, "transport": descriptor.transport }],
        "push_kinds": descriptor.push_kinds,
        "urgent_kinds": [],
        "h_grammar": "uuid-v4-lowercase",
        "class_support": { "apns": ["silent", "default", "time_sensitive"] },
        "limitation": {
            "max_lease_ttl": descriptor.limits.max_lease_ttl,
            "max_leases_per_pubkey": descriptor.limits.max_leases_per_pubkey,
            "max_subscriptions_per_lease": descriptor.limits.max_subscriptions_per_lease,
            "max_kinds": descriptor.limits.max_kinds,
            "max_authors": descriptor.limits.max_authors,
            "max_h": descriptor.limits.max_h,
            "max_tag_values": descriptor.limits.max_tag_values,
            "max_ignore": descriptor.limits.max_ignore,
            "max_content_len": descriptor.limits.max_content_len,
            "max_plaintext_len": descriptor.limits.max_plaintext_len,
            "max_endpoint_len": descriptor.limits.max_endpoint_len,
            "max_string_len": descriptor.limits.max_string_len,
        }
    })
}

/// Accept plaintext that was decrypted under the descriptor's current key.
///
/// `previous` is the stored lease at the same address. `other_endpoints`
/// and `active_others` describe the author's other active leases.
///
/// # Errors
///
/// Returns the specification's `invalid:` reason without that prefix.
pub fn accept_lease(
    event: &Event,
    plaintext: &str,
    now: u64,
    descriptor: &PushDescriptor,
    previous: Option<&AcceptedLease>,
    other_endpoints: &[String],
    active_others: usize,
) -> Result<AcceptedLease, String> {
    validate_descriptor(descriptor).map_err(|reason| reason.to_owned())?;
    if event.kind != 30_350 {
        return Err("lease has the wrong kind".to_owned());
    }
    if event.tag_values("exec").next() != Some(descriptor.key_id.as_str()) {
        return Err("unknown executor key".to_owned());
    }
    let expiration = event
        .expiration()
        .ok_or_else(|| "lease expiration is required".to_owned())?;
    if expiration <= now.saturating_sub(ALLOWED_SKEW) {
        return Err("lease already expired".to_owned());
    }
    if expiration > now.saturating_add(descriptor.limits.max_lease_ttl) {
        return Err("lease ttl too long".to_owned());
    }
    if plaintext.len() > descriptor.limits.max_plaintext_len {
        return Err("plaintext too large".to_owned());
    }
    reject_duplicate_keys(plaintext)?;
    let value: serde_json::Value =
        serde_json::from_str(plaintext).map_err(|_| "lease plaintext is not JSON".to_owned())?;
    let object = value
        .as_object()
        .ok_or_else(|| "lease plaintext is not an object".to_owned())?;
    let active = object
        .get("active")
        .and_then(serde_json::Value::as_bool)
        .ok_or_else(|| "active must be a boolean".to_owned())?;
    let allowed: &[&str] = if active {
        &[
            "v",
            "origin",
            "app_profile",
            "transport",
            "endpoint",
            "generation",
            "active",
            "subscriptions",
        ]
    } else {
        &["v", "origin", "generation", "active"]
    };
    if object.len() != allowed.len() || object.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err("unknown field".to_owned());
    }
    if object.get("v").and_then(serde_json::Value::as_u64) != Some(1) {
        return Err("unsupported lease version".to_owned());
    }
    let origin = text_field(object, "origin", descriptor.limits.max_string_len)?;
    if origin != descriptor.origin {
        return Err("origin mismatch".to_owned());
    }
    let generation = object
        .get("generation")
        .and_then(serde_json::Value::as_u64)
        .filter(|value| *value >= 1)
        .ok_or_else(|| "generation must be a positive integer".to_owned())?;
    if let Some(previous) = previous {
        if !replaces(event, previous) {
            return Err("stale replacement".to_owned());
        }
        if generation <= previous.generation {
            return Err("stale generation".to_owned());
        }
    }
    let (endpoint, subscriptions) = if active {
        let profile = text_field(object, "app_profile", descriptor.limits.max_string_len)?;
        let transport = text_field(object, "transport", descriptor.limits.max_string_len)?;
        if profile != descriptor.app_profile || transport != descriptor.transport {
            return Err("transport mismatch".to_owned());
        }
        let endpoint = text_field(object, "endpoint", descriptor.limits.max_endpoint_len)?;
        if endpoint.is_empty() {
            return Err("endpoint is empty".to_owned());
        }
        if other_endpoints.iter().any(|existing| existing == &endpoint) {
            return Err("endpoint already leased".to_owned());
        }
        if active_others >= descriptor.limits.max_leases_per_pubkey {
            return Err("lease quota exceeded".to_owned());
        }
        let subscriptions = parse_subscriptions(
            object
                .get("subscriptions")
                .ok_or_else(|| "subscriptions are required".to_owned())?,
            &event.pubkey,
            descriptor,
        )?;
        (Some(endpoint), subscriptions)
    } else {
        (None, Vec::new())
    };
    Ok(AcceptedLease {
        author: event.pubkey.clone(),
        installation: event.tag_values("d").next().unwrap_or_default().to_owned(),
        event_id: event.id.clone(),
        created_at: event.created_at,
        expiration,
        generation,
        active,
        endpoint,
        subscriptions,
    })
}

/// Whether an active unexpired lease matches `event` for its author.
#[must_use]
pub fn lease_matches(lease: &AcceptedLease, event: &Event, now: u64) -> bool {
    if !lease.active || lease.expiration <= now || event.kind == 30_350 {
        return false;
    }
    lease.subscriptions.iter().any(|subscription| {
        subscription.filter.matches(event)
            && subscription
                .ignore
                .iter()
                .all(|filter| !filter.matches(event))
            && subscription.p_tags_max.is_none_or(|limit| {
                event.tag_values("p").count() <= usize::try_from(limit).unwrap_or(usize::MAX)
            })
    })
}

/// Kinds the executor may wake for, using the same privacy rule as a read.
#[must_use]
pub fn author_may_read(event: &Event, author: &str) -> bool {
    match event.kind {
        1_059 => {
            let recipients = event.tag_values("p").collect::<Vec<_>>();
            recipients.len() == 1 && recipients[0] == author
        }
        30_174 => event.pubkey == author || event.tag_values("p").any(|owner| owner == author),
        30_300 | 30_350 => event.pubkey == author,
        _ => true,
    }
}

fn replaces(incoming: &Event, previous: &AcceptedLease) -> bool {
    incoming.created_at > previous.created_at
        || (incoming.created_at == previous.created_at
            && incoming.id.as_str() < previous.event_id.as_str())
}

fn parse_subscriptions(
    value: &serde_json::Value,
    author: &str,
    descriptor: &PushDescriptor,
) -> Result<Vec<LeaseSubscription>, String> {
    let items = value
        .as_array()
        .ok_or_else(|| "subscriptions must be an array".to_owned())?;
    if items.is_empty() || items.len() > descriptor.limits.max_subscriptions_per_lease {
        return Err("subscriptions are outside the advertised bound".to_owned());
    }
    items
        .iter()
        .map(|item| parse_subscription(item, author, descriptor))
        .collect()
}

fn parse_subscription(
    value: &serde_json::Value,
    author: &str,
    descriptor: &PushDescriptor,
) -> Result<LeaseSubscription, String> {
    let object = value
        .as_object()
        .ok_or_else(|| "subscription must be an object".to_owned())?;
    if object
        .keys()
        .any(|key| !matches!(key.as_str(), "filter" | "class" | "ignore" | "suppress"))
        || !object.contains_key("filter")
        || !object.contains_key("class")
    {
        return Err("unknown field".to_owned());
    }
    let class = object
        .get("class")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "class must be a string".to_owned())?;
    if !matches!(class, "silent" | "default" | "time_sensitive") {
        return Err("class not supported".to_owned());
    }
    let filter = parse_filter(
        object.get("filter").expect("filter was checked"),
        author,
        descriptor,
        true,
    )?;
    let ignore = match object.get("ignore") {
        None => Vec::new(),
        Some(value) => {
            let items = value
                .as_array()
                .ok_or_else(|| "ignore must be an array".to_owned())?;
            if items.len() > descriptor.limits.max_ignore {
                return Err("ignore exceeds the advertised bound".to_owned());
            }
            items
                .iter()
                .map(|item| parse_filter(item, author, descriptor, false))
                .collect::<Result<Vec<_>, _>>()?
        }
    };
    let p_tags_max = match object.get("suppress") {
        None => None,
        Some(value) => {
            let suppress = value
                .as_object()
                .ok_or_else(|| "suppress must be an object".to_owned())?;
            if suppress.len() != 1 || !suppress.contains_key("p_tags_max") {
                return Err("unknown field".to_owned());
            }
            Some(
                suppress
                    .get("p_tags_max")
                    .and_then(serde_json::Value::as_u64)
                    .filter(|value| *value >= 1)
                    .ok_or_else(|| "p_tags_max must be a positive integer".to_owned())?,
            )
        }
    };
    Ok(LeaseSubscription {
        filter,
        ignore,
        p_tags_max,
    })
}

fn parse_filter(
    value: &serde_json::Value,
    author: &str,
    descriptor: &PushDescriptor,
    narrowed: bool,
) -> Result<Filter, String> {
    let object = value
        .as_object()
        .ok_or_else(|| "lease filter must be an object".to_owned())?;
    for key in object.keys() {
        if !matches!(key.as_str(), "kinds" | "authors" | "#p" | "#h" | "#e") {
            return Err("lease filter contains a forbidden member".to_owned());
        }
    }
    let kinds = number_list(object.get("kinds"), descriptor.limits.max_kinds, "kinds")?;
    if kinds.is_empty()
        || kinds
            .iter()
            .any(|kind| !descriptor.push_kinds.contains(kind))
    {
        return Err("kind not push-eligible".to_owned());
    }
    let authors = hex_list(
        object.get("authors"),
        descriptor.limits.max_authors,
        64,
        "authors",
    )?;
    let mentions = hex_list(object.get("#p"), descriptor.limits.max_tag_values, 64, "p")?;
    let threads = hex_list(object.get("#e"), descriptor.limits.max_tag_values, 64, "e")?;
    let channels = string_list(object.get("#h"), descriptor.limits.max_h, descriptor)?;
    if mentions.iter().any(|value| value != author) {
        return Err("p-tag must be self".to_owned());
    }
    if narrowed && mentions.is_empty() && channels.is_empty() && authors.is_empty() {
        return Err("lease filter not narrowed".to_owned());
    }
    let mut raw = serde_json::json!({ "kinds": kinds });
    let map = raw.as_object_mut().expect("object");
    if !authors.is_empty() {
        map.insert("authors".into(), serde_json::json!(authors));
    }
    if !mentions.is_empty() {
        map.insert("#p".into(), serde_json::json!(mentions));
    }
    if !channels.is_empty() {
        map.insert("#h".into(), serde_json::json!(channels));
    }
    if !threads.is_empty() {
        map.insert("#e".into(), serde_json::json!(threads));
    }
    serde_json::from_value(raw).map_err(|_| "lease filter is not a NIP-01 filter".to_owned())
}

fn text_field(
    object: &serde_json::Map<String, serde_json::Value>,
    key: &str,
    max_bytes: usize,
) -> Result<String, String> {
    let value = object
        .get(key)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("{key} must be a string"))?;
    if value.len() > max_bytes {
        return Err(format!("{key} exceeds the advertised bound"));
    }
    Ok(value.to_owned())
}

fn number_list(
    value: Option<&serde_json::Value>,
    max: usize,
    name: &str,
) -> Result<Vec<u16>, String> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let items = value
        .as_array()
        .ok_or_else(|| format!("{name} must be an array"))?;
    if items.len() > max {
        return Err(format!("{name} exceeds the advertised bound"));
    }
    items
        .iter()
        .map(|item| {
            item.as_u64()
                .and_then(|number| u16::try_from(number).ok())
                .ok_or_else(|| format!("{name} contains a value outside u16"))
        })
        .collect()
}

fn hex_list(
    value: Option<&serde_json::Value>,
    max: usize,
    len: usize,
    name: &str,
) -> Result<Vec<String>, String> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let items = value
        .as_array()
        .ok_or_else(|| format!("{name} must be an array"))?;
    if items.is_empty() || items.len() > max {
        return Err("non-exact match value".to_owned());
    }
    items
        .iter()
        .map(|item| {
            let text = item
                .as_str()
                .ok_or_else(|| "non-exact match value".to_owned())?;
            if text.len() == len
                && text
                    .bytes()
                    .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
            {
                Ok(text.to_owned())
            } else {
                Err("non-exact match value".to_owned())
            }
        })
        .collect()
}

fn string_list(
    value: Option<&serde_json::Value>,
    max: usize,
    descriptor: &PushDescriptor,
) -> Result<Vec<String>, String> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let items = value
        .as_array()
        .ok_or_else(|| "h must be an array".to_owned())?;
    if items.is_empty() || items.len() > max {
        return Err("h exceeds the advertised bound".to_owned());
    }
    items
        .iter()
        .map(|item| {
            let text = item
                .as_str()
                .ok_or_else(|| "h must be a string".to_owned())?;
            if text.len() > descriptor.limits.max_string_len || !uuid_v4(text) {
                return Err("h value fails uuid-v4-lowercase".to_owned());
            }
            Ok(text.to_owned())
        })
        .collect()
}

fn uuid_v4(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 36 {
        return false;
    }
    let hex = |index: usize| matches!(bytes[index], b'0'..=b'9' | b'a'..=b'f');
    (0..8).all(hex)
        && bytes[8] == b'-'
        && (9..13).all(hex)
        && bytes[13] == b'-'
        && bytes[14] == b'4'
        && (15..18).all(hex)
        && bytes[18] == b'-'
        && matches!(bytes[19], b'8' | b'9' | b'a' | b'b')
        && (20..23).all(hex)
        && bytes[23] == b'-'
        && (24..36).all(hex)
}

fn decode_hex32(value: &str) -> Result<[u8; 32], ()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Err(());
    }
    let mut out = [0_u8; 32];
    for (index, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).map_err(|_| ())?;
    }
    Ok(out)
}

fn reject_duplicate_keys(input: &str) -> Result<(), String> {
    let mut scan = Scan {
        bytes: input.as_bytes(),
        index: 0,
    };
    scan.value()
        .and_then(|_| {
            scan.skip_ws();
            if scan.index == scan.bytes.len() {
                Ok(())
            } else {
                Err(())
            }
        })
        .map_err(|()| "duplicate key".to_owned())
}

struct Scan<'a> {
    bytes: &'a [u8],
    index: usize,
}

impl Scan<'_> {
    fn value(&mut self) -> Result<(), ()> {
        self.skip_ws();
        match self.peek().ok_or(())? {
            b'{' => self.object(),
            b'[' => self.array(),
            b'"' => self.string().map(|_| ()),
            b't' => self.word(b"true"),
            b'f' => self.word(b"false"),
            b'n' => self.word(b"null"),
            b'-' | b'0'..=b'9' => self.number(),
            _ => Err(()),
        }
    }

    fn object(&mut self) -> Result<(), ()> {
        self.bump(b'{')?;
        let mut keys = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.index += 1;
            return Ok(());
        }
        loop {
            self.skip_ws();
            let key = self.string()?;
            if keys.iter().any(|existing: &String| existing == &key) {
                return Err(());
            }
            keys.push(key);
            self.skip_ws();
            self.bump(b':')?;
            self.value()?;
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.index += 1,
                Some(b'}') => {
                    self.index += 1;
                    return Ok(());
                }
                _ => return Err(()),
            }
        }
    }

    fn array(&mut self) -> Result<(), ()> {
        self.bump(b'[')?;
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.index += 1;
            return Ok(());
        }
        loop {
            self.value()?;
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.index += 1,
                Some(b']') => {
                    self.index += 1;
                    return Ok(());
                }
                _ => return Err(()),
            }
        }
    }

    fn string(&mut self) -> Result<String, ()> {
        self.bump(b'"')?;
        let mut out = String::new();
        while let Some(byte) = self.peek() {
            self.index += 1;
            match byte {
                b'"' => return Ok(out),
                b'\\' => {
                    let escaped = self.peek().ok_or(())?;
                    self.index += 1;
                    match escaped {
                        b'"' | b'\\' | b'/' => out.push(char::from(escaped)),
                        b'n' => out.push('\n'),
                        b't' => out.push('\t'),
                        _ => return Err(()),
                    }
                }
                byte if byte < 0x20 => return Err(()),
                byte => out.push(char::from(byte)),
            }
        }
        Err(())
    }

    fn word(&mut self, expected: &[u8]) -> Result<(), ()> {
        for byte in expected {
            self.bump(*byte)?;
        }
        Ok(())
    }

    fn number(&mut self) -> Result<(), ()> {
        if self.peek() == Some(b'-') {
            self.index += 1;
        }
        let start = self.index;
        while self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
            self.index += 1;
        }
        (self.index > start).then_some(()).ok_or(())
    }

    fn skip_ws(&mut self) {
        while self.peek().is_some_and(|byte| byte.is_ascii_whitespace()) {
            self.index += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.index).copied()
    }

    fn bump(&mut self, expected: u8) -> Result<(), ()> {
        if self.peek() == Some(expected) {
            self.index += 1;
            Ok(())
        } else {
            Err(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{RelaySigner, Tag};
    use crate::nip44::encrypt;
    use serde_json::json;

    fn signer(byte: u8) -> (RelaySigner, SecretKey) {
        let secret = SecretKey::from_byte_array([byte; 32]).unwrap();
        let hex: String = [byte; 32]
            .iter()
            .map(|item| format!("{item:02x}"))
            .collect();
        (RelaySigner::from_secret_hex(&hex).unwrap(), secret)
    }

    fn descriptor(pubkey: &str) -> PushDescriptor {
        PushDescriptor {
            origin: "ws://127.0.0.1:7447".into(),
            key_id: "current".into(),
            pubkey: pubkey.to_owned(),
            app_profile: "com.openagents.relay/ios".into(),
            transport: "apns".into(),
            push_kinds: vec![1, 7, 9],
            limits: LeaseLimits::default(),
        }
    }

    #[test]
    fn an_accepted_lease_wakes_with_the_fixed_reconnect_constant() {
        let (author, author_secret) = signer(0x41);
        let (executor, executor_secret) = signer(0x42);
        let descriptor = descriptor(executor.pubkey());
        let author_key =
            secp256k1::XOnlyPublicKey::from_byte_array(decode_hex32(author.pubkey()).unwrap())
                .unwrap();
        let executor_key =
            secp256k1::XOnlyPublicKey::from_byte_array(decode_hex32(executor.pubkey()).unwrap())
                .unwrap();
        let plaintext = json!({
            "v": 1,
            "origin": descriptor.origin,
            "app_profile": descriptor.app_profile,
            "transport": "apns",
            "endpoint": "abc123",
            "generation": 1,
            "active": true,
            "subscriptions": [{
                "filter": {"kinds": [1], "#p": [author.pubkey()]},
                "class": "default"
            }]
        })
        .to_string();
        let ciphertext = encrypt(
            &plaintext,
            &conversation_key(&author_secret, &executor_key),
            [9_u8; 32],
        )
        .unwrap();
        let opened = open_lease(&ciphertext, &executor_secret, &author_key).unwrap();
        assert_eq!(opened, plaintext);
        let lease_event = author.sign(
            1_000,
            30_350,
            vec![
                Tag::new(vec!["d".into(), "installation-1".into()]),
                Tag::new(vec!["expiration".into(), "200000".into()]),
                Tag::new(vec!["exec".into(), "current".into()]),
            ],
            ciphertext.clone(),
        );
        let accepted =
            accept_lease(&lease_event, &opened, 1_100, &descriptor, None, &[], 0).unwrap();
        let message = author.sign(
            1_200,
            1,
            vec![Tag::new(vec!["p".into(), author.pubkey().to_owned()])],
            "hello".into(),
        );
        assert!(lease_matches(&accepted, &message, 1_200));
        assert!(author_may_read(&message, author.pubkey()));
        let body = application_body("apns").unwrap();
        assert_eq!(body, APNS_BODY);
        assert!(!body.contains(&message.id));
        assert!(application_body("fcm").is_err());
        let newer = author.sign(
            1_300,
            30_350,
            vec![
                Tag::new(vec!["d".into(), "installation-1".into()]),
                Tag::new(vec!["expiration".into(), "200000".into()]),
                Tag::new(vec!["exec".into(), "current".into()]),
            ],
            ciphertext.clone(),
        );
        let stale = accept_lease(&newer, &opened, 1_300, &descriptor, Some(&accepted), &[], 0);
        assert_eq!(stale.unwrap_err(), "stale generation");
    }
}
