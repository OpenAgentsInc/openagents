//! The combined verdict: pass, fail, or unknown, with a stated precision.
//!
//! Escalation, persistence, best-of-N, and a Microluna handoff all need
//! one answer to "did this candidate work?" that is right when it speaks.
//! [`judge`] gives it from what an episode already has about its final
//! candidate, plus Jev's answers to a few questions over the task and the
//! executor's final report ([`report_questions`]). The parameters in
//! [`fitted`] were fitted on the calibration half of the label set in
//! [`super::truth`] and never on its held-out half. Each call carries the
//! precision measured on the historical comparison half. That half was
//! inspected during the original study; this is reused validation, not an
//! untouched estimate or a guarantee for a new executor population.

use std::collections::BTreeMap;

use jev::{Noul, NoulCriteria, Questions};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::truth::{Row, Says};

/// The report questions: ID, what a yes means, and which way a yes points.
pub const REPORT_SIGNALS: &[(&str, &str, Says)] = &[
    (
        "admits_unmet",
        "the report says part of the task is not done, fails, or does not meet what the task asks",
        Says::Fail,
    ),
    (
        "rests_on_reading",
        "the report says the result rests on an assumption or a reading of the task a grader could read differently",
        Says::Fail,
    ),
    (
        "left_untested",
        "the report says the main behavior the task asks for was not run or not tested",
        Says::Fail,
    ),
    (
        "checked_against_task",
        "the report shows the result run against the task's own example, tests, thresholds, or exact expected output, and passing",
        Says::Pass,
    ),
    (
        "strict_grader_accepts",
        "a strict grader of the task would accept the result the report describes",
        Says::Pass,
    ),
];

const ADMITS_UNMET: &str = "Read the executor's final report in `report` against the task in `task`. Does the report say that some part of what the task asks for is not done, does not work, still fails a test, is infeasible, or does not meet a requirement the task states?";
const RESTS_ON_READING: &str = "Read the executor's final report in `report` against the task in `task`. Does the report say that the result depends on an assumption, a guess, or one reading of an ambiguous part of the task, such that a grader who reads the task differently would reject it?";
const LEFT_UNTESTED: &str = "Read the executor's final report in `report` against the task in `task`. Does the report say that the main behavior the task asks for was never run or tested, or could only be checked indirectly, for example by reading code or building it?";
const CHECKED_AGAINST_TASK: &str = "Read the executor's final report in `report` against the task in `task`. Does the report describe running the finished result against something the task itself states, such as its example, its named tests or command, its thresholds, or its exact expected output, and seeing it pass?";
const STRICT_GRADER: &str = "A strict automated grader will test the result described in `report` against every requirement in `task`, including exact paths, formats, values, and hidden edge cases. Judging only from what the report says was done and checked, would that grader most likely accept the result?";

/// The report question set, asked together over one state.
#[must_use]
pub fn report_questions() -> Questions {
    Questions::new()
        .with(
            "admits_unmet",
            Noul::with_criteria(
                ADMITS_UNMET,
                NoulCriteria::new()
                    .when_true("The report states an unmet requirement, a failing step or test, an unfinished part, or an infeasible result.")
                    .when_false("The report states no unmet requirement; it may still describe limits of how the work was verified."),
            ),
        )
        .with(
            "rests_on_reading",
            Noul::with_criteria(
                RESTS_ON_READING,
                NoulCriteria::new()
                    .when_true("The report names an assumption, guess, or interpretation the result depends on and that the task doesn't settle.")
                    .when_false("The report names no such assumption, or only ones the task settles."),
            ),
        )
        .with(
            "left_untested",
            Noul::with_criteria(
                LEFT_UNTESTED,
                NoulCriteria::new()
                    .when_true("The report says the central behavior wasn't exercised, or was checked only by inspection or a build.")
                    .when_false("The report says the central behavior was exercised, or says nothing about untested behavior."),
            ),
        )
        .with(
            "checked_against_task",
            Noul::with_criteria(
                CHECKED_AGAINST_TASK,
                NoulCriteria::new()
                    .when_true("The report shows a run against a check the task states, and that check passing.")
                    .when_false("The report shows no such run, only the executor's own checks, or a run that didn't pass."),
            ),
        )
        .with(
            "strict_grader_accepts",
            Noul::with_criteria(
                STRICT_GRADER,
                NoulCriteria::new()
                    .when_true("The report shows every requirement done and checked, with nothing left open.")
                    .when_false("Some requirement is unmet, unverified, uncertain, or left to an interpretation."),
            ),
        )
}

