//! NIP-02 follow lists.
//!
//! A kind `3` event is one author's whole follow list. A newer event from
//! the same author replaces it. Each `p` tag names a profile, an optional
//! relay URL, and an optional petname.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use super::hex::decode_lower_hex;
use super::{DomainError, Tag};

/// One followed profile, in the order the list published it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Follow {
    pub pubkey: String,
    /// Empty when the tag omits a relay.
    pub relay: String,
    /// Empty when the tag omits a petname.
    pub petname: String,
}

/// Read the `p` tags of a follow list.
///
/// # Errors
///
/// Returns an invalid-event error when a `p` tag is not a 32-byte hex key,
/// a `ws://` or `wss://` relay (or an empty relay), and an optional petname.
pub fn parse_follow_list(tags: &[Tag]) -> Result<Vec<Follow>, DomainError> {
    let mut follows = Vec::new();
    let mut seen = BTreeSet::new();
    for tag in tags.iter().filter(|tag| tag.name() == Some("p")) {
        let values = tag.as_slice();
        if values.len() < 2 || values.len() > 4 {
            return Err(DomainError::InvalidEvent(
                "NIP-02 p tag must name a pubkey, an optional relay, and an optional petname"
                    .into(),
            ));
        }
        let pubkey = values[1].clone();
        decode_lower_hex::<32>(&pubkey, "follow pubkey").map_err(|_| {
            DomainError::InvalidEvent("NIP-02 follow pubkey must be 32 lowercase hex bytes".into())
        })?;
        if !seen.insert(pubkey.clone()) {
            return Err(DomainError::InvalidEvent(
                "NIP-02 follow list names a pubkey twice".into(),
            ));
        }
        let relay = values.get(2).cloned().unwrap_or_default();
        if !relay_url(&relay) {
            return Err(DomainError::InvalidEvent(
                "NIP-02 relay must be empty or a ws:// or wss:// URL".into(),
            ));
        }
        let petname = values.get(3).cloned().unwrap_or_default();
        follows.push(Follow {
            pubkey,
            relay,
            petname,
        });
    }
    Ok(follows)
}

/// Append a profile the author newly follows.
///
/// An existing pubkey keeps its place. A new pubkey goes at the end, which
/// is the chronological order the pinned text asks clients to keep.
pub fn append_follow(list: &mut Vec<Follow>, follow: Follow) {
    if let Some(existing) = list
        .iter_mut()
        .find(|current| current.pubkey == follow.pubkey)
    {
        existing.relay = follow.relay;
        existing.petname = follow.petname;
        return;
    }
    list.push(follow);
}

/// The petname path the viewer's lists assign to `target`.
///
/// A direct petname is that name. A petname from someone the viewer follows
/// is `their-name.viewer-name`, and the chain continues through published
/// lists. The shortest path wins. Two paths of the same length use the
/// lexicographically earlier text.
#[must_use]
pub fn displayed_petname(
    viewer_follows: &[Follow],
    published: &BTreeMap<&str, &[Follow]>,
    target: &str,
) -> Option<String> {
    let mut queue = VecDeque::new();
    let mut seen = BTreeSet::new();
    for follow in viewer_follows {
        if follow.petname.is_empty() {
            continue;
        }
        if follow.pubkey == target {
            return Some(follow.petname.clone());
        }
        if seen.insert(follow.pubkey.clone()) {
            queue.push_back((follow.pubkey.clone(), follow.petname.clone()));
        }
    }
    while !queue.is_empty() {
        let width = queue.len();
        let mut found = Vec::new();
        let mut next = BTreeMap::<String, String>::new();
        for _ in 0..width {
            let (person, suffix) = queue.pop_front().expect("width matches the queue");
            let Some(list) = published.get(person.as_str()) else {
                continue;
            };
            for follow in *list {
                if follow.petname.is_empty() {
                    continue;
                }
                let name = format!("{}.{}", follow.petname, suffix);
                if follow.pubkey == target {
                    found.push(name);
                    continue;
                }
                if seen.contains(&follow.pubkey) {
                    continue;
                }
                next.entry(follow.pubkey.clone())
                    .and_modify(|current| {
                        if name < *current {
                            *current = name.clone();
                        }
                    })
                    .or_insert(name);
            }
        }
        if !found.is_empty() {
            found.sort();
            return found.into_iter().next();
        }
        for (pubkey, name) in next {
            seen.insert(pubkey.clone());
            queue.push_back((pubkey, name));
        }
    }
    None
}

fn relay_url(value: &str) -> bool {
    if value.is_empty() {
        return true;
    }
    let Some(rest) = value
        .strip_prefix("wss://")
        .or_else(|| value.strip_prefix("ws://"))
    else {
        return false;
    };
    !rest.is_empty() && value.len() <= 2_048 && !value.chars().any(char::is_whitespace)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(byte: u8) -> String {
        format!("{byte:02x}").repeat(32)
    }

    fn follow(pubkey: &str, relay: &str, petname: &str) -> Follow {
        Follow {
            pubkey: pubkey.to_owned(),
            relay: relay.to_owned(),
            petname: petname.to_owned(),
        }
    }

    #[test]
    fn a_follow_list_keeps_pubkey_relay_and_petname_in_order() {
        let tags = vec![
            Tag::new(vec![
                "p".into(),
                key(1),
                "wss://alicerelay.com/".into(),
                "alice".into(),
            ]),
            Tag::new(vec!["p".into(), key(2), "".into(), "bob".into()]),
            Tag::new(vec!["p".into(), key(3)]),
        ];
        let follows = parse_follow_list(&tags).unwrap();
        assert_eq!(
            follows,
            vec![
                follow(&key(1), "wss://alicerelay.com/", "alice"),
                follow(&key(2), "", "bob"),
                follow(&key(3), "", ""),
            ]
        );
        assert!(parse_follow_list(&[Tag::new(vec!["p".into(), "zz".into()])]).is_err());
        assert!(
            parse_follow_list(&[Tag::new(vec!["p".into(), key(1), "https://nope".into()])])
                .is_err()
        );
    }

    #[test]
    fn new_follows_append_and_petnames_chain_through_published_lists() {
        let erin = key(1);
        let david = key(2);
        let frank = key(3);
        let mut viewer = vec![follow(&erin, "", "erin")];
        append_follow(
            &mut viewer,
            follow(&david, "wss://david.example", "david-direct"),
        );
        assert_eq!(viewer[0].petname, "erin");
        assert_eq!(viewer[1].pubkey, david);
        append_follow(&mut viewer, follow(&erin, "wss://erin.example", "erin"));
        assert_eq!(viewer.len(), 2);
        assert_eq!(viewer[0].relay, "wss://erin.example");

        let erin_list = [follow(&david, "", "david")];
        let david_list = [follow(&frank, "", "frank")];
        let mut published = BTreeMap::new();
        published.insert(erin.as_str(), erin_list.as_slice());
        published.insert(david.as_str(), david_list.as_slice());
        let viewer_only = [follow(&erin, "", "erin")];
        assert_eq!(
            displayed_petname(&viewer_only, &published, &erin).as_deref(),
            Some("erin")
        );
        assert_eq!(
            displayed_petname(&viewer_only, &published, &david).as_deref(),
            Some("david.erin")
        );
        assert_eq!(
            displayed_petname(&viewer_only, &published, &frank).as_deref(),
            Some("frank.david.erin")
        );
    }
}
