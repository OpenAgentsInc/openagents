//! Decision settings: how an answer becomes an action, kept apart from the
//! question that produced it.
//!
//! A question's wording decides what the model is asked. A decision setting
//! decides what your code does with the probabilities that come back: the
//! probability at which a Noul reads as yes, the cuts that turn a Score's
//! probability-weighted mean into a level, and the weights that tilt a
//! Choice toward one option. None of the three is sent. Changing one leaves
//! the request, and every answer recorded for it, exactly as it was, so a
//! setting can be refitted on recorded answers without asking again.
//!
//! A question-set file carries the settings as an optional `decision` block
//! beside a question's wording:
//!
//! ```json
//! {
//!   "type": "noul",
//!   "instructions": "Does the customer ask for money back?",
//!   "decision": { "threshold": 0.7 }
//! }
//! ```
//!
//! [`split`] lifts the block out before the question is sent or digested,
//! and a reader digests the settings on their own.
//!
//! Every setting is optional, and an absent one reproduces what a caller did
//! before settings existed: a Noul reads as yes at or above 0.5
//! ([`Threshold::MIDPOINT`]), a Score's level is the one the estimator
//! selected, and a Choice is the option the model picked.
//!
//! Choice weights are a rule applied where the answer is used. They never
//! change what the model answered or what a calibration map is fitted on: a
//! map never overrides the model's pick, and a weight is not a map.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::answers::{ChoiceAnswer, NoulAnswer, ScoreAnswer};

/// The key a question-set file puts a question's decision settings under.
pub const DECISION_KEY: &str = "decision";

/// The probability at or above which a Noul reads as yes.
///
/// Serialized as the bare number, so a record that carried a threshold as a
/// number carries it the same way.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Threshold(f64);

impl Threshold {
    /// The midpoint, 0.5: the threshold a caller used before settings
    /// existed.
    pub const MIDPOINT: Self = Self(0.5);

    /// A threshold at `p`. A value outside 0 to 1 never reads as yes, or
    /// always does; [`Decision::validate`] refuses one from a file.
    #[must_use]
    pub const fn at(p: f64) -> Self {
        Self(p)
    }

    /// The threshold as a probability.
    #[must_use]
    pub const fn value(self) -> f64 {
        self.0
    }

    /// Whether a probability of yes reads as yes: at or above the threshold.
    #[must_use]
    pub fn yes(self, p: f64) -> bool {
        p >= self.0
    }

    /// Whether a Noul answer reads as yes.
    #[must_use]
    pub fn noul(self, answer: &NoulAnswer) -> bool {
        self.yes(answer.noul)
    }
}

impl Default for Threshold {
    fn default() -> Self {
        Self::MIDPOINT
    }
}

impl fmt::Display for Threshold {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, formatter)
    }
}

/// The boundaries that turn a Score's probability-weighted mean into a
/// level: the mean falls in the level above every cut it reaches.
///
/// With levels `0, 1, 2` and cuts `[0.8, 1.5]`, a mean of 0.7 is level 0,
/// 0.8 is level 1, and 1.5 is level 2. A Score with `n` levels takes
/// `n − 1` cuts, in ascending order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "Vec<f64>", into = "Vec<f64>")]
pub struct Cuts(Vec<f64>);

impl Cuts {
    /// Cuts from a list of boundaries.
    ///
    /// # Errors
    ///
    /// Returns a sentence when the list is empty, holds a number that is
    /// not finite, or is not strictly ascending.
    pub fn new(cuts: Vec<f64>) -> Result<Self, String> {
        if cuts.is_empty() {
            return Err("cuts name no boundary; leave them out instead".to_string());
        }
        if cuts.iter().any(|cut| !cut.is_finite()) {
            return Err("every cut must be a finite number".to_string());
        }
        if cuts.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err("cuts must be strictly ascending".to_string());
        }
        Ok(Self(cuts))
    }

    /// The boundaries, in ascending order.
    #[must_use]
    pub fn values(&self) -> &[f64] {
        &self.0
    }

    /// The level a Score answer falls in under these cuts.
    ///
    /// The mean is recomputed from `probabilities` when the answer carries
    /// them, and is the answer's own `score` otherwise. `None` when the
    /// answer names no levels, or a count of levels these cuts do not fit.
    #[must_use]
    pub fn level(&self, answer: &ScoreAnswer) -> Option<u32> {
        let levels = levels(answer);
        if levels.len() != self.0.len() + 1 {
            return None;
        }
        let mean = weighted_mean(answer);
        let reached = self.0.iter().filter(|cut| mean >= **cut).count();
        levels.get(reached).copied()
    }
}

