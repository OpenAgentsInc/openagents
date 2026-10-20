//! Public agents (NIP-AGENT, "Public agents"; #12255): an agent key of its
//! own, owned by a person through a NIP-OA `auth` tag, with a kind 0 and a
//! public definition (kind 30187).
//!
//! Builders take the agent's signer and the owner's tag; nothing here holds
//! or derives an owner secret. Readers verify the event, the tag, and that
//! the `p` tag names the tag's owner before anything is shown.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::domain::{Event, RelaySigner, Tag, verify_owner_attestation};
use crate::kinds::AGENT_DEFINITION;

/// Kind 30187, the public agent definition.
pub const DEFINITION_KIND: u16 = AGENT_DEFINITION;
/// The largest definition content.
pub const CONTENT_MAX: usize = 262_144;
/// The longest an attestation may stand, from when it was signed: a year
/// and a day.
pub const ATTESTATION_MAX_SECS: u64 = 366 * 24 * 60 * 60;
/// The `alt` text (NIP-31).
pub const ALT: &str = "OpenAgents agent definition";

/// The conditions an owner signs: `created_at<until`.
#[must_use]
pub fn conditions_until(until: u64) -> String {
    format!("created_at<{until}")
}

/// Whether `id` is a published agent ID: 64 lowercase hex characters.
#[must_use]
pub fn valid_id(id: &str) -> bool {
    id.len() == 64 && id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// The owner's `auth` tag as it was handed over, checked against the agent
/// key that will carry it, at `now`: a valid NIP-OA signature whose only
/// condition is `created_at<T`, with `now < T <= now + ATTESTATION_MAX_SECS`.
/// Answers the owner's key.
///
/// # Errors
///
/// A sentence saying what's wrong with the tag.
pub fn check_owner_tag(agent: &RelaySigner, tag: &Tag, now: u64) -> Result<String, String> {
    let values = tag.as_slice();
    if values.len() != 4 || values[0] != "auth" {
        return Err("the owner tag must be [\"auth\", owner, conditions, signature]".into());
    }
    let until = values[2]
        .strip_prefix("created_at<")
        .and_then(|t| t.parse::<u64>().ok())
        .filter(|t| t.to_string() == values[2]["created_at<".len()..])
        .ok_or("the owner tag's only condition must be created_at<T")?;
    if until <= now || until > now.saturating_add(ATTESTATION_MAX_SECS) {
        return Err("the owner tag must stand for at most a year from now".into());
    }
    // A probe the agent signs now proves the tag verifies for this key.
    let probe = agent.sign(now, 0, vec![tag.clone()], "{}".into());
    match verify_owner_attestation(&probe)? {
        Some(attestation) => Ok(attestation.owner_pubkey),
        None => Err("the owner tag is missing".into()),
    }
}

/// The owner's `auth` tag for `agent_pubkey` (hex), checked without the
/// agent's signer: a valid NIP-OA signature over the agent key whose only
/// condition is `created_at<T`, `now < T <= now + ATTESTATION_MAX_SECS`.
/// Answers the owner's key. Used for keys the checking side doesn't hold,
/// such as a browser's Verse key (#12313).
///
/// # Errors
///
/// A sentence saying what's wrong with the tag.
pub fn check_owner_tag_for(agent_pubkey: &str, tag: &Tag, now: u64) -> Result<String, String> {
    use secp256k1::{Secp256k1, XOnlyPublicKey, schnorr::Signature};
    use sha2::{Digest, Sha256};
    let values = tag.as_slice();
    if values.len() != 4 || values[0] != "auth" {
        return Err("the owner tag must be [\"auth\", owner, conditions, signature]".into());
    }
    let until = values[2]
        .strip_prefix("created_at<")
        .and_then(|t| t.parse::<u64>().ok())
        .filter(|t| t.to_string() == values[2]["created_at<".len()..])
        .ok_or("the owner tag's only condition must be created_at<T")?;
    if until <= now || until > now.saturating_add(ATTESTATION_MAX_SECS) {
        return Err("the owner tag must stand for at most a year from now".into());
    }
    let hex32 = |text: &str| -> Option<[u8; 32]> {
        (text.len() == 64 && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))).then(
            || {
                let mut out = [0u8; 32];
                for (i, byte) in out.iter_mut().enumerate() {
                    *byte = u8::from_str_radix(&text[2 * i..2 * i + 2], 16).unwrap_or(0);
                }
                out
            },
        )
    };
    let owner_bytes = hex32(&values[1]).ok_or("the owner key is malformed")?;
    hex32(agent_pubkey).ok_or("the agent key is malformed")?;
    if values[1] == agent_pubkey {
        return Err("an owner tag can't name its own key".into());
    }
    let owner =
        XOnlyPublicKey::from_byte_array(owner_bytes).map_err(|_| "the owner key is malformed")?;
    let sig = &values[3];
    if sig.len() != 128 || !sig.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
        return Err("the owner signature is malformed".into());
    }
    let mut sig_bytes = [0u8; 64];
    for (i, byte) in sig_bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&sig[2 * i..2 * i + 2], 16).unwrap_or(0);
    }
    let digest: [u8; 32] =
        Sha256::digest(format!("nostr:agent-auth:{agent_pubkey}:{}", values[2]).as_bytes()).into();
    Secp256k1::verification_only()
        .verify_schnorr(&Signature::from_byte_array(sig_bytes), &digest, &owner)
        .map_err(|_| "the owner signature doesn't check out".to_string())?;
    Ok(values[1].clone())
}

