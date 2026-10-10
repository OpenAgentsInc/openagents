//! Calibration of the router's probabilities (#9959).
//!
//! The router's `route` and `answer` readings are Jev's Choice
//! probabilities, and the policy table ([`super::policy`]) serves through
//! fixed thresholds on them. Whether those probabilities mean what they
//! say is a claim the eval measures: the live eval
//! (`crates/coder/tests/router_eval.rs`, `ROUTER_EVAL_PUBLISH=1`) fits one
//! [`Map`] per question on the labeled set's calibration partition
//! ([`crate::router_eval::partition_of`]), scores the raw and the
//! calibrated probabilities on the held-out split (ECE and Brier,
//! `gym::calibrate::score`), asks the `probability-v2` gate whether the map
//! may be served, and writes the result here as
//! `crates/coder/fixtures/chat-router/calibration-v2.json` ([`FIXTURE`]).
//!
//! Serving the map is on by default since #10386:
//! `CODER_WORKER_ROUTER_CALIBRATION` ([`VAR`]) is `on` unless set to
//! `off`. Only maps that passed their held-out gate serve (today `answer`;
//! `route` failed), and a mapped `answer` is read against the cost-derived
//! [`super::thresholds::CALIBRATED_ANSWER_CONFIDENCE`] rather than the raw
//! threshold it was tuned for. With it on, the worker applies [`Calibration::apply`] to each
//! reading before the policy decides, refuses to start when the map was
//! fitted for another question set ([`Calibration::check`]), and names the
//! map in every `router` log line.

use gym::calibrate::{Map, Metrics, Observation};
use serde::{Deserialize, Serialize};

use super::judge::Routing;
use super::{RouteId, set_id};

/// The fixture's schema.
pub const SCHEMA: &str = "openagents.chat-router.calibration.v1";

/// The environment variable that turns the map off: `on` (the default since
/// #10386), or `off`.
pub const VAR: &str = "CODER_WORKER_ROUTER_CALIBRATION";

/// The committed map, fitted by the last published eval.
pub const FIXTURE: &str = include_str!("../../fixtures/chat-router/calibration-v2.json");

/// The probability at or above which a reading counts as sure, for the
/// operating-point table.
pub const SURE: f64 = 0.9;

/// One question's fitted map and how it scored on the held-out split.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Question {
    /// The map, fitted on the calibration partition.
    pub map: Map,
    /// The held-out rows with a reading for this question.
    pub held_out_items: usize,
    /// The raw probabilities' scores on the held-out split.
    pub raw: Metrics,
    /// The mapped probabilities' scores on the same rows.
    pub calibrated: Metrics,
    /// The `probability-v2` gate's verdict on serving the map in place of
    /// the raw signal (`passed`, `unverifiable`, or `failed`).
    pub verdict: String,
    /// That gate's digest.
    pub gate: String,
}

impl Question {
    /// Whether the map passed its held-out gate and may serve.
    #[must_use]
    pub fn serves(&self) -> bool {
        self.verdict == "passed"
    }

    /// Fits a map on `fit` and scores it on `held_out`, judged by `gate`.
    #[must_use]
    pub fn fit(fit: &[Observation], held_out: &[Observation], gate: &gym::gate::Gate) -> Self {
        let map = Map::fit_auto(fit);
        let calibrated: Vec<Observation> = held_out
            .iter()
            .map(|o| Observation::new(map.apply(o.raw), o.correct))
            .collect();
        let raw = gym::calibrate::score(held_out);
        let mapped = gym::calibrate::score(&calibrated);
        let outcome = gate.judge(
            &gym::gate::Comparison::new("held_out", raw.scores(), mapped.scores())
                .fitted_on(fit.len()),
        );
        Self {
            map,
            held_out_items: held_out.len(),
            raw,
            calibrated: mapped,
            verdict: outcome.verdict.as_str().to_string(),
            gate: outcome.gate_digest,
        }
    }
}

/// The router's calibration record: one map per question, pinned to the
/// question set and bank they were fitted against.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Calibration {
    pub schema: String,
    /// The question set the maps were fitted for ([`set_id`]).
    pub set: String,
    /// The bank whose entries the `answer` question offered
    /// ([`super::Bank::id`]).
    pub bank: String,
    /// The day the eval ran, `YYYY-MM-DD`.
    pub created: String,
    /// The partition the maps were fitted on, and its rows.
    pub fitted_on: String,
    pub fitted_rows: usize,
    pub route: Question,
    pub answer: Question,
}

