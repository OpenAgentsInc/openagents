//! NIP-53 live streams and meeting spaces.
//!
//! Kind `30311` advertises a live stream. Kind `1311` is a chat message
//! in that stream. Kind `30312` is a meeting room and kind `30313` is a
//! meeting inside it. Kind `10312` says which room a pubkey is in.
//!
//! A participant proof is a Schnorr signature over the SHA-256 of the
//! activity address `kind:pubkey:d`. A missing proof leaves the
//! participant unmarked. A `live` status that has not been updated for
//! more than one hour is stale. The relay does not open the stream or
//! the meeting service, and it does not rewrite that status. The
//! suggestion to name fewer than 1000 participants is not enforced.
//! NIP-53 is a draft, so these kinds stay off the NIP-11 list.

use std::str::FromStr;

use secp256k1::{Keypair, Secp256k1, SecretKey, XOnlyPublicKey, schnorr::Signature};
use sha2::{Digest, Sha256};

use super::hex::{decode_lower_hex, encode_lower_hex};
use super::{DomainError, Event, ReplacementAddress};

const STREAM_KIND: u16 = 30_311;
const CHAT_KIND: u16 = 1_311;
const ROOM_KIND: u16 = 30_312;
const MEETING_KIND: u16 = 30_313;
const PRESENCE_KIND: u16 = 10_312;
const STALE_AFTER_SECONDS: u64 = 3_600;

/// Whether a stream or meeting is scheduled, current, or finished.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LiveStatus {
    Planned,
    Live,
    Ended,
}

/// Whether a meeting room is open, private, or closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomStatus {
    Open,
    Private,
    Closed,
}

/// One pubkey named on a stream, room, or meeting.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiveParticipant {
    pub pubkey: String,
    pub relay: Option<String>,
    pub role: Option<String>,
    /// True when the proof signs this activity's address.
    pub agreed: bool,
}

/// A kind `30311` live stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiveStream {
    pub identifier: String,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub image: Option<String>,
    pub streaming: Option<String>,
    pub recording: Option<String>,
    pub starts: Option<u64>,
    pub ends: Option<u64>,
    pub status: Option<LiveStatus>,
    pub current_participants: Option<u64>,
    pub total_participants: Option<u64>,
    pub participants: Vec<LiveParticipant>,
    pub relays: Vec<String>,
    pub pinned: Vec<String>,
    pub hashtags: Vec<String>,
    pub content: String,
}

/// A kind `1311` chat message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiveChat {
    pub activity: ReplacementAddress,
    pub relay: Option<String>,
    pub root: bool,
    pub parent: Option<String>,
    pub quotes: Vec<String>,
    pub content: String,
}

/// A kind `30312` meeting room.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MeetingRoom {
    pub identifier: String,
    pub name: String,
    pub summary: Option<String>,
    pub image: Option<String>,
    pub status: RoomStatus,
    pub service: String,
    pub endpoint: Option<String>,
    pub participants: Vec<LiveParticipant>,
    pub relays: Vec<String>,
    pub hashtags: Vec<String>,
    pub content: String,
}

/// A kind `30313` meeting inside a room.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Meeting {
    pub identifier: String,
    pub room: ReplacementAddress,
    pub room_relay: Option<String>,
    pub title: String,
    pub summary: Option<String>,
    pub image: Option<String>,
    pub starts: u64,
    pub ends: Option<u64>,
    pub status: LiveStatus,
    pub current_participants: Option<u64>,
    pub total_participants: Option<u64>,
    pub participants: Vec<LiveParticipant>,
    pub content: String,
}

/// A kind `10312` presence in one room.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Presence {
    pub room: ReplacementAddress,
    pub relay: Option<String>,
    pub hand_raised: bool,
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.to_owned())
}

fn http_url(value: &str) -> bool {
    let Some(rest) = value
        .strip_prefix("https://")
        .or_else(|| value.strip_prefix("http://"))
    else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    !authority.is_empty() && value.len() <= 2_048 && !value.chars().any(char::is_whitespace)
}

fn relay_url(value: &str) -> bool {
    value.is_empty()
        || ((value.starts_with("wss://") || value.starts_with("ws://"))
            && value.len() > "wss://".len()
            && value.len() <= 2_048
            && !value.chars().any(char::is_whitespace))
}

