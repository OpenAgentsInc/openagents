//! How much a calibration metric moves when only the seed block moves.
//!
//! `docs/lev/measurements/2026-09-19-seed-variance.md` measured that for
//! accuracy: eight disjoint seed blocks over one unchanged door and one
//! unchanged item set gave a standard deviation of 0.0197. [`crate::ab`]
//! turns that number into the bar a win has to clear. ECE, Brier, and log
//! loss had no such number, so [`crate::ab::Rule`] recorded them as
//! [`crate::gate::Basis::Unmeasured`] and a win on any of them could never
//! earn a keep.
//!
//! This module computes those spreads from recorded draws. It asks no door:
//! `lev-calibration-sweep` writes one [`Draw`] per item per block, and
//! everything here is arithmetic over that file, so a mistake in the
//! analysis costs a recompile rather than another two hours of device time.
//!
//! # The wrinkle, and what is done about it
//!
//! Accuracy, Brier, and log loss are means over items. Resampling seeds
//! changes each item's contribution, the mean moves, and the spread of that
//! mean is exactly what a gate wants.
//!
//! **ECE is not a mean over items.** It is computed inside ten fixed bins:
//! each bin contributes the gap between its mean reported probability and
//! its share correct, weighted by how many items landed in it. So changing
//! the seed block moves an ECE two ways at once. The values move, which is
//! the same effect the other metrics see. And items cross bin edges, which
//! regroups the terms — and because each bin takes an absolute value,
//! regrouping changes how much the errors inside a bin cancel. A spread
//! taken over blocks mixes the two.
//!
//! Both are measured here, and they are named differently.
//!
//! - **Live ECE** is [`crate::calibrate::score`]'s: each block bins its own
//!   values. This is what a door reports, what every published number in
//!   `docs/lev/` is, and what a gate will compare. Its spread is the floor.
//! - **Frozen ECE** is [`ece_frozen`]: every item is assigned the bin its
//!   *mean* probability over all blocks falls in, and that membership is
//!   held fixed while each block's own values and outcomes are scored inside
//!   it. No block is privileged, and nothing regroups. Its spread is the
//!   part of the movement that is the values alone.
//!
//! The difference between the two spreads is what regrouping adds. Reporting
//! only the live spread would not be wrong, but it would not be readable:
//! nobody could tell whether an ECE floor is measuring the door or the
//! binning. Reporting only the frozen spread would understate the floor,
//! because a real comparison does rebin.
//!
//! # Why the floor is the block spread and not the bootstrap
//!
//! A bootstrap over items answers a different question: how much would this
//! number move on a different sample of items. That is the larger half —
//! the binomial standard error of an accuracy near 0.78 on 98 items is about
//! 0.042, roughly twice the seed noise — and [`stratified_bootstrap`]
//! reports it, resampling inside each family so the suite's family mix is
//! held fixed.
//!
//! It is not the floor a two-door comparison should use, because both doors
//! answer the *same* 98 items. The item draw is common to both sides and
//! mostly cancels in the difference; the seed draw does not. So the gate
//! takes the block spread, and the bootstrap interval says how far the
//! resulting statement generalizes.
//!
//! The clustered bootstrap that `openjev-sglang` uses over BoolQ passages
//! has no analogue here. Its clusters are items sharing a passage, of which
//! there are many. The nearest thing in `support-v2` is the family, of which
//! there are three, and a bootstrap over three clusters has no resolution.
//! Every one of the 196 items carries a distinct state, so the family is a
//! stratum rather than a cluster, and that is how it is used.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::calibrate::Observation;

/// The schema tag `lev-calibration-sweep` writes on every draw.
pub const DRAW_SCHEMA: &str = "openagents.gym.block_draw.v1";

/// How many bins [`crate::calibrate::score`] computes an ECE over.
const ECE_BINS: usize = 10;

