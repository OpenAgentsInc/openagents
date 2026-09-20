//! Turning a raw signal into a number a caller may act on.
//!
//! An estimator reports how often the model selected an option. That is not a
//! probability of being right, and the gap between the two is what this
//! module measures and, where it can, closes.
//!
//! The map is a binned reliability table rather than a fit with a shape
//! assumption. A table shows its own resolution and the count behind each
//! bin, so a thin bin is visible instead of smoothed over. Temperature
//! scaling, which is what kev uses, does not apply: there are no logits to
//! scale.
//!
//! # Why this is in the Gym
//!
//! It was `crates/lev/src/calibrate.rs`, and nothing in it is Apple's. A
//! reliability table over labelled outcomes fits any door that answers
//! `POST /v1/systemone`, and the records it writes were already being written
//! for kev. What is Apple's — the constrained certainty band, the runtime
//! probe — stayed behind in `crates/lev`.
//!
//! # What a record has to be able to say
//!
//! [`Record`] carries the provenance a served probability rests on, because
//! the committed maps this module inherited could not. They named an
//! operating system build, which is identical for every door on one machine,
//! and so they sat on disk through two adapter changes that altered which
//! question families are admitted at all. A record now names the door, what
//! that door was running, the suite partition it was fitted on, the estimator
//! block it drew, the gate that judged it, and any locked-partition read it
//! rests on. [`Record::serve_to`] checks the record against the door that is
//! actually running and names the field that does not match.
//!
//! The verdict is not computed here. `admit`, the three-condition function
//! this module used to carry, is now [`crate::gate`], where a rule has a
//! digest and a candidate can be the better decision and the worse
//! probability at once.
//!
//! # A map calibrates a fixed answer, and does not choose a new one
//!
//! [`Map::apply_distribution`] gives the selected option its calibrated
//! probability and shares what is left among the rest. It does **not**
//! guarantee that the selected option still holds the largest number
//! afterwards, and two documents on `main` disagreed about that until
//! openagents#9438 enumerated it. The rescaled distribution's own argmax
//! moves to the runner-up exactly when the calibrated probability falls
//! below `m / (m + rest)`, where `m` is the largest losing share and `rest`
//! is all of them together. Because `m` never exceeds `rest`, that threshold
//! never exceeds one half: a map that reads every signal at or above 0.5
//! cannot move an argmax whatever the distribution looks like, and a map
//! with a bin below 0.5 can. [`Map::fit`] produces such a bin whenever fewer
//! than half the observations in it were right.
//!
//! The contract this repository holds is that **the selected option is the
//! estimator's argmax and a map never replaces it**. A map is fitted on
//! [`Observation`]s whose `correct` means "the estimator's choice was the
//! labelled answer", so `fitted` estimates how often that choice is right
//! and estimates nothing about which other option would be right instead.
//! The remainder is spread in the estimator's own proportions, which is a
//! display convention rather than a fitted quantity, and reading a new
//! answer out of it reads a prediction from a number nobody fitted.
//!
//! So every consumer of a rescaled distribution reads the selected option
//! from [`selected`] rather than from the rescaled distribution's argmax.
//! A calibrated probability below one half is not a different answer; it is
//! the same answer, reported as more likely wrong than right, which is what
//! a caller's refusal threshold is for.
//!
//! The other contract — letting the predictor change and recomputing
//! correctness — is not available over the committed store.
//! [`crate::row::Row`] carries `correct` and no label, so nothing in a row
//! can say whether the runner-up was the answer.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::gate::Scores;
use crate::row::DoorIdentity;

/// One observation: what the estimator reported for the winning option, and
/// whether that option turned out to be right.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Observation {
    /// The raw frequency the winning option carried.
    pub raw: f64,
    /// Whether the winning option was the labelled answer.
    pub correct: bool,
    /// The certainty band the model selected, when one was asked for.
    ///
    /// On the base model this is noise — every item comes back `likely` and
    /// the lowest band scores highest. On an adapter trained against
    /// outcomes it is monotone, and a map can use it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub band: Option<String>,
}

impl Observation {
    /// An observation with no band.
    #[must_use]
    pub const fn new(raw: f64, correct: bool) -> Self {
        Self { raw, correct, band: None }
    }

    /// An observation carrying the band the model selected.
    #[must_use]
    pub fn banded(raw: f64, correct: bool, band: impl Into<String>) -> Self {
        Self { raw, correct, band: Some(band.into()) }
    }
}

/// One bin of a reliability table.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bin {
    /// Lower edge, inclusive.
    pub lo: f64,
    /// Upper edge, inclusive at the top bin.
    pub hi: f64,
    /// The share of items in this bin that were correct.
    pub fitted: f64,
    /// How many items landed here. A thin bin is a weak claim.
    pub count: usize,
}

