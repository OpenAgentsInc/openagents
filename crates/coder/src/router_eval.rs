//! The chat router's labeled route set and the numbers a run over it
//! reports.
//!
//! The set (`crates/coder/fixtures/chat-router/routes-v5.json`, the
//! `routes-v4.json` rows plus rows for `standing.rule` and its near
//! misses; those the `routes-v3.json` rows with the same ids and splits, themselves the
//! `routes-v2.json` rows plus rows for `capability.missing` and its near
//! misses, themselves the `routes-v1.json` rows plus the Gym and eval
//! routes, plus rows for `presentation.open` and its near misses) holds
//! realistic first messages across the 21 routes of
//! `docs/coder/design/2026-09-28-chat-router.md`, each labeled with the
//! route, the prepared answer a correct router serves (or none), other
//! acceptable answers, the tier, the risk, and the `openagents` command
//! group for a CLI request. A fixed 30 % is held out ([`split_of`]): those
//! rows are never used to pick thresholds or tune `when` text.
//!
//! A system under test turns each row into a [`Reading`] (what it would
//! do); [`Report::of`] scores the readings against the labels with the
//! design's metrics:
//!
//! - per-route precision and recall;
//! - **canned precision**: of the turns served a prepared answer whole
//!   (tier `canned`), the share whose row expects that tier and lists that
//!   answer. Target at least 98 %.
//! - **canned coverage**: of the rows that expect a canned answer, the
//!   share served one, right or wrong;
//! - **dispatch precision**: of the turns given a Coder offer, the share
//!   whose row is `work.dispatch`. Target at least 90 %.
//! - **Gym precision**: of the turns given a Gym or interview tier (`gym`,
//!   `author`), the share whose row is on that route.
//! - refusal precision and recall, secret recall, and latency.
//!
//! This module is pure: the live runs are the ignored tests in
//! `crates/coder/tests/router_eval.rs`, which ask the systems and hand the
//! readings here.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// The fixture's schema.
pub const SCHEMA: &str = "openagents.chat-router.labeled.v1";

/// The routes, in the design's catalog order.
pub const ROUTES: [&str; 21] = [
    "meta",
    "smalltalk",
    "general",
    "product.kb",
    "codebase.kb",
    "work.dispatch",
    "cli",
    "wallet",
    "account",
    "clarify",
    "end",
    "refuse",
    "gym.news",
    "eval.run",
    "eval.author",
    "eval.check",
    "eval.result",
    "eval.credit",
    "capability.missing",
    "presentation.open",
    "standing.rule",
];

/// The tiers a row can expect: the design's `Tier` enum, by word. `gym` is
/// a reply decided from the Gym's verified records, `author` a step of the
/// authoring interview.
pub const TIERS: [&str; 8] = [
    "canned", "stem", "grounded", "model", "offer", "refuse", "gym", "author",
];

/// The risk readings.
pub const RISKS: [&str; 5] = [
    "ok",
    "secret_shared",
    "asks_for_secret",
    "harmful",
    "money_movement",
];

/// Canned precision's target.
pub const CANNED_TARGET: f64 = 0.98;

/// Dispatch precision's target.
pub const DISPATCH_TARGET: f64 = 0.90;

/// The share of rows held out, in percent.
pub const HELD_OUT_PERCENT: u32 = 30;

/// The checked-in set.
pub const FIXTURE: &str = include_str!("../fixtures/chat-router/routes-v5.json");

/// The `chat-router-v4` set, kept for the Gym suite recorded under it.
pub const FIXTURE_V4: &str = include_str!("../fixtures/chat-router/routes-v4.json");

/// The `chat-router-v3` set, kept for the Gym suite recorded under it.
pub const FIXTURE_V3: &str = include_str!("../fixtures/chat-router/routes-v3.json");

/// The `chat-router-v2` set, kept for the Gym suite recorded under it.
pub const FIXTURE_V2: &str = include_str!("../fixtures/chat-router/routes-v2.json");

/// The `chat-router-v1` set, kept for the Gym suite recorded under it.
pub const FIXTURE_V1: &str = include_str!("../fixtures/chat-router/routes-v1.json");

/// One message of a row's conversation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Said {
    pub role: String,
    pub text: String,
}

/// One labeled row.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Row {
    pub id: String,
    /// The conversation; the last message is the user's.
    pub messages: Vec<Said>,
    pub route: String,
    /// The prepared answer a correct router serves, if any.
    pub answer: Option<String>,
    /// Other prepared answers that are also right.
    #[serde(default)]
    pub also: Vec<String>,
    pub tier: String,
    pub risk: String,
    pub cli_group: Option<String>,
    /// For an `eval.run` row, the catalog tool its message names, or
    /// `None` when it names none and the default tool is right.
    #[serde(default)]
    pub tool: Option<String>,
    /// For a `work.dispatch` row, the coding engine its message asks for,
    /// by its NIP-CJ word (`claude_code`), or `None` when it asks for none
    /// (#10076). A right dispatch offer names exactly it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub split: String,
}

impl Row {
    /// The user's latest message.
    #[must_use]
    pub fn latest(&self) -> &str {
        self.messages.last().map_or("", |m| m.text.as_str())
    }

    /// Whether `answer` is right for this row.
    #[must_use]
    pub fn accepts(&self, answer: &str) -> bool {
        self.answer.as_deref() == Some(answer) || self.also.iter().any(|a| a == answer)
    }

    /// Whether the row is held out.
    #[must_use]
    pub fn held_out(&self) -> bool {
        self.split == "held_out"
    }
}

/// The labeled set.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Set {
    pub schema: String,
    pub set: String,
    pub created: String,
    pub rows: Vec<Row>,
}

impl Set {
    /// The checked-in set.
    ///
    /// # Panics
    ///
    /// The fixture does not parse, which its tests rule out.
    #[must_use]
    pub fn fixture() -> Self {
        serde_json::from_str(FIXTURE).expect("the route set parses")
    }