/// One item, answered once, from one seed block.
///
/// The door reproduces exactly from a seed, so the block is part of the
/// identity of the draw: block 0 and block 3 over the same item are two
/// trials, and rerunning block 0 is not.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Draw {
    /// The schema tag.
    pub schema: String,
    /// The suite the item came from.
    pub suite: String,
    /// The door that answered.
    pub door: String,
    /// The adapter identifier, when the door served through one.
    pub adapter: Option<String>,
    /// The base model signature the door reported.
    pub base_signature: String,
    /// The suite split the item belongs to.
    pub split: String,
    /// The question family.
    pub family: String,
    /// The item id.
    pub item: String,
    /// The seed block the estimate drew.
    pub block: u64,
    /// How many draws the estimate rests on.
    pub samples: u64,
    /// The frequency the winning option carried.
    pub top: f64,
    /// The option that won.
    pub choice: String,
    /// The labelled answer.
    pub truth: String,
    /// Whether the winning option was the labelled answer.
    pub correct: bool,
    /// The certainty band the door selected, when it reported one.
    pub band: Option<String>,
    /// Draws the runtime refused.
    pub refused: u64,
}

impl Draw {
    /// The draw as an observation a reliability table can be fitted on.
    #[must_use]
    pub fn observation(&self) -> Observation {
        match &self.band {
            Some(band) => Observation::banded(self.top, self.correct, band.clone()),
            None => Observation::new(self.top, self.correct),
        }
    }
}

/// Why a set of draws cannot be reported on.
///
/// A mean over the rows that happen to be present is a different measurement
/// from a mean over the rows that were asked for, and the two must not be
/// confused. Every one of these names what is missing.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Incomplete {
    /// A line was not a draw.
    #[error("line {line}: {problem}")]
    Unreadable {
        /// Which line.
        line: usize,
        /// What went wrong.
        problem: String,
    },
    /// A line carried another schema.
    #[error("line {line} is tagged {found}, not {DRAW_SCHEMA}")]
    Schema {
        /// Which line.
        line: usize,
        /// The tag the line carried.
        found: String,
    },
    /// The file holds nothing for this door and split.
    #[error("no draws for door {door} on the {split} split")]
    Empty {
        /// The door asked for.
        door: String,
        /// The split asked for.
        split: String,
    },
    /// A block is missing items another block has.
    #[error(
        "door {door}, {split} split: block {block} holds {held} of the {expected} items the \
         other blocks hold, and the first missing one is {first}"
    )]
    Ragged {
        /// The door.
        door: String,
        /// The split.
        split: String,
        /// The thin block.
        block: u64,
        /// How many items it holds.
        held: usize,
        /// How many the union holds.
        expected: usize,
        /// The first item it does not hold.
        first: String,
    },
    /// One block is not a spread.
    #[error(
        "door {door}, {split} split: {blocks} seed block(s); a spread needs at least two, and \
         rerunning one block is not a second trial"
    )]
    OneBlock {
        /// The door.
        door: String,
        /// The split.
        split: String,
        /// How many blocks are present.
        blocks: usize,
    },
}

/// Every draw a sweep recorded.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Draws {
    rows: Vec<Draw>,
}

impl Draws {
    /// Reads a draws file, refusing a line that is not a draw.
    ///
    /// # Errors
    ///
    /// Returns the first line that does not parse or carries another schema.
    pub fn parse(text: &str) -> Result<Self, Incomplete> {
        let mut rows = Vec::new();
        for (index, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let draw: Draw =
                serde_json::from_str(line).map_err(|error| Incomplete::Unreadable {
                    line: index + 1,
                    problem: error.to_string(),
                })?;
            if draw.schema != DRAW_SCHEMA {
                return Err(Incomplete::Schema {
                    line: index + 1,
                    found: draw.schema,
                });
            }
            rows.push(draw);
        }
        Ok(Self { rows })
    }

    /// Every draw, in the order the file held them.
    #[must_use]
    pub fn rows(&self) -> &[Draw] {
        &self.rows
    }

    /// The doors that answered, in the order they first appear.
    #[must_use]
    pub fn doors(&self) -> Vec<String> {
        let mut seen = Vec::new();
        for row in &self.rows {
            if !seen.contains(&row.door) {
                seen.push(row.door.clone());
            }
        }
        seen
    }

