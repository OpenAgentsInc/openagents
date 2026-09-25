//! A frozen union of candidate-source evidence and explicit failure admissions.
//!
//! This component consumes retained review and audit records without inference.
//! It binds each observation to the supplied candidate before applying the
//! cutoffs frozen in the candidate-review experiment. It never treats absence
//! of a failure finding as proof of success. Runtime policies must opt in.

use futures_util::future::LocalBoxFuture;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{report_audit, review, verdict};
use crate::component::{Component, Fixture, Ran, jev::JevMode};
use crate::record::{Implementation, Recorder};

pub const COMPONENT: &str = "verify.truthful";
pub const REVIEW_AT: f64 = 0.9;
pub const REPORT_AT: f64 = 0.5;
pub const REVIEW_MODEL: &str = "gpt-6-astra";

/// Public candidate evidence and the host's retained observations about it.
/// Grader labels and task identities are not part of this input.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    pub candidate: review::Input,
    pub report: Option<String>,
    pub review: Option<Value>,
    pub audit: Option<Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Assessment {
    pub call: String,
    pub source_score: Option<f64>,
    pub report_score: Option<f64>,
    pub source_available: bool,
    pub report_available: bool,
    pub candidate_digest: String,
    pub implementation: Implementation,
}

#[must_use]
pub fn implementation() -> Implementation {
    Implementation::new(
        COMPONENT,
        "public-source-or-admission-v1",
        &json!({
            "review_threshold": REVIEW_AT, "report_threshold": REPORT_AT,
            "review_model": REVIEW_MODEL, "review_instructions": review::INSTRUCTIONS,
            "review_questions": review::questions(), "report_questions": report_audit::questions(),
            "report_text_chars": verdict::TEXT_CHARS,
            "source_limits": {"task_bytes":64000,"files":100,"file_bytes":160000},
            "no_failure":"unknown",
        }),
    )
}

fn source_score(input: &Input) -> Option<Option<f64>> {
    let record = input.review.as_ref()?;
    let mut expected = review::request(&input.candidate);
    expected.model = REVIEW_MODEL.to_string();
    if record["schema"] != review::SCHEMA
        || !(record["scope"].is_null() || record["scope"] == false)
        || record["reply"]["model"] != REVIEW_MODEL
        || record["input_digest"] != atif::digest(&json!(input.candidate))
        || record["request_digest"] != atif::digest(&review::request_value(&expected))
        || !record["error"].is_null()
    {
        return None;
    }
    let parsed: review::Review = serde_json::from_value(record["review"].clone()).ok()?;
    let observed = record["findings"].as_array()?;
    if parsed.findings.len() > 3 || parsed.findings.len() != observed.len() {
        return None;
    }
    let mut best: Option<f64> = None;
    for (index, (finding, observation)) in parsed.findings.iter().zip(observed).enumerate() {
        if !review::grounded(&input.candidate, finding) {
            continue;
        }
        let expected_state = json!({"task":input.candidate.task,"finding":finding,
            "file":input.candidate.files.get(&finding.path),"coverage":input.candidate.coverage});
        if observation["finding"] != json!(finding) || observation["state"] != expected_state {
            return None;
        }
        let call_id = format!("review-{index}");
        let recorded_call = record["decision_steps"].as_array()?.iter().find(|step| {
            step["call"]["id"] == call_id && step["call"]["name"] == "jev_candidate_counterexample"
        })?;
        if recorded_call["call"]["arguments"]["state"] != expected_state
            || recorded_call["call"]["arguments"]["questions"] != json!(review::questions())
        {
            return None;
        }
        if let Some(score) = review::score(&observation["answers"]) {
            best = Some(best.map_or(score, |value| value.max(score)));
        }
    }
    Some(best)
}

fn report_score(input: &Input) -> Option<f64> {
    let report = input.report.as_deref()?;
    let record = input.audit.as_ref()?;
    let state = verdict::report_state(&input.candidate.task, report);
    let questions = json!(report_audit::questions());
    if record["schema"] != "openagents.coder-one.report-audit.v1"
        || record["state"] != state
        || record["questions"] != questions
        || record["digest"] != crate::component::jev::key(&state, &questions)
        || !record["error"].is_null()
    {
        return None;
    }
    report_audit::score(&record["answers"])
}

/// Return fail or unknown from observations of this exact candidate.
///
/// Recorded summary scores are ignored: code recomputes them from the retained
/// answers. Stale records, different models or questions, and incomplete
/// judgments cannot inherit this rule's failure call.
#[must_use]
pub fn judge(input: &Input) -> Assessment {
    let source = source_score(input);
    let report = report_score(input);
    let source_score = source.flatten();
    Assessment {
        call: if source_score.is_some_and(|p| p >= REVIEW_AT)
            || report.is_some_and(|p| crate::decision::TRUTHFUL_REPORT.yes(p))
        {
            "fail"
        } else {
            "unknown"
        }
        .to_string(),
        source_score,
        report_score: report,
        source_available: source.is_some(),
        report_available: report.is_some(),
        candidate_digest: atif::digest(&json!(input.candidate)),
        implementation: implementation(),
    }
}

