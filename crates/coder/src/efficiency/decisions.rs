//! Join retained gates to independent run outcomes. Run success is a proxy,
//! not a label that an engine, risk, or task-class prediction was correct.

use route_contract::decision::DecisionReading;
use route_contract::lifecycle::CheckLabel;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// One gate and its joined outcome. Missing checks and costs stay unknown.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Row {
    pub reading: DecisionReading,
    pub request: Option<String>,
    pub task: Option<String>,
    pub independent_pass: Option<bool>,
    pub cost_microusd: Option<u64>,
    pub wall_ms: Option<u64>,
}

fn checked(label: CheckLabel) -> Option<bool> {
    match label {
        CheckLabel::Verified => Some(true),
        CheckLabel::CheckFailed => Some(false),
        _ => None,
    }
}

/// Read latest route records and current task traces from explicit roots.
/// The task owner is authoritative for check results; a route can be stale.
pub fn rows(store: &Path) -> Vec<Row> {
    let tasks = crate::task::Store::open(store)
        .and_then(|s| s.list())
        .unwrap_or_default();
    let by_task: BTreeMap<_, _> = tasks.iter().map(|t| (t.task_id.as_str(), t)).collect();
    let journal = openagents_chat::route::Journal::beside(store);
    let mut records = Vec::new();
    if let Ok(entries) = std::fs::read_dir(store.with_file_name("routes")) {
        let mut threads: Vec<_> = entries
            .flatten()
            .filter_map(|e| {
                e.file_name()
                    .to_str()?
                    .strip_suffix(".jsonl")
                    .map(str::to_owned)
            })
            .collect();
        threads.sort();
        for thread in threads {
            records.extend(journal.records(&thread));
        }
    }
    let mut rows = Vec::new();
    let mut seen = BTreeSet::new();
    for record in records {
        // A fan-out joins each gate to each run, with task identity retained.
        for reading in record.decisions {
            if record.runs.is_empty() {
                rows.push(Row {
                    reading,
                    request: Some(record.request.clone()),
                    task: None,
                    independent_pass: None,
                    cost_microusd: None,
                    wall_ms: None,
                });
                continue;
            }
            for run in &record.runs {
                if !seen.insert((
                    record.request.clone(),
                    run.task.clone(),
                    serde_json::to_string(&reading).unwrap_or_default(),
                )) {
                    continue;
                }
                let observed = by_task
                    .get(run.task.as_str())
                    .map(|t| crate::task::lifecycle::observation(t));
                let check = observed
                    .as_ref()
                    .map(|o| route_contract::lifecycle::project(o.disposition).check)
                    .unwrap_or(run.projection.check);
                rows.push(Row {
                    reading: reading.clone(),
                    request: Some(record.request.clone()),
                    task: Some(run.task.clone()),
                    independent_pass: checked(check),
                    cost_microusd: observed
                        .as_ref()
                        .and_then(|o| o.cost_microusd)
                        .or(run.cost_microusd),
                    wall_ms: observed.as_ref().and_then(|o| o.wall_ms).or(run.wall_ms),
                });
            }
        }
    }
    for task in tasks {
        let Some(run) = &task.run else { continue };
        let Ok(trace) = atif::log::read_whole(&store.join(&run.admission.trace_file)) else {
            continue;
        };
        let outcome_key = format!("{}-{}", task.task_id, task.turn());
        let check = checked(crate::task::lifecycle::project(&task).check);
        for step in trace.steps {
            // Continuations carry earlier evidence, which is not a new prediction.
            if step.extensions.contains_key("carried_from") {
                continue;
            }
            let readings: Vec<DecisionReading> = step
                .extensions
                .get("decision_readings")
                .cloned()
                .and_then(|v| serde_json::from_value(v).ok())
                .unwrap_or_default();
            for reading in readings {
                if reading.outcome_key != outcome_key {
                    continue;
                }
                rows.push(Row {
                    reading,
                    request: None,
                    task: Some(task.task_id.clone()),
                    independent_pass: check,
                    cost_microusd: run.result.as_ref().and_then(|r| r.cost_microusd),
                    wall_ms: run.result.as_ref().map(|r| r.elapsed_ms),
                });
            }
        }
    }
    rows
}