    /// The families present, in the order they first appear.
    #[must_use]
    pub fn families(&self) -> Vec<String> {
        let mut seen = Vec::new();
        for row in &self.rows {
            if !seen.contains(&row.family) {
                seen.push(row.family.clone());
            }
        }
        seen
    }

    /// The seed blocks one door drew on one split.
    #[must_use]
    pub fn blocks(&self, door: &str, split: &str) -> Vec<u64> {
        let blocks: BTreeSet<u64> = self
            .rows
            .iter()
            .filter(|row| row.door == door && row.split == split)
            .map(|row| row.block)
            .collect();
        blocks.into_iter().collect()
    }

    /// Refuses a split that cannot carry a spread.
    ///
    /// One block is not a trial repeated: these doors reproduce a block
    /// exactly, so rerunning it returns the same arithmetic. A split that is
    /// only ever fitted on, rather than reported over, does not need this.
    ///
    /// # Errors
    ///
    /// Names the door, the split, and how many blocks it holds.
    pub fn is_a_spread(&self, door: &str, split: &str) -> Result<(), Incomplete> {
        let blocks = self.blocks(door, split).len();
        if blocks < 2 {
            return Err(Incomplete::OneBlock {
                door: door.to_string(),
                split: split.to_string(),
                blocks,
            });
        }
        Ok(())
    }

    /// Refuses a grid with holes in it.
    ///
    /// # Errors
    ///
    /// Names the block that is thin and the first item it is missing.
    pub fn complete(&self, door: &str, split: &str) -> Result<(), Incomplete> {
        let mine: Vec<&Draw> = self
            .rows
            .iter()
            .filter(|row| row.door == door && row.split == split)
            .collect();
        if mine.is_empty() {
            return Err(Incomplete::Empty {
                door: door.to_string(),
                split: split.to_string(),
            });
        }
        let items: BTreeSet<&str> = mine.iter().map(|row| row.item.as_str()).collect();
        let mut by_block: BTreeMap<u64, BTreeSet<&str>> = BTreeMap::new();
        for row in &mine {
            by_block
                .entry(row.block)
                .or_default()
                .insert(row.item.as_str());
        }
        for (block, held) in &by_block {
            if held.len() == items.len() {
                continue;
            }
            let first = items
                .iter()
                .find(|item| !held.contains(*item))
                .map_or_else(String::new, |item| (*item).to_string());
            return Err(Incomplete::Ragged {
                door: door.to_string(),
                split: split.to_string(),
                block: *block,
                held: held.len(),
                expected: items.len(),
                first,
            });
        }
        Ok(())
    }

    /// One block's observations, in item order so two blocks pair item by
    /// item.
    ///
    /// `family` of `None` takes every family.
    #[must_use]
    pub fn observations(
        &self,
        door: &str,
        split: &str,
        family: Option<&str>,
        block: u64,
    ) -> Vec<Observation> {
        self.draws_of(door, split, family, block)
            .iter()
            .map(|row| row.observation())
            .collect()
    }

    /// One block's draws, in item order.
    #[must_use]
    pub fn draws_of(
        &self,
        door: &str,
        split: &str,
        family: Option<&str>,
        block: u64,
    ) -> Vec<&Draw> {
        let mut mine: Vec<&Draw> = self
            .rows
            .iter()
            .filter(|row| {
                row.door == door
                    && row.split == split
                    && row.block == block
                    && family.is_none_or(|name| row.family == name)
            })
            .collect();
        mine.sort_by(|left, right| left.item.cmp(&right.item));
        mine
    }
}

/// A spread over blocks: what the number was, and how far it moved when only
/// the seeds moved.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Spread {
    /// The value per block, in block order.
    pub values: Vec<f64>,
    /// The mean over blocks.
    pub mean: f64,
    /// The sample standard deviation over blocks. These blocks are a sample
    /// of the blocks that could have been drawn, not all of them.
    pub sd: f64,
    /// The lowest block.
    pub low: f64,
    /// The highest block.
    pub high: f64,
}

