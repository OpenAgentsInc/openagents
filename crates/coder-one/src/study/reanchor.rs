//! Fitting decision settings on recorded answers, after DSPy's ReAnchor.
//!
//! A decision setting ([`jev::decision`]) turns the probabilities Jev
//! returned into what code does: a Noul threshold, a Score's cuts, or a
//! Choice's weights. None of them is sent, so every recorded answer stays
//! valid when a setting moves, and a setting can be refitted on recorded
//! answers alone, with no Jev call.
//!
//! The fit, for each setting in name order and each of its coordinates:
//!
//! 1. **Candidates.** For a threshold, the midpoints between neighboring
//!    observed probabilities, with 0 and 1 as the outer neighbors. For a
//!    cut, the midpoints between neighboring observed probability-weighted
//!    means that lie between the cut's neighbors. For a Choice weight, the
//!    points where an example's pick flips to or from the option, searched
//!    on a log scale from 1/16 to 16, with the midpoints taken in log space.
//!    At most [`CAP`] candidates per coordinate, evenly spaced by rank.
//! 2. **Training.** Every candidate is scored on the training examples with
//!    the other settings held where they are. The best score wins; a tie
//!    goes to the candidate in the widest gap between observed values, then
//!    to the lowest value.
//! 3. **Acceptance.** The winner replaces the current value only when it
//!    scores strictly better on the training examples **and** passes the
//!    held-out check: the training groups are split into [`FOLDS`] folds by
//!    a digest of each group's name, the whole selection is rerun on four
//!    folds and the chosen value predicts the fifth, and the pooled
//!    held-out predictions must score strictly better than the current
//!    value's on the same examples. Otherwise the current value stays.
//!
//! A fold holds whole groups, so the examples of one trial or task never sit
//! on both sides of a fold. Everything is deterministic: the same examples
//! give the same settings, byte for byte.
//!
//! Reimplemented from the algorithm DSPy 3.4.0 calls ReAnchor
//! (stanfordnlp/dspy#10475, MIT); no source copied.

use std::collections::{BTreeMap, BTreeSet};

use jev::{ChoiceAnswer, Cuts, Decision, ScoreAnswer, Threshold, Weights};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

/// The most candidates one coordinate is searched over.
pub const CAP: usize = 40;

/// The folds of the held-out check.
pub const FOLDS: usize = 5;

/// The widest a Choice weight is searched: from `1 / WEIGHT_SPAN` to
/// `WEIGHT_SPAN`.
pub const WEIGHT_SPAN: f64 = 16.0;

/// The salt a fold assignment digests each group's name with.
pub const FOLD_SALT: &str = "openagents.decision-fit.fold:";

/// The prediction a Noul setting makes, as a label: `true` or `false`.
#[must_use]
pub fn yes_no(yes: bool) -> String {
    if yes { "true" } else { "false" }.to_string()
}

// ---------------------------------------------------------------------------
// Examples and settings
// ---------------------------------------------------------------------------

/// The answer one setting reads.
#[derive(Debug, Clone, PartialEq)]
pub enum Answer {
    /// A Noul's probability of yes.
    Noul(f64),
    /// A Score answer.
    Score(ScoreAnswer),
    /// A Choice answer.
    Choice(ChoiceAnswer),
}

impl Answer {
    /// Reads an answer as Jev returns it: an object with a `type` of
    /// `noul`, `score`, or `choice`.
    ///
    /// # Errors
    ///
    /// Returns a sentence when the object is none of the three.
    pub fn from_json(value: &Value) -> Result<Self, String> {
        match value.get("type").and_then(Value::as_str) {
            Some("noul") => value
                .get("noul")
                .and_then(Value::as_f64)
                .filter(|p| p.is_finite())
                .map(Answer::Noul)
                .ok_or_else(|| "a Noul answer needs a finite `noul`".to_string()),
            Some("score") => {
                let mut value = value.clone();
                if value.get("legend").is_none() {
                    value["legend"] = json!({});
                }
                if value.get("confidence").is_none() {
                    value["confidence"] = json!(0.0);
                }
                serde_json::from_value(value)
                    .map(Answer::Score)
                    .map_err(|error| format!("a Score answer does not read: {error}"))
            }
            Some("choice") => {
                let mut value = value.clone();
                if value.get("confidence").is_none() {
                    value["confidence"] = json!(0.0);
                }
                serde_json::from_value(value)
                    .map(Answer::Choice)
                    .map_err(|error| format!("a Choice answer does not read: {error}"))
            }
            other => Err(format!(
                "an answer's type is noul, score, or choice, not {}",
                other.unwrap_or("missing")
            )),
        }
    }

    /// What `decision` decides on this answer, as a label: `true` or
    /// `false` for a Noul, the level for a Score, and the option for a
    /// Choice. `None` when a Score's level can't be read.
    #[must_use]
    pub fn decide(&self, decision: &Decision) -> Option<String> {
        match self {
            Answer::Noul(p) => Some(yes_no(decision.threshold_or(Threshold::MIDPOINT).yes(*p))),
            Answer::Score(answer) => decision.level(answer).map(|level| level.to_string()),
            Answer::Choice(answer) => Some(decision.choice(answer).to_string()),
        }
    }
}

/// One labeled example: the answers each setting reads, the outcome that
/// was right, and whatever else the component's rule needs.
#[derive(Debug, Clone)]
pub struct Example<C> {
    pub id: String,
    /// The unit a fold keeps whole, such as a trial.
    pub group: String,
    /// The unit an interval resamples, such as a task.
    pub task: String,
    /// The answer each setting reads, by setting name.
    pub answers: BTreeMap<String, Answer>,
    /// The right outcome, in the rule's labels.
    pub label: String,
    pub context: C,
}