/// A fitted map from a raw signal to a probability.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Map {
    /// The bins, in order, covering 0 to 1.
    pub bins: Vec<Bin>,
    /// The overall correct rate, used where a bin is empty.
    pub base_rate: f64,
    /// How many observations the map was fitted on.
    pub fitted_on: usize,
    /// One table per certainty band, when the model reports a band that
    /// carries signal.
    ///
    /// The frequency alone says how consistently the model answered. The
    /// band says how reliable an answer like this is. Splitting the table by
    /// band conditions on both, which is the whole reason for training a
    /// band in the first place.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub by_band: BTreeMap<String, Vec<Bin>>,
}

/// The fewest observations a bin should rest on before it is worth having.
///
/// Below this a bin is noise with a decimal point: the first suite fitted
/// five bins on twelve items and turned a raw ECE of 0.031 into 0.113.
pub const ITEMS_PER_BIN: usize = 15;

impl Map {
    /// Fits a table whose bin count follows the evidence.
    ///
    /// More data buys more resolution. Less data buys fewer, wider bins
    /// rather than a finer table with nothing in it.
    #[must_use]
    pub fn fit_auto(observations: &[Observation]) -> Self {
        let bins = (observations.len() / ITEMS_PER_BIN).clamp(2, 10);
        Self::fit(observations, bins)
    }

    /// Fits a table on observations from the calibration split.
    ///
    /// A bin with no observations falls back to the base rate rather than
    /// interpolating, because interpolation invents a claim the data did not
    /// make.
    #[must_use]
    pub fn fit(observations: &[Observation], bins: usize) -> Self {
        let bins = bins.max(1);
        let base_rate = if observations.is_empty() {
            0.0
        } else {
            observations.iter().filter(|o| o.correct).count() as f64 / observations.len() as f64
        };
        let mut built = Vec::with_capacity(bins);
        for index in 0..bins {
            let lo = index as f64 / bins as f64;
            let hi = (index + 1) as f64 / bins as f64;
            let inside: Vec<&Observation> = observations
                .iter()
                .filter(|o| o.raw >= lo && (o.raw < hi || (index + 1 == bins && o.raw <= hi)))
                .collect();
            let count = inside.len();
            let fitted = if count == 0 {
                base_rate
            } else {
                // Jeffreys smoothing. A bin holding six items that were all
                // correct has not established that the next one is certain,
                // and an unsmoothed 1.0 makes the map claim it — which costs
                // nothing in ECE and is unbounded in log loss the first time
                // it is wrong. The prior pulls a small bin toward the middle
                // in proportion to how little it saw.
                let correct = inside.iter().filter(|o| o.correct).count() as f64;
                (correct + 0.5) / (count as f64 + 1.0)
            };
            built.push(Bin { lo, hi, fitted, count });
        }
        Self { bins: built, base_rate, fitted_on: observations.len(), by_band: BTreeMap::new() }
    }

    /// Fits a table per band, falling back to the pooled table where a band
    /// is too thin to say anything.
    ///
    /// A band table is kept only when it rests on at least `ITEMS_PER_BIN`
    /// observations. Below that the pooled table is the better estimate, and
    /// a two-item band claiming its own probability is exactly the
    /// small-sample failure the admission gate exists to catch.
    #[must_use]
    pub fn fit_banded(observations: &[Observation]) -> Self {
        let mut map = Self::fit_auto(observations);
        let mut grouped: BTreeMap<String, Vec<Observation>> = BTreeMap::new();
        for observation in observations {
            if let Some(band) = &observation.band {
                grouped.entry(band.clone()).or_default().push(observation.clone());
            }
        }
        for (band, inside) in grouped {
            if inside.len() < ITEMS_PER_BIN {
                continue;
            }
            map.by_band.insert(band, Self::fit_auto(&inside).bins);
        }
        map
    }

    /// Maps a raw signal to a probability, using the band's own table when
    /// one was fitted for it.
    #[must_use]
    pub fn apply_banded(&self, raw: f64, band: Option<&str>) -> f64 {
        let bins = band
            .and_then(|band| self.by_band.get(band))
            .unwrap_or(&self.bins);
        for bin in bins {
            if raw >= bin.lo && (raw < bin.hi || (bin.hi - 1.0).abs() < f64::EPSILON) {
                return bin.fitted;
            }
        }
        self.base_rate
    }

    /// Rescales a distribution using the band's table where one exists.
    #[must_use]
    pub fn apply_distribution_banded(
        &self,
        raw: &IndexMap<String, f64>,
        band: Option<&str>,
    ) -> IndexMap<String, f64> {
        let Some((winner, top)) = selected(raw) else {
            return raw.clone();
        };
        let winner = winner.to_string();
        let calibrated = self.apply_banded(top, band).clamp(0.0, 1.0);
        rescale(raw, &winner, calibrated)
    }