/// The most characters of the task and of the report one request carries.
pub const TEXT_CHARS: usize = 7_000;

/// `text` within `max` characters: its head and its tail, since a report
/// states its caveats last.
#[must_use]
pub fn head_tail(text: &str, max: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max {
        return text.to_string();
    }
    let head = max * 2 / 5;
    let tail = max - head;
    let mut out: String = chars[..head].iter().collect();
    out.push_str("\n[…]\n");
    out.extend(&chars[chars.len() - tail..]);
    out
}

/// The state the report questions read.
#[must_use]
pub fn report_state(instruction: &str, report: &str) -> Value {
    json!({
        "task": head_tail(instruction, TEXT_CHARS),
        "report": head_tail(report, TEXT_CHARS),
    })
}

/// The features the verdict reads, in order. Chosen on the calibration
/// half by leave-one-task-out cross-validation among six candidate sets:
/// the grader question, the admission question, and the self-report
/// detector. The other report questions and the scenario verdicts added
/// nothing there.
pub const FEATURES: [&str; 3] = [
    "report.strict_grader_accepts",
    "report.admits_unmet",
    "self-report.detector",
];

/// The fail precision the calibration half must reach at `fail_at`.
pub const FAIL_TARGET: f64 = 0.8;

/// The pass precision the calibration half must reach at `pass_at`.
pub const PASS_TARGET: f64 = 0.85;

/// The fewest calibration trials a cutoff may speak for.
pub const MIN_SUPPORT: usize = 5;

/// The ridge penalty, the step, and the steps of the logistic fit.
const L2: f64 = 1.0;
const STEP: f64 = 0.5;
const STEPS: usize = 3_000;

/// The combined verdict's parameters: a logistic score over
/// [`FEATURES`], and the two cutoffs on it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Params {
    /// The weights, in [`FEATURES`] order.
    pub weights: Vec<f64>,
    pub bias: f64,
    /// At or above this failure probability the verdict is fail.
    pub fail_at: f64,
    /// At or below this failure probability the verdict is pass.
    pub pass_at: f64,
    /// The calibration half's precision for each call, stated with it.
    pub fail_precision: f64,
    pub pass_precision: f64,
    /// The calibration trials each call covered.
    pub fail_support: usize,
    pub pass_support: usize,
}

/// The fitted parameters. [`fit`] reproduces them from the calibration
/// half of the checked-in label rows, `fixtures/truth/rows.jsonl`; a test
/// holds the two equal.
#[must_use]
pub fn fitted() -> Params {
    Params {
        weights: vec![
            -1.098_282_112_540_086_3,
            2.252_439_808_628_326,
            1.198_307_176_561_253_6,
        ],
        bias: -0.657_218_128_204_646_9,
        fail_at: 0.552_922_424_228_451_2,
        pass_at: 0.296_750_804_968_398_9,
        fail_precision: 34.0 / 42.0,
        pass_precision: 20.0 / 23.0,
        fail_support: 42,
        pass_support: 23,
    }
}

/// What the verdict reads about one candidate.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Evidence {
    /// Jev's answers to [`report_questions`], by question ID; `None` when
    /// they weren't asked or didn't arrive.
    pub report_answers: Option<BTreeMap<String, f64>>,
    /// Whether the self-report detector found an admission in the final
    /// report.
    pub admitted: bool,
}

impl Evidence {
    /// The evidence a labeled row holds.
    #[must_use]
    pub fn of_row(row: &Row) -> Evidence {
        Evidence {
            report_answers: row.report_answers.clone(),
            admitted: !row.admissions.is_empty(),
        }
    }

