//! NIP-58 badges.
//!
//! Kind `30009` defines a badge. Kind `8` awards it to one or more pubkeys
//! and is not replaceable. Kind `10008` is the profile's ordered list of
//! accepted badges. Kind `30008` is a labeled set of those pairs, except
//! `d = profile_badges`, which is the deprecated profile list.
//!
//! An `a` tag without the following `e` tag, and an `e` tag without a
//! preceding `a` tag, are ignored. The relay does not fetch images and does
//! not check that the award event repeats the definition address.

use std::str::FromStr;

use super::hex::decode_lower_hex;
use super::{DomainError, Event, ReplacementAddress, Tag};

const DEFINITION_KIND: u16 = 30_009;
const AWARD_KIND: u16 = 8;
const PROFILE_KIND: u16 = 10_008;
const SET_KIND: u16 = 30_008;
const DEPRECATED_PROFILE: &str = "profile_badges";

/// A badge image or thumbnail and its optional pixel size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BadgeImage {
    pub url: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

/// A kind `30009` badge definition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BadgeDefinition {
    pub identifier: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub image: Option<BadgeImage>,
    pub thumbs: Vec<BadgeImage>,
}

/// One pubkey named by a kind `8` award.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AwardRecipient {
    pub pubkey: String,
    pub relay: Option<String>,
}

/// A kind `8` badge award.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BadgeAward {
    pub definition: ReplacementAddress,
    pub recipients: Vec<AwardRecipient>,
}

/// One accepted badge: a definition address and the award event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileBadge {
    pub definition: ReplacementAddress,
    pub award_id: String,
    pub relay: Option<String>,
}

/// A kind `10008` list, or the deprecated kind `30008` profile list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileBadges {
    pub badges: Vec<ProfileBadge>,
    pub sets: Vec<ReplacementAddress>,
    pub deprecated_kind: bool,
}

/// A kind `30008` labeled group of accepted badges.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BadgeSet {
    pub identifier: String,
    pub badges: Vec<ProfileBadge>,
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

fn relay_at(tag: &Tag, index: usize) -> Result<Option<String>, DomainError> {
    match tag.as_slice().get(index) {
        None => Ok(None),
        Some(value) if is_relay(value) => Ok(Some(value.to_owned())),
        Some(_) => Err(invalid("a badge relay hint must be ws:// or wss://")),
    }
}

fn dimensions(value: &str) -> Result<(u32, u32), DomainError> {
    let Some((width, height)) = value.split_once('x') else {
        return Err(invalid("badge image dimensions are widthxheight"));
    };
    let parsed_width = width
        .parse::<u32>()
        .map_err(|_| invalid("badge image dimensions are widthxheight"))?;
    let parsed_height = height
        .parse::<u32>()
        .map_err(|_| invalid("badge image dimensions are widthxheight"))?;
    if parsed_width == 0
        || parsed_height == 0
        || width != parsed_width.to_string()
        || height != parsed_height.to_string()
    {
        return Err(invalid("badge image dimensions are widthxheight"));
    }
    Ok((parsed_width, parsed_height))
}

fn image_of(tag: &Tag) -> Result<BadgeImage, DomainError> {
    let Some(url) = tag.value().filter(|url| valid_http_url(url)) else {
        return Err(invalid("a badge image is an http:// or https:// URL"));
    };
    let (width, height) = match tag.as_slice().get(2) {
        None => (None, None),
        Some(value) => {
            let (width, height) = dimensions(value)?;
            (Some(width), Some(height))
        }
    };
    Ok(BadgeImage {
        url: url.to_owned(),
        width,
        height,
    })
}

fn one_text(event: &Event, name: &str, reason: &str) -> Result<Option<String>, DomainError> {
    let values = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some(name))
        .collect::<Vec<_>>();
    if values.len() > 1 {
        return Err(invalid(reason));
    }
    match values.first() {
        None => Ok(None),
        Some(tag) => {
            let Some(value) = tag.value().filter(|value| !value.is_empty()) else {
                return Err(invalid(reason));
            };
            Ok(Some(value.to_owned()))
        }
    }
}