    /// Maps a raw signal to a calibrated probability for the winning option.
    #[must_use]
    pub fn apply(&self, raw: f64) -> f64 {
        for bin in &self.bins {
            if raw >= bin.lo && (raw < bin.hi || (bin.hi - 1.0).abs() < f64::EPSILON) {
                return bin.fitted;
            }
        }
        self.base_rate
    }

    /// Rescales a whole distribution so the selected option carries its
    /// calibrated probability and the rest share what is left, in their
    /// observed proportions.
    ///
    /// The selected option is [`selected`]'s, and it is unchanged by this
    /// call. It is not always the largest number in what comes back: a
    /// calibrated probability below `m / (m + rest)` leaves a runner-up
    /// above it. Read the answer from [`selected`] on the raw distribution,
    /// never from the argmax of this one. The module documentation carries
    /// the reasoning.
    #[must_use]
    pub fn apply_distribution(&self, raw: &IndexMap<String, f64>) -> IndexMap<String, f64> {
        let Some((winner, top)) = selected(raw) else {
            return raw.clone();
        };
        let winner = winner.to_string();
        let calibrated = self.apply(top).clamp(0.0, 1.0);
        rescale(raw, &winner, calibrated)
    }
}

/// The option a distribution selects, and the frequency it carries.
///
/// This is the estimator's argmax, it is what an [`Observation`]'s `correct`
/// refers to, and it is what a door answers with. A map rescales its
/// probability and never replaces it, so this reads the raw distribution
/// rather than a rescaled one.
///
/// Equal leaders resolve to the last of them, which is the option the
/// estimator listed last rather than one the numbers chose. `None` for an
/// empty distribution, which is not an answer.
#[must_use]
pub fn selected(raw: &IndexMap<String, f64>) -> Option<(&str, f64)> {
    raw.iter()
        .max_by(|left, right| left.1.total_cmp(right.1))
        .map(|(option, top)| (option.as_str(), *top))
}

/// Gives the winner its calibrated probability and shares what is left among
/// the rest, in the proportions the estimator observed.
fn rescale(
    raw: &IndexMap<String, f64>,
    winner: &str,
    calibrated: f64,
) -> IndexMap<String, f64> {
    let rest: f64 = raw.iter().filter(|(k, _)| k.as_str() != winner).map(|(_, v)| *v).sum();
    let remaining = 1.0 - calibrated;
    raw.iter()
        .map(|(key, value)| {
            if key == winner {
                (key.clone(), calibrated)
            } else if rest > 0.0 {
                (key.clone(), remaining * value / rest)
            } else {
                // The estimator was unanimous, so the leftover mass has no
                // observed shape to follow and is spread evenly.
                (key.clone(), remaining / (raw.len() - 1).max(1) as f64)
            }
        })
        .collect()
}

/// How well a set of probabilities matched what happened.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Metrics {
    /// Share of items whose winning option was the labelled answer.
    pub accuracy: f64,
    /// Expected calibration error over ten bins.
    pub ece: f64,
    /// Brier score on the winning option.
    pub brier: f64,
    /// Negative log likelihood of the winning option's outcome.
    pub nll: f64,
    /// Items that were wrong at a reported probability of 0.9 or above.
    pub confident_errors: usize,
    /// How many items were scored.
    pub items: usize,
}

/// Scores observations whose `raw` is already a reported probability.
#[must_use]
pub fn score(observations: &[Observation]) -> Metrics {
    if observations.is_empty() {
        return Metrics::default();
    }
    let n = observations.len() as f64;
    let accuracy = observations.iter().filter(|o| o.correct).count() as f64 / n;
    let brier = observations
        .iter()
        .map(|o| {
            let outcome = if o.correct { 1.0 } else { 0.0 };
            (o.raw - outcome).powi(2)
        })
        .sum::<f64>()
        / n;
    let nll = observations
        .iter()
        .map(|o| {
            let p = if o.correct { o.raw } else { 1.0 - o.raw };
            -p.clamp(1e-12, 1.0).ln()
        })
        .sum::<f64>()
        / n;
    let confident_errors = observations.iter().filter(|o| !o.correct && o.raw >= 0.9).count();

    let mut ece = 0.0;
    for index in 0..10 {
        let lo = index as f64 / 10.0;
        let hi = (index + 1) as f64 / 10.0;
        let inside: Vec<&Observation> = observations
            .iter()
            .filter(|o| o.raw >= lo && (o.raw < hi || (index == 9 && o.raw <= hi)))
            .collect();
        if inside.is_empty() {
            continue;
        }
        let share = inside.len() as f64 / n;
        let mean_p = inside.iter().map(|o| o.raw).sum::<f64>() / inside.len() as f64;
        let mean_correct =
            inside.iter().filter(|o| o.correct).count() as f64 / inside.len() as f64;
        ece += share * (mean_p - mean_correct).abs();
    }

    Metrics { accuracy, ece, brier, nll, confident_errors, items: observations.len() }
}