fn identifier(value: &str) -> Result<String, DomainError> {
    if value.is_empty()
        || value.len() > 1_024
        || value
            .chars()
            .any(|char| char.is_control() || char.is_whitespace())
    {
        return Err(invalid("an activity identifier is 1 to 1024 characters"));
    }
    Ok(value.to_owned())
}

fn one<'a>(event: &'a Event, name: &str, reason: &str) -> Result<Option<&'a str>, DomainError> {
    let mut found = event.tags.iter().filter(|tag| tag.name() == Some(name));
    let Some(tag) = found.next() else {
        return Ok(None);
    };
    if found.next().is_some() {
        return Err(invalid(reason));
    }
    tag.value()
        .filter(|value| !value.is_empty())
        .map(Some)
        .ok_or_else(|| invalid(reason))
}

fn required<'a>(event: &'a Event, name: &str, reason: &str) -> Result<&'a str, DomainError> {
    one(event, name, reason)?.ok_or_else(|| invalid(reason))
}

fn text(value: &str, limit: usize, reason: &str) -> Result<String, DomainError> {
    if value.is_empty() || value.len() > limit || value.chars().any(char::is_control) {
        return Err(invalid(reason));
    }
    Ok(value.to_owned())
}

fn whole(value: &str, reason: &str) -> Result<u64, DomainError> {
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(invalid(reason));
    }
    value.parse().map_err(|_| invalid(reason))
}

fn live_status(value: &str) -> Result<LiveStatus, DomainError> {
    match value {
        "planned" => Ok(LiveStatus::Planned),
        "live" => Ok(LiveStatus::Live),
        "ended" => Ok(LiveStatus::Ended),
        _ => Err(invalid("a live status is planned, live, or ended")),
    }
}

fn room_status(value: &str) -> Result<RoomStatus, DomainError> {
    match value {
        "open" => Ok(RoomStatus::Open),
        "private" => Ok(RoomStatus::Private),
        "closed" => Ok(RoomStatus::Closed),
        _ => Err(invalid("a room status is open, private, or closed")),
    }
}

fn optional_url(event: &Event, name: &str) -> Result<Option<String>, DomainError> {
    match one(event, name, "an activity URL is http:// or https://")? {
        None => Ok(None),
        Some(value) if http_url(value) => Ok(Some(value.to_owned())),
        Some(_) => Err(invalid("an activity URL is http:// or https://")),
    }
}

fn optional_time(event: &Event, name: &str) -> Result<Option<u64>, DomainError> {
    match one(event, name, "an activity time is unix seconds")? {
        None => Ok(None),
        Some(value) => Ok(Some(whole(value, "an activity time is unix seconds")?)),
    }
}

fn counts(event: &Event) -> Result<(Option<u64>, Option<u64>), DomainError> {
    let current = match one(
        event,
        "current_participants",
        "a participant count is a whole number",
    )? {
        None => None,
        Some(value) => Some(whole(value, "a participant count is a whole number")?),
    };
    let total = match one(
        event,
        "total_participants",
        "a participant count is a whole number",
    )? {
        None => None,
        Some(value) => Some(whole(value, "a participant count is a whole number")?),
    };
    if let (Some(current), Some(total)) = (current, total)
        && current > total
    {
        return Err(invalid("current participants do not exceed the total"));
    }
    Ok((current, total))
}

fn hashtags(event: &Event) -> Result<Vec<String>, DomainError> {
    let mut tags = Vec::new();
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("t")) {
        let Some(value) = tag.value() else {
            return Err(invalid("a hashtag is lowercase"));
        };
        if value.is_empty()
            || value.len() > 64
            || value
                .chars()
                .any(|char| char.is_ascii_uppercase() || char.is_whitespace())
            || tags.iter().any(|seen| seen == value)
        {
            return Err(invalid("a hashtag is lowercase"));
        }
        tags.push(value.to_owned());
    }
    Ok(tags)
}