/// Every setting's decision block, by setting name.
pub type Settings = BTreeMap<String, Decision>;

/// What a setting is, so its coordinates and candidates follow.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Kind {
    /// A Noul threshold.
    Threshold,
    /// A Score's cuts between these levels, in ascending order.
    Cuts { levels: Vec<u32> },
    /// A Choice's weights over these options.
    Weights { options: Vec<String> },
}

/// One setting to fit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spec {
    pub name: String,
    pub kind: Kind,
}

/// One searchable number of a setting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "coordinate", content = "at", rename_all = "snake_case")]
pub enum Coordinate {
    Threshold,
    /// The cut at this index.
    Cut(usize),
    /// The weight of this option.
    Weight(String),
}

impl Coordinate {
    fn label(&self) -> String {
        match self {
            Coordinate::Threshold => "threshold".to_string(),
            Coordinate::Cut(index) => format!("cuts[{index}]"),
            Coordinate::Weight(option) => format!("weights.{option}"),
        }
    }
}

impl Spec {
    /// The setting's coordinates, in search order.
    #[must_use]
    pub fn coordinates(&self) -> Vec<Coordinate> {
        match &self.kind {
            Kind::Threshold => vec![Coordinate::Threshold],
            Kind::Cuts { levels } => (0..levels.len().saturating_sub(1))
                .map(Coordinate::Cut)
                .collect(),
            Kind::Weights { options } => options.iter().cloned().map(Coordinate::Weight).collect(),
        }
    }
}

/// A component's rule: the outcome the settings decide on one example.
pub type Rule<'a, C> = &'a dyn Fn(&Settings, &Example<C>) -> String;

// ---------------------------------------------------------------------------
// Metrics
// ---------------------------------------------------------------------------

/// The counts a metric is computed from.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tally {
    pub examples: usize,
    pub correct: usize,
    /// Positive calls that were positive.
    pub true_positive: usize,
    /// Positive calls that were not.
    pub false_positive: usize,
    /// Positives not called.
    pub false_negative: usize,
}

impl Tally {
    /// Counts `(prediction, label)` pairs against `positive`.
    #[must_use]
    pub fn of(pairs: &[(String, String)], positive: &str) -> Self {
        let mut tally = Tally::default();
        for (prediction, label) in pairs {
            tally.examples += 1;
            if prediction == label {
                tally.correct += 1;
            }
            match (prediction == positive, label == positive) {
                (true, true) => tally.true_positive += 1,
                (true, false) => tally.false_positive += 1,
                (false, true) => tally.false_negative += 1,
                (false, false) => {}
            }
        }
        tally
    }

    /// Positive calls.
    #[must_use]
    pub fn calls(&self) -> usize {
        self.true_positive + self.false_positive
    }

    /// Positive labels.
    #[must_use]
    pub fn positives(&self) -> usize {
        self.true_positive + self.false_negative
    }

    /// The share of positive calls that were right; `None` without a call.
    #[must_use]
    pub fn precision(&self) -> Option<f64> {
        (self.calls() > 0).then(|| self.true_positive as f64 / self.calls() as f64)
    }

    /// The share of positives called; `None` without a positive.
    #[must_use]
    pub fn recall(&self) -> Option<f64> {
        (self.positives() > 0).then(|| self.true_positive as f64 / self.positives() as f64)
    }
}

/// What a fit maximizes, over pooled predictions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "metric", rename_all = "kebab-case")]
pub enum Metric {
    /// The share of examples decided right.
    Accuracy,
    /// F1 of the positive label. 0 when nothing is called or nothing is
    /// positive.
    F1 { positive: String },
    /// Recall of the positive label when precision reaches `floor`, and
    /// precision minus 1 otherwise, so every setting that reaches the floor
    /// beats every one that doesn't. The rule the stall and truthful-check
    /// protocols chose their thresholds by.
    RecallAtPrecision { floor: f64, positive: String },
}

impl Metric {
    /// Reads `accuracy`, `f1`, or `recall-at-precision:FLOOR`, with the
    /// label a binary metric counts as positive.
    ///
    /// # Errors
    ///
    /// Returns a sentence naming the metrics when `text` is none of them.
    pub fn parse(text: &str, positive: &str) -> Result<Self, String> {
        match text {
            "accuracy" => Ok(Metric::Accuracy),
            "f1" => Ok(Metric::F1 {
                positive: positive.to_string(),
            }),
            other => {
                let floor = other
                    .strip_prefix("recall-at-precision:")
                    .and_then(|floor| floor.parse::<f64>().ok())
                    .filter(|floor| (0.0..=1.0).contains(floor))
                    .ok_or_else(|| {
                        format!(
                            "unknown metric {other}: use accuracy, f1, or recall-at-precision:FLOOR with FLOOR from 0 to 1"
                        )
                    })?;
                Ok(Metric::RecallAtPrecision {
                    floor,
                    positive: positive.to_string(),
                })
            }
        }
    }

    /// The metric's name, as `parse` reads it.
    #[must_use]
    pub fn name(&self) -> String {
        match self {
            Metric::Accuracy => "accuracy".to_string(),
            Metric::F1 { .. } => "f1".to_string(),
            Metric::RecallAtPrecision { floor, .. } => format!("recall-at-precision:{floor}"),
        }
    }

