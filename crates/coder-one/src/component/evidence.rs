//! The evidence components' judgments: the setup gate, the probe keep
//! question, and the survey's relevance and edit questions.
//!
//! Each is a pair of pure functions around one Jev request: one builds the
//! request's state and questions from the component's input, and one turns
//! the answers into the component's decision. The episode's judge and the
//! standalone runner call the same functions, so a recorded answer from an
//! episode replays for a fixture built from it, and a changed question
//! misses.

use jev::{Noul, Questions};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::judge::clip;
use crate::record::Implementation;

/// A Noul at or above this reads as yes. An unmeasured development value.
pub const YES: f64 = 0.5;

/// The most characters of the issue body a Jev state carries.
pub const ISSUE_BODY_CHARS: usize = 8_000;

/// The issue as every evidence request's state carries it.
#[must_use]
pub fn issue_state(title: &str, body: &str) -> Value {
    json!({ "title": title, "body": clip(body, ISSUE_BODY_CHARS) })
}

/// The task text a fixture holds: the issue's title and body.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Task {
    pub title: String,
    pub body: String,
}

impl Task {
    /// The issue state for a Jev request.
    #[must_use]
    pub fn state(&self) -> Value {
        issue_state(&self.title, &self.body)
    }
}

// ---------------------------------------------------------------------------
// evidence.setup: the setup gate
// ---------------------------------------------------------------------------

/// The setup gate's question for command `i`.
#[must_use]
pub fn setup_question(i: usize) -> String {
    format!(
        "Does the task in `issue` require running the command `setup[{i}]` as written before the rest of the work can start?"
    )
}

/// The setup gate's parameters, digested into its implementation.
#[must_use]
pub fn setup_implementation() -> Implementation {
    Implementation::new(
        "evidence.setup",
        "jev-gated setup pack",
        &json!({
            "threshold": YES,
            "question": setup_question(0),
            "max_commands": 3,
            "deadline_sec": 240,
            "patterns": ["git clone", "pip install", "pip3 install", "python3 -m pip install"],
        }),
    )
}

/// The setup gate's request: the issue and the commands it names.
#[must_use]
pub fn setup_request(issue: &Value, commands: &[String]) -> (Value, Questions) {
    let mut questions = Questions::new();
    for i in 0..commands.len() {
        questions = questions.with(format!("setup_{i}"), Noul::new(setup_question(i)));
    }
    (json!({ "issue": issue, "setup": commands }), questions)
}

/// One command the gate judged.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Gated {
    pub command: String,
    /// Jev's probability that the task needs it run first; `None` when
    /// unknown.
    pub p: Option<f64>,
    pub approved: bool,
}

