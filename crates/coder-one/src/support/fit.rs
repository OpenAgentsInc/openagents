//! Fitting the support cutoffs on development evidence, and scoring every
//! decision rule's error directions on the labeled fixtures.
//!
//! A false accept is a requirement, or a candidate, the rule accepts
//! though it isn't met: the costly direction, since the episode then ends
//! on a failure. A false reject is one the rule doesn't accept though it
//! is met: an unneeded repair. An unresolved requirement is never
//! accepted, so it counts toward false rejects when it's met, and the
//! report says how many of those were unresolved rather than contradicted.
//!
//! The cutoffs are fitted on the development split only. The evaluation
//! split, the recovered v3 candidates, is scored and never fitted on.

use serde_json::{Value, json};

use super::Params;

/// The fitted cutoffs, supports then contradicts. A test refits them on
/// the development fixtures with recorded Jev and checks they match.
pub const FITTED: (f64, f64) = (0.5, 0.3);

/// The grid the fit searches, for both cutoffs.
pub const GRID: [f64; 17] = [
    0.1, 0.15, 0.2, 0.25, 0.3, 0.35, 0.4, 0.45, 0.5, 0.55, 0.6, 0.65, 0.7, 0.75, 0.8, 0.85, 0.9,
];

/// The best choice so far, by (errors, false accepts, distance from 0.5).
type Best<T> = Option<((usize, usize, u64), T)>;

/// One labeled requirement judgment.
#[derive(Clone, Debug, PartialEq)]
pub struct Triple {
    pub fixture: String,
    pub split: String,
    pub requirement: String,
    pub met: bool,
    pub supports: Option<f64>,
    pub contradicts: Option<f64>,
    pub clipped: bool,
    /// The requirement's state from `verify.checks`.
    pub scenario_state: String,
}

/// One candidate: its outcome, the broad "done" answer, and its triples.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub fixture: String,
    pub split: String,
    pub met: Option<bool>,
    pub done: Option<f64>,
    /// Every judged requirement, labeled or not: (supports, contradicts,
    /// clipped, scenario state).
    pub judged: Vec<(Option<f64>, Option<f64>, bool, String)>,
    pub triples: Vec<Triple>,
}

/// Reads rows from `verify.support` component outputs.
#[must_use]
pub fn rows(outputs: &[(String, Value)]) -> Vec<Row> {
    outputs
        .iter()
        .map(|(fixture, output)| {
            let split = output["split"]
                .as_str()
                .unwrap_or("development")
                .to_string();
            let states = output["states"].as_array().map_or(&[][..], Vec::as_slice);
            let triples = states
                .iter()
                .filter_map(|s| {
                    Some(Triple {
                        fixture: fixture.clone(),
                        split: split.clone(),
                        requirement: s["id"].as_str()?.to_string(),
                        met: s["label"].as_bool()?,
                        supports: s["supports"].as_f64(),
                        contradicts: s["contradicts"].as_f64(),
                        clipped: s["clipped"].as_bool().unwrap_or(false),
                        scenario_state: s["scenario_state"]
                            .as_str()
                            .unwrap_or_default()
                            .to_string(),
                    })
                })
                .collect();
            Row {
                fixture: fixture.clone(),
                split,
                met: output.pointer("/outcome/met").and_then(Value::as_bool),
                done: output["done"].as_f64(),
                judged: states
                    .iter()
                    .map(|s| {
                        (
                            s["supports"].as_f64(),
                            s["contradicts"].as_f64(),
                            s["clipped"].as_bool().unwrap_or(false),
                            s["scenario_state"].as_str().unwrap_or_default().to_string(),
                        )
                    })
                    .collect(),
                triples,
            }
        })
        .collect()
}