impl Calibration {
    /// Parses a calibration record.
    ///
    /// # Errors
    ///
    /// The JSON error, or a record of another schema.
    pub fn parse(source: &str) -> Result<Self, String> {
        let record: Self =
            serde_json::from_str(source).map_err(|error| format!("calibration: {error}"))?;
        if record.schema != SCHEMA {
            return Err(format!(
                "calibration: schema {} is not {SCHEMA}",
                record.schema
            ));
        }
        Ok(record)
    }

    /// The committed record.
    ///
    /// # Errors
    ///
    /// When the fixture does not parse, which the tests rule out.
    pub fn builtin() -> Result<Self, String> {
        Self::parse(FIXTURE)
    }

    /// The map from [`VAR`]: `Some` when it is `on` or unset, `None` when
    /// it is `off`.
    ///
    /// # Errors
    ///
    /// Another value, a fixture that does not parse, or a map fitted for
    /// another question set than this build asks
    /// ([`Calibration::check`]).
    pub fn from_env(bank: &str) -> Result<Option<Self>, String> {
        let value = std::env::var(VAR).ok();
        Self::from_setting(value.as_deref(), bank)
    }

    /// [`Calibration::from_env`] for a value of [`VAR`].
    ///
    /// # Errors
    ///
    /// As [`Calibration::from_env`].
    pub fn from_setting(value: Option<&str>, bank: &str) -> Result<Option<Self>, String> {
        match value.map(str::trim) {
            Some("on") => {
                let record = Self::builtin()?;
                record.check(&set_id(), bank)?;
                Ok(Some(record))
            }
            // The default: serve the map when it was fitted for this
            // build's question set, and the raw readings when it was not
            // (a question-set change must not stop the worker; the
            // published eval refits the map).
            Some("") | None => {
                let record = Self::builtin()?;
                Ok(record.check(&set_id(), bank).is_ok().then_some(record))
            }
            Some("off") => Ok(None),
            Some(other) => Err(format!("{VAR} is on or off, not `{other}`")),
        }
    }

    /// Whether the record may serve with question set `set` and bank
    /// `bank`.
    ///
    /// # Errors
    ///
    /// A record fitted for another question set: the `route` question it
    /// mapped is not the one this build asks. A bank that moved is
    /// allowed, since the `answer` question's shape is the bank's entry
    /// list and a reviewed text edit does not change it; the record still
    /// names the bank it was fitted with so a reader can see the drift.
    pub fn check(&self, set: &str, bank: &str) -> Result<(), String> {
        if self.set != set {
            return Err(format!(
                "calibration: the map was fitted for {} and this build asks {set}; \
                 rerun the published eval to refit it",
                self.set
            ));
        }
        let _ = bank;
        Ok(())
    }

    /// Whether the bank the record was fitted with is `bank`.
    #[must_use]
    pub fn fitted_with(&self, bank: &str) -> bool {
        self.bank == bank
    }

    /// Maps the reading's `route` and `answer` probabilities through the
    /// fitted tables. The argmax is untouched: a map rescales the winner's
    /// probability and never picks another option. A reading with no route
    /// (`Unknown`) or no answer keeps its zero.
    ///
    /// Only a map whose held-out verdict is `passed` serves (#10386): the
    /// `route` map failed its gate in `calibration-v2`, so `route` stays
    /// raw and keeps [`super::policy::ROUTE_CONFIDENCE`]; the `answer` map
    /// passed, so `answer` is mapped and marked
    /// ([`Routing::answer_calibrated`]) for the calibrated threshold.
    pub fn apply(&self, routing: &mut Routing) {
        if self.route.serves() && routing.route != RouteId::Unknown {
            routing.route_p = self.route.map.apply(routing.route_p).clamp(0.0, 1.0);
        }
        if self.answer.serves()
            && let Some((_, p)) = &mut routing.answer
        {
            *p = self.answer.map.apply(*p).clamp(0.0, 1.0);
            routing.answer_calibrated = true;
        }
    }

