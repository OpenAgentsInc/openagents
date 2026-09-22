//! NIP-F4 podcasts.
//!
//! Kind `10154` is one replaceable show record per podcast key. Kind `54`
//! is a regular episode. Kind `10064` is the author's list of podcast
//! keys. A `p` tag on the show is a claim. `confirmed_hosts` keeps a
//! person only when that person's kind `10064` lists the show.
//!
//! The pinned prose says kind `10164` once. The example and NIP-51 both
//! say kind `10064`, so admission follows `10064`. Kind `10054` stays
//! with NIP-51. The relay does not download audio or images, and it does
//! not read a kind `0` profile. NIP-F4 is a draft, so these kinds stay
//! off the NIP-11 list.

use super::hex::decode_lower_hex;
use super::{DomainError, Event};

const SHOW_KIND: u16 = 10_154;
const EPISODE_KIND: u16 = 54;
const AUTHORED_KIND: u16 = 10_064;

/// A role on a podcast show.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PodcastRole {
    Host,
    Cohost,
    Editor,
}

/// A person named on a kind `10154` show.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PodcastCredit {
    pub pubkey: String,
    pub role: Option<PodcastRole>,
}

/// A kind `10154` podcast show.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PodcastShow {
    pub pubkey: String,
    pub title: String,
    pub image: Option<String>,
    pub description: Option<String>,
    pub websites: Vec<String>,
    pub people: Vec<PodcastCredit>,
}

/// One audio file on an episode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PodcastAudio {
    pub url: String,
    pub media_type: Option<String>,
}

/// A kind `54` episode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PodcastEpisode {
    pub title: String,
    pub image: Option<String>,
    pub description: Option<String>,
    pub audio: Vec<PodcastAudio>,
    pub content: String,
}

/// A kind `10064` list of podcasts one person authors.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthoredPodcasts {
    pub author: String,
    pub podcasts: Vec<String>,
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

fn title_ok(value: &str) -> bool {
    !value.is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
}

fn prose_ok(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && !value
            .chars()
            .any(|char| char.is_control() && char != '\n' && char != '\t')
}

fn media_type(value: &str) -> bool {
    let Some((kind, subtype)) = value.split_once('/') else {
        return false;
    };
    !kind.is_empty()
        && !subtype.is_empty()
        && value.len() <= 64
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'/' | b'+' | b'-' | b'.')
        })
}

fn pubkey(value: &str) -> Result<String, DomainError> {
    decode_lower_hex::<32>(value, "podcast pubkey")
        .map_err(|_| invalid("a podcast pubkey is 32 lowercase hex bytes"))?;
    Ok(value.to_owned())
}

fn role(value: &str) -> Result<PodcastRole, DomainError> {
    match value {
        "host" => Ok(PodcastRole::Host),
        "cohost" => Ok(PodcastRole::Cohost),
        "editor" => Ok(PodcastRole::Editor),
        _ => Err(invalid("a podcast role is host, cohost, or editor")),
    }
}

fn one<'a>(tag: &'a super::Tag, reason: &'static str) -> Result<&'a str, DomainError> {
    if tag.as_slice().len() != 2 {
        return Err(invalid(reason));
    }
    tag.value().ok_or_else(|| invalid(reason))
}