/// Whether the pair of answers accepts a requirement under the cutoffs:
/// supported and not contradicted, from a whole source.
#[must_use]
pub fn accepts(
    supports: Option<f64>,
    contradicts: Option<f64>,
    clipped: bool,
    t: (f64, f64),
) -> bool {
    match (supports, contradicts) {
        (Some(s), Some(c)) => !clipped && s >= t.0 && c < t.1,
        _ => false,
    }
}

/// Whether the pair contradicts a requirement: contradicted and not
/// supported.
#[must_use]
pub fn rejects(
    supports: Option<f64>,
    contradicts: Option<f64>,
    clipped: bool,
    t: (f64, f64),
) -> bool {
    match (supports, contradicts) {
        (Some(s), Some(c)) => !clipped && c >= t.1 && s < t.0,
        _ => false,
    }
}

/// Counts of one rule's decisions against labels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Errors {
    pub n: usize,
    pub accepted: usize,
    pub false_accepts: usize,
    pub false_rejects: usize,
    /// False rejects the rule left unresolved rather than contradicted.
    pub unresolved_rejects: usize,
    /// Unmet cases.
    pub unmet: usize,
}

impl Errors {
    fn add(&mut self, met: bool, accepted: bool, contradicted: bool) {
        self.n += 1;
        self.accepted += usize::from(accepted);
        self.unmet += usize::from(!met);
        if accepted && !met {
            self.false_accepts += 1;
        }
        if !accepted && met {
            self.false_rejects += 1;
            self.unresolved_rejects += usize::from(!contradicted);
        }
    }

    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "n": self.n,
            "unmet": self.unmet,
            "accepted": self.accepted,
            "false_accepts": self.false_accepts,
            "false_rejects": self.false_rejects,
            "false_rejects_unresolved": self.unresolved_rejects,
        })
    }
}

fn requirement_errors(triples: &[&Triple], t: (f64, f64)) -> Errors {
    let mut errors = Errors::default();
    for triple in triples {
        errors.add(
            triple.met,
            accepts(triple.supports, triple.contradicts, triple.clipped, t),
            rejects(triple.supports, triple.contradicts, triple.clipped, t),
        );
    }
    errors
}

/// Fits the cutoffs on `triples`: the fewest false accepts plus false
/// rejects, then the fewest false accepts, then the pair nearest 0.5.
#[must_use]
pub fn fit(triples: &[&Triple]) -> (f64, f64) {
    let mut best: Best<(f64, f64)> = None;
    for s in GRID {
        for c in GRID {
            let errors = requirement_errors(triples, (s, c));
            let distance = (((s - 0.5).abs() + (c - 0.5).abs()) * 1000.0).round() as u64;
            let key = (
                errors.false_accepts + errors.false_rejects,
                errors.false_accepts,
                distance,
            );
            if best.as_ref().is_none_or(|(k, _)| key < *k) {
                best = Some((key, (s, c)));
            }
        }
    }
    best.map_or((0.5, 0.5), |(_, t)| t)
}

/// The Brier score of `p` against `met`, over the pairs where `p` is known.
fn brier(pairs: &[(Option<f64>, bool)]) -> Value {
    let known: Vec<(f64, bool)> = pairs
        .iter()
        .filter_map(|(p, m)| p.map(|p| (p, *m)))
        .collect();
    if known.is_empty() {
        return Value::Null;
    }
    let sum: f64 = known
        .iter()
        .map(|(p, m)| (p - f64::from(u8::from(*m))).powi(2))
        .sum();
    json!({ "n": known.len(), "brier": round(sum / known.len() as f64) })
}