impl TryFrom<Vec<f64>> for Cuts {
    type Error = String;

    fn try_from(cuts: Vec<f64>) -> Result<Self, String> {
        Self::new(cuts)
    }
}

impl From<Cuts> for Vec<f64> {
    fn from(cuts: Cuts) -> Self {
        cuts.0
    }
}

/// A weight per Choice option. The decision is the option with the largest
/// probability times weight; an option with no weight weighs 1.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "BTreeMap<String, f64>", into = "BTreeMap<String, f64>")]
pub struct Weights(BTreeMap<String, f64>);

impl Weights {
    /// Weights from a map of option to weight.
    ///
    /// # Errors
    ///
    /// Returns a sentence when the map is empty or a weight is negative or
    /// not finite.
    pub fn new(weights: BTreeMap<String, f64>) -> Result<Self, String> {
        if weights.is_empty() {
            return Err("weights name no option; leave them out instead".to_string());
        }
        if let Some((option, weight)) = weights
            .iter()
            .find(|(_, weight)| !weight.is_finite() || **weight < 0.0)
        {
            return Err(format!(
                "the weight for {option} is {weight}; a weight is a finite number of 0 or more"
            ));
        }
        Ok(Self(weights))
    }

    /// The weight of one option: 1 when the map names none.
    #[must_use]
    pub fn of(&self, option: &str) -> f64 {
        self.0.get(option).copied().unwrap_or(1.0)
    }

    /// The options the map names.
    pub fn options(&self) -> impl Iterator<Item = &str> {
        self.0.keys().map(String::as_str)
    }

    /// The option these weights decide on: the largest probability times
    /// weight. A tie keeps the model's own pick when it is among the tied,
    /// and the first tied option in the response's order otherwise. An
    /// answer without probabilities is the model's pick.
    #[must_use]
    pub fn pick<'a>(&self, answer: &'a ChoiceAnswer) -> &'a str {
        let mut best: Option<(&'a str, f64)> = None;
        for (option, p) in &answer.probabilities {
            let weighed = p * self.of(option);
            match best {
                Some((_, held)) if weighed < held => {}
                Some((_, held)) if weighed == held && option != &answer.choice => {}
                _ => best = Some((option.as_str(), weighed)),
            }
        }
        best.map_or(answer.choice.as_str(), |(option, _)| option)
    }
}

impl TryFrom<BTreeMap<String, f64>> for Weights {
    type Error = String;

    fn try_from(weights: BTreeMap<String, f64>) -> Result<Self, String> {
        Self::new(weights)
    }
}

impl From<Weights> for BTreeMap<String, f64> {
    fn from(weights: Weights) -> Self {
        weights.0
    }
}

/// One question's decision settings. Each one is optional, and an absent
/// one reproduces the decision a caller made before settings existed.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Decision {
    /// The probability at or above which a Noul reads as yes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threshold: Option<Threshold>,
    /// The boundaries that turn a Score's mean into a level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cuts: Option<Cuts>,
    /// A weight per Choice option.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weights: Option<Weights>,
}

// Every number a `Decision` holds is finite: `Cuts` and `Weights` refuse
// one that is not, and JSON has no spelling for NaN.
impl Eq for Decision {}

