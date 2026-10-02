//! Running graders over a finished run.
//!
//! Structural graders (`regex`, `operation_used`, `operation_order`,
//! `file_exists`) read the run's trajectory, final message, and created
//! files, and cost nothing. `decision` and `judge` graders call a door three
//! times; `receipt` graders replay Wasm invocation receipts. A grader that
//! can't evaluate reports why and fails; nothing here passes by default.

use std::collections::BTreeSet;
use std::path::Path;

use serde::Serialize;
use serde_json::{Value, json};

use crate::case::{Case, MAX_FILE_BYTES};
use crate::door::{Doors, ReplayVerdict, RunKey};
use crate::grader::{Check, Focus, Grader, Match};
use crate::record::{Arm, RunOutcome, RunRecord};

/// Calls a door grader makes at most.
pub const DOOR_VOTES: usize = 3;
/// Votes a `decision` or `judge` grader needs.
pub const VOTES_NEEDED: usize = 2;

/// The `judge` grader's system prompt.
pub const JUDGE_SYSTEM: &str = "You are a strict, terse evaluation judge for agent runs.";

/// One door call's answer, as a report records it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Vote {
    /// What the door said: a probability, `PASS`, `FAIL`, or the text.
    pub answer: String,
    /// Whether the answer counts toward a pass.
    pub counted: bool,
}

/// One grader's verdict on one run.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct GraderResult {
    /// The grader's name.
    pub name: String,
    /// Its type word.
    #[serde(rename = "type")]
    pub kind: &'static str,
    /// Its weight.
    pub weight: f64,
    /// Whether it counts toward the run's score. A subject-only grader
    /// still reports, with `scored = false`.
    pub scored: bool,
    /// Whether it passed.
    pub passed: bool,
    /// Why, in one line.
    pub explanation: String,
    /// The door's answers, for `decision` and `judge` graders.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub votes: Vec<Vote>,
}

/// Grades one run with every grader that applies in its arm.
///
/// In the baseline arm, subject-only graders do not run. In the subject
/// arm they run and report with `scored = false`, unless every grader of
/// the case is subject-only, in which case they score the subject arm alone.
/// A cancelled or unknown run is not graded; a run that ended with an error
/// fails every grader with that reason and calls no door.
#[must_use]
pub fn grade_run(
    case: &Case,
    record: &RunRecord,
    extension_operations: &BTreeSet<String>,
    doors: Doors<'_>,
) -> Vec<GraderResult> {
    if !record.outcome.scored() {
        return Vec::new();
    }
    let all_subject_only = case.subject_only(extension_operations);
    let mut results = Vec::new();
    for grader in &case.graders {
        let subject_only = grader.subject_only(extension_operations);
        if record.arm == Arm::Baseline && subject_only {
            continue;
        }
        let scored = !subject_only || all_subject_only;
        let (passed, explanation, votes) = match record.outcome {
            RunOutcome::Errored(failure) => (
                false,
                format!("the run ended with {}", failure.word()),
                Vec::new(),
            ),
            _ => check(case, grader, record, doors),
        };
        results.push(GraderResult {
            name: grader.name.clone(),
            kind: grader.check.type_word(),
            weight: grader.weight,
            scored,
            passed,
            explanation,
            votes,
        });
    }
    results
}

type Checked = (bool, String, Vec<Vote>);

fn fail(explanation: impl Into<String>) -> Checked {
    (false, explanation.into(), Vec::new())
}

