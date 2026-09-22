//! NIP-38 user statuses.
//!
//! A kind `30315` event is an addressable live status: the `d` tag is
//! the status type — `general` and `music` are defined, any other
//! string is a client-specific type — and the content is the status
//! text. An `r`, `p`, `e`, or `a` tag may link a URL, profile, note,
//! or addressable event; an `expiration` tag ends it. An empty
//! content clears the status. NIP-38 is a draft, so the kind is not
//! added to the NIP-11 list.

use super::hex::decode_lower_hex;
use super::{DomainError, Event};

const USER_STATUS_KIND: u16 = 30_315;

/// The status type a `d` tag names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StatusType {
    /// General activity: working, hiking, out of office.
    General,
    /// Live-streamed now-playing track.
    Music,
    /// A type this NIP does not define.
    Other(String),
}

/// A kind `30315` live status.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserStatus {
    /// The `d`-named status type.
    pub status_type: StatusType,
    /// The status text; empty clears the status.
    pub text: String,
    /// The linked URL, profile, note, or address, when present.
    pub link: Option<String>,
    /// The NIP-40 expiry, when declared.
    pub expires_at: Option<u64>,
}

/// Read a kind `30315` event into a live status.
///
/// # Errors
///
/// Returns `DomainError::InvalidEvent` for another kind, a missing or
/// empty `d`, or a link tag whose value is malformed for its kind.
pub fn open_user_status(event: &Event) -> Result<UserStatus, DomainError> {
    if event.kind != USER_STATUS_KIND {
        return Err(invalid("a user status is kind 30315"));
    }
    let identifier = event
        .distinct_parameter()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid("a user status needs a d tag naming the status type"))?;
    let status_type = match identifier {
        "general" => StatusType::General,
        "music" => StatusType::Music,
        other => StatusType::Other(other.to_string()),
    };
    let link = event
        .tags
        .iter()
        .find_map(|tag| {
            let name = tag.name()?;
            let value = tag.value()?;
            let ok = match name {
                "r" => {
                    value.starts_with("http://")
                        || value.starts_with("https://")
                        || value.contains(':')
                }
                "p" | "e" => decode_lower_hex::<32>(value, "link").is_ok(),
                "a" => {
                    let parts: Vec<&str> = value.split(':').collect();
                    parts.len() == 3
                        && parts[0].parse::<u16>().is_ok()
                        && decode_lower_hex::<32>(parts[1], "a pubkey").is_ok()
                }
                _ => return None,
            };
            ok.then(|| value.to_string())
        })
        .map(Some)
        .unwrap_or(None);
    Ok(UserStatus {
        status_type,
        text: event.content.clone(),
        link,
        expires_at: event.expiration(),
    })
}

/// Whether the status is live now: not cleared and not expired.
#[must_use]
pub fn is_live(status: &UserStatus, now: u64) -> bool {
    !status.text.is_empty() && status.expires_at.is_none_or(|at| at > now)
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{RelaySigner, Tag};

    fn sign(tags: Vec<Tag>, content: &str) -> Event {
        let signer = RelaySigner::from_secret_hex(&"42".repeat(32)).unwrap();
        signer.sign(1_700_000_000, USER_STATUS_KIND, tags, content.to_string())
    }

    #[test]
    fn a_status_names_its_type_and_may_link_and_expire() {
        let music = sign(
            vec![
                Tag::new(vec!["d".into(), "music".into()]),
                Tag::new(vec!["r".into(), "spotify:search:Intergalatic".into()]),
                Tag::new(vec!["expiration".into(), "1692845589".into()]),
            ],
            "Intergalatic - Beastie Boys",
        );
        let status = open_user_status(&music).unwrap();
        assert_eq!(status.status_type, StatusType::Music);
        assert_eq!(status.text, "Intergalatic - Beastie Boys");
        assert_eq!(status.link.as_deref(), Some("spotify:search:Intergalatic"));
        assert_eq!(status.expires_at, Some(1_692_845_589));
        assert!(is_live(&status, 1_692_800_000));
        assert!(!is_live(&status, 1_692_845_590));

        let general = sign(
            vec![Tag::new(vec!["d".into(), "general".into()])],
            "Sign up for nostrasia!",
        );
        assert_eq!(
            open_user_status(&general).unwrap().status_type,
            StatusType::General
        );

        let custom = sign(
            vec![Tag::new(vec!["d".into(), "meeting".into()])],
            "in a call",
        );
        assert_eq!(
            open_user_status(&custom).unwrap().status_type,
            StatusType::Other("meeting".to_string())
        );

        let cleared = sign(vec![Tag::new(vec!["d".into(), "general".into()])], "");
        assert!(!is_live(&open_user_status(&cleared).unwrap(), 0));
    }

    #[test]
    fn malformed_statuses_are_refused() {
        assert!(open_user_status(&sign(Vec::new(), "no d")).is_err());
        assert!(open_user_status(&sign(vec![Tag::new(vec!["d".into()])], "x")).is_err());
        let wrong_kind = {
            let signer = RelaySigner::from_secret_hex(&"42".repeat(32)).unwrap();
            signer.sign(1_700_000_000, 1, Vec::new(), "note".to_string())
        };
        assert!(open_user_status(&wrong_kind).is_err());
    }
}
