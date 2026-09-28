//! Trainers: which keys asked to have their level shown, read from NIP-XP
//! trainer profiles (`13193`).
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
        let profiles = by_author
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
        Trainers { profiles }
    }

    /// The trainer `key` belongs to: the key itself when it published a
    /// profile.
    #[must_use]
    pub fn trainer_of(&self, key: &str) -> Option<&str> {
        self.profiles.get_key_value(key).map(|(k, _)| k.as_str())
    }

    /// The keys whose XP is `trainer`'s: the trainer's own key.
    #[must_use]
    pub fn keys_of(&self, trainer: &str) -> Vec<String> {
        vec![trainer.to_owned()]
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
