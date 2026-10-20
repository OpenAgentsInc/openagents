//! Published skills (NIP-SKILL; #12303): one addressable listing (kind
//! 30188) per skill a directory reviewed and admitted, signed by that
//! directory's key.

use serde::{Deserialize, Serialize};

use crate::domain::{Event, RelaySigner, Tag};

/// Kind 30188, the skill listing.
pub const KIND: u16 = crate::kinds::SKILL_LISTING;
/// The `alt` text (NIP-31).
pub const ALT: &str = "OpenAgents skill";
/// The largest listing content.
pub const CONTENT_MAX: usize = 16_384;

/// What a listing says.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Skill {
    pub name: String,
    pub version: String,
    /// `sha256:` and 64 lowercase hex characters.
    pub digest: String,
    /// The HTTPS URL serving that exact document.
    pub url: String,
    pub license: String,
    pub category: String,
    pub tags: Vec<String>,
}

/// A listing a reader accepted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Listing {
    /// The directory that signed it.
    pub directory: String,
    pub skill: Skill,
    /// The author's key, when the directory named one.
    pub author: Option<String>,
    pub created_at: u64,
    pub withdrawn: bool,
}

/// Whether `name` is a skill name: 1 to 64 lowercase letters, digits, `-`.
#[must_use]
pub fn valid_name(name: &str) -> bool {
    (1..=64).contains(&name.len())
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn valid_key(key: &str) -> bool {
    key.len() == 64 && key.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn valid_digest(digest: &str) -> bool {
    digest.strip_prefix("sha256:").is_some_and(valid_key)
}

/// The listing's address, which reviews name in an `a` tag.
#[must_use]
pub fn address(directory: &str, name: &str) -> String {
    format!("{KIND}:{directory}:{name}")
}

/// The signed listing for `skill`, by `author` when known.
///
/// # Errors
///
/// When the skill's name, digest, or URL is malformed, or it's too large.
pub fn listing_event(
    directory: &RelaySigner,
    skill: &Skill,
    author: Option<&str>,
    at: u64,
) -> Result<Event, String> {
    if !valid_name(&skill.name)
        || !valid_digest(&skill.digest)
        || !skill.url.starts_with("https://")
    {
        return Err("a skill listing needs a name, a sha256 digest, and an https URL".into());
    }
    let content = serde_json::to_string(skill).map_err(|e| e.to_string())?;
    if content.len() > CONTENT_MAX {
        return Err("the listing is too large".into());
    }
    let mut tags = vec![
        Tag::new(vec!["d".into(), skill.name.clone()]),
        Tag::new(vec!["version".into(), skill.version.clone()]),
        Tag::new(vec!["x".into(), skill.digest.clone()]),
        Tag::new(vec!["r".into(), skill.url.clone()]),
    ];
    if let Some(author) = author.filter(|a| valid_key(a)) {
        tags.push(Tag::new(vec!["p".into(), author.into()]));
    }
    for t in std::iter::once(&skill.category).chain(skill.tags.iter()) {
        let t = t.to_ascii_lowercase();
        if !t.is_empty() && t.len() <= 64 {
            tags.push(Tag::new(vec!["t".into(), t]));
        }
    }
    tags.push(Tag::new(vec!["alt".into(), ALT.into()]));
    Ok(directory.sign(at, KIND, tags, content))
}

/// The replacement that withdraws skill `name`.
#[must_use]
pub fn withdrawn_event(directory: &RelaySigner, name: &str, at: u64) -> Event {
    directory.sign(
        at,
        KIND,
        vec![
            Tag::new(vec!["d".into(), name.into()]),
            Tag::new(vec!["status".into(), "withdrawn".into()]),
            Tag::new(vec!["alt".into(), ALT.into()]),
        ],
        String::new(),
    )
}

/// Read a listing.
///
/// # Errors
///
/// A sentence saying why it doesn't count.
pub fn read(event: &Event) -> Result<Listing, String> {
    if event.kind != KIND {
        return Err("not a skill listing".into());
    }
    event.validate_crypto().map_err(|e| e.to_string())?;
    let d = event
        .tag_values("d")
        .next()
        .filter(|d| valid_name(d))
        .ok_or("bad d tag")?;
    let withdrawn = event
        .tags
        .iter()
        .any(|t| t.as_slice() == ["status", "withdrawn"]);
    let skill = if withdrawn {
        Skill {
            name: d.into(),
            version: String::new(),
            digest: String::new(),
            url: String::new(),
            license: String::new(),
            category: String::new(),
            tags: Vec::new(),
        }
    } else {
        if event.content.len() > CONTENT_MAX {
            return Err("the listing is too large".into());
        }
        let skill: Skill = serde_json::from_str(&event.content).map_err(|e| e.to_string())?;
        if skill.name != d || !valid_digest(&skill.digest) || !skill.url.starts_with("https://") {
            return Err("the listing's content disagrees with its tags".into());
        }
        skill
    };
    Ok(Listing {
        directory: event.pubkey.clone(),
        author: event
            .tag_values("p")
            .next()
            .filter(|p| valid_key(p))
            .map(str::to_string),
        skill,
        created_at: event.created_at,
        withdrawn,
    })
}

/// The newest listing per `(directory, name)` among `events` signed by one
/// of `directories`, live ones only; a withdrawal wins a tie.
#[must_use]
pub fn current(events: &[Event], directories: &[String]) -> Vec<Listing> {
    let mut best: std::collections::BTreeMap<(String, String), Listing> = Default::default();
    for listing in events
        .iter()
        .filter(|e| directories.contains(&e.pubkey))
        .filter_map(|e| read(e).ok())
    {
        let key = (listing.directory.clone(), listing.skill.name.clone());
        if best.get(&key).is_none_or(|held| {
            (listing.created_at, listing.withdrawn) > (held.created_at, held.withdrawn)
        }) {
            best.insert(key, listing);
        }
    }
    best.into_values().filter(|l| !l.withdrawn).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signer(b: u8) -> RelaySigner {
        RelaySigner::from_secret_hex(&format!("{b:02x}").repeat(32)).unwrap()
    }

    fn skill() -> Skill {
        Skill {
            name: "repo-map".into(),
            version: "1.2.0".into(),
            digest: format!("sha256:{}", "a".repeat(64)),
            url: "https://openagents.com/v1/skills/repo-map/versions/1.2.0/SKILL.md".into(),
            license: "MIT".into(),
            category: "Coding".into(),
            tags: vec!["git".into()],
        }
    }

    #[test]
    fn listings_round_trip_and_only_trusted_directories_count() {
        let (dir, other, author) = (signer(1), signer(2), signer(3));
        let event = listing_event(&dir, &skill(), Some(author.pubkey()), 10).unwrap();
        let read = read(&event).unwrap();
        assert_eq!(read.skill, skill());
        assert_eq!(read.author.as_deref(), Some(author.pubkey()));
        assert!(event.tag_values("t").any(|t| t == "coding"));
        let forged = listing_event(&other, &skill(), None, 11).unwrap();
        let trusted = vec![dir.pubkey().to_string()];
        let found = current(&[event.clone(), forged], &trusted);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].directory, dir.pubkey());
        let gone = withdrawn_event(&dir, "repo-map", 10);
        assert!(current(&[event, gone], &trusted).is_empty());
        let mut bad = skill();
        bad.url = "http://x".into();
        assert!(listing_event(&dir, &bad, None, 1).is_err());
        assert_eq!(
            address(dir.pubkey(), "repo-map"),
            format!("30188:{}:repo-map", dir.pubkey())
        );
    }
}
