//! The owner host directory: an owner-signed, owner-encrypted list of hosts.
//!
//! Only the owner key signs and reads it. A host cannot add itself: an
//! artifact signed by any other key refuses, whatever its body says.

use nostr::domain::Event;
use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    Error, Refusal, Result, artifact, fail, label, parse_pubkey, pubkey, random_id, relay_url,
    requires,
};

/// Schema of a directory body.
pub const SCHEMA: &str = "openagents.host-directory.v1";
/// Most hosts one directory lists.
pub const MAX_HOSTS: usize = 256;
/// Most relays one entry lists.
pub const MAX_RELAYS: usize = 8;
/// Largest placement weight an owner can assign.
pub const MAX_WEIGHT: u32 = 1000;
/// Largest label, in UTF-8 bytes.
pub const MAX_LABEL_BYTES: usize = 64;
/// Most world instances one entry advertises.
pub const MAX_WORLDS: usize = 16;

/// A world instance a host serves over its direct channel. Joining it takes
/// a NIP-HOST grant with the `world` right; the entry grants nothing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorldInstance {
    /// The instance number the world's opening challenge names; never zero.
    pub instance: u64,
    /// Owner-chosen display text. Never an identity.
    pub label: String,
    /// The chamber wire version the instance speaks.
    pub wire: u16,
    /// Lowercase hex SHA-256 of the zone content the instance binds, when it
    /// binds one. A client refuses an instance whose challenge differs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

/// One host the owner admitted to the directory.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostEntry {
    /// The host's x-only public key.
    pub host: String,
    /// Owner-chosen display text. Never an identity.
    pub label: String,
    /// Relays where the host publishes presence and accepts relay control.
    pub relays: Vec<String>,
    /// Owner-assigned placement weight; zero excludes the host from placement.
    pub weight: u32,
    /// When the owner added this host.
    pub added_at: u64,
    /// World instances the host serves, unique by instance number.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub worlds: Vec<WorldInstance>,
}

/// The directory body.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Directory {
    pub v: String,
    pub requires: Vec<String>,
    /// The owner key that signs and reads this directory.
    pub owner: String,
    /// Monotonic revision; the highest valid revision is current.
    pub revision: u64,
    pub issued_at: u64,
    pub hosts: Vec<HostEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

impl Directory {
    /// An empty directory at revision zero.
    #[must_use]
    pub fn empty(owner: &str, issued_at: u64) -> Self {
        Self {
            v: SCHEMA.to_owned(),
            requires: Vec::new(),
            owner: owner.to_owned(),
            revision: 0,
            issued_at,
            hosts: Vec::new(),
            meta: None,
        }
    }

    /// Check the closed schema, bounds, and uniqueness.
    ///
    /// # Errors
    /// Refuses unknown versions or features, owner-as-host entries, duplicate
    /// hosts, bad relays, labels, weights, or oversized lists.
    pub fn validate(&self) -> Result<()> {
        if self.v != SCHEMA {
            return fail(Refusal::UnsupportedVersion, "directory version");
        }
        requires(&self.requires)?;
        parse_pubkey(&self.owner)?;
        if self.hosts.len() > MAX_HOSTS {
            return fail(Refusal::LimitExceeded, "too many hosts");
        }
        for (index, entry) in self.hosts.iter().enumerate() {
            parse_pubkey(&entry.host)?;
            if entry.host == self.owner {
                return fail(Refusal::Malformed, "the owner key cannot be a host");
            }
            if self.hosts[..index].iter().any(|old| old.host == entry.host) {
                return fail(Refusal::Conflict, "duplicate host key");
            }
            label(&entry.label, MAX_LABEL_BYTES)?;
            if entry.relays.len() > MAX_RELAYS {
                return fail(Refusal::LimitExceeded, "too many relays");
            }
            for (i, relay) in entry.relays.iter().enumerate() {
                // As with relay hints, `ws` is accepted only for a loopback
                // test relay, which no other machine can reach.
                relay_url(relay, true)?;
                if entry.relays[..i].contains(relay) {
                    return fail(Refusal::Malformed, "duplicate relay");
                }
            }
            if entry.weight > MAX_WEIGHT {
                return fail(Refusal::LimitExceeded, "weight exceeds its bound");
            }
            if entry.worlds.len() > MAX_WORLDS {
                return fail(Refusal::LimitExceeded, "too many world instances");
            }
            for (i, world) in entry.worlds.iter().enumerate() {
                if world.instance == 0 {
                    return fail(Refusal::Malformed, "world instance must be nonzero");
                }
                if entry.worlds[..i]
                    .iter()
                    .any(|old| old.instance == world.instance)
                {
                    return fail(Refusal::Conflict, "duplicate world instance");
                }
                label(&world.label, MAX_LABEL_BYTES)?;
                if let Some(content) = &world.content {
                    random_id(content)?;
                }
            }
            if entry.added_at > self.issued_at {
                return fail(
                    Refusal::Malformed,
                    "host added after the directory was issued",
                );
            }
        }
        Ok(())
    }