impl Metrics {
    /// The same numbers in the shape a gate judges.
    ///
    /// Every field a gate reads is optional there and measured here, so the
    /// conversion fills each one. A measure that was never taken never
    /// reaches this type: it is absent from the set of observations, and the
    /// items count says how many there were.
    #[must_use]
    pub const fn scores(&self) -> Scores {
        Scores {
            items: self.items,
            accuracy: Some(self.accuracy),
            ece: Some(self.ece),
            brier: Some(self.brier),
            nll: Some(self.nll),
            confident_errors: Some(self.confident_errors),
        }
    }
}

fn english() -> String {
    "en".to_string()
}

/// The schema tag a calibration record carries.
pub const RECORD_SCHEMA: &str = "openagents.gym.calibration_record.v1";

/// Which estimator drew the raw signal, and from where.
///
/// The seed block is part of this because the doors here reproduce exactly:
/// `l2` over block 0 and `l2` over block 3 are two trials of the same items,
/// and a record that names only the estimator cannot say which one it saw.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EstimatorConfig {
    /// The estimator's label, such as `l2`.
    pub estimator: String,
    /// How many draws one estimate rests on.
    pub samples: u64,
    /// The seed block the draws came from. Block 0 is the default.
    #[serde(default)]
    pub seed_base: u64,
}

impl EstimatorConfig {
    /// A configuration naming the estimator, its draws, and its seed block.
    #[must_use]
    pub fn new(estimator: impl Into<String>, samples: u64, seed_base: u64) -> Self {
        Self { estimator: estimator.into(), samples, seed_base }
    }
}

/// Why a record may not serve the door that is running.
///
/// Each value names one field, because "the record does not match" is not an
/// answer a person can act on. A door that refuses says which of these it
/// found.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Mismatch {
    /// The document is tagged as something other than a calibration record.
    #[error("the record is tagged {found}, not {RECORD_SCHEMA}")]
    Schema {
        /// The tag the file carried.
        found: String,
    },
    /// The gate refused this map, so there is nothing to serve.
    #[error("the record was not admitted: {verdict}")]
    NotAdmitted {
        /// The verdict the gate recorded.
        verdict: String,
    },
    /// The operating system build has moved.
    #[error("os_build: the map was fitted on {fitted} and this door runs {serving}")]
    OsBuild {
        /// The build the map was fitted on.
        fitted: String,
        /// The build the door reports.
        serving: String,
    },
    /// The record cannot name what it was fitted against.
    #[error(
        "door_identity.verified: the record names no base model signature, so there is nothing \
         to match it against"
    )]
    RecordUnverifiable,
    /// The door cannot say what it is running.
    #[error(
        "door_identity.verified: this door publishes no base model signature, so no record can \
         be checked against it"
    )]
    DoorUnverifiable,
    /// The base model underneath has changed.
    #[error("base_model_signature: the map was fitted against {fitted} and this door runs {serving}")]
    BaseModelSignature {
        /// The signature the map was fitted against.
        fitted: String,
        /// The signature the door reports.
        serving: String,
    },
    /// The adapter has changed, which changes the door.
    #[error("adapter: the map was fitted against {fitted} and this door serves {serving}")]
    Adapter {
        /// The adapter the map was fitted against, or `none`.
        fitted: String,
        /// The adapter the door serves, or `none`.
        serving: String,
    },
}