impl Decision {
    /// Whether the block sets nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.threshold.is_none() && self.cuts.is_none() && self.weights.is_none()
    }

    /// The threshold, or `default` when the block sets none.
    #[must_use]
    pub fn threshold_or(&self, default: Threshold) -> Threshold {
        self.threshold.unwrap_or(default)
    }

    /// Whether a Noul answer reads as yes: at or above the threshold, which
    /// is 0.5 when the block sets none.
    #[must_use]
    pub fn noul(&self, answer: &NoulAnswer) -> bool {
        self.threshold_or(Threshold::MIDPOINT).noul(answer)
    }

    /// The level a Score answer decides on.
    ///
    /// With cuts, the level the probability-weighted mean falls in. Without
    /// them, the level the estimator selected, as [`selected_level`] reads
    /// it.
    #[must_use]
    pub fn level(&self, answer: &ScoreAnswer) -> Option<u32> {
        match &self.cuts {
            Some(cuts) => cuts.level(answer),
            None => selected_level(answer),
        }
    }

    /// The option a Choice answer decides on: the weighted argmax with
    /// weights, and the model's pick without them.
    #[must_use]
    pub fn choice<'a>(&self, answer: &'a ChoiceAnswer) -> &'a str {
        match &self.weights {
            Some(weights) => weights.pick(answer),
            None => answer.choice.as_str(),
        }
    }

    /// Whether the block fits the question it sits beside.
    ///
    /// A threshold belongs to a Noul, cuts to a Score, and weights to a
    /// Choice. A threshold lies from 0 to 1. Cuts fit a Score's level
    /// count, and weights name only a Choice's options, when the question
    /// spells them out.
    ///
    /// # Errors
    ///
    /// Returns a sentence naming the first setting that does not fit.
    pub fn validate(&self, question: &Value) -> Result<(), String> {
        let kind = question.get("type").and_then(Value::as_str);
        if let Some(threshold) = self.threshold {
            if kind != Some("noul") {
                return Err("a threshold applies only to a Noul question".to_string());
            }
            if !(0.0..=1.0).contains(&threshold.value()) {
                return Err(format!("the threshold is {threshold}, outside 0 to 1"));
            }
        }
        if let Some(cuts) = &self.cuts {
            if kind != Some("score") {
                return Err("cuts apply only to a Score question".to_string());
            }
            if let Some(levels) = question.get("criteria").and_then(Value::as_array)
                && levels.len() != cuts.values().len() + 1
            {
                return Err(format!(
                    "the question has {} levels, so it takes {} cuts, not {}",
                    levels.len(),
                    levels.len().saturating_sub(1),
                    cuts.values().len()
                ));
            }
        }
        if let Some(weights) = &self.weights {
            if kind != Some("choice") {
                return Err("weights apply only to a Choice question".to_string());
            }
            if let Some(options) = question.get("criteria").and_then(Value::as_object)
                && let Some(stranger) = weights
                    .options()
                    .find(|option| !options.contains_key(*option))
            {
                return Err(format!(
                    "the weights name {stranger}, which is not one of the question's options"
                ));
            }
        }
        Ok(())
    }
}

/// Lifts a question's `decision` block out of it.
///
/// The question is left as it is sent and digested, and the block is
/// returned on its own. A question with no block, or one that is not an
/// object, returns `None` and is left as it was.
///
/// # Errors
///
/// Returns a sentence when the block is not a decision block or does not
/// fit the question.
pub fn split(question: &mut Value) -> Result<Option<Decision>, String> {
    let Some(object) = question.as_object_mut() else {
        return Ok(None);
    };
    let Some(block) = object.remove(DECISION_KEY) else {
        return Ok(None);
    };
    let decision: Decision = serde_json::from_value(block)
        .map_err(|error| format!("the {DECISION_KEY} block does not read: {error}"))?;
    decision.validate(question)?;
    Ok(Some(decision))
}

/// Puts a decision block back beside a question's wording: the inverse of
/// [`split`]. An empty block is left out.
pub fn join(question: &mut Value, decision: &Decision) {
    if decision.is_empty() {
        return;
    }
    if let Some(object) = question.as_object_mut() {
        object.insert(
            DECISION_KEY.to_string(),
            serde_json::to_value(decision).unwrap_or(Value::Null),
        );
    }
}

/// The level the estimator selected: `selected` when the answer carries it,
/// and otherwise the highest level among the maxima of `probabilities`.
///
/// This is the categorical rule in
/// `docs/decision-models/measurements/2026-09-20-score-contract.md`.
#[must_use]
pub fn selected_level(answer: &ScoreAnswer) -> Option<u32> {
    if let Some(level) = answer
        .selected
        .as_deref()
        .and_then(|selected| selected.parse().ok())
    {
        return Some(level);
    }
    let mut best: Option<(u32, f64)> = None;
    for (level, p) in &answer.probabilities {
        // Levels come in ascending order, so `>=` keeps the highest tie.
        if best.is_none_or(|(_, held)| *p >= held) {
            best = Some((*level, *p));
        }
    }
    best.map(|(level, _)| level)
}