fn identifier(event: &Event) -> Result<String, DomainError> {
    let tags = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("d"))
        .collect::<Vec<_>>();
    if tags.len() != 1 {
        return Err(invalid("a badge address has one d tag"));
    }
    let Some(value) = tags[0].value() else {
        return Err(invalid("a badge address has one d tag"));
    };
    if value.is_empty() || value.len() > 256 || value.chars().any(char::is_whitespace) {
        return Err(invalid(
            "a badge identifier contains 1 to 256 characters and no whitespace",
        ));
    }
    Ok(value.to_owned())
}

fn badge_address(tag: &Tag) -> Result<ReplacementAddress, DomainError> {
    let Some(value) = tag.value() else {
        return Err(invalid(
            "a badge reference is a kind 30009 or 30008 address",
        ));
    };
    let address = ReplacementAddress::from_str(value)
        .map_err(|_| invalid("a badge reference is a kind 30009 or 30008 address"))?;
    if !matches!(address.kind, DEFINITION_KIND | SET_KIND) || address.identifier.is_empty() {
        return Err(invalid(
            "a badge reference is a kind 30009 or 30008 address",
        ));
    }
    Ok(address)
}

fn award_ref(tag: &Tag) -> Result<(String, Option<String>), DomainError> {
    let Some(id) = tag.value() else {
        return Err(invalid("a badge award id must be 32 lowercase hex bytes"));
    };
    decode_lower_hex::<32>(id, "badge award id")
        .map_err(|_| invalid("a badge award id must be 32 lowercase hex bytes"))?;
    Ok((id.to_owned(), relay_at(tag, 2)?))
}

/// Pairs of definition `a` and award `e`, plus profile references to sets.
///
/// `allow_sets` is true for a profile list. A badge set names only pairs.
fn accepted(
    tags: &[Tag],
    allow_sets: bool,
) -> Result<(Vec<ProfileBadge>, Vec<ReplacementAddress>), DomainError> {
    let mut badges = Vec::new();
    let mut sets = Vec::new();
    let mut index = 0;
    while index < tags.len() {
        match tags[index].name() {
            Some("a") => {
                let address = badge_address(&tags[index])?;
                if address.kind == SET_KIND {
                    if !allow_sets {
                        return Err(invalid("a badge set names definition and award pairs"));
                    }
                    sets.push(address);
                    index += 1;
                } else if tags
                    .get(index + 1)
                    .is_some_and(|tag| tag.name() == Some("e"))
                {
                    let (award_id, relay) = award_ref(&tags[index + 1])?;
                    badges.push(ProfileBadge {
                        definition: address,
                        award_id,
                        relay,
                    });
                    index += 2;
                } else {
                    index += 1;
                }
            }
            Some("e") => {
                award_ref(&tags[index])?;
                index += 1;
            }
            _ => index += 1,
        }
    }
    Ok((badges, sets))
}

/// Read a kind `30009` badge definition.
///
/// # Errors
///
/// Returns a sentence when the identifier or an image tag is not the pinned shape.
pub fn open_badge_definition(event: &Event) -> Result<BadgeDefinition, DomainError> {
    if event.kind != DEFINITION_KIND {
        return Err(invalid("a badge definition has kind 30009"));
    }
    let image = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("image"))
        .collect::<Vec<_>>();
    if image.len() > 1 {
        return Err(invalid("a badge definition has one image"));
    }
    let mut thumbs = Vec::new();
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("thumb")) {
        thumbs.push(image_of(tag)?);
    }
    Ok(BadgeDefinition {
        identifier: identifier(event)?,
        name: one_text(event, "name", "a badge name is one non-empty value")?,
        description: one_text(
            event,
            "description",
            "a badge description is one non-empty value",
        )?,
        image: image.first().map(|tag| image_of(tag)).transpose()?,
        thumbs,
    })
}

