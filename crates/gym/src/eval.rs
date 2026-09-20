//! One run over a suite: what a door's reply becomes, and what a table of
//! rows says afterwards.
//!
//! This module is the part of an evaluation that has no network in it. A
//! door's reply arrives as a [`jev`] answer or a [`jev::Error`]; what comes
//! out is a [`Row`], or nothing at all. Everything a run prints is then
//! computed from the rows, so the table and the store cannot disagree.
//!
//! # The line this module draws
//!
//! An evaluation asks a door to judge an item. Three things can come back:
//! the door judges it, the door declines to judge it, or the harness never
//! got an answer out of the door. The first two are results about the door
//! and belong in the record. The third is a fact about the wire.
//!
//! Getting that line wrong in either direction is a scoring fault:
//!
//! - Count a refusal as a missing item, and a door whose guardrails fire on
//!   the hard questions scores better for refusing them. The denominator
//!   shrinks and nothing says why.
//! - Count a reset connection as a refusal, and a door is charged for a
//!   network that dropped, which flatters whatever door happened to run on a
//!   quieter machine.
//!
//! [`Disposition`] is where the line is drawn, and [`classify`] draws it.

use std::collections::BTreeMap;

use indexmap::IndexMap;
use serde_json::Value;

use crate::calibrate::{Map, Metrics, Observation, score};
use crate::gate::{Comparison, Gate, Outcome};
use crate::row::{DoorIdentity, RefusalCode, Row};
use crate::suite::Item;

/// What one item's reply amounts to.
#[derive(Clone, Debug, PartialEq)]
pub enum Disposition {
    /// The door judged the item.
    Answered {
        /// The option the door picked.
        chosen: String,
        /// The probability it reported for each option, in the order it sent
        /// them.
        distribution: IndexMap<String, f64>,
    },
    /// The door declined to judge it, and said why.
    Refused(RefusalCode),
    /// The harness never got an answer out of the door.
    Harness(String),
}

impl Disposition {
    /// Whether this disposition produces a row.
    ///
    /// A harness failure does not. It is neither the door's fault nor its
    /// credit, so it leaves the record set entirely and the run reports it as
    /// a loss instead.
    #[must_use]
    pub const fn is_recorded(&self) -> bool {
        !matches!(self, Self::Harness(_))
    }
}

/// Decides whether a failed call was the door declining or the harness
/// failing.
///
/// **The rule: a typed refusal code in the body is the door's own answer and
/// stays in the denominator. A failure that carries no code is the harness
/// and produces no row at all.**
///
/// The question that settles every case is *did the door decline to judge,
/// or did the harness fail to ask?* A door that answers with
/// `{"error": {"code": ...}}` was reachable, read the request, and replied.
/// That reply is behavior, whatever the code says, and behavior belongs in
/// the record. A connection reset, a client timeout, or a body that does not
/// parse is not a reply; nobody can tell what the door would have said.
///
/// This replaces a list of six codes matched as substrings of the body, which
/// disagreed with itself on exactly the cases where it mattered.
/// `model_unavailable` was a refusal while `busy` and `bridge_error` fell
/// through to the harness, and all three describe a door that was reachable
/// and did not answer. The three that were ambiguous, and why they land where
/// they do:
///
/// - **`busy`** is the door shedding load. It is not about the item, and it
///   is retryable, which is a real argument for treating it as a harness
///   failure and asking again. But the harness cannot prove independence:
///   a door under pressure sheds the longest requests first, and the longest
///   requests are the hardest items. Keeping it in the denominator is the
///   direction that cannot flatter a door. Retrying is the client's job
///   before this function ever sees the error, and `jev`'s retry policy does
///   it; a `busy` that survives the retries is the door's settled answer.
/// - **`model_unavailable`** is the runtime being off, ineligible, or still
///   preparing. It is the least item-dependent of the three, which is an
///   argument for calling it infrastructure — but it is infrastructure *the
///   door owns*, and a run against a door that is off should report a
///   denominator full of `model_unavailable` rather than an empty table and
///   a shrug. Availability is part of what is being measured.
/// - **`bridge_error`** is the door's own helper failing. `crates/row.rs`
///   already states the case: the door answered, and the answer was that it
///   could not. A door whose helper dies on certain states is a worse door,
///   and the row keeps the code so a reader can see that is what happened.
///
/// The asymmetry is deliberate. A refusal that should have been a harness
/// failure costs a door one item in its denominator, which is visible in the
/// refusal column and correctable from the rows. A harness failure that
/// should have been a refusal removes the item from the record entirely, and
/// nothing downstream can tell it was ever asked.
#[must_use]
pub fn classify(error: &jev::Error) -> Disposition {
    match error {
        jev::Error::Api(api) => {
            classify_response(api.status, api.body.as_ref(), &error.to_string())
        }
        // No status and no body: the request never became a response.
        other => Disposition::Harness(other.to_string()),
    }
}

