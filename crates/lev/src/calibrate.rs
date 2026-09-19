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

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// One observation: what the estimator reported for the winning option, and
/// whether that option turned out to be right.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Observation {
    /// The raw frequency the winning option carried.
    pub raw: f64,
    /// Whether the winning option was the labelled answer.
    pub correct: bool,
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
        Self { bins: built, base_rate, fitted_on: observations.len() }
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

    /// Rescales a whole distribution so the winner carries its calibrated
    /// probability and the rest share what is left, in their observed
    /// proportions.
    #[must_use]
    pub fn apply_distribution(&self, raw: &IndexMap<String, f64>) -> IndexMap<String, f64> {
        let Some((winner, top)) = raw.iter().max_by(|a, b| a.1.total_cmp(b.1)) else {
            return raw.clone();
        };
        let winner = winner.clone();
        let calibrated = self.apply(*top).clamp(0.0, 1.0);
        let rest: f64 = raw.iter().filter(|(k, _)| **k != winner).map(|(_, v)| *v).sum();
        let remaining = 1.0 - calibrated;
        raw.iter()
            .map(|(key, value)| {
                if *key == winner {
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

fn english() -> String {
    "en".to_string()
}

/// What a calibrated question family carries.
///
/// A family without one of these does not serve probabilities, and a record
/// whose base signature no longer matches the serving runtime does not
/// either.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Record {
    /// The question family this map covers.
    pub family: String,
    /// Which estimator produced the raw signal.
    pub estimator: String,
    /// How many samples an L2 estimate drew.
    pub samples: u64,
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
    /// The operating system build the runtime reported.
    pub os_build: String,
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

impl Record {
    /// Whether this record may be served against the given host.
    #[must_use]
    pub fn valid_for(&self, os_build: &str) -> bool {
        self.admitted && self.os_build == os_build
    }
}

/// Decides whether a fitted map earned the right to serve.
///
/// A map is not admitted because it exists. It is admitted because it beat
/// the raw signal on items it was not fitted on.
///
/// The three conditions, and why each one:
///
/// - **ECE falls by at least a tenth.** Calibration is what a calibration
///   map is for, and ECE measures it directly. A marginal move is binning
///   noise, not an improvement.
/// - **NLL does not rise.** Log loss is strictly proper and punishes
///   confident errors hardest, so a map that buys calibration by hedging
///   everything into the middle fails here.
/// - **Brier rises by no more than a tenth.** Brier is calibration and
///   refinement together. A binned map cannot improve refinement — it is
///   monotone in the raw signal and leaves the argmax alone — so it can only
///   lose a little to binning. Requiring no loss at all rejects maps that cut
///   ECE several-fold, which is the wrong trade; requiring the loss stay
///   small keeps a map from destroying sharpness to flatter its ECE.
///
/// The Brier tolerance was widened from zero after the first run on a
/// 196-item suite, where maps cutting ECE from 0.157 to 0.005 were refused
/// over a Brier move of 0.02. The test below pins that case so the reasoning
/// does not get lost.
#[must_use]
pub fn admit(raw: Metrics, calibrated: Metrics, fitted_on: usize) -> (bool, String) {
    const MIN_FITTED: usize = 8;
    /// The relative ECE reduction a map has to earn.
    const ECE_MARGIN: f64 = 0.10;
    /// How much Brier may degrade to buy that reduction.
    const BRIER_TOLERANCE: f64 = 0.10;

    if fitted_on < MIN_FITTED {
        return (
            false,
            format!("refused: fitted on {fitted_on} items, below the floor of {MIN_FITTED}"),
        );
    }
    if calibrated.items < MIN_FITTED {
        return (
            false,
            format!("refused: scored on {} items, too few to judge", calibrated.items),
        );
    }
    if calibrated.ece > raw.ece * (1.0 - ECE_MARGIN) {
        return (
            false,
            format!(
                "refused: ECE {:.3} to {:.3}, short of the {:.0}% reduction a map has to earn",
                raw.ece,
                calibrated.ece,
                ECE_MARGIN * 100.0
            ),
        );
    }
    if calibrated.nll > raw.nll {
        return (
            false,
            format!(
                "refused: ECE improved {:.3} to {:.3} but log loss rose {:.3} to {:.3}, so the \
                 map bought calibration by hedging",
                raw.ece, calibrated.ece, raw.nll, calibrated.nll
            ),
        );
    }
    if calibrated.brier > raw.brier * (1.0 + BRIER_TOLERANCE) {
        return (
            false,
            format!(
                "refused: ECE improved {:.3} to {:.3} but Brier rose {:.3} to {:.3}, past the \
                 {:.0}% the binning is allowed to cost",
                raw.ece,
                calibrated.ece,
                raw.brier,
                calibrated.brier,
                BRIER_TOLERANCE * 100.0
            ),
        );
    }
    (
        true,
        format!(
            "admitted: ECE {:.3} to {:.3}, log loss {:.3} to {:.3}, Brier {:.3} to {:.3} on \
             held-out items",
            raw.ece, calibrated.ece, raw.nll, calibrated.nll, raw.brier, calibrated.brier
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observations(pairs: &[(f64, bool)]) -> Vec<Observation> {
        pairs.iter().map(|(raw, correct)| Observation { raw: *raw, correct: *correct }).collect()
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
    fn a_map_that_makes_calibration_worse_is_not_admitted() {
        let raw = Metrics { ece: 0.031, brier: 0.012, nll: 0.1, items: 12, ..Metrics::default() };
        let worse = Metrics { ece: 0.113, brier: 0.013, nll: 0.1, items: 12, ..Metrics::default() };
        let (admitted, why) = admit(raw, worse, 12);
        assert!(!admitted, "{why}");
        assert!(why.contains("short of"));
    }

    #[test]
    fn a_map_that_helps_on_held_out_items_is_admitted() {
        let raw = Metrics { ece: 0.219, brier: 0.215, nll: 0.6, items: 12, ..Metrics::default() };
        let better = Metrics { ece: 0.132, brier: 0.184, nll: 0.5, items: 12, ..Metrics::default() };
        let (admitted, why) = admit(raw, better, 12);
        assert!(admitted, "{why}");
    }

    #[test]
    fn a_large_calibration_gain_survives_a_small_brier_cost() {
        // The case that widened the tolerance: measured on kev's urgency
        // family, 196-item suite. A zero-tolerance gate refused this.
        let raw = Metrics { ece: 0.157, brier: 0.199, nll: 0.6, items: 30, ..Metrics::default() };
        let mapped = Metrics { ece: 0.005, brier: 0.210, nll: 0.58, items: 30, ..Metrics::default() };
        let (admitted, why) = admit(raw, mapped, 30);
        assert!(admitted, "{why}");
    }

    #[test]
    fn a_map_that_hedges_everything_into_the_middle_is_refused() {
        // Flat probabilities score a fine ECE and a terrible log loss. That
        // is the failure the NLL condition exists to catch.
        let raw = Metrics { ece: 0.200, brier: 0.150, nll: 0.40, items: 30, ..Metrics::default() };
        let hedged = Metrics { ece: 0.010, brier: 0.155, nll: 0.90, items: 30, ..Metrics::default() };
        let (admitted, why) = admit(raw, hedged, 30);
        assert!(!admitted, "{why}");
        assert!(why.contains("hedging"));
    }

    #[test]
    fn a_map_that_destroys_sharpness_is_refused() {
        let raw = Metrics { ece: 0.200, brier: 0.100, nll: 0.40, items: 30, ..Metrics::default() };
        let blunt = Metrics { ece: 0.010, brier: 0.180, nll: 0.39, items: 30, ..Metrics::default() };
        let (admitted, why) = admit(raw, blunt, 30);
        assert!(!admitted, "{why}");
        assert!(why.contains("Brier rose"));
    }

    #[test]
    fn a_map_fitted_on_too_little_is_refused_whatever_it_scores() {
        let raw = Metrics { ece: 0.5, brier: 0.5, items: 12, ..Metrics::default() };
        let better = Metrics { ece: 0.0, brier: 0.0, items: 12, ..Metrics::default() };
        let (admitted, why) = admit(raw, better, 4);
        assert!(!admitted, "{why}");
        assert!(why.contains("below the floor"));
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
    fn a_record_refuses_a_host_it_was_not_fitted_on() {
        let record = Record {
            family: "routing".to_string(),
            language: english(),
            estimator: "l2".to_string(),
            samples: 8,
            suite: "support-v1".to_string(),
            suite_digest: "abc".to_string(),
            os_build: "25E246".to_string(),
            fitted: "2026-09-19".to_string(),
            map: Map::fit(&observations(&[(1.0, true)]), 2),
            raw_metrics: Metrics::default(),
            calibrated_metrics: Metrics::default(),
            admitted: true,
            verdict: "admitted: test".to_string(),
        };
        assert!(record.valid_for("25E246"));
        assert!(!record.valid_for("25F100"));
    }
}