fn check(case: &Case, grader: &Grader, record: &RunRecord, doors: Doors<'_>) -> Checked {
    match &grader.check {
        Check::Regex {
            regex,
            matching,
            target,
            ..
        } => {
            let text = match focus_text(target, record) {
                Ok(text) => text,
                Err(reason) => return fail(reason),
            };
            let count = regex.find_iter(&text).count();
            let (passed, wanted) = match matching {
                Match::Contains => (count > 0, "at least one match".to_string()),
                Match::NotContains => (count == 0, "no match".to_string()),
                Match::Count(n) => (count == *n, format!("exactly {n}")),
            };
            (
                passed,
                format!("{count} matches in {}; wanted {wanted}", target.describe()),
                Vec::new(),
            )
        }
        Check::OperationUsed {
            operation,
            input_regex,
            min,
            max,
            ..
        } => {
            let Some(trajectory) = &record.trajectory else {
                return fail("the run wrote no trajectory");
            };
            let count = trajectory
                .calls()
                .iter()
                .filter(|call| call.is(operation))
                .filter(|call| {
                    input_regex
                        .as_ref()
                        .is_none_or(|re| re.is_match(&call.input()))
                })
                .count();
            let within = count >= *min && max.is_none_or(|max| count <= max);
            let range = match max {
                Some(max) if max == min => format!("exactly {min}"),
                Some(max) => format!("{min} to {max}"),
                None => format!("at least {min}"),
            };
            (
                within,
                format!("{operation} ran {count} times; wanted {range}"),
                Vec::new(),
            )
        }
        Check::OperationOrder { before, after } => {
            let Some(trajectory) = &record.trajectory else {
                return fail("the run wrote no trajectory");
            };
            let calls = trajectory.calls();
            let first = |operation: &str| calls.iter().position(|call| call.is(operation));
            match (first(before), first(after)) {
                (Some(b), Some(a)) if b < a => (
                    true,
                    format!(
                        "{before} ran first (call {}), then {after} (call {})",
                        b + 1,
                        a + 1
                    ),
                    Vec::new(),
                ),
                (Some(b), Some(a)) => fail(format!(
                    "{after} ran first (call {}), before {before} (call {})",
                    a + 1,
                    b + 1
                )),
                (None, _) => fail(format!("{before} never ran")),
                (_, None) => fail(format!("{after} never ran")),
            }
        }
        Check::FileExists { path, exists } => {
            let matched: Vec<&String> = record
                .created_files
                .iter()
                .filter(|file| crate::glob::matches(path, file))
                .collect();
            match (exists, matched.first()) {
                (true, Some(file)) => (true, format!("created {file}"), Vec::new()),
                (true, None) => fail(format!("no created file matches {path}")),
                (false, Some(file)) => fail(format!("created {file}, which matches {path}")),
                (false, None) => (true, format!("no created file matches {path}"), Vec::new()),
            }
        }
        Check::Decision {
            question,
            threshold,
            focus,
            rubric,
        } => {
            let Some(door) = doors.decision else {
                return fail("no decision door is configured");
            };
            let content = match focus_value(focus, record) {
                Ok(content) => content,
                Err(reason) => return fail(reason),
            };
            let state = json!({
                "task": case.prompt,
                "focus": focus.describe(),
                "run": content,
            });
            let mut votes = Vec::new();
            let (mut yes, mut no) = (0, 0);
            for call in 1..=DOOR_VOTES {
                let answer = match door.ask(&state, question, rubric.as_deref()) {
                    Ok(answer) => answer,
                    Err(reason) => {
                        return (
                            false,
                            format!("the decision door failed on call {call}: {reason}"),
                            votes,
                        );
                    }
                };
                let probability = match answer.probability(question) {
                    Ok(probability) => probability,
                    Err(reason) => {
                        return (
                            false,
                            format!("the decision door failed on call {call}: {reason}"),
                            votes,
                        );
                    }
                };
                let counted = probability >= *threshold;
                if counted {
                    yes += 1;
                } else {
                    no += 1;
                }
                votes.push(Vote {
                    answer: format!("{probability:.3}"),
                    counted,
                });
                if yes >= VOTES_NEEDED || no > DOOR_VOTES - VOTES_NEEDED {
                    break;
                }
            }
            (
                yes >= VOTES_NEEDED,
                format!(
                    "{yes} of {} answers at or above {threshold}; {VOTES_NEEDED} needed",
                    votes.len()
                ),
                votes,
            )
        }
        Check::Judge { criteria, focus } => {
            let Some(door) = doors.judge else {
                return fail("no judge door is configured");
            };
            let record_text = match focus_value(focus, record) {
                Ok(Value::String(text)) => text,
                Ok(other) => serde_json::to_string_pretty(&other).unwrap_or_default(),
                Err(reason) => return fail(reason),
            };
            let user = judge_prompt(&record_text, criteria);
            let mut votes = Vec::new();
            let (mut pass, mut failed) = (0, 0);
            for call in 1..=DOOR_VOTES {
                let text = match door.complete(JUDGE_SYSTEM, &user) {
                    Ok(text) => text,
                    Err(reason) => {
                        return (
                            false,
                            format!("the judge door failed on call {call}: {reason}"),
                            votes,
                        );
                    }
                };
                let answer = judge_answer(&text);
                match answer {
                    Some(true) => pass += 1,
                    Some(false) => failed += 1,
                    None => {}
                }
                votes.push(Vote {
                    answer: match answer {
                        Some(true) => "PASS".into(),
                        Some(false) => "FAIL".into(),
                        None => bounded(&text, 80),
                    },
                    counted: answer == Some(true),
                });
                let remaining = DOOR_VOTES - call;
                if failed > 0 || pass + remaining < VOTES_NEEDED {
                    break;
                }
            }
            (
                pass >= VOTES_NEEDED && failed == 0,
                format!(
                    "{pass} PASS and {failed} FAIL of {} answers; {VOTES_NEEDED} PASS and no FAIL \
                     needed",
                    votes.len()
                ),
                votes,
            )
        }
        Check::Receipt { operation } => {
            let Some(replayer) = doors.replayer else {
                return fail("no receipt replayer is configured");
            };
            let key = RunKey {
                case: &record.case,
                arm: record.arm,
                attempt: record.attempt,
            };
            let verdicts = match replayer.replay(key, operation) {
                Ok(verdicts) => verdicts,
                Err(reason) => return fail(format!("the receipts could not be read: {reason}")),
            };
            if verdicts.is_empty() {
                return fail(format!("the run recorded no receipt for {operation}"));
            }
            for (index, verdict) in verdicts.iter().enumerate() {
                match verdict {
                    ReplayVerdict::Passed => {}
                    ReplayVerdict::Failed {
                        field,
                        expected,
                        actual,
                    } => {
                        return fail(format!(
                            "receipt {} of {operation} replayed differently: {field} was \
                             {expected}, the replay gave {actual}",
                            index + 1
                        ));
                    }
                    ReplayVerdict::Unverifiable { reason } => {
                        return fail(format!(
                            "receipt {} of {operation} could not be replayed: {reason}",
                            index + 1
                        ));
                    }
                }
            }
            (
                true,
                format!(
                    "{} receipts of {operation} replayed exactly",
                    verdicts.len()
                ),
                Vec::new(),
            )
        }
        Check::Command {
            command, exit_code, ..
        } => {
            let Some(ran) = record.commands.get(&grader.name) else {
                return fail(format!("`{command}` did not run"));
            };
            let tail = |output: &str| {
                let output = output.trim();
                if output.is_empty() {
                    String::new()
                } else {
                    let last: Vec<&str> = output.lines().rev().take(3).collect();
                    let last: Vec<&str> = last.into_iter().rev().collect();
                    format!(": {}", bounded(&last.join(" | "), 300))
                }
            };
            match ran.exit_code {
                _ if ran.timed_out => fail(format!(
                    "`{command}` ran past its deadline and was stopped{}",
                    tail(&ran.output)
                )),
                Some(code) if code == *exit_code => (
                    true,
                    format!("`{command}` exited {code}; wanted {exit_code}"),
                    Vec::new(),
                ),
                Some(code) => fail(format!(
                    "`{command}` exited {code}; wanted {exit_code}{}",
                    tail(&ran.output)
                )),
                None => fail(format!(
                    "`{command}` ended by a signal{}",
                    tail(&ran.output)
                )),
            }
        }
    }
}