/// [`classify`] over one response, for a caller that holds the parts rather
/// than a [`jev::Error`].
#[must_use]
pub fn classify_response(
    status: u16,
    body: Option<&jev::ResponseBody>,
    detail: &str,
) -> Disposition {
    match body.and_then(refusal_code) {
        Some(code) => Disposition::Refused(RefusalCode::from(code)),
        None => Disposition::Harness(format!("HTTP {status}: {detail}")),
    }
}

/// The refusal code a body publishes, at `error.code` or at `code`.
///
/// Read as a field rather than searched for as a substring. The rule this
/// replaces scanned the debug rendering of the whole body for a known code,
/// so a door whose message mentioned a code in prose was recorded as having
/// refused with it.
fn refusal_code(body: &jev::ResponseBody) -> Option<String> {
    let jev::ResponseBody::Json(value) = body else {
        return None;
    };
    let code = value
        .get("error")
        .and_then(|error| error.get("code"))
        .or_else(|| value.get("code"))?;
    let code = code.as_str()?.trim();
    if code.is_empty() { None } else { Some(code.to_string()) }
}

/// Reads a typed answer as an option and a distribution over options.
///
/// A Noul is two options, `no` and `yes`, which is how a labelled suite
/// scores one. A Score's levels are named by their numbers.
#[must_use]
pub fn read_answer(answer: &jev::Answer) -> Disposition {
    let (chosen, distribution) = match answer {
        jev::Answer::Noul(noul) => {
            let yes = noul.noul;
            let chosen = if yes >= 0.5 { "yes" } else { "no" };
            let distribution: IndexMap<String, f64> =
                [("no".to_string(), 1.0 - yes), ("yes".to_string(), yes)].into_iter().collect();
            (chosen.to_string(), distribution)
        }
        jev::Answer::Choice(choice) => (choice.choice.clone(), choice.probabilities.clone()),
        jev::Answer::Score(score) => {
            let distribution: IndexMap<String, f64> = score
                .probabilities
                .iter()
                .map(|(level, probability)| (level.to_string(), *probability))
                .collect();
            let chosen = distribution
                .iter()
                .max_by(|left, right| left.1.total_cmp(right.1))
                .map(|(key, _)| key.clone())
                .unwrap_or_default();
            (chosen, distribution)
        }
    };
    Disposition::Answered { chosen, distribution }
}

/// What every row of one run shares: the suite, the door, and the estimator.
#[derive(Clone, Debug)]
pub struct Run {
    /// The suite's name.
    pub suite: String,
    /// The suite's content digest.
    pub suite_digest: String,
    /// The question set the run served, by id.
    pub question_set: Option<String>,
    /// That set's content digest, which every row of the run pins.
    pub question_digest: Option<String>,
    /// The door, by the name the run used for it.
    pub door: String,
    /// What that door is running, as far as it can be verified.
    pub door_identity: DoorIdentity,
    /// Which estimator produced the raw signal.
    pub estimator: String,
    /// How many draws one estimate rests on, when the door reports it.
    pub samples: Option<u64>,
    /// The seed block the door drew, when it reports one.
    pub seed_base: Option<u64>,
    /// When the run started, as an RFC 3339 timestamp in UTC.
    pub recorded_at: String,
    /// The acceptance rule the run judges under, when one is loaded.
    pub gate_id: Option<String>,
    /// That rule's content digest.
    pub gate_digest: Option<String>,
}