/// Read a kind `8` badge award.
///
/// # Errors
///
/// Returns a sentence when the definition address or a recipient is missing.
pub fn open_badge_award(event: &Event) -> Result<BadgeAward, DomainError> {
    if event.kind != AWARD_KIND {
        return Err(invalid("a badge award has kind 8"));
    }
    let addresses = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("a"))
        .collect::<Vec<_>>();
    if addresses.len() != 1 {
        return Err(invalid("a badge award references one definition"));
    }
    let definition = badge_address(addresses[0])?;
    if definition.kind != DEFINITION_KIND {
        return Err(invalid("a badge award references a kind 30009 definition"));
    }
    relay_at(addresses[0], 2)?;
    let mut recipients = Vec::new();
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("p")) {
        let Some(pubkey) = tag.value() else {
            return Err(invalid("an awarded pubkey must be 32 lowercase hex bytes"));
        };
        decode_lower_hex::<32>(pubkey, "awarded pubkey")
            .map_err(|_| invalid("an awarded pubkey must be 32 lowercase hex bytes"))?;
        recipients.push(AwardRecipient {
            pubkey: pubkey.to_owned(),
            relay: relay_at(tag, 2)?,
        });
    }
    if recipients.is_empty() {
        return Err(invalid("a badge award names at least one pubkey"));
    }
    Ok(BadgeAward {
        definition,
        recipients,
    })
}

/// Read a profile badge list from kind `10008` or deprecated kind `30008`.
///
/// # Errors
///
/// Returns a sentence when the kind is neither, or a reference is malformed.
pub fn open_profile_badges(event: &Event) -> Result<ProfileBadges, DomainError> {
    let deprecated_kind = match event.kind {
        PROFILE_KIND => false,
        SET_KIND if identifier(event)? == DEPRECATED_PROFILE => true,
        _ => return Err(invalid("a profile badge list has kind 10008")),
    };
    let (badges, sets) = accepted(&event.tags, true)?;
    Ok(ProfileBadges {
        badges,
        sets,
        deprecated_kind,
    })
}

