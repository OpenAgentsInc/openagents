use secp256k1::{Keypair, Secp256k1, SecretKey};
use sha2::{Digest, Sha256};

use super::hex::{decode_lower_hex, encode_lower_hex};
use super::{BLOCK_GLOBAL_ONLY_KINDS, DomainError, Event, Tag};

const HTTP_AUTH_KIND: u16 = 27_235;
const HTTP_AUTH_WINDOW_SECONDS: u64 = 60;
const MAX_GROUP_ID_BYTES: usize = 128;

#[derive(Clone)]
pub struct RelaySigner {
    keypair: Keypair,
    pubkey: String,
}

impl RelaySigner {
    pub fn from_secret_hex(value: &str) -> Result<Self, DomainError> {
        let secret = SecretKey::from_byte_array(decode_lower_hex::<32>(value, "relay secret key")?)
            .map_err(|_| DomainError::InvalidEvent("invalid relay secret key".to_owned()))?;
        let secp = Secp256k1::new();
        let keypair = Keypair::from_secret_key(&secp, &secret);
        let pubkey = keypair.x_only_public_key().0.to_string();
        Ok(Self { keypair, pubkey })
    }

    pub fn pubkey(&self) -> &str {
        &self.pubkey
    }

    pub fn sign(&self, created_at: u64, kind: u16, tags: Vec<Tag>, content: String) -> Event {
        let mut event = Event {
            id: "0".repeat(64),
            pubkey: self.pubkey.clone(),
            created_at,
            kind,
            tags,
            content,
            sig: "0".repeat(128),
        };
        let id = event
            .computed_id_bytes()
            .expect("serializing owned relay metadata cannot fail");
        event.id = encode_lower_hex(&id);
        event.sig = Secp256k1::signing_only()
            .sign_schnorr_no_aux_rand(&id, &self.keypair)
            .to_string();
        event
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpAuth {
    pub event_id: String,
    pub pubkey: String,
}

pub fn parse_http_authorization(
    header: &str,
    method: &str,
    absolute_url: &str,
    payload: &[u8],
    now: u64,
) -> Result<HttpAuth, DomainError> {
    let payload_hash = encode_lower_hex(&Sha256::digest(payload));
    parse_http_authorization_hash(header, method, absolute_url, Some(&payload_hash), now)
}

pub fn parse_http_authorization_hash(
    header: &str,
    method: &str,
    absolute_url: &str,
    payload_hash: Option<&str>,
    now: u64,
) -> Result<HttpAuth, DomainError> {
    let claim = parse_http_authorization_claim(header, method, absolute_url, now)?;
    if claim.payload_hash.as_deref() != payload_hash {
        return Err(DomainError::InvalidEvent(if payload_hash.is_some() {
            "HTTP authorization payload tag does not match".into()
        } else {
            "HTTP authorization payload tag is not allowed without a payload".into()
        }));
    }
    Ok(claim.auth)
}

/// A NIP-98 authorization verified against everything but the body: the
/// signature, kind, timestamp, URL, and method. `payload_hash` is what the
/// event claims the body's SHA-256 is, for the caller to hold the body to
/// once it has read it. A server uses this to refuse a request before it
/// accepts a single body byte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpAuthClaim {
    pub auth: HttpAuth,
    pub payload_hash: Option<String>,
}

pub fn parse_http_authorization_claim(
    header: &str,
    method: &str,
    absolute_url: &str,
    now: u64,
) -> Result<HttpAuthClaim, DomainError> {
    let encoded = header.strip_prefix("Nostr ").ok_or_else(|| {
        DomainError::InvalidEvent("HTTP authorization scheme must be Nostr".into())
    })?;
    let decoded = decode_base64(encoded)?;
    let event = serde_json::from_slice::<Event>(&decoded)
        .map_err(|_| DomainError::InvalidEvent("HTTP authorization is not an event".into()))?;
    event.validate_structure()?;
    event.validate_crypto()?;
    if event.kind != HTTP_AUTH_KIND {
        return Err(DomainError::InvalidEvent(
            "HTTP authorization event must have kind 27235".into(),
        ));
    }
    if event.created_at.abs_diff(now) > HTTP_AUTH_WINDOW_SECONDS {
        return Err(DomainError::InvalidEvent(
            "HTTP authorization timestamp is outside 60 seconds".into(),
        ));
    }
    require_single_tag(&event, "u", absolute_url)?;
    require_single_tag(&event, "method", method)?;
    let payloads = event.tag_values("payload").collect::<Vec<_>>();
    let payload_hash = match payloads.as_slice() {
        [] => None,
        [hash]
            if hash.len() == 64 && hash.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) =>
        {
            Some((*hash).to_owned())
        }
        _ => {
            return Err(DomainError::InvalidEvent(
                "HTTP authorization payload tag must be one lowercase SHA-256".into(),
            ));
        }
    };
    Ok(HttpAuthClaim {
        auth: HttpAuth {
            event_id: event.id,
            pubkey: event.pubkey,
        },
        payload_hash,
    })
}