impl Run {
    /// Builds the row for one item, or nothing when the harness failed.
    ///
    /// The `None` is the rule from [`classify`] made structural: a harness
    /// failure has no row to write, so it cannot reach the store by accident.
    #[must_use]
    pub fn row(
        &self,
        item: &Item,
        permutation: Option<Vec<usize>>,
        disposition: &Disposition,
        latency_ms: Option<f64>,
    ) -> Option<Row> {
        let mut row = Row::new(&self.suite, &self.suite_digest, &item.id, &self.door);
        row.recorded_at = self.recorded_at.clone();
        row.question_set = self.question_set.clone();
        row.question_digest = self.question_digest.clone();
        row.split = item.partition.as_str().to_string();
        row.family = item.family.clone();
        row.door_identity = self.door_identity.clone();
        row.estimator = self.estimator.clone();
        row.samples = self.samples;
        row.seed_base = self.seed_base;
        row.permutation = permutation;
        row.latency_ms = latency_ms;
        row.gate_id = self.gate_id.clone();
        row.gate_digest = self.gate_digest.clone();
        // The item says what kind of evidence its label rests on, and the
        // row carries it. Without this a reader of the store cannot tell an
        // outcome-labelled result from a read one, and pooling them silently
        // is the fault the field exists to stop.
        row.label_source = item.evidence();
        match disposition {
            Disposition::Answered { chosen, distribution } => {
                Some(row.scored(distribution.clone(), *chosen == item.truth))
            }
            Disposition::Refused(code) => Some(row.refused(code.clone())),
            Disposition::Harness(_) => None,
        }
    }
}

/// The observations behind the scored rows, in row order.
///
/// A refused row contributes nothing to fit and nothing to score, and it is
/// still in the record. That is the arrangement under which a door cannot
/// improve its numbers by refusing what it finds hard: the refusal is visible
/// beside every average rather than folded into one.
#[must_use]
pub fn observations(rows: &[Row]) -> Vec<Observation> {
    rows.iter()
        .filter(|row| row.is_scored())
        .filter_map(|row| {
            Some(Observation::new(row.raw_top?, row.correct.unwrap_or(false)))
        })
        .collect()
}

/// The same observations with a map applied to each distribution.
///
/// The probability is the selected option's, and the selected option is the
/// raw estimator's argmax — the one [`Row::correct`] is about. It is not the
/// largest number in the rescaled distribution, which is a different
/// quantity whenever a map reads a signal below one half: there a runner-up
/// ends up above the selected option, and pairing its probability with the
/// selected option's outcome records a wrong answer at a confidence the door
/// never claimed for it. `crates/gym/src/calibrate.rs` carries the contract
/// and openagents#9438 the enumeration.
#[must_use]
pub fn mapped_observations(rows: &[Row], map: &Map) -> Vec<Observation> {
    rows.iter()
        .filter(|row| row.is_scored())
        .filter_map(|row| {
            let distribution = row.distribution.as_ref()?;
            let (selected, _) = crate::calibrate::selected(distribution)?;
            let mapped = map.apply_distribution(distribution);
            let probability = mapped.get(selected).copied()?;
            Some(Observation::new(probability, row.correct.unwrap_or(false)))
        })
        .collect()
}

/// How many rows each refusal code accounts for.
#[must_use]
pub fn refusals(rows: &[Row]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for row in rows.iter().filter(|row| row.is_refused()) {
        if let Some(code) = &row.refusal {
            *counts.entry(code.label().to_string()).or_insert(0) += 1;
        }
    }
    counts
}

/// The families these rows cover, in first-seen order.
#[must_use]
pub fn families(rows: &[Row]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for row in rows {
        if !out.contains(&row.family) {
            out.push(row.family.clone());
        }
    }
    out
}

/// The rows of one family, borrowed in row order.
#[must_use]
pub fn of_family<'a>(rows: &'a [Row], family: &str) -> Vec<&'a Row> {
    rows.iter().filter(|row| row.family == family).collect()
}

/// One family's map, the numbers on both sides of it, and the gate's verdict.
#[derive(Clone, Debug)]
pub struct Fit {
    /// The question family.
    pub family: String,
    /// The fitted table.
    pub map: Map,
    /// The scores on the held-out partition, before the map.
    pub raw: Metrics,
    /// The scores on the held-out partition, after it.
    pub calibrated: Metrics,
    /// The comparison the gate judged.
    pub comparison: Comparison,
    /// What the gate concluded.
    pub outcome: Outcome,
}

impl Fit {
    /// Whether the gate let this map serve.
    #[must_use]
    pub fn admitted(&self) -> bool {
        self.outcome.verdict == crate::gate::Verdict::Passed
    }

