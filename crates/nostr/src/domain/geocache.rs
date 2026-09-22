//! NIP-CC geocaching.
//!
//! Kind `37516` is an addressable cache. It needs a `d` identifier, a
//! `name`, one or more `g` geohashes, difficulty `D`, terrain `T`, and
//! size `S`. Kind `7516` is a found log. Kind `7517` is a verification
//! signed by the cache's verification key. Kind `37517` is a curation
//! list. A non-found log is a kind `1111` comment.
//!
//! Unknown `n` modifiers are kept. The 8-character submission guidance is
//! not enforced, because the pinned example includes shorter prefixes.
//! The relay does not fetch images. These kinds are not added to the
//! NIP-11 list.

use std::str::FromStr;

use super::hex::decode_lower_hex;
use super::{DomainError, Event, ReplacementAddress};

const CACHE_KIND: u16 = 37_516;
const LEGACY_CACHE_KIND: u16 = 37_515;
const FOUND_KIND: u16 = 7_516;
const VERIFY_KIND: u16 = 7_517;
const LIST_KIND: u16 = 37_517;
const GEOHASH: &[u8] = b"0123456789bcdefghjkmnpqrstuvwxyz";

/// Cache size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CacheSize {
    Micro,
    Small,
    Regular,
    Large,
    Other,
}

/// A non-found log type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CacheLogType {
    Dnf,
    Note,
    Maintenance,
    Archived,
}

/// A kind `37516` geocache.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Geocache {
    pub owner: String,
    pub identifier: String,
    pub name: String,
    pub content: String,
    pub geohashes: Vec<String>,
    pub difficulty: u8,
    pub terrain: u8,
    pub size: CacheSize,
    pub cache_type: String,
    pub archived: bool,
    pub hint: Option<String>,
    pub mission: Option<String>,
    pub images: Vec<String>,
    pub relays: Vec<String>,
    pub verification: Option<String>,
    pub first_to_find: bool,
    pub art: bool,
    pub other_modifiers: Vec<String>,
    pub winner: Option<String>,
}

/// A kind `7516` found log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FoundLog {
    pub id: String,
    pub author: String,
    pub created_at: u64,
    pub cache: ReplacementAddress,
    pub verification: Option<Event>,
    pub verified: bool,
}

/// A kind `7517` verification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Verification {
    pub finder: String,
    pub reference: String,
}

/// A kind `37517` curation list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CurationList {
    pub identifier: String,
    pub title: String,
    pub content: String,
    pub description: Option<String>,
    pub images: Vec<String>,
    pub geohashes: Vec<String>,
    pub theme: Option<String>,
    pub map: Option<String>,
    pub caches: Vec<ReplacementAddress>,
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.to_owned())
}

fn is_relay(value: &str) -> bool {
    (value.starts_with("ws://") || value.starts_with("wss://"))
        && value.len() <= 2_048
        && !value.chars().any(char::is_whitespace)
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

fn geohash_ok(value: &str, min: usize, max: usize) -> bool {
    (min..=max).contains(&value.len()) && value.bytes().all(|byte| GEOHASH.contains(&byte))
}

fn score(value: &str) -> Result<u8, DomainError> {
    match value {
        "1" | "2" | "3" | "4" | "5" => Ok(value.as_bytes()[0] - b'0'),
        _ => Err(invalid("a geocache score is an integer from 1 to 5")),
    }
}

fn size_of(value: &str) -> Result<CacheSize, DomainError> {
    match value {
        "micro" => Ok(CacheSize::Micro),
        "small" => Ok(CacheSize::Small),
        "regular" => Ok(CacheSize::Regular),
        "large" => Ok(CacheSize::Large),
        "other" => Ok(CacheSize::Other),
        _ => Err(invalid(
            "a geocache size is micro, small, regular, large, or other",
        )),
    }
}

fn pubkey(value: &str, reason: &'static str) -> Result<String, DomainError> {
    decode_lower_hex::<32>(value, reason).map_err(|_| invalid(reason))?;
    Ok(value.to_owned())
}

fn one<'a>(event: &'a Event, name: &str, reason: &str) -> Result<&'a str, DomainError> {
    let tags = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some(name))
        .collect::<Vec<_>>();
    if tags.len() != 1 {
        return Err(invalid(reason));
    }
    tags[0]
        .value()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid(reason))
}