    /// The feature values in [`FEATURES`] order, or `None` when a report
    /// answer is missing.
    #[must_use]
    pub fn features(&self) -> Option<Vec<f64>> {
        let answers = self.report_answers.as_ref()?;
        let strict = *answers.get("strict_grader_accepts")?;
        let unmet = *answers.get("admits_unmet")?;
        if ![strict, unmet]
            .iter()
            .all(|p| p.is_finite() && (0.0..=1.0).contains(p))
        {
            return None;
        }
        Some(vec![strict, unmet, if self.admitted { 1.0 } else { 0.0 }])
    }
}

/// The held-out half's count for each call of [`fitted`]: of the trials
/// it called failed, how many the verifier failed, and of those it called
/// passed, how many the verifier passed. A test measures them again on
/// the checked-in rows.
pub const HELD_OUT_FAIL: (usize, usize) = (22, 37);
pub const HELD_OUT_PASS: (usize, usize) = (11, 13);

fn ratio((k, n): (usize, usize)) -> f64 {
    k as f64 / n.max(1) as f64
}

/// One verdict.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Verdict {
    /// `pass`, `fail`, or `unknown`.
    pub call: String,
    /// The failure probability the score gives; `None` without evidence.
    pub p_fail: Option<f64>,
    /// The held-out half's precision for this call: the share of such
    /// calls the verifier agreed with on tasks the fit never saw. `None`
    /// for unknown.
    pub precision: Option<f64>,
    /// Why, including the population behind any stated precision.
    pub why: String,
}

impl Verdict {
    /// The verdict as a signal.
    #[must_use]
    pub fn says(&self) -> Option<Says> {
        match self.call.as_str() {
            "fail" => Some(Says::Fail),
            "pass" => Some(Says::Pass),
            _ => None,
        }
    }
}

fn sigmoid(x: f64) -> f64 {
    1.0 / (1.0 + (-x).exp())
}

/// The failure probability `params` give feature values `x`.
#[must_use]
pub fn score(params: &Params, x: &[f64]) -> f64 {
    sigmoid(
        params.bias
            + params
                .weights
                .iter()
                .zip(x)
                .map(|(w, v)| w * v)
                .sum::<f64>(),
    )
}

/// The combined verdict on one candidate's evidence.
#[must_use]
pub fn judge(evidence: &Evidence, params: &Params) -> Verdict {
    let Some(x) = evidence.features() else {
        return Verdict {
            call: "unknown".to_string(),
            p_fail: None,
            precision: None,
            why: "Jev didn't answer the report questions".to_string(),
        };
    };
    if params.weights.len() != x.len()
        || !params.bias.is_finite()
        || params.weights.iter().any(|w| !w.is_finite())
        || params.fail_at.is_nan()
        || params.pass_at.is_nan()
        || params.pass_at >= params.fail_at
    {
        return Verdict {
            call: "unknown".to_string(),
            p_fail: None,
            precision: None,
            why: "Invalid verdict parameters".to_string(),
        };
    }
    let p = score(params, &x);
    let (call, precision, why) = if p >= params.fail_at {
        (
            "fail",
            (params == &fitted()).then(|| ratio(HELD_OUT_FAIL)),
            format!(
                "failure probability {p:.2} is at least {:.2}",
                params.fail_at
            ),
        )
    } else if p <= params.pass_at {
        (
            "pass",
            (params == &fitted()).then(|| ratio(HELD_OUT_PASS)),
            format!(
                "failure probability {p:.2} is at most {:.2}",
                params.pass_at
            ),
        )
    } else {
        (
            "unknown",
            None,
            format!(
                "failure probability {p:.2} is between {:.2} and {:.2}",
                params.pass_at, params.fail_at
            ),
        )
    };
    Verdict {
        call: call.to_string(),
        p_fail: Some(p),
        precision,
        why: if precision.is_some() {
            format!(
                "{why}; precision is historical, on 185 mostly Opus trials, not a guarantee for this candidate or Microluna"
            )
        } else {
            why
        },
    }
}

/// The corroboration threshold selected on the original calibration rows.
pub const ADMISSION_AT: f64 = 0.8;
/// Historical comparison counts for the frozen corroborated rule.
pub const CORROBORATED_FAIL: (usize, usize) = (13, 20);

/// An experimental alternative: retain a failure call only with an admission.
/// This trades recall for precision. It is measured alongside the original
/// verdict and does not change the episode's default decision policy.
#[must_use]
pub fn corroborated(evidence: &Evidence, params: &Params) -> Verdict {
    corroborated_at(
        evidence,
        params,
        crate::decision::VERDICT_ADMISSION.threshold(),
    )
}