/// Read a kind `10154` show.
pub fn open_podcast_show(event: &Event) -> Result<PodcastShow, DomainError> {
    if event.kind != SHOW_KIND {
        return Err(invalid("a podcast show has kind 10154"));
    }
    if event.content.len() > 2_048 || event.content.chars().any(char::is_control) {
        return Err(invalid("a podcast show comment is plain text"));
    }
    let mut title = None;
    let mut image = None;
    let mut description = None;
    let mut websites = Vec::new();
    let mut people = Vec::new();
    for tag in &event.tags {
        match tag.name() {
            Some("title") => {
                if title.is_some() {
                    return Err(invalid("a podcast show has one title"));
                }
                let value = one(tag, "a podcast show has one title")?;
                if !title_ok(value) {
                    return Err(invalid("a podcast title is 1 to 256 characters"));
                }
                title = Some(value.to_owned());
            }
            Some("image") => {
                if image.is_some() {
                    return Err(invalid("a podcast image is an http URL"));
                }
                let value = one(tag, "a podcast image is an http URL")?;
                if !http_url(value) {
                    return Err(invalid("a podcast image is an http URL"));
                }
                image = Some(value.to_owned());
            }
            Some("description") => {
                if description.is_some() {
                    return Err(invalid("a podcast show has one description"));
                }
                let value = one(tag, "a podcast show has one description")?;
                if !prose_ok(value, 4_096) {
                    return Err(invalid("a podcast description is 1 to 4096 characters"));
                }
                description = Some(value.to_owned());
            }
            Some("website") => {
                let value = one(tag, "a podcast website is an http URL")?;
                if !http_url(value) || websites.iter().any(|seen| seen == value) {
                    return Err(invalid("a podcast website is an http URL"));
                }
                websites.push(value.to_owned());
            }
            Some("p") => {
                let parts = tag.as_slice();
                if !(2..=3).contains(&parts.len()) {
                    return Err(invalid("a podcast credit names a pubkey"));
                }
                let person = pubkey(&parts[1])?;
                if people
                    .iter()
                    .any(|credit: &PodcastCredit| credit.pubkey == person)
                {
                    return Err(invalid("a podcast credit is listed once"));
                }
                let role = match parts.get(2) {
                    None => None,
                    Some(value) => Some(role(value)?),
                };
                people.push(PodcastCredit {
                    pubkey: person,
                    role,
                });
            }
            _ => {}
        }
    }
    let Some(title) = title else {
        return Err(invalid("a podcast show has one title"));
    };
    Ok(PodcastShow {
        pubkey: event.pubkey.clone(),
        title,
        image,
        description,
        websites,
        people,
    })
}

/// Read a kind `54` episode.
pub fn open_podcast_episode(event: &Event) -> Result<PodcastEpisode, DomainError> {
    if event.kind != EPISODE_KIND {
        return Err(invalid("a podcast episode has kind 54"));
    }
    if event.content.len() > 65_536
        || event
            .content
            .chars()
            .any(|char| char.is_control() && char != '\n' && char != '\t')
    {
        return Err(invalid("episode content is markdown"));
    }
    let mut title = None;
    let mut image = None;
    let mut description = None;
    let mut audio = Vec::new();
    for tag in &event.tags {
        match tag.name() {
            Some("title") => {
                if title.is_some() {
                    return Err(invalid("a podcast episode has one title"));
                }
                let value = one(tag, "a podcast episode has one title")?;
                if !title_ok(value) {
                    return Err(invalid("a podcast title is 1 to 256 characters"));
                }
                title = Some(value.to_owned());
            }
            Some("image") => {
                if image.is_some() {
                    return Err(invalid("a podcast image is an http URL"));
                }
                let value = one(tag, "a podcast image is an http URL")?;
                if !http_url(value) {
                    return Err(invalid("a podcast image is an http URL"));
                }
                image = Some(value.to_owned());
            }
            Some("description") => {
                if description.is_some() {
                    return Err(invalid("a podcast episode has one description"));
                }
                let value = one(tag, "a podcast episode has one description")?;
                if !prose_ok(value, 4_096) {
                    return Err(invalid("a podcast description is 1 to 4096 characters"));
                }
                description = Some(value.to_owned());
            }
            Some("audio") => {
                let parts = tag.as_slice();
                if !(2..=3).contains(&parts.len()) || !http_url(&parts[1]) {
                    return Err(invalid("episode audio is an http URL"));
                }
                let media_type = match parts.get(2) {
                    None => None,
                    Some(value) if media_type(value) => Some(value.clone()),
                    Some(_) => return Err(invalid("episode audio names a media type")),
                };
                if audio.iter().any(|item: &PodcastAudio| item.url == parts[1]) {
                    return Err(invalid("episode audio is listed once"));
                }
                audio.push(PodcastAudio {
                    url: parts[1].clone(),
                    media_type,
                });
            }
            _ => {}
        }
    }
    let Some(title) = title else {
        return Err(invalid("a podcast episode has one title"));
    };
    if audio.is_empty() {
        return Err(invalid("a podcast episode has one audio URL"));
    }
    Ok(PodcastEpisode {
        title,
        image,
        description,
        audio,
        content: event.content.clone(),
    })
}

/// Read a kind `10064` authored-podcast list.
pub fn open_authored_podcasts(event: &Event) -> Result<AuthoredPodcasts, DomainError> {
    if event.kind != AUTHORED_KIND {
        return Err(invalid("an authored podcast list has kind 10064"));
    }
    if !event.content.is_empty() {
        return Err(invalid("an authored podcast list has empty content"));
    }
    let mut podcasts = Vec::new();
    for tag in &event.tags {
        if tag.name() != Some("p") {
            continue;
        }
        let value = one(tag, "an authored podcast is a pubkey")?;
        let value = pubkey(value)?;
        if podcasts.iter().any(|seen| seen == &value) {
            return Err(invalid("an authored podcast is listed once"));
        }
        podcasts.push(value);
    }
    if podcasts.is_empty() {
        return Err(invalid("an authored podcast list names a show"));
    }
    Ok(AuthoredPodcasts {
        author: event.pubkey.clone(),
        podcasts,
    })
}