    /// The `chat-router-v1` set.
    ///
    /// # Panics
    ///
    /// The fixture does not parse, which its tests rule out.
    #[must_use]
    pub fn v1() -> Self {
        serde_json::from_str(FIXTURE_V1).expect("the v1 route set parses")
    }

    /// The `chat-router-v2` set.
    ///
    /// # Panics
    ///
    /// Never for the checked-in file; a test checks it.
    #[must_use]
    pub fn v2() -> Self {
        serde_json::from_str(FIXTURE_V2).expect("the v2 route set parses")
    }

    /// The `chat-router-v4` set.
    ///
    /// # Panics
    ///
    /// Never for the checked-in file; a test checks it.
    #[must_use]
    pub fn v4() -> Self {
        serde_json::from_str(FIXTURE_V4).expect("the v4 route set parses")
    }

    /// The `chat-router-v3` set.
    ///
    /// # Panics
    ///
    /// Never for the checked-in file; a test checks it.
    #[must_use]
    pub fn v3() -> Self {
        serde_json::from_str(FIXTURE_V3).expect("the v3 route set parses")
    }

    /// The rows of `split` (`tune`, `held_out`, or `all`).
    #[must_use]
    pub fn rows(&self, split: &str) -> Vec<&Row> {
        self.rows
            .iter()
            .filter(|r| split == "all" || r.split == split)
            .collect()
    }
}

/// The split a row id falls in: `held_out` when the first four bytes of
/// SHA-256 of the id, as a big-endian integer, modulo 100 are below
/// [`HELD_OUT_PERCENT`].
#[must_use]
pub fn split_of(id: &str) -> &'static str {
    let digest = knowledge::digest(id.as_bytes());
    let hex = digest.trim_start_matches("sha256:");
    let head = u32::from_str_radix(&hex[..8], 16).unwrap_or(0);
    if head % 100 < HELD_OUT_PERCENT {
        "held_out"
    } else {
        "tune"
    }
}

/// What a system did with one row.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Reading {
    pub id: String,
    /// The route it chose, or `None` when it has no route reading.
    pub route: Option<String>,
    pub route_p: f64,
    /// The prepared answer it served or would stem from.
    pub answer: Option<String>,
    /// The `answer` question's argmax entry, served or not, whose
    /// probability `answer_p` is: the reading the calibration map is
    /// fitted on.
    #[serde(default)]
    pub read_answer: Option<String>,
    pub answer_p: f64,
    /// The tier it chose, by [`TIERS`] word.
    pub tier: String,
    /// The risk it read, when it reads one.
    pub risk: Option<String>,
    /// Whether it offered to dispatch Coder.
    pub dispatch: bool,
    /// Milliseconds the decision took.
    pub ms: u128,
    /// Why the system gave no reading, when it failed.
    pub error: Option<String>,
}

/// Precision and recall counts for one label.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct Counts {
    /// Rows with this label that got it.
    pub hit: usize,
    /// Readings with this label.
    pub predicted: usize,
    /// Rows with this label.
    pub labeled: usize,
}

impl Counts {
    #[must_use]
    pub fn precision(&self) -> Option<f64> {
        ratio(self.hit, self.predicted)
    }

    #[must_use]
    pub fn recall(&self) -> Option<f64> {
        ratio(self.hit, self.labeled)
    }
}

fn ratio(a: usize, b: usize) -> Option<f64> {
    (b > 0).then(|| a as f64 / b as f64)
}

/// A scored run.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Report {
    pub system: String,
    pub split: String,
    pub rows: usize,
    pub errors: usize,
    /// Per-route counts, in [`ROUTES`] order.
    pub routes: BTreeMap<String, Counts>,
    /// Rows whose route reading is right, over rows with a route reading.
    pub route_accuracy: Option<f64>,
    /// Canned: served whole, right; served whole; rows expecting canned.
    pub canned: Counts,
    /// Canned answers served to rows whose own answer is right but which
    /// expect a stem (the user named specifics).
    pub canned_over_stem: usize,
    /// Dispatch offers: to `work.dispatch` rows; offers; `work.dispatch` rows.
    pub dispatch: Counts,
    /// Refusals: to `refuse` rows; refusals; `refuse` rows.
    pub refuse: Counts,
    /// Gym and interview tiers: to rows of the route read; readings with
    /// tier `gym` or `author`; rows expecting one.
    pub gym: Counts,
    /// Rows with a secret (`secret_shared` or `asks_for_secret`) that the
    /// system refused or read as that risk.
    pub secret: Counts,
    /// Readings on which the router stood back and the model answered
    /// alone (tier `model`): the abstentions, over the rows read.
    pub abstained: usize,
    pub latency_p50: Option<u128>,
    pub latency_p95: Option<u128>,
    /// Every wrong canned answer: row id, served answer, expected answer.
    pub wrong_canned: Vec<(String, String, String)>,
    /// Every wrong dispatch offer: row id and the row's route.
    pub wrong_dispatch: Vec<(String, String)>,
    /// Every Gym or interview tier on another route's row: row id, the
    /// route read, the row's route.
    pub wrong_gym: Vec<(String, String, String)>,
    /// The owner-reported rows: id, message, tier, answer.
    pub owner: Vec<(String, String, String, String)>,
}

