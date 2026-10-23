//! Room tokens (#12263): short-lived, signed admissions to one `coder-sync`
//! room.
//!
//! A browser can't set headers on a WebSocket, so its credential rides the
//! URL query, as the phone's session token does. A room token keeps the
//! account's session out of that URL: it names one room kind, one room, the
//! account, and an expiry a few minutes out, so a token seen in a log opens
//! nothing else and soon nothing at all. The website signs it for a
//! signed-in browser with its room-token key; `coder-sync` checks the
//! signature against the public keys it is configured with, without asking
//! the account service.
//!
//! A token is [`PREFIX`] and then a signed Nostr event of kind [`KIND`] as
//! JSON in URL-safe base64 without padding. Its tags are exactly
//! `["room", KIND, ID]`, `["account", ACCOUNT]`, and `["expiration", UNIX]`,
//! and its content is empty.

use crate::{Code, Error, Result};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use nostr::domain::{Event, RelaySigner, Tag};

/// What every room token starts with.
pub const PREFIX: &str = "ort1_";
/// The signed event's kind (ephemeral range, never published).
pub const KIND: u16 = 27_236;
/// The longest life a token may claim, in seconds.
pub const MAX_LIFETIME: u64 = 600;
/// The life the website gives a token, in seconds.
pub const LIFETIME: u64 = 300;
/// How far ahead of the checker's clock a token may be signed, in seconds.
const SKEW: u64 = 30;
/// The longest token a checker reads.
pub const MAX_LEN: usize = 4096;

/// The `coder-sync` room kinds a token can open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Room {
    /// `/registry/{org}/ws`: the account's chats, devices, and projects.
    Registry,
    /// `/device/{id}/ws`, as a client: one computer's host RPC.
    Device,
    /// `/chat2/{id}/ws`: one chat's document.
    Chat,
    /// Native website adapter: bounded visit-document rows, never a device socket.
    Visit,
}

impl Room {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Registry => "registry",
            Self::Device => "device",
            Self::Chat => "chat2",
            Self::Visit => "visit",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "registry" => Some(Self::Registry),
            "device" => Some(Self::Device),
            "chat2" | "chat" => Some(Self::Chat),
            "visit" => Some(Self::Visit),
            _ => None,
        }
    }

    /// Browser rooms open only their socket. The native visit adapter instead
    /// opens `/visit/{id}/rows`; it cannot open sockets or checkpoints.
    #[must_use]
    pub fn of_path(path: &str) -> Option<(Self, &str)> {
        let mut parts = path.strip_prefix('/')?.split('/');
        let (kind, id, last) = (parts.next()?, parts.next()?, parts.next()?);
        if kind == "visit" && last == "rows" && parts.next().is_none() {
            return Some((Self::Visit, id));
        }
        if last != "ws" || parts.next().is_some() {
            return None;
        }
        let room = match kind {
            "registry" => Self::Registry,
            "device" => Self::Device,
            "chat2" => Self::Chat,
            _ => return None,
        };
        Some((room, id))
    }
}

/// What a checked token admits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Admission {
    pub account: String,
    pub room: Room,
    pub id: String,
    /// Unix seconds; a socket it opened closes then.
    pub expires: u64,
}

fn refused() -> Error {
    Error::new(Code::Forbidden, "This room token isn't valid here.")
}

fn visit_scope(room: Room, account: &str, id: &str, lifetime: u64) -> bool {
    room != Room::Visit
        || (account == "verse_visits"
            && lifetime <= 30
            && id.len() == 67
            && id.starts_with("vv_")
            && id[3..]
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()))
}

fn valid_account(account: &str) -> bool {
    !account.is_empty()
        && account.len() <= 128
        && account
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 256 && !id.contains('/') && !id.chars().any(char::is_control)
}

/// Whether `token` is a room token rather than an account session.
#[must_use]
pub fn is_room_token(token: &str) -> bool {
    token.starts_with(PREFIX)
}