fn relays(event: &Event) -> Result<Vec<String>, DomainError> {
    let found: Vec<_> = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("relays"))
        .collect();
    if found.len() > 1 {
        return Err(invalid("an activity lists its relays once"));
    }
    let Some(tag) = found.first() else {
        return Ok(Vec::new());
    };
    let mut relays = Vec::new();
    for value in tag.as_slice().iter().skip(1) {
        if !relay_url(value) || value.is_empty() || relays.iter().any(|seen| seen == value) {
            return Err(invalid("an activity relay is ws:// or wss://"));
        }
        relays.push(value.clone());
    }
    if relays.is_empty() {
        return Err(invalid("an activity relay is ws:// or wss://"));
    }
    Ok(relays)
}

fn pinned(event: &Event) -> Result<Vec<String>, DomainError> {
    let mut ids = Vec::new();
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("pinned")) {
        let Some(value) = tag.value() else {
            return Err(invalid("a pinned chat is an event id"));
        };
        decode_lower_hex::<32>(value, "pinned chat")
            .map_err(|_| invalid("a pinned chat is an event id"))?;
        if ids.contains(&value.to_owned()) {
            return Err(invalid("a pinned chat is listed once"));
        }
        ids.push(value.to_owned());
    }
    Ok(ids)
}

pub fn activity_address(kind: u16, pubkey: &str, identifier: &str) -> String {
    format!("{kind}:{pubkey}:{identifier}")
}

fn digest(address: &str) -> [u8; 32] {
    Sha256::digest(address.as_bytes()).into()
}

/// Sign the activity address with the participant's key.
pub fn participation_proof(secret: &SecretKey, address: &str) -> String {
    let keypair = Keypair::from_secret_key(&Secp256k1::signing_only(), secret);
    let signature = Secp256k1::signing_only().sign_schnorr_no_aux_rand(&digest(address), &keypair);
    encode_lower_hex(signature.as_ref())
}

fn proof_agrees(pubkey: &str, address: &str, proof: &str) -> Result<(), DomainError> {
    let bytes = decode_lower_hex::<32>(pubkey, "participant")
        .map_err(|_| invalid("a participant is a pubkey"))?;
    let key =
        XOnlyPublicKey::from_byte_array(bytes).map_err(|_| invalid("a participant is a pubkey"))?;
    let signature = Signature::from_byte_array(
        decode_lower_hex::<64>(proof, "participation proof")
            .map_err(|_| invalid("a participation proof signs the activity address"))?,
    );
    Secp256k1::verification_only()
        .verify_schnorr(&signature, &digest(address), &key)
        .map_err(|_| invalid("a participation proof signs the activity address"))
}

fn participants(
    event: &Event,
    address: &str,
    role_required: bool,
) -> Result<Vec<LiveParticipant>, DomainError> {
    let mut people = Vec::new();
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("p")) {
        let parts = tag.as_slice();
        if parts.len() < 2 || parts.len() > 5 {
            return Err(invalid("a participant tag names a pubkey"));
        }
        decode_lower_hex::<32>(&parts[1], "participant")
            .map_err(|_| invalid("a participant is a pubkey"))?;
        if people
            .iter()
            .any(|person: &LiveParticipant| person.pubkey == parts[1])
        {
            return Err(invalid("a participant is listed once"));
        }
        let relay = match parts.get(2) {
            None => None,
            Some(value) if relay_url(value) => {
                if value.is_empty() {
                    None
                } else {
                    Some(value.clone())
                }
            }
            Some(_) => return Err(invalid("a participant relay is ws:// or wss://")),
        };
        let role = match parts.get(3) {
            None if role_required => return Err(invalid("a room participant has a role")),
            None => None,
            Some(value) => {
                if value.is_empty()
                    || value.len() > 32
                    || value.chars().any(char::is_control)
                    || value.chars().any(char::is_whitespace)
                {
                    return Err(invalid("a participant role is a short name"));
                }
                Some(value.clone())
            }
        };
        let agreed = match parts.get(4) {
            None => false,
            Some(proof) => {
                proof_agrees(&parts[1], address, proof)?;
                true
            }
        };
        people.push(LiveParticipant {
            pubkey: parts[1].clone(),
            relay,
            role,
            agreed,
        });
    }
    Ok(people)
}

fn span(starts: Option<u64>, ends: Option<u64>) -> Result<(), DomainError> {
    if let (Some(starts), Some(ends)) = (starts, ends)
        && ends < starts
    {
        return Err(invalid("an activity ends at or after it starts"));
    }
    Ok(())
}