impl Report {
    /// Scores `readings` (by row id) against `rows`.
    #[must_use]
    pub fn of(system: &str, split: &str, rows: &[&Row], readings: &[Reading]) -> Self {
        let by_id: BTreeMap<&str, &Reading> = readings.iter().map(|r| (r.id.as_str(), r)).collect();
        let mut report = Report {
            system: system.to_string(),
            split: split.to_string(),
            rows: rows.len(),
            routes: ROUTES
                .iter()
                .map(|r| ((*r).to_string(), Counts::default()))
                .collect(),
            ..Report::default()
        };
        let mut routed = 0;
        let mut routed_right = 0;
        let mut latency = Vec::new();
        for row in rows {
            let is_secret = matches!(row.risk.as_str(), "secret_shared" | "asks_for_secret");
            if let Some(counts) = report.routes.get_mut(&row.route) {
                counts.labeled += 1;
            }
            report.canned.labeled += usize::from(row.tier == "canned");
            report.dispatch.labeled += usize::from(row.route == "work.dispatch");
            report.refuse.labeled += usize::from(row.route == "refuse");
            report.gym.labeled += usize::from(matches!(row.tier.as_str(), "gym" | "author"));
            report.secret.labeled += usize::from(is_secret);
            let Some(reading) = by_id.get(row.id.as_str()) else {
                report.errors += 1;
                continue;
            };
            if reading.error.is_some() {
                report.errors += 1;
                continue;
            }
            latency.push(reading.ms);
            report.abstained += usize::from(reading.tier == "model");
            if let Some(route) = &reading.route {
                routed += 1;
                if let Some(counts) = report.routes.get_mut(route) {
                    counts.predicted += 1;
                }
                if route == &row.route {
                    routed_right += 1;
                    if let Some(counts) = report.routes.get_mut(route) {
                        counts.hit += 1;
                    }
                }
            }
            if reading.tier == "canned" {
                report.canned.predicted += 1;
                let served = reading.answer.clone().unwrap_or_default();
                if row.tier == "canned" && row.accepts(&served) {
                    report.canned.hit += 1;
                } else {
                    if row.tier == "stem" && row.accepts(&served) {
                        report.canned_over_stem += 1;
                    }
                    report.wrong_canned.push((
                        row.id.clone(),
                        served,
                        format!(
                            "{} ({})",
                            row.answer.clone().unwrap_or_else(|| "none".to_string()),
                            row.tier
                        ),
                    ));
                }
            }
            if reading.dispatch {
                report.dispatch.predicted += 1;
                if row.route == "work.dispatch" {
                    report.dispatch.hit += 1;
                } else {
                    report
                        .wrong_dispatch
                        .push((row.id.clone(), row.route.clone()));
                }
            }
            if matches!(reading.tier.as_str(), "gym" | "author") {
                report.gym.predicted += 1;
                if reading.route.as_deref() == Some(row.route.as_str()) {
                    report.gym.hit += 1;
                } else {
                    report.wrong_gym.push((
                        row.id.clone(),
                        reading.route.clone().unwrap_or_default(),
                        row.route.clone(),
                    ));
                }
            }
            if reading.tier == "refuse" {
                report.refuse.predicted += 1;
                report.refuse.hit += usize::from(row.route == "refuse");
            }
            let read_secret = reading.tier == "refuse"
                || matches!(
                    reading.risk.as_deref(),
                    Some("secret_shared" | "asks_for_secret")
                );
            if read_secret {
                report.secret.predicted += 1;
                report.secret.hit += usize::from(is_secret);
            }
            if row.tags.iter().any(|t| t == "owner") {
                report.owner.push((
                    row.id.clone(),
                    row.latest().to_string(),
                    reading.tier.clone(),
                    reading.answer.clone().unwrap_or_else(|| "-".to_string()),
                ));
            }
        }
        report.route_accuracy = ratio(routed_right, routed);
        latency.sort_unstable();
        let at = |p: f64| {
            latency
                .get(((latency.len() as f64 - 1.0) * p).round() as usize)
                .copied()
        };
        report.latency_p50 = at(0.5);
        report.latency_p95 = at(0.95);
        report
    }

    /// The report as Markdown.
    #[must_use]
    pub fn markdown(&self) -> String {
        let pct = |v: Option<f64>| v.map_or("-".to_string(), |v| format!("{:.1} %", v * 100.0));
        let mut out = String::new();
        let _ = writeln!(
            out,
            "### {} on {} ({} rows, {} errors)\n",
            self.system, self.split, self.rows, self.errors
        );
        let _ = writeln!(out, "| Metric | Value | Counts |\n| --- | --- | --- |");
        let _ = writeln!(
            out,
            "| Canned precision (target ≥ 98 %) | {} | {}/{} |",
            pct(self.canned.precision()),
            self.canned.hit,
            self.canned.predicted
        );
        let _ = writeln!(
            out,
            "| Canned coverage of canned rows | {} | {}/{} |",
            pct(ratio(self.canned.predicted, self.canned.labeled)),
            self.canned.predicted,
            self.canned.labeled
        );
        let _ = writeln!(
            out,
            "| Canned recall (right answer served) | {} | {}/{} |",
            pct(self.canned.recall()),
            self.canned.hit,
            self.canned.labeled
        );
        let _ = writeln!(
            out,
            "| Dispatch precision (target ≥ 90 %) | {} | {}/{} |",
            pct(self.dispatch.precision()),
            self.dispatch.hit,
            self.dispatch.predicted
        );
        let _ = writeln!(
            out,
            "| Dispatch recall | {} | {}/{} |",
            pct(self.dispatch.recall()),
            self.dispatch.hit,
            self.dispatch.labeled
        );
        let _ = writeln!(
            out,
            "| Gym and interview precision / recall | {} / {} | {}/{}, {}/{} |",
            pct(self.gym.precision()),
            pct(self.gym.recall()),
            self.gym.hit,
            self.gym.predicted,
            self.gym.hit,
            self.gym.labeled
        );
        let _ = writeln!(
            out,
            "| Refusal precision / recall | {} / {} | {}/{}, {}/{} |",
            pct(self.refuse.precision()),
            pct(self.refuse.recall()),
            self.refuse.hit,
            self.refuse.predicted,
            self.refuse.hit,
            self.refuse.labeled
        );
        let _ = writeln!(
            out,
            "| Secret recall | {} | {}/{} |",
            pct(self.secret.recall()),
            self.secret.hit,
            self.secret.labeled
        );
        let _ = writeln!(out, "| Route accuracy | {} | |", pct(self.route_accuracy));
        let _ = writeln!(
            out,
            "| Abstention (the model alone) | {} | {}/{} |",
            pct(self.abstention_rate()),
            self.abstained,
            self.read()
        );
        let _ = writeln!(
            out,
            "| Latency p50 / p95 | {} / {} ms | |\n",
            self.latency_p50.map_or("-".into(), |v| v.to_string()),
            self.latency_p95.map_or("-".into(), |v| v.to_string())
        );
        let _ = writeln!(
            out,
            "| Route | Precision | Recall | Hit / predicted / labeled |\n| --- | --- | --- | --- |"
        );
        for route in ROUTES {
            let c = self.routes.get(route).copied().unwrap_or_default();
            let _ = writeln!(
                out,
                "| `{route}` | {} | {} | {}/{}/{} |",
                pct(c.precision()),
                pct(c.recall()),
                c.hit,
                c.predicted,
                c.labeled
            );
        }
        if !self.wrong_canned.is_empty() {
            let _ = writeln!(out, "\nWrong canned answers (row, served, expected):");
            for (id, served, expected) in &self.wrong_canned {
                let _ = writeln!(out, "- `{id}`: {served} for {expected}");
            }
        }
        if !self.wrong_dispatch.is_empty() {
            let _ = writeln!(out, "\nWrong dispatch offers (row, labeled route):");
            for (id, route) in &self.wrong_dispatch {
                let _ = writeln!(out, "- `{id}`: {route}");
            }
        }
        if !self.wrong_gym.is_empty() {
            let _ = writeln!(
                out,
                "\nWrong Gym or interview tiers (row, route read, labeled route):"
            );
            for (id, read, route) in &self.wrong_gym {
                let _ = writeln!(out, "- `{id}`: {read} for {route}");
            }
        }
        if !self.owner.is_empty() {
            let _ = writeln!(out, "\nOwner-reported messages:");
            for (id, message, tier, answer) in &self.owner {
                let _ = writeln!(out, "- `{id}` \"{message}\": {tier}, {answer}");
            }
        }
        out
    }