/// The `judge` grader's user prompt.
#[must_use]
pub fn judge_prompt(record: &str, criteria: &str) -> String {
    format!(
        "## Run record\n{record}\n\n## Criterion\n{criteria}\n\nAnswer only with \"PASS\" or \
         \"FAIL\"."
    )
}

/// Reads a judge's answer: `PASS` or `FAIL`, alone, in any case, with an
/// optional trailing period or quotes. Anything else is neither. This is a
/// bounded enum the prompt asked for, not a reading of prose.
#[must_use]
pub fn judge_answer(text: &str) -> Option<bool> {
    let word = text
        .trim()
        .trim_matches(|c: char| c == '"' || c == '\'' || c == '*' || c == '`')
        .trim_end_matches('.')
        .trim();
    if word.eq_ignore_ascii_case("PASS") {
        Some(true)
    } else if word.eq_ignore_ascii_case("FAIL") {
        Some(false)
    } else {
        None
    }
}

fn bounded(text: &str, limit: usize) -> String {
    let text = text.trim();
    match text.char_indices().nth(limit) {
        Some((index, _)) => format!("{}…", &text[..index]),
        None => text.to_string(),
    }
}

/// The focus as text, for a `regex` grader.
fn focus_text(focus: &Focus, record: &RunRecord) -> Result<String, String> {
    match focus_value(focus, record)? {
        Value::String(text) => Ok(text),
        _ => record
            .trajectory
            .as_ref()
            .map(|trajectory| serde_json::to_string(&trajectory.document).unwrap_or_default())
            .ok_or_else(|| "the run wrote no trajectory".to_string()),
    }
}