/// People on `show` whose kind `10064` lists that show.
#[must_use]
pub fn confirmed_hosts<'a>(
    show: &'a PodcastShow,
    claims: &[AuthoredPodcasts],
) -> Vec<&'a PodcastCredit> {
    show.people
        .iter()
        .filter(|person| {
            claims.iter().any(|claim| {
                claim.author == person.pubkey
                    && claim.podcasts.iter().any(|podcast| podcast == &show.pubkey)
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{EventClass, RelaySigner, ReplacementDecision, Tag, compare_replacement};

    fn signer(byte: &str) -> RelaySigner {
        RelaySigner::from_secret_hex(&byte.repeat(32)).unwrap()
    }

    #[test]
    fn a_show_replaces_and_a_host_is_confirmed_by_their_list() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../nips/official/F4.md"
        ))
        .unwrap();
        assert!(text.contains("10154"));
        assert!(text.contains("title"));
        assert!(text.contains("10064"));
        assert!(text.contains("10164"));
        let row = crate::lane::PROVEN
            .iter()
            .find(|row| row.file == "F4.md")
            .unwrap();
        assert_eq!(row.status, "configured-and-proven");
        assert!(
            crate::lane::SHAPES
                .iter()
                .all(|shape| shape.file != "F4.md")
        );

        let show_key = signer("f4");
        let host = signer("f5");
        let show = show_key.sign(
            1_700_000_000,
            SHOW_KIND,
            vec![
                Tag::new(vec!["title".into(), "Open Agents".into()]),
                Tag::new(vec!["image".into(), "https://cdn.example/cover.png".into()]),
                Tag::new(vec!["description".into(), "A show about relays.".into()]),
                Tag::new(vec!["website".into(), "https://show.example".into()]),
                Tag::new(vec!["p".into(), host.pubkey().to_owned(), "host".into()]),
            ],
            String::new(),
        );
        show.validate_structure().unwrap();
        assert_eq!(show.class(), EventClass::Replaceable);
        let opened = open_podcast_show(&show).unwrap();
        assert_eq!(opened.title, "Open Agents");
        assert_eq!(opened.people[0].role, Some(PodcastRole::Host));
        assert!(confirmed_hosts(&opened, &[]).is_empty());

        let revised = show_key.sign(
            1_700_000_100,
            SHOW_KIND,
            vec![Tag::new(vec!["title".into(), "Open Agents Daily".into()])],
            String::new(),
        );
        revised.validate_structure().unwrap();
        assert_eq!(
            compare_replacement(&show, &revised),
            Ok(ReplacementDecision::ReplaceCurrent)
        );

        let episode = show_key.sign(
            1_700_000_200,
            EPISODE_KIND,
            vec![
                Tag::new(vec!["title".into(), "Pilot".into()]),
                Tag::new(vec![
                    "audio".into(),
                    "https://cdn.example/pilot.mp3".into(),
                    "audio/mpeg".into(),
                ]),
            ],
            "Hello.\n".into(),
        );
        episode.validate_structure().unwrap();
        assert_eq!(episode.class(), EventClass::Regular);
        let played = open_podcast_episode(&episode).unwrap();
        assert_eq!(played.audio[0].media_type.as_deref(), Some("audio/mpeg"));
        assert!(matches!(
            compare_replacement(&episode, &episode),
            Err(DomainError::NotReplaceable)
        ));

        let claim = host.sign(
            1_700_000_300,
            AUTHORED_KIND,
            vec![Tag::new(vec!["p".into(), show_key.pubkey().to_owned()])],
            String::new(),
        );
        claim.validate_structure().unwrap();
        assert_eq!(claim.class(), EventClass::Replaceable);
        let authored = open_authored_podcasts(&claim).unwrap();
        let confirmed = confirmed_hosts(&opened, &[authored]);
        assert_eq!(confirmed.len(), 1);
        assert_eq!(confirmed[0].pubkey, host.pubkey());

        let untitled = show_key.sign(1_700_000_400, SHOW_KIND, Vec::new(), String::new());
        assert!(untitled.validate_structure().is_err());
    }
}