/// What a calibrated question family carries.
///
/// A family without one of these does not serve probabilities, and a record
/// whose base signature no longer matches the serving runtime does not
/// either.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Record {
    /// What this document is. Always [`RECORD_SCHEMA`] for a record this
    /// crate wrote, and written into the file rather than inferred from its
    /// path, so a record that escapes its directory still says what it is.
    #[serde(default = "record_schema")]
    pub schema: String,
    /// The question family this map covers.
    pub family: String,
    /// Which estimator produced the raw signal, and from which seed block.
    pub estimator_config: EstimatorConfig,
    /// The language the suite is written in. English, and only English —
    /// every suite here is English and the maps are fitted on English items.
    /// Recorded rather than implied so a later reader does not assume the
    /// map covers input it never saw.
    #[serde(default = "english")]
    pub language: String,
    /// The suite the map was fitted and scored on.
    pub suite: String,
    /// The suite's content digest.
    pub suite_digest: String,
    /// The partition the map was fitted on.
    ///
    /// A map fitted on the calibration partition and a map fitted on the
    /// development partition are different claims, and a record that names
    /// only the suite cannot tell them apart. The digest pins the items; this
    /// pins which of them were spent on the fit.
    #[serde(default)]
    pub partition_id: String,
    /// The operating system build the runtime reported.
    pub os_build: String,
    /// Which door produced the observations.
    ///
    /// Without this a record is unattributable, and an unattributable
    /// calibration map is worse than none: the committed records were fitted
    /// against the base model and stayed on disk while two adapters changed
    /// which families are admitted at all. `os_build` is identical across
    /// every door on one machine, so it could not reveal the drift.
    #[serde(default)]
    pub door: String,
    /// What the door was running, as far as it can be verified.
    ///
    /// For an on-device door this is the base model signature plus the
    /// adapter package digest. For a hosted door there is nothing to verify
    /// and the fields stay empty rather than being invented.
    #[serde(default)]
    pub door_identity: DoorIdentity,
    /// The acceptance rule that judged this map.
    ///
    /// A record whose verdict names no rule is a claim about a candidate that
    /// nobody can re-run, which is the fault `crate::gate` exists to stop.
    #[serde(default)]
    pub gate_id: Option<String>,
    /// That rule's content digest, so retuning a floor produces a new rule
    /// rather than rewriting this verdict's meaning.
    #[serde(default)]
    pub gate_digest: Option<String>,
    /// The locked-partition reads this record rests on, by their subjects.
    ///
    /// Empty for a map fitted and scored on the open partitions, which is the
    /// normal case. A record that spent the held-out set says so here, and
    /// the claim is checkable against the ledger the read was written to.
    #[serde(default)]
    pub locked_reads: Vec<String>,
    /// The day it was fitted.
    pub fitted: String,
    /// The map.
    pub map: Map,
    /// Scores on the evaluation split, before the map is applied.
    pub raw_metrics: Metrics,
    /// Scores on the evaluation split, after the map is applied.
    pub calibrated_metrics: Metrics,
    /// Whether the map earned the right to serve.
    pub admitted: bool,
    /// Why it was admitted or refused, in one line.
    pub verdict: String,
}

fn record_schema() -> String {
    RECORD_SCHEMA.to_string()
}

impl Record {
    /// Reads a record from JSON.
    ///
    /// # Errors
    ///
    /// Returns what `serde_json` reports when the document is not a record.
    pub fn from_json(source: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(source)
    }

    /// Reads every record in a directory, ordered by file name.
    ///
    /// A file that does not parse is an error rather than a skip. A serving
    /// door that silently drops an unreadable record serves the families it
    /// happened to understand and says nothing about the rest.
    ///
    /// # Errors
    ///
    /// Returns the path and the reason for the first file that cannot be read
    /// or does not parse. A directory that does not exist reads as no
    /// records, because holding no calibration is a normal state.
    pub fn load_dir(dir: &Path) -> Result<Vec<(PathBuf, Self)>, String> {
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(format!("{}: {error}", dir.display())),
        };
        let mut paths = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|error| format!("{}: {error}", dir.display()))?;
            let path = entry.path();
            if path.extension().and_then(|extension| extension.to_str()) == Some("json") {
                paths.push(path);
            }
        }
        paths.sort();
        let mut records = Vec::with_capacity(paths.len());
        for path in paths {
            let text = std::fs::read_to_string(&path)
                .map_err(|error| format!("{}: {error}", path.display()))?;
            let record =
                Self::from_json(&text).map_err(|error| format!("{}: {error}", path.display()))?;
            records.push((path, record));
        }
        Ok(records)
    }

    /// Whether this record may serve the door that is running, and which
    /// field says no.
    ///
    /// `docs/lev/calibration.md`'s fifth gate says the base model signature
    /// the map was fitted against must match the one serving. That gate was
    /// written down before the field existed, so it was unimplementable; it
    /// is implementable now, and this is it.
    ///
    /// # Errors
    ///
    /// Returns the first [`Mismatch`] found, which names the field.
    pub fn serve_to(&self, os_build: &str, identity: &DoorIdentity) -> Result<(), Mismatch> {
        if self.schema != RECORD_SCHEMA {
            return Err(Mismatch::Schema { found: self.schema.clone() });
        }
        if !self.admitted {
            return Err(Mismatch::NotAdmitted { verdict: self.verdict.clone() });
        }
        if self.os_build != os_build {
            return Err(Mismatch::OsBuild {
                fitted: name_or_none(&self.os_build),
                serving: name_or_none(os_build),
            });
        }
        // An unverifiable door cannot match: there is nothing to compare.
        // Refusing here is what keeps a hosted door, which publishes a name
        // and no signature, from picking up a map fitted on something else
        // that happens to carry the same name.
        if !self.door_identity.verified {
            return Err(Mismatch::RecordUnverifiable);
        }
        if !identity.verified {
            return Err(Mismatch::DoorUnverifiable);
        }
        if self.door_identity.base_model_signature != identity.base_model_signature {
            return Err(Mismatch::BaseModelSignature {
                fitted: name_or_none(&self.door_identity.base_model_signature),
                serving: name_or_none(&identity.base_model_signature),
            });
        }
        if self.door_identity.adapter != identity.adapter {
            return Err(Mismatch::Adapter {
                fitted: name_or_none(&self.door_identity.adapter),
                serving: name_or_none(&identity.adapter),
            });
        }
        Ok(())
    }

    /// Whether this record may be served against the given host and door.
    #[must_use]
    pub fn valid_for(&self, os_build: &str, identity: &DoorIdentity) -> bool {
        self.serve_to(os_build, identity).is_ok()
    }
}