    /// The entry for `host`, if the owner listed it.
    #[must_use]
    pub fn entry(&self, host: &str) -> Option<&HostEntry> {
        self.hosts.iter().find(|entry| entry.host == host)
    }

    /// The next revision with `entry` added. Adding is an owner action.
    ///
    /// # Errors
    /// Refuses a host already listed or an invalid result.
    pub fn with_host(&self, entry: HostEntry, issued_at: u64) -> Result<Self> {
        if self.entry(&entry.host).is_some() {
            return fail(Refusal::Conflict, "host already listed");
        }
        let mut next = self.next(issued_at)?;
        next.hosts.push(entry);
        next.validate()?;
        Ok(next)
    }

    /// The next revision without `host`. Removing is an owner action.
    ///
    /// # Errors
    /// Refuses a host that is not listed.
    pub fn without_host(&self, host: &str, issued_at: u64) -> Result<Self> {
        if self.entry(host).is_none() {
            return fail(Refusal::NotAdmitted, "host not listed");
        }
        let mut next = self.next(issued_at)?;
        next.hosts.retain(|entry| entry.host != host);
        next.validate()?;
        Ok(next)
    }

    /// The next revision with a listed host's entry replaced by `entry`, as
    /// when the owner changes its label or weight. Editing is an owner
    /// action.
    ///
    /// # Errors
    /// Refuses a host that is not listed or an invalid result.
    pub fn with_entry(&self, entry: HostEntry, issued_at: u64) -> Result<Self> {
        if self.entry(&entry.host).is_none() {
            return fail(Refusal::NotAdmitted, "host not listed");
        }
        let mut next = self.next(issued_at)?;
        for listed in &mut next.hosts {
            if listed.host == entry.host {
                listed.clone_from(&entry);
            }
        }
        next.validate()?;
        Ok(next)
    }

    /// This body republished above `conflict`, the revision at which two
    /// different directories were found. The owner chooses it to end the
    /// conflict; readers select it because its revision is higher.
    ///
    /// # Errors
    /// Refuses a revision overflow, an issue time before this body's, or an
    /// invalid result.
    pub fn superseding(&self, conflict: u64, issued_at: u64) -> Result<Self> {
        let base = Self {
            revision: conflict.max(self.revision),
            ..self.clone()
        };
        let next = base.next(issued_at)?;
        next.validate()?;
        Ok(next)
    }

    fn next(&self, issued_at: u64) -> Result<Self> {
        if issued_at < self.issued_at {
            return fail(Refusal::Malformed, "a revision cannot predate its parent");
        }
        let revision = self
            .revision
            .checked_add(1)
            .ok_or_else(|| Error::new(Refusal::LimitExceeded, "revision overflow"))?;
        Ok(Self {
            revision,
            issued_at,
            ..self.clone()
        })
    }

    /// Sign with the owner key and encrypt to the owner key.
    ///
    /// # Errors
    /// Refuses when `owner` does not match the body or the body is invalid.
    pub fn seal(&self, owner: &SecretKey, mailbox: &str, retain_until: u64) -> Result<Event> {
        self.validate()?;
        let key = pubkey(owner);
        if key != self.owner {
            return fail(
                Refusal::IdentityMismatch,
                "only the owner seals its directory",
            );
        }
        artifact::seal(
            self,
            SCHEMA,
            owner,
            &key,
            mailbox,
            self.issued_at,
            retain_until,
        )
    }

    /// Open a directory with the owner key. Only an artifact the owner signed
    /// to itself opens; a host-signed copy refuses.
    ///
    /// # Errors
    /// Refuses any other signer or recipient, a body naming another owner, or
    /// an invalid body.
    pub fn open(event: &Event, owner: &SecretKey) -> Result<Self> {
        let key = pubkey(owner);
        let (body, sealed): (Self, _) = artifact::open(event, owner, &key, &key, SCHEMA)?;
        if body.owner != key {
            return fail(Refusal::IdentityMismatch, "directory names another owner");
        }
        if body.issued_at != sealed.issued_at {
            return fail(Refusal::Malformed, "body and envelope issue times differ");
        }
        body.validate()?;
        Ok(body)
    }

