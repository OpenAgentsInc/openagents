//! Trainers: which keys asked to have their level shown, read from NIP-XP
//! trainer profiles (`13193`), and which keys belong to one trainer, read
//! from two-sided key links (`13195`).
//!
//! The ledger counts every key's awards whether or not it published a
//! profile. A profile only decides whether a client advertises the key: a
//! name tag or a rank board shows a level only for a trainer whose newest
//! profile says `shown: true`.

use std::collections::BTreeMap;

use nostr::domain::Event;
use nostr::xp;
use serde::Serialize;

/// One trainer's newest valid profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Profile {
    /// The `13193` event ID.
    pub event: String,
    pub created_at: u64,
    /// Whether the trainer asked clients to show its level.
    pub shown: bool,
    /// The other keys the profile lists, hex. Each counts only with a
    /// link back.
    pub keys: Vec<String>,
}

/// The trainers a set of events describes.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Trainers {
    /// Each author's newest valid profile, by hex public key.
    pub profiles: BTreeMap<String, Profile>,
    /// Each key's newest valid link: the trainer it names, or `None` for a
    /// withdrawn link.
    pub links: BTreeMap<String, Option<String>>,
    /// Keys linked both ways, to their trainer: the trainer's profile lists
    /// the key and the key's link names the trainer.
    pub linked: BTreeMap<String, String>,
}

impl Trainers {
    /// Reads the newest valid profile of each author in `events`. Invalid
    /// profiles are skipped; events may repeat.
    #[must_use]
    pub fn read(events: &[Event]) -> Self {
        let mut by_author: BTreeMap<&str, Vec<&Event>> = BTreeMap::new();
        for event in events.iter().filter(|e| e.kind == xp::PROFILE_KIND) {
            if xp::parse_profile(event).is_ok() {
                by_author.entry(&event.pubkey).or_default().push(event);
            }
        }
        let profiles: BTreeMap<String, Profile> = by_author
            .into_iter()
            .filter_map(|(author, events)| {
                let event = xp::trainer::newest(events)?;
                let parsed = xp::parse_profile(event).ok()?;
                Some((
                    author.to_owned(),
                    Profile {
                        event: event.id.clone(),
                        created_at: event.created_at,
                        shown: parsed.shown,
                        keys: parsed.keys,
                    },
                ))
            })
            .collect();
        let mut by_key: BTreeMap<&str, Vec<&Event>> = BTreeMap::new();
        for event in events.iter().filter(|e| e.kind == xp::LINK_KIND) {
            if xp::parse_link(event).is_ok() {
                by_key.entry(&event.pubkey).or_default().push(event);
            }
        }
        let links: BTreeMap<String, Option<String>> = by_key
            .into_iter()
            .filter_map(|(key, events)| {
                let event = xp::trainer::newest(events)?;
                Some((key.to_owned(), xp::parse_link(event).ok()?.trainer))
            })
            .collect();
        // Both sides signed: the trainer's profile lists the key, and the
        // key's link names the trainer.
        let mutual: BTreeMap<&str, &str> = profiles
            .iter()
            .flat_map(|(trainer, profile): (&String, &Profile)| {
                profile
                    .keys
                    .iter()
                    .map(move |key| (key.as_str(), trainer.as_str()))
            })
            .filter(|(key, trainer)| links.get(*key).and_then(Option::as_deref) == Some(*trainer))
            .collect();
        // One level deep: a trainer that is itself linked to another
        // trainer holds no keys of its own.
        let linked = mutual
            .iter()
            .filter(|(_, trainer)| !mutual.contains_key(**trainer))
            .map(|(key, trainer)| ((*key).to_owned(), (*trainer).to_owned()))
            .collect();
        Trainers {
            profiles,
            links,
            linked,
        }
    }

    /// The trainer `key` belongs to: the trainer it is linked to both
    /// ways, else the key itself when it published a profile and isn't
    /// linked to another trainer.
    #[must_use]
    pub fn trainer_of(&self, key: &str) -> Option<&str> {
        if let Some(trainer) = self.linked.get(key) {
            return Some(trainer);
        }
        if self.is_linked_elsewhere(key) {
            return None;
        }
        self.profiles.get_key_value(key).map(|(k, _)| k.as_str())
    }

    /// A key both sides linked to a trainer that is itself linked onward:
    /// it belongs to no trainer.
    fn is_linked_elsewhere(&self, key: &str) -> bool {
        self.links
            .get(key)
            .and_then(Option::as_deref)
            .and_then(|trainer| self.profiles.get(trainer))
            .is_some_and(|profile| profile.keys.iter().any(|k| k == key))
    }

    /// The keys whose XP is `trainer`'s: the trainer's own key, then the
    /// keys linked to it both ways, each once.
    #[must_use]
    pub fn keys_of(&self, trainer: &str) -> Vec<String> {
        let mut keys = vec![trainer.to_owned()];
        keys.extend(
            self.linked
                .iter()
                .filter(|(_, t)| t.as_str() == trainer)
                .map(|(key, _)| key.clone()),
        );
        keys
    }

    /// The keys `trainer`'s profile lists that haven't linked back yet.
    #[must_use]
    pub fn waiting(&self, trainer: &str) -> Vec<String> {
        self.profiles.get(trainer).map_or_else(Vec::new, |profile| {
            profile
                .keys
                .iter()
                .filter(|key| self.linked.get(*key).map(String::as_str) != Some(trainer))
                .cloned()
                .collect()
        })
    }

    /// Whether a client may show `key`'s level on a name tag or a board:
    /// its trainer's newest profile says `shown: true`.
    #[must_use]
    pub fn shown(&self, key: &str) -> bool {
        self.trainer_of(key)
            .and_then(|trainer| self.profiles.get(trainer))
            .is_some_and(|profile| profile.shown)
    }
}