    /// The rows that got a reading: the rows less the errors.
    #[must_use]
    pub fn read(&self) -> usize {
        self.rows.saturating_sub(self.errors)
    }

    /// The share of read rows on which the model answered alone.
    #[must_use]
    pub fn abstention_rate(&self) -> Option<f64> {
        ratio(self.abstained, self.read())
    }

    /// The report as JSON.
    #[must_use]
    pub fn json(&self) -> Value {
        json!({
            "schema": "openagents.chat-router.eval.v1",
            "report": self,
            "canned_precision": self.canned.precision(),
            "dispatch_precision": self.dispatch.precision(),
            "gym_precision": self.gym.precision(),
            "abstention_rate": self.abstention_rate(),
        })
    }
}

/// One point of a risk–coverage curve: what serving on a question's
/// reading at `threshold` or above would give.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct OperatingPoint {
    pub threshold: f64,
    /// Readings at or above the threshold.
    pub served: usize,
    /// Of those, the ones that were right.
    pub right: usize,
    /// Readings on the question.
    pub read: usize,
}

impl OperatingPoint {
    /// The share of readings served at this threshold.
    #[must_use]
    pub fn coverage(&self) -> Option<f64> {
        ratio(self.served, self.read)
    }

    /// The share of served readings that were right.
    #[must_use]
    pub fn precision(&self) -> Option<f64> {
        ratio(self.right, self.served)
    }

    /// The share of readings held back at this threshold.
    #[must_use]
    pub fn abstention(&self) -> Option<f64> {
        ratio(self.read - self.served, self.read)
    }
}

/// The thresholds the operating-point table is drawn at: the policy's
/// serving floors ([`crate::router::policy`]) and the tenths around them.
pub const THRESHOLDS: [f64; 9] = [0.5, 0.6, 0.7, 0.75, 0.8, 0.85, 0.9, 0.95, 0.99];

/// A question's readings as (probability, right) pairs, for the
/// operating-point table and the calibration map: `route` is the route
/// reading against the labeled route, `answer` the `answer` question's
/// argmax entry against the row's accepted answers. A row with no reading
/// on the question, or with an error, is left out (and counted as
/// unavailable by the report).
#[must_use]
pub fn observations(
    rows: &[&Row],
    readings: &[Reading],
) -> (
    Vec<gym::calibrate::Observation>,
    Vec<gym::calibrate::Observation>,
) {
    let by_id: BTreeMap<&str, &Reading> = readings.iter().map(|r| (r.id.as_str(), r)).collect();
    let mut route = Vec::new();
    let mut answer = Vec::new();
    for row in rows {
        let Some(reading) = by_id.get(row.id.as_str()) else {
            continue;
        };
        if reading.error.is_some() {
            continue;
        }
        if let Some(read) = &reading.route {
            route.push(gym::calibrate::Observation::new(
                reading.route_p,
                read == &row.route,
            ));
        }
        if let Some(read) = &reading.read_answer {
            answer.push(gym::calibrate::Observation::new(
                reading.answer_p,
                row.accepts(read),
            ));
        }
    }
    (route, answer)
}

/// The risk–coverage curve of one question's observations at
/// [`THRESHOLDS`].
#[must_use]
pub fn operating_points(observations: &[gym::calibrate::Observation]) -> Vec<OperatingPoint> {
    THRESHOLDS
        .iter()
        .map(|&threshold| {
            let served: Vec<_> = observations.iter().filter(|o| o.raw >= threshold).collect();
            OperatingPoint {
                threshold,
                served: served.len(),
                right: served.iter().filter(|o| o.correct).count(),
                read: observations.len(),
            }
        })
        .collect()
}

/// One row's cosine similarities: to each bank entry (id, route,
/// similarity) and to each route description (route, similarity).
pub type Nearness = (Vec<(String, String, f64)>, Vec<(String, f64)>);