/// The focus as a door sees it.
fn focus_value(focus: &Focus, record: &RunRecord) -> Result<Value, String> {
    match focus {
        Focus::LastMessage => {
            let trajectory = record
                .trajectory
                .as_ref()
                .ok_or("the run wrote no trajectory")?;
            Ok(Value::String(
                trajectory.final_message().unwrap_or_default().to_string(),
            ))
        }
        Focus::Trajectory => record
            .trajectory
            .as_ref()
            .map(crate::trajectory::Trajectory::door_view)
            .ok_or_else(|| "the run wrote no trajectory".to_string()),
        Focus::Files => Ok(Value::String(record.created_files.join("\n"))),
        Focus::Changed => Ok(Value::String(
            record
                .changes
                .iter()
                .map(crate::workspace::Change::line)
                .collect::<Vec<_>>()
                .join("\n"),
        )),
        Focus::Diff => record
            .diff
            .clone()
            .map(Value::String)
            .ok_or_else(|| "the run kept no diff; only a files test has one".to_string()),
        Focus::File(path) => {
            let workspace = record
                .workspace
                .as_deref()
                .ok_or("the run kept no workspace to read")?;
            read_workspace_file(workspace, path).map(Value::String)
        }
    }
}

/// Reads one text file from a run's workspace: inside it, at most 1 MiB.
///
/// # Errors
///
/// Returns why the file can't be read.
pub fn read_workspace_file(workspace: &Path, path: &str) -> Result<String, String> {
    crate::grader::workspace_path(path).map_err(|detail| format!("the path {detail}"))?;
    let root = workspace
        .canonicalize()
        .map_err(|error| format!("the workspace can't be read: {error}"))?;
    let full = root.join(path);
    let resolved = full
        .canonicalize()
        .map_err(|_| format!("{path} does not exist in the workspace"))?;
    if !resolved.starts_with(&root) {
        return Err(format!("{path} resolves outside the workspace"));
    }
    let metadata = std::fs::metadata(&resolved).map_err(|error| format!("{path}: {error}"))?;
    if !metadata.is_file() {
        return Err(format!("{path} is not a file"));
    }
    if metadata.len() > MAX_FILE_BYTES {
        return Err(format!(
            "{path} is {} bytes; a file focus reads at most 1 MiB",
            metadata.len()
        ));
    }
    let bytes = std::fs::read(&resolved).map_err(|error| format!("{path}: {error}"))?;
    String::from_utf8(bytes).map_err(|_| format!("{path} is not UTF-8 text"))
}

#[cfg(test)]
mod tests {
    use super::judge_answer;

    #[test]
    fn a_judge_answer_is_the_bounded_word_or_nothing() {
        assert_eq!(judge_answer("PASS"), Some(true));
        assert_eq!(judge_answer(" pass.\n"), Some(true));
        assert_eq!(judge_answer("\"FAIL\""), Some(false));
        assert_eq!(judge_answer("**FAIL**"), Some(false));
        assert_eq!(judge_answer("It passes"), None);
        assert_eq!(judge_answer("PASS, mostly"), None);
        assert_eq!(judge_answer(""), None);
    }
}