/// Read a kind `30008` badge set. `d = profile_badges` is a profile list.
///
/// # Errors
///
/// Returns a sentence when the identifier or a pair is not a set.
pub fn open_badge_set(event: &Event) -> Result<BadgeSet, DomainError> {
    if event.kind != SET_KIND {
        return Err(invalid("a badge set has kind 30008"));
    }
    let identifier = identifier(event)?;
    if identifier == DEPRECATED_PROFILE {
        return Err(invalid(
            "a kind 30008 event with d profile_badges is a profile badge list",
        ));
    }
    let (badges, sets) = accepted(&event.tags, false)?;
    if !sets.is_empty() {
        return Err(invalid("a badge set names definition and award pairs"));
    }
    Ok(BadgeSet { identifier, badges })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        DomainError, EventClass, RelaySigner, ReplacementDecision, Tag, compare_replacement,
    };

    fn signer() -> RelaySigner {
        RelaySigner::from_secret_hex(&"58".repeat(32)).unwrap()
    }

    #[test]
    fn a_badge_definition_is_awarded_and_the_profile_lists_it() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../nips/official/58.md"
        ))
        .unwrap();
        assert!(text.contains("kind `30009`"));
        assert!(text.contains("bravery"));
        assert!(text.contains("kind `10008`"));
        assert!(text.contains("profile_badges"));
        let row = crate::lane::PROVEN
            .iter()
            .find(|row| row.file == "58.md")
            .unwrap();
        assert_eq!(row.status, "configured-and-proven");
        assert!(
            crate::lane::SHAPES
                .iter()
                .all(|shape| shape.file != "58.md")
        );

        let issuer = signer();
        let definition = issuer.sign(
            1_700_000_000,
            DEFINITION_KIND,
            vec![
                Tag::new(vec!["d".into(), "bravery".into()]),
                Tag::new(vec!["name".into(), "Medal of Bravery".into()]),
                Tag::new(vec![
                    "description".into(),
                    "Awarded to users demonstrating bravery".into(),
                ]),
                Tag::new(vec![
                    "image".into(),
                    "https://nostr.academy/awards/bravery.png".into(),
                    "1024x1024".into(),
                ]),
                Tag::new(vec![
                    "thumb".into(),
                    "https://nostr.academy/awards/bravery_256x256.png".into(),
                    "256x256".into(),
                ]),
            ],
            String::new(),
        );
        definition.validate_structure().unwrap();
        assert_eq!(definition.class(), EventClass::Addressable);
        let opened = open_badge_definition(&definition).unwrap();
        assert_eq!(opened.identifier, "bravery");
        assert_eq!(opened.name.as_deref(), Some("Medal of Bravery"));
        let image = opened.image.unwrap();
        assert_eq!(image.width, Some(1024));
        assert_eq!(image.height, Some(1024));
        assert_eq!(opened.thumbs[0].width, Some(256));

        let newer = issuer.sign(
            1_700_000_100,
            DEFINITION_KIND,
            vec![
                Tag::new(vec!["d".into(), "bravery".into()]),
                Tag::new(vec!["name".into(), "Medal of Bravery".into()]),
            ],
            String::new(),
        );
        newer.validate_structure().unwrap();
        assert_eq!(
            compare_replacement(&definition, &newer).unwrap(),
            ReplacementDecision::ReplaceCurrent
        );

        let bob = "b0".repeat(32);
        let charlie = "cc".repeat(32);
        let definition_address = format!("30009:{}:bravery", issuer.pubkey());
        let award = issuer.sign(
            1_700_000_200,
            AWARD_KIND,
            vec![
                Tag::new(vec!["a".into(), definition_address.clone()]),
                Tag::new(vec!["p".into(), bob.clone(), "wss://relay.example".into()]),
                Tag::new(vec!["p".into(), charlie, "wss://relay.example".into()]),
            ],
            String::new(),
        );
        award.validate_structure().unwrap();
        assert_eq!(award.class(), EventClass::Regular);
        assert!(matches!(
            compare_replacement(&award, &award),
            Err(DomainError::NotReplaceable)
        ));
        let awarded = open_badge_award(&award).unwrap();
        assert_eq!(awarded.definition.identifier, "bravery");
        assert_eq!(awarded.recipients[0].pubkey, bob);
        assert_eq!(
            awarded.recipients[0].relay.as_deref(),
            Some("wss://relay.example")
        );

        let profile = issuer.sign(
            1_700_000_300,
            PROFILE_KIND,
            vec![
                Tag::new(vec!["a".into(), definition_address.clone()]),
                Tag::new(vec![
                    "e".into(),
                    award.id.clone(),
                    "wss://nostr.academy".into(),
                ]),
                Tag::new(vec!["a".into(), format!("30009:{}:honor", issuer.pubkey())]),
                Tag::new(vec![
                    "a".into(),
                    format!("30008:{}:medals", issuer.pubkey()),
                ]),
            ],
            String::new(),
        );
        profile.validate_structure().unwrap();
        assert_eq!(profile.class(), EventClass::Replaceable);
        let listed = open_profile_badges(&profile).unwrap();
        assert!(!listed.deprecated_kind);
        assert_eq!(listed.badges.len(), 1);
        assert_eq!(listed.badges[0].award_id, award.id);
        assert_eq!(listed.sets.len(), 1);
        assert_eq!(listed.sets[0].identifier, "medals");

        let deprecated = issuer.sign(
            1_700_000_400,
            SET_KIND,
            vec![
                Tag::new(vec!["d".into(), "profile_badges".into()]),
                Tag::new(vec!["a".into(), definition_address.clone()]),
                Tag::new(vec![
                    "e".into(),
                    award.id.clone(),
                    "wss://nostr.academy".into(),
                ]),
            ],
            String::new(),
        );
        deprecated.validate_structure().unwrap();
        assert!(open_profile_badges(&deprecated).unwrap().deprecated_kind);

        let set = issuer.sign(
            1_700_000_500,
            SET_KIND,
            vec![
                Tag::new(vec!["d".into(), "medals".into()]),
                Tag::new(vec!["a".into(), definition_address]),
                Tag::new(vec!["e".into(), award.id, "wss://nostr.academy".into()]),
            ],
            String::new(),
        );
        set.validate_structure().unwrap();
        assert_eq!(set.class(), EventClass::Addressable);
        assert_eq!(open_badge_set(&set).unwrap().badges.len(), 1);

        let missing_name = issuer.sign(
            1_700_000_600,
            DEFINITION_KIND,
            vec![Tag::new(vec!["name".into(), "Medal of Bravery".into()])],
            String::new(),
        );
        assert!(missing_name.validate_structure().is_err());
        let missing_recipient = issuer.sign(
            1_700_000_700,
            AWARD_KIND,
            vec![Tag::new(vec![
                "a".into(),
                format!("30009:{}:bravery", issuer.pubkey()),
            ])],
            String::new(),
        );
        assert!(missing_recipient.validate_structure().is_err());
    }
}