fn address_tag(
    event: &Event,
    expected_kind: u16,
) -> Result<(ReplacementAddress, Option<String>, bool), DomainError> {
    let tags: Vec<_> = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("a"))
        .collect();
    if tags.len() != 1 {
        return Err(invalid("an activity link has one a tag"));
    }
    let parts = tags[0].as_slice();
    if parts.len() < 2 || parts.len() > 4 {
        return Err(invalid("an activity link has one a tag"));
    }
    let address = ReplacementAddress::from_str(&parts[1])
        .map_err(|_| invalid("an activity link names an addressable event"))?;
    if address.kind != expected_kind {
        return Err(invalid("an activity link names the parent kind"));
    }
    let relay = match parts.get(2) {
        None => None,
        Some(value) if relay_url(value) => {
            if value.is_empty() {
                None
            } else {
                Some(value.clone())
            }
        }
        Some(_) => return Err(invalid("an activity relay is ws:// or wss://")),
    };
    let root = match parts.get(3) {
        None => false,
        Some(value) if value == "root" => true,
        Some(_) => return Err(invalid("an activity link marker is root")),
    };
    Ok((address, relay, root))
}

/// Read a kind `30311` live stream.
pub fn open_live_stream(event: &Event) -> Result<LiveStream, DomainError> {
    if event.kind != STREAM_KIND {
        return Err(invalid("a live stream has kind 30311"));
    }
    let identifier = identifier(required(event, "d", "a live stream has one identifier")?)?;
    let starts = optional_time(event, "starts")?;
    let ends = optional_time(event, "ends")?;
    span(starts, ends)?;
    let (current_participants, total_participants) = counts(event)?;
    let address = activity_address(STREAM_KIND, &event.pubkey, &identifier);
    Ok(LiveStream {
        identifier,
        title: one(event, "title", "a live stream title is text")?
            .map(|value| text(value, 1_024, "a live stream title is text"))
            .transpose()?,
        summary: one(event, "summary", "a live stream summary is text")?
            .map(|value| text(value, 4_096, "a live stream summary is text"))
            .transpose()?,
        image: optional_url(event, "image")?,
        streaming: optional_url(event, "streaming")?,
        recording: optional_url(event, "recording")?,
        starts,
        ends,
        status: one(event, "status", "a live status is planned, live, or ended")?
            .map(live_status)
            .transpose()?,
        current_participants,
        total_participants,
        participants: participants(event, &address, false)?,
        relays: relays(event)?,
        pinned: pinned(event)?,
        hashtags: hashtags(event)?,
        content: event.content.clone(),
    })
}

/// Read a kind `1311` live chat message.
pub fn open_live_chat(event: &Event) -> Result<LiveChat, DomainError> {
    if event.kind != CHAT_KIND {
        return Err(invalid("a live chat message has kind 1311"));
    }
    if event.content.is_empty() {
        return Err(invalid("a live chat message has text"));
    }
    let (activity, relay, root) = address_tag(event, STREAM_KIND)?;
    let parent = match one(event, "e", "a live chat parent is an event id")? {
        None => None,
        Some(value) => {
            decode_lower_hex::<32>(value, "chat parent")
                .map_err(|_| invalid("a live chat parent is an event id"))?;
            Some(value.to_owned())
        }
    };
    let mut quotes = Vec::new();
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("q")) {
        let Some(value) = tag.value() else {
            return Err(invalid("a live chat quote is an event id or address"));
        };
        if decode_lower_hex::<32>(value, "quote").is_err()
            && ReplacementAddress::from_str(value).is_err()
        {
            return Err(invalid("a live chat quote is an event id or address"));
        }
        quotes.push(value.to_owned());
    }
    Ok(LiveChat {
        activity,
        relay,
        root,
        parent,
        quotes,
        content: event.content.clone(),
    })
}