    /// The record's identity for a log line: `calibration-v2@<map digest>`,
    /// twelve hex digits of SHA-256 over the two maps' canonical JSON.
    #[must_use]
    pub fn id(&self) -> String {
        use sha2::Digest as _;
        let maps = serde_json::json!({ "route": self.route.map, "answer": self.answer.map });
        let digest = sha2::Sha256::digest(maps.to_string().as_bytes());
        let hex: String = digest.iter().take(6).map(|b| format!("{b:02x}")).collect();
        format!("calibration-v2@{hex}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::router::Bank;

    fn fit(pairs: &[(f64, bool)]) -> Vec<Observation> {
        pairs
            .iter()
            .map(|(p, c)| Observation::new(*p, *c))
            .collect()
    }

    /// The committed record parses, names this build's question set, and
    /// its maps are fitted on enough rows to be more than noise.
    #[test]
    fn the_committed_record_is_this_builds() {
        let record = Calibration::builtin().expect("the fixture parses");
        assert_eq!(record.schema, SCHEMA);
        record
            .check(&set_id(), &Bank::builtin().id())
            .expect("fitted for the question set this build asks");
        assert!(record.fitted_rows >= 30, "{}", record.fitted_rows);
        assert!(record.route.map.fitted_on >= 30);
        assert!(record.answer.map.fitted_on >= 30);
        assert!(record.route.held_out_items >= 30);
        for question in [&record.route, &record.answer] {
            assert!(
                matches!(
                    question.verdict.as_str(),
                    "passed" | "unverifiable" | "failed"
                ),
                "{}",
                question.verdict
            );
            assert!(question.gate.starts_with("gate:"));
        }
        assert!(record.id().starts_with("calibration-v2@"));
    }

    /// A record fitted for another question set refuses to serve.
    #[test]
    fn a_record_for_another_question_set_refuses() {
        let mut record = Calibration::builtin().expect("the fixture parses");
        record.set = "chat-router-v2@000000000000".into();
        let refused = record.check(&set_id(), &Bank::builtin().id());
        assert!(refused.is_err(), "{refused:?}");
        let bank = Bank::builtin().id();
        assert!(Calibration::from_setting(Some("on"), &bank).is_ok_and(|c| c.is_some()));
        assert!(Calibration::from_setting(Some("off"), &bank).is_ok_and(|c| c.is_none()));
        // On by default since #10386.
        assert!(Calibration::from_setting(None, &bank).is_ok_and(|c| c.is_some()));
        assert!(Calibration::from_setting(Some("maybe"), &bank).is_err());
    }

    /// The map rescales the winner's probability and leaves the argmax
    /// and every other reading alone.
    #[test]
    fn applying_the_map_moves_only_the_probabilities() {
        let gate = gym::gate::load("probability-v2").expect("the gate loads");
        let sure: Vec<(f64, bool)> = (0..40)
            .map(|n| (0.95, n % 10 != 0))
            .chain((0..40).map(|n| (0.55, n % 2 == 0)))
            .collect();
        let mut question = Question::fit(&fit(&sure), &fit(&sure), &gate);
        // Only a map that passed its gate serves (#10386); this test is
        // about what serving one does.
        question.verdict = "passed".into();
        let mut record = Calibration {
            schema: SCHEMA.into(),
            set: set_id(),
            bank: Bank::builtin().id(),
            created: "2026-09-29".into(),
            fitted_on: "calibration".into(),
            fitted_rows: 80,
            route: question.clone(),
            answer: question,
        };
        let bank = Bank::builtin();
        let mut routing = Routing {
            action: crate::classify::Route::Respond,
            route: RouteId::Meta,
            route_p: 0.95,
            runner_up: Some((RouteId::General, 0.03)),
            clarify_p: 0.0,
            answer: bank.entry("meta.model").map(|entry| (entry.clone(), 0.55)),
            needs_specifics: 0.07,
            lane: crate::first::Lane::Chat,
            lane_p: 0.97,
            opener: None,
            cli_group: None,
            cli_alternatives: Vec::new(),
            tool: None,
            capability: None,
            capability_missing_p: 0.0,
            capability_closest: None,
            deck: None,
            repository: None,
            engine: None,
            fanout: None,
            read_only: 0.0,
            summarize: 0.0,
            risk: crate::router::Risk::Ok,
            risk_p: 0.99,
            answer_calibrated: false,
        };
        record.apply(&mut routing);
        assert_eq!(routing.route, RouteId::Meta);
        assert!((routing.route_p - 0.9).abs() < 0.02, "{}", routing.route_p);
        let (entry, p) = routing.answer.as_ref().unwrap();
        assert_eq!(entry.id, "meta.model");
        assert!((p - 0.5).abs() < 0.03, "{p}");
        assert_eq!(routing.runner_up, Some((RouteId::General, 0.03)));
        assert!((routing.lane_p - 0.97).abs() < f64::EPSILON);

        let mut unknown = routing.clone();
        unknown.route = RouteId::Unknown;
        unknown.route_p = 0.0;
        unknown.answer = None;
        record.apply(&mut unknown);
        assert!(unknown.route_p.abs() < f64::EPSILON);
        assert!(unknown.answer.is_none());
        assert!(routing.answer_calibrated);

        // A map that failed its gate leaves its reading raw.
        record.route.verdict = "failed".into();
        let raw = 0.95;
        let mut again = routing.clone();
        again.route_p = raw;
        again.answer_calibrated = false;
        record.apply(&mut again);
        assert!((again.route_p - raw).abs() < 1e-12);
        assert!(again.answer_calibrated);
    }
}