    /// The label a binary metric counts as positive.
    #[must_use]
    pub fn positive(&self) -> &str {
        match self {
            Metric::Accuracy => "true",
            Metric::F1 { positive } | Metric::RecallAtPrecision { positive, .. } => positive,
        }
    }

    /// The metric over `(prediction, label)` pairs. 0 for no pairs.
    #[must_use]
    pub fn score(&self, pairs: &[(String, String)]) -> f64 {
        let tally = Tally::of(pairs, self.positive());
        match self {
            Metric::Accuracy => {
                if tally.examples == 0 {
                    0.0
                } else {
                    tally.correct as f64 / tally.examples as f64
                }
            }
            Metric::F1 { .. } => {
                let denominator =
                    2 * tally.true_positive + tally.false_positive + tally.false_negative;
                if denominator == 0 {
                    0.0
                } else {
                    2.0 * tally.true_positive as f64 / denominator as f64
                }
            }
            Metric::RecallAtPrecision { floor, .. } => match tally.precision() {
                Some(precision) if precision >= *floor => tally.recall().unwrap_or(0.0),
                Some(precision) => precision - 1.0,
                None => -1.0,
            },
        }
    }
}

/// The pairs `settings` predict on `examples`.
#[must_use]
pub fn predict<C>(
    rule: Rule<'_, C>,
    settings: &Settings,
    examples: &[&Example<C>],
) -> Vec<(String, String)> {
    examples
        .iter()
        .map(|example| (rule(settings, example), example.label.clone()))
        .collect()
}

fn score_on<C>(
    metric: &Metric,
    rule: Rule<'_, C>,
    settings: &Settings,
    examples: &[&Example<C>],
) -> f64 {
    metric.score(&predict(rule, settings, examples))
}

// ---------------------------------------------------------------------------
// Candidates
// ---------------------------------------------------------------------------

/// One candidate value of a coordinate.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    /// The setting's whole block with the coordinate replaced.
    pub decision: Decision,
    /// The coordinate's value.
    pub value: f64,
    /// The width of the gap between observed values the candidate sits in;
    /// in log space for a weight.
    pub gap: f64,
}

/// The midpoints between neighboring points, each with its gap, in
/// ascending order; at most [`CAP`], evenly spaced by rank.
#[must_use]
pub fn midpoints(points: &[f64]) -> Vec<(f64, f64)> {
    let mut sorted: Vec<f64> = points.iter().copied().filter(|p| p.is_finite()).collect();
    sorted.sort_by(f64::total_cmp);
    sorted.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
    let all: Vec<(f64, f64)> = sorted
        .windows(2)
        .map(|pair| (f64::midpoint(pair[0], pair[1]), pair[1] - pair[0]))
        .collect();
    if all.len() <= CAP {
        return all;
    }
    let last = all.len() - 1;
    let mut picked: Vec<usize> = (0..CAP)
        .map(|k| ((k * last) as f64 / (CAP - 1) as f64).round() as usize)
        .collect();
    picked.dedup();
    picked.into_iter().map(|index| all[index]).collect()
}

/// The probability-weighted mean level of a Score answer: from its
/// probabilities when their mass is positive, and its own `score`
/// otherwise. The same mean [`Cuts::level`] places.
#[must_use]
pub fn mean_level(answer: &ScoreAnswer) -> f64 {
    let mass: f64 = answer.probabilities.values().sum();
    if mass > 0.0 {
        answer
            .probabilities
            .iter()
            .map(|(level, p)| f64::from(*level) * p)
            .sum::<f64>()
            / mass
    } else {
        answer.score
    }
}

/// The cuts a Score setting starts from when it has none: halfway between
/// neighboring levels.
#[must_use]
pub fn start_cuts(levels: &[u32]) -> Vec<f64> {
    levels
        .windows(2)
        .map(|pair| f64::midpoint(f64::from(pair[0]), f64::from(pair[1])))
        .collect()
}