impl Spread {
    /// The spread of a set of block values, or `None` when there are fewer
    /// than two: one block is not a trial repeated.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn over(values: &[f64]) -> Option<Self> {
        if values.len() < 2 {
            return None;
        }
        let n = values.len() as f64;
        let mean = values.iter().sum::<f64>() / n;
        let variance = values
            .iter()
            .map(|value| (value - mean).powi(2))
            .sum::<f64>()
            / (n - 1.0);
        Some(Self {
            values: values.to_vec(),
            mean,
            sd: variance.sqrt(),
            low: values.iter().copied().fold(f64::INFINITY, f64::min),
            high: values.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        })
    }

    /// The distance between the lowest block and the highest.
    #[must_use]
    pub fn range(&self) -> f64 {
        self.high - self.low
    }

    /// The smallest difference two doors could show, at `sigmas` standard
    /// deviations, with `control` and `candidate` blocks behind them.
    ///
    /// A comparison carries both sides' noise: `sd * sqrt(1/b + 1/c)`.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn detectable(&self, sigmas: f64, control: usize, candidate: usize) -> Option<f64> {
        if control == 0 || candidate == 0 {
            return None;
        }
        let paired = (1.0 / control as f64 + 1.0 / candidate as f64).sqrt();
        Some(sigmas * self.sd * paired)
    }
}

/// Which bin of a ten-bin table a signal falls in.
///
/// This walks the same edges [`crate::calibrate::score`] walks rather than
/// multiplying and flooring, because the two disagree: `0.3 * 10.0` is
/// 2.9999999999999996 in binary floating point, so flooring puts 0.3 in the
/// bin below the one a scan puts it in. A frozen ECE that binned differently
/// from the live one would be measuring the arithmetic.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn bin_of(signal: f64) -> usize {
    for index in 0..ECE_BINS {
        let lo = index as f64 / ECE_BINS as f64;
        let hi = (index + 1) as f64 / ECE_BINS as f64;
        if signal >= lo && (signal < hi || (index + 1 == ECE_BINS && signal <= hi)) {
            return index;
        }
    }
    ECE_BINS - 1
}

/// ECE with bin membership held fixed.
///
/// `reference[i]` decides which bin item `i` belongs to; `observations[i]`
/// supplies the probability and the outcome scored inside it. Pass each
/// item's mean signal over every block as the reference and no block is
/// privileged, so what is left is the movement of the values alone.
///
/// Returns 0 when the two slices are different lengths, which is not a
/// number anyone should act on and is why [`Draws::complete`] is checked
/// first.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn ece_frozen(reference: &[f64], observations: &[Observation]) -> f64 {
    if reference.len() != observations.len() || observations.is_empty() {
        return 0.0;
    }
    let n = observations.len() as f64;
    let mut ece = 0.0;
    for bin in 0..ECE_BINS {
        let inside: Vec<&Observation> = observations
            .iter()
            .enumerate()
            .filter(|(index, _)| bin_of(reference[*index]) == bin)
            .map(|(_, observation)| observation)
            .collect();
        if inside.is_empty() {
            continue;
        }
        let share = inside.len() as f64 / n;
        let mean_p = inside.iter().map(|o| o.raw).sum::<f64>() / inside.len() as f64;
        let mean_correct = inside.iter().filter(|o| o.correct).count() as f64 / inside.len() as f64;
        ece += share * (mean_p - mean_correct).abs();
    }
    ece
}

/// The mean signal each item carried across blocks, in item order.
///
/// This is the reference [`ece_frozen`] takes. `blocks` is one slice per
/// block, each already in item order.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn mean_signal(blocks: &[Vec<Observation>]) -> Vec<f64> {
    let Some(first) = blocks.first() else {
        return Vec::new();
    };
    let mut means = Vec::with_capacity(first.len());
    for index in 0..first.len() {
        let mut total = 0.0;
        let mut seen = 0.0;
        for block in blocks {
            if let Some(observation) = block.get(index) {
                total += observation.raw;
                seen += 1.0;
            }
        }
        means.push(if seen > 0.0 { total / seen } else { 0.0 });
    }
    means
}