    /// Pick the current directory: the highest revision. Two different bodies
    /// at the same revision are a conflict the owner must resolve.
    ///
    /// # Errors
    /// Refuses conflicting bodies at the highest revision.
    pub fn current(candidates: &[Self]) -> Result<Option<&Self>> {
        let Some(top) = candidates.iter().map(|d| d.revision).max() else {
            return Ok(None);
        };
        let mut best: Option<&Self> = None;
        for directory in candidates.iter().filter(|d| d.revision == top) {
            match best {
                Some(existing) if existing != directory => {
                    return fail(Refusal::Conflict, "two directories share a revision");
                }
                _ => best = Some(directory),
            }
        }
        Ok(best)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::new_id;

    fn key(n: u8) -> SecretKey {
        SecretKey::from_byte_array([n; 32]).unwrap()
    }
    fn entry(n: u8) -> HostEntry {
        HostEntry {
            host: pubkey(&key(n)),
            label: format!("host {n}"),
            relays: vec!["wss://relay.example".into()],
            weight: 10,
            added_at: 100,
            worlds: Vec::new(),
        }
    }

    fn world(instance: u64) -> WorldInstance {
        WorldInstance {
            instance,
            label: "Everglade".into(),
            wire: 22,
            content: Some("ab".repeat(32)),
        }
    }

    #[test]
    fn an_entry_advertises_world_instances() {
        let owner = key(1);
        let dir = Directory::empty(&pubkey(&owner), 100)
            .with_host(
                HostEntry {
                    worlds: vec![world(7)],
                    ..entry(2)
                },
                110,
            )
            .unwrap();
        let event = dir.seal(&owner, &new_id(), 10_000).unwrap();
        let opened = Directory::open(&event, &owner).unwrap();
        assert_eq!(opened.entry(&entry(2).host).unwrap().worlds, vec![world(7)]);
        // An entry without worlds keeps the original wire form.
        let plain = serde_json::to_value(entry(3)).unwrap();
        assert!(plain.get("worlds").is_none());
        let parsed: HostEntry = serde_json::from_value(plain).unwrap();
        assert!(parsed.worlds.is_empty());
        for bad in [
            vec![world(0)],
            vec![world(7), world(7)],
            vec![WorldInstance {
                label: String::new(),
                ..world(7)
            }],
            vec![WorldInstance {
                content: Some("AB".repeat(32)),
                ..world(7)
            }],
            (1..=MAX_WORLDS as u64 + 1).map(world).collect(),
        ] {
            let mut dir = Directory::empty(&pubkey(&owner), 100);
            dir.hosts.push(HostEntry {
                worlds: bad,
                ..entry(2)
            });
            assert!(dir.validate().is_err());
        }
    }

    #[test]
    fn directory_round_trip() {
        let owner = key(1);
        let dir = Directory::empty(&pubkey(&owner), 100)
            .with_host(entry(2), 110)
            .unwrap()
            .with_host(entry(3), 120)
            .unwrap();
        assert_eq!(dir.revision, 2);
        let mailbox = new_id();
        let event = dir.seal(&owner, &mailbox, 10_000).unwrap();
        let opened = Directory::open(&event, &owner).unwrap();
        assert_eq!(opened, dir);
        assert!(opened.entry(&pubkey(&key(2))).is_some());
        // Nobody else can read it, including a listed host.
        assert!(Directory::open(&event, &key(2)).is_err());
        let removed = opened.without_host(&pubkey(&key(2)), 130).unwrap();
        assert_eq!(removed.revision, 3);
        assert!(removed.entry(&pubkey(&key(2))).is_none());
        let all = vec![dir.clone(), removed.clone(), opened];
        assert_eq!(Directory::current(&all).unwrap(), Some(&removed));
    }

    #[test]
    fn host_published_directory_is_refused() {
        let owner = key(1);
        let host = key(2);
        // A host writes a body that names the owner and lists itself.
        let mut forged = Directory::empty(&pubkey(&owner), 100);
        forged.revision = 9;
        forged.hosts.push(entry(2));
        let event = artifact::seal(
            &forged,
            SCHEMA,
            &host,
            &pubkey(&owner),
            &new_id(),
            100,
            1000,
        )
        .unwrap();
        let err = Directory::open(&event, &owner).unwrap_err();
        assert_eq!(err.code, Refusal::IdentityMismatch);
        // The public seal helper refuses a non-owner key outright.
        assert_eq!(
            forged.seal(&host, &new_id(), 1000).unwrap_err().code,
            Refusal::IdentityMismatch
        );
    }

    #[test]
    fn invalid_directories_refuse() {
        let owner = pubkey(&key(1));
        let mut dir = Directory::empty(&owner, 100);
        dir.hosts.push(entry(2));
        dir.hosts.push(entry(2));
        assert_eq!(dir.validate().unwrap_err().code, Refusal::Conflict);
        let mut dir = Directory::empty(&owner, 100);
        dir.hosts.push(HostEntry {
            host: owner.clone(),
            ..entry(2)
        });
        assert!(dir.validate().is_err());
        let mut dir = Directory::empty(&owner, 100);
        dir.hosts.push(HostEntry {
            relays: vec!["wss://user:pw@relay.example".into()],
            ..entry(2)
        });
        assert!(dir.validate().is_err());
        // Plain `ws` is refused except for a loopback test relay.
        let mut dir = Directory::empty(&owner, 100);
        dir.hosts.push(HostEntry {
            relays: vec!["ws://relay.example".into()],
            ..entry(2)
        });
        assert!(dir.validate().is_err());
        let mut dir = Directory::empty(&owner, 100);
        dir.hosts.push(HostEntry {
            relays: vec!["ws://127.0.0.1:7000".into()],
            ..entry(2)
        });
        assert!(dir.validate().is_ok());
        let mut dir = Directory::empty(&owner, 100);
        dir.requires.push("oa-future".into());
        assert_eq!(
            dir.validate().unwrap_err().code,
            Refusal::UnsupportedFeature
        );
        let mut dir = Directory::empty(&owner, 100);
        dir.v = "openagents.host-directory.v2".into();
        assert_eq!(
            dir.validate().unwrap_err().code,
            Refusal::UnsupportedVersion
        );
    }

    #[test]
    fn conflicting_revisions_refuse() {
        let owner = pubkey(&key(1));
        let a = Directory::empty(&owner, 100)
            .with_host(entry(2), 110)
            .unwrap();
        let b = Directory::empty(&owner, 100)
            .with_host(entry(3), 110)
            .unwrap();
        assert_eq!(
            Directory::current(&[a.clone(), b]).unwrap_err().code,
            Refusal::Conflict
        );
        assert_eq!(
            Directory::current(&[a.clone(), a.clone()]).unwrap(),
            Some(&a)
        );
        assert_eq!(Directory::current(&[]).unwrap(), None);
    }

    #[test]
    fn edits_and_conflict_resolution_produce_the_next_revision() {
        let owner = pubkey(&key(1));
        let first = Directory::empty(&owner, 100)
            .with_host(entry(2), 110)
            .unwrap();
        let edited = first
            .with_entry(
                HostEntry {
                    label: "renamed".into(),
                    weight: 0,
                    ..entry(2)
                },
                120,
            )
            .unwrap();
        assert_eq!(edited.revision, first.revision + 1);
        assert_eq!(edited.hosts.len(), 1);
        assert_eq!(edited.entry(&entry(2).host).unwrap().label, "renamed");
        assert_eq!(edited.entry(&entry(2).host).unwrap().weight, 0);
        // Only a listed host can be edited, and the result stays valid.
        assert_eq!(
            first.with_entry(entry(3), 120).unwrap_err().code,
            Refusal::NotAdmitted
        );
        assert!(
            first
                .with_entry(
                    HostEntry {
                        weight: MAX_WEIGHT + 1,
                        ..entry(2)
                    },
                    120
                )
                .is_err()
        );
        assert!(first.with_entry(entry(2), 90).is_err());
        let removed = edited.without_host(&entry(2).host, 130).unwrap();
        assert_eq!(removed.revision, edited.revision + 1);
        assert!(removed.hosts.is_empty());

        // Two bodies at revision 2; the owner keeps one above both.
        let other = first.with_host(entry(3), 120).unwrap();
        assert_eq!(
            Directory::current(&[edited.clone(), other.clone()])
                .unwrap_err()
                .code,
            Refusal::Conflict
        );
        let kept = edited.superseding(2, 140).unwrap();
        assert_eq!(kept.revision, 3);
        assert_eq!(kept.hosts, edited.hosts);
        assert_eq!(
            Directory::current(&[edited, other, kept.clone()]).unwrap(),
            Some(&kept)
        );
        // A held body older than the conflict still lands above it.
        assert_eq!(first.superseding(5, 140).unwrap().revision, 6);
    }
}