/// Per-question and policy-site counts, operating-point accuracy against
/// independent pass/fail, and ten reliability bins of the raw probability.
/// Models, question sets, options, and thresholds never share a group.
pub fn report(rows: &[Row]) -> Value {
    let mut groups: BTreeMap<String, Vec<&Row>> = BTreeMap::new();
    for row in rows {
        let r = &row.reading;
        let key = json!([
            r.question,
            r.site,
            r.question_set,
            r.model,
            r.option,
            r.threshold,
            r.comparison
        ])
        .to_string();
        groups.entry(key).or_default().push(row);
    }
    let questions: Vec<Value> = groups.values().map(|rows| {
        let r = &rows[0].reading;
        let labeled: Vec<_> = rows.iter().filter(|r| r.independent_pass.is_some()).collect();
        let accuracy = (!labeled.is_empty()).then(|| labeled.iter().filter(|row| Some(row.reading.decision) == row.independent_pass).count() as f64 / labeled.len() as f64);
        let reliability: Vec<_> = (0..10).map(|i| {
            let in_bin: Vec<_> = labeled.iter().filter(|row| {
                let p = row.reading.raw_probability.as_f64().unwrap_or(-1.0);
                p >= i as f64 / 10.0 && (p < (i + 1) as f64 / 10.0 || (i == 9 && p == 1.0))
            }).collect();
            let mean = (!in_bin.is_empty()).then(|| in_bin.iter().map(|r| r.reading.raw_probability.as_f64().unwrap()).sum::<f64>() / in_bin.len() as f64);
            let rate = (!in_bin.is_empty()).then(|| in_bin.iter().filter(|r| r.independent_pass == Some(true)).count() as f64 / in_bin.len() as f64);
            json!({"lower": i as f64 / 10.0, "upper": (i+1) as f64 / 10.0, "n": in_bin.len(), "mean_probability": mean, "observed_pass_rate": rate})
        }).collect();
        json!({"question": r.question, "site": r.site, "question_set": r.question_set, "model": r.model, "option": r.option, "threshold": r.threshold, "comparison": r.comparison, "n": rows.len(), "checked_n": labeled.len(), "unchecked_n": rows.len()-labeled.len(), "accuracy_at_threshold": accuracy, "reliability": reliability})
    }).collect();
    json!({"schema": "openagents.efficiency.decisions.v1", "label": "independent_run_pass_proxy", "notes": "Accuracy compares the recorded gate decision with independent run pass/fail. Run success is a proxy, not question-specific correctness; unchecked runs are excluded. Fan-out contributes one joined sample per run. Reliability uses raw probabilities.", "questions": questions, "joined": rows})
}

