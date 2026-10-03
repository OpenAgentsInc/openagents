//! Offline regression of the retained #10245 class-only measurement.
use coder_delegate::recipe::{self, TaskClass};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

const ROWS: &str =
    include_str!("../../../docs/cost/2026-10-02-shadow-baseline/class-v2/rows.jsonl");
const RESULTS: &str =
    include_str!("../../../docs/cost/2026-10-02-shadow-baseline/class-v2/results.jsonl");
const LABELS: &str =
    include_str!("../../../docs/cost/2026-10-02-shadow-baseline/class-v2/labels.json");
const RUNS: &str = include_str!("../../../docs/cost/2026-10-02-shadow-baseline/collected.jsonl");

fn lines(text: &str) -> Vec<Value> {
    text.lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(f64::total_cmp);
    let middle = values.len() / 2;
    if values.len() % 2 == 0 {
        (values[middle - 1] + values[middle]) / 2.0
    } else {
        values[middle]
    }
}

#[test]
fn labels_cover_all_105_runs_and_follow_passing_time_and_steps() {
    let labels: Value = serde_json::from_str(LABELS).unwrap();
    // The collected study later gained the Codex arms (#10250); the
    // class-v2 labels cover the five arms measured then.
    let labelled_arms: BTreeSet<String> = labels["labels"][0]["arms"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    let runs: Vec<Value> = lines(RUNS)
        .into_iter()
        .filter(|r| r["arm"].as_str().is_some_and(|a| labelled_arms.contains(a)))
        .collect();
    assert_eq!(runs.len(), 105);
    assert_eq!(labels["labels"].as_array().unwrap().len(), 7);
    let mut covered = BTreeSet::new();
    for label in labels["labels"].as_array().unwrap() {
        let task = label["task"].as_str().unwrap();
        let mut substantial = false;
        for (arm, stats) in label["arms"].as_object().unwrap() {
            let matching: Vec<_> = runs
                .iter()
                .filter(|r| r["task"] == task && r["arm"] == *arm)
                .collect();
            for run in &matching {
                assert!(covered.insert((
                    task.to_owned(),
                    arm.clone(),
                    run["trial"].as_u64().unwrap()
                )));
                let retained = label["runs"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|r| r["arm"] == *arm && r["trial"] == run["trial"])
                    .unwrap();
                assert_eq!(retained["passed"], run["passed"]);
                assert_eq!(retained["wall_s"], run["wall_s"]);
                assert_eq!(retained["cost_usd"], run["cost_usd"]);
                assert_eq!(
                    retained["turns_or_steps"],
                    run.get("turns")
                        .or_else(|| run.get("steps"))
                        .unwrap()
                        .clone()
                );
            }
            let passed: Vec<_> = matching
                .into_iter()
                .filter(|r| r["passed"] == true)
                .collect();
            assert_eq!(
                stats["passing_runs"].as_u64().unwrap() as usize,
                passed.len()
            );
            let wall = median(
                passed
                    .iter()
                    .map(|r| r["wall_s"].as_f64().unwrap())
                    .collect(),
            );
            let steps = median(
                passed
                    .iter()
                    .map(|r| {
                        r.get("turns")
                            .or_else(|| r.get("steps"))
                            .unwrap()
                            .as_f64()
                            .unwrap()
                    })
                    .collect(),
            );
            assert_eq!(stats["median_wall_s"].as_f64().unwrap(), wall);
            assert_eq!(stats["median_turns_or_steps"].as_f64().unwrap(), steps);
            // Raw Claude tool turns and loop steps are not interchangeable.
            if arm != "raw-claude" {
                substantial |= steps > 20.0 || wall > 600.0;
            }
        }
        assert_eq!(
            label["expected"],
            if substantial { "hard" } else { "change" }
        );
    }
    assert_eq!(covered.len(), 105);
}

#[test]
fn measured_answers_use_the_shipped_questions_and_select_the_expected_effort() {
    let rows = lines(ROWS);
    let results = lines(RESULTS);
    assert_eq!(rows.len(), 48);
    assert_eq!(results.len(), rows.len());
    let mut ids = BTreeSet::new();
    let mut counts = BTreeMap::new();
    let mut old_hard = 0;
    for (row, result) in rows.iter().zip(&results) {
        assert!(ids.insert(row["id"].as_str().unwrap()));
        assert_eq!(row["id"], result["id"]);
        assert_eq!(result["set"], recipe::CLASS_SET);
        assert_eq!(result["hard_at"], recipe::HARD_AT);
        assert!(result["error"].is_null());
        let state = recipe::class_state(
            row["state"]["request"].as_str().unwrap(),
            row["state"]["earlier"].as_str().unwrap(),
        );
        let body =
            jev::SystemOneRequest::new(jev::Entry::from(state.clone()), recipe::class_questions())
                .body("jev-1.13.0")
                .unwrap();
        let call = result["steps"]
            .as_array()
            .unwrap()
            .iter()
            .find_map(|s| s.get("call"))
            .unwrap();
        assert_eq!(call["arguments"], Value::Object(body));
        assert_eq!(result["state"], state);
        let answers: Value = serde_json::from_str(call["output"].as_str().unwrap()).unwrap();
        assert_eq!(answers["hard"]["noul"], result["hard"]);
        assert_eq!(answers["asks_only"]["noul"], result["asks_only"]);
        let class =
            recipe::class_of(result["asks_only"].as_f64(), result["hard"].as_f64()).unwrap();
        assert_eq!(class.word(), row["expected"]);
        assert_eq!(result["class"], row["expected"]);
        *counts.entry(class.word()).or_insert(0) += 1;
        old_hard += usize::from(row["v1"]["class"] == "hard");
        assert_eq!(
            route_contract::recipe::effort("codex", Some(class), Some("medium")).as_deref(),
            Some(if class == TaskClass::Hard {
                "high"
            } else {
                "medium"
            })
        );
        assert_eq!(
            route_contract::recipe::effort("claude", Some(class), Some("low")).as_deref(),
            Some(if class == TaskClass::Hard {
                "medium"
            } else {
                "low"
            })
        );
        assert_eq!(result["usage"]["calls"]["decisions"], 1);
        assert_eq!(result["usage"]["calls"]["generation"], 0);
        assert_eq!(result["usage"]["calls"]["delegates"], 0);
        assert_eq!(result["usage"]["cost"]["unknown_calls"], 0);
    }
    assert_eq!(old_hard, 36);
    assert_eq!(
        counts,
        BTreeMap::from([("change", 42), ("hard", 3), ("question", 3)])
    );
}

#[test]
fn cutoff_boundaries_question_precedence_and_missing_answers() {
    assert_eq!(recipe::CLASS_SET, "openagents.delegate.recipe.class.v2");
    assert_eq!(recipe::HARD_AT, 0.8);
    assert_eq!(
        recipe::class_of(Some(0.59), Some(0.799999)),
        Some(TaskClass::Change)
    );
    assert_eq!(
        recipe::class_of(Some(0.59), Some(0.8)),
        Some(TaskClass::Hard)
    );
    assert_eq!(
        recipe::class_of(Some(0.6), Some(1.0)),
        Some(TaskClass::Question)
    );
    assert_eq!(recipe::class_of(None, None), None);
    assert_eq!(recipe::class_of(Some(0.1), None), Some(TaskClass::Change));
    assert_eq!(recipe::class_of(None, Some(0.8)), Some(TaskClass::Hard));
    let state = recipe::class_state(&"x".repeat(7000), &"y".repeat(3000));
    assert_eq!(state["request"], crate_clip_expected('x', 6000));
    assert_eq!(state["earlier"], crate_clip_expected('y', 2000));
}

fn crate_clip_expected(character: char, limit: usize) -> String {
    format!("{}…", character.to_string().repeat(limit))
}