/// The gate's decision for each command. An unknown answer isn't approved.
#[must_use]
pub fn setup_decide(commands: &[String], noul: impl Fn(&str) -> Option<f64>) -> Vec<Gated> {
    commands
        .iter()
        .enumerate()
        .map(|(i, command)| {
            let p = noul(&format!("setup_{i}"));
            Gated {
                command: command.clone(),
                p,
                approved: p.is_some_and(|p| p >= YES),
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// evidence.probes.selector: the probe keep question
// ---------------------------------------------------------------------------

/// The most characters of one probe's output Jev reads.
pub const PROBE_JEV_CHARS: usize = 3_000;
/// The most probe outputs, and characters, a briefing carries.
pub const PROBE_KEEP: usize = 6;
pub const PROBE_TOTAL_CHARS: usize = 14_000;

/// The keep question for probe `i`.
#[must_use]
pub fn probe_question(i: usize) -> String {
    format!(
        "Does the output in `probes[{i}].output` contain information someone needs to complete the task in `issue`, such as where the relevant code or data is, what state it is in, or what went wrong?"
    )
}

/// The capture selector's parameters, digested into its implementation.
#[must_use]
pub fn probe_implementation() -> Implementation {
    Implementation::new(
        "evidence.probes.selector",
        "probe keep question",
        &json!({
            "threshold": YES,
            "keep": PROBE_KEEP,
            "total_chars": PROBE_TOTAL_CHARS,
            "jev_output_chars": PROBE_JEV_CHARS,
            "question": probe_question(0),
        }),
    )
}

/// One probe's command and output.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Probe {
    pub command: String,
    pub output: String,
}

/// The keep question's request: the issue and each output, clipped to what
/// Jev reads.
#[must_use]
pub fn probe_request(issue: &Value, probes: &[Probe]) -> (Value, Questions) {
    let mut questions = Questions::new();
    for i in 0..probes.len() {
        questions = questions.with(format!("probe_{i}"), Noul::new(probe_question(i)));
    }
    let state = json!({
        "issue": issue,
        "probes": probes.iter().map(|probe| json!({
            "command": probe.command,
            "output": clip(&probe.output, PROBE_JEV_CHARS),
        })).collect::<Vec<_>>(),
    });
    (state, questions)
}

/// What the selector did with one probe.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Selected {
    pub command: String,
    pub p: Option<f64>,
    pub chars: usize,
    /// `kept`, `below_threshold`, `over_keep`, `over_budget`, or `unknown`.
    pub decision: String,
}

/// The keep decision: of the probes at or above the threshold, the
/// [`PROBE_KEEP`] most probable are considered, and each is kept while the
/// total stays within [`PROBE_TOTAL_CHARS`] characters.
#[must_use]
pub fn probe_keep(probes: &[Probe], noul: impl Fn(&str) -> Option<f64>) -> Vec<Selected> {
    let mut judged: Vec<(usize, Option<f64>)> = (0..probes.len())
        .map(|i| (i, noul(&format!("probe_{i}"))))
        .collect();
    // Most probable first; a stable sort keeps ties in probe order.
    judged.sort_by(|a, b| b.1.unwrap_or(-1.0).total_cmp(&a.1.unwrap_or(-1.0)));
    let mut total = 0;
    let mut considered = 0;
    judged
        .into_iter()
        .map(|(i, p)| {
            let probe = &probes[i];
            let chars = probe.output.chars().count();
            let decision = match p {
                None => "unknown",
                Some(p) if p < YES => "below_threshold",
                Some(_) if considered >= PROBE_KEEP => "over_keep",
                Some(_) if total + chars > PROBE_TOTAL_CHARS => {
                    considered += 1;
                    "over_budget"
                }
                Some(_) => {
                    considered += 1;
                    total += chars;
                    "kept"
                }
            };
            Selected {
                command: probe.command.clone(),
                p,
                chars,
                decision: decision.to_string(),
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// evidence.select: the survey's relevance and edit questions
// ---------------------------------------------------------------------------

/// The most files one survey request judges.
pub const SURVEY_BATCH: usize = 20;
/// The most files the survey puts in the prompt.
pub const SURVEY_KEEP: usize = 6;
/// An edit probability at or above this marks a likely edit target.
pub const EDIT_TARGET: f64 = 0.8;

/// The relevance question for file `i`.
#[must_use]
pub fn relevance_question(i: usize) -> String {
    format!(
        "Would reading or editing the file `files[{i}]` help resolve the task described in `issue`?"
    )
}

/// The edit question for file `i`.
#[must_use]
pub fn edit_question(i: usize) -> String {
    format!(
        "Will resolving the task described in `issue` most likely require changing the file `files[{i}]`?"
    )
}

/// The evidence selector's parameters, digested into its implementation.
#[must_use]
pub fn select_implementation() -> Implementation {
    Implementation::new(
        "evidence.select",
        "survey relevance and edit",
        &json!({
            "batch": SURVEY_BATCH,
            "keep": SURVEY_KEEP,
            "threshold": YES,
            "edit_target": EDIT_TARGET,
            "rank": "relevance + 0.1 × edit",
            "relevance_question": relevance_question(0),
            "edit_question": edit_question(0),
        }),
    )
}

/// One candidate file: its path and the excerpt code found.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Candidate {
    pub path: String,
    pub excerpt: String,
}

/// One survey request's state and questions, for a batch of candidates.
#[must_use]
pub fn survey_request(issue: &Value, batch: &[Candidate]) -> (Value, Questions) {
    let mut questions = Questions::new();
    for i in 0..batch.len() {
        questions = questions
            .with(format!("rel_{i}"), Noul::new(relevance_question(i)))
            .with(format!("edit_{i}"), Noul::new(edit_question(i)));
    }
    let state = json!({
        "issue": issue,
        "files": batch.iter().map(|c| json!({ "path": c.path, "excerpt": c.excerpt })).collect::<Vec<_>>(),
    });
    (state, questions)
}

/// One candidate with its scores.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Scored {
    pub path: String,
    pub relevance: Option<f64>,
    pub edit: Option<f64>,
    /// Whether it is among the files the survey reads into the prompt.
    pub selected: bool,
}

/// Ranks scored candidates by relevance, letting a likely edit break ties,
/// and selects at most [`SURVEY_KEEP`] at or above the threshold. An
/// unknown score ranks as zero, as the episode has always read it.
#[must_use]
pub fn survey_rank(scored: Vec<(String, Option<f64>, Option<f64>)>) -> Vec<Scored> {
    let mut scored = scored;
    let rank = |rel: Option<f64>, edit: Option<f64>| rel.unwrap_or(0.0) + 0.1 * edit.unwrap_or(0.0);
    scored.sort_by(|a, b| rank(b.1, b.2).total_cmp(&rank(a.1, a.2)));
    let mut kept = 0;
    scored
        .into_iter()
        .map(|(path, relevance, edit)| {
            let selected = kept < SURVEY_KEEP && relevance.is_some_and(|p| p >= YES);
            if selected {
                kept += 1;
            }
            Scored {
                path,
                relevance,
                edit,
                selected,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// verify.close: the closing check
// ---------------------------------------------------------------------------

/// The closing check's `done` question.
pub const CLOSE_QUESTION: &str = "Do the delegate's report in `delegate.report` and the changes in `delegate.changes` show that the task described in `issue` is complete?";

/// The closing check's question for requirement `j`.
#[must_use]
pub fn close_criterion_question(j: usize) -> String {
    format!(
        "Do `delegate.report` and `delegate.changes` show that the requirement `criteria[{j}]` from the issue is satisfied?"
    )
}

/// The closing check's parameters, digested into its implementation.
#[must_use]
pub fn close_implementation() -> Implementation {
    Implementation::new(
        "verify.close",
        "broad done noul",
        &json!({
            "question": CLOSE_QUESTION,
            "criterion_question": close_criterion_question(0),
            "report_chars": 3_000,
            "changes_chars": 5_000,
            "acts_on_answer": false,
        }),
    )
}

/// The closing check's request.
#[must_use]
pub fn close_request(
    issue: &Value,
    criteria: &[String],
    report: &str,
    changes: &str,
) -> (Value, Questions) {
    let state = json!({
        "issue": issue,
        "criteria": criteria,
        "delegate": {
            "report": crate::judge::clip_tail(report, 3_000),
            "changes": clip(changes, 5_000),
        },
    });
    let mut questions = Questions::new().with("done", Noul::new(CLOSE_QUESTION));
    for j in 0..criteria.len() {
        questions = questions.with(
            format!("criterion_{j}"),
            Noul::new(close_criterion_question(j)),
        );
    }
    (state, questions)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn probes() -> Vec<Probe> {
        (0..8)
            .map(|i| Probe {
                command: format!("probe {i}"),
                output: "x".repeat(3_000),
            })
            .collect()
    }

    #[test]
    fn the_keep_decision_holds_to_the_threshold_count_and_budget() {
        let probes = probes();
        let decided = probe_keep(&probes, |id| match id {
            "probe_0" => Some(0.2),
            "probe_7" => None,
            _ => Some(0.9),
        });
        let kept = decided.iter().filter(|s| s.decision == "kept").count();
        // Six pass the threshold, but 14,000 characters hold four outputs
        // of 3,000.
        assert_eq!(kept, 4);
        assert!(decided.iter().any(|s| s.decision == "over_budget"));
        assert!(
            decided
                .iter()
                .any(|s| s.command == "probe 0" && s.decision == "below_threshold")
        );
        assert!(
            decided
                .iter()
                .any(|s| s.command == "probe 7" && s.decision == "unknown")
        );
    }

    #[test]
    fn clipping_for_jev_is_idempotent_so_fixtures_replay() {
        let issue = issue_state("t", "b");
        let long = Probe {
            command: "cat big".to_string(),
            output: "y".repeat(5_000),
        };
        let (first, _) = probe_request(&issue, std::slice::from_ref(&long));
        let clipped = Probe {
            command: long.command.clone(),
            output: first["probes"][0]["output"].as_str().unwrap().to_string(),
        };
        let (second, _) = probe_request(&issue, &[clipped]);
        assert_eq!(first, second);
    }

    #[test]
    fn the_survey_ranks_by_relevance_and_breaks_ties_by_edit() {
        let ranked = survey_rank(vec![
            ("a.py".to_string(), Some(0.7), Some(0.1)),
            ("b.py".to_string(), Some(0.7), Some(0.9)),
            ("c.py".to_string(), Some(0.3), Some(0.9)),
            ("d.py".to_string(), None, None),
        ]);
        assert_eq!(ranked[0].path, "b.py");
        assert!(ranked[0].selected && ranked[1].selected);
        assert!(!ranked[2].selected && !ranked[3].selected);
    }
}