/// The levels a Score answer names, in ascending order: its legend's, or
/// its probabilities' when it sends no legend.
fn levels(answer: &ScoreAnswer) -> Vec<u32> {
    if answer.legend.is_empty() {
        answer.probabilities.keys().copied().collect()
    } else {
        answer.legend.keys().copied().collect()
    }
}

/// The probability-weighted mean level, from the probabilities when their
/// mass is positive and from the answer's own `score` otherwise.
fn weighted_mean(answer: &ScoreAnswer) -> f64 {
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

#[cfg(test)]
mod tests {
    use super::*;
    use indexmap::IndexMap;
    use serde_json::json;

    fn choice(pick: &str, probabilities: &[(&str, f64)]) -> ChoiceAnswer {
        ChoiceAnswer {
            choice: pick.to_string(),
            confidence: 0.5,
            probabilities: probabilities
                .iter()
                .map(|(option, p)| ((*option).to_string(), *p))
                .collect::<IndexMap<_, _>>(),
        }
    }

    fn score(mean: f64, selected: Option<&str>, probabilities: &[(u32, f64)]) -> ScoreAnswer {
        ScoreAnswer {
            score: mean,
            confidence: 0.5,
            selected: selected.map(str::to_string),
            legend: BTreeMap::new(),
            probabilities: probabilities.iter().copied().collect(),
        }
    }

    #[test]
    fn the_default_threshold_is_the_midpoint_and_reads_yes_at_or_above_it() {
        let decision = Decision::default();
        for (p, yes) in [
            (0.0, false),
            (0.49, false),
            (0.5, true),
            (0.51, true),
            (1.0, true),
        ] {
            let answer = NoulAnswer {
                noul: p,
                selected: None,
            };
            assert_eq!(decision.noul(&answer), p >= 0.5, "{p}");
            assert_eq!(decision.noul(&answer), yes, "{p}");
        }
        assert_eq!(Threshold::default(), Threshold::MIDPOINT);
        assert_eq!(Threshold::at(0.75).to_string(), "0.75");
        assert_eq!(format!("{:.2}", Threshold::at(0.5)), "0.50");
        assert_eq!(json!(Threshold::at(0.6)), json!(0.6));
    }

    #[test]
    fn a_set_threshold_moves_the_decision_and_nothing_else() {
        let decision = Decision {
            threshold: Some(Threshold::at(0.75)),
            ..Decision::default()
        };
        let answer = NoulAnswer {
            noul: 0.64,
            selected: Some("true".to_string()),
        };
        assert!(!decision.noul(&answer));
        assert!(Decision::default().noul(&answer));
        assert_eq!(
            decision.threshold_or(Threshold::MIDPOINT),
            Threshold::at(0.75)
        );
        assert_eq!(
            Decision::default().threshold_or(Threshold::at(0.2)),
            Threshold::at(0.2)
        );
    }

    #[test]
    fn a_choice_without_weights_is_the_models_pick_even_off_the_argmax() {
        // A calibrated answer can carry a pick that is not the largest
        // rescaled probability; the default decision keeps the pick.
        let answer = choice("billing", &[("billing", 0.4), ("technical", 0.45)]);
        assert_eq!(Decision::default().choice(&answer), "billing");
    }

    #[test]
    fn weights_take_the_argmax_of_probability_times_weight() {
        let answer = choice("billing", &[("billing", 0.6), ("technical", 0.4)]);
        let weights = Weights::new([("technical".to_string(), 2.0)].into()).unwrap();
        let decision = Decision {
            weights: Some(weights.clone()),
            ..Decision::default()
        };
        assert_eq!(decision.choice(&answer), "technical");
        let even = Weights::new([("billing".to_string(), 1.0)].into()).unwrap();
        assert_eq!(even.pick(&answer), "billing");
        // A tie keeps the model's pick.
        let tied = choice("technical", &[("billing", 0.5), ("technical", 0.5)]);
        assert_eq!(even.pick(&tied), "technical");
        // No probabilities: the pick stands.
        assert_eq!(weights.pick(&choice("billing", &[])), "billing");
    }

    #[test]
    fn a_score_without_cuts_is_the_selected_level_then_the_highest_tied_argmax() {
        let decision = Decision::default();
        assert_eq!(
            decision.level(&score(1.2, Some("2"), &[(0, 0.1), (1, 0.6), (2, 0.3)])),
            Some(2)
        );
        assert_eq!(
            decision.level(&score(1.2, None, &[(0, 0.1), (1, 0.6), (2, 0.3)])),
            Some(1)
        );
        assert_eq!(
            decision.level(&score(1.5, None, &[(0, 0.0), (1, 0.5), (2, 0.5)])),
            Some(2)
        );
        assert_eq!(decision.level(&score(1.0, None, &[])), None);
    }

    #[test]
    fn cuts_place_the_weighted_mean_between_levels() {
        let cuts = Cuts::new(vec![0.8, 1.5]).unwrap();
        let decision = Decision {
            cuts: Some(cuts),
            ..Decision::default()
        };
        // Mean 0.7: level 0, although level 1 is the argmax.
        let low = score(0.0, Some("1"), &[(0, 0.45), (1, 0.4), (2, 0.15)]);
        assert_eq!(decision.level(&low), Some(0));
        // Mean exactly at a cut reaches it.
        let at = score(0.0, None, &[(0, 0.2), (1, 0.8), (2, 0.0)]);
        assert_eq!(decision.level(&at), Some(1));
        // No probabilities: the answer's own mean.
        let mut bare = score(1.6, None, &[]);
        bare.legend = [(0, "a".into()), (1, "b".into()), (2, "c".into())].into();
        assert_eq!(decision.level(&bare), Some(2));
        // Cuts that do not fit the level count decide nothing.
        let two = score(0.5, None, &[(0, 0.5), (1, 0.5)]);
        assert_eq!(decision.level(&two), None);
    }

    #[test]
    fn a_split_leaves_the_wording_as_it_was_written_without_the_block() {
        let bare = json!({ "type": "noul", "instructions": "Money back?" });
        let mut with = json!({
            "type": "noul",
            "instructions": "Money back?",
            "decision": { "threshold": 0.7 },
        });
        let decision = split(&mut with).unwrap().unwrap();
        assert_eq!(with, bare);
        assert_eq!(decision.threshold, Some(Threshold::at(0.7)));
        let mut joined = with.clone();
        join(&mut joined, &decision);
        assert_eq!(joined["decision"], json!({ "threshold": 0.7 }));
        let mut plain = bare.clone();
        assert_eq!(split(&mut plain).unwrap(), None);
        assert_eq!(plain, bare);
    }

    #[test]
    fn the_request_body_is_the_same_with_and_without_a_block() {
        let body = |question: Value| {
            let mut question = question;
            split(&mut question).unwrap();
            crate::SystemOneRequest::new(
                "I was charged twice.",
                crate::Questions::new().with("refund", crate::Question::Raw(question)),
            )
            .body("jev-latest")
            .unwrap()
        };
        let bare = json!({ "type": "noul", "instructions": "Money back?" });
        let mut with = bare.clone();
        with["decision"] = json!({ "threshold": 0.9 });
        assert_eq!(body(with), body(bare));
    }

    #[test]
    fn a_block_that_does_not_fit_its_question_is_refused() {
        let refused = |question: Value| {
            let mut question = question;
            split(&mut question).unwrap_err()
        };
        assert!(
            refused(json!({ "type": "choice", "decision": { "threshold": 0.5 } })).contains("Noul")
        );
        assert!(
            refused(json!({ "type": "noul", "decision": { "threshold": 1.5 } }))
                .contains("outside")
        );
        assert!(
            refused(json!({ "type": "score", "criteria": ["a", "b", "c"], "decision": { "cuts": [1.0] } }))
                .contains("takes 2 cuts")
        );
        assert!(
            refused(json!({ "type": "score", "decision": { "cuts": [1.0, 0.5] } }))
                .contains("ascending")
        );
        assert!(
            refused(json!({ "type": "choice", "criteria": { "a": "x" }, "decision": { "weights": { "b": 2.0 } } }))
                .contains("not one of")
        );
        assert!(
            refused(json!({ "type": "choice", "decision": { "weights": { "a": -1.0 } } }))
                .contains("0 or more")
        );
        assert!(
            refused(json!({ "type": "noul", "decision": { "limit": 0.5 } }))
                .contains("does not read")
        );
    }
}
