//! What Coder should favor when OpenAgents picks a model: cost, speed, or
//! intelligence, each from 1 to 5, with caller-provided persistence.

use serde::{Deserialize, Serialize};

use crate::plugin_store::Store;

const FILENAME: &str = "preferences.json";
const VERSION: u32 = 1;
/// The lowest and highest a preference can be set to.
pub const MIN: u8 = 1;
pub const MAX: u8 = 5;
/// What every preference starts at before the person picks anything.
pub const DEFAULT: u8 = 3;
const LOAD_ERROR: &str = "Cannot load the saved preferences. The file was not changed.";
const SAVE_ERROR: &str = "Cannot save the preferences. The previous preferences were not changed.";

/// The three things a person can favor, in the order the screen lists them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Axis {
    #[default]
    Cost,
    Speed,
    Intelligence,
}

/// Every axis in the order the screen shows them.
pub const AXES: [Axis; 3] = [Axis::Cost, Axis::Speed, Axis::Intelligence];

impl Axis {
    /// The name the preferences screen shows.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Cost => "Cost efficiency",
            Self::Speed => "Speed",
            Self::Intelligence => "Intelligence",
        }
    }
}

/// The three preferences and which row the screen has selected.
pub struct Preferences {
    pub cost: u8,
    pub speed: u8,
    pub intelligence: u8,
    pub selected: usize,
    store: Option<Store>,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            cost: DEFAULT,
            speed: DEFAULT,
            intelligence: DEFAULT,
            selected: 0,
            store: None,
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Document {
    version: u32,
    #[serde(default = "default_value")]
    cost: u8,
    #[serde(default = "default_value")]
    speed: u8,
    #[serde(default = "default_value")]
    intelligence: u8,
}

const fn default_value() -> u8 {
    DEFAULT
}

impl Default for Document {
    fn default() -> Self {
        Self {
            version: VERSION,
            cost: DEFAULT,
            speed: DEFAULT,
            intelligence: DEFAULT,
        }
    }
}

impl Document {
    fn valid(&self) -> bool {
        self.version == VERSION
            && [self.cost, self.speed, self.intelligence]
                .into_iter()
                .all(|value| (MIN..=MAX).contains(&value))
    }
}

impl Preferences {
    /// Load saved preferences, or use defaults when no file has been saved.
    ///
    /// # Errors
    /// The file cannot be read or has an invalid or unsupported schema.
    pub fn load(&mut self, store: Store) -> Result<(), String> {
        self.store = Some(store.clone());
        let document = read(&store)?;
        self.apply(&document);
        Ok(())
    }

    /// The value on the selected row.
    #[must_use]
    pub fn value(&self, axis: Axis) -> u8 {
        match axis {
            Axis::Cost => self.cost,
            Axis::Speed => self.speed,
            Axis::Intelligence => self.intelligence,
        }
    }

    /// Move the selection up one row, stopping at the first.
    pub fn up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    /// Move the selection down one row, stopping at the last.
    pub fn down(&mut self) {
        self.selected = (self.selected + 1).min(AXES.len() - 1);
    }

    /// Raise the selected preference by one and save it.
    ///
    /// # Errors
    /// Invalid existing settings are preserved. A failed save leaves the preference unchanged.
    pub fn higher(&mut self) -> Result<(), String> {
        self.adjust(1)
    }

    /// Lower the selected preference by one and save it.
    ///
    /// # Errors
    /// Invalid existing settings are preserved. A failed save leaves the preference unchanged.
    pub fn lower(&mut self) -> Result<(), String> {
        self.adjust(-1)
    }

    fn adjust(&mut self, by: i8) -> Result<(), String> {
        let axis = AXES[self.selected.min(AXES.len() - 1)];
        let current = i16::from(self.value(axis));
        let wanted = (current + i16::from(by)).clamp(i16::from(MIN), i16::from(MAX)) as u8;
        if wanted == self.value(axis) {
            return Ok(());
        }
        let (cost, speed, intelligence) = match axis {
            Axis::Cost => (wanted, self.speed, self.intelligence),
            Axis::Speed => (self.cost, wanted, self.intelligence),
            Axis::Intelligence => (self.cost, self.speed, wanted),
        };
        let document = Document {
            version: VERSION,
            cost,
            speed,
            intelligence,
        };
        if let Some(store) = &self.store {
            read(store)?;
            store
                .save_extra(FILENAME, &document)
                .map_err(|_| SAVE_ERROR.to_owned())?;
        }
        self.apply(&document);
        Ok(())
    }

