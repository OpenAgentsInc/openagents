//! Owner edits to the host directory: add, relabel, reweigh, and remove a
//! host, and settle a conflict. Each publishes the next revision.
//!
//! Every edit follows the NIP-REACH owner-authority rule: it needs the owner
//! key held on this device and a successful read with no conflict, and it
//! reuses the mailbox of the revision it read. An edit names the revision
//! the screen showed, and the service refuses it as stale when the revision
//! it holds differs. Settling a conflict is the one edit made during a
//! conflict: it republishes the version this device last trusted above the
//! conflicting revision, and never merges the two.
use super::{DIRECTORY_RETENTION, Handle, Result, Saved, SavedOwner, Shared, lock, owner_key};
use crate::model::{DirectoryState, LOCAL_WEIGHT, ListingChange};
use coder_access::{Code, Error};
use coder_host::client::publish_directory;
use coder_reach::directory::{Directory, HostEntry};

/// One owner edit.
pub(super) enum Edit<'a> {
    /// List an enrolled host under `label` with weight 100.
    Add { host: &'a str, label: &'a str },
    /// Change a listed host's label or weight.
    Change {
        host: &'a str,
        revision: u64,
        change: &'a ListingChange,
    },
    /// Remove a listed host.
    Remove { host: &'a str, revision: u64 },
    /// Republish this device's version above a conflict at `revision`.
    Keep { revision: u64 },
}

/// Build the next revision for `edit`, publish it to the directory's relays,
/// and trust it once one relay accepts it.
pub(super) fn publish(shared: &Shared, runtime: &Handle, edit: &Edit<'_>) -> Result<()> {
    let now = (shared.settings.now)();
    let (owner, held, next, mailbox, relays) = {
        let state = lock(&shared.state);
        let owner = owner_key(&state.saved, &shared.secret)
            .ok_or_else(|| Error::new(Code::Forbidden, "this device holds no owner key"))?;
        let owner_hex = coder_reach::pubkey(&owner);
        let record = state.saved.owner.as_ref();
        let held = record.and_then(|record| record.directory.clone());
        match (edit, state.directory.state) {
            (Edit::Keep { revision }, DirectoryState::Conflict { revision: shown })
                if shown == *revision => {}
            (Edit::Keep { .. }, _) => {
                return Err(Error::new(
                    Code::Stale,
                    "the directory has no such conflict",
                ));
            }
            (_, state) if !state.writable() => {
                return Err(Error::new(Code::Stale, "the directory was not read"));
            }
            (Edit::Change { revision, .. } | Edit::Remove { revision, .. }, _)
                if held.as_ref().map(|directory| directory.revision) != Some(*revision) =>
            {
                return Err(Error::new(
                    Code::Stale,
                    "the directory changed since the screen was drawn",
                ));
            }
            _ => {}
        }
        let mut relays = shared.directory_relays(&state.saved, &owner);
        let current = match (edit, &held) {
            (_, Some(held)) => held.clone(),
            (Edit::Keep { .. }, None) => {
                return Err(Error::new(
                    Code::Unavailable,
                    "this device holds no version of the directory to keep",
                ));
            }
            (_, None) => Directory::empty(&owner_hex, now),
        };
        let issued_at = now.max(current.issued_at);
        let next = match edit {
            Edit::Add { host, label } => {
                let saved = state
                    .saved
                    .hosts
                    .iter()
                    .find(|saved| saved.access.grant.host == *host)
                    .ok_or_else(|| Error::new(Code::Stale, "unknown computer"))?;
                if saved.access.grant.owner != owner_hex {
                    return Err(Error::new(
                        Code::Forbidden,
                        "the computer names another owner",
                    ));
                }
                let relay = saved.access.grant.relay.clone();
                if !relays.contains(&relay) {
                    relays.push(relay.clone());
                }
                current.with_host(
                    HostEntry {
                        host: (*host).to_owned(),
                        label: (*label).to_owned(),
                        relays: vec![relay],
                        weight: LOCAL_WEIGHT,
                        added_at: now,
                    },
                    issued_at,
                )
            }
            Edit::Change { host, change, .. } => {
                let mut entry = current
                    .entry(host)
                    .cloned()
                    .ok_or_else(|| Error::new(Code::Stale, "the directory doesn't list it"))?;
                match change {
                    ListingChange::Label(label) => label.clone_into(&mut entry.label),
                    ListingChange::Weight(weight) => entry.weight = *weight,
                }
                current.with_entry(entry, issued_at)
            }
            Edit::Remove { host, .. } => current.without_host(host, issued_at),
            Edit::Keep { revision } => current.superseding(*revision, issued_at),
        }
        .map_err(|error| Error::new(Code::Malformed, error.to_string()))?;
        let mailbox = record
            .and_then(|record| record.mailbox.clone())
            .unwrap_or_else(coder_reach::new_id);
        (owner, held, next, mailbox, relays)
    };
    let retain_until = next.issued_at.saturating_add(DIRECTORY_RETENTION);
    // Publish to every directory relay; one that accepts is enough.
    let mut published = false;
    for relay in &relays {
        published |= runtime
            .block_on(publish_directory(
                relay,
                &owner,
                &next,
                &mailbox,
                retain_until,
                shared.settings.policy,
            ))
            .is_ok();
    }
    if !published {
        return Err(Error::new(
            Code::Transport,
            "no relay accepted the directory",
        ));
    }
    let saved = {
        let mut state = lock(&shared.state);
        // A read that finished during the publish may hold a higher revision;
        // never replace it with a lower one.
        // A read that already found this revision changes nothing.
        let trusted = state
            .saved
            .owner
            .as_ref()
            .and_then(|record| record.directory.as_ref());
        if trusted.is_some_and(|trusted| *trusted != next && trusted.revision >= next.revision) {
            return Err(Error::new(
                Code::Stale,
                "the directory changed while this edit was published",
            ));
        }
        mark_delisted(&mut state.saved, held.as_ref(), &next);
        let relabel = match edit {
            Edit::Add { host, label } => Some((*host, *label)),
            Edit::Change {
                host,
                change: ListingChange::Label(label),
                ..
            } => Some((*host, label.as_str())),
            _ => None,
        };
        if let Some((host, label)) = relabel
            && let Some(saved) = state
                .saved
                .hosts
                .iter_mut()
                .find(|saved| saved.access.grant.host == host)
        {
            label.clone_into(&mut saved.label);
        }
        state.directory.state = DirectoryState::Current {
            revision: Some(next.revision),
            as_of: now,
        };
        let record = state.saved.owner.get_or_insert_with(SavedOwner::default);
        record.mailbox = Some(mailbox);
        record.directory = Some(next);
        state.saved.clone()
    };
    shared.save(&saved)
}

/// Record which held hosts the owner removed. A host the new revision lists
/// is not removed; a host the previous revision listed and the new one does
/// not is removed: it stays reachable with its grant and leaves placement.
pub(super) fn mark_delisted(saved: &mut Saved, previous: Option<&Directory>, next: &Directory) {
    for host in &mut saved.hosts {
        let key = &host.access.grant.host;
        if next.entry(key).is_some() {
            host.delisted = false;
        } else if previous.is_some_and(|previous| previous.entry(key).is_some()) {
            host.delisted = true;
        }
    }
}
