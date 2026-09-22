//! NIP-72 moderated communities.
//!
//! Kind `34550` is an addressable community. Its author and any `p`
//! tag marked `moderator` may approve posts. `relay` tags name the
//! author, request, and approval relays. A kind `1111` post whose `K`
//! tag is `34550` is rooted at the community. Kind `4550` approves one
//! post. A cross-post is a kind `6` or `16` repost whose `a` tag names
//! the community and whose content is the original event.
//!
//! The relay does not fetch posts or rank approvals. A top-level post
//! includes the NIP-22 `e` version tag. Kind `1` is not admitted as a
//! new community post. Deleting an approval is a kind `5` request.
//! NIP-72 is unrecommended, so these kinds stay off the NIP-11 list.

use std::str::FromStr;

use super::comment::CommentScope;
use super::hex::decode_lower_hex;
use super::{DomainError, Event, ReplacementAddress};

const COMMUNITY_KIND: u16 = 34_550;
const APPROVAL_KIND: u16 = 4_550;
const POST_KIND: u16 = 1_111;

/// Where a community relay is used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommunityRelayRole {
    Author,
    Requests,
    Approvals,
    Unmarked,
}

/// One relay in a community definition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommunityRelay {
    pub url: String,
    pub role: CommunityRelayRole,
}

/// A listed moderator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommunityModerator {
    pub pubkey: String,
    pub relay: Option<String>,
}

/// An image and its optional pixel size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommunityImage {
    pub url: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

/// A kind `34550` community.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Community {
    pub founder: String,
    pub identifier: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub image: Option<CommunityImage>,
    pub moderators: Vec<CommunityModerator>,
    pub relays: Vec<CommunityRelay>,
}

/// A kind `1111` post rooted at a community.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommunityPost {
    pub community: ReplacementAddress,
    pub top_level: bool,
    pub content: String,
}

/// A kind `4550` approval.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommunityApproval {
    pub communities: Vec<ReplacementAddress>,
    pub post_id: Option<String>,
    pub post_address: Option<ReplacementAddress>,
    pub post_author: String,
    pub post_kind: Option<u16>,
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
    (value.starts_with("wss://") || value.starts_with("ws://"))
        && value.len() > "wss://".len()
        && value.len() <= 2_048
        && !value.chars().any(char::is_whitespace)
}

fn pubkey(value: &str) -> Result<String, DomainError> {
    decode_lower_hex::<32>(value, "community pubkey")
        .map_err(|_| invalid("a community pubkey is 32 lowercase hex bytes"))?;
    Ok(value.to_owned())
}

fn event_id(value: &str) -> Result<String, DomainError> {
    decode_lower_hex::<32>(value, "community event")
        .map_err(|_| invalid("a community event id is 32 lowercase hex bytes"))?;
    Ok(value.to_owned())
}

fn identifier(value: &str) -> Result<String, DomainError> {
    if value.is_empty() || value.len() > 1_024 || value.chars().any(char::is_control) {
        return Err(invalid("a community identifier is 1 to 1024 characters"));
    }
    Ok(value.to_owned())
}

fn text(value: &str, reason: &str) -> Result<String, DomainError> {
    if value.is_empty() || value.len() > 4_096 || value.chars().any(char::is_control) {
        return Err(invalid(reason));
    }
    Ok(value.to_owned())
}

fn one<'a>(
    event: &'a Event,
    name: &str,
    reason: &str,
) -> Result<Option<&'a super::Tag>, DomainError> {
    let mut found = event.tags.iter().filter(|tag| tag.name() == Some(name));
    let tag = found.next();
    if found.next().is_some() {
        return Err(invalid(reason));
    }
    Ok(tag)
}

fn side(value: &str) -> Result<u32, DomainError> {
    if value.is_empty()
        || value == "0"
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(invalid("a community image size is widthxheight"));
    }
    value
        .parse()
        .map_err(|_| invalid("a community image size is widthxheight"))
}