/// A reliability table: five bins of `p`, each with its count, mean
/// probability, and the fraction met.
fn bins(pairs: &[(Option<f64>, bool)]) -> Value {
    let mut out = Vec::new();
    for b in 0..5 {
        let (lo, hi) = (f64::from(b) / 5.0, f64::from(b + 1) / 5.0);
        let inside: Vec<(f64, bool)> = pairs
            .iter()
            .filter_map(|(p, m)| p.map(|p| (p, *m)))
            .filter(|(p, _)| *p >= lo && (*p < hi || (b == 4 && *p <= hi)))
            .collect();
        if inside.is_empty() {
            continue;
        }
        let n = inside.len() as f64;
        out.push(json!({
            "bin": format!("{lo:.1}–{hi:.1}"),
            "n": inside.len(),
            "mean_p": round(inside.iter().map(|(p, _)| p).sum::<f64>() / n),
            "met": round(inside.iter().filter(|(_, m)| *m).count() as f64 / n),
        }));
    }
    json!(out)
}

fn round(x: f64) -> f64 {
    (x * 10_000.0).round() / 10_000.0
}

/// Candidate-level decisions: the broad "done" judgment at a cutoff, and
/// the support pair, which accepts a candidate only when it accepts every
/// judged requirement.
fn candidate_errors(rows: &[&Row], rule: &dyn Fn(&Row) -> Option<bool>) -> Errors {
    let mut errors = Errors::default();
    for row in rows {
        let (Some(met), Some(accepted)) = (row.met, rule(row)) else {
            continue;
        };
        errors.add(met, accepted, !accepted);
    }
    errors
}

fn split_report(rows: &[&Row], t: (f64, f64), done_cut: f64) -> Value {
    let triples: Vec<&Triple> = rows.iter().flat_map(|r| r.triples.iter()).collect();
    let support = requirement_errors(&triples, t);
    let at_half = requirement_errors(&triples, (0.5, 0.5));
    let mut checks = Errors::default();
    let mut combined = Errors::default();
    for triple in &triples {
        let scenario_ok = triple.scenario_state == "observed";
        let scenario_bad = triple.scenario_state == "contradicted";
        checks.add(triple.met, scenario_ok, scenario_bad);
        let jev_ok = accepts(triple.supports, triple.contradicts, triple.clipped, t);
        combined.add(triple.met, jev_ok && !scenario_bad, scenario_bad);
    }
    let done = |cut: f64| move |row: &Row| row.done.map(|p| p >= cut);
    let support_candidate = |row: &Row| {
        (!row.judged.is_empty()).then(|| {
            row.judged
                .iter()
                .all(|(s, c, clipped, _)| accepts(*s, *c, *clipped, t))
        })
    };
    let combined_candidate = |row: &Row| {
        (!row.judged.is_empty()).then(|| {
            row.judged.iter().all(|(s, c, clipped, scenario)| {
                accepts(*s, *c, *clipped, t) && scenario != "contradicted"
            })
        })
    };
    let supports_pairs: Vec<(Option<f64>, bool)> =
        triples.iter().map(|t| (t.supports, t.met)).collect();
    let not_contradicts: Vec<(Option<f64>, bool)> = triples
        .iter()
        .map(|t| (t.contradicts.map(|c| 1.0 - c), t.met))
        .collect();
    let done_pairs: Vec<(Option<f64>, bool)> = rows
        .iter()
        .filter_map(|r| r.met.map(|m| (r.done, m)))
        .collect();
    json!({
        "candidates": rows.len(),
        "labeled_requirements": triples.len(),
        "requirements": {
            "support_fitted": support.to_json(),
            "support_at_0.5": at_half.to_json(),
            "checks_alone": checks.to_json(),
            "support_and_checks": combined.to_json(),
        },
        "candidates_by_rule": {
            "done_at_0.5": candidate_errors(rows, &done(0.5)).to_json(),
            "done_at_dev_cutoff": candidate_errors(rows, &done(done_cut)).to_json(),
            "support_fitted": candidate_errors(rows, &support_candidate).to_json(),
            "support_and_checks": candidate_errors(rows, &combined_candidate).to_json(),
        },
        "calibration": {
            "supports": { "score": brier(&supports_pairs), "bins": bins(&supports_pairs) },
            "one_minus_contradicts": { "score": brier(&not_contradicts), "bins": bins(&not_contradicts) },
            "done": { "score": brier(&done_pairs), "bins": bins(&done_pairs) },
        },
    })
}