fn optional<'a>(
    event: &'a Event,
    name: &str,
    reason: &str,
) -> Result<Option<&'a str>, DomainError> {
    let tags = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some(name))
        .collect::<Vec<_>>();
    if tags.len() > 1 {
        return Err(invalid(reason));
    }
    match tags.first() {
        None => Ok(None),
        Some(tag) => match tag.value() {
            Some(value) if !value.is_empty() => Ok(Some(value)),
            _ => Err(invalid(reason)),
        },
    }
}

/// Hide or reveal a hint. ROT13 is its own inverse.
pub fn rot13(text: &str) -> String {
    text.chars()
        .map(|char| match char {
            'a'..='z' => char::from(b'a' + (char as u8 - b'a' + 13) % 26),
            'A'..='Z' => char::from(b'A' + (char as u8 - b'A' + 13) % 26),
            other => other,
        })
        .collect()
}

/// Read a kind `37516` geocache.
///
/// # Errors
///
/// Returns a sentence when a required tag or a score is refused.
pub fn open_geocache(event: &Event) -> Result<Geocache, DomainError> {
    if event.kind != CACHE_KIND {
        return Err(invalid("a geocache listing has kind 37516"));
    }
    let identifier = one(event, "d", "a geocache has one d tag")?;
    if identifier.len() > 256 || identifier.chars().any(char::is_whitespace) {
        return Err(invalid("a geocache has one d tag"));
    }
    let name = one(event, "name", "a geocache has one name")?;
    let mut geohashes = Vec::new();
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("g")) {
        let Some(value) = tag.value().filter(|value| geohash_ok(value, 3, 9)) else {
            return Err(invalid("a geocache geohash is 3 to 9 characters"));
        };
        geohashes.push(value.to_owned());
    }
    if geohashes.is_empty() {
        return Err(invalid("a geocache geohash is 3 to 9 characters"));
    }
    let difficulty = score(one(
        event,
        "D",
        "a geocache score is an integer from 1 to 5",
    )?)?;
    let terrain = score(one(
        event,
        "T",
        "a geocache score is an integer from 1 to 5",
    )?)?;
    let size = size_of(one(
        event,
        "S",
        "a geocache size is micro, small, regular, large, or other",
    )?)?;
    let mut cache_type = None;
    let mut archived = false;
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("t")) {
        match tag.value() {
            Some("archived") => archived = true,
            Some(value) if !value.is_empty() && cache_type.is_none() => {
                cache_type = Some(value.to_owned());
            }
            Some(value) if !value.is_empty() => {}
            _ => return Err(invalid("a geocache type is non-empty")),
        }
    }
    let mut first_to_find = false;
    let mut art = false;
    let mut other_modifiers = Vec::new();
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("n")) {
        match tag.value() {
            Some("first-to-find") if !first_to_find => first_to_find = true,
            Some("art") if !art => art = true,
            Some("first-to-find" | "art") => {}
            Some(value) if !value.is_empty() => other_modifiers.push(value.to_owned()),
            _ => return Err(invalid("a geocache modifier is non-empty")),
        }
    }
    let verification = match optional(
        event,
        "verification",
        "a geocache verification key is 32 lowercase hex bytes",
    )? {
        None => None,
        Some(value) => Some(pubkey(
            value,
            "a geocache verification key is 32 lowercase hex bytes",
        )?),
    };
    if first_to_find && verification.is_none() {
        return Err(invalid("a first-to-find geocache has a verification key"));
    }
    let mut winner = None;
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("F")) {
        if winner.is_some() {
            continue;
        }
        let Some(value) = tag.value() else {
            return Err(invalid("a first-to-find winner is 32 lowercase hex bytes"));
        };
        winner = Some(pubkey(
            value,
            "a first-to-find winner is 32 lowercase hex bytes",
        )?);
    }
    if winner.is_some() && !first_to_find {
        return Err(invalid(
            "a first-to-find winner requires the first-to-find modifier",
        ));
    }
    let mission = event
        .tags
        .iter()
        .find(|tag| tag.name() == Some("mission"))
        .map(|tag| {
            tag.value()
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .ok_or_else(|| invalid("a geocache mission is non-empty"))
        })
        .transpose()?;
    let mut images = Vec::new();
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("image")) {
        let Some(value) = tag.value().filter(|value| valid_http_url(value)) else {
            return Err(invalid("a geocache image is an http:// or https:// URL"));
        };
        images.push(value.to_owned());
    }
    let mut relays = Vec::new();
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("r")) {
        let Some(value) = tag.value().filter(|value| is_relay(value)) else {
            return Err(invalid("a geocache relay is ws:// or wss://"));
        };
        relays.push(value.to_owned());
    }
    Ok(Geocache {
        owner: event.pubkey.clone(),
        identifier: identifier.to_owned(),
        name: name.to_owned(),
        content: event.content.clone(),
        geohashes,
        difficulty,
        terrain,
        size,
        cache_type: cache_type.unwrap_or_else(|| "traditional".to_owned()),
        archived,
        hint: optional(event, "hint", "a geocache hint is one non-empty value")?.map(str::to_owned),
        mission,
        images,
        relays,
        verification,
        first_to_find,
        art,
        other_modifiers,
        winner,
    })
}

