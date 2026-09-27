//! Owner edits to the NIP-REACH host directory.
//!
//! Every edit follows the owner-authority rule: it needs the owner key, a
//! successful read of the retained revisions with no conflict, and it reuses
//! the mailbox of the revision it read (a fresh one when none exists). An
//! edit that changes nothing publishes nothing, so re-running `coder link`
//! does not add revisions.

use coder_access::RelayPolicy;
use coder_host::client::{fetch_directory_revisions, publish_directory};
use coder_reach::directory::{Directory, HostEntry};
use secp256k1::SecretKey;

use crate::{Error, Result};

/// How long relays keep a directory revision.
const RETENTION: u64 = 365 * 86_400;
/// The placement weight a newly listed host gets.
const WEIGHT: u32 = 100;

/// The current directory and the mailbox its revision used.
#[derive(Clone, Debug, PartialEq)]
pub struct Current {
    pub directory: Directory,
    pub mailbox: String,
}

/// Read every retained revision from `relays` and select the current one.
/// `None` when no revision exists yet.
///
/// # Errors
/// Reports a relay no read succeeded on, and a conflict: two different
/// directories at the highest revision.
pub async fn read(
    relays: &[String],
    owner: &SecretKey,
    policy: RelayPolicy,
) -> Result<Option<Current>> {
    let mut candidates: Vec<(Directory, String)> = Vec::new();
    let mut read_any = false;
    for relay in relays {
        match fetch_directory_revisions(relay, owner, policy).await {
            Ok(found) => {
                read_any = true;
                candidates.extend(found);
            }
            Err(error) => eprintln!("coder link: cannot read the directory from {relay}: {error}"),
        }
    }
    if !read_any {
        return Err(Error::new("no relay answered the directory read"));
    }
    let owner_hex = coder_reach::pubkey(owner);
    candidates.retain(|(directory, _)| directory.owner == owner_hex);
    let bodies: Vec<Directory> = candidates.iter().map(|(d, _)| d.clone()).collect();
    let current = Directory::current(&bodies).map_err(|error| {
        Error::new(format!(
            "the directory has conflicting revisions ({error}); settle it from a Computers screen"
        ))
    })?;
    Ok(current.and_then(|current| {
        candidates
            .into_iter()
            .find(|(directory, _)| directory == current)
            .map(|(directory, mailbox)| Current { directory, mailbox })
    }))
}

/// The next revision that lists `host` under `label` with `relays`, or
/// `None` when the directory already says exactly that. A listed host keeps
/// its weight and the time it was added.
///
/// # Errors
/// Refuses an invalid label or result.
pub fn with_host(
    current: Option<&Directory>,
    owner: &str,
    host: &str,
    label: &str,
    relays: &[String],
    now: u64,
) -> Result<Option<Directory>> {
    let empty = Directory::empty(owner, now);
    let base = current.unwrap_or(&empty);
    let issued_at = now.max(base.issued_at);
    let next = match base.entry(host) {
        Some(listed) if listed.label == label && listed.relays == relays => return Ok(None),
        Some(listed) => base.with_entry(
            HostEntry {
                label: label.to_owned(),
                relays: relays.to_vec(),
                ..listed.clone()
            },
            issued_at,
        ),
        None => base.with_host(
            HostEntry {
                host: host.to_owned(),
                label: label.to_owned(),
                relays: relays.to_vec(),
                weight: WEIGHT,
                added_at: now,
            },
            issued_at,
        ),
    };
    next.map(Some)
        .map_err(|error| Error::new(format!("cannot list the host: {error}")))
}

/// The next revision without `host`, or `None` when it is not listed.
///
/// # Errors
/// Refuses an invalid result.
pub fn without_host(
    current: Option<&Directory>,
    host: &str,
    now: u64,
) -> Result<Option<Directory>> {
    let Some(base) = current.filter(|d| d.entry(host).is_some()) else {
        return Ok(None);
    };
    base.without_host(host, now.max(base.issued_at))
        .map(Some)
        .map_err(|error| Error::new(format!("cannot remove the host: {error}")))
}

/// Publish `next` under the read revision's mailbox, or a fresh one, to
/// every relay. One relay accepting is enough.
///
/// # Errors
/// Reports that no relay accepted it.
pub async fn publish(
    relays: &[String],
    owner: &SecretKey,
    current: Option<&Current>,
    next: &Directory,
    policy: RelayPolicy,
) -> Result<()> {
    let mailbox = current.map_or_else(coder_reach::new_id, |c| c.mailbox.clone());
    let retain_until = next.issued_at.saturating_add(RETENTION);
    let mut accepted = false;
    for relay in relays {
        match publish_directory(relay, owner, next, &mailbox, retain_until, policy).await {
            Ok(()) => accepted = true,
            Err(error) => eprintln!("coder link: {relay} refused the directory: {error}"),
        }
    }
    if accepted {
        Ok(())
    } else {
        Err(Error::new("no relay accepted the directory"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> String {
        coder_reach::pubkey(&SecretKey::new(&mut secp256k1::rand::rng()))
    }

    #[test]
    fn listing_is_idempotent_and_keeps_weight_and_added_at() {
        let owner = key();
        let host = key();
        let relays = vec!["wss://relay.example/".to_owned()];
        let first = with_host(None, &owner, &host, "box", &relays, 100)
            .unwrap()
            .unwrap();
        assert_eq!(first.revision, 1);
        assert_eq!(first.entry(&host).unwrap().weight, WEIGHT);
        // The same listing again publishes nothing.
        assert_eq!(
            with_host(Some(&first), &owner, &host, "box", &relays, 200).unwrap(),
            None
        );
        // A new label is the next revision; weight and added_at stay.
        let mut reweighed = first.clone();
        reweighed.hosts[0].weight = 7;
        let renamed = with_host(Some(&reweighed), &owner, &host, "big box", &relays, 50)
            .unwrap()
            .unwrap();
        assert_eq!(renamed.revision, 2);
        assert_eq!(renamed.issued_at, 100, "never before the parent");
        let entry = renamed.entry(&host).unwrap();
        assert_eq!(
            (entry.label.as_str(), entry.weight, entry.added_at),
            ("big box", 7, 100)
        );
        // A second host joins the same directory.
        let other = key();
        let two = with_host(Some(&renamed), &owner, &other, "other", &relays, 300)
            .unwrap()
            .unwrap();
        assert_eq!(two.hosts.len(), 2);
        let removed = without_host(Some(&two), &other, 400).unwrap().unwrap();
        assert_eq!(removed.hosts.len(), 1);
        assert_eq!(without_host(Some(&removed), &other, 500).unwrap(), None);
    }

    #[test]
    fn a_bad_label_refuses() {
        let relays = vec!["wss://relay.example/".to_owned()];
        assert!(with_host(None, &key(), &key(), &"x".repeat(65), &relays, 1).is_err());
    }
}