pub fn text(report: &Value) -> String {
    let mut lines = vec![
        "Decisions · joined independent run outcomes".to_owned(),
        report["notes"].as_str().unwrap_or("").to_owned(),
    ];
    for q in report["questions"].as_array().into_iter().flatten() {
        let accuracy = q["accuracy_at_threshold"]
            .as_f64()
            .map(|a| format!("{:.1}%", 100.0 * a))
            .unwrap_or_else(|| "unknown".into());
        lines.push(format!("{} / {} · model {} · option {} · threshold {} {} · n={} checked={} unchecked={} · accuracy at threshold (run-pass proxy): {}", q["question"].as_str().unwrap_or(""), q["site"].as_str().unwrap_or(""), q["model"].as_str().unwrap_or(""), q["option"], q["comparison"].as_str().unwrap_or(""), q["threshold"], q["n"], q["checked_n"], q["unchecked_n"], accuracy));
        lines.push("  Raw probability bin   n   mean p   observed pass rate".into());
        for b in q["reliability"].as_array().into_iter().flatten() {
            let number = |key: &str| {
                b[key]
                    .as_f64()
                    .map(|v| format!("{v:.3}"))
                    .unwrap_or_else(|| "—".into())
            };
            lines.push(format!(
                "  {:.1}–{:.1}              {}   {}   {}",
                b["lower"].as_f64().unwrap(),
                b["upper"].as_f64().unwrap(),
                b["n"],
                number("mean_probability"),
                number("observed_pass_rate")
            ));
        }
    }
    if report["questions"].as_array().is_none_or(Vec::is_empty) {
        lines.push(
            "No recorded decision readings yet. Legacy records have no probabilities.".into(),
        );
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_counts_accuracy_bins_and_unknowns() {
        let rows: Vec<Row> =
            serde_json::from_str(include_str!("../../fixtures/efficiency/decisions.json")).unwrap();
        let value = report(&rows);
        let q = &value["questions"][0];
        assert_eq!(q["n"], 5);
        assert_eq!(q["checked_n"], 4);
        assert_eq!(q["unchecked_n"], 1);
        assert_eq!(q["accuracy_at_threshold"], 0.5);
        assert_eq!(q["reliability"][0]["n"], 1);
        assert_eq!(q["reliability"][2]["observed_pass_rate"], 0.0);
        assert_eq!(q["reliability"][8]["mean_probability"], 0.8);
        assert_eq!(q["reliability"][9]["n"], 1, "p=1 belongs to last bin");
        assert!(q["reliability"][3]["observed_pass_rate"].is_null());
        assert_eq!(value["joined"][0]["cost_microusd"], 40502);
        assert_eq!(value["joined"][0]["wall_ms"], 1200);
        let rendered = text(&value);
        assert!(rendered.contains("n=5 checked=4 unchecked=1"));
        assert!(rendered.contains("50.0%"));
        assert!(rendered.contains("Raw probability bin"));
        assert!(rendered.contains("run-pass proxy"));
    }

    #[test]
    fn models_thresholds_sets_and_options_are_separate_and_empty_is_unknown() {
        let mut rows: Vec<Row> =
            serde_json::from_str(include_str!("../../fixtures/efficiency/decisions.json")).unwrap();
        rows[0].reading.model = "another-model".into();
        rows[1].reading.threshold = serde_json::Number::from_f64(0.5).unwrap();
        rows[2].reading.question_set = "another-set".into();
        rows[3].reading.option = Some("another-option".into());
        assert_eq!(report(&rows)["questions"].as_array().unwrap().len(), 5);
        let q = report(&rows[4..]);
        assert!(q["questions"][0]["accuracy_at_threshold"].is_null());
        assert!(text(&report(&[])).contains("No recorded decision readings"));
    }

    #[test]
    fn joins_latest_route_fixture_by_request_and_task_with_legacy_support() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("tasks");
        std::fs::create_dir_all(&store).unwrap();
        std::fs::create_dir_all(dir.path().join("routes")).unwrap();
        let mut route: Value =
            serde_json::from_str(include_str!("../../fixtures/efficiency/route-record.json"))
                .unwrap();
        let samples: Vec<Row> =
            serde_json::from_str(include_str!("../../fixtures/efficiency/decisions.json")).unwrap();
        let mut reading = samples[0].reading.clone();
        reading.outcome_key = route["request"].as_str().unwrap().into();
        route["decisions"] = json!([reading]);
        let old = route.clone();
        route["runs"][0]["projection"]["check"] = json!("verified");
        let thread = route["thread"].as_str().unwrap();
        let mut legacy = route.clone();
        legacy["request"] = json!("legacy-request");
        legacy.as_object_mut().unwrap().remove("decisions");
        std::fs::write(
            dir.path().join("routes").join(format!("{thread}.jsonl")),
            format!("{old}\n{route}\n{legacy}\n"),
        )
        .unwrap();
        let joined = rows(&store);
        assert_eq!(
            joined.len(),
            1,
            "latest request only; no invented legacy probabilities"
        );
        assert_eq!(joined[0].independent_pass, Some(true));
        assert_eq!(joined[0].request.as_deref(), route["request"].as_str());
        assert_eq!(joined[0].task.as_deref(), route["runs"][0]["task"].as_str());
        assert_eq!(joined[0].cost_microusd, Some(40502));
        assert_eq!(joined[0].wall_ms, route["runs"][0]["wall_ms"].as_u64());
    }
}
