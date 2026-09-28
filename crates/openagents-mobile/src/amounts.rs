//! How the app shows bitcoin amounts, app-wide: BIP 177 integer base units
//! (`₿12,345`) by default, or legacy BTC (`0.00012345 BTC`) when the person
//! switches. The choice and whether the person read the transitional note
//! are plain files under the app's state directory; neither is a secret.
//! Every amount the app shows or reads goes through [`Format`], and every
//! stored or internal amount stays a `u64` of base units.

use serde::Serialize;
use std::path::{Path, PathBuf};

pub use bitcoin_amount::Format;

const FORMAT_FILE: &str = "amount-format";
const NOTE_FILE: &str = "amount-note-acknowledged";

/// The transitional note, shown in the Wallet until acknowledged.
pub const NOTE_TITLE: &str = "Bitcoin amounts are whole numbers now";
pub const NOTE_LINES: [&str; 3] = [
    "Following BIP 177, amounts show in bitcoin's base unit: ₿10,000 is the same as 0.00010000 BTC. Nothing about your balance changed.",
    "The smallest unit, once called a satoshi, is now one bitcoin (₿1); 100,000,000 of them make 1 BTC.",
    "Prefer decimals? Choose BTC under Show amounts as. You can switch back at any time.",
];

pub struct Amounts {
    dir: PathBuf,
    format: Format,
    note_acknowledged: bool,
}

/// What the host needs to show and collect amounts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AmountsView {
    /// `bip177` or `btc`; send one back with `amount_format`.
    pub format: &'static str,
    /// The unit amount fields name: "₿" or "BTC".
    pub unit: &'static str,
    /// Amount fields take a decimal point (legacy BTC).
    pub decimal: bool,
    pub choices: Vec<Choice>,
    /// The transitional note, until the person acknowledges it.
    pub note: Option<Note>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Choice {
    pub id: &'static str,
    pub label: &'static str,
    pub selected: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Note {
    pub title: &'static str,
    pub lines: Vec<&'static str>,
}

impl Amounts {
    /// Read the saved choice; anything unreadable is the BIP 177 default.
    pub fn open(dir: &Path) -> Self {
        let format = std::fs::read_to_string(dir.join(FORMAT_FILE))
            .ok()
            .and_then(|text| Format::from_id(&text))
            .unwrap_or_default();
        Self {
            dir: dir.to_owned(),
            format,
            note_acknowledged: dir.join(NOTE_FILE).exists(),
        }
    }

    pub fn format(&self) -> Format {
        self.format
    }

    /// Choose a format by its id and save it. An unknown id changes
    /// nothing. Choosing is also reading the note.
    pub fn choose(&mut self, id: &str) -> Option<Format> {
        let format = Format::from_id(id)?;
        let _ = std::fs::create_dir_all(&self.dir);
        // A failed write only means the next launch shows the default.
        let _ = std::fs::write(self.dir.join(FORMAT_FILE), format!("{}\n", format.id()));
        self.format = format;
        self.acknowledge();
        Some(format)
    }

    /// The person read the transitional note.
    pub fn acknowledge(&mut self) {
        let _ = std::fs::create_dir_all(&self.dir);
        let _ = std::fs::write(self.dir.join(NOTE_FILE), b"1\n");
        self.note_acknowledged = true;
    }

    pub fn view(&self) -> AmountsView {
        AmountsView {
            format: self.format.id(),
            unit: self.format.unit(),
            decimal: self.format.decimal_entry(),
            choices: Format::ALL
                .iter()
                .map(|format| Choice {
                    id: format.id(),
                    label: format.label(),
                    selected: *format == self.format,
                })
                .collect(),
            note: (!self.note_acknowledged).then(|| Note {
                title: NOTE_TITLE,
                lines: NOTE_LINES.to_vec(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_choice_persists_and_defaults_to_bip177() {
        let dir = tempfile::tempdir().unwrap();
        let mut amounts = Amounts::open(dir.path());
        assert_eq!(amounts.format(), Format::Bip177);
        let view = amounts.view();
        assert_eq!(
            (view.format, view.unit, view.decimal),
            ("bip177", "₿", false)
        );
        assert!(view.note.is_some());
        assert_eq!(amounts.choose("sats"), None);
        assert_eq!(amounts.format(), Format::Bip177);
        assert_eq!(amounts.choose("btc"), Some(Format::LegacyBtc));
        let reopened = Amounts::open(dir.path());
        assert_eq!(reopened.format(), Format::LegacyBtc);
        let view = reopened.view();
        assert_eq!((view.format, view.unit, view.decimal), ("btc", "BTC", true));
        assert!(view.note.is_none(), "choosing reads the note");
        assert!(view.choices.iter().any(|c| c.id == "btc" && c.selected));
    }

    #[test]
    fn the_note_stays_read() {
        let dir = tempfile::tempdir().unwrap();
        Amounts::open(dir.path()).acknowledge();
        let amounts = Amounts::open(dir.path());
        assert!(amounts.view().note.is_none());
        assert_eq!(amounts.format(), Format::Bip177);
    }

    #[test]
    fn the_note_explains_without_sats_wording() {
        for line in NOTE_LINES {
            assert!(!line.contains(" sat ") && !line.contains("sats"), "{line}");
        }
    }
}
