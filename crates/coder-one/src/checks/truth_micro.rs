//! Select Microluna evidence for the workspace that received the final grade.
//! Reading these records never executes the candidate or the evaluator.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The self-written evaluator's score, not a benchmark outcome.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalScore {
    pub passed: u64,
    pub total: u64,
}

impl LocalScore {
    fn read(value: &Value) -> Option<Self> {
        let passed = value["passed"].as_u64()?;
        let total = value["total"].as_u64()?;
        (total > 0 && passed <= total).then_some(Self { passed, total })
    }
}

/// Evidence tied to the selected session, or an explicit reason it is missing.
#[derive(Default)]
pub(crate) struct Selected {
    pub report: Option<String>,
    pub score: Option<LocalScore>,
    pub source: Option<String>,
    pub unavailable: Option<String>,
    pub reviews: Vec<u64>,
}

fn missing(reason: &str) -> Selected {
    Selected {
        unavailable: Some(reason.to_string()),
        ..Selected::default()
    }
}

/// Prefer a recorded submission, then a restore, then the last sequential
/// session. A failed or ambiguous selection never falls back to a later report.
pub(crate) fn select(record: &Value, source: &str) -> Selected {
    let Some(sessions) = record["sessions"].as_array().filter(|s| !s.is_empty()) else {
        return missing("Microluna has no recorded sessions");
    };
    let moves: Vec<&Value> = record["moves"].as_array().into_iter().flatten().collect();
    let last = sessions.last().expect("nonempty sessions");
    let selected = if record["mode"] == "lean" {
        if let Some(submitted) = moves.iter().rev().find(|m| m["kind"] == "lean.submitted") {
            if submitted["selection_matches_workspace"] != true {
                return missing("The submitted workspace does not match a recorded candidate");
            }
            submitted["selected_session"].as_u64()
        } else if let Some(restored) = moves.iter().rev().find(|m| m["kind"] == "lean.restore") {
            restored["session"].as_u64()
        } else if record["policy"]["lean"]["lanes"].as_u64().unwrap_or(1) > 1 {
            return missing("Parallel lean candidates have no explicit submission identity");
        } else if record["stopped"]
            .as_str()
            .is_some_and(|s| s.contains("restoring the best workspace failed"))
        {
            return missing("Restoring the selected candidate failed");
        } else {
            last["number"].as_u64()
        }
    } else if matches!(record["mode"].as_str(), Some("single" | "requirements")) {
        last["number"].as_u64()
    } else {
        return missing("This Microluna mode has no unambiguous final report adapter");
    };
    let Some(session) = sessions
        .iter()
        .find(|s| selected.is_some() && s["number"].as_u64() == selected)
    else {
        return missing("The selected Microluna session is missing");
    };
    let mut report = ["summary", "answer"]
        .into_iter()
        .filter_map(|k| session["finish"][k].as_str())
        .filter(|s| !s.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    let selected_files = moves
        .iter()
        .find(|m| m["kind"] == "lean" && m["after_session"].as_u64() == selected)
        .and_then(|m| m["workspace_files"].as_object())
        .filter(|files| !files.is_empty());
    let mut reviews = Vec::new();
    if let Some(files) = selected_files {
        for review in sessions.iter().filter(|s| {
            s["number"].as_u64() > selected
                && s["read_only"] == true
                && s["changed_workspace"] == false
        }) {
            let number = review["number"].as_u64();
            let same_files = moves.iter().any(|m| {
                m["kind"] == "lean"
                    && m["after_session"].as_u64() == number
                    && m["snapshot_error"].is_null()
                    && m["workspace_files"].as_object() == Some(files)
            });
            if !same_files {
                continue;
            }
            let text = ["summary", "answer"]
                .into_iter()
                .filter_map(|k| review["finish"][k].as_str())
                .filter(|s| !s.trim().is_empty())
                .collect::<Vec<_>>()
                .join("\n\n");
            if let Some(number) = number.filter(|_| !text.is_empty()) {
                report.push_str(&format!(
                    "\n\nRead-only review {number} of the same candidate files:\n{text}"
                ));
                reviews.push(number);
            }
        }
    }
    let score = moves
        .iter()
        .rev()
        .find(|m| m["kind"] == "lean.submitted")
        .or_else(|| {
            moves
                .iter()
                .rev()
                .find(|m| m["kind"] == "lean" && m["after_session"].as_u64() == selected)
        })
        .and_then(|m| LocalScore::read(&m["score"]));
    Selected {
        report: (!report.is_empty()).then_some(report.clone()),
        reviews,
        score,
        source: Some(format!("{source}#session-{}", selected.unwrap_or(0))),
        unavailable: report
            .is_empty()
            .then(|| "The selected session has no final report".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn record() -> Value {
        json!({"mode":"lean", "sessions":[
            {"number":1,"finish":{"summary":"selected report"}},
            {"number":2,"finish":{"summary":"discarded report"}}
        ], "moves":[
            {"kind":"lean","after_session":1,"score":{"passed":3,"total":4}},
            {"kind":"lean","after_session":2,"score":{"passed":1,"total":4}},
            {"kind":"lean.restore","session":1}
        ]})
    }

    #[test]
    fn restoration_uses_the_selected_report_and_score() {
        let selected = select(&record(), "microluna-1.json");
        assert_eq!(selected.report.as_deref(), Some("selected report"));
        assert_eq!(
            selected.score,
            Some(LocalScore {
                passed: 3,
                total: 4
            })
        );
    }

    #[test]
    fn ambiguous_submission_does_not_inherit_the_final_grade() {
        let mut r = record();
        r["moves"]
            .as_array_mut()
            .unwrap()
            .push(json!({"kind":"lean.submitted","selection_matches_workspace":false}));
        let selected = select(&r, "loop");
        assert!(selected.report.is_none() && selected.score.is_none());
        assert!(selected.unavailable.is_some());
    }

    #[test]
    fn explicit_submission_wins_and_invalid_score_stays_unknown() {
        let mut r = record();
        r["moves"].as_array_mut().unwrap().push(json!({"kind":"lean.submitted", "selection_matches_workspace":true,"selected_session":2,"score":{"passed":4,"total":0}}));
        let selected = select(&r, "loop");
        assert_eq!(selected.report.as_deref(), Some("discarded report"));
        assert_eq!(selected.score, None);
    }

    #[test]
    fn unchanged_read_only_reviews_join_evidence_but_other_candidates_do_not() {
        let mut r = record();
        r["sessions"][1]["read_only"] = json!(true);
        r["sessions"][1]["changed_workspace"] = json!(false);
        r["moves"][0]["workspace_files"] = json!({"app.py":"same"});
        r["moves"][1]["workspace_files"] = json!({"app.py":"same"});
        let selected = select(&r, "loop");
        assert_eq!(selected.reviews, [2]);
        assert!(selected.report.unwrap().contains("Read-only review 2"));
        r["moves"][1]["workspace_files"] = json!({"app.py":"different"});
        assert!(select(&r, "loop").reviews.is_empty());
        r["moves"][1]["workspace_files"] = json!({"app.py":"same"});
        r["sessions"][1]["read_only"] = json!(false);
        assert!(select(&r, "loop").reviews.is_empty());
    }

    #[test]
    fn missing_finish_does_not_mean_success() {
        let mut r = record();
        r["sessions"][0]["finish"] = Value::Null;
        let selected = select(&r, "loop");
        assert!(selected.report.is_none());
        assert!(selected.unavailable.is_some());
    }
}
