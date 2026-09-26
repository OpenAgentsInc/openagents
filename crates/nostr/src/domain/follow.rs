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
/// is `~/viewer-name/their-name`, in traversal order. Only unambiguous
/// ASCII letters, digits, and underscore labels participate in indirect paths. The shortest path wins. Two paths of the same length use the
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
        if resolvable_label(viewer_follows, &follow.petname) && seen.insert(follow.pubkey.clone()) {
            queue.push_back((follow.pubkey.clone(), format!("~/{}", follow.petname)));
        }
    }
    while !queue.is_empty() {
        let width = queue.len();
        let mut found = Vec::new();
        let mut next = BTreeMap::<String, String>::new();
        for _ in 0..width {
            let (person, prefix) = queue.pop_front().expect("width matches the queue");
            let Some(list) = published.get(person.as_str()) else {
                continue;
            };
            for follow in *list {
                if !resolvable_label(list, &follow.petname) {
                    continue;
                }
                let name = format!("{prefix}/{}", follow.petname);
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

/// Whether a display label qualifies for NIP-02 name-path resolution.
#[must_use]
pub fn petname_is_resolvable(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn resolvable_label(list: &[Follow], name: &str) -> bool {
    petname_is_resolvable(name) && list.iter().filter(|follow| follow.petname == name).count() == 1
}

/// Resolve a relative or absolute petname path without network access.
///
/// Relative `~/alice/bob` (or a direct `alice`) uses `viewer`. Absolute roots
/// accept `~npub1...` or a NIP-05 name from `verified_roots`. The caller must
/// have independently verified those name-to-key bindings; this function does
/// not fetch or authenticate them. Missing roots, lists, or labels return
/// `None`, never a guessed key. Follow lists must be keyed by their verified
/// author and selected using normal replacement rules.
///
/// # Errors
///
/// Returns an error for invalid path syntax, malformed keys, or an ambiguous
/// component. Non-resolvable petnames remain valid display labels.
pub fn resolve_petname(
    path: &str,
    viewer: Option<&str>,
    published: &BTreeMap<&str, &[Follow]>,
    verified_roots: &BTreeMap<&str, &str>,
) -> Result<Option<String>, DomainError> {
    let (root, tail) = if let Some(tail) = path.strip_prefix("~/") {
        (viewer.map(str::to_owned), tail)
    } else if let Some(absolute) = path.strip_prefix('~') {
        let (root, tail) = absolute.split_once('/').ok_or_else(|| {
            DomainError::InvalidEvent("an absolute petname path needs a root and name".into())
        })?;
        let key = if root.starts_with("npub1") {
            let (prefix, bytes) = crate::nip19::decode(root).map_err(|_| {
                DomainError::InvalidEvent("a petname root must be a valid npub".into())
            })?;
            if prefix != "npub" || bytes.len() != 32 {
                return Err(DomainError::InvalidEvent(
                    "a petname root must be an npub".into(),
                ));
            }
            Some(super::hex::encode_lower_hex(&bytes))
        } else {
            crate::lane::nip05_identifier(root).map_err(|_| {
                DomainError::InvalidEvent(
                    "a petname root must be an npub or verified NIP-05 name".into(),
                )
            })?;
            verified_roots.get(root).map(|key| (*key).to_owned())
        };
        (key, tail)
    } else {
        (viewer.map(str::to_owned), path)
    };
    let components: Vec<_> = tail.split('/').collect();
    if components.iter().any(|name| !petname_is_resolvable(name)) {
        return Err(DomainError::InvalidEvent(
            "petname components use ASCII letters, digits, and underscores".into(),
        ));
    }
    let Some(mut current) = root else {
        return Ok(None);
    };
    decode_lower_hex::<32>(&current, "petname root")?;
    for component in components {
        let Some(list) = published.get(current.as_str()) else {
            return Ok(None);
        };
        let mut candidates = list.iter().filter(|follow| follow.petname == component);
        let Some(found) = candidates.next() else {
            return Ok(None);
        };
        if candidates.next().is_some() {
            return Err(DomainError::InvalidEvent(
                "a petname component names more than one profile".into(),
            ));
        }
        decode_lower_hex::<32>(&found.pubkey, "petname target")?;
        current = found.pubkey.clone();
    }
    Ok(Some(current))
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
            Some("~/erin/david")
        );
        assert_eq!(
            displayed_petname(&viewer_only, &published, &frank).as_deref(),
            Some("~/erin/david/frank")
        );
    }
    #[test]
    fn name_paths_resolve_forward_from_verified_roots_and_reject_ambiguity() {
        let viewer = key(8);
        let erin = key(1);
        let charlie = key(2);
        let first = [
            follow(&erin, "", "erin"),
            follow(&charlie, "", "not-a-path"),
        ];
        let second = [follow(&charlie, "", "charlie"), follow(&viewer, "", "back")];
        let published = BTreeMap::from([
            (viewer.as_str(), first.as_slice()),
            (erin.as_str(), second.as_slice()),
        ]);
        let roots = BTreeMap::from([("carol@names.com", viewer.as_str())]);
        for path in [
            "~/erin/charlie",
            "~carol@names.com/erin/charlie",
            "~/erin/back/erin/charlie",
        ] {
            assert_eq!(
                resolve_petname(path, Some(&viewer), &published, &roots).unwrap(),
                Some(charlie.clone())
            );
        }
        let npub =
            crate::nip19::encode_npub(&super::decode_lower_hex::<32>(&viewer, "key").unwrap());
        assert_eq!(
            resolve_petname(&format!("~{npub}/erin/charlie"), None, &published, &roots).unwrap(),
            Some(charlie.clone())
        );
        assert_eq!(
            resolve_petname("~/erin/charlie", None, &published, &roots).unwrap(),
            None
        );
        assert_eq!(
            resolve_petname("~absent@names.com/erin", None, &published, &roots).unwrap(),
            None
        );
        assert_eq!(
            resolve_petname("~/missing", Some(&viewer), &published, &roots).unwrap(),
            None
        );
        for path in [
            "~/",
            "~/erin//charlie",
            "~/erin/../charlie",
            "~/not-a-path",
            "~/é",
        ] {
            assert!(
                resolve_petname(path, Some(&viewer), &published, &roots).is_err(),
                "{path}"
            );
        }
        let ambiguous = [follow(&erin, "", "same"), follow(&charlie, "", "same")];
        let lists = BTreeMap::from([(viewer.as_str(), ambiguous.as_slice())]);
        assert!(resolve_petname("~/same", Some(&viewer), &lists, &roots).is_err());
        // Display labels with punctuation remain available, but cannot be paths.
        assert_eq!(
            displayed_petname(&first, &published, &charlie).as_deref(),
            Some("not-a-path")
        );
        assert_eq!(displayed_petname(&first, &published, &key(9)), None);
    }
}
