//! Adopted calibration (#10387): thresholds the nightly refit
//! (`openagents efficiency refit --write`) measured against joined run
//! outcomes and adopted only after a held-out check. Serving reads them
//! through [`crate::decision::Setting::threshold`]; a setting with no
//! adopted value keeps its default.
//!
//! The file is `$OPENAGENTS_CALIBRATION`, else
//! `~/.openagents/calibration/current.json`. A missing, unreadable, or
//! malformed file, or a value outside (0, 1), adopts nothing, and
//! `OPENAGENTS_CALIBRATION=off` turns adoption off (Coder's checks set it,
//! so tests see the code's defaults on a host that has adopted values).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::OnceLock;

/// The adopted-settings file's schema.
pub const SCHEMA: &str = "openagents.calibration.settings.v1";

/// One adopted threshold and the evidence it was adopted on.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Adopted {
    /// The threshold serving uses for this setting.
    pub threshold: f64,
    /// The default it replaced, as the refit read it.
    pub default: f64,
    /// Joined samples the threshold was fitted on.
    pub fit_n: usize,
    /// Held-out samples it was checked on.
    pub held_out_n: usize,
    /// Held-out accuracy at the default threshold.
    pub held_out_accuracy_default: f64,
    /// Held-out accuracy at the adopted threshold.
    pub held_out_accuracy: f64,
    /// What the accuracy is measured against.
    pub label: String,
}

/// An adopted-settings file: a version, when it was fitted, and the
/// settings that passed their held-out check, by setting name.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct File {
    pub schema: String,
    pub version: u64,
    pub fitted_at: String,
    #[serde(default)]
    pub settings: BTreeMap<String, Adopted>,
}

impl File {
    /// The adopted threshold for `name`, when one is in range.
    #[must_use]
    pub fn threshold(&self, name: &str) -> Option<f64> {
        (self.schema == SCHEMA)
            .then(|| self.settings.get(name))
            .flatten()
            .map(|a| a.threshold)
            .filter(|t| *t > 0.0 && *t < 1.0)
    }
}

/// Where the adopted-settings file lives.
#[must_use]
pub fn path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("OPENAGENTS_CALIBRATION") {
        return (p != "off").then(|| PathBuf::from(p));
    }
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".openagents/calibration/current.json"))
}

/// Read an adopted-settings file; anything unreadable adopts nothing.
#[must_use]
pub fn read(path: &std::path::Path) -> File {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

/// The adopted settings this process uses, read once.
pub fn current() -> &'static File {
    static CURRENT: OnceLock<File> = OnceLock::new();
    CURRENT.get_or_init(|| path().map(|p| read(&p)).unwrap_or_default())
}

/// The adopted threshold for setting `name`, if any.
#[must_use]
pub fn adopted(name: &str) -> Option<f64> {
    current().threshold(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adopted_at(t: f64) -> Adopted {
        Adopted {
            threshold: t,
            default: 0.8,
            fit_n: 60,
            held_out_n: 30,
            held_out_accuracy_default: 0.6,
            held_out_accuracy: 0.7,
            label: "wall_time_over_10_minutes".into(),
        }
    }

    #[test]
    fn only_in_range_values_under_the_schema_are_adopted() {
        let mut file = File {
            schema: SCHEMA.into(),
            version: 3,
            fitted_at: "2026-10-03T04:00:00Z".into(),
            settings: BTreeMap::new(),
        };
        file.settings.insert("recipe.hard".into(), adopted_at(0.65));
        file.settings
            .insert("recipe.check_keep".into(), adopted_at(1.2));
        assert_eq!(file.threshold("recipe.hard"), Some(0.65));
        assert_eq!(file.threshold("recipe.check_keep"), None);
        assert_eq!(file.threshold("system.select"), None);
        file.schema = "other".into();
        assert_eq!(file.threshold("recipe.hard"), None);
    }

    #[test]
    fn a_missing_or_malformed_file_adopts_nothing() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(read(&dir.path().join("none.json")), File::default());
        let bad = dir.path().join("bad.json");
        std::fs::write(&bad, "{not json").unwrap();
        assert_eq!(read(&bad), File::default());
    }
}