    /// The gate's verdict in one line, with the numbers behind it.
    ///
    /// A verdict that did not pass names the criterion that decided it,
    /// because that is the one thing a reader has to act on. A verdict that
    /// passed names the measures the rule ranks first instead: "passed" over
    /// a floor criterion says nothing about whether the map is any good.
    #[must_use]
    pub fn verdict(&self) -> String {
        if self.outcome.verdict != crate::gate::Verdict::Passed {
            return match self.outcome.deciding() {
                Some(criterion) => {
                    format!("{}: {} ({})", self.outcome.verdict, criterion.name, criterion.detail)
                }
                None => self.outcome.verdict.to_string(),
            };
        }
        let decisive: Vec<String> = self
            .outcome
            .criteria
            .iter()
            .filter(|criterion| criterion.rank == 1 && !criterion.name.contains(">="))
            .map(|criterion| criterion.detail.clone())
            .collect();
        if decisive.is_empty() {
            self.outcome.verdict.to_string()
        } else {
            format!("passed: {}", decisive.join(", "))
        }
    }
}

/// Fits one family's map on one set of rows and judges it on another.
///
/// The two sets never share an item: fitting a map and scoring it on the same
/// items produces a number that means nothing. The caller passes the
/// partitions, because which partition is spent on what is the suite's
/// decision and not this function's.
#[must_use]
pub fn fit_family(family: &str, fit_on: &[Row], score_on: &[Row], gate: &Gate) -> Fit {
    let map = Map::fit_auto(&observations(fit_on));
    let raw = score(&observations(score_on));
    let calibrated = score(&mapped_observations(score_on, &map));
    let comparison = Comparison::new(family, raw.scores(), calibrated.scores())
        .fitted_on(map.fitted_on);
    let outcome = gate.judge(&comparison);
    Fit {
        family: family.to_string(),
        map,
        raw,
        calibrated,
        comparison,
        outcome,
    }
}

/// A Choice question's options, in the order the suite serves them.
///
/// `None` for anything else. A Noul's two options and a Score's ordered
/// levels carry meaning in their order, so permuting them asks a different
/// question rather than the same one differently.
#[must_use]
pub fn options_of(question: &Value) -> Option<Vec<String>> {
    if question.get("type").and_then(Value::as_str) != Some("choice") {
        return None;
    }
    let criteria = question.get("criteria")?.as_object()?;
    Some(criteria.keys().cloned().collect())
}

/// The same question with its options served in `order`.
///
/// `order` indexes the suite's own order, which is what [`Row::permutation`]
/// records, so a row and the request it describes cannot drift apart.
#[must_use]
pub fn permuted(question: &Value, order: &[usize]) -> Option<Value> {
    let criteria = question.get("criteria")?.as_object()?;
    let keys: Vec<String> = criteria.keys().cloned().collect();
    if order.len() != keys.len() {
        return None;
    }
    let mut reordered = serde_json::Map::new();
    for index in order {
        let key = keys.get(*index)?;
        reordered.insert(key.clone(), criteria.get(key)?.clone());
    }
    let mut question = question.clone();
    question.as_object_mut()?.insert("criteria".to_string(), Value::Object(reordered));
    Some(question)
}

/// The suite's order, reversed.
#[must_use]
pub fn reversed(count: usize) -> Vec<usize> {
    (0..count).rev().collect()
}

/// The current time as an RFC 3339 timestamp in UTC, to the second.
///
/// Written here rather than taken from a date library because the repository
/// has none and a row needs one field.
#[must_use]
pub fn now_utc() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or_default();
    utc_from_unix(seconds)
}