fn image(event: &Event) -> Result<Option<CommunityImage>, DomainError> {
    let Some(tag) = one(event, "image", "a community has one image")? else {
        return Ok(None);
    };
    let parts = tag.as_slice();
    if parts.len() < 2 || parts.len() > 3 || !http_url(&parts[1]) {
        return Err(invalid("a community image is an http:// or https:// URL"));
    }
    let (width, height) = match parts.get(2) {
        None => (None, None),
        Some(value) => {
            let Some((width, height)) = value.split_once('x') else {
                return Err(invalid("a community image size is widthxheight"));
            };
            (Some(side(width)?), Some(side(height)?))
        }
    };
    Ok(Some(CommunityImage {
        url: parts[1].clone(),
        width,
        height,
    }))
}

fn moderators(event: &Event) -> Result<Vec<CommunityModerator>, DomainError> {
    let mut people = Vec::new();
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("p")) {
        let parts = tag.as_slice();
        if parts.len() != 4 || parts[3] != "moderator" {
            return Err(invalid("a community moderator is marked moderator"));
        }
        let pubkey = pubkey(&parts[1])?;
        if people
            .iter()
            .any(|person: &CommunityModerator| person.pubkey == pubkey)
        {
            return Err(invalid("a community moderator is listed once"));
        }
        let relay = if parts[2].is_empty() {
            None
        } else if relay_url(&parts[2]) {
            Some(parts[2].clone())
        } else {
            return Err(invalid("a moderator relay is ws:// or wss://"));
        };
        people.push(CommunityModerator { pubkey, relay });
    }
    Ok(people)
}

fn relays(event: &Event) -> Result<Vec<CommunityRelay>, DomainError> {
    let mut relays = Vec::new();
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("relay")) {
        let parts = tag.as_slice();
        if parts.len() < 2 || parts.len() > 3 || !relay_url(&parts[1]) {
            return Err(invalid("a community relay is ws:// or wss://"));
        }
        let role = match parts.get(2).map(String::as_str) {
            None => CommunityRelayRole::Unmarked,
            Some("author") => CommunityRelayRole::Author,
            Some("requests") => CommunityRelayRole::Requests,
            Some("approvals") => CommunityRelayRole::Approvals,
            Some(_) => {
                return Err(invalid(
                    "a community relay marker is author, requests, or approvals",
                ));
            }
        };
        if relays
            .iter()
            .any(|relay: &CommunityRelay| relay.url == parts[1] && relay.role == role)
        {
            return Err(invalid("a community relay is listed once"));
        }
        relays.push(CommunityRelay {
            url: parts[1].clone(),
            role,
        });
    }
    Ok(relays)
}

/// Read a kind `34550` community.
pub fn open_community(event: &Event) -> Result<Community, DomainError> {
    if event.kind != COMMUNITY_KIND {
        return Err(invalid("a community has kind 34550"));
    }
    let Some(tag) = one(event, "d", "a community has one identifier")? else {
        return Err(invalid("a community has one identifier"));
    };
    let Some(value) = tag.value() else {
        return Err(invalid("a community has one identifier"));
    };
    let name = match one(event, "name", "a community name is text")? {
        None => None,
        Some(tag) => Some(text(
            tag.value()
                .ok_or_else(|| invalid("a community name is text"))?,
            "a community name is text",
        )?),
    };
    let description = match one(event, "description", "a community description is text")? {
        None => None,
        Some(tag) => Some(text(
            tag.value()
                .ok_or_else(|| invalid("a community description is text"))?,
            "a community description is text",
        )?),
    };
    Ok(Community {
        founder: event.pubkey.clone(),
        identifier: identifier(value)?,
        name,
        description,
        image: image(event)?,
        moderators: moderators(event)?,
        relays: relays(event)?,
    })
}

/// The founder and listed moderators can approve posts.
pub fn is_community_moderator(community: &Community, pubkey: &str) -> bool {
    pubkey == community.founder
        || community
            .moderators
            .iter()
            .any(|moderator| moderator.pubkey == pubkey)
}

