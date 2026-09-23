//! `verify.support` as a component: one labeled candidate's frozen
//! evidence, judged per requirement, beside the broad "done" baseline.

use futures_util::future::LocalBoxFuture;
use serde::Deserialize;
use serde_json::{Map, Value, json};

use super::{Component, Fixture, Ran, ask_one, input};
use crate::checks::TaskText;
use crate::component::evidence;
use crate::component::jev::JevMode;
use crate::record::{Implementation, Recorder};
use crate::support::{self, Evidence, Params, Skipped};

#[derive(Deserialize)]
struct Label {
    requirement: String,
    met: bool,
}

#[derive(Deserialize, Default)]
struct Baseline {
    #[serde(default)]
    done: Option<f64>,
    #[serde(default)]
    request: Option<CloseRequest>,
}

#[derive(Deserialize)]
struct CloseRequest {
    report: String,
    changes: String,
    #[serde(default)]
    criteria: Vec<String>,
}

#[derive(Deserialize)]
struct SupportInput {
    task: TaskText,
    candidate: Value,
    evidence: Vec<Evidence>,
    #[serde(default)]
    skipped: Vec<Skipped>,
    #[serde(default)]
    labels: Vec<Label>,
    #[serde(default)]
    outcome: Value,
    #[serde(default)]
    baseline: Baseline,
    #[serde(default)]
    split: Option<String>,
    #[serde(default)]
    params: Option<Params>,
}

/// `verify.support`: paired Jev judgments per requirement.
pub struct Support;

impl Component for Support {
    fn id(&self) -> &'static str {
        "verify.support"
    }
    fn implementation(&self) -> Implementation {
        support::implementation(Params::default())
    }
    fn about(&self) -> &'static str {
        "Jev judges whether the evidence supports each requirement, and separately whether it contradicts it."
    }
    fn run<'a>(
        &'a self,
        fixture: &'a Fixture,
        jev: &'a JevMode,
        recorder: &'a Recorder,
    ) -> LocalBoxFuture<'a, Result<Ran, String>> {
        Box::pin(async move {
            let given: SupportInput = input(fixture)?;
            let params = given.params.unwrap_or_default();
            let candidate = given.candidate["digest"]
                .as_str()
                .unwrap_or_default()
                .to_string();
            let report = support::judge_evidence(
                &given.task,
                &candidate,
                given.evidence,
                given.skipped,
                jev,
                recorder,
                params,
                None,
            )
            .await;
            // The broad "done" baseline: the retained answer, or the
            // closing check the episode asks, through the same Jev mode.
            let (done, done_how) = match (&given.baseline.done, &given.baseline.request) {
                (Some(p), _) => (Some(*p), "retained"),
                (None, Some(request)) => {
                    let issue = evidence::issue_state(&given.task.title, &given.task.instruction);
                    let asked = ask_one(
                        jev,
                        recorder,
                        "verify.close",
                        "jev_close",
                        evidence::close_request(
                            &issue,
                            &request.criteria,
                            &request.report,
                            &request.changes,
                        ),
                    )
                    .await;
                    (asked.noul("done"), asked.how)
                }
                (None, None) => (None, "none"),
            };
            let label = |id: &str| {
                given
                    .labels
                    .iter()
                    .find(|l| l.requirement == id)
                    .map(|l| l.met)
            };
            let mut metrics = Map::new();
            let count = |word: &str| report.states.iter().filter(|s| s.state == word).count();
            metrics.insert("judged".to_string(), json!(report.states.len()));
            metrics.insert("supported".to_string(), json!(count("supported")));
            metrics.insert("contradicted".to_string(), json!(count("contradicted")));
            metrics.insert("unresolved".to_string(), json!(count("unresolved")));
            let labeled: Vec<(bool, &support::State)> = report
                .states
                .iter()
                .filter_map(|s| label(&s.id).map(|m| (m, s)))
                .collect();
            metrics.insert("labeled".to_string(), json!(labeled.len()));
            metrics.insert(
                "false_accepts".to_string(),
                json!(
                    labeled
                        .iter()
                        .filter(|(m, s)| !m && s.state == "supported")
                        .count()
                ),
            );
            metrics.insert(
                "false_rejects".to_string(),
                json!(
                    labeled
                        .iter()
                        .filter(|(m, s)| *m && s.state != "supported")
                        .count()
                ),
            );
            let met = given.outcome.get("met").and_then(Value::as_bool);
            metrics.insert("done".to_string(), json!(done));
            metrics.insert(
                "done_false_accept".to_string(),
                match (met, done) {
                    (Some(m), Some(p)) => json!(!m && p >= evidence::YES),
                    _ => Value::Null,
                },
            );
            metrics.insert(
                "done_false_reject".to_string(),
                match (met, done) {
                    (Some(m), Some(p)) => json!(m && p < evidence::YES),
                    _ => Value::Null,
                },
            );
            let accepted =
                !report.states.is_empty() && report.states.iter().all(|s| s.state == "supported");
            metrics.insert(
                "support_false_accept".to_string(),
                met.map_or(Value::Null, |m| json!(!m && accepted)),
            );
            metrics.insert(
                "support_false_reject".to_string(),
                met.map_or(Value::Null, |m| json!(m && !accepted)),
            );
            Ok(Ran {
                output: json!({
                    "split": given.split.unwrap_or_else(|| "development".to_string()),
                    "candidate": candidate,
                    "outcome": given.outcome,
                    "done": done,
                    "done_how": done_how,
                    "states": report.states.iter().map(|s| json!({
                        "id": s.id,
                        "state": s.state,
                        "why": s.why,
                        "supports": s.judgment.supports,
                        "contradicts": s.judgment.contradicts,
                        "how": s.judgment.how,
                        "clipped": s.evidence.clipped(),
                        "scenario_state": s.scenario_state,
                        "label": label(&s.id),
                    })).collect::<Vec<_>>(),
                    "skipped": report.skipped,
                }),
                metrics,
            })
        })
    }
}
