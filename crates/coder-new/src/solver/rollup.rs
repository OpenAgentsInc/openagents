//! The hand-back rollup: every `hand_back` record in the `events.jsonl`
//! of the runs under a state directory, grouped by class (the target hand
//! and the failing checks), so a class that recurs across issues shows up
//! as a harness fix to make rather than another round to pay.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{Value, json};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Class {
    pub target: String,
    pub failing_checks: Vec<String>,
    pub count: usize,
    pub issues: Vec<u64>,
    pub hypotheses: Vec<String>,
}

impl Class {
    #[must_use]
    pub fn json(&self) -> Value {
        json!({
            "target": self.target,
            "failing_checks": self.failing_checks,
            "count": self.count,
            "issues": self.issues,
            "hypotheses": self.hypotheses,
        })
    }
}

/// The hand-back records in one run's event lines, with the run's issue.
fn hand_backs(events: &str) -> Vec<Value> {
    events
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|event| event["entry"]["tool"] == "hand_back")
        .map(|event| event["entry"]["input"].clone())
        .collect()
}

/// Classes over `(issue, hand_back record)` pairs, most frequent first,
/// then by target and checks for a stable order.
#[must_use]
pub fn classes(records: &[(u64, Value)]) -> Vec<Class> {
    let mut by_key: BTreeMap<(String, Vec<String>), Class> = BTreeMap::new();
    for (issue, record) in records {
        let target = record["target"].as_str().unwrap_or("unknown").to_owned();
        let mut failing_checks: Vec<String> = record["failing_checks"]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        failing_checks.sort();
        let class = by_key
            .entry((target.clone(), failing_checks.clone()))
            .or_insert_with(|| Class {
                target,
                failing_checks,
                count: 0,
                issues: Vec::new(),
                hypotheses: Vec::new(),
            });
        class.count += 1;
        if !class.issues.contains(issue) {
            class.issues.push(*issue);
        }
        if let Some(hypothesis) = record["hypothesis"].as_str() {
            let hypothesis = hypothesis.lines().next().unwrap_or_default().trim();
            if !hypothesis.is_empty() && !class.hypotheses.iter().any(|h| h == hypothesis) {
                class.hypotheses.push(hypothesis.to_owned());
            }
        }
    }
    let mut out: Vec<Class> = by_key.into_values().collect();
    out.sort_by(|a, b| b.count.cmp(&a.count));
    out
}

#[must_use]
pub fn read(state: &Path) -> Vec<Class> {
    let mut records = Vec::new();
    for entry in std::fs::read_dir(state).into_iter().flatten().flatten() {
        let folder = entry.path();
        let issue = std::fs::read(folder.join("summary.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            .and_then(|summary| summary["issue"].as_u64())
            .unwrap_or(0);
        if let Ok(events) = std::fs::read_to_string(folder.join("events.jsonl")) {
            records.extend(hand_backs(&events).into_iter().map(|r| (issue, r)));
        }
    }
    classes(&records)
}

pub fn command(args: &[String]) -> Result<String, String> {
    let mut state = super::default_state();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--state" => state = args.next().ok_or("--state needs a directory")?.into(),
            other => {
                return Err(format!(
                    "unknown option {other}; coder solver hand-backs takes --state DIR"
                ));
            }
        }
    }
    let mut out = String::new();
    for class in read(&state) {
        out.push_str(&class.json().to_string());
        out.push('\n');
    }
    Ok(out)
}
