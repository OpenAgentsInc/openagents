//! How the app shows bitcoin amounts, app-wide: BIP 177 integer base units
//! (`₿12,345`) by default, or legacy BTC (`0.00012345 BTC`) when the person
//! switches. The choice is a plain file under the app's state directory;
//! it is not a secret.
//! Every amount the app shows or reads goes through [`Format`], and every
//! stored or internal amount stays a `u64` of base units.

use serde::Serialize;
use std::path::{Path, PathBuf};

pub use bitcoin_amount::Format;

const FORMAT_FILE: &str = "amount-format";

pub struct Amounts {
    dir: PathBuf,
    format: Format,
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
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Choice {
    pub id: &'static str,
    pub label: &'static str,
    pub selected: bool,
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
        }
    }

    pub fn format(&self) -> Format {
        self.format
    }

    /// Choose a format by its id and save it. An unknown id changes
    /// nothing.
    pub fn choose(&mut self, id: &str) -> Option<Format> {
        let format = Format::from_id(id)?;
        let _ = std::fs::create_dir_all(&self.dir);
        // A failed write only means the next launch shows the default.
        let _ = std::fs::write(self.dir.join(FORMAT_FILE), format!("{}\n", format.id()));
        self.format = format;
        Some(format)
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
        assert_eq!(amounts.choose("sats"), None);
        assert_eq!(amounts.format(), Format::Bip177);
        assert_eq!(amounts.choose("btc"), Some(Format::LegacyBtc));
        let reopened = Amounts::open(dir.path());
        assert_eq!(reopened.format(), Format::LegacyBtc);
        let view = reopened.view();
        assert_eq!((view.format, view.unit, view.decimal), ("btc", "BTC", true));
        assert!(view.choices.iter().any(|c| c.id == "btc" && c.selected));
    }
}