    fn apply(&mut self, document: &Document) {
        self.cost = document.cost;
        self.speed = document.speed;
        self.intelligence = document.intelligence;
    }
}

fn read(store: &Store) -> Result<Document, String> {
    let document = store
        .read_extra::<Document>(FILENAME)
        .map_err(|_| LOAD_ERROR.to_owned())?
        .unwrap_or_default();
    if !document.valid() {
        return Err(LOAD_ERROR.into());
    }
    Ok(document)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn temporary() -> tempfile::TempDir {
        match std::env::var_os("OPENAGENTS_SCRATCH") {
            Some(root) => tempfile::tempdir_in(root).unwrap(),
            None => tempfile::tempdir().unwrap(),
        }
    }

    #[test]
    fn missing_file_and_missing_values_keep_the_defaults() {
        let temporary = temporary();
        let root = temporary.path().join("coder");
        let mut preferences = Preferences::default();
        preferences.load(Store::under(&root)).unwrap();
        assert_eq!(
            [
                preferences.cost,
                preferences.speed,
                preferences.intelligence
            ],
            [DEFAULT; 3]
        );
        assert!(!root.exists());

        fs::create_dir(&root).unwrap();
        fs::write(root.join(FILENAME), r#"{"version":1,"speed":5}"#).unwrap();
        preferences.load(Store::under(&root)).unwrap();
        assert_eq!(preferences.speed, 5);
        assert_eq!(preferences.cost, DEFAULT);
        assert_eq!(preferences.intelligence, DEFAULT);
    }

    #[test]
    fn each_setting_survives_reloading_and_stops_at_the_ends() {
        let temporary = temporary();
        let store = Store::under(temporary.path().join("coder"));
        let mut preferences = Preferences::default();
        preferences.load(store.clone()).unwrap();
        for (selected, expected) in [(0, Axis::Cost), (1, Axis::Speed), (2, Axis::Intelligence)] {
            preferences.selected = selected;
            assert_eq!(AXES[selected], expected);
            for _ in 0..10 {
                preferences.higher().unwrap();
            }
            assert_eq!(preferences.value(expected), MAX);
            for _ in 0..10 {
                preferences.lower().unwrap();
            }
            assert_eq!(preferences.value(expected), MIN);

            let mut reloaded = Preferences::default();
            reloaded.load(store.clone()).unwrap();
            assert_eq!(reloaded.value(expected), MIN);
        }
        preferences.down();
        assert_eq!(preferences.selected, AXES.len() - 1);
        preferences.up();
        preferences.up();
        assert_eq!(preferences.selected, 0);
        preferences.up();
        assert_eq!(preferences.selected, 0);
    }

    #[test]
    fn a_damaged_or_newer_file_is_kept_and_reports_the_load() {
        let temporary = temporary();
        let store = Store::under(temporary.path());
        let path = temporary.path().join(FILENAME);
        fs::write(
            &path,
            r#"{"version":1,"cost":5,"speed":5,"intelligence":5}"#,
        )
        .unwrap();
        let mut preferences = Preferences::default();
        preferences.load(store.clone()).unwrap();
        for document in [
            "invalid JSON",
            r#"{"version":2,"speed":1}"#,
            r#"{"version":1,"speed":9}"#,
            r#"{"version":1,"unexpected":true}"#,
        ] {
            fs::write(&path, document).unwrap();
            let mut fresh = Preferences::default();
            assert_eq!(fresh.load(store.clone()).unwrap_err(), LOAD_ERROR);
            assert_eq!(fresh.speed, DEFAULT);
            fresh.selected = 1;
            assert_eq!(fresh.higher().unwrap_err(), LOAD_ERROR);
            assert_eq!(fresh.speed, DEFAULT);
            assert_eq!(fs::read_to_string(&path).unwrap(), document);
        }
    }
}