/// What an agent's profile says.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Profile {
    pub name: String,
    pub about: String,
    /// The definition's HTTPS URL.
    pub website: String,
    pub picture: Option<String>,
}

/// The agent's kind 0: `bot: true`, its name, about, website, and the
/// owner's tag.
#[must_use]
pub fn profile_event(agent: &RelaySigner, owner_tag: &Tag, profile: &Profile, at: u64) -> Event {
    let mut content = json!({
        "name": profile.name,
        "display_name": profile.name,
        "about": profile.about,
        "website": profile.website,
        "bot": true,
    });
    if let Some(picture) = &profile.picture {
        content["picture"] = json!(picture);
    }
    agent.sign(at, 0, vec![owner_tag.clone()], content.to_string())
}

fn tags(owner_tag: &Tag, owner: &str, id: &str, url: &str) -> Vec<Tag> {
    vec![
        Tag::new(vec!["d".into(), id.into()]),
        owner_tag.clone(),
        Tag::new(vec!["p".into(), owner.into()]),
        Tag::new(vec!["r".into(), url.into()]),
        Tag::new(vec!["alt".into(), ALT.into()]),
    ]
}

/// Exact NIP-ATT events the owner says this agent runs on. These are
/// references, not proof that the release or endpoint verifies.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Runtime {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
}

impl Runtime {
    pub fn is_empty(&self) -> bool {
        self.release.is_none() && self.endpoint.is_none()
    }

    pub fn validate(&self) -> Result<(), String> {
        if [&self.release, &self.endpoint]
            .into_iter()
            .flatten()
            .any(|id| !valid_id(id))
        {
            return Err("runtime references must be exact event IDs".into());
        }
        Ok(())
    }

    fn from_event(event: &Event) -> Result<Self, String> {
        let mut runtime = Self::default();
        for tag in &event.tags {
            let values = tag.as_slice();
            let field = match values.get(3).map(String::as_str) {
                Some("att-release") if values.first().is_some_and(|v| v == "e") => {
                    &mut runtime.release
                }
                Some("att-endpoint") if values.first().is_some_and(|v| v == "e") => {
                    &mut runtime.endpoint
                }
                _ => continue,
            };
            if values.len() != 4 || !values[2].is_empty() || field.is_some() {
                return Err("invalid or repeated runtime reference".into());
            }
            *field = Some(values[1].clone());
        }
        runtime.validate()?;
        Ok(runtime)
    }
}

/// Sign a definition with explicit NIP-ATT release or endpoint references.
#[allow(clippy::too_many_arguments)]
pub fn definition_with_runtime(
    agent: &RelaySigner,
    owner_tag: &Tag,
    owner: &str,
    id: &str,
    url: &str,
    content: &str,
    runtime: &Runtime,
    at: u64,
) -> Result<Event, String> {
    runtime.validate()?;
    let mut event = definition_event(agent, owner_tag, owner, id, url, content, at)?;
    for (marker, reference) in [
        ("att-release", &runtime.release),
        ("att-endpoint", &runtime.endpoint),
    ] {
        if let Some(reference) = reference {
            event.tags.push(Tag::new(vec![
                "e".into(),
                reference.clone(),
                String::new(),
                marker.into(),
            ]));
        }
    }
    Ok(agent.sign(at, DEFINITION_KIND, event.tags, event.content))
}

/// The public definition (kind 30187) for published agent `id`.
///
/// # Errors
///
/// When `content` is over [`CONTENT_MAX`] or `id` isn't a published ID.
pub fn definition_event(
    agent: &RelaySigner,
    owner_tag: &Tag,
    owner: &str,
    id: &str,
    url: &str,
    content: &str,
    at: u64,
) -> Result<Event, String> {
    if !valid_id(id) {
        return Err("a published agent ID is 64 lowercase hex characters".into());
    }
    if content.len() > CONTENT_MAX {
        return Err("the definition is too large to publish".into());
    }
    Ok(agent.sign(
        at,
        DEFINITION_KIND,
        tags(owner_tag, owner, id, url),
        content.into(),
    ))
}

