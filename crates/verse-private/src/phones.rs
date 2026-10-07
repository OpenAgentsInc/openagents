//! The phones that asked for the owner's placements: `private-phones.json`
//! in Verse's home on the owner's computer.
//!
//! A phone paired with the owner's host asks for the placements over
//! NIP-HOST (`verse.private`) and names its Verse world key. The host
//! records the key here, so the owner can grant it with `verse-private
//! grant NAME KEY`, and `verse-private phones` lists what to grant. A
//! record grants nothing: only a manifest's `readers` list does.

use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::valid_digest;

/// The record's schema.
pub const SCHEMA: &str = "openagents.verse.private-phones.v1";
/// The record's name in Verse's home.
pub const FILE: &str = "private-phones.json";
/// Most phones kept; the oldest seen goes first.
pub const MAX_PHONES: usize = 32;
const MAX_FILE_BYTES: u64 = 64 * 1024;

/// The whole record.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Phones {
    pub schema: String,
    pub phones: Vec<Phone>,
}

/// One phone's ask.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Phone {
    /// The phone's NIP-HOST device key, hex.
    pub device: String,
    /// The phone's Verse world key, hex: the key to grant.
    pub world_key: String,
    /// When it first asked, Unix seconds.
    pub first_seen: u64,
    /// When it last asked, Unix seconds.
    pub last_seen: u64,
}

/// The record in `home`.
#[must_use]
pub fn path(home: &Path) -> PathBuf {
    home.join(FILE)
}

/// Reads the record in `home`; an empty one when there is none, or when it
/// can't be read or parsed, since it only remembers asks.
#[must_use]
pub fn load(home: &Path) -> Phones {
    let path = path(home);
    let read = || -> Option<Phones> {
        let metadata = std::fs::symlink_metadata(&path).ok()?;
        if !metadata.file_type().is_file() || metadata.len() > MAX_FILE_BYTES {
            return None;
        }
        let mut bytes = Vec::new();
        std::fs::File::open(&path)
            .ok()?
            .take(MAX_FILE_BYTES)
            .read_to_end(&mut bytes)
            .ok()?;
        let phones: Phones = serde_json::from_slice(&bytes).ok()?;
        (phones.schema == SCHEMA).then_some(phones)
    };
    read().unwrap_or_else(|| Phones {
        schema: SCHEMA.into(),
        phones: Vec::new(),
    })
}

/// Notes that `device` asked with `world_key` at `now`. Returns `true` when
/// this world key is new to the record, so the caller can offer the grant
/// once.
///
/// # Errors
///
/// Returns a message when a key is not 64 hexadecimal digits or the record
/// can't be written.
pub fn note(home: &Path, device: &str, world_key: &str, now: u64) -> Result<bool, String> {
    if !valid_digest(device) || !valid_digest(world_key) {
        return Err("a phone's keys are 64 lowercase hexadecimal digits".into());
    }
    let mut record = load(home);
    let new = !record.phones.iter().any(|p| p.world_key == world_key);
    match record
        .phones
        .iter_mut()
        .find(|p| p.device == device && p.world_key == world_key)
    {
        // Unchanged within a minute: no write, so a phone that asks on
        // every poll doesn't rewrite the file.
        Some(phone) if now.saturating_sub(phone.last_seen) < 60 => return Ok(new),
        Some(phone) => phone.last_seen = now,
        None => record.phones.push(Phone {
            device: device.into(),
            world_key: world_key.into(),
            first_seen: now,
            last_seen: now,
        }),
    }
    record
        .phones
        .sort_by_key(|p| std::cmp::Reverse(p.last_seen));
    record.phones.truncate(MAX_PHONES);
    let mut bytes = serde_json::to_vec_pretty(&record).map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    crate::placements::write_private(home, FILE, &bytes)?;
    Ok(new)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_phone_is_noted_once_and_refreshed_privately() {
        let home = tempfile::tempdir().unwrap();
        let (device, world) = ("aa".repeat(32), "bb".repeat(32));
        assert!(load(home.path()).phones.is_empty());
        assert!(note(home.path(), &device, &world, 1_000).unwrap());
        assert!(!note(home.path(), &device, &world, 1_010).unwrap());
        assert_eq!(load(home.path()).phones[0].last_seen, 1_000);
        assert!(!note(home.path(), &device, &world, 2_000).unwrap());
        let phones = load(home.path()).phones;
        assert_eq!(phones.len(), 1);
        assert_eq!((phones[0].first_seen, phones[0].last_seen), (1_000, 2_000));
        // A second world key from the same phone is new.
        assert!(note(home.path(), &device, &"cc".repeat(32), 2_100).unwrap());
        assert_eq!(load(home.path()).phones[0].world_key, "cc".repeat(32));
        assert!(note(home.path(), &device, "not-a-key", 2_200).is_err());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(path(home.path()))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
    }

    #[test]
    fn the_record_keeps_the_most_recent_phones() {
        let home = tempfile::tempdir().unwrap();
        for i in 0..(MAX_PHONES as u64 + 4) {
            let world = format!("{i:064x}");
            note(home.path(), &"aa".repeat(32), &world, 100 * (i + 1)).unwrap();
        }
        let phones = load(home.path()).phones;
        assert_eq!(phones.len(), MAX_PHONES);
        assert!(phones.iter().all(|p| p.world_key != format!("{:064x}", 0)));
    }
}