/// [`corroborated`] with the corroboration threshold on `admits_unmet`
/// given, so a fit can score another value on recorded answers. The stated
/// precision belongs to the fitted parameters at the setting in effect, and
/// is left out otherwise.
#[must_use]
pub fn corroborated_at(evidence: &Evidence, params: &Params, admission: jev::Threshold) -> Verdict {
    let mut verdict = judge(evidence, params);
    if verdict.call == "fail" {
        let supported = evidence.admitted
            || evidence
                .report_answers
                .as_ref()
                .and_then(|a| a.get("admits_unmet"))
                .is_some_and(|p| admission.yes(*p));
        if supported {
            verdict.precision = (params == &fitted()
                && admission == crate::decision::VERDICT_ADMISSION.threshold())
            .then(|| ratio(CORROBORATED_FAIL));
            verdict
                .why
                .push_str("; corroborated by a reported unmet requirement");
        } else {
            verdict.call = "unknown".to_string();
            verdict.precision = None;
            verdict.why =
                "The failure score lacks a corroborating admission; abstaining".to_string();
        }
    }
    verdict
}

/// Fits the verdict on `rows`, which should be the calibration half: a
/// ridge logistic regression on [`FEATURES`] by plain gradient descent
/// from zero, so the fit is deterministic, then the lowest cutoff whose
/// trials reach [`FAIL_TARGET`] and the highest whose trials reach
/// [`PASS_TARGET`], each over at least [`MIN_SUPPORT`] trials and each
/// halfway between two trials' scores. `None` when
/// no row has evidence.
#[must_use]
pub fn fit(rows: &[&Row]) -> Option<Params> {
    let data: Vec<(Vec<f64>, f64)> = rows
        .iter()
        .filter_map(|r| {
            Some((
                Evidence::of_row(r).features()?,
                if r.failed() { 1.0 } else { 0.0 },
            ))
        })
        .collect();
    if data.is_empty() {
        return None;
    }
    let n = data.len() as f64;
    let mut weights = vec![0.0; FEATURES.len()];
    let mut bias = 0.0;
    for _ in 0..STEPS {
        let mut gw = vec![0.0; weights.len()];
        let mut gb = 0.0;
        for (x, y) in &data {
            let z = bias + weights.iter().zip(x).map(|(w, v)| w * v).sum::<f64>();
            let e = sigmoid(z) - y;
            gb += e;
            for (g, v) in gw.iter_mut().zip(x) {
                *g += e * v;
            }
        }
        for (w, g) in weights.iter_mut().zip(&gw) {
            *w -= STEP * (g / n + L2 * *w / n);
        }
        bias -= STEP * gb / n;
    }
    let mut params = Params {
        weights,
        bias,
        fail_at: f64::INFINITY,
        pass_at: f64::NEG_INFINITY,
        fail_precision: 0.0,
        pass_precision: 0.0,
        fail_support: 0,
        pass_support: 0,
    };
    let scored: Vec<(f64, bool)> = data
        .iter()
        .map(|(x, y)| (score(&params, x), *y > 0.5))
        .collect();
    let mut cutoffs: Vec<f64> = scored.iter().map(|(s, _)| *s).collect();
    cutoffs.sort_by(f64::total_cmp);
    cutoffs.dedup();
    // A cutoff sits halfway between two adjacent scores, so a trial is
    // never on it.
    let below = |i: usize| {
        if i == 0 {
            cutoffs[0] - 1e-6
        } else {
            f64::midpoint(cutoffs[i - 1], cutoffs[i])
        }
    };
    let above = |i: usize| {
        cutoffs
            .get(i + 1)
            .map_or(cutoffs[i] + 1e-6, |next| f64::midpoint(cutoffs[i], *next))
    };
    for (i, &t) in cutoffs.iter().enumerate() {
        let said: Vec<bool> = scored
            .iter()
            .filter(|(s, _)| *s >= t)
            .map(|(_, y)| *y)
            .collect();
        let right = said.iter().filter(|y| **y).count();
        if said.len() >= MIN_SUPPORT && right as f64 / said.len() as f64 >= FAIL_TARGET {
            params.fail_at = below(i);
            params.fail_precision = right as f64 / said.len() as f64;
            params.fail_support = said.len();
            break;
        }
    }
    for (i, &t) in cutoffs.iter().enumerate().rev() {
        let said: Vec<bool> = scored
            .iter()
            .filter(|(s, _)| *s <= t)
            .map(|(_, y)| *y)
            .collect();
        let right = said.iter().filter(|y| !**y).count();
        if above(i) < params.fail_at
            && said.len() >= MIN_SUPPORT
            && right as f64 / said.len() as f64 >= PASS_TARGET
        {
            params.pass_at = above(i);
            params.pass_precision = right as f64 / said.len() as f64;
            params.pass_support = said.len();
            break;
        }
    }
    Some(params)
}