/// The replacement that retires published agent `id`.
///
/// # Errors
///
/// When `id` isn't a published ID.
pub fn revoked_event(
    agent: &RelaySigner,
    owner_tag: &Tag,
    owner: &str,
    id: &str,
    url: &str,
    at: u64,
) -> Result<Event, String> {
    if !valid_id(id) {
        return Err("a published agent ID is 64 lowercase hex characters".into());
    }
    let mut tags = tags(owner_tag, owner, id, url);
    tags.push(Tag::new(vec!["status".into(), "revoked".into()]));
    Ok(agent.sign(at, DEFINITION_KIND, tags, String::new()))
}

/// A public definition a reader accepted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Definition {
    /// The agent's key.
    pub agent: String,
    /// The owner's key, from a verified `auth` tag.
    pub owner: String,
    /// The published agent ID.
    pub id: String,
    pub url: Option<String>,
    pub revoked: bool,
    /// The export document, when not revoked and it parses.
    pub document: Option<Value>,
    pub created_at: u64,
    pub runtime: Runtime,
}

fn single<'a>(event: &'a Event, name: &'a str) -> Option<&'a str> {
    let mut values = event.tag_values(name);
    let first = values.next()?;
    values.next().is_none().then_some(first)
}

/// Read a kind 30187 event under the rules above.
///
/// # Errors
///
/// A sentence saying why it doesn't count.
pub fn read_definition(event: &Event) -> Result<Definition, String> {
    if event.kind != DEFINITION_KIND {
        return Err("not a public agent definition".into());
    }
    let attestation =
        verify_owner_attestation(event)?.ok_or("a public agent definition needs an owner tag")?;
    let id = single(event, "d")
        .filter(|d| valid_id(d))
        .ok_or("bad d tag")?;
    let owner = single(event, "p").ok_or("a public agent definition names one owner")?;
    if owner != attestation.owner_pubkey {
        return Err("the p tag must name the owner who signed the auth tag".into());
    }
    if event.content.len() > CONTENT_MAX {
        return Err("the definition is too large".into());
    }
    let revoked = event
        .tags
        .iter()
        .any(|t| t.as_slice() == ["status", "revoked"]);
    let document = (!revoked)
        .then(|| serde_json::from_str::<Value>(&event.content).ok())
        .flatten()
        .filter(Value::is_object);
    Ok(Definition {
        agent: event.pubkey.clone(),
        owner: owner.to_string(),
        id: id.to_string(),
        url: single(event, "r")
            .filter(|u| u.starts_with("https://") || u.starts_with("http://"))
            .map(str::to_string),
        revoked,
        document,
        created_at: event.created_at,
        runtime: Runtime::from_event(event)?,
    })
}

/// The newest accepted definition per `(agent, id)` among `events`.
#[must_use]
pub fn newest_definitions(events: &[Event]) -> Vec<Definition> {
    let mut best: std::collections::BTreeMap<(String, String), Definition> =
        std::collections::BTreeMap::new();
    for definition in events.iter().filter_map(|e| read_definition(e).ok()) {
        let key = (definition.agent.clone(), definition.id.clone());
        // Newest wins; at the same second a retirement wins, so a tie
        // never brings a retired agent back.
        if best.get(&key).is_none_or(|held| {
            (definition.created_at, definition.revoked) > (held.created_at, held.revoked)
        }) {
            best.insert(key, definition);
        }
    }
    best.into_values().collect()
}

