//! Who may send zone commands (NIP-MV kind 23302) to this desktop operator.
//!
//! Authority is explicit and local: the operator's own key, then the hex
//! pubkeys listed one per line in `<VERSE_HOME>/zone-operators`, then the
//! comma-separated `VERSE_ZONE_OPERATORS` environment variable. An empty
//! list admits only the operator itself. Nothing on the wire can widen it.

use std::collections::BTreeSet;
use std::path::Path;

/// The file under the Verse home that lists trusted operator pubkeys.
pub const OPERATORS_FILE: &str = "zone-operators";

/// The set of pubkeys allowed to command this operator's zone.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Operators {
    keys: BTreeSet<String>,
    /// Lines that were not 64 lowercase hex characters, for the log.
    pub rejected: Vec<String>,
}

impl Operators {
    /// The operator itself plus the keys from `dir/zone-operators` and
    /// `VERSE_ZONE_OPERATORS`.
    #[must_use]
    pub fn load(dir: &Path, me: &str) -> Self {
        let mut list = Self::default();
        list.keys.insert(me.to_owned());
        if let Ok(text) = std::fs::read_to_string(dir.join(OPERATORS_FILE)) {
            list.extend(text.lines());
        }
        if let Ok(env) = std::env::var("VERSE_ZONE_OPERATORS") {
            list.extend(env.split(','));
        }
        list
    }

    /// Adds every well-formed key in `lines`; blank lines and `#` comments
    /// are skipped, anything else malformed lands in `rejected`.
    pub fn extend<'a>(&mut self, lines: impl IntoIterator<Item = &'a str>) {
        for line in lines {
            let key = line.trim();
            if key.is_empty() || key.starts_with('#') {
                continue;
            }
            if key.len() == 64 && key.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
                self.keys.insert(key.to_owned());
            } else {
                self.rejected.push(key.chars().take(80).collect());
            }
        }
    }

    /// Whether `pubkey` may command the zone.
    #[must_use]
    pub fn allows(&self, pubkey: &str) -> bool {
        self.keys.contains(pubkey)
    }

    /// How many keys are admitted, including the operator itself.
    #[must_use]
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ME: &str = "aa00aa00aa00aa00aa00aa00aa00aa00aa00aa00aa00aa00aa00aa00aa00aa00";
    const OTHER: &str = "bb00bb00bb00bb00bb00bb00bb00bb00bb00bb00bb00bb00bb00bb00bb00bb00";

    #[test]
    fn admits_only_self_by_default() {
        let mut list = Operators::default();
        list.extend([ME]);
        assert!(list.allows(ME));
        assert!(!list.allows(OTHER));
    }

    #[test]
    fn extends_from_lines_and_rejects_malformed() {
        let mut list = Operators::default();
        list.extend([
            "# comment",
            "",
            &format!("  {OTHER}  "),
            "npub1notallowed",
            "ABCD",
        ]);
        assert!(list.allows(OTHER));
        assert_eq!(list.rejected, vec!["npub1notallowed", "ABCD"]);
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn loads_the_file_beside_the_keys() {
        let dir = std::env::temp_dir().join(format!("verse-operators-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(OPERATORS_FILE), format!("{OTHER}\n")).unwrap();
        let list = Operators::load(&dir, ME);
        assert!(list.allows(ME));
        assert!(list.allows(OTHER));
        std::fs::remove_dir_all(&dir).ok();
    }
}