/// An empty field reads as `none` rather than as an empty string, so a
/// refusal message says what it means.
fn name_or_none(value: &str) -> String {
    if value.is_empty() { "none".to_string() } else { value.to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observations(pairs: &[(f64, bool)]) -> Vec<Observation> {
        pairs.iter().map(|(raw, correct)| Observation::new(*raw, *correct)).collect()
    }

    fn banded(pairs: &[(f64, bool, &str)]) -> Vec<Observation> {
        pairs
            .iter()
            .map(|(raw, correct, band)| Observation::banded(*raw, *correct, *band))
            .collect()
    }

    #[test]
    fn a_map_reports_what_a_bin_actually_did() {
        // Everything the estimator called 1.0 was right half the time, and
        // the smoothing prior is symmetric, so a balanced bin still reads 0.5.
        let fitted = Map::fit(
            &observations(&[(1.0, true), (1.0, false), (1.0, true), (1.0, false)]),
            4,
        );
        assert!((fitted.apply(1.0) - 0.5).abs() < 1e-12);
    }

    #[test]
    fn a_small_unanimous_bin_does_not_claim_certainty() {
        // Six items, all correct. An unsmoothed table would report 1.0 and
        // take an unbounded log-loss penalty the first time it is wrong.
        let fitted = Map::fit(&observations(&[(1.0, true); 6]), 2);
        let claimed = fitted.apply(1.0);
        assert!(claimed < 1.0, "the map claimed {claimed}");
        assert!(claimed > 0.85, "the map gave away too much, at {claimed}");
        // More evidence buys more confidence.
        let firmer = Map::fit(&observations(&[(1.0, true); 60]), 2);
        assert!(firmer.apply(1.0) > claimed);
    }

    #[test]
    fn an_empty_bin_falls_back_to_the_base_rate_rather_than_inventing_one() {
        let fitted = Map::fit(&observations(&[(1.0, true), (1.0, true), (1.0, false)]), 10);
        // Nothing ever landed near 0.15, so the map declines to claim more
        // than the overall rate.
        assert!((fitted.apply(0.15) - fitted.base_rate).abs() < 1e-12);
        assert_eq!(fitted.bins.iter().find(|b| b.lo < 0.2 && b.lo >= 0.1).unwrap().count, 0);
    }

    #[test]
    fn a_distribution_keeps_its_shape_when_it_is_rescaled() {
        let fitted = Map::fit(&observations(&[(1.0, true), (1.0, false)]), 2);
        let raw: IndexMap<String, f64> =
            [("a".to_string(), 0.8), ("b".to_string(), 0.15), ("c".to_string(), 0.05)]
                .into_iter()
                .collect();
        let out = fitted.apply_distribution(&raw);
        let total: f64 = out.values().sum();
        assert!((total - 1.0).abs() < 1e-9, "a distribution sums to one, got {total}");
        // b kept three times c's share, as it had before.
        assert!((out["b"] / out["c"] - 3.0).abs() < 1e-9);
    }

    #[test]
    fn a_unanimous_estimate_spreads_the_remainder_evenly() {
        let fitted = Map::fit(&observations(&[(1.0, true), (1.0, false)]), 2);
        let raw: IndexMap<String, f64> =
            [("a".to_string(), 1.0), ("b".to_string(), 0.0), ("c".to_string(), 0.0)]
                .into_iter()
                .collect();
        let out = fitted.apply_distribution(&raw);
        assert!((out["b"] - out["c"]).abs() < 1e-12);
        assert!((out.values().sum::<f64>() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn perfect_confidence_that_is_always_wrong_scores_as_badly_as_it_should() {
        let metrics = score(&observations(&[(1.0, false), (1.0, false)]));
        assert!((metrics.accuracy - 0.0).abs() < 1e-12);
        assert!((metrics.ece - 1.0).abs() < 1e-12);
        assert!((metrics.brier - 1.0).abs() < 1e-12);
        assert_eq!(metrics.confident_errors, 2);
    }

    #[test]
    fn a_well_calibrated_set_scores_near_zero_error() {
        // Eight items at 0.75, six of them right.
        let mut pairs = vec![(0.75, true); 6];
        pairs.extend(vec![(0.75, false); 2]);
        let metrics = score(&observations(&pairs));
        assert!(metrics.ece < 1e-9, "ece was {}", metrics.ece);
    }

    #[test]
    fn the_bin_count_follows_the_evidence() {
        let few = Map::fit_auto(&observations(&[(1.0, true); 10]));
        assert_eq!(few.bins.len(), 2, "ten items do not support a fine table");
        let some = Map::fit_auto(&observations(&[(1.0, true); 75]));
        assert_eq!(some.bins.len(), 5);
        let many = Map::fit_auto(&observations(&[(1.0, true); 1000]));
        assert_eq!(many.bins.len(), 10, "the table stops widening at ten bins");
    }

    #[test]
    fn a_band_table_separates_what_the_frequency_alone_cannot() {
        // Thirty items, all reported at 1.00 by the estimator, so the pooled
        // table can only say one thing about them. The band splits them: the
        // `unlikely` half is right a third of the time and the
        // `almost certain` half almost always.
        let mut rows: Vec<(f64, bool, &str)> = Vec::new();
        for index in 0..15 {
            rows.push((1.0, index % 3 == 0, "unlikely"));
        }
        for index in 0..15 {
            rows.push((1.0, index != 0, "almost certain"));
        }
        let map = Map::fit_banded(&banded(&rows));

        let pooled = map.apply(1.0);
        let low = map.apply_banded(1.0, Some("unlikely"));
        let high = map.apply_banded(1.0, Some("almost certain"));
        assert!(low < pooled, "the low band should read below the pool: {low} against {pooled}");
        assert!(high > pooled, "the high band should read above it: {high} against {pooled}");
        assert!(low < 0.5 && high > 0.8, "low {low}, high {high}");
    }

    #[test]
    fn a_thin_band_falls_back_to_the_pooled_table() {
        // Two items in a band is not a probability. The map declines to fit
        // one and the pooled answer is used instead.
        let mut rows: Vec<(f64, bool, &str)> = vec![(1.0, false, "unlikely"), (1.0, false, "unlikely")];
        for _ in 0..20 {
            rows.push((1.0, true, "almost certain"));
        }
        let map = Map::fit_banded(&banded(&rows));
        assert!(!map.by_band.contains_key("unlikely"), "a two-item band was fitted");
        assert!(map.by_band.contains_key("almost certain"));
        // Falling back means the thin band reads the pool, not its own two items.
        assert!((map.apply_banded(1.0, Some("unlikely")) - map.apply(1.0)).abs() < 1e-12);
    }

    #[test]
    fn an_unknown_band_reads_the_pooled_table() {
        let map = Map::fit_banded(&banded(&[(1.0, true, "likely"); 20]));
        assert!((map.apply_banded(1.0, Some("never seen")) - map.apply(1.0)).abs() < 1e-12);
        assert!((map.apply_banded(1.0, None) - map.apply(1.0)).abs() < 1e-12);
    }

    fn record() -> Record {
        Record {
            schema: RECORD_SCHEMA.to_string(),
            family: "routing".to_string(),
            language: english(),
            estimator_config: EstimatorConfig::new("l2", 8, 0),
            suite: "support-v2-three-way".to_string(),
            suite_digest: "abc".to_string(),
            partition_id: "calibration".to_string(),
            os_build: "25E246".to_string(),
            door: "lev-base".to_string(),
            door_identity: DoorIdentity::published(
                "lev-base",
                "9799725ff8e851184037110b422d891ad3b92ec1",
                "",
            ),
            gate_id: Some("probability-v1".to_string()),
            gate_digest: Some("gate:abc".to_string()),
            locked_reads: Vec::new(),
            fitted: "2026-09-19".to_string(),
            map: Map::fit(&observations(&[(1.0, true)]), 2),
            raw_metrics: Metrics::default(),
            calibrated_metrics: Metrics::default(),
            admitted: true,
            verdict: "admitted: test".to_string(),
        }
    }

    #[test]
    fn a_record_refuses_a_host_it_was_not_fitted_on() {
        let record = record();
        let same = record.door_identity.clone();
        assert!(record.valid_for("25E246", &same));

        let moved = record.serve_to("25F100", &same).expect_err("a new build is a new host");
        assert!(matches!(moved, Mismatch::OsBuild { .. }), "{moved}");
        assert!(moved.to_string().starts_with("os_build:"), "{moved}");
    }

    #[test]
    fn a_base_fitted_record_refuses_an_adapted_door_and_names_the_field() {
        // The drift that actually happened: two adapters landed and the
        // committed maps, which could name only an operating system build,
        // went on matching.
        let record = record();
        let adapted = DoorIdentity {
            adapter: "fmadapter-lev-9799725".to_string(),
            ..record.door_identity.clone()
        };
        let refused = record.serve_to("25E246", &adapted).expect_err("an adapter is a new door");
        assert_eq!(
            refused,
            Mismatch::Adapter {
                fitted: "none".to_string(),
                serving: "fmadapter-lev-9799725".to_string(),
            }
        );
        assert!(refused.to_string().starts_with("adapter:"), "{refused}");

        let rebased = DoorIdentity::published("lev-base", "a-later-base", "");
        let refused = record.serve_to("25E246", &rebased).expect_err("a new base is a new door");
        assert!(refused.to_string().starts_with("base_model_signature:"), "{refused}");
    }

    #[test]
    fn nothing_verifiable_serves_nothing() {
        let record = record();
        // A hosted door publishes a name and no signature, so no record may
        // claim it. Matching on the name alone is how a map fitted against
        // one model serves another that reused the label.
        let hosted = DoorIdentity::hosted("jev-latest");
        assert_eq!(record.serve_to("25E246", &hosted), Err(Mismatch::DoorUnverifiable));

        // And a record that cannot say what it was fitted against never
        // serves, whatever door asks. This is what the three committed maps
        // were: an operating system build, and nothing else.
        let unattributable = Record { door_identity: DoorIdentity::default(), ..record.clone() };
        assert_eq!(
            unattributable.serve_to("25E246", &record.door_identity),
            Err(Mismatch::RecordUnverifiable)
        );
    }

    #[test]
    fn a_refused_map_does_not_serve_and_says_the_verdict() {
        let refused = Record {
            admitted: false,
            verdict: "refused: Brier rose".to_string(),
            ..record()
        };
        let identity = refused.door_identity.clone();
        assert_eq!(
            refused.serve_to("25E246", &identity),
            Err(Mismatch::NotAdmitted { verdict: "refused: Brier rose".to_string() })
        );
    }

    #[test]
    fn a_record_round_trips_and_carries_its_provenance() {
        let record = record();
        let rendered = serde_json::to_string(&record).expect("a record serializes");
        for field in [
            "schema",
            "estimator_config",
            "partition_id",
            "gate_id",
            "gate_digest",
            "locked_reads",
            "door",
            "door_identity",
        ] {
            assert!(rendered.contains(field), "{field} is recorded: {rendered}");
        }
        let read = Record::from_json(&rendered).expect("a record parses");
        assert_eq!(read, record);
        assert_eq!(read.estimator_config.seed_base, 0, "the seed block travels with the record");
    }

    #[test]
    fn a_document_tagged_as_something_else_does_not_serve() {
        let mislabelled = Record { schema: "openagents.lev.calibration.v1".to_string(), ..record() };
        let identity = mislabelled.door_identity.clone();
        assert!(matches!(
            mislabelled.serve_to("25E246", &identity),
            Err(Mismatch::Schema { .. })
        ));
    }

    #[test]
    fn a_directory_of_records_reads_in_order_and_an_absent_one_reads_as_none() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        assert!(
            Record::load_dir(&dir.path().join("nothing-here")).expect("an absent directory").is_empty(),
            "holding no calibration is a normal state, not an error"
        );

        let record = record();
        let text = serde_json::to_string_pretty(&record).expect("a record serializes");
        std::fs::write(dir.path().join("routing.json"), &text).expect("the record writes");
        std::fs::write(dir.path().join("notes.txt"), "not a record").expect("the note writes");
        let loaded = Record::load_dir(dir.path()).expect("the directory reads");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].1, record);

        std::fs::write(dir.path().join("broken.json"), "{").expect("the broken file writes");
        let error = Record::load_dir(dir.path()).expect_err("a record that does not parse is an error");
        assert!(error.contains("broken.json"), "the error names the file: {error}");
    }

    #[test]
    fn metrics_convert_to_the_shape_a_gate_judges() {
        let metrics = score(&observations(&[(0.9, true), (0.9, false)]));
        let scores = metrics.scores();
        assert_eq!(scores.items, 2);
        assert_eq!(scores.accuracy, Some(0.5));
        assert_eq!(scores.confident_errors, Some(1));
        assert_eq!(scores.nll, Some(metrics.nll));
    }
}