/// The candidates of one coordinate on `examples`, with the setting's
/// current block `current`.
#[must_use]
pub fn candidates<C>(
    spec: &Spec,
    coordinate: &Coordinate,
    current: &Decision,
    examples: &[&Example<C>],
) -> Vec<Candidate> {
    let answers = examples.iter().filter_map(|e| e.answers.get(&spec.name));
    match (coordinate, &spec.kind) {
        (Coordinate::Threshold, Kind::Threshold) => {
            let mut points: Vec<f64> = answers
                .filter_map(|answer| match answer {
                    Answer::Noul(p) => Some(p.clamp(0.0, 1.0)),
                    _ => None,
                })
                .collect();
            points.extend([0.0, 1.0]);
            midpoints(&points)
                .into_iter()
                .map(|(value, gap)| Candidate {
                    decision: Decision {
                        threshold: Some(Threshold::at(value)),
                        ..current.clone()
                    },
                    value,
                    gap,
                })
                .collect()
        }
        (Coordinate::Cut(index), Kind::Cuts { levels }) => {
            let base: Vec<f64> = current
                .cuts
                .as_ref()
                .map_or_else(|| start_cuts(levels), |cuts| cuts.values().to_vec());
            let (Some(&first), Some(&last)) = (levels.first(), levels.last()) else {
                return Vec::new();
            };
            let lower = if *index == 0 {
                f64::from(first)
            } else {
                base[index - 1]
            };
            let upper = base.get(index + 1).copied().unwrap_or(f64::from(last));
            let mut points: Vec<f64> = answers
                .filter_map(|answer| match answer {
                    Answer::Score(score) => Some(mean_level(score)),
                    _ => None,
                })
                .filter(|mean| *mean > lower && *mean < upper)
                .collect();
            points.extend([lower, upper]);
            midpoints(&points)
                .into_iter()
                .filter_map(|(value, gap)| {
                    let mut cuts = base.clone();
                    cuts[*index] = value;
                    Some(Candidate {
                        decision: Decision {
                            cuts: Some(Cuts::new(cuts).ok()?),
                            ..current.clone()
                        },
                        value,
                        gap,
                    })
                })
                .collect()
        }
        (Coordinate::Weight(option), Kind::Weights { .. }) => {
            let base: BTreeMap<String, f64> = current
                .weights
                .as_ref()
                .map(|weights| {
                    weights
                        .options()
                        .map(|name| (name.to_string(), weights.of(name)))
                        .collect()
                })
                .unwrap_or_default();
            let weight = |name: &str| base.get(name).copied().unwrap_or(1.0);
            let span = WEIGHT_SPAN.ln();
            let mut points: Vec<f64> = answers
                .filter_map(|answer| match answer {
                    Answer::Choice(choice) => {
                        let own = *choice.probabilities.get(option)?;
                        let other = choice
                            .probabilities
                            .iter()
                            .filter(|(name, _)| *name != option)
                            .map(|(name, p)| p * weight(name))
                            .fold(0.0_f64, f64::max);
                        (own > 0.0 && other > 0.0).then(|| (other / own).ln())
                    }
                    _ => None,
                })
                .filter(|flip| flip.abs() < span)
                .collect();
            points.extend([-span, span]);
            midpoints(&points)
                .into_iter()
                .filter_map(|(log, gap)| {
                    let mut weights = base.clone();
                    let value = log.exp();
                    weights.insert(option.clone(), value);
                    Some(Candidate {
                        decision: Decision {
                            weights: Some(Weights::new(weights).ok()?),
                            ..current.clone()
                        },
                        value,
                        gap,
                    })
                })
                .collect()
        }
        _ => Vec::new(),
    }
}

/// The best candidate on `examples` and its score: the highest score, then
/// the widest gap, then the lowest value.
fn select<C>(
    spec: &Spec,
    coordinate: &Coordinate,
    settings: &Settings,
    examples: &[&Example<C>],
    metric: &Metric,
    rule: Rule<'_, C>,
) -> Option<(Candidate, f64, usize)> {
    let current = settings.get(&spec.name).cloned().unwrap_or_default();
    let found = candidates(spec, coordinate, &current, examples);
    let count = found.len();
    let mut best: Option<(Candidate, f64)> = None;
    for candidate in found {
        let mut trial = settings.clone();
        trial.insert(spec.name.clone(), candidate.decision.clone());
        let score = score_on(metric, rule, &trial, examples);
        let better = match &best {
            None => true,
            Some((held, held_score)) => {
                score > *held_score || (score == *held_score && candidate.gap > held.gap)
            }
        };
        if better {
            best = Some((candidate, score));
        }
    }
    best.map(|(candidate, score)| (candidate, score, count))
}

// ---------------------------------------------------------------------------
// Folds
// ---------------------------------------------------------------------------

/// Assigns each group a fold: the groups are ordered by the SHA-256 of
/// [`FOLD_SALT`] and the name, then dealt round-robin into
/// `min(FOLDS, groups)` folds. The same groups always get the same folds.
#[must_use]
pub fn folds<'a>(groups: impl IntoIterator<Item = &'a str>) -> BTreeMap<String, usize> {
    let distinct: BTreeSet<&str> = groups.into_iter().collect();
    let mut keyed: Vec<(String, &str)> = distinct
        .into_iter()
        .map(|group| {
            let digest = Sha256::digest(format!("{FOLD_SALT}{group}").as_bytes());
            let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
            (hex, group)
        })
        .collect();
    keyed.sort();
    let k = FOLDS.min(keyed.len()).max(1);
    keyed
        .into_iter()
        .enumerate()
        .map(|(rank, (_, group))| (group.to_string(), rank % k))
        .collect()
}

// ---------------------------------------------------------------------------
// The fit
// ---------------------------------------------------------------------------

/// One fold of a held-out check.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FoldResult {
    pub fold: usize,
    pub examples: usize,
    /// The value the selection on the other folds chose; `None` when it
    /// kept the current value.
    pub chosen: Option<f64>,
    pub before: f64,
    pub after: f64,
}

/// A held-out check: the pooled score of the current value and of the value
/// rechosen without each fold, on that fold.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeldOut {
    pub folds: usize,
    pub before: f64,
    pub after: f64,
    pub passed: bool,
    pub per_fold: Vec<FoldResult>,
}

/// One coordinate's search and what became of it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Step {
    pub setting: String,
    pub coordinate: String,
    /// The setting's block before the step.
    pub before: Value,
    /// The best candidate's block, when there was one.
    pub proposed: Option<Value>,
    pub candidates: usize,
    pub train_before: f64,
    pub train_after: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub held_out: Option<HeldOut>,
    pub accepted: bool,
    pub why: String,
}

/// A finished fit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Fit {
    pub metric: String,
    pub before: Settings,
    pub after: Settings,
    pub steps: Vec<Step>,
    pub train_before: f64,
    pub train_after: f64,
    pub train_examples: usize,
    pub groups: usize,
}

impl Fit {
    /// The settings the fit moved, by name.
    #[must_use]
    pub fn changed(&self) -> Settings {
        self.after
            .iter()
            .filter(|(name, decision)| self.before.get(*name) != Some(*decision))
            .map(|(name, decision)| (name.clone(), decision.clone()))
            .collect()
    }
}