/// Read a kind `1111` post rooted at a community.
pub fn open_community_post(event: &Event) -> Result<CommunityPost, DomainError> {
    if event.kind != POST_KIND {
        return Err(invalid("a community post has kind 1111"));
    }
    if event.content.is_empty() {
        return Err(invalid("a community post has text"));
    }
    let comment = super::comment::comment_scopes(event)?;
    let CommentScope::Address { address, .. } = &comment.root else {
        return Err(invalid("a community post is rooted at the community"));
    };
    if address.kind != COMMUNITY_KIND {
        return Err(invalid("a community post is rooted at the community"));
    }
    Ok(CommunityPost {
        community: address.clone(),
        top_level: super::comment::is_top_level(&comment),
        content: event.content.clone(),
    })
}

fn community_addresses(event: &Event) -> Result<Vec<ReplacementAddress>, DomainError> {
    let mut communities = Vec::new();
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("a")) {
        let Some(value) = tag.value() else {
            return Err(invalid("a community address names a community"));
        };
        let address = ReplacementAddress::from_str(value)
            .map_err(|_| invalid("a community address names a community"))?;
        if address.kind == COMMUNITY_KIND {
            if let Some(relay) = tag.as_slice().get(2)
                && !relay.is_empty()
                && !relay_url(relay)
            {
                return Err(invalid("a community relay is ws:// or wss://"));
            }
            communities.push(address);
        }
    }
    if communities.is_empty() {
        return Err(invalid("an approval names one or more communities"));
    }
    Ok(communities)
}

/// Read a kind `4550` approval.
pub fn open_community_approval(event: &Event) -> Result<CommunityApproval, DomainError> {
    if event.kind != APPROVAL_KIND {
        return Err(invalid("a community approval has kind 4550"));
    }
    let communities = community_addresses(event)?;
    let mut post_id = None;
    let mut post_address = None;
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("e")) {
        if post_id.is_some() {
            return Err(invalid("an approval names one post"));
        }
        let Some(value) = tag.value() else {
            return Err(invalid("an approval names one post"));
        };
        post_id = Some(event_id(value)?);
        if let Some(relay) = tag.as_slice().get(2)
            && !relay.is_empty()
            && !relay_url(relay)
        {
            return Err(invalid("a community relay is ws:// or wss://"));
        }
    }
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("a")) {
        let Some(value) = tag.value() else {
            continue;
        };
        let address = ReplacementAddress::from_str(value)
            .map_err(|_| invalid("an approval names the post address"))?;
        if address.kind == COMMUNITY_KIND {
            continue;
        }
        if post_address.is_some() {
            return Err(invalid("an approval names one post address"));
        }
        post_address = Some(address);
    }
    if post_id.is_none() && post_address.is_none() {
        return Err(invalid("an approval names the post"));
    }
    let authors: Vec<_> = event.tag_values("p").collect();
    if authors.len() != 1 {
        return Err(invalid("an approval names the post author"));
    }
    let post_author = pubkey(authors[0])?;
    let post_kind = match one(event, "k", "an approval has one k tag")? {
        None => None,
        Some(tag) => Some(
            tag.value()
                .ok_or_else(|| invalid("an approval k tag is an event kind"))?
                .parse::<u16>()
                .map_err(|_| invalid("an approval k tag is an event kind"))?,
        ),
    };
    if post_id.is_some() {
        let inner: Event = serde_json::from_str(&event.content)
            .map_err(|_| invalid("an approval of a version embeds that event"))?;
        inner
            .validate_nip01_structure()
            .map_err(|_| invalid("an approval of a version embeds that event"))?;
        inner
            .validate_crypto()
            .map_err(|_| invalid("an approval of a version embeds a signed event"))?;
        if Some(inner.id.as_str()) != post_id.as_deref() {
            return Err(invalid("an approval e tag matches the embedded event"));
        }
        if inner.pubkey != post_author {
            return Err(invalid("an approval names the post author"));
        }
        if let Some(kind) = post_kind
            && kind != inner.kind
        {
            return Err(invalid("an approval k tag matches the post kind"));
        }
    }
    Ok(CommunityApproval {
        communities,
        post_id,
        post_address,
        post_author,
        post_kind,
    })
}