/// The owner a kind 0 names through a valid `auth` tag, if any.
#[must_use]
pub fn profile_owner(event: &Event) -> Option<String> {
    (event.kind == 0)
        .then(|| verify_owner_attestation(event).ok().flatten())
        .flatten()
        .map(|a| a.owner_pubkey)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::mint_owner_attestation;

    const NOW: u64 = 1_800_000_000;

    fn signer(byte: u8) -> RelaySigner {
        RelaySigner::from_secret_hex(&format!("{byte:02x}").repeat(32)).unwrap()
    }

    fn owner_tag(owner: u8, agent: &RelaySigner, until: u64) -> Tag {
        let secret = secp256k1::SecretKey::from_byte_array([owner; 32]).unwrap();
        mint_owner_attestation(&secret, agent.pubkey(), &conditions_until(until))
            .unwrap()
            .tag()
    }

    #[test]
    fn an_owner_tag_is_checked_for_its_agent_and_its_lifetime() {
        let (owner, agent, other) = (signer(1), signer(2), signer(3));
        let tag = owner_tag(1, &agent, NOW + 3600);
        assert_eq!(check_owner_tag(&agent, &tag, NOW).unwrap(), owner.pubkey());
        assert!(check_owner_tag(&other, &tag, NOW).is_err(), "another agent");
        assert!(
            check_owner_tag(&agent, &tag, NOW + 7200).is_err(),
            "expired"
        );
        let long = owner_tag(1, &agent, NOW + ATTESTATION_MAX_SECS + 10);
        assert!(check_owner_tag(&agent, &long, NOW).is_err(), "too long");
        let bad = Tag::new(vec!["auth".into(), owner.pubkey().into()]);
        assert!(check_owner_tag(&agent, &bad, NOW).is_err());
    }

    #[test]
    fn runtime_pins_are_signed_exact_and_unambiguous() {
        let (owner, agent) = (signer(1), signer(2));
        let tag = owner_tag(1, &agent, NOW + 3600);
        let runtime = Runtime {
            release: Some("ab".repeat(32)),
            endpoint: Some("cd".repeat(32)),
        };
        let event = definition_with_runtime(
            &agent,
            &tag,
            owner.pubkey(),
            &"ef".repeat(32),
            "https://example.org",
            "{}",
            &runtime,
            NOW,
        )
        .unwrap();
        assert_eq!(read_definition(&event).unwrap().runtime, runtime);
        let mut unsigned = event.clone();
        unsigned.tags.push(Tag::new(vec![
            "e".into(),
            "aa".repeat(32),
            String::new(),
            "att-release".into(),
        ]));
        assert!(read_definition(&unsigned).is_err());
        let repeated = agent.sign(NOW, DEFINITION_KIND, unsigned.tags, unsigned.content);
        assert!(read_definition(&repeated).is_err());
        let mut malformed = runtime.clone();
        malformed.endpoint = Some("not-an-event".into());
        assert!(
            definition_with_runtime(
                &agent,
                &tag,
                owner.pubkey(),
                &"ef".repeat(32),
                "https://example.org",
                "{}",
                &malformed,
                NOW
            )
            .is_err()
        );
        let legacy = definition_event(
            &agent,
            &tag,
            owner.pubkey(),
            &"ef".repeat(32),
            "https://example.org",
            "{}",
            NOW,
        )
        .unwrap();
        assert!(read_definition(&legacy).unwrap().runtime.is_empty());
    }

    #[test]
    fn a_tag_checks_against_a_key_the_checker_does_not_hold() {
        let (owner, agent, other) = (signer(1), signer(2), signer(3));
        let tag = owner_tag(1, &agent, NOW + 3600);
        assert_eq!(
            check_owner_tag_for(agent.pubkey(), &tag, NOW).unwrap(),
            owner.pubkey()
        );
        assert!(check_owner_tag_for(other.pubkey(), &tag, NOW).is_err());
        assert!(check_owner_tag_for(agent.pubkey(), &tag, NOW + 7200).is_err());
    }

    #[test]
    fn events_round_trip_and_revocation_reads() {
        let (owner, agent) = (signer(1), signer(2));
        let tag = owner_tag(1, &agent, NOW + 3600);
        let id = "ab".repeat(32);
        let profile = profile_event(
            &agent,
            &tag,
            &Profile {
                name: "Helper".into(),
                about: "Helps.".into(),
                website: "https://openagents.com/agent/x/definition.json".into(),
                picture: None,
            },
            NOW,
        );
        assert_eq!(profile_owner(&profile).as_deref(), Some(owner.pubkey()));
        let content: Value = serde_json::from_str(&profile.content).unwrap();
        assert_eq!(content["bot"], true);

        let live = definition_event(
            &agent,
            &tag,
            owner.pubkey(),
            &id,
            "https://x.test/d.json",
            r#"{"v":1}"#,
            NOW,
        )
        .unwrap();
        let read = read_definition(&live).unwrap();
        assert_eq!(read.owner, owner.pubkey());
        assert_eq!(read.id, id);
        assert!(!read.revoked);
        assert_eq!(read.document.unwrap()["v"], 1);

        let gone = revoked_event(
            &agent,
            &tag,
            owner.pubkey(),
            &id,
            "https://x.test/d.json",
            NOW,
        )
        .unwrap();
        let newest = newest_definitions(&[live.clone(), gone]);
        assert_eq!(newest.len(), 1);
        assert!(newest[0].revoked);
        assert!(newest[0].document.is_none());

        // A p tag naming someone else, or no auth tag, doesn't count.
        let stranger = signer(4);
        let lying = definition_event(
            &agent,
            &tag,
            stranger.pubkey(),
            &id,
            "https://x.test",
            "{}",
            NOW,
        )
        .unwrap();
        assert!(read_definition(&lying).is_err());
        let bare = agent.sign(
            NOW,
            DEFINITION_KIND,
            vec![Tag::new(vec!["d".into(), id.clone()])],
            "{}".into(),
        );
        assert!(read_definition(&bare).is_err());
        assert!(definition_event(&agent, &tag, owner.pubkey(), "short", "u", "{}", NOW).is_err());
    }
}
