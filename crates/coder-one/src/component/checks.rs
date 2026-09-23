//! `verify.checks` as a component: a synthetic case, or a candidate
//! recovered from a retained trial with its verifier reward.

use futures_util::future::LocalBoxFuture;
use serde::Deserialize;
use serde_json::{Map, Value, json};

use super::{Component, Fixture, Ran, input};
use crate::checks::{self, Input, synthetic};
use crate::component::jev::JevMode;
use crate::record::{Implementation, Recorder};

#[derive(Deserialize)]
struct ChecksInput {
    /// A synthetic case by name.
    #[serde(default)]
    synthetic: Option<String>,
    /// Or a whole check input, such as a recovered candidate's.
    #[serde(default)]
    input: Option<Input>,
    /// The verifier's reward for a recovered candidate.
    #[serde(default)]
    reward: Option<f64>,
}

/// `verify.checks`: admitted scenarios run against one candidate.
pub struct Checks;

impl Component for Checks {
    fn id(&self) -> &'static str {
        "verify.checks"
    }
    fn implementation(&self) -> Implementation {
        checks::implementation()
    }
    fn about(&self) -> &'static str {
        "Admitted scenarios observe each requirement against the candidate."
    }
    fn run<'a>(
        &'a self,
        fixture: &'a Fixture,
        _jev: &'a JevMode,
        recorder: &'a Recorder,
    ) -> LocalBoxFuture<'a, Result<Ran, String>> {
        Box::pin(async move {
            let given: ChecksInput = input(fixture)?;
            let (input, case) = match (&given.synthetic, given.input) {
                (Some(name), _) => {
                    let case =
                        synthetic::case(name).ok_or_else(|| format!("no synthetic case {name}"))?;
                    (case.input.clone(), Some(case))
                }
                (None, Some(input)) => (input, None),
                (None, None) => {
                    return Err(
                        "the input names no synthetic case and holds no check input".to_string()
                    );
                }
            };
            let scratch = std::env::temp_dir().join(format!(
                "coder-one-component-checks-{}-{}",
                std::process::id(),
                atif::now_ms()
            ));
            let report = checks::check(&input, recorder, &scratch).await;
            let count = |word: &str| report.verdicts.iter().filter(|v| v.verdict == word).count();
            let verdict = |id: &str| {
                report
                    .verdicts
                    .iter()
                    .find(|v| v.scenario == id)
                    .map_or("not run", |v| v.verdict.as_str())
            };
            let mut metrics = Map::new();
            metrics.insert("scenarios".to_string(), json!(report.verdicts.len()));
            metrics.insert("failed".to_string(), json!(count("failed")));
            metrics.insert("passed".to_string(), json!(count("passed")));
            metrics.insert(
                "unavailable".to_string(),
                json!(count("unavailable") + count("inconclusive")),
            );
            let conclusive = count("passed") + count("failed") > 0;
            if let Some(case) = &case {
                let separates = case.fails.iter().all(|id| verdict(id) == "failed")
                    && case.passes.iter().all(|id| verdict(id) == "passed");
                metrics.insert(
                    "separates".to_string(),
                    if conclusive {
                        json!(separates)
                    } else {
                        Value::Null
                    },
                );
            }
            metrics.insert(
                "detected_failure".to_string(),
                match given.reward {
                    Some(r) if r == 0.0 && conclusive => json!(report.detected()),
                    _ => Value::Null,
                },
            );
            metrics.insert(
                "false_alarm".to_string(),
                match given.reward {
                    Some(r) if r == 1.0 && conclusive => json!(report.detected()),
                    _ => Value::Null,
                },
            );
            Ok(Ran {
                output: json!({
                    "candidate": report.candidate["digest"],
                    "verdicts": report.verdicts.iter().map(|v| json!({ "scenario": v.scenario, "verdict": v.verdict })).collect::<Vec<_>>(),
                    "requirements": report.coverage.iter().filter(|c| !c.scenarios.is_empty()).map(|c| json!({ "id": c.id, "state": c.state })).collect::<Vec<_>>(),
                    "hypotheses": report.packets.iter().map(|p| json!({ "requirement": p.requirement, "scenario": p.scenario, "hypotheses": p.hypotheses })).collect::<Vec<_>>(),
                    "ineligible": report.ineligible,
                }),
                metrics,
            })
        })
    }
}
