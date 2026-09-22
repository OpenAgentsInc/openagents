//! NIP-39 links from a Nostr profile to another platform.
//!
//! Kind `10011` is a replaceable list of `i` tags. Each tag carries
//! `platform:identity` and a proof. The identity is stored in lowercase.
//! Known platforms check the identity and proof shapes and name the public
//! URL and the statement the proof should contain.
//!
//! The relay does not fetch the proof. A newer list from the same author
//! replaces the older one.

use crate::nip19;

use super::{DomainError, Event, Tag};

const LINK_KIND: u16 = 10_011;

/// One claimed identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdentityClaim {
    pub platform: String,
    pub identity: String,
    pub proof: String,
    /// Values after the required platform and proof pair.
    pub extra: Vec<String>,
}

/// A kind `10011` profile link list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileLinks {
    pub claims: Vec<IdentityClaim>,
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.to_owned())
}

fn platform_name(value: &str) -> bool {
    !value.is_empty()
        && value.chars().all(|char| {
            char.is_ascii_lowercase()
                || char.is_ascii_digit()
                || matches!(char, '.' | '_' | '-' | '/')
        })
}

fn github_name(value: &str) -> bool {
    let chars: Vec<char> = value.chars().collect();
    !chars.is_empty()
        && chars.len() <= 39
        && chars
            .iter()
            .all(|char| char.is_ascii_lowercase() || char.is_ascii_digit() || *char == '-')
        && chars.first() != Some(&'-')
        && chars.last() != Some(&'-')
}

fn twitter_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 15
        && value
            .chars()
            .all(|char| char.is_ascii_lowercase() || char.is_ascii_digit() || char == '_')
}

fn hostname(value: &str) -> bool {
    !value.is_empty()
        && value.contains('.')
        && value.split('.').all(|label| {
            !label.is_empty()
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .chars()
                    .all(|char| char.is_ascii_lowercase() || char.is_ascii_digit() || char == '-')
        })
}

fn mastodon(identity: &str) -> bool {
    let Some((instance, username)) = identity.split_once("/@") else {
        return false;
    };
    hostname(instance)
        && !username.is_empty()
        && !username.contains('/')
        && username
            .chars()
            .all(|char| char.is_ascii_lowercase() || char.is_ascii_digit() || char == '_')
}

fn digits(value: &str) -> bool {
    !value.is_empty() && value.chars().all(|char| char.is_ascii_digit())
}

fn hex_proof(value: &str) -> bool {
    !value.is_empty() && value.len() <= 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn known(platform: &str, identity: &str, proof: &str) -> Result<(), DomainError> {
    let ok = match platform {
        "github" => github_name(identity) && hex_proof(proof),
        "twitter" => twitter_name(identity) && digits(proof),
        "mastodon" => mastodon(identity) && digits(proof),
        "telegram" => {
            digits(identity)
                && proof.split_once('/').is_some_and(|(channel, id)| {
                    !channel.is_empty()
                        && !channel.contains('/')
                        && channel
                            .chars()
                            .all(|char| char.is_ascii_alphanumeric() || char == '_')
                        && digits(id)
                })
        }
        _ => true,
    };
    if ok {
        Ok(())
    } else {
        Err(invalid("an identity claim does not match its platform"))
    }
}

fn claim(tag: &Tag) -> Result<IdentityClaim, DomainError> {
    let values = tag.as_slice();
    if values.len() < 3 {
        return Err(invalid(
            "an identity tag needs a platform:identity and a proof",
        ));
    }
    let Some((platform, identity)) = values[1].split_once(':') else {
        return Err(invalid(
            "an identity tag needs a platform:identity and a proof",
        ));
    };
    if !platform_name(platform) || identity.is_empty() || values[2].is_empty() {
        return Err(invalid(
            "an identity tag needs a platform:identity and a proof",
        ));
    }
    let identity = identity.to_ascii_lowercase();
    let proof = if platform == "github" {
        values[2].to_ascii_lowercase()
    } else {
        values[2].clone()
    };
    known(platform, &identity, &proof)?;
    Ok(IdentityClaim {
        platform: platform.to_owned(),
        identity,
        proof,
        extra: values[3..].to_vec(),
    })
}

/// Read a kind `10011` list. It must name at least one identity.
pub fn open_profile_links(event: &Event) -> Result<ProfileLinks, DomainError> {
    if event.kind != LINK_KIND {
        return Err(invalid("a profile link list has kind 10011"));
    }
    let claims = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("i"))
        .map(claim)
        .collect::<Result<Vec<_>, _>>()?;
    if claims.is_empty() {
        return Err(invalid("a profile link list names at least one identity"));
    }
    Ok(ProfileLinks { claims })
}

/// Public page where a client can read the proof. Unknown platforms have none.
pub fn proof_url(claim: &IdentityClaim) -> Option<String> {
    match claim.platform.as_str() {
        "github" => Some(format!(
            "https://gist.github.com/{}/{}",
            claim.identity, claim.proof
        )),
        "twitter" => Some(format!(
            "https://twitter.com/{}/status/{}",
            claim.identity, claim.proof
        )),
        "mastodon" => Some(format!("https://{}/{}", claim.identity, claim.proof)),
        "telegram" => Some(format!("https://t.me/{}", claim.proof)),
        _ => None,
    }
}