/// The embedding baseline's decision for one row, from cosine similarities
/// already computed: the nearest prepared answer's `when` text serves it
/// whole at `threshold` or above, and the route is the nearest route
/// description's.
#[must_use]
pub fn nearest_reading(
    id: &str,
    answers: &[(String, String, f64)],
    routes: &[(String, f64)],
    threshold: f64,
) -> Reading {
    let best_answer = answers.iter().max_by(|a, b| a.2.total_cmp(&b.2));
    let best_route = routes.iter().max_by(|a, b| a.1.total_cmp(&b.1));
    let canned = best_answer.filter(|(_, _, s)| *s >= threshold);
    let route = canned
        .map(|(_, route, _)| route.clone())
        .or_else(|| best_route.map(|(r, _)| r.clone()));
    let dispatch = route.as_deref() == Some("work.dispatch");
    Reading {
        id: id.to_string(),
        route_p: best_route.map_or(0.0, |(_, s)| *s),
        answer: canned.map(|(a, _, _)| a.clone()),
        read_answer: best_answer.map(|(a, _, _)| a.clone()),
        answer_p: best_answer.map_or(0.0, |(_, _, s)| *s),
        tier: if canned.is_some() {
            "canned".to_string()
        } else if dispatch {
            "offer".to_string()
        } else {
            "model".to_string()
        },
        risk: None,
        dispatch,
        route,
        ms: 0,
        error: None,
    }
}

/// The least threshold at which the embedding baseline's canned precision
/// on `rows` reaches `target`, from the candidates `0.30` to `0.95` by
/// `0.01`; the most precise threshold when none reaches it. Fit on the tune
/// split only.
#[must_use]
pub fn fit_threshold(rows: &[&Row], scored: &BTreeMap<String, Nearness>, target: f64) -> f64 {
    let mut best = (0.95, -1.0);
    for step in 30..=95 {
        let threshold = f64::from(step) / 100.0;
        let readings: Vec<Reading> = rows
            .iter()
            .filter_map(|row| {
                let (answers, routes) = scored.get(&row.id)?;
                Some(nearest_reading(&row.id, answers, routes, threshold))
            })
            .collect();
        let report = Report::of("embedding", "tune", rows, &readings);
        let precision = report.canned.precision().unwrap_or(0.0);
        if precision >= target && report.canned.predicted > 0 {
            return threshold;
        }
        if precision > best.1 {
            best = (threshold, precision);
        }
    }
    best.0
}

/// The route descriptions the embedding baseline compares messages with:
/// the ones Jev reads ([`crate::router::RouteId::description`]).
#[must_use]
pub fn route_descriptions() -> Vec<(&'static str, &'static str)> {
    crate::router::RouteId::ALL
        .into_iter()
        .map(|route| (route.word(), route.description()))
        .collect()
}

/// The Gym suite the set is exported as, and its question set.
pub const SUITE: &str = "chat-router-v5";

/// The question set the Gym suite names: the router's `route` Choice.
pub const SUITE_QUESTIONS: &str = "chat-router-route-v6";

/// The `chat-router-v4` suite, kept as recorded with the twenty-route
/// question it was scored with.
pub const SUITE_V4: &str = "chat-router-v4";

/// The `chat-router-v4` suite's question set.
pub const SUITE_QUESTIONS_V4: &str = "chat-router-route-v5";

/// The `chat-router-v3` suite, kept as recorded with the nineteen-route
/// question it was scored with.
pub const SUITE_V3: &str = "chat-router-v3";

/// The `chat-router-v3` suite's question set.
pub const SUITE_QUESTIONS_V3: &str = "chat-router-route-v4";

/// The `chat-router-v2` suite, kept as recorded with the eighteen-route
/// question it was scored with.
pub const SUITE_V2: &str = "chat-router-v2";

/// The `chat-router-v2` suite's question set.
pub const SUITE_QUESTIONS_V2: &str = "chat-router-route-v3";

/// The Gym suite exported from the `chat-router-v1` set, and the question
/// set it was scored with; both are kept as recorded.
pub const SUITE_V1: &str = "chat-router-v1";

/// The `chat-router-v1` suite's question set: the twelve-route question.
pub const SUITE_QUESTIONS_V1: &str = "chat-router-route-v2";

/// The `route` question the Gym suite asks: the production router's own
/// ([`crate::router::judge::route`]), structured instructions and option
/// rubrics included, so a Gym score measures the question production asks.
#[must_use]
pub fn route_question() -> jev::Choice {
    crate::router::judge::route()
}

/// A row's Gym partition: `locked` for a held-out row, else
/// `calibration` or `development` by the parity of the fifth byte of
/// SHA-256 of its id.
#[must_use]
pub fn partition_of(row: &Row) -> &'static str {
    if row.held_out() {
        return "locked";
    }
    let digest = knowledge::digest(row.id.as_bytes());
    let hex = digest.trim_start_matches("sha256:");
    let byte = u8::from_str_radix(&hex[8..10], 16).unwrap_or(0);
    if byte.is_multiple_of(2) {
        "calibration"
    } else {
        "development"
    }
}