/// Sign a token for `account` to open `room` `id` from `now` for
/// `lifetime` seconds (at most [`MAX_LIFETIME`]) with the website's key.
pub fn mint(
    secret_hex: &str,
    account: &str,
    room: Room,
    id: &str,
    now: u64,
    lifetime: u64,
) -> Result<String> {
    if !valid_account(account)
        || !valid_id(id)
        || lifetime == 0
        || lifetime > MAX_LIFETIME
        || !visit_scope(room, account, id, lifetime)
    {
        return Err(Error::new(Code::Malformed, "This room can't be opened."));
    }
    let signer = RelaySigner::from_secret_hex(secret_hex)
        .map_err(|_| Error::new(Code::Unavailable, "Room tokens aren't set up here."))?;
    let event = signer.sign(
        now,
        KIND,
        vec![
            Tag::new(vec!["room".into(), room.as_str().into(), id.into()]),
            Tag::new(vec!["account".into(), account.into()]),
            Tag::new(vec!["expiration".into(), (now + lifetime).to_string()]),
        ],
        String::new(),
    );
    let json = serde_json::to_vec(&event).map_err(|_| refused())?;
    Ok(format!("{PREFIX}{}", URL_SAFE_NO_PAD.encode(json)))
}

/// The public key (lowercase hex, x-only) of a website room-token key.
pub fn public_key(secret_hex: &str) -> Result<String> {
    RelaySigner::from_secret_hex(secret_hex)
        .map(|signer| signer.pubkey().to_owned())
        .map_err(|_| Error::new(Code::Malformed, "That room-token key isn't valid."))
}