fn require_single_tag(event: &Event, name: &str, expected: &str) -> Result<(), DomainError> {
    let values = event.tag_values(name).collect::<Vec<_>>();
    if values.as_slice() == [expected] {
        Ok(())
    } else {
        Err(DomainError::InvalidEvent(format!(
            "HTTP authorization {name} tag does not match"
        )))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupMetadata {
    pub name: String,
    pub about: String,
    pub picture: String,
    pub banner: String,
    pub closed: bool,
    /// Only members can read timeline events. Absent on the event, anyone can read.
    pub private: bool,
    /// Hide kinds 39000–39005 from non-members. Absent on the event, metadata is public.
    pub hidden: bool,
    /// Only members can write. Absent on the event, anyone can write.
    pub restricted: bool,
    pub livekit: String,
    /// The parent group's `d` identifier, when this group is a subgroup.
    pub parent: Option<String>,
    /// Ordered child group ids. A metadata edit replaces this list only when
    /// it names the same children.
    pub children: Vec<String>,
    pub supported_kinds: Option<Vec<u16>>,
}

impl Default for GroupMetadata {
    fn default() -> Self {
        Self {
            name: String::new(),
            about: String::new(),
            picture: String::new(),
            banner: String::new(),
            closed: false,
            private: false,
            hidden: false,
            restricted: true,
            livekit: String::new(),
            parent: None,
            children: Vec::new(),
            supported_kinds: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupAction {
    PutUser { pubkey: String, roles: Vec<String> },
    RemoveUser { pubkey: String },
    EditMetadata(GroupMetadata),
    DeleteEvent { event_id: String },
    CreateGroup,
    DeleteGroup,
    CreateInvite { code: String },
    UpdatePins { tags: Vec<Tag> },
    Join { code: Option<String> },
    Leave,
}

impl GroupAction {
    pub fn from_event(event: &Event) -> Result<Option<Self>, DomainError> {
        let action = match event.kind {
            9_000 => {
                let tag = exactly_one(event, "p")?;
                let pubkey = tag.value().unwrap_or_default().to_owned();
                decode_lower_hex::<32>(&pubkey, "group member pubkey")?;
                Self::PutUser {
                    pubkey,
                    roles: tag.as_slice().iter().skip(2).cloned().collect(),
                }
            }
            9_001 => {
                let pubkey = exactly_one(event, "p")?
                    .value()
                    .unwrap_or_default()
                    .to_owned();
                decode_lower_hex::<32>(&pubkey, "group member pubkey")?;
                Self::RemoveUser { pubkey }
            }
            9_002 => Self::EditMetadata(GroupMetadata::from_tags(&event.tags)?),
            9_005 => {
                let event_id = exactly_one(event, "e")?
                    .value()
                    .unwrap_or_default()
                    .to_owned();
                decode_lower_hex::<32>(&event_id, "group event id")?;
                Self::DeleteEvent { event_id }
            }
            9_007 => Self::CreateGroup,
            9_008 => Self::DeleteGroup,
            9_009 => Self::CreateInvite {
                code: exactly_one(event, "code")?
                    .value()
                    .unwrap_or_default()
                    .to_owned(),
            },
            9_010 => Self::UpdatePins {
                tags: event
                    .tags
                    .iter()
                    .filter(|tag| matches!(tag.name(), Some("e" | "a")))
                    .cloned()
                    .collect(),
            },
            9_021 => Self::Join {
                code: event.tag_values("code").next().map(str::to_owned),
            },
            9_022 => Self::Leave,
            9_003..=9_004 | 9_006 | 9_011..=9_020 => {
                return Err(DomainError::InvalidEvent(
                    "the pinned NIP-29 moderation table does not define this kind".into(),
                ));
            }
            _ => return Ok(None),
        };
        Ok(Some(action))
    }
}

impl GroupMetadata {
    pub fn from_tags(tags: &[Tag]) -> Result<Self, DomainError> {
        let scalar = |name: &str| {
            tags.iter()
                .find(|tag| tag.name() == Some(name))
                .and_then(Tag::value)
                .unwrap_or_default()
                .to_owned()
        };
        let supported = tags
            .iter()
            .find(|tag| tag.name() == Some("supported_kinds"))
            .map(|tag| {
                tag.as_slice()
                    .iter()
                    .skip(1)
                    .map(|value| {
                        value.parse::<u16>().map_err(|_| {
                            DomainError::InvalidEvent("invalid supported group kind".into())
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?;
        let parents = tags
            .iter()
            .filter(|tag| tag.name() == Some("parent"))
            .collect::<Vec<_>>();
        if parents.len() > 1 {
            return Err(DomainError::InvalidEvent(
                "NIP-29 metadata has at most one parent tag".into(),
            ));
        }
        let parent = match parents.first() {
            None => None,
            Some(tag) => {
                let value = tag.value().unwrap_or_default();
                if value.is_empty() || value.len() > MAX_GROUP_ID_BYTES {
                    return Err(DomainError::InvalidEvent(
                        "NIP-29 parent must contain 1 to 128 bytes".into(),
                    ));
                }
                Some(value.to_owned())
            }
        };
        let mut children = Vec::new();
        for tag in tags.iter().filter(|tag| tag.name() == Some("child")) {
            let value = tag.value().unwrap_or_default();
            if value.is_empty()
                || value.len() > MAX_GROUP_ID_BYTES
                || children.iter().any(|seen: &String| seen == value)
            {
                return Err(DomainError::InvalidEvent(
                    "NIP-29 child identifiers must be unique and 1 to 128 bytes".into(),
                ));
            }
            children.push(value.to_owned());
        }
        Ok(Self {
            name: scalar("name"),
            about: scalar("about"),
            picture: scalar("picture"),
            banner: scalar("banner"),
            closed: tags.iter().any(|tag| tag.as_slice() == ["closed"]),
            private: tags.iter().any(|tag| tag.as_slice() == ["private"]),
            hidden: tags.iter().any(|tag| tag.as_slice() == ["hidden"]),
            restricted: tags.iter().any(|tag| tag.as_slice() == ["restricted"]),
            livekit: scalar("livekit"),
            parent,
            children,
            supported_kinds: supported,
        })
    }
}

/// Whether attaching `group_id` under `new_parent` walks back to itself.
///
/// `parents` maps each group id to its current parent. The walk includes
/// `new_parent` itself, so a self-parent and a longer cycle both refuse.
#[must_use]
pub fn parent_would_cycle(
    group_id: &str,
    new_parent: &str,
    parents: &std::collections::BTreeMap<String, String>,
) -> bool {
    if new_parent == group_id {
        return true;
    }
    let mut cursor = new_parent;
    let mut seen = std::collections::BTreeSet::new();
    while let Some(next) = parents.get(cursor) {
        if next == group_id || !seen.insert(next.clone()) {
            return true;
        }
        cursor = next;
    }
    false
}

/// Replace a group's child order.
///
/// The pinned rule is that a metadata edit names every current child. A
/// different set is refused. The returned order is the proposed order.
///
/// # Errors
///
/// Returns an invalid-event error when the sets differ.
pub fn reorder_children(
    current: &[String],
    proposed: &[String],
) -> Result<Vec<String>, DomainError> {
    let mut left = current.to_vec();
    let mut right = proposed.to_vec();
    left.sort();
    right.sort();
    if left != right {
        return Err(DomainError::InvalidEvent(
            "NIP-29 metadata must name every current child".into(),
        ));
    }
    Ok(proposed.to_vec())
}

pub(crate) fn validate_expanded_event(event: &Event) -> Result<(), DomainError> {
    if event.kind == 3 {
        super::follow::parse_follow_list(&event.tags)?;
    }
    if event.kind == 1_040 {
        super::ots::open_attestation(event)?;
    }
    if event.kind == 4 {
        crate::nip04::direct_message(event)?;
    }
    if matches!(
        event.kind,
        1_021 | 1_022 | 30_017 | 30_018 | 30_019 | 30_020
    ) {
        super::market::validate_marketplace(event)?;
    }
    let group_tags = event
        .tags
        .iter()
        // NIP-MP is explicitly global-only. Its unknown tags are opaque, so a
        // stray h (or previous) must not turn a project into NIP-29 traffic.
        .filter(|tag| !BLOCK_GLOBAL_ONLY_KINDS.contains(&event.kind) && tag.name() == Some("h"))
        .collect::<Vec<_>>();
    if !group_tags.is_empty() {
        if group_tags.len() != 1 {
            return Err(DomainError::InvalidEvent(
                "NIP-29 events require exactly one h tag".into(),
            ));
        }
        let group = group_tags[0].value().unwrap_or_default();
        if group.is_empty() || group.len() > MAX_GROUP_ID_BYTES {
            return Err(DomainError::InvalidEvent(
                "group id must contain 1 to 128 bytes".into(),
            ));
        }
    }
    if (9_000..=9_022).contains(&event.kind) && group_tags.is_empty() {
        return Err(DomainError::InvalidEvent(
            "NIP-29 management events require an h tag".into(),
        ));
    }
    if !group_tags.is_empty() {
        for tag in event
            .tags
            .iter()
            .filter(|tag| tag.name() == Some("previous"))
        {
            if tag.as_slice().len() < 2
                || tag.as_slice().iter().skip(1).any(|prefix| {
                    prefix.len() != 8
                        || !prefix
                            .bytes()
                            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                })
            {
                return Err(DomainError::InvalidEvent(
                    "NIP-29 previous references must be 8 lowercase hexadecimal characters".into(),
                ));
            }
        }
    }
    if let Some(GroupAction::CreateInvite { code }) = GroupAction::from_event(event)?
        && code.len() > 256
    {
        return Err(DomainError::InvalidEvent(
            "group invite code must contain at most 256 bytes".into(),
        ));
    }
    if event.kind == 1_111 {
        super::comment::open_comment(event)?;
    }
    if matches!(event.kind, 30_023 | 30_024) {
        super::article::open_article(event)?;
    }
    if event.kind == 1_985
        || event
            .tags
            .iter()
            .any(|tag| matches!(tag.name(), Some("l" | "L")))
    {
        super::label::open_labeling(event)?;
    }
    if event.kind == 1_059 {
        crate::nip17::validate_gift_wrap(event)?;
    }
    if event.kind == 10_050 {
        crate::nip17::validate_inbox(event)?;
    }
    if event.kind == 10_002 {
        let relays = event
            .tags
            .iter()
            .filter(|tag| tag.name() == Some("r"))
            .collect::<Vec<_>>();
        if relays.is_empty()
            || relays.iter().any(|tag| {
                let values = tag.as_slice();
                values.len() < 2
                    || values.len() > 3
                    || (!values[1].starts_with("ws://") && !values[1].starts_with("wss://"))
                    || values[1].len() > 2_048
                    || values[1].chars().any(char::is_whitespace)
                    || values
                        .get(2)
                        .is_some_and(|marker| !matches!(marker.as_str(), "read" | "write"))
            })
        {
            return Err(DomainError::InvalidEvent(
                "kind 10002 requires valid r relay tags and read/write markers".into(),
            ));
        }
    }
    if event.kind == 10_063 {
        let servers = event.tag_values("server").collect::<Vec<_>>();
        if servers.is_empty() || servers.iter().any(|server| !valid_http_url(server)) {
            return Err(DomainError::InvalidEvent(
                "kind 10063 requires valid http:// or https:// server tags".into(),
            ));
        }
    }
    if event.kind == 1_063 {
        let url = exactly_one(event, "url")?.value().unwrap_or_default();
        if !valid_http_url(url) {
            return Err(DomainError::InvalidEvent(
                "kind 1063 requires one valid HTTP url tag".into(),
            ));
        }
        let media_type = exactly_one(event, "m")?.value().unwrap_or_default();
        if !valid_media_type(media_type) {
            return Err(DomainError::InvalidEvent(
                "kind 1063 requires one lowercase MIME type tag".into(),
            ));
        }
        let sha256 = exactly_one(event, "x")?.value().unwrap_or_default();
        decode_lower_hex::<32>(sha256, "file metadata hash")?;
        let sizes = event.tag_values("size").collect::<Vec<_>>();
        if sizes.len() > 1
            || sizes
                .first()
                .is_some_and(|size| size.parse::<u64>().is_err())
        {
            return Err(DomainError::InvalidEvent(
                "kind 1063 size tag must be one unsigned integer".into(),
            ));
        }
    }
    Ok(())
}

fn valid_media_type(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 127
        && value == value.to_ascii_lowercase()
        && value.split_once('/').is_some_and(|(top, subtype)| {
            !top.is_empty()
                && !subtype.is_empty()
                && top.bytes().all(media_type_byte)
                && subtype.bytes().all(media_type_byte)
        })
}

fn valid_http_url(value: &str) -> bool {
    let Some(rest) = value
        .strip_prefix("http://")
        .or_else(|| value.strip_prefix("https://"))
    else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    !authority.is_empty() && value.len() <= 2_048 && !value.chars().any(char::is_whitespace)
}

fn media_type_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"!#$&^_.+-".contains(&byte)
}

fn exactly_one<'a>(event: &'a Event, name: &str) -> Result<&'a Tag, DomainError> {
    let tags = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some(name))
        .collect::<Vec<_>>();
    if tags.len() == 1 && tags[0].value().is_some_and(|value| !value.is_empty()) {
        Ok(tags[0])
    } else {
        Err(DomainError::InvalidEvent(format!(
            "event requires exactly one {name} tag"
        )))
    }
}

fn decode_base64(value: &str) -> Result<Vec<u8>, DomainError> {
    let compact = value.trim_end_matches('=');
    let mut output = Vec::with_capacity(compact.len().saturating_mul(3) / 4);
    let mut accumulator = 0_u32;
    let mut bits = 0_u8;
    for byte in compact.bytes() {
        let digit = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => {
                return Err(DomainError::InvalidEvent(
                    "HTTP authorization is not valid base64".into(),
                ));
            }
        };
        accumulator = (accumulator << 6) | u32::from(digit);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push((accumulator >> bits) as u8);
            accumulator &= (1_u32 << bits).saturating_sub(1);
        }
    }
    if bits >= 6 || accumulator != 0 {
        return Err(DomainError::InvalidEvent(
            "HTTP authorization has invalid base64 padding".into(),
        ));
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tag(values: &[&str]) -> Tag {
        Tag::new(values.iter().map(|value| (*value).to_owned()).collect())
    }

    #[test]
    fn private_hidden_and_subgroup_fields_follow_the_pinned_metadata_event() {
        let metadata = GroupMetadata::from_tags(&[
            tag(&["name", "Nostr"]),
            tag(&["private"]),
            tag(&["hidden"]),
            tag(&["parent", "tech"]),
            tag(&["child", "nip29"]),
            tag(&["livekit", "wss://live.example"]),
        ])
        .unwrap();
        assert!(metadata.private);
        assert!(metadata.hidden);
        assert!(!metadata.restricted);
        assert!(!metadata.closed);
        assert_eq!(metadata.parent.as_deref(), Some("tech"));
        assert_eq!(metadata.children, vec!["nip29".to_owned()]);
        assert_eq!(metadata.livekit, "wss://live.example");

        let pinned = include_str!("../../../../nips/official/29.md");
        assert!(pinned.contains("| 9000 |"));
        assert!(pinned.contains("| 9010 |"));
        assert!(
            !pinned.contains("| 9003 |"),
            "the pinned moderation table does not assign kind 9003"
        );
    }

    #[test]
    fn an_undefined_moderation_kind_is_refused() {
        let event = Event {
            id: "ab".repeat(32),
            pubkey: "cd".repeat(32),
            created_at: 1,
            kind: 9_003,
            tags: vec![tag(&["h", "tech"])],
            content: String::new(),
            sig: "ef".repeat(64),
        };
        let error = GroupAction::from_event(&event).unwrap_err();
        assert!(error.to_string().contains("does not define"), "{error}");
    }

    #[test]
    fn a_parent_cycle_and_a_partial_child_list_are_refused() {
        let mut parents = std::collections::BTreeMap::new();
        parents.insert("tech".to_owned(), "nostr".to_owned());
        assert!(parent_would_cycle("nostr", "tech", &parents));
        assert!(parent_would_cycle("tech", "tech", &parents));
        assert!(!parent_would_cycle("nip29", "tech", &parents));
        assert!(reorder_children(&["nip29".into(), "spec".into()], &["spec".into()]).is_err());
        assert_eq!(
            reorder_children(
                &["nip29".into(), "spec".into()],
                &["spec".into(), "nip29".into()]
            )
            .unwrap(),
            vec!["spec".to_owned(), "nip29".to_owned()]
        );
    }
}