/// A router decision as a reading: the route the judgment chose, and the
/// tier and answer the policy table decided. `opener` and `model` both
/// read as `model`, and a CLI proposal as `offer`, the words the set
/// labels with; a bank entry that dispatches Coder is the dispatch offer.
#[must_use]
pub fn routed_reading(
    id: &str,
    routing: &crate::router::Routing,
    tier: &crate::router::Tier,
    ms: u128,
) -> Reading {
    use crate::router::RouteId;
    let answer = tier.answer();
    let dispatch = answer.is_some_and(|entry| entry.answers(RouteId::WorkDispatch))
        && matches!(tier.word(), "offer");
    Reading {
        id: id.to_string(),
        route: (routing.route != RouteId::Unknown).then(|| routing.route.word().to_string()),
        route_p: routing.route_p,
        answer: answer.map(|entry| entry.id.clone()),
        read_answer: routing.answer.as_ref().map(|(entry, _)| entry.id.clone()),
        answer_p: routing.answer.as_ref().map_or(0.0, |(_, p)| *p),
        tier: match tier.word() {
            "opener" | "model" => "model",
            "cli" => "offer",
            word => word,
        }
        .to_string(),
        risk: Some(routing.risk.word().to_string()),
        dispatch,
        ms,
        error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// Every prepared answer id the set may name.
    const ANSWER_IDS: &[&str] = &[
        "meta.map",
        "meta.map.desktop",
        "meta.who",
        "meta.products",
        "meta.model",
        "meta.jev",
        "meta.coder",
        "meta.codebase",
        "meta.plugins",
        "meta.capabilities",
        "meta.limits_chat",
        "meta.pricing",
        "meta.quota",
        "meta.privacy",
        "meta.data_retention",
        "meta.open_source",
        "meta.computers",
        "meta.offline",
        "meta.languages",
        "meta.web_access",
        "meta.memory",
        "meta.team",
        "meta.github",
        "smalltalk.hello",
        "smalltalk.thanks",
        "smalltalk.bye",
        "smalltalk.how_are_you",
        "smalltalk.test",
        "dispatch.stem",
        "dispatch.no_computer",
        "dispatch.explore_stem",
        "dispatch.github_stem",
        "wallet.what",
        "wallet.receive",
        "wallet.send",
        "wallet.units",
        "wallet.backup",
        "wallet.never_share",
        "account.computers",
        "account.keys",
        "account.playtest",
        "account.report_problem",
        "refuse.secret_shared",
        "refuse.asks_for_secret",
        "refuse.harmful",
        "clarify.generic",
        "gym.what",
        "gym.what_test",
        "gym.what_tool",
        "eval.credit.how",
        "eval.credit.mine",
        "capability.missing",
        "capability.missing_near",
        "presentation.open",
        "presentation.unknown",
        "presentation.elsewhere",
        "standing.rule",
        "standing.elsewhere",
        "dispatch.capability_stem",
        "dispatch.engine_stem",
    ];

    /// An `eval.run` row's tool is a tool of the product corpus's catalog
    /// or none; no other row names one.
    #[test]
    fn eval_run_rows_name_catalog_tools() {
        // The labeled set names the sample plugins' notes, kept as fixtures.
        let tools: BTreeSet<String> = crate::gym_kb::fixture_tools()
            .into_iter()
            .map(|tool| tool.id)
            .collect();
        let set = Set::fixture();
        let mut named = 0;
        for row in &set.rows {
            match (&row.tool, row.route.as_str()) {
                (Some(tool), "eval.run") => {
                    assert!(tools.contains(tool), "{}: {tool}", row.id);
                    named += 1;
                }
                (Some(tool), _) => panic!("{} names {tool} off eval.run", row.id),
                (None, _) => {}
            }
        }
        assert!(named >= 10, "{named}");
    }

    #[test]
    fn the_set_is_well_formed_and_covers_every_route() {
        let set = Set::fixture();
        assert_eq!(set.schema, SCHEMA);
        assert!(set.rows.len() >= 400, "{} rows", set.rows.len());
        let mut ids = BTreeSet::new();
        for row in &set.rows {
            assert!(ids.insert(row.id.as_str()), "duplicate {}", row.id);
            assert!(ROUTES.contains(&row.route.as_str()), "{}", row.id);
            assert!(TIERS.contains(&row.tier.as_str()), "{}", row.id);
            assert!(RISKS.contains(&row.risk.as_str()), "{}", row.id);
            assert_eq!(
                row.messages.last().map(|m| m.role.as_str()),
                Some("user"),
                "{}",
                row.id
            );
            for answer in row.answer.iter().chain(&row.also) {
                assert!(
                    ANSWER_IDS.contains(&answer.as_str()),
                    "{}: {answer}",
                    row.id
                );
            }
            assert_eq!(row.cli_group.is_some(), row.route == "cli", "{}", row.id);
            if matches!(row.tier.as_str(), "canned" | "stem" | "refuse") {
                assert!(row.answer.is_some(), "{} needs an answer", row.id);
            }
        }
        for route in ROUTES {
            let n = set.rows.iter().filter(|r| r.route == route).count();
            assert!(n >= 20, "{route} has {n} rows");
        }
    }

    /// Each set keeps every row of the one before as it was, so their
    /// splits and labels still mean what the earlier measurements said.
    #[test]
    fn each_set_keeps_every_earlier_row() {
        let v1 = Set::v1();
        let v2 = Set::v2();
        let v3 = Set::v3();
        let v4 = Set::v4();
        let v5 = Set::fixture();
        assert_eq!(v1.rows.len(), 457);
        assert!(v2.rows.len() >= 641, "{}", v2.rows.len());
        for row in &v1.rows {
            let kept = v2.rows.iter().find(|r| r.id == row.id).expect("kept");
            assert_eq!(kept, row, "{} changed", row.id);
        }
        for row in &v2.rows {
            let kept = v3.rows.iter().find(|r| r.id == row.id).expect("kept");
            assert_eq!(kept, row, "{} changed", row.id);
        }
        for row in &v3.rows {
            let kept = v4.rows.iter().find(|r| r.id == row.id).expect("kept");
            assert_eq!(kept, row, "{} changed", row.id);
        }
        for row in &v4.rows {
            let kept = v5.rows.iter().find(|r| r.id == row.id).expect("kept");
            assert_eq!(kept, row, "{} changed", row.id);
        }
        for route in crate::router::RouteId::GYM.into_iter().chain([
            crate::router::RouteId::CapabilityMissing,
            crate::router::RouteId::PresentationOpen,
        ]) {
            let held = v5
                .rows("held_out")
                .iter()
                .filter(|r| r.route == route.word())
                .count();
            assert!(held >= 6, "{} has {held} held-out rows", route.word());
        }
        // The missing-capability rows and their admitted near misses (#9960).
        let missing = v3
            .rows
            .iter()
            .filter(|r| r.route == "capability.missing")
            .count();
        assert!(missing >= 30, "{missing}");
        let near = v3
            .rows
            .iter()
            .filter(|r| {
                r.tags.iter().any(|t| t == "near-miss") && r.tags.iter().any(|t| t == "capability")
            })
            .count();
        assert!(near >= 20, "{near}");
        for row in v3.rows.iter().filter(|r| r.route == "capability.missing") {
            assert_eq!(row.tier, "canned", "{}", row.id);
            assert_eq!(
                row.answer.as_deref(),
                Some("capability.missing"),
                "{}",
                row.id
            );
        }
        // The deck rows and their near misses (#10058). Labeled for the
        // set's default phone context, where a deck reads as the line
        // that says decks open in the desktop app.
        let decks: Vec<&Row> = v4
            .rows
            .iter()
            .filter(|r| r.route == "presentation.open")
            .collect();
        assert!(decks.len() >= 30, "{}", decks.len());
        for row in decks {
            assert_eq!(row.tier, "canned", "{}", row.id);
            assert_eq!(row.answer.as_deref(), Some("presentation.elsewhere"));
            assert!(row.accepts("presentation.open") && row.accepts("presentation.unknown"));
        }
        let near = v4
            .rows
            .iter()
            .filter(|r| {
                r.tags.iter().any(|t| t == "near-miss")
                    && r.tags.iter().any(|t| t == "presentation")
            })
            .count();
        assert!(near >= 10, "{near}");
        // The standing-rule rows and their near misses (#10157), labeled
        // for the default phone context, where the line says rules are
        // set up on the computer.
        let standing: Vec<&Row> = v5
            .rows
            .iter()
            .filter(|r| r.route == "standing.rule")
            .collect();
        assert!(standing.len() >= 40, "{}", standing.len());
        for row in standing {
            assert_eq!(row.tier, "canned", "{}", row.id);
            assert_eq!(row.answer.as_deref(), Some("standing.elsewhere"));
            assert!(row.accepts("standing.rule"), "{}", row.id);
        }
        let near = v5
            .rows
            .iter()
            .filter(|r| {
                r.tags.iter().any(|t| t == "near-miss") && r.tags.iter().any(|t| t == "standing")
            })
            .count();
        assert!(near >= 12, "{near}");
        // Every standing row is in the tune split, so the held-out
        // partition stays at NIP-EVAL's 256 cases.
        assert!(
            v5.rows
                .iter()
                .filter(|r| r.tags.iter().any(|t| t == "standing"))
                .all(|r| r.split == "tune")
        );
    }

    /// The engine rows (#10076): dispatch rows that name an engine, by a
    /// word of NIP-CJ's closed set, dispatch rows that name none, and near
    /// misses that name one only as a subject; only a `work.dispatch` row
    /// names an engine, and some of each kind are held out.
    #[test]
    fn engine_rows_name_a_closed_engine_on_dispatch_only() {
        let set = Set::fixture();
        let tagged: Vec<&Row> = set
            .rows
            .iter()
            .filter(|r| r.tags.iter().any(|t| t == "engine"))
            .collect();
        for row in &set.rows {
            if let Some(engine) = &row.engine {
                assert_eq!(row.route, "work.dispatch", "{}", row.id);
                assert!(
                    crate::router::CodingEngine::parse(engine).is_some(),
                    "{}: {engine}",
                    row.id
                );
                assert_eq!(row.answer.as_deref(), Some("dispatch.engine_stem"));
            }
        }
        let named = tagged.iter().filter(|r| r.engine.is_some()).count();
        let unnamed = tagged
            .iter()
            .filter(|r| r.route == "work.dispatch" && r.engine.is_none())
            .count();
        let near = tagged.iter().filter(|r| r.route != "work.dispatch").count();
        assert!(
            named >= 20 && unnamed >= 5 && near >= 10,
            "{named} {unnamed} {near}"
        );
        let held = |want: &dyn Fn(&&&Row) -> bool| {
            tagged.iter().filter(|r| r.held_out()).filter(want).count()
        };
        assert!(held(&|r| r.engine.is_some()) >= 5);
        assert!(held(&|r| r.route != "work.dispatch") >= 3);
        // The owner's message is in the set.
        assert!(
            set.rows
                .iter()
                .any(|r| r.latest() == "Do a test delegation to claude"
                    && r.engine.as_deref() == Some("claude_code"))
        );
    }

    #[test]
    fn the_owner_reported_messages_are_in_the_set() {
        let set = Set::fixture();
        for (message, answer) in [
            ("Who are you?", "meta.who"),
            ("Connect to my GitHub", "meta.github"),
            ("What can you do", "meta.capabilities"),
        ] {
            let row = set
                .rows
                .iter()
                .find(|r| r.latest() == message)
                .unwrap_or_else(|| panic!("{message} is missing"));
            assert_eq!(row.answer.as_deref(), Some(answer));
            assert_eq!(row.tier, "canned");
            assert!(row.tags.iter().any(|t| t == "owner"));
        }
    }

    #[test]
    fn the_split_is_the_ids_hash_and_holds_out_about_thirty_percent() {
        let set = Set::fixture();
        for row in &set.rows {
            assert_eq!(row.split, split_of(&row.id), "{} moved split", row.id);
        }
        let held = set.rows.iter().filter(|r| r.held_out()).count();
        let share = held as f64 / set.rows.len() as f64;
        assert!((0.24..=0.36).contains(&share), "{share}");
    }

    fn row(id: &str, route: &str, answer: Option<&str>, tier: &str) -> Row {
        Row {
            id: id.to_string(),
            messages: vec![Said {
                role: "user".to_string(),
                text: id.to_string(),
            }],
            route: route.to_string(),
            answer: answer.map(str::to_string),
            also: Vec::new(),
            tier: tier.to_string(),
            risk: "ok".to_string(),
            cli_group: None,
            tool: None,
            engine: None,
            tags: Vec::new(),
            split: "tune".to_string(),
        }
    }

    fn reading(id: &str, route: &str, answer: Option<&str>, tier: &str, dispatch: bool) -> Reading {
        Reading {
            id: id.to_string(),
            route: Some(route.to_string()),
            answer: answer.map(str::to_string),
            tier: tier.to_string(),
            dispatch,
            ms: 100,
            ..Reading::default()
        }
    }

    #[test]
    fn canned_and_dispatch_precision_count_what_was_served() {
        let rows = [
            row("a", "meta", Some("meta.who"), "canned"),
            row("b", "meta", Some("meta.model"), "canned"),
            row("c", "meta", Some("meta.capabilities"), "stem"),
            row("d", "work.dispatch", Some("dispatch.stem"), "offer"),
            row("e", "codebase.kb", None, "grounded"),
        ];
        let refs: Vec<&Row> = rows.iter().collect();
        let readings = [
            reading("a", "meta", Some("meta.who"), "canned", false),
            reading("b", "meta", Some("meta.who"), "canned", false),
            reading("c", "meta", Some("meta.capabilities"), "canned", false),
            reading("d", "work.dispatch", Some("dispatch.stem"), "offer", true),
            reading("e", "work.dispatch", None, "offer", true),
        ];
        let report = Report::of("test", "tune", &refs, &readings);
        assert_eq!((report.canned.hit, report.canned.predicted), (1, 3));
        assert_eq!(report.canned_over_stem, 1);
        assert_eq!((report.dispatch.hit, report.dispatch.predicted), (1, 2));
        assert_eq!(report.routes["meta"].recall(), Some(1.0));
        assert_eq!(report.routes["work.dispatch"].precision(), Some(0.5));
        assert_eq!(report.route_accuracy, Some(0.8));
        assert_eq!(report.abstained, 0);
        assert_eq!(report.abstention_rate(), Some(0.0));
        assert!(
            report
                .markdown()
                .contains("| Canned precision (target ≥ 98 %) | 33.3 % | 1/3 |")
        );
    }

    /// The operating-point table and the calibration observations read
    /// each question's probability against the row's label: a route is
    /// right when it is the labeled route, an answer reading when the row
    /// accepts it, served or not; an errored or unread row is left out.
    #[test]
    fn observations_and_operating_points_read_each_question_against_its_label() {
        let rows = [
            row("a", "meta", Some("meta.who"), "canned"),
            row("b", "meta", Some("meta.model"), "canned"),
            row("c", "general", None, "model"),
            row("d", "general", None, "model"),
        ];
        let refs: Vec<&Row> = rows.iter().collect();
        let mut a = reading("a", "meta", Some("meta.who"), "canned", false);
        (a.route_p, a.read_answer, a.answer_p) = (0.95, Some("meta.who".into()), 0.92);
        let mut b = reading("b", "general", None, "model", false);
        (b.route_p, b.read_answer, b.answer_p) = (0.55, Some("meta.who".into()), 0.65);
        let mut c = reading("c", "general", None, "model", false);
        (c.route_p, c.read_answer) = (0.85, None);
        let mut d = reading("d", "general", None, "model", false);
        d.error = Some("timeout".into());
        let (route, answer) = observations(&refs, &[a, b, c, d]);
        assert_eq!(route.len(), 3);
        assert_eq!(answer.len(), 2);
        assert!(route[0].correct && !route[1].correct && route[2].correct);
        assert!(answer[0].correct && !answer[1].correct);
        let points = operating_points(&route);
        assert_eq!(points.len(), THRESHOLDS.len());
        let at = |t: f64| {
            points
                .iter()
                .find(|p| (p.threshold - t).abs() < 1e-9)
                .unwrap()
        };
        assert_eq!((at(0.8).served, at(0.8).right), (2, 2));
        assert_eq!(at(0.8).coverage(), Some(2.0 / 3.0));
        assert_eq!(at(0.8).precision(), Some(1.0));
        assert_eq!(at(0.5).abstention(), Some(0.0));
        assert_eq!((at(0.99).served, at(0.99).precision()), (0, None));
        let report = Report::of("test", "tune", &refs, &[]);
        assert_eq!(report.errors, 4);
        assert_eq!(report.abstention_rate(), None);
    }

    #[test]
    fn a_failed_reading_counts_as_an_error_not_a_miss() {
        let rows = [row("a", "meta", Some("meta.who"), "canned")];
        let refs: Vec<&Row> = rows.iter().collect();
        let mut failed = reading("a", "meta", None, "model", false);
        failed.error = Some("timeout".to_string());
        let report = Report::of("test", "tune", &refs, &[failed]);
        assert_eq!(report.errors, 1);
        assert_eq!(report.canned.predicted, 0);
        assert_eq!(report.routes["meta"].labeled, 1);
    }

    #[test]
    fn the_embedding_baseline_serves_only_above_its_threshold() {
        let answers = vec![
            ("meta.who".to_string(), "meta".to_string(), 0.71),
            ("meta.model".to_string(), "meta".to_string(), 0.52),
        ];
        let routes = vec![("general".to_string(), 0.4), ("meta".to_string(), 0.3)];
        let sure = nearest_reading("x", &answers, &routes, 0.7);
        assert_eq!(sure.tier, "canned");
        assert_eq!(sure.answer.as_deref(), Some("meta.who"));
        assert_eq!(sure.route.as_deref(), Some("meta"));
        let unsure = nearest_reading("x", &answers, &routes, 0.8);
        assert_eq!(unsure.tier, "model");
        assert_eq!(unsure.route.as_deref(), Some("general"));
    }

    #[test]
    fn the_threshold_fit_reaches_the_target_when_it_can() {
        let rows = [
            row("a", "meta", Some("meta.who"), "canned"),
            row("b", "general", None, "model"),
        ];
        let refs: Vec<&Row> = rows.iter().collect();
        let mut scored = BTreeMap::new();
        scored.insert(
            "a".to_string(),
            (
                vec![("meta.who".to_string(), "meta".to_string(), 0.8)],
                vec![],
            ),
        );
        scored.insert(
            "b".to_string(),
            (
                vec![("meta.who".to_string(), "meta".to_string(), 0.6)],
                vec![],
            ),
        );
        assert!((fit_threshold(&refs, &scored, 0.98) - 0.61).abs() < 1e-9);
    }
}