/// A two-sided interval from a resampling.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Interval {
    /// The 2.5th percentile.
    pub low: f64,
    /// The 97.5th percentile.
    pub high: f64,
}

impl Interval {
    /// How wide it is.
    #[must_use]
    pub fn width(&self) -> f64 {
        self.high - self.low
    }
}

/// A small deterministic generator, so a resampling reproduces.
///
/// The repository's estimators are seeded and replayable, and an interval
/// that moves every time it is printed is not a measurement. This is
/// SplitMix64, which is short enough to read and good enough to resample
/// with.
struct SplitMix64(u64);

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, bound: usize) -> usize {
        if bound == 0 {
            return 0;
        }
        (self.next() % bound as u64) as usize
    }
}

/// Resamples items inside each stratum and reports where the statistic
/// landed.
///
/// The suite fixes its family mix — 50 routing, 30 urgency, 18 severity —
/// so resampling across families would report an interval for a suite this
/// one is not. Each stratum is resampled to its own size with replacement,
/// the strata are concatenated in order, and the statistic is computed on
/// the whole.
///
/// The statistic takes references, so a paired quantity — the same items
/// scored twice, raw and through a map — resamples once and is differenced
/// inside, which is what makes the interval an interval on the difference.
#[must_use]
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
pub fn stratified_bootstrap<T>(
    strata: &[Vec<T>],
    statistic: impl Fn(&[&T]) -> f64,
    resamples: usize,
    seed: u64,
) -> Option<Interval> {
    if strata.iter().all(Vec::is_empty) || resamples < 2 {
        return None;
    }
    let mut rng = SplitMix64(seed);
    let mut seen = Vec::with_capacity(resamples);
    for _ in 0..resamples {
        let mut sample: Vec<&T> = Vec::new();
        for stratum in strata {
            for _ in 0..stratum.len() {
                sample.push(&stratum[rng.below(stratum.len())]);
            }
        }
        seen.push(statistic(&sample));
    }
    seen.sort_by(f64::total_cmp);
    let low = seen[((seen.len() as f64) * 0.025).floor() as usize];
    let high = seen[(((seen.len() as f64) * 0.975).ceil() as usize).min(seen.len() - 1)];
    Some(Interval { low, high })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calibrate::score;

    fn draw(
        door: &str,
        split: &str,
        family: &str,
        item: &str,
        block: u64,
        top: f64,
        correct: bool,
    ) -> Draw {
        Draw {
            schema: DRAW_SCHEMA.to_string(),
            suite: "support-v2".to_string(),
            door: door.to_string(),
            adapter: None,
            base_signature: "9799725".to_string(),
            split: split.to_string(),
            family: family.to_string(),
            item: item.to_string(),
            block,
            samples: 8,
            top,
            choice: "a".to_string(),
            truth: if correct { "a" } else { "b" }.to_string(),
            correct,
            band: None,
            refused: 0,
        }
    }

    fn line(draw: &Draw) -> String {
        serde_json::to_string(draw).expect("a draw serializes")
    }

    #[test]
    fn a_line_that_is_not_a_draw_is_refused_by_line_number() {
        let text = format!(
            "{}\nnot json\n",
            line(&draw(
                "lev-base",
                "evaluation",
                "routing",
                "routing/001",
                0,
                1.0,
                true
            ))
        );
        let problem = Draws::parse(&text).expect_err("the second line is not a draw");
        assert!(
            matches!(problem, Incomplete::Unreadable { line: 2, .. }),
            "the refusal names the line: {problem}"
        );
    }

    #[test]
    fn another_schema_is_refused_rather_than_read_as_a_draw() {
        let mut row = draw(
            "lev-base",
            "evaluation",
            "routing",
            "routing/001",
            0,
            1.0,
            true,
        );
        row.schema = "openagents.gym.eval_row.v1".to_string();
        let problem = Draws::parse(&line(&row)).expect_err("the tag is wrong");
        assert!(
            matches!(problem, Incomplete::Schema { line: 1, .. }),
            "the refusal names the tag it found: {problem}"
        );
    }

    #[test]
    fn a_block_missing_an_item_is_named_rather_than_averaged_over() {
        let rows = [
            draw(
                "lev-base",
                "evaluation",
                "routing",
                "routing/001",
                0,
                1.0,
                true,
            ),
            draw(
                "lev-base",
                "evaluation",
                "routing",
                "routing/002",
                0,
                0.5,
                false,
            ),
            draw(
                "lev-base",
                "evaluation",
                "routing",
                "routing/001",
                1,
                0.875,
                true,
            ),
        ];
        let text: String = rows.iter().map(|row| format!("{}\n", line(row))).collect();
        let draws = Draws::parse(&text).expect("the draws parse");
        let problem = draws
            .complete("lev-base", "evaluation")
            .expect_err("block 1 is a row short");
        match problem {
            Incomplete::Ragged {
                block,
                held,
                expected,
                first,
                ..
            } => {
                assert_eq!(block, 1);
                assert_eq!((held, expected), (1, 2));
                assert_eq!(first, "routing/002", "the missing item is named");
            }
            other => panic!("the refusal should name the thin block, got {other}"),
        }
    }

    #[test]
    fn one_block_is_not_a_spread_but_is_a_complete_grid() {
        let rows = [
            draw(
                "lev-base",
                "evaluation",
                "routing",
                "routing/001",
                0,
                1.0,
                true,
            ),
            draw(
                "lev-base",
                "evaluation",
                "routing",
                "routing/002",
                0,
                0.5,
                false,
            ),
        ];
        let text: String = rows.iter().map(|row| format!("{}\n", line(row))).collect();
        let draws = Draws::parse(&text).expect("the draws parse");
        draws
            .complete("lev-base", "evaluation")
            .expect("one block holding every item has no holes in it");
        assert!(
            matches!(
                draws.is_a_spread("lev-base", "evaluation"),
                Err(Incomplete::OneBlock { blocks: 1, .. })
            ),
            "rerunning one block is not a second trial"
        );
    }

    #[test]
    fn observations_come_back_in_item_order_so_two_blocks_pair() {
        let rows = [
            draw(
                "lev-base",
                "evaluation",
                "routing",
                "routing/002",
                0,
                0.5,
                false,
            ),
            draw(
                "lev-base",
                "evaluation",
                "routing",
                "routing/001",
                0,
                1.0,
                true,
            ),
        ];
        let text: String = rows.iter().map(|row| format!("{}\n", line(row))).collect();
        let draws = Draws::parse(&text).expect("the draws parse");
        let observations = draws.observations("lev-base", "evaluation", None, 0);
        assert_eq!(observations[0].raw, 1.0, "routing/001 sorts first");
        assert_eq!(observations[1].raw, 0.5);
    }

    #[test]
    fn a_spread_needs_two_blocks() {
        assert!(Spread::over(&[0.5]).is_none());
        let spread = Spread::over(&[0.745, 0.796]).expect("two blocks are a spread");
        assert!((spread.mean - 0.7705).abs() < 1e-9);
        assert!((spread.range() - 0.051).abs() < 1e-9);
    }

    #[test]
    fn the_detectable_difference_falls_as_blocks_are_added() {
        let spread = Spread::over(&[0.0, 0.0394]).expect("two blocks");
        let one = spread.detectable(2.0, 1, 1).expect("one block a side");
        let four = spread.detectable(2.0, 4, 4).expect("four blocks a side");
        assert!(
            (one / four - 2.0).abs() < 1e-9,
            "four blocks a side halves it"
        );
    }

    #[test]
    fn frozen_ece_equals_live_ece_when_the_reference_is_the_block_itself() {
        let observations = vec![
            Observation::new(0.95, true),
            Observation::new(0.92, false),
            Observation::new(0.55, true),
            Observation::new(0.12, false),
        ];
        let reference: Vec<f64> = observations.iter().map(|o| o.raw).collect();
        let frozen = ece_frozen(&reference, &observations);
        assert!(
            (frozen - score(&observations).ece).abs() < 1e-12,
            "binning a block by its own values is the live computation"
        );
    }

    #[test]
    fn frozen_ece_holds_membership_when_an_item_crosses_an_edge() {
        // One item moves from 0.55 to 0.45, crossing the edge at 0.5. Live
        // scoring regroups it; frozen scoring does not.
        let before = [Observation::new(0.55, true), Observation::new(0.55, false)];
        let after = vec![Observation::new(0.45, true), Observation::new(0.55, false)];
        let reference: Vec<f64> = before.iter().map(|o| o.raw).collect();
        let frozen = ece_frozen(&reference, &after);
        let live = score(&after).ece;
        assert!(
            (frozen - live).abs() > 1e-6,
            "the two computations differ exactly when an item changes bin: frozen {frozen}, \
             live {live}"
        );
    }

    #[test]
    fn a_suite_ece_is_not_the_item_weighted_mean_of_its_families() {
        // Two families of two items each, every item reported at 0.75. One
        // family is right half the time and one is right always, so each
        // carries an ECE of 0.25 and the item-weighted mean of the two is
        // 0.25. Scored in one table the four items are right three times in
        // four, the bin's gap closes, and the suite ECE is zero.
        //
        // This is why `ab::pool` leaves a pooled ECE unknown, and why the
        // floor the rule takes for ECE is a per-family number.
        let left = [Observation::new(0.75, true), Observation::new(0.75, false)];
        let right = [Observation::new(0.75, true), Observation::new(0.75, true)];
        assert!((score(&left).ece - 0.25).abs() < 1e-12);
        assert!((score(&right).ece - 0.25).abs() < 1e-12);
        let together: Vec<Observation> = left.iter().chain(right.iter()).cloned().collect();
        assert!(
            score(&together).ece.abs() < 1e-12,
            "the families' errors cancel inside the shared bin: {}",
            score(&together).ece
        );
    }

    #[test]
    fn the_mean_signal_is_taken_item_by_item_across_blocks() {
        let blocks = vec![
            vec![Observation::new(1.0, true), Observation::new(0.5, false)],
            vec![Observation::new(0.5, true), Observation::new(0.5, false)],
        ];
        assert_eq!(mean_signal(&blocks), vec![0.75, 0.5]);
    }

    #[test]
    fn the_bootstrap_reproduces_from_its_seed() {
        let strata = vec![
            (0..40).map(|n| f64::from(n) / 40.0).collect::<Vec<f64>>(),
            (0..20).map(|n| f64::from(n) / 20.0).collect::<Vec<f64>>(),
        ];
        #[allow(clippy::cast_precision_loss)]
        let mean = |sample: &[&f64]| sample.iter().copied().sum::<f64>() / sample.len() as f64;
        let first = stratified_bootstrap(&strata, mean, 200, 7).expect("an interval");
        let again = stratified_bootstrap(&strata, mean, 200, 7).expect("an interval");
        assert_eq!(first, again, "the same seed gives the same interval");
        assert!(first.low < first.high);
    }

    #[test]
    fn the_bootstrap_keeps_each_stratum_at_its_own_size() {
        // One stratum is all zeros and twice the size of the other, which is
        // all ones. Any resample that respected the sizes has a mean near
        // one third; one that pooled them would not.
        let strata = vec![vec![0.0_f64; 40], vec![1.0_f64; 20]];
        #[allow(clippy::cast_precision_loss)]
        let mean = |sample: &[&f64]| sample.iter().copied().sum::<f64>() / sample.len() as f64;
        let interval = stratified_bootstrap(&strata, mean, 200, 11).expect("an interval");
        assert!(
            (interval.low - 1.0 / 3.0).abs() < 0.001 && (interval.high - 1.0 / 3.0).abs() < 0.001,
            "the family mix is held fixed, so the mean cannot move: {interval:?}"
        );
    }

    #[test]
    fn a_bin_edge_lands_in_the_bin_it_opens() {
        assert_eq!(bin_of(0.0), 0);
        assert_eq!(bin_of(0.1), 1);
        assert_eq!(bin_of(0.95), 9);
        assert_eq!(bin_of(1.0), 9, "the top bin is closed at one");
    }
}