/// The "done" cutoff fitted on development candidates the same way, so
/// the baseline gets the same chance to tune as the pair.
#[must_use]
pub fn fit_done(rows: &[&Row]) -> f64 {
    let mut best: Best<f64> = None;
    for cut in GRID {
        let errors = candidate_errors(rows, &|row: &Row| row.done.map(|p| p >= cut));
        let key = (
            errors.false_accepts + errors.false_rejects,
            errors.false_accepts,
            ((cut - 0.5).abs() * 1000.0).round() as u64,
        );
        if best.as_ref().is_none_or(|(k, _)| key < *k) {
            best = Some((key, cut));
        }
    }
    best.map_or(0.5, |(_, cut)| cut)
}

/// The whole evaluation: the cutoffs fitted on development, and each
/// split's error directions under every rule.
#[must_use]
pub fn evaluate(rows: &[Row]) -> Value {
    let dev: Vec<&Row> = rows.iter().filter(|r| r.split == "development").collect();
    let eval: Vec<&Row> = rows.iter().filter(|r| r.split == "evaluation").collect();
    let dev_triples: Vec<&Triple> = dev.iter().flat_map(|r| r.triples.iter()).collect();
    let fitted = fit(&dev_triples);
    let done_cut = fit_done(&dev);
    json!({
        "schema": "openagents.coder-one.support-evaluation.v1",
        "fitted_on": "development",
        "cutoffs": { "supports": fitted.0, "contradicts": fitted.1, "done": done_cut },
        "checked_in": { "supports": FITTED.0, "contradicts": FITTED.1, "matches": fitted == FITTED },
        "params": Params::default(),
        "development": split_report(&dev, fitted, done_cut),
        "evaluation": split_report(&eval, fitted, done_cut),
        "candidates": rows.iter().map(|r| json!({
            "fixture": r.fixture,
            "split": r.split,
            "met": r.met,
            "done": r.done,
            "judged": r.judged.iter().map(|(s, c, clipped, scenario)| json!({ "supports": s, "contradicts": c, "clipped": clipped, "scenario_state": scenario })).collect::<Vec<_>>(),
            "support_accepts": !r.judged.is_empty() && r.judged.iter().all(|(s, c, clipped, _)| accepts(*s, *c, *clipped, fitted)),
        })).collect::<Vec<_>>(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn triple(met: bool, s: f64, c: f64) -> Triple {
        Triple {
            fixture: "f".into(),
            split: "development".into(),
            requirement: "R1".into(),
            met,
            supports: Some(s),
            contradicts: Some(c),
            clipped: false,
            scenario_state: "observed".into(),
        }
    }

    #[test]
    fn the_fit_separates_labeled_pairs_and_prefers_fewer_false_accepts() {
        let triples = [
            triple(true, 0.8, 0.1),
            triple(true, 0.7, 0.2),
            triple(false, 0.6, 0.7),
            triple(false, 0.3, 0.8),
        ];
        let refs: Vec<&Triple> = triples.iter().collect();
        let t = fit(&refs);
        let errors = requirement_errors(&refs, t);
        assert_eq!((errors.false_accepts, errors.false_rejects), (0, 0));
    }

    #[test]
    fn a_missing_answer_or_a_clipped_source_is_never_accepted() {
        assert!(!accepts(None, Some(0.1), false, (0.5, 0.5)));
        assert!(!accepts(Some(0.9), Some(0.1), true, (0.5, 0.5)));
        assert!(accepts(Some(0.9), Some(0.1), false, (0.5, 0.5)));
        let mut errors = Errors::default();
        errors.add(true, false, false);
        assert_eq!((errors.false_rejects, errors.unresolved_rejects), (1, 1));
    }
}