fn cache_address(value: &str) -> Result<ReplacementAddress, DomainError> {
    let address = ReplacementAddress::from_str(value)
        .map_err(|_| invalid("a found log references a kind 37516 geocache"))?;
    if address.kind != CACHE_KIND || address.identifier.is_empty() {
        return Err(invalid("a found log references a kind 37516 geocache"));
    }
    Ok(address)
}

/// Read a kind `7516` found log.
///
/// # Errors
///
/// Returns a sentence when the cache address or embedded verification is refused.
pub fn open_found_log(event: &Event) -> Result<FoundLog, DomainError> {
    if event.kind != FOUND_KIND {
        return Err(invalid("a found log has kind 7516"));
    }
    let addresses = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("a"))
        .collect::<Vec<_>>();
    if addresses.len() != 1 {
        return Err(invalid("a found log references a kind 37516 geocache"));
    }
    let Some(value) = addresses[0].value() else {
        return Err(invalid("a found log references a kind 37516 geocache"));
    };
    let cache = cache_address(value)?;
    let verification_tags = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("verification"))
        .collect::<Vec<_>>();
    if verification_tags.len() > 1 {
        return Err(invalid("a found log has one verification event"));
    }
    let verification = match verification_tags.first() {
        None => None,
        Some(tag) => {
            let Some(value) = tag.value() else {
                return Err(invalid("a found log has one verification event"));
            };
            let embedded: Event = serde_json::from_str(value)
                .map_err(|_| invalid("a found log verification is a kind 7517 event"))?;
            open_verification(&embedded)?;
            Some(embedded)
        }
    };
    Ok(FoundLog {
        id: event.id.clone(),
        author: event.pubkey.clone(),
        created_at: event.created_at,
        cache,
        verification,
        verified: false,
    })
}

/// Read a kind `7517` verification.
///
/// # Errors
///
/// Returns a sentence when the content or finder reference is refused.
pub fn open_verification(event: &Event) -> Result<Verification, DomainError> {
    if event.kind != VERIFY_KIND {
        return Err(invalid("a geocache verification has kind 7517"));
    }
    let Some(npub) = event
        .content
        .strip_prefix("Geocache verification for ")
        .filter(|value| !value.is_empty())
    else {
        return Err(invalid("a geocache verification names the finder npub"));
    };
    let finder_bytes = crate::nip19::decode_npub(npub)
        .map_err(|_| invalid("a geocache verification names the finder npub"))?;
    let encoded = crate::nip19::encode_npub(&finder_bytes);
    if encoded != npub {
        return Err(invalid("a geocache verification names the finder npub"));
    }
    let finder = hex_encode(&finder_bytes);
    let tags = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("a"))
        .collect::<Vec<_>>();
    if tags.len() != 1 {
        return Err(invalid(
            "a geocache verification names the finder and the cache",
        ));
    }
    let Some(value) = tags[0].value() else {
        return Err(invalid(
            "a geocache verification names the finder and the cache",
        ));
    };
    let Some((claimed, reference)) = value.split_once(':') else {
        return Err(invalid(
            "a geocache verification names the finder and the cache",
        ));
    };
    if claimed != finder || reference.is_empty() || reference.chars().any(char::is_whitespace) {
        return Err(invalid(
            "a geocache verification names the finder and the cache",
        ));
    }
    Ok(Verification {
        finder,
        reference: reference.to_owned(),
    })
}

