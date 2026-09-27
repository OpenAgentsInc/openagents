//! The owner host directory: an owner-signed, owner-encrypted list of hosts.
//!
//! Only the owner key signs and reads it. A host cannot add itself: an
//! artifact signed by any other key refuses, whatever its body says.

use nostr::domain::Event;
use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    Error, Refusal, Result, artifact, fail, label, parse_pubkey, pubkey, relay_url, requires,
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
                relay_url(relay, false)?;
                if entry.relays[..i].contains(relay) {
                    return fail(Refusal::Malformed, "duplicate relay");
                }
            }
            if entry.weight > MAX_WEIGHT {
                return fail(Refusal::LimitExceeded, "weight exceeds its bound");
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
}