/// The civil date and time of a Unix timestamp, by Howard Hinnant's
/// days-from-civil inverse.
#[must_use]
pub fn utc_from_unix(seconds: u64) -> String {
    let days = (seconds / 86_400) as i64;
    let time = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        time / 3600,
        (time % 3600) / 60,
        time % 60
    )
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * shifted_month + 2) / 5 + 1) as u32;
    let month = if shifted_month < 10 { shifted_month + 3 } else { shifted_month - 9 } as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gate;
    use crate::suite::Partition;
    use serde_json::json;

    fn refused_with(status: u16, body: Value) -> Disposition {
        classify_response(status, Some(&jev::ResponseBody::Json(body)), "the door refused")
    }

    fn item(id: &str, truth: &str) -> Item {
        Item {
            id: id.to_string(),
            family: "routing".to_string(),
            kind: "choice".to_string(),
            state: json!("a message"),
            question: Some(json!({
                "type": "choice",
                "instructions": "Which team?",
                "criteria": { "billing": "money", "technical": "bugs", "sales": "plans" },
            })),
            truth: truth.to_string(),
            partition: Partition::Development,
            label_source: None,
            label_rule: None,
        }
    }

    fn run() -> Run {
        Run {
            suite: "support-v2-three-way".to_string(),
            suite_digest: "sha256:abc".to_string(),
            question_set: Some("support-v2-three-way-v1".to_string()),
            question_digest: Some("sha256:questions".to_string()),
            door: "lev".to_string(),
            door_identity: DoorIdentity::published("lev-base", "sig:base-1", ""),
            estimator: "l2".to_string(),
            samples: Some(8),
            seed_base: Some(0),
            recorded_at: "2026-09-19T12:00:00Z".to_string(),
            gate_id: Some("probability-v1".to_string()),
            gate_digest: Some("gate:abc".to_string()),
        }
    }

    fn answered(chosen: &str) -> Disposition {
        Disposition::Answered {
            chosen: chosen.to_string(),
            distribution: [
                ("billing".to_string(), 0.75),
                ("technical".to_string(), 0.15),
                ("sales".to_string(), 0.10),
            ]
            .into_iter()
            .collect(),
        }
    }

    #[test]
    fn a_typed_refusal_is_the_door_declining_whatever_the_code_is() {
        // The three that used to disagree. All of them reached the door.
        for code in ["guardrail", "busy", "model_unavailable", "bridge_error"] {
            let refused = refused_with(503, json!({ "error": { "code": code, "message": "no" } }));
            assert_eq!(
                refused,
                Disposition::Refused(RefusalCode::from(code.to_string())),
                "{code} is the door's own answer"
            );
        }
    }

    #[test]
    fn an_unnamed_code_is_kept_rather_than_bucketed() {
        let refused = refused_with(451, json!({ "error": { "code": "policy_withheld" } }));
        assert_eq!(
            refused,
            Disposition::Refused(RefusalCode::Other("policy_withheld".to_string()))
        );
    }

    #[test]
    fn a_failure_with_no_code_is_the_harness_and_writes_no_row() {
        let reset = jev::Error::Connection {
            message: "connection reset".to_string(),
            source: None,
        };
        assert!(matches!(classify(&reset), Disposition::Harness(_)));

        // A 502 from something in front of the door is not a refusal either.
        let disposition = classify_response(
            502,
            Some(&jev::ResponseBody::Text("<html>bad gateway</html>".to_string())),
            "bad gateway",
        );
        assert!(matches!(disposition, Disposition::Harness(_)));
        assert!(!disposition.is_recorded());
        assert!(run().row(&item("routing/000", "billing"), None, &disposition, None).is_none());
    }

    #[test]
    fn a_code_mentioned_in_prose_is_not_a_refusal() {
        // The rule this replaces scanned the whole body for a known code.
        let scanned = refused_with(
            500,
            json!({ "error": { "message": "the upstream said model_unavailable earlier" } }),
        );
        assert!(
            matches!(scanned, Disposition::Harness(_)),
            "a code has to be the code field, not a word in the message"
        );
    }

    #[test]
    fn a_refused_row_stays_in_the_denominator_with_no_score() {
        let asked = item("routing/000", "billing");
        let refused = run()
            .row(&asked, None, &Disposition::Refused(RefusalCode::Guardrail), Some(12.0))
            .expect("a refusal is recorded");
        refused.check().expect("the row is coherent");
        assert!(refused.is_refused());
        assert_eq!(refused.correct, None, "a refusal is not a wrong answer");
        assert_eq!(refused.raw_top, None);
        assert_eq!(refused.family, "routing");
        assert_eq!(refused.split, "development");

        let rows = vec![refused];
        assert!(observations(&rows).is_empty(), "a refusal scores nothing");
        assert_eq!(refusals(&rows).get("guardrail"), Some(&1));
    }

    #[test]
    fn a_scored_row_carries_the_verdict_and_the_whole_distribution() {
        let asked = item("routing/000", "billing");
        let row = run()
            .row(&asked, Some(vec![2, 1, 0]), &answered("billing"), Some(2100.0))
            .expect("an answer is recorded");
        row.check().expect("the row is coherent");
        assert_eq!(row.correct, Some(true));
        assert_eq!(row.raw_top, Some(0.75));
        assert_eq!(row.permutation, Some(vec![2, 1, 0]));
        assert_eq!(row.seed_base, Some(0));
        assert_eq!(row.gate_id.as_deref(), Some("probability-v1"));

        let wrong = run()
            .row(&item("routing/001", "sales"), None, &answered("billing"), None)
            .expect("an answer is recorded");
        assert_eq!(wrong.correct, Some(false));
        assert_eq!(wrong.latency_ms, None, "an unmeasured latency stays unknown");
    }

    #[test]
    fn the_table_is_computed_from_the_rows() {
        let rows: Vec<Row> = vec![
            run().row(&item("a", "billing"), None, &answered("billing"), None).unwrap(),
            run().row(&item("b", "sales"), None, &answered("billing"), None).unwrap(),
            run()
                .row(&item("c", "billing"), None, &Disposition::Refused(RefusalCode::Busy), None)
                .unwrap(),
        ];
        let metrics = score(&observations(&rows));
        assert_eq!(metrics.items, 2, "the refusal is not scored");
        assert!((metrics.accuracy - 0.5).abs() < 1e-12);
        assert_eq!(families(&rows), vec!["routing".to_string()]);
        assert_eq!(of_family(&rows, "routing").len(), 3, "the refusal is still in the record");
    }

    #[test]
    fn a_family_is_fitted_on_one_partition_and_judged_on_another() {
        let gate = gate::load("probability-v1").expect("the committed gate loads");
        let mut fit_rows = Vec::new();
        let mut score_rows = Vec::new();
        for index in 0..40 {
            let truth = if index % 4 == 0 { "sales" } else { "billing" };
            let fitting = item(&format!("fit/{index}"), truth);
            fit_rows.push(run().row(&fitting, None, &answered("billing"), None).unwrap());
            let scoring = item(&format!("score/{index}"), truth);
            score_rows.push(run().row(&scoring, None, &answered("billing"), None).unwrap());
        }
        let fit = fit_family("routing", &fit_rows, &score_rows, &gate);
        assert_eq!(fit.map.fitted_on, 40);
        assert_eq!(fit.raw.items, 40);
        assert_eq!(fit.calibrated.items, 40);
        assert_eq!(fit.outcome.gate_id, "probability-v1");
        assert_eq!(fit.outcome.gate_digest, gate.digest());
        // The estimator reported 0.75 for every item and three quarters of
        // them were right, so the raw signal is already well calibrated and a
        // map has nothing to win.
        assert!(fit.raw.ece < 0.05, "raw ECE {}", fit.raw.ece);
        assert!(!fit.verdict().is_empty());
    }

    #[test]
    fn only_a_choice_has_an_order_to_permute() {
        let question = item("a", "billing").question.expect("the test item carries its text");
        assert_eq!(
            options_of(&question),
            Some(vec!["billing".to_string(), "technical".to_string(), "sales".to_string()])
        );
        assert_eq!(options_of(&json!({ "type": "noul" })), None);
        assert_eq!(options_of(&json!({ "type": "score", "criteria": { "1": "low" } })), None);
    }

    #[test]
    fn a_permutation_reorders_the_options_and_nothing_else() {
        let question = item("a", "billing").question.expect("the test item carries its text");
        let order = reversed(3);
        assert_eq!(order, vec![2, 1, 0]);
        let backward = permuted(&question, &order).expect("a choice permutes");
        assert_eq!(
            options_of(&backward),
            Some(vec!["sales".to_string(), "technical".to_string(), "billing".to_string()])
        );
        assert_eq!(
            backward.get("instructions"),
            question.get("instructions"),
            "only the order moved"
        );
        assert_eq!(
            backward.get("criteria").and_then(|c| c.get("sales")),
            question.get("criteria").and_then(|c| c.get("sales")),
            "each option kept its description"
        );
        assert_eq!(permuted(&question, &[0, 1]), None, "a short order is not a permutation");
    }

    #[test]
    fn every_row_of_a_run_pins_the_question_set_it_served() {
        let row = run()
            .row(&item("a", "billing"), None, &answered("billing"), None)
            .expect("an answered item produces a row");
        assert_eq!(row.question_set.as_deref(), Some("support-v2-three-way-v1"));
        assert_eq!(row.question_digest.as_deref(), Some("sha256:questions"));
    }

    #[test]
    fn a_timestamp_reads_as_rfc_3339_in_utc() {
        assert_eq!(utc_from_unix(0), "1970-01-01T00:00:00Z");
        assert_eq!(utc_from_unix(1_789_819_200), "2026-09-19T12:00:00Z");
        // A leap day, because the arithmetic is the only part of this that
        // can be wrong quietly.
        assert_eq!(utc_from_unix(951_868_799), "2000-02-29T23:59:59Z");
        let now = now_utc();
        assert!(now.ends_with('Z') && now.len() == 20, "{now}");
    }
}