/// Read a kind `30312` meeting room.
pub fn open_meeting_room(event: &Event) -> Result<MeetingRoom, DomainError> {
    if event.kind != ROOM_KIND {
        return Err(invalid("a meeting room has kind 30312"));
    }
    let identifier = identifier(required(event, "d", "a meeting room has one identifier")?)?;
    let address = activity_address(ROOM_KIND, &event.pubkey, &identifier);
    let people = participants(event, &address, true)?;
    if !people
        .iter()
        .any(|person| person.role.as_deref() == Some("Host"))
    {
        return Err(invalid("a meeting room has one Host"));
    }
    Ok(MeetingRoom {
        identifier,
        name: text(
            required(event, "room", "a meeting room has a name")?,
            1_024,
            "a meeting room has a name",
        )?,
        summary: one(event, "summary", "a meeting room summary is text")?
            .map(|value| text(value, 4_096, "a meeting room summary is text"))
            .transpose()?,
        image: optional_url(event, "image")?,
        status: room_status(required(
            event,
            "status",
            "a room status is open, private, or closed",
        )?)?,
        service: {
            let value = required(event, "service", "a meeting room has a service URL")?;
            if !http_url(value) {
                return Err(invalid(
                    "a meeting room service is an http:// or https:// URL",
                ));
            }
            value.to_owned()
        },
        endpoint: optional_url(event, "endpoint")?,
        participants: people,
        relays: relays(event)?,
        hashtags: hashtags(event)?,
        content: event.content.clone(),
    })
}

/// Read a kind `30313` meeting.
pub fn open_meeting(event: &Event) -> Result<Meeting, DomainError> {
    if event.kind != MEETING_KIND {
        return Err(invalid("a meeting has kind 30313"));
    }
    let identifier = identifier(required(event, "d", "a meeting has one identifier")?)?;
    let (room, room_relay, _) = address_tag(event, ROOM_KIND)?;
    let starts = whole(
        required(event, "starts", "a meeting has a start time")?,
        "a meeting has a start time",
    )?;
    let ends = optional_time(event, "ends")?;
    span(Some(starts), ends)?;
    let (current_participants, total_participants) = counts(event)?;
    let address = activity_address(MEETING_KIND, &event.pubkey, &identifier);
    Ok(Meeting {
        identifier,
        room,
        room_relay,
        title: text(
            required(event, "title", "a meeting has a title")?,
            1_024,
            "a meeting has a title",
        )?,
        summary: one(event, "summary", "a meeting summary is text")?
            .map(|value| text(value, 4_096, "a meeting summary is text"))
            .transpose()?,
        image: optional_url(event, "image")?,
        starts,
        ends,
        status: live_status(required(
            event,
            "status",
            "a live status is planned, live, or ended",
        )?)?,
        current_participants,
        total_participants,
        participants: participants(event, &address, false)?,
        content: event.content.clone(),
    })
}

/// Read a kind `10312` presence event.
pub fn open_presence(event: &Event) -> Result<Presence, DomainError> {
    if event.kind != PRESENCE_KIND {
        return Err(invalid("a room presence has kind 10312"));
    }
    let (room, relay, _) = address_tag(event, ROOM_KIND)?;
    let hand_raised = match one(event, "hand", "a raised hand is 0 or 1")? {
        None => false,
        Some("1") => true,
        Some("0") => false,
        Some(_) => return Err(invalid("a raised hand is 0 or 1")),
    };
    Ok(Presence {
        room,
        relay,
        hand_raised,
    })
}

/// A `live` stream with no update for more than one hour is stale.
pub fn live_status_is_stale(status: LiveStatus, updated_at: u64, now: u64) -> bool {
    status == LiveStatus::Live && now > updated_at.saturating_add(STALE_AFTER_SECONDS)
}

/// Whether a presence event is still inside the caller's window.
pub fn presence_is_fresh(created_at: u64, now: u64, window_seconds: u64) -> bool {
    created_at <= now && now.saturating_sub(created_at) <= window_seconds
}

#[cfg(test)]
mod tests {
    use super::decode_lower_hex;
    use super::*;
    use crate::domain::{
        DomainError, EventClass, RelaySigner, ReplacementDecision, Tag, compare_replacement,
    };

    fn signer(byte: &str) -> RelaySigner {
        RelaySigner::from_secret_hex(&byte.repeat(32)).unwrap()
    }

    fn secret(byte: &str) -> SecretKey {
        SecretKey::from_byte_array(decode_lower_hex::<32>(&byte.repeat(32), "key").unwrap())
            .unwrap()
    }