/// Check `token` at `now` against the signing keys `keys` (lowercase hex).
pub fn verify(token: &str, keys: &[String], now: u64) -> Result<Admission> {
    if token.len() > MAX_LEN {
        return Err(refused());
    }
    let body = token.strip_prefix(PREFIX).ok_or_else(refused)?;
    let bytes = URL_SAFE_NO_PAD.decode(body).map_err(|_| refused())?;
    let event: Event = serde_json::from_slice(&bytes).map_err(|_| refused())?;
    if event.kind != KIND || !event.content.is_empty() || event.tags.len() != 3 {
        return Err(refused());
    }
    if !keys.iter().any(|key| *key == event.pubkey) {
        return Err(refused());
    }
    event.validate_crypto().map_err(|_| refused())?;
    let one = |name: &str| -> Result<&[String]> {
        let found: Vec<_> = event
            .tags
            .iter()
            .filter(|tag| tag.name() == Some(name))
            .collect();
        match found.as_slice() {
            [tag] => Ok(tag.as_slice()),
            _ => Err(refused()),
        }
    };
    let (room, id) = match one("room")? {
        [_, kind, id] => (Room::parse(kind).ok_or_else(refused)?, id.clone()),
        _ => return Err(refused()),
    };
    let account = match one("account")? {
        [_, account] => account.clone(),
        _ => return Err(refused()),
    };
    let expires = match one("expiration")? {
        [_, at] => at.parse::<u64>().map_err(|_| refused())?,
        _ => return Err(refused()),
    };
    if !valid_account(&account)
        || !valid_id(&id)
        || event.created_at > now.saturating_add(SKEW)
        || expires <= event.created_at
        || expires - event.created_at > MAX_LIFETIME
        || !visit_scope(
            room,
            &account,
            &id,
            expires.saturating_sub(event.created_at),
        )
    {
        return Err(refused());
    }
    if expires <= now {
        return Err(Error::new(Code::Expired, "This room token has expired."));
    }
    Ok(Admission {
        account,
        room,
        id,
        expires,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "0101010101010101010101010101010101010101010101010101010101010101";
    const OTHER: &str = "0202020202020202020202020202020202020202020202020202020202020202";

    fn keys() -> Vec<String> {
        vec![public_key(SECRET).unwrap()]
    }

    #[test]
    fn a_token_opens_its_own_room_until_it_expires() {
        let token = mint(SECRET, "acct_1", Room::Registry, "acct_1", 1_000, LIFETIME).unwrap();
        assert!(is_room_token(&token));
        assert!(token.len() < 1024, "{}", token.len());
        let admitted = verify(&token, &keys(), 1_010).unwrap();
        assert_eq!(
            admitted,
            Admission {
                account: "acct_1".into(),
                room: Room::Registry,
                id: "acct_1".into(),
                expires: 1_000 + LIFETIME,
            }
        );
        let expired = verify(&token, &keys(), 1_000 + LIFETIME).unwrap_err();
        assert_eq!(expired.code, Code::Expired);
    }

    #[test]
    fn a_token_from_another_key_or_changed_in_transit_is_refused() {
        let token = mint(OTHER, "acct_1", Room::Device, "mac-1", 1_000, 60).unwrap();
        assert_eq!(
            verify(&token, &keys(), 1_000).unwrap_err().code,
            Code::Forbidden
        );
        let token = mint(SECRET, "acct_1", Room::Device, "mac-1", 1_000, 60).unwrap();
        let mut event: Event =
            serde_json::from_slice(&URL_SAFE_NO_PAD.decode(&token[PREFIX.len()..]).unwrap())
                .unwrap();
        event.tags[0] = Tag::new(vec!["room".into(), "device".into(), "mac-2".into()]);
        let changed = format!(
            "{PREFIX}{}",
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(&event).unwrap())
        );
        assert!(verify(&changed, &keys(), 1_000).is_err());
        assert!(verify("sess_abc", &keys(), 1_000).is_err());
        assert!(verify(&format!("{PREFIX}!!"), &keys(), 1_000).is_err());
    }

    #[test]
    fn lifetimes_and_names_are_bounded() {
        assert!(mint(SECRET, "acct_1", Room::Chat, "c1", 1_000, MAX_LIFETIME + 1).is_err());
        assert!(mint(SECRET, "acct_1", Room::Chat, "c1", 1_000, 0).is_err());
        assert!(mint(SECRET, "a/b", Room::Chat, "c1", 1_000, 60).is_err());
        assert!(mint(SECRET, "acct_1", Room::Chat, "c/1", 1_000, 60).is_err());
        assert!(mint("not-hex", "acct_1", Room::Chat, "c1", 1_000, 60).is_err());
        // Signed in the future beyond the allowed skew.
        let token = mint(SECRET, "acct_1", Room::Chat, "c1", 2_000, 60).unwrap();
        assert!(verify(&token, &keys(), 1_000).is_err());
        assert!(verify(&token, &keys(), 1_990).is_ok());
    }

    #[test]
    fn only_a_room_socket_is_a_room() {
        assert_eq!(
            Room::of_path("/registry/org_1/ws"),
            Some((Room::Registry, "org_1"))
        );
        assert_eq!(
            Room::of_path("/device/mac-1/ws"),
            Some((Room::Device, "mac-1"))
        );
        assert_eq!(Room::of_path("/chat2/c1/ws"), Some((Room::Chat, "c1")));
        for path in [
            "/registry/org_1/rows",
            "/registry/org_1/push",
            "/device/mac-1/key",
            "/device/mac-1/grants/acct",
            "/device/mac-1/ws/extra",
            "/v1/notify",
            "/blob/x/1",
            "/",
        ] {
            assert_eq!(Room::of_path(path), None, "{path}");
        }
    }

    #[test]
    fn visit_adapter_tokens_are_short_and_cannot_open_other_room_paths() {
        let id = format!("vv_{}", "a".repeat(64));
        let token = mint(SECRET, "verse_visits", Room::Visit, &id, 1000, 30).unwrap();
        assert_eq!(verify(&token, &keys(), 1001).unwrap().room, Room::Visit);
        assert_eq!(
            Room::of_path(&format!("/visit/{id}/rows")),
            Some((Room::Visit, id.as_str()))
        );
        assert_eq!(
            Room::of_path(&format!("/chat2/{id}/ws")),
            Some((Room::Chat, id.as_str()))
        );
        assert!(mint(SECRET, "acct_1", Room::Visit, &id, 1000, 30).is_err());
        assert!(mint(SECRET, "verse_visits", Room::Visit, &id, 1000, 31).is_err());
        assert!(
            mint(
                SECRET,
                "verse_visits",
                Room::Visit,
                "ordinary_chat",
                1000,
                30
            )
            .is_err()
        );
        assert_eq!(Room::of_path(&format!("/visit/{id}/checkpoint")), None);
        assert_eq!(Room::of_path(&format!("/visit/{id}/ws")), None);
    }
}