/// A cross-post names the community and embeds the original event.
pub fn open_community_repost(event: &Event) -> Result<Vec<ReplacementAddress>, DomainError> {
    let repost = super::repost::open_repost(event)?;
    if event.content.is_empty() || repost.embedded_kind == Some(APPROVAL_KIND) {
        return Err(invalid("a community cross-post embeds the original event"));
    }
    community_addresses(event)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        DomainError, EventClass, RelaySigner, ReplacementDecision, Tag, compare_replacement,
    };

    fn signer(byte: &str) -> RelaySigner {
        RelaySigner::from_secret_hex(&byte.repeat(32)).unwrap()
    }

    #[test]
    fn a_community_lists_moderators_and_a_moderator_approves_a_post() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../nips/official/72.md"
        ))
        .unwrap();
        assert!(text.contains("34550"));
        assert!(text.contains("4550"));
        assert!(text.contains("1111"));
        assert!(text.contains("moderator"));
        let row = crate::lane::PROVEN
            .iter()
            .find(|row| row.file == "72.md")
            .unwrap();
        assert_eq!(row.status, "configured-and-proven");
        assert!(
            crate::lane::SHAPES
                .iter()
                .all(|shape| shape.file != "72.md")
        );

        let founder = signer("72");
        let moderator = signer("73");
        let member = signer("74");
        let stranger = signer("75");
        let relay = "wss://relay.example";
        let community = founder.sign(
            1_700_000_000,
            COMMUNITY_KIND,
            vec![
                Tag::new(vec!["d".into(), "gardening".into()]),
                Tag::new(vec!["name".into(), "Garden Club".into()]),
                Tag::new(vec![
                    "description".into(),
                    "A place to talk about gardens.".into(),
                ]),
                Tag::new(vec![
                    "image".into(),
                    "https://cdn.example/garden.png".into(),
                    "640x480".into(),
                ]),
                Tag::new(vec![
                    "p".into(),
                    moderator.pubkey().to_owned(),
                    relay.into(),
                    "moderator".into(),
                ]),
                Tag::new(vec![
                    "relay".into(),
                    "wss://profiles.example".into(),
                    "author".into(),
                ]),
                Tag::new(vec![
                    "relay".into(),
                    "wss://requests.example".into(),
                    "requests".into(),
                ]),
                Tag::new(vec![
                    "relay".into(),
                    "wss://approvals.example".into(),
                    "approvals".into(),
                ]),
            ],
            String::new(),
        );
        community.validate_structure().unwrap();
        assert_eq!(community.class(), EventClass::Addressable);
        let opened = open_community(&community).unwrap();
        assert_eq!(opened.identifier, "gardening");
        assert_eq!(opened.name.as_deref(), Some("Garden Club"));
        assert_eq!(
            opened.image.as_ref().map(|image| image.width),
            Some(Some(640))
        );
        assert_eq!(opened.moderators.len(), 1);
        assert!(is_community_moderator(&opened, founder.pubkey()));
        assert!(is_community_moderator(&opened, moderator.pubkey()));
        assert!(!is_community_moderator(&opened, stranger.pubkey()));
        assert!(
            opened
                .relays
                .iter()
                .any(|item| item.role == CommunityRelayRole::Requests)
        );

        let revised = founder.sign(
            1_700_000_100,
            COMMUNITY_KIND,
            vec![
                Tag::new(vec!["d".into(), "gardening".into()]),
                Tag::new(vec!["description".into(), "Still about gardens.".into()]),
            ],
            String::new(),
        );
        revised.validate_structure().unwrap();
        assert_eq!(
            compare_replacement(&community, &revised),
            Ok(ReplacementDecision::ReplaceCurrent)
        );

        let address = format!("34550:{}:gardening", founder.pubkey());
        let post = member.sign(
            1_700_000_200,
            POST_KIND,
            vec![
                Tag::new(vec!["A".into(), address.clone(), relay.into()]),
                Tag::new(vec!["K".into(), "34550".into()]),
                Tag::new(vec!["P".into(), founder.pubkey().to_owned(), relay.into()]),
                Tag::new(vec!["a".into(), address.clone(), relay.into()]),
                Tag::new(vec![
                    "e".into(),
                    community.id.clone(),
                    relay.into(),
                    founder.pubkey().to_owned(),
                ]),
                Tag::new(vec!["k".into(), "34550".into()]),
                Tag::new(vec!["p".into(), founder.pubkey().to_owned(), relay.into()]),
            ],
            "Hi everyone. It's great to be here!".into(),
        );
        post.validate_structure().unwrap();
        let opened_post = open_community_post(&post).unwrap();
        assert!(opened_post.top_level);
        assert_eq!(opened_post.community.identifier, "gardening");
        assert_eq!(opened_post.community.pubkey, founder.pubkey());

        let reply = stranger.sign(
            1_700_000_250,
            POST_KIND,
            vec![
                Tag::new(vec!["A".into(), address.clone(), relay.into()]),
                Tag::new(vec!["K".into(), "34550".into()]),
                Tag::new(vec!["P".into(), founder.pubkey().to_owned(), relay.into()]),
                Tag::new(vec![
                    "e".into(),
                    post.id.clone(),
                    relay.into(),
                    member.pubkey().to_owned(),
                ]),
                Tag::new(vec!["k".into(), "1111".into()]),
                Tag::new(vec!["p".into(), member.pubkey().to_owned(), relay.into()]),
            ],
            "Agreed! Welcome everyone!".into(),
        );
        reply.validate_structure().unwrap();
        assert!(!open_community_post(&reply).unwrap().top_level);

        let approval = moderator.sign(
            1_700_000_300,
            APPROVAL_KIND,
            vec![
                Tag::new(vec!["a".into(), address.clone(), relay.into()]),
                Tag::new(vec!["e".into(), post.id.clone(), relay.into()]),
                Tag::new(vec!["p".into(), member.pubkey().to_owned(), relay.into()]),
                Tag::new(vec!["k".into(), "1111".into()]),
            ],
            serde_json::to_string(&post).unwrap(),
        );
        approval.validate_structure().unwrap();
        assert_eq!(approval.class(), EventClass::Regular);
        let opened_approval = open_community_approval(&approval).unwrap();
        assert_eq!(opened_approval.post_id.as_deref(), Some(post.id.as_str()));
        assert_eq!(opened_approval.post_author, member.pubkey());
        assert_eq!(opened_approval.post_kind, Some(POST_KIND));
        assert!(is_community_moderator(&opened, moderator.pubkey()));
        assert!(matches!(
            compare_replacement(&approval, &approval),
            Err(DomainError::NotReplaceable)
        ));

        let outside = stranger.sign(
            1_700_000_350,
            APPROVAL_KIND,
            vec![
                Tag::new(vec!["a".into(), address.clone(), relay.into()]),
                Tag::new(vec!["e".into(), post.id.clone(), relay.into()]),
                Tag::new(vec!["p".into(), member.pubkey().to_owned()]),
                Tag::new(vec!["k".into(), "1111".into()]),
            ],
            serde_json::to_string(&post).unwrap(),
        );
        outside.validate_structure().unwrap();
        assert!(!is_community_moderator(&opened, outside.pubkey.as_str()));

        let missing = moderator.sign(
            1_700_000_360,
            APPROVAL_KIND,
            vec![
                Tag::new(vec!["a".into(), address.clone()]),
                Tag::new(vec!["e".into(), post.id.clone()]),
                Tag::new(vec!["p".into(), member.pubkey().to_owned()]),
            ],
            String::new(),
        );
        assert!(missing.validate_structure().is_err());

        let note = member.sign(1_700_000_400, 1, Vec::new(), "Look at this tomato.".into());
        note.validate_structure().unwrap();
        let cross_post = member.sign(
            1_700_000_500,
            6,
            vec![
                Tag::new(vec!["e".into(), note.id.clone(), relay.into()]),
                Tag::new(vec!["p".into(), member.pubkey().to_owned()]),
                Tag::new(vec!["a".into(), address]),
            ],
            serde_json::to_string(&note).unwrap(),
        );
        cross_post.validate_structure().unwrap();
        let communities = open_community_repost(&cross_post).unwrap();
        assert_eq!(communities[0].identifier, "gardening");

        let unmarked = founder.sign(
            1_700_000_600,
            COMMUNITY_KIND,
            vec![
                Tag::new(vec!["d".into(), "weeds".into()]),
                Tag::new(vec!["p".into(), moderator.pubkey().to_owned()]),
            ],
            String::new(),
        );
        assert!(unmarked.validate_structure().is_err());
    }
}