    #[test]
    fn a_live_stream_replaces_and_a_chat_names_it() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../nips/official/53.md"
        ))
        .unwrap();
        assert!(text.contains("30311"));
        assert!(text.contains("1311"));
        assert!(text.contains("30312"));
        assert!(text.contains("10312"));
        let row = crate::lane::PROVEN
            .iter()
            .find(|row| row.file == "53.md")
            .unwrap();
        assert_eq!(row.status, "configured-and-proven");
        assert!(
            crate::lane::SHAPES
                .iter()
                .all(|shape| shape.file != "53.md")
        );

        let author = signer("53");
        let host = signer("54");
        let identifier = "demo-cf-stream";
        let address = activity_address(STREAM_KIND, author.pubkey(), identifier);
        let proof = participation_proof(&secret("54"), &address);
        let stream = author.sign(
            1_687_182_672,
            STREAM_KIND,
            vec![
                Tag::new(vec!["d".into(), identifier.into()]),
                Tag::new(vec!["title".into(), "Adult Swim Metalocalypse".into()]),
                Tag::new(vec![
                    "summary".into(),
                    "Live stream from IPTV-ORG collection".into(),
                ]),
                Tag::new(vec![
                    "streaming".into(),
                    "https://adultswim-vodlive.cdn.turner.com/live/metalocalypse/stream.m3u8"
                        .into(),
                ]),
                Tag::new(vec!["starts".into(), "1687182672".into()]),
                Tag::new(vec!["status".into(), "live".into()]),
                Tag::new(vec!["t".into(), "animation".into()]),
                Tag::new(vec!["t".into(), "iptv".into()]),
                Tag::new(vec![
                    "image".into(),
                    "https://i.imgur.com/CaKq6Mt.png".into(),
                ]),
                Tag::new(vec![
                    "p".into(),
                    host.pubkey().to_owned(),
                    "wss://provider.example/".into(),
                    "Host".into(),
                    proof,
                ]),
                Tag::new(vec![
                    "p".into(),
                    signer("55").pubkey().to_owned(),
                    "wss://speaker.example/".into(),
                    "Speaker".into(),
                ]),
            ],
            String::new(),
        );
        stream.validate_structure().unwrap();
        assert_eq!(stream.class(), EventClass::Addressable);
        let opened = open_live_stream(&stream).unwrap();
        assert_eq!(opened.identifier, identifier);
        assert_eq!(opened.status, Some(LiveStatus::Live));
        assert!(opened.participants[0].agreed);
        assert_eq!(opened.participants[0].role.as_deref(), Some("Host"));
        assert!(!opened.participants[1].agreed);
        assert!(!live_status_is_stale(
            LiveStatus::Live,
            stream.created_at,
            stream.created_at + STALE_AFTER_SECONDS
        ));
        assert!(live_status_is_stale(
            LiveStatus::Live,
            stream.created_at,
            stream.created_at + STALE_AFTER_SECONDS + 1
        ));

        let chat = signer("56").sign(
            1_687_286_726,
            CHAT_KIND,
            vec![Tag::new(vec![
                "a".into(),
                address.clone(),
                String::new(),
                "root".into(),
            ])],
            "Zaps to live streams is beautiful.".into(),
        );
        chat.validate_structure().unwrap();
        assert_eq!(chat.class(), EventClass::Regular);
        let message = open_live_chat(&chat).unwrap();
        assert!(message.root);
        assert_eq!(message.activity.kind, STREAM_KIND);
        assert_eq!(message.activity.identifier, identifier);
        assert_eq!(message.activity.pubkey, author.pubkey());
        assert!(matches!(
            compare_replacement(&chat, &chat),
            Err(DomainError::NotReplaceable)
        ));

        let updated = author.sign(
            1_687_286_800,
            STREAM_KIND,
            vec![
                Tag::new(vec!["d".into(), identifier.into()]),
                Tag::new(vec!["status".into(), "ended".into()]),
                Tag::new(vec![
                    "recording".into(),
                    "https://cdn.example/metalocalypse.mp4".into(),
                ]),
                Tag::new(vec!["pinned".into(), chat.id.clone()]),
            ],
            String::new(),
        );
        updated.validate_structure().unwrap();
        assert_eq!(
            compare_replacement(&stream, &updated),
            Ok(ReplacementDecision::ReplaceCurrent)
        );
        assert_eq!(open_live_stream(&updated).unwrap().pinned, vec![chat.id]);

        let room_id = "main-conference-room";
        let room = author.sign(
            1_687_300_000,
            ROOM_KIND,
            vec![
                Tag::new(vec!["d".into(), room_id.into()]),
                Tag::new(vec!["room".into(), "Main Conference Hall".into()]),
                Tag::new(vec!["status".into(), "open".into()]),
                Tag::new(vec![
                    "service".into(),
                    "https://meet.example.com/room".into(),
                ]),
                Tag::new(vec![
                    "p".into(),
                    host.pubkey().to_owned(),
                    "wss://provider.example/".into(),
                    "Host".into(),
                ]),
            ],
            String::new(),
        );
        room.validate_structure().unwrap();
        let opened_room = open_meeting_room(&room).unwrap();
        assert_eq!(opened_room.status, RoomStatus::Open);
        assert_eq!(opened_room.participants[0].role.as_deref(), Some("Host"));

        let meeting = author.sign(
            1_687_300_100,
            MEETING_KIND,
            vec![
                Tag::new(vec!["d".into(), "annual-meeting-2025".into()]),
                Tag::new(vec![
                    "a".into(),
                    activity_address(ROOM_KIND, author.pubkey(), room_id),
                    "wss://relay.example".into(),
                ]),
                Tag::new(vec!["title".into(), "Annual Company Meeting 2025".into()]),
                Tag::new(vec!["starts".into(), "1687300100".into()]),
                Tag::new(vec!["ends".into(), "1687303700".into()]),
                Tag::new(vec!["status".into(), "planned".into()]),
            ],
            String::new(),
        );
        meeting.validate_structure().unwrap();
        let opened_meeting = open_meeting(&meeting).unwrap();
        assert_eq!(opened_meeting.room.kind, ROOM_KIND);
        assert_eq!(opened_meeting.room.identifier, room_id);
        assert_eq!(opened_meeting.status, LiveStatus::Planned);

        let listener = signer("57");
        let presence = listener.sign(
            1_687_300_200,
            PRESENCE_KIND,
            vec![
                Tag::new(vec![
                    "a".into(),
                    activity_address(ROOM_KIND, author.pubkey(), room_id),
                    "wss://relay.example".into(),
                    "root".into(),
                ]),
                Tag::new(vec!["hand".into(), "1".into()]),
            ],
            String::new(),
        );
        presence.validate_structure().unwrap();
        assert_eq!(presence.class(), EventClass::Replaceable);
        let here = open_presence(&presence).unwrap();
        assert!(here.hand_raised);
        assert_eq!(here.room.identifier, room_id);
        assert!(presence_is_fresh(
            presence.created_at,
            presence.created_at + 60,
            60
        ));
        assert!(!presence_is_fresh(
            presence.created_at,
            presence.created_at + 61,
            60
        ));
        let elsewhere = listener.sign(
            1_687_300_300,
            PRESENCE_KIND,
            vec![Tag::new(vec![
                "a".into(),
                activity_address(ROOM_KIND, author.pubkey(), "other-room"),
            ])],
            String::new(),
        );
        elsewhere.validate_structure().unwrap();
        assert_eq!(
            compare_replacement(&presence, &elsewhere),
            Ok(ReplacementDecision::ReplaceCurrent)
        );

        let refused = author.sign(
            1_687_300_400,
            STREAM_KIND,
            vec![
                Tag::new(vec!["d".into(), "forged".into()]),
                Tag::new(vec![
                    "p".into(),
                    host.pubkey().to_owned(),
                    "wss://provider.example/".into(),
                    "Host".into(),
                    participation_proof(&secret("54"), &address),
                ]),
            ],
            String::new(),
        );
        assert!(refused.validate_structure().is_err());
        let unhosted = author.sign(
            1_687_300_500,
            ROOM_KIND,
            vec![
                Tag::new(vec!["d".into(), "empty".into()]),
                Tag::new(vec!["room".into(), "Empty".into()]),
                Tag::new(vec!["status".into(), "open".into()]),
                Tag::new(vec![
                    "service".into(),
                    "https://meet.example.com/empty".into(),
                ]),
                Tag::new(vec![
                    "p".into(),
                    host.pubkey().to_owned(),
                    "wss://provider.example/".into(),
                    "Speaker".into(),
                ]),
            ],
            String::new(),
        );
        assert!(unhosted.validate_structure().is_err());
    }
}