fn hex_encode(bytes: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(64);
    for byte in bytes {
        out.push(char::from(HEX[usize::from(byte >> 4)]));
        out.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    out
}

/// Confirm an embedded verification against the cache and the log author.
///
/// # Errors
///
/// Returns a sentence when the signature, finder, or verification key does not match.
pub fn confirm_find(cache: &Geocache, mut log: FoundLog) -> Result<FoundLog, DomainError> {
    if log.cache.identifier != cache.identifier || log.cache.pubkey != cache.owner {
        return Err(invalid("a found log references this geocache"));
    }
    let Some(verification) = log.verification.clone() else {
        log.verified = false;
        return Ok(log);
    };
    let expected = cache
        .verification
        .as_deref()
        .ok_or_else(|| invalid("a verified find requires the cache verification key"))?;
    if verification.pubkey != expected {
        return Err(invalid(
            "a verified find is signed by the cache verification key",
        ));
    }
    verification
        .validate_crypto()
        .map_err(|_| invalid("a verified find is signed by the cache verification key"))?;
    let parsed = open_verification(&verification)?;
    if parsed.finder != log.author {
        return Err(invalid("a verified find names the log author"));
    }
    log.verified = true;
    Ok(log)
}

/// The exclusive finder for a `first-to-find` cache.
///
/// An `F` tag wins over timestamp order. Otherwise the earliest verified
/// log wins, and equal timestamps break toward the lower event id.
pub fn exclusive_finder<'a>(cache: &Geocache, logs: &'a [FoundLog]) -> Option<&'a FoundLog> {
    let mut verified = logs.iter().filter(|log| log.verified);
    if let Some(winner) = &cache.winner {
        return verified.find(|log| &log.author == winner);
    }
    verified.min_by(|left, right| {
        left.created_at
            .cmp(&right.created_at)
            .then_with(|| left.id.cmp(&right.id))
    })
}

/// The log type of a kind `1111` comment. A missing `t` tag is `note`.
///
/// # Errors
///
/// Returns a sentence when a `t` tag is not a known log type.
pub fn cache_log_type(event: &Event) -> Result<CacheLogType, DomainError> {
    let tags = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("t"))
        .collect::<Vec<_>>();
    match tags.first().and_then(|tag| tag.value()) {
        None => Ok(CacheLogType::Note),
        Some("dnf") => Ok(CacheLogType::Dnf),
        Some("note") => Ok(CacheLogType::Note),
        Some("maintenance") => Ok(CacheLogType::Maintenance),
        Some("archived") => Ok(CacheLogType::Archived),
        _ => Err(invalid(
            "a geocache log type is dnf, note, maintenance, or archived",
        )),
    }
}