/// The verdict's questions and parameters as the summary records them.
#[must_use]
pub fn describe(params: &Params) -> Value {
    json!({
        "features": FEATURES,
        "questions": atif::digest(&serde_json::to_value(report_questions()).unwrap_or(Value::Null)),
        "targets": { "fail_precision": FAIL_TARGET, "pass_precision": PASS_TARGET, "min_support": MIN_SUPPORT },
        "stated": { "fail": HELD_OUT_FAIL, "pass": HELD_OUT_PASS },
        "params": params,
        "corroboration": {"admission_at": ADMISSION_AT, "stated_fail": CORROBORATED_FAIL, "experimental": true},
    })
}

/// Asks the report questions about one candidate and judges it: what an
/// episode calls after its checks. `instruction` is the task's public
/// text and `report` the final report of the session that produced the
/// candidate.
pub async fn assess(
    mode: &crate::component::jev::JevMode,
    recorder: &crate::record::Recorder,
    instruction: &str,
    report: &str,
    deadline: Option<crate::deadline::Deadline>,
    params: &Params,
) -> (Evidence, Verdict, crate::component::jev::Asked) {
    let asked = crate::component::jev::ask(
        mode,
        recorder,
        crate::component::jev::Ask {
            component: "verify.verdict",
            name: "jev_report_verdict",
            id: "jev-report-verdict".to_string(),
            state: report_state(instruction, report),
            questions: report_questions(),
            parent: None,
            deadline,
        },
    )
    .await;
    let answers: BTreeMap<String, f64> = REPORT_SIGNALS
        .iter()
        .filter_map(|(id, _, _)| Some(((*id).to_string(), asked.noul(id)?)))
        .collect();
    let evidence = Evidence {
        report_answers: (!answers.is_empty()).then_some(answers),
        admitted: !super::selfreport::admissions(report).is_empty(),
    };
    let verdict = judge(&evidence, params);
    (evidence, verdict, asked)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows() -> Vec<Row> {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/truth/rows.jsonl");
        super::super::truth::read_rows(&path).expect("the checked-in label rows read")
    }

    #[test]
    fn the_fitted_parameters_refit_from_the_calibration_half() {
        let rows = rows();
        let calibration: Vec<&Row> = rows
            .iter()
            .filter(|r| r.split == super::super::truth::Split::Calibration)
            .collect();
        let refit = fit(&calibration).expect("the calibration half has evidence");
        let fitted = fitted();
        for (a, b) in refit.weights.iter().zip(&fitted.weights) {
            assert!(
                (a - b).abs() < 1e-9,
                "weights {:?} against {:?}",
                refit.weights,
                fitted.weights
            );
        }
        assert!(
            (refit.bias - fitted.bias).abs() < 1e-9,
            "bias {}",
            refit.bias
        );
        assert!(
            (refit.fail_at - fitted.fail_at).abs() < 1e-9,
            "fail_at {}",
            refit.fail_at
        );
        assert!(
            (refit.pass_at - fitted.pass_at).abs() < 1e-9,
            "pass_at {}",
            refit.pass_at
        );
        assert_eq!(
            (refit.fail_support, refit.pass_support),
            (fitted.fail_support, fitted.pass_support)
        );
        assert!((refit.fail_precision - fitted.fail_precision).abs() < 1e-12);
        assert!((refit.pass_precision - fitted.pass_precision).abs() < 1e-12);
    }

    #[test]
    fn no_task_is_in_both_halves_of_the_label_rows() {
        let rows = rows();
        let mut split: BTreeMap<&str, super::super::truth::Split> = BTreeMap::new();
        for r in &rows {
            assert_eq!(
                *split.entry(&r.task).or_insert(r.split),
                r.split,
                "{}",
                r.task
            );
        }
    }

    #[test]
    fn the_verdict_beats_todays_checks_on_the_held_out_half() {
        let rows = rows();
        let held: Vec<&Row> = rows
            .iter()
            .filter(|r| r.split == super::super::truth::Split::HeldOut)
            .collect();
        let params = fitted();
        let count = |f: &dyn Fn(&Row) -> Option<Says>| {
            let said: Vec<&&Row> = held.iter().filter(|r| f(r) == Some(Says::Fail)).collect();
            let right = said.iter().filter(|r| r.failed()).count();
            (right, said.len())
        };
        let verdict = count(&|r| judge(&Evidence::of_row(r), &params).says());
        let today = count(&super::super::truth::todays_checks);
        let failures = held.iter().filter(|r| r.failed()).count();
        let recall = |(right, _): (usize, usize)| super::super::truth::Rate::of(right, failures);
        assert!(
            recall(verdict).low > recall(today).high,
            "the verdict's failure recall {verdict:?} clears today's {today:?} with room"
        );
        let precision = |(right, said): (usize, usize)| right as f64 / said.max(1) as f64;
        assert!(precision(verdict) >= precision(today));
        assert_eq!(
            verdict, HELD_OUT_FAIL,
            "the stated fail precision is the held-out one"
        );
        let passes: Vec<&&Row> = held
            .iter()
            .filter(|r| judge(&Evidence::of_row(r), &params).says() == Some(Says::Pass))
            .collect();
        assert_eq!(
            (passes.iter().filter(|r| !r.failed()).count(), passes.len()),
            HELD_OUT_PASS
        );
    }

    #[test]
    fn missing_answers_make_the_verdict_unknown() {
        let v = judge(&Evidence::default(), &fitted());
        assert_eq!(v.call, "unknown");
        assert_eq!(v.says(), None);
    }

    #[test]
    fn corroboration_replays_the_frozen_comparison_counts() {
        let params = fitted();
        let rows = rows();
        let fail_calls: Vec<_> = rows
            .iter()
            .filter(|r| r.split == super::super::truth::Split::HeldOut)
            .filter(|r| corroborated(&Evidence::of_row(r), &params).says() == Some(Says::Fail))
            .collect();
        assert_eq!(
            (
                fail_calls.iter().filter(|r| r.failed()).count(),
                fail_calls.len()
            ),
            CORROBORATED_FAIL
        );
        for row in &rows {
            let evidence = Evidence::of_row(row);
            if corroborated(&evidence, &params).call == "fail" {
                assert_eq!(judge(&evidence, &params).call, "fail");
            }
        }
    }

    #[test]
    fn invalid_probabilities_abstain_and_custom_fits_have_no_inherited_precision() {
        let mut evidence = Evidence {
            report_answers: Some(BTreeMap::from([
                ("strict_grader_accepts".to_string(), 0.0),
                ("admits_unmet".to_string(), f64::NAN),
            ])),
            admitted: true,
        };
        assert_eq!(judge(&evidence, &fitted()).call, "unknown");
        evidence
            .report_answers
            .as_mut()
            .unwrap()
            .insert("admits_unmet".to_string(), 1.0);
        let mut custom = fitted();
        custom.bias += 0.01;
        let result = judge(&evidence, &custom);
        assert_eq!(result.call, "fail");
        assert!(result.precision.is_none());
    }

    #[test]
    fn the_report_questions_validate() {
        assert!(report_questions().validate().is_ok());
        assert_eq!(report_questions().len(), REPORT_SIGNALS.len());
    }

    #[test]
    fn head_tail_keeps_both_ends() {
        let text = format!("{}{}", "a".repeat(100), "z".repeat(100));
        let clipped = head_tail(&text, 50);
        assert!(clipped.starts_with('a') && clipped.ends_with('z'));
        assert!(clipped.chars().count() < 60);
    }
}