/// The held-out check of one coordinate.
fn held_out<C>(
    spec: &Spec,
    coordinate: &Coordinate,
    settings: &Settings,
    examples: &[&Example<C>],
    metric: &Metric,
    rule: Rule<'_, C>,
) -> HeldOut {
    let assignment = folds(examples.iter().map(|e| e.group.as_str()));
    let k = assignment.values().copied().max().map_or(0, |max| max + 1);
    let mut pooled_before = Vec::new();
    let mut pooled_after = Vec::new();
    let mut per_fold = Vec::new();
    if k < 2 {
        return HeldOut {
            folds: k,
            before: 0.0,
            after: 0.0,
            passed: false,
            per_fold,
        };
    }
    for fold in 0..k {
        let (test, train): (Vec<&Example<C>>, Vec<&Example<C>>) = examples
            .iter()
            .copied()
            .partition(|e| assignment[&e.group] == fold);
        let current_train = score_on(metric, rule, settings, &train);
        let mut chosen_settings = settings.clone();
        let mut chosen = None;
        if let Some((candidate, score, _)) =
            select(spec, coordinate, settings, &train, metric, rule)
            && score > current_train
        {
            chosen = Some(candidate.value);
            chosen_settings.insert(spec.name.clone(), candidate.decision);
        }
        let before = predict(rule, settings, &test);
        let after = predict(rule, &chosen_settings, &test);
        per_fold.push(FoldResult {
            fold,
            examples: test.len(),
            chosen,
            before: metric.score(&before),
            after: metric.score(&after),
        });
        pooled_before.extend(before);
        pooled_after.extend(after);
    }
    let before = metric.score(&pooled_before);
    let after = metric.score(&pooled_after);
    HeldOut {
        folds: k,
        before,
        after,
        passed: after > before,
        per_fold,
    }
}

/// Fits `specs` on `examples`, starting from `start`: one pass over every
/// setting's coordinates, in the order given, each kept only when it beats
/// the current value on training and on the held-out check.
#[must_use]
pub fn fit<C>(
    specs: &[Spec],
    start: &Settings,
    examples: &[&Example<C>],
    metric: &Metric,
    rule: Rule<'_, C>,
) -> Fit {
    let mut settings = start.clone();
    for spec in specs {
        settings.entry(spec.name.clone()).or_default();
    }
    let before = settings.clone();
    let train_before = score_on(metric, rule, &settings, examples);
    let mut steps = Vec::new();
    for spec in specs {
        for coordinate in spec.coordinates() {
            let current = settings.get(&spec.name).cloned().unwrap_or_default();
            let current_score = score_on(metric, rule, &settings, examples);
            let block = |decision: &Decision| serde_json::to_value(decision).unwrap_or(Value::Null);
            let mut step = Step {
                setting: spec.name.clone(),
                coordinate: coordinate.label(),
                before: block(&current),
                proposed: None,
                candidates: 0,
                train_before: current_score,
                train_after: None,
                held_out: None,
                accepted: false,
                why: String::new(),
            };
            match select(spec, &coordinate, &settings, examples, metric, rule) {
                None => step.why = "no candidate: the answers give no value to try".to_string(),
                Some((candidate, score, count)) => {
                    step.candidates = count;
                    step.proposed = Some(block(&candidate.decision));
                    step.train_after = Some(score);
                    if score <= current_score {
                        step.why =
                            "kept: no candidate scores strictly better on training".to_string();
                    } else {
                        let check = held_out(spec, &coordinate, &settings, examples, metric, rule);
                        if check.passed {
                            step.accepted = true;
                            step.why = "accepted: better on training and on the held-out check"
                                .to_string();
                            settings.insert(spec.name.clone(), candidate.decision);
                        } else if check.folds < 2 {
                            step.why = "kept: fewer than two groups, so no held-out check can run"
                                .to_string();
                        } else {
                            step.why = "kept: better on training, but not on the held-out check"
                                .to_string();
                        }
                        step.held_out = Some(check);
                    }
                }
            }
            steps.push(step);
        }
    }
    let train_after = score_on(metric, rule, &settings, examples);
    Fit {
        metric: metric.name(),
        before,
        after: settings,
        steps,
        train_before,
        train_after,
        train_examples: examples.len(),
        groups: examples
            .iter()
            .map(|e| e.group.as_str())
            .collect::<BTreeSet<_>>()
            .len(),
    }
}

// ---------------------------------------------------------------------------
// Comparison on a held-out partition
// ---------------------------------------------------------------------------

/// A score, its task-resampled interval, and its counts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Summary {
    pub score: f64,
    /// The 2.5th and 97.5th percentiles over resampled tasks.
    pub interval: [f64; 2],
    pub tally: Tally,
    pub precision: Option<f64>,
    pub precision_wilson: Option<[f64; 2]>,
    pub recall: Option<f64>,
    pub recall_wilson: Option<[f64; 2]>,
}

/// Two settings' scores on the same examples, and their difference.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Comparison {
    pub examples: usize,
    pub tasks: usize,
    pub resamples: usize,
    pub seed: u64,
    pub before: Summary,
    pub after: Summary,
    /// After minus before, and its interval over the same resamples.
    pub difference: f64,
    pub difference_interval: [f64; 2],
}

fn percentile(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    sorted[((sorted.len() - 1) as f64 * q).round() as usize]
}

fn interval(mut values: Vec<f64>) -> [f64; 2] {
    values.sort_by(f64::total_cmp);
    [percentile(&values, 0.025), percentile(&values, 0.975)]
}

