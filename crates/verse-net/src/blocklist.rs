//! The players this person blocked or muted, kept across sessions.
//!
//! A blocked player disappears: the session drops every event they sign, so
//! their avatar, chat, and gestures never reach the crowd or the log. A
//! muted player still walks in the world, but their chat, private messages,
//! and gesture lines are hidden. Both lists are local; the relay and the
//! other player learn nothing.
//!
//! The lists live in one small JSON file, [`FILE`], in a directory the
//! platform chooses: `~/.openagents/verse` on a computer (shared by the
//! Verse app, the desktop Grid, and `openagents verse block`), and the zone
//! cache directory on a phone.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The file name in the chosen directory.
pub const FILE: &str = "blocked.json";
/// Most keys each list holds.
pub const MAX: usize = 1_000;

/// Blocked and muted players, by hex public key.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Blocklist {
    /// Players whose events are dropped.
    #[serde(default)]
    pub blocked: BTreeSet<String>,
    /// Players whose avatars show but whose words don't.
    #[serde(default)]
    pub muted: BTreeSet<String>,
}

impl Blocklist {
    /// Where the lists live in `dir`.
    #[must_use]
    pub fn path(dir: &Path) -> PathBuf {
        dir.join(FILE)
    }

    /// Reads the lists from `dir`. A missing file is two empty lists, and
    /// an entry that isn't a hex public key is dropped.
    ///
    /// # Errors
    ///
    /// Returns a message when the file exists but can't be read or parsed.
    pub fn load(dir: &Path) -> Result<Self, String> {
        let path = Self::path(dir);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(error) => return Err(format!("couldn't read {}: {error}", path.display())),
        };
        let mut list: Self = serde_json::from_str(&text)
            .map_err(|error| format!("couldn't parse {}: {error}", path.display()))?;
        list.blocked.retain(|key| is_pubkey(key));
        list.muted.retain(|key| is_pubkey(key));
        while list.blocked.len() > MAX {
            list.blocked.pop_last();
        }
        while list.muted.len() > MAX {
            list.muted.pop_last();
        }
        Ok(list)
    }

    /// Writes the lists to `dir`, replacing the file in one rename.
    ///
    /// # Errors
    ///
    /// Returns a message when the directory or file can't be written.
    pub fn save(&self, dir: &Path) -> Result<(), String> {
        std::fs::create_dir_all(dir)
            .map_err(|error| format!("couldn't create {}: {error}", dir.display()))?;
        let path = Self::path(dir);
        let temporary = dir.join(format!(".{FILE}.tmp"));
        let text = serde_json::to_string_pretty(self).map_err(|error| error.to_string())?;
        std::fs::write(&temporary, text)
            .map_err(|error| format!("couldn't write {}: {error}", temporary.display()))?;
        std::fs::rename(&temporary, &path)
            .map_err(|error| format!("couldn't replace {}: {error}", path.display()))
    }

    /// Whether `pubkey` is blocked.
    #[must_use]
    pub fn is_blocked(&self, pubkey: &str) -> bool {
        self.blocked.contains(pubkey)
    }

    /// Whether `pubkey`'s words are hidden: muted or blocked.
    #[must_use]
    pub fn is_muted(&self, pubkey: &str) -> bool {
        self.muted.contains(pubkey) || self.blocked.contains(pubkey)
    }

    /// Blocks `pubkey`. Returns whether the list changed.
    ///
    /// # Errors
    ///
    /// Returns a message when `pubkey` isn't a hex public key or the list
    /// is full.
    pub fn block(&mut self, pubkey: &str) -> Result<bool, String> {
        insert(&mut self.blocked, pubkey)
    }

    /// Unblocks `pubkey`. Returns whether the list changed.
    pub fn unblock(&mut self, pubkey: &str) -> bool {
        self.blocked.remove(pubkey)
    }

    /// Mutes `pubkey`. Returns whether the list changed.
    ///
    /// # Errors
    ///
    /// Returns a message when `pubkey` isn't a hex public key or the list
    /// is full.
    pub fn mute(&mut self, pubkey: &str) -> Result<bool, String> {
        insert(&mut self.muted, pubkey)
    }

    /// Unmutes `pubkey`. Returns whether the list changed.
    pub fn unmute(&mut self, pubkey: &str) -> bool {
        self.muted.remove(pubkey)
    }
}

fn insert(list: &mut BTreeSet<String>, pubkey: &str) -> Result<bool, String> {
    if !is_pubkey(pubkey) {
        return Err("expected a 64-character hex public key".into());
    }
    if list.contains(pubkey) {
        return Ok(false);
    }
    if list.len() >= MAX {
        return Err(format!("the list already holds {MAX} players"));
    }
    Ok(list.insert(pubkey.to_owned()))
}

/// Whether `key` is a lowercase 64-character hex public key.
#[must_use]
pub fn is_pubkey(key: &str) -> bool {
    key.len() == 64
        && key
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::Blocklist;

    #[test]
    fn lists_survive_a_save_and_load_and_refuse_a_bad_key() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(Blocklist::load(dir.path()).unwrap(), Blocklist::default());
        let mut list = Blocklist::default();
        let walker = "a".repeat(64);
        let talker = "b".repeat(64);
        assert!(list.block(&walker).unwrap());
        assert!(!list.block(&walker).unwrap());
        assert!(list.mute(&talker).unwrap());
        assert!(list.block("walker").is_err());
        list.save(dir.path()).unwrap();
        let again = Blocklist::load(dir.path()).unwrap();
        assert_eq!(again, list);
        assert!(again.is_blocked(&walker));
        assert!(again.is_muted(&walker), "a blocked player is also unheard");
        assert!(again.is_muted(&talker));
        assert!(!again.is_blocked(&talker));
    }

    #[test]
    fn a_hand_edited_file_keeps_only_public_keys() {
        let dir = tempfile::tempdir().unwrap();
        let walker = "c".repeat(64);
        std::fs::write(
            Blocklist::path(dir.path()),
            format!(r#"{{"blocked": ["{walker}", "NOT-A-KEY"]}}"#),
        )
        .unwrap();
        let list = Blocklist::load(dir.path()).unwrap();
        assert_eq!(list.blocked.len(), 1);
        assert!(list.is_blocked(&walker));
        assert!(list.muted.is_empty());
    }
}