/// Read a kind `37517` curation list.
///
/// # Errors
///
/// Returns a sentence when the title or a cache reference is refused.
pub fn open_curation(event: &Event) -> Result<CurationList, DomainError> {
    if event.kind != LIST_KIND {
        return Err(invalid("a geocache list has kind 37517"));
    }
    let identifier = one(event, "d", "a geocache list has one d tag")?;
    let title = one(event, "title", "a geocache list has one title")?;
    let theme = match optional(
        event,
        "theme",
        "a geocache list theme is adventure or mojave",
    )? {
        None => None,
        Some(value @ ("adventure" | "mojave")) => Some(value.to_owned()),
        Some(_) => return Err(invalid("a geocache list theme is adventure or mojave")),
    };
    let map = match optional(
        event,
        "map",
        "a geocache list map is original, dark, satellite, or adventure",
    )? {
        None => None,
        Some(value @ ("original" | "dark" | "satellite" | "adventure")) => Some(value.to_owned()),
        Some(_) => {
            return Err(invalid(
                "a geocache list map is original, dark, satellite, or adventure",
            ));
        }
    };
    let mut geohashes = Vec::new();
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("g")) {
        let Some(value) = tag.value().filter(|value| geohash_ok(value, 3, 6)) else {
            return Err(invalid("a geocache list geohash is 3 to 6 characters"));
        };
        geohashes.push(value.to_owned());
    }
    let mut images = Vec::new();
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("image")) {
        let Some(value) = tag.value().filter(|value| valid_http_url(value)) else {
            return Err(invalid("a geocache image is an http:// or https:// URL"));
        };
        images.push(value.to_owned());
    }
    let mut caches = Vec::new();
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("a")) {
        let Some(value) = tag.value() else {
            return Err(invalid(
                "a geocache list references a kind 37516 or 37515 cache",
            ));
        };
        let address = ReplacementAddress::from_str(value)
            .map_err(|_| invalid("a geocache list references a kind 37516 or 37515 cache"))?;
        if !matches!(address.kind, CACHE_KIND | LEGACY_CACHE_KIND) || address.identifier.is_empty()
        {
            return Err(invalid(
                "a geocache list references a kind 37516 or 37515 cache",
            ));
        }
        caches.push(address);
    }
    if caches.is_empty() {
        return Err(invalid(
            "a geocache list references a kind 37516 or 37515 cache",
        ));
    }
    Ok(CurationList {
        identifier: identifier.to_owned(),
        title: title.to_owned(),
        content: event.content.clone(),
        description: optional(
            event,
            "description",
            "a geocache list description is non-empty",
        )?
        .map(str::to_owned),
        images,
        geohashes,
        theme,
        map,
        caches,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        EventClass, RelaySigner, ReplacementDecision, Tag, compare_replacement, is_top_level,
        open_comment,
    };
    use crate::nip19::encode_npub;

    use super::decode_lower_hex;

    fn signer(byte: &str) -> RelaySigner {
        RelaySigner::from_secret_hex(&byte.repeat(32)).unwrap()
    }

    #[test]
    fn a_geocache_is_found_verified_and_collected() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../nips/official/CC.md"
        ))
        .unwrap();
        assert!(text.contains("kind 37516") || text.contains("Kind 37516"));
        assert!(text.contains("First Treasure"));
        assert!(text.contains("u4xsu6ry"));
        assert!(text.contains("first-to-find"));
        assert!(text.contains("kind 7516") || text.contains("Kind 7516"));
        assert!(text.contains("kind 7517") || text.contains("Kind 7517"));
        assert!(text.contains("kind 37517") || text.contains("Kind 37517"));
        let row = crate::lane::PROVEN
            .iter()
            .find(|row| row.file == "CC.md")
            .unwrap();
        assert_eq!(row.status, "configured-and-proven");
        assert!(
            crate::lane::SHAPES
                .iter()
                .all(|shape| shape.file != "CC.md")
        );

        let owner = signer("cc");
        let hint = "In the branches";
        assert_eq!(rot13(&rot13(hint)), hint);
        let cache = owner.sign(
            1_748_619_568,
            CACHE_KIND,
            vec![
                Tag::new(vec!["d".into(), "first-treasure-1748619568668".into()]),
                Tag::new(vec!["name".into(), "First Treasure".into()]),
                Tag::new(vec!["g".into(), "u4x".into()]),
                Tag::new(vec!["g".into(), "u4xsu6ry".into()]),
                Tag::new(vec!["g".into(), "u4xsu6ryb".into()]),
                Tag::new(vec!["D".into(), "1".into()]),
                Tag::new(vec!["T".into(), "1".into()]),
                Tag::new(vec!["S".into(), "small".into()]),
                Tag::new(vec!["t".into(), "traditional".into()]),
                Tag::new(vec!["hint".into(), hint.into()]),
                Tag::new(vec![
                    "image".into(),
                    "https://blossom.primal.net/74efe01a767b27dead71b8a9bb8278a108360438e78e55194ed9ab14a9382dd3.jpg".into(),
                ]),
            ],
            "The first Nostr treasure, left in the aftermath of Oslo Freedom Forum!".into(),
        );
        cache.validate_structure().unwrap();
        assert_eq!(cache.class(), EventClass::Addressable);
        let opened = open_geocache(&cache).unwrap();
        assert_eq!(opened.name, "First Treasure");
        assert_eq!(opened.difficulty, 1);
        assert_eq!(opened.terrain, 1);
        assert_eq!(opened.size, CacheSize::Small);
        assert_eq!(opened.cache_type, "traditional");
        assert_eq!(opened.geohashes[0], "u4x");
        assert!(opened.geohashes.iter().any(|value| value == "u4xsu6ry"));

        let newer = owner.sign(
            1_748_619_600,
            CACHE_KIND,
            vec![
                Tag::new(vec!["d".into(), "first-treasure-1748619568668".into()]),
                Tag::new(vec!["name".into(), "First Treasure".into()]),
                Tag::new(vec!["g".into(), "u4xsu6ry".into()]),
                Tag::new(vec!["D".into(), "1".into()]),
                Tag::new(vec!["T".into(), "1".into()]),
                Tag::new(vec!["S".into(), "small".into()]),
            ],
            String::new(),
        );
        assert_eq!(
            compare_replacement(&cache, &newer).unwrap(),
            ReplacementDecision::ReplaceCurrent
        );
        assert_eq!(open_geocache(&newer).unwrap().cache_type, "traditional");

        let finder = signer("c2");
        let verifier = signer("c1");
        let npub =
            encode_npub(&decode_lower_hex::<32>(finder.pubkey(), "finder").expect("finder pubkey"));
        let verification = verifier.sign(
            1_672_531_200,
            VERIFY_KIND,
            vec![Tag::new(vec![
                "a".into(),
                format!("{}:naddr1cache", finder.pubkey()),
            ])],
            format!("Geocache verification for {npub}"),
        );
        verification.validate_structure().unwrap();
        assert_eq!(verification.class(), EventClass::Regular);
        let treasure = format!("37516:{}:verified-treasure-1748619568669", owner.pubkey());
        let log = finder.sign(
            1_748_619_700,
            FOUND_KIND,
            vec![
                Tag::new(vec!["a".into(), treasure.clone()]),
                Tag::new(vec![
                    "verification".into(),
                    serde_json::to_string(&verification).unwrap(),
                ]),
            ],
            "Found it! Great hiding spot.".into(),
        );
        log.validate_structure().unwrap();
        let verified_cache = owner.sign(
            1_748_619_650,
            CACHE_KIND,
            vec![
                Tag::new(vec!["d".into(), "verified-treasure-1748619568669".into()]),
                Tag::new(vec!["name".into(), "Verified Treasure".into()]),
                Tag::new(vec!["g".into(), "u4xsu6ry".into()]),
                Tag::new(vec!["D".into(), "3".into()]),
                Tag::new(vec!["T".into(), "2".into()]),
                Tag::new(vec!["S".into(), "small".into()]),
                Tag::new(vec!["n".into(), "first-to-find".into()]),
                Tag::new(vec!["n".into(), "art".into()]),
                Tag::new(vec!["n".into(), "glow".into()]),
                Tag::new(vec!["verification".into(), verifier.pubkey().to_owned()]),
                Tag::new(vec!["F".into(), finder.pubkey().to_owned()]),
            ],
            "High-security treasure requiring physical verification!".into(),
        );
        verified_cache.validate_structure().unwrap();
        let verified_cache = open_geocache(&verified_cache).unwrap();
        assert!(verified_cache.first_to_find);
        assert!(verified_cache.art);
        assert_eq!(verified_cache.other_modifiers, vec!["glow".to_owned()]);
        let found = confirm_find(&verified_cache, open_found_log(&log).unwrap()).unwrap();
        assert!(found.verified);
        let earlier = FoundLog {
            id: "aa".repeat(32),
            author: "ab".repeat(32),
            created_at: 1,
            cache: found.cache.clone(),
            verification: None,
            verified: true,
        };
        assert_eq!(
            exclusive_finder(&verified_cache, &[earlier, found])
                .unwrap()
                .author,
            finder.pubkey()
        );

        let address = format!("37516:{}:first-treasure-1748619568668", owner.pubkey());
        let dnf = finder.sign(
            1_748_619_800,
            1_111,
            vec![
                Tag::new(vec![
                    "A".into(),
                    address.clone(),
                    "wss://relay.example".into(),
                ]),
                Tag::new(vec!["K".into(), "37516".into()]),
                Tag::new(vec![
                    "P".into(),
                    owner.pubkey().to_owned(),
                    "wss://relay.example".into(),
                ]),
                Tag::new(vec!["a".into(), address, "wss://relay.example".into()]),
                Tag::new(vec!["e".into(), cache.id, "wss://relay.example".into()]),
                Tag::new(vec!["k".into(), "37516".into()]),
                Tag::new(vec![
                    "p".into(),
                    owner.pubkey().to_owned(),
                    "wss://relay.example".into(),
                ]),
                Tag::new(vec!["t".into(), "dnf".into()]),
            ],
            "Searched for 30 minutes but couldn't find it. Maybe it's missing?".into(),
        );
        dnf.validate_structure().unwrap();
        assert!(is_top_level(&open_comment(&dnf).unwrap()));
        assert_eq!(cache_log_type(&dnf).unwrap(), CacheLogType::Dnf);

        let list = owner.sign(
            1_748_619_900,
            LIST_KIND,
            vec![
                Tag::new(vec!["d".into(), "ren-fest-hunt-1748619568670".into()]),
                Tag::new(vec!["title".into(), "Texas Ren Fest Treasure Hunt".into()]),
                Tag::new(vec!["g".into(), "9vk".into()]),
                Tag::new(vec!["g".into(), "9vk5b7".into()]),
                Tag::new(vec!["theme".into(), "adventure".into()]),
                Tag::new(vec!["map".into(), "adventure".into()]),
                Tag::new(vec![
                    "a".into(),
                    format!("37516:{}:first-treasure-1748619568668", owner.pubkey()),
                ]),
                Tag::new(vec![
                    "a".into(),
                    format!("37516:{}:verified-treasure-1748619568669", owner.pubkey()),
                ]),
            ],
            "Explore the festival grounds and find all hidden treasures before the jousting tournament!".into(),
        );
        list.validate_structure().unwrap();
        assert_eq!(list.class(), EventClass::Addressable);
        let list = open_curation(&list).unwrap();
        assert_eq!(list.caches.len(), 2);
        assert_eq!(list.theme.as_deref(), Some("adventure"));

        let missing = owner.sign(
            1_748_619_901,
            CACHE_KIND,
            vec![
                Tag::new(vec!["d".into(), "incomplete".into()]),
                Tag::new(vec!["g".into(), "u4x".into()]),
                Tag::new(vec!["D".into(), "1".into()]),
                Tag::new(vec!["T".into(), "1".into()]),
                Tag::new(vec!["S".into(), "small".into()]),
            ],
            String::new(),
        );
        assert!(missing.validate_structure().is_err());
        let locked = owner.sign(
            1_748_619_902,
            CACHE_KIND,
            vec![
                Tag::new(vec!["d".into(), "locked".into()]),
                Tag::new(vec!["name".into(), "Locked".into()]),
                Tag::new(vec!["g".into(), "u4x".into()]),
                Tag::new(vec!["D".into(), "6".into()]),
                Tag::new(vec!["T".into(), "1".into()]),
                Tag::new(vec!["S".into(), "small".into()]),
            ],
            String::new(),
        );
        assert!(locked.validate_structure().is_err());
    }
}