fn summary(metric: &Metric, pairs: &[(String, String)], resampled: Vec<f64>) -> Summary {
    let tally = Tally::of(pairs, metric.positive());
    let wilson =
        |k: usize, n: usize| crate::component::finish::wilson(k, n).map(|(low, high)| [low, high]);
    Summary {
        score: metric.score(pairs),
        interval: interval(resampled),
        tally,
        precision: tally.precision(),
        precision_wilson: wilson(tally.true_positive, tally.calls()),
        recall: tally.recall(),
        recall_wilson: wilson(tally.true_positive, tally.positives()),
    }
}

/// Compares `before` and `after` on `examples`, with a bootstrap that
/// resamples whole tasks with replacement.
#[must_use]
pub fn compare<C>(
    metric: &Metric,
    rule: Rule<'_, C>,
    before: &Settings,
    after: &Settings,
    examples: &[&Example<C>],
    resamples: usize,
    seed: u64,
) -> Comparison {
    // Each task's pairs under `before` and under `after`.
    type Both = (Vec<(String, String)>, Vec<(String, String)>);
    let mut by_task: BTreeMap<&str, Both> = BTreeMap::new();
    for example in examples {
        let entry = by_task.entry(example.task.as_str()).or_default();
        entry.0.push((rule(before, example), example.label.clone()));
        entry.1.push((rule(after, example), example.label.clone()));
    }
    let tasks: Vec<&Both> = by_task.values().collect();
    let all_before: Vec<(String, String)> =
        tasks.iter().flat_map(|(b, _)| b.iter().cloned()).collect();
    let all_after: Vec<(String, String)> =
        tasks.iter().flat_map(|(_, a)| a.iter().cloned()).collect();
    let mut rng = super::Rng::new(seed);
    let (mut scores_before, mut scores_after, mut differences) = (
        Vec::with_capacity(resamples),
        Vec::with_capacity(resamples),
        Vec::with_capacity(resamples),
    );
    if !tasks.is_empty() {
        for _ in 0..resamples {
            let mut sample_before = Vec::new();
            let mut sample_after = Vec::new();
            for _ in 0..tasks.len() {
                let (b, a) = tasks[rng.below(tasks.len())];
                sample_before.extend(b.iter().cloned());
                sample_after.extend(a.iter().cloned());
            }
            let b = metric.score(&sample_before);
            let a = metric.score(&sample_after);
            scores_before.push(b);
            scores_after.push(a);
            differences.push(a - b);
        }
    }
    let before_summary = summary(metric, &all_before, scores_before);
    let after_summary = summary(metric, &all_after, scores_after);
    Comparison {
        examples: all_before.len(),
        tasks: tasks.len(),
        resamples,
        seed,
        difference: after_summary.score - before_summary.score,
        difference_interval: interval(differences),
        before: before_summary,
        after: after_summary,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use indexmap::IndexMap;

    /// A Noul example whose label is yes exactly when `p` reaches `cut`,
    /// with `flip` of them labeled the other way.
    fn noul(i: usize, p: f64, label: bool) -> Example<()> {
        Example {
            id: format!("e{i}"),
            group: format!("g{}", i % 10),
            task: format!("t{}", i % 5),
            answers: [("q".to_string(), Answer::Noul(p))].into(),
            label: yes_no(label),
            context: (),
        }
    }

    fn single(settings: &Settings, example: &Example<()>) -> String {
        let (name, answer) = example.answers.iter().next().expect("one answer");
        answer
            .decide(settings.get(name).unwrap_or(&Decision::default()))
            .unwrap_or_default()
    }

    fn threshold_spec() -> Vec<Spec> {
        vec![Spec {
            name: "q".to_string(),
            kind: Kind::Threshold,
        }]
    }

    fn threshold_of(fit: &Fit) -> f64 {
        fit.after["q"].threshold.expect("a threshold").value()
    }

    /// One of 32 evenly spread probabilities, fewer than [`CAP`], so every
    /// gap is a candidate.
    fn grid(i: usize) -> f64 {
        ((i % 32) as f64 + 0.5) / 32.0
    }

    /// 200 examples with probabilities spread over 0 to 1, labeled yes at
    /// or above 0.7: the right threshold is the midpoint of the gap 0.7
    /// falls in, 0.6875.
    fn separable() -> Vec<Example<()>> {
        (0..200)
            .map(|i| {
                let p = grid(i * 7);
                noul(i, p, p >= 0.7)
            })
            .collect()
    }

    #[test]
    fn a_threshold_moves_to_the_gap_the_labels_put_it_in() {
        let examples = separable();
        let refs: Vec<&Example<()>> = examples.iter().collect();
        let start: Settings = [("q".to_string(), Decision::default())].into();
        let fit = fit(&threshold_spec(), &start, &refs, &Metric::Accuracy, &single);
        let t = threshold_of(&fit);
        assert!((t - 0.6875).abs() < 1e-9, "{t}");
        assert!(fit.steps[0].accepted, "{:?}", fit.steps[0]);
        assert!((fit.train_after - 1.0).abs() < 1e-12);
        assert!(fit.train_before < 0.85);
        let check = fit.steps[0].held_out.as_ref().expect("a check");
        assert_eq!(check.folds, FOLDS);
        assert!(check.passed && check.after > check.before);
        // The same examples give the same fit.
        let again = super::fit(&threshold_spec(), &start, &refs, &Metric::Accuracy, &single);
        assert_eq!(again, fit);
    }

    #[test]
    fn noise_that_only_fits_training_keeps_the_original_value() {
        // Labels that are coin flips: some candidate beats 0.5 on training
        // by chance, and the held-out check refuses it.
        let mut rng = crate::study::Rng::new(9659);
        let examples: Vec<Example<()>> = (0..60)
            .map(|i| {
                let p = (rng.below(1000) as f64 + 0.5) / 1000.0;
                noul(i, p, rng.below(2) == 1)
            })
            .collect();
        let refs: Vec<&Example<()>> = examples.iter().collect();
        let start: Settings = [("q".to_string(), Decision::default())].into();
        let fit = fit(&threshold_spec(), &start, &refs, &Metric::Accuracy, &single);
        let step = &fit.steps[0];
        assert!(
            step.train_after.expect("a candidate") > step.train_before,
            "{step:?}"
        );
        assert!(!step.accepted, "{step:?}");
        assert!(step.why.contains("held-out"), "{}", step.why);
        assert!(!step.held_out.as_ref().expect("a check").passed);
        assert_eq!(fit.after, start);
        assert!(fit.changed().is_empty());
    }

    #[test]
    fn a_threshold_already_right_is_kept() {
        let examples: Vec<Example<()>> = (0..100)
            .map(|i| {
                let p = (i as f64 + 0.5) / 100.0;
                noul(i, p, p >= 0.5)
            })
            .collect();
        let refs: Vec<&Example<()>> = examples.iter().collect();
        let start: Settings = [("q".to_string(), Decision::default())].into();
        let fit = fit(&threshold_spec(), &start, &refs, &Metric::Accuracy, &single);
        assert!(!fit.steps[0].accepted);
        assert!(
            fit.steps[0].why.contains("training"),
            "{}",
            fit.steps[0].why
        );
        assert_eq!(fit.after, start);
    }

    #[test]
    fn a_tie_goes_to_the_candidate_in_the_widest_gap() {
        // Yes at 0.9 and above, no at 0.1 and below, and nothing between:
        // every midpoint from 0.1 to 0.9 scores the same, and 0.5 sits in
        // the widest gap.
        let examples: Vec<Example<()>> = (0..20)
            .map(|i| {
                let p = if i % 2 == 0 {
                    0.9 + i as f64 / 1000.0
                } else {
                    0.1 - i as f64 / 1000.0
                };
                noul(i, p, i % 2 == 0)
            })
            .collect();
        let refs: Vec<&Example<()>> = examples.iter().collect();
        let start: Settings = [("q".to_string(), Decision::default())].into();
        let spec = &threshold_spec()[0];
        let (best, score, _) = select(
            spec,
            &Coordinate::Threshold,
            &start,
            &refs,
            &Metric::Accuracy,
            &single,
        )
        .expect("a candidate");
        assert!((score - 1.0).abs() < 1e-12);
        assert!((best.value - 0.5).abs() < 0.02, "{}", best.value);
    }

    #[test]
    fn candidates_are_midpoints_capped_and_evenly_spread() {
        let mids = midpoints(&[0.0, 0.2, 0.2, 0.6, 1.0]);
        assert_eq!(mids.len(), 3);
        assert!((mids[0].0 - 0.1).abs() < 1e-12 && (mids[0].1 - 0.2).abs() < 1e-12);
        assert!((mids[1].0 - 0.4).abs() < 1e-12);
        let many: Vec<f64> = (0..=500).map(|i| f64::from(i) / 500.0).collect();
        let capped = midpoints(&many);
        assert_eq!(capped.len(), CAP);
        assert!(capped.windows(2).all(|pair| pair[0].0 < pair[1].0));
        assert!(capped[0].0 < 0.01 && capped[CAP - 1].0 > 0.99);
    }

    #[test]
    fn folds_keep_groups_whole_and_are_stable() {
        let groups: Vec<String> = (0..23).map(|i| format!("trial-{i}")).collect();
        let one = folds(groups.iter().map(String::as_str));
        let two = folds(groups.iter().rev().map(String::as_str));
        assert_eq!(one, two);
        let mut sizes = [0usize; FOLDS];
        for fold in one.values() {
            sizes[*fold] += 1;
        }
        assert_eq!(sizes.iter().sum::<usize>(), 23);
        assert!(sizes.iter().all(|size| (4..=5).contains(size)), "{sizes:?}");
        let few = folds(["a", "b", "a"]);
        assert_eq!(few.len(), 2);
        assert!(few.values().all(|fold| *fold < 2));
    }

    fn choice_example(i: usize, pick: &str, probs: &[(&str, f64)], label: &str) -> Example<()> {
        Example {
            id: format!("c{i}"),
            group: format!("g{}", i % 10),
            task: format!("t{}", i % 5),
            answers: [(
                "pick".to_string(),
                Answer::Choice(ChoiceAnswer {
                    choice: pick.to_string(),
                    confidence: 0.5,
                    probabilities: probs
                        .iter()
                        .map(|(o, p)| ((*o).to_string(), *p))
                        .collect::<IndexMap<_, _>>(),
                }),
            )]
            .into(),
            label: label.to_string(),
            context: (),
        }
    }

    #[test]
    fn a_choice_weight_moves_the_pick_where_the_labels_say() {
        // `rare` is right whenever its probability is at least 0.3, though
        // the model picks it only above 0.5: tilting the pick about 2.3 to 1
        // toward it (0.7 / 0.3) is right.
        let examples: Vec<Example<()>> = (0..200)
            .map(|i| {
                let p = grid(i * 7);
                let pick = if p > 0.5 { "rare" } else { "common" };
                let label = if p >= 0.3 { "rare" } else { "common" };
                choice_example(i, pick, &[("common", 1.0 - p), ("rare", p)], label)
            })
            .collect();
        let refs: Vec<&Example<()>> = examples.iter().collect();
        let specs = vec![Spec {
            name: "pick".to_string(),
            kind: Kind::Weights {
                options: vec!["common".to_string(), "rare".to_string()],
            },
        }];
        let start: Settings = [("pick".to_string(), Decision::default())].into();
        let fit = fit(&specs, &start, &refs, &Metric::Accuracy, &single);
        assert!((fit.train_after - 1.0).abs() < 1e-12, "{fit:?}");
        let weights = fit.after["pick"].weights.clone().expect("weights");
        let ratio = weights.of("rare") / weights.of("common");
        assert!((2.05..2.37).contains(&ratio), "{ratio}");
    }

    #[test]
    fn score_cuts_move_to_the_mean_the_labels_put_them_at() {
        // Three levels; the right level is 0 below a mean of 0.6, 2 at or
        // above 1.7, and 1 between.
        let examples: Vec<Example<()>> = (0..200)
            .map(|i| {
                let mean = 2.0 * grid(i * 7);
                let (p0, p2) = if mean < 1.0 {
                    (1.0 - mean, 0.0)
                } else {
                    (0.0, mean - 1.0)
                };
                let p1 = 1.0 - p0 - p2;
                let label = if mean < 0.6 {
                    0
                } else if mean >= 1.7 {
                    2
                } else {
                    1
                };
                Example {
                    id: format!("s{i}"),
                    group: format!("g{}", i % 10),
                    task: format!("t{}", i % 5),
                    answers: [(
                        "level".to_string(),
                        Answer::Score(ScoreAnswer {
                            score: mean,
                            confidence: 0.5,
                            selected: None,
                            legend: BTreeMap::new(),
                            probabilities: [(0, p0), (1, p1), (2, p2)].into(),
                        }),
                    )]
                    .into(),
                    label: label.to_string(),
                    context: (),
                }
            })
            .collect();
        let refs: Vec<&Example<()>> = examples.iter().collect();
        let specs = vec![Spec {
            name: "level".to_string(),
            kind: Kind::Cuts {
                levels: vec![0, 1, 2],
            },
        }];
        let start: Settings = [("level".to_string(), Decision::default())].into();
        let fit = fit(&specs, &start, &refs, &Metric::Accuracy, &single);
        let cuts = fit.after["level"].cuts.clone().expect("cuts");
        assert!((0.59..0.66).contains(&cuts.values()[0]), "{cuts:?}");
        assert!((1.65..1.72).contains(&cuts.values()[1]), "{cuts:?}");
        assert!((fit.train_after - 1.0).abs() < 1e-12);
    }

    #[test]
    fn metrics_count_pooled_predictions() {
        let pairs: Vec<(String, String)> = [
            ("true", "true"),
            ("true", "false"),
            ("false", "true"),
            ("false", "false"),
            ("true", "true"),
        ]
        .iter()
        .map(|(p, l)| ((*p).to_string(), (*l).to_string()))
        .collect();
        assert!((Metric::Accuracy.score(&pairs) - 0.6).abs() < 1e-12);
        let f1 = Metric::parse("f1", "true").unwrap();
        assert!((f1.score(&pairs) - 2.0 / 3.0).abs() < 1e-12);
        let floor = Metric::parse("recall-at-precision:0.6", "true").unwrap();
        assert!((floor.score(&pairs) - 2.0 / 3.0).abs() < 1e-12);
        let high = Metric::parse("recall-at-precision:0.9", "true").unwrap();
        assert!((high.score(&pairs) - (2.0 / 3.0 - 1.0)).abs() < 1e-12);
        assert!(Metric::parse("auc", "true").is_err());
        assert_eq!(high.name(), "recall-at-precision:0.9");
    }

    #[test]
    fn a_comparison_resamples_tasks_and_is_seeded() {
        let examples = separable();
        let refs: Vec<&Example<()>> = examples.iter().collect();
        let before: Settings = [("q".to_string(), Decision::default())].into();
        let after: Settings = [(
            "q".to_string(),
            Decision {
                threshold: Some(Threshold::at(0.7)),
                ..Decision::default()
            },
        )]
        .into();
        let one = compare(&Metric::Accuracy, &single, &before, &after, &refs, 500, 7);
        assert_eq!(
            one,
            compare(&Metric::Accuracy, &single, &before, &after, &refs, 500, 7)
        );
        assert_eq!(one.tasks, 5);
        assert!(one.difference > 0.0);
        assert!(one.difference_interval[0] > 0.0);
        assert!(
            one.before.interval[0] <= one.before.score
                && one.before.score <= one.before.interval[1]
        );
    }

    #[test]
    fn answers_read_as_jev_returns_them() {
        let noul = Answer::from_json(&json!({"type": "noul", "noul": 0.3})).unwrap();
        assert_eq!(noul.decide(&Decision::default()).as_deref(), Some("false"));
        let choice = Answer::from_json(
            &json!({"type": "choice", "choice": "a", "probabilities": {"a": 0.6, "b": 0.4}}),
        )
        .unwrap();
        assert_eq!(choice.decide(&Decision::default()).as_deref(), Some("a"));
        let score = Answer::from_json(
            &json!({"type": "score", "score": 1.2, "probabilities": {"0": 0.1, "1": 0.6, "2": 0.3}}),
        )
        .unwrap();
        assert_eq!(score.decide(&Decision::default()).as_deref(), Some("1"));
        assert!(Answer::from_json(&json!({"type": "rank"})).is_err());
    }
}