/// Sentence the proof post should contain, with this author's `npub`.
pub fn expected_statement(claim: &IdentityClaim, npub: &str) -> Option<String> {
    match claim.platform.as_str() {
        "github" => Some(format!(
            "Verifying that I control the following Nostr public key: {npub}"
        )),
        "twitter" => Some(format!(
            "Verifying my account on nostr My Public Key: \"{npub}\""
        )),
        "mastodon" | "telegram" => Some(format!(
            "Verifying that I control the following Nostr public key: \"{npub}\""
        )),
        _ => None,
    }
}

/// `npub` for the author of `event`.
pub fn author_npub(event: &Event) -> Result<String, DomainError> {
    let bytes = decode_pubkey(&event.pubkey)?;
    Ok(nip19::encode_npub(&bytes))
}

fn decode_pubkey(value: &str) -> Result<[u8; 32], DomainError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(invalid("a profile link author must be a pubkey"));
    }
    let mut bytes = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let high = hex_nibble(pair[0])?;
        let low = hex_nibble(pair[1])?;
        bytes[index] = (high << 4) | low;
    }
    Ok(bytes)
}

fn hex_nibble(byte: u8) -> Result<u8, DomainError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(invalid("a profile link author must be a pubkey")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{EventClass, RelaySigner, ReplacementDecision, compare_replacement};

    fn signer() -> RelaySigner {
        RelaySigner::from_secret_hex(&"39".repeat(32)).unwrap()
    }

    #[test]
    fn a_profile_link_list_names_each_platform_and_replaces() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../nips/official/39.md"
        ))
        .unwrap();
        assert!(text.contains("kind `10011`"));
        assert!(text.contains("gist.github.com"));
        assert!(text.contains("twitter.com"));
        let row = crate::lane::PROVEN
            .iter()
            .find(|row| row.file == "39.md")
            .unwrap();
        assert_eq!(row.status, "configured-and-proven");
        assert!(
            crate::lane::SHAPES
                .iter()
                .all(|shape| shape.file != "39.md")
        );

        let event = signer().sign(
            1_700_000_000,
            LINK_KIND,
            vec![
                Tag::new(vec![
                    "i".into(),
                    "github:SemiSol".into(),
                    "9721CE4EE4FCEB91C9711CA2A6C9A5AB".into(),
                    "future".into(),
                ]),
                Tag::new(vec![
                    "i".into(),
                    "twitter:semisol_public".into(),
                    "1619358434134196225".into(),
                ]),
                Tag::new(vec![
                    "i".into(),
                    "mastodon:bitcoinhackers.org/@semisol".into(),
                    "109775066355589974".into(),
                ]),
                Tag::new(vec![
                    "i".into(),
                    "telegram:1087295469".into(),
                    "nostrdirectory/770".into(),
                ]),
            ],
            String::new(),
        );
        event.validate_structure().unwrap();
        assert_eq!(event.class(), EventClass::Replaceable);
        let links = open_profile_links(&event).unwrap();
        assert_eq!(links.claims[0].identity, "semisol");
        assert_eq!(links.claims[0].proof, "9721ce4ee4fceb91c9711ca2a6c9a5ab");
        assert_eq!(links.claims[0].extra, vec!["future".to_owned()]);
        assert_eq!(
            proof_url(&links.claims[0]).as_deref(),
            Some("https://gist.github.com/semisol/9721ce4ee4fceb91c9711ca2a6c9a5ab")
        );
        assert_eq!(
            proof_url(&links.claims[1]).as_deref(),
            Some("https://twitter.com/semisol_public/status/1619358434134196225")
        );
        assert_eq!(
            proof_url(&links.claims[2]).as_deref(),
            Some("https://bitcoinhackers.org/@semisol/109775066355589974")
        );
        assert_eq!(
            proof_url(&links.claims[3]).as_deref(),
            Some("https://t.me/nostrdirectory/770")
        );
        let npub = author_npub(&event).unwrap();
        let github = expected_statement(&links.claims[0], &npub).unwrap();
        assert_eq!(
            github,
            format!("Verifying that I control the following Nostr public key: {npub}")
        );
        let twitter = expected_statement(&links.claims[1], &npub).unwrap();
        assert!(twitter.contains(&format!("\"{npub}\"")));
        assert!(
            expected_statement(&links.claims[2], &npub)
                .unwrap()
                .contains(&format!("\"{npub}\""))
        );

        let newer = signer().sign(
            1_700_000_100,
            LINK_KIND,
            vec![Tag::new(vec![
                "i".into(),
                "github:semisol".into(),
                "9721ce4ee4fceb91c9711ca2a6c9a5ab".into(),
            ])],
            String::new(),
        );
        assert_eq!(
            compare_replacement(&event, &newer).unwrap(),
            ReplacementDecision::ReplaceCurrent
        );

        let broken = signer().sign(
            1_700_000_000,
            LINK_KIND,
            vec![Tag::new(vec!["i".into(), "github:semisol".into()])],
            String::new(),
        );
        assert!(broken.validate_structure().is_err());
        let uppercase_platform = signer().sign(
            1_700_000_000,
            LINK_KIND,
            vec![Tag::new(vec![
                "i".into(),
                "GitHub:semisol".into(),
                "9721ce4ee4fceb91c9711ca2a6c9a5ab".into(),
            ])],
            String::new(),
        );
        assert!(uppercase_platform.validate_structure().is_err());
        let empty = signer().sign(1_700_000_000, LINK_KIND, Vec::new(), String::new());
        assert!(empty.validate_structure().is_err());
    }
}