/// The component runner and orchestration code share [`judge`]. This operation
/// makes no new model calls; producing its source review has a separate cost.
pub struct Truthful;

impl Component for Truthful {
    fn id(&self) -> &'static str {
        COMPONENT
    }
    fn implementation(&self) -> Implementation {
        implementation()
    }
    fn about(&self) -> &'static str {
        "Combine attributable source-review and report-audit evidence into fail or unknown."
    }
    fn run<'a>(
        &'a self,
        fixture: &'a Fixture,
        _: &'a JevMode,
        _: &'a Recorder,
    ) -> LocalBoxFuture<'a, Result<Ran, String>> {
        Box::pin(async move {
            let input: Input =
                serde_json::from_value(fixture.input.clone()).map_err(|e| e.to_string())?;
            let assessment = judge(&input);
            Ok(Ran {
                metrics: serde_json::from_value(json!({"failed":assessment.call=="fail",
                    "source_score":assessment.source_score,"report_score":assessment.report_score}))
                .expect("object"),
                output: json!(assessment),
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn admitted() -> Input {
        let candidate = review::Input {
            task: "Write a working parser.".into(),
            files: Default::default(),
            coverage: "No source retained.".into(),
        };
        let report = "The parser is unfinished.".to_string();
        let state = verdict::report_state(&candidate.task, &report);
        let questions = json!(report_audit::questions());
        let digest = crate::component::jev::key(&state, &questions);
        let audit = json!({"schema":"openagents.coder-one.report-audit.v1","state":state,
            "questions":questions,"digest":digest,"answers":{"observed":{"noul":0.9},"current":{"noul":0.8},"required":{"noul":0.7}}});
        Input {
            candidate,
            report: Some(report),
            review: None,
            audit: Some(audit),
        }
    }

    #[test]
    fn stale_reports_and_missing_conditions_abstain() {
        let mut input = admitted();
        assert_eq!(judge(&input).call, "fail");
        input.report = Some("The parser now works.".into());
        assert_eq!(judge(&input).call, "unknown");
        let mut input = admitted();
        input.audit.as_mut().unwrap()["answers"]["current"] = Value::Null;
        input.audit.as_mut().unwrap()["score"] = json!(1.0);
        assert_eq!(judge(&input).call, "unknown");
    }

    #[test]
    fn different_questions_do_not_inherit_the_cutoff() {
        let mut input = admitted();
        input.audit.as_mut().unwrap()["questions"] = json!({});
        assert_eq!(judge(&input).call, "unknown");
        let mut input = admitted();
        input.candidate.task = "Write a draft parser.".into();
        assert_eq!(judge(&input).call, "unknown");
    }

    #[test]
    fn source_evidence_belongs_to_the_reviewed_files_and_model() {
        let mut input = admitted();
        input.audit = None;
        input.candidate.task = "Return twice the integer x.".into();
        input
            .candidate
            .files
            .insert("answer.py".into(), "def answer(x): return x".into());
        let finding = review::Finding {
            requirement: input.candidate.task.clone(),
            path: "answer.py".into(),
            quote: "return x".into(),
            example: "x=2".into(),
            expected: "4".into(),
            actual: "2".into(),
            reasoning: "The function returns its input unchanged.".into(),
        };
        let mut request = review::request(&input.candidate);
        request.model = REVIEW_MODEL.into();
        let state = json!({"task":input.candidate.task,"finding":finding,
            "file":input.candidate.files.get(&finding.path),"coverage":input.candidate.coverage});
        input.review = Some(
            json!({"schema":review::SCHEMA,"scope":false,"reply":{"model":REVIEW_MODEL},
            "input_digest":atif::digest(&json!(input.candidate)),"request_digest":atif::digest(&review::request_value(&request)),
            "review":{"findings":[finding],"coverage":"Static review."},
            "decision_steps":[{"call":{"id":"review-0","name":"jev_candidate_counterexample","arguments":{"state":state,"questions":review::questions()}}}],
            "findings":[{"finding":finding,"state":state,"answers":{"violation":{"noul":0.95},"consequential":{"noul":0.97}},"score":0.0}]}),
        );
        assert_eq!(judge(&input).call, "fail");
        let original = input.clone();
        input
            .review
            .as_mut()
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove("scope");
        assert_eq!(judge(&input).call, "fail");
        input.review.as_mut().unwrap()["decision_steps"][0]["call"]["arguments"]["questions"] =
            json!({});
        assert_eq!(judge(&input).call, "unknown");
        input = original.clone();
        input
            .candidate
            .files
            .insert("answer.py".into(), "def answer(x): return 2*x".into());
        assert_eq!(judge(&input).call, "unknown");
        let mut input = original.clone();
        input.review.as_mut().unwrap()["reply"]["model"] = json!("gpt-6-luna");
        assert_eq!(judge(&input).call, "unknown");
        let mut input = original;
        input.review.as_mut().unwrap()["findings"][0]["answers"]["violation"] = Value::Null;
        input.review.as_mut().unwrap()["findings"][0]["score"] = json!(1.0);
        assert_eq!(judge(&input).call, "unknown");
    }
}
