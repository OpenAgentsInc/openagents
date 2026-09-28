//! A retained Microcoder run record as a scrubbed, bounded trace bundle.
//!
//! Microcoder writes `summary.json` and `events.jsonl` per run; retained
//! copies live under `bench/terminal-bench/microcoder-runs/<host>/`, with a
//! `MANIFEST.json` naming each file's SHA-256. That manifest stands in for
//! an episode's `retention.json`: a record file whose digest differs from
//! the manifest's, or that the manifest doesn't list, refuses to bundle.
//!
//! The events are read with `gym`'s own reader
//! ([`gym::runs_microcoder::parse_events`]), not a second parser, and each
//! becomes a [`TraceStep`] on the run's own clock (`seconds` from its
//! start):
//!
//! | Event | Step |
//! | --- | --- |
//! | `started` | `delegate_started` (agent `Microcoder`) |
//! | `generated` | `model_step`, then its rationale as `say` and its notes as `host` |
//! | `ran` | `command`, then `command_result` |
//! | `judged`, `disputed`, `covered`, `conformed`, `assessed`, `reviewed` | `decision`, with every answer's probability |
//! | `retrieved` | `retrieval` |
//! | `tested` | `tests` |
//! | `ended` | `ended` |
//! | `verified` | the bundle's `verifier` |
//!
//! The verifier's output (`summary.json`'s `verifier_output`, often apt
//! logs before the test report) goes through the tail bound, and its test
//! names come from pytest's short summary lines.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::bundle::{INSTRUCTION_BOUND, VERIFIER_TAIL, fit, keep_ends, last_part};
use crate::contract::{
    Answer, Attempt, Bar, EvidenceFile, ScrubReport, StepKind, TRACE_BUNDLE_SCHEMA, TestResult,
    Text, TraceBundle, TraceOutcome, TraceStep, VerifierDetail,
};
use crate::evidence::{Reader, Result, at, fail, opt_number, opt_string, opt_u64};
use crate::scrub::Scrubber;

/// The retained manifest's file name and schema, as `gym` reads them.
pub use gym::runs_microcoder::{MANIFEST, MANIFEST_SCHEMA};

/// One retained host directory's manifest: each run's recorded digests.
#[derive(Clone, Debug)]
pub struct Manifest {
    /// Repository-relative host directory.
    pub dir: String,
    pub file: EvidenceFile,
    /// Per run directory: file name to lower-case hex SHA-256.
    pub digests: BTreeMap<String, BTreeMap<String, String>>,
}

impl Manifest {
    /// Reads `<dir>/MANIFEST.json`.
    pub fn read(reader: &Reader, dir: &str) -> Result<Self> {
        let rel = format!("{dir}/{MANIFEST}");
        let (value, file) = reader.json(&rel)?;
        if at(&value, "schema").as_str() != Some(MANIFEST_SCHEMA) {
            return Err(fail!("{rel}: not a {MANIFEST_SCHEMA} manifest"));
        }
        let mut digests = BTreeMap::new();
        for run in at(&value, "runs").as_array().into_iter().flatten() {
            let Some(name) = opt_string(run, "name") else {
                continue;
            };
            let files = at(run, "files")
                .as_object()
                .map(|files| {
                    files
                        .iter()
                        .filter_map(|(file, digest)| {
                            let hex = digest.as_str()?.strip_prefix("sha256:")?;
                            Some((file.clone(), hex.to_owned()))
                        })
                        .collect()
                })
                .unwrap_or_default();
            digests.insert(name, files);
        }
        Ok(Self {
            dir: dir.to_owned(),
            file,
            digests,
        })
    }

    /// A run's file, read and checked against the manifest's digest.
    pub fn read_checked(
        &self,
        reader: &Reader,
        run: &str,
        name: &str,
    ) -> Result<(Vec<u8>, EvidenceFile)> {
        let rel = format!("{}/{run}/{name}", self.dir);
        let recorded = self
            .digests
            .get(run)
            .and_then(|files| files.get(name))
            .ok_or_else(|| fail!("{rel}: {MANIFEST} doesn't record its digest"))?;
        let (bytes, entry) = reader.bytes(&rel)?;
        if entry.sha256 != *recorded {
            return Err(fail!("{rel}: its digest differs from {MANIFEST}"));
        }
        Ok((bytes, entry))
    }
}

/// What a Microcoder bundle is built from.
pub struct Input<'a> {
    pub board: &'a str,
    pub attempt: &'a Attempt,
    pub bar: &'a Bar,
    pub manifest: &'a Manifest,
    /// The run directory's name.
    pub run: &'a str,
}

/// The run's retained files, checked against the manifest.
struct Files {
    sources: Vec<EvidenceFile>,
    summary: Value,
    events: Vec<Value>,
}

/// Builds the bundle, shrinking field bounds until it fits.
pub fn build(reader: &Reader, input: &Input<'_>) -> Result<TraceBundle> {
    let (summary_bytes, summary_file) =
        input
            .manifest
            .read_checked(reader, input.run, "summary.json")?;
    let (events_bytes, events_file) =
        input
            .manifest
            .read_checked(reader, input.run, "events.jsonl")?;
    let summary: Value =
        serde_json::from_slice(&summary_bytes).map_err(|e| fail!("{}: {e}", summary_file.path))?;
    let (events, _) = gym::runs_microcoder::parse_events(&String::from_utf8_lossy(&events_bytes));
    let files = Files {
        sources: vec![input.manifest.file.clone(), summary_file, events_file],
        summary,
        events,
    };
    fit(&input.attempt.id, |bound, keep| {
        assemble(&files, input, bound, keep)
    })
}

/// What Jev was asked, by the event that records its answers.
fn question(event: &str) -> Option<&'static str> {
    Some(match event {
        "judged" => "Is it done, making progress, repeating?",
        "disputed" => "Is a failing frozen test itself wrong?",
        "covered" => "Do the passing tests leave the task uncovered?",
        "conformed" => "Does the finished code contradict a relevant entry?",
        "assessed" => "Is the task hard enough for the stronger model?",
        "reviewed" => "Do the acceptance tests check what the task asks?",
        _ => return None,
    })
}

fn assemble(
    files: &Files,
    input: &Input<'_>,
    bound: usize,
    keep: Option<usize>,
) -> Result<TraceBundle> {
    let mut scrub = Scrubber::new(bound);
    let mut steps = Vec::new();
    let mut reward_event = None;
    for event in &files.events {
        let name = at(event, "event").as_str().unwrap_or_default();
        // `started` opens the run's clock and records no time of its own.
        let at_ms = at(event, "seconds")
            .as_f64()
            .filter(|s| s.is_finite() && *s >= 0.0)
            .map(|s| (s * 1000.0).round() as u64)
            .or((name == "started").then_some(0));
        let step_no = opt_u64(event, "step");
        let mut push = |kind: StepKind| steps.push(TraceStep { at_ms, kind });
        if at(event, "judgment").is_object() {
            let judgment = at(event, "judgment");
            let answers = at(judgment, "answers")
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|pair| {
                    Some(Answer {
                        name: pair.get(0)?.as_str()?.to_owned(),
                        p: pair.get(1)?.as_f64()?,
                    })
                })
                .collect();
            let mut detail = Vec::new();
            for key in ["dropped", "flagged"] {
                let list: Vec<String> = at(event, key)
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|v| v.as_str().map_or_else(|| v.to_string(), str::to_owned))
                    .collect();
                if !list.is_empty() {
                    detail.push(scrub.text(&format!("{key}: {}", list.join(", "))));
                }
            }
            if let Some(error) = opt_string(judgment, "error") {
                detail.push(scrub.text(&format!("error: {error}")));
            }
            push(StepKind::Decision {
                name: name.to_owned(),
                duration_ms: opt_u64(judgment, "milliseconds"),
                question: question(name).map(str::to_owned),
                answers,
                detail,
                cost_usd: opt_number(judgment, "usd"),
            });
            continue;
        }
        match name {
            "started" => push(StepKind::DelegateStarted {
                agent: "Microcoder".into(),
                model: opt_string(event, "model"),
            }),
            "generated" => {
                let generated = at(event, "generated");
                push(StepKind::ModelStep {
                    step: step_no,
                    model: opt_string(generated, "model"),
                    milliseconds: opt_u64(generated, "milliseconds"),
                    cost_usd: opt_number(generated, "usd"),
                    prompt_tokens: opt_u64(generated, "prompt_tokens"),
                    completion_tokens: opt_u64(generated, "completion_tokens"),
                });
                let action = at(generated, "action");
                if let Some(error) = opt_string(action, "Err") {
                    push(StepKind::Host {
                        text: scrub.text(&format!("The reply didn't parse: {error}")),
                    });
                    continue;
                }
                let ok = at(action, "Ok");
                if let Some(rationale) = opt_string(ok, "rationale") {
                    push(StepKind::Say {
                        text: scrub.text(&rationale),
                    });
                }
                if at(ok, "freeze_tests").as_bool() == Some(true) {
                    push(StepKind::Host {
                        text: plain("It froze its acceptance tests."),
                    });
                }
                if at(ok, "finished").as_bool() == Some(true) {
                    push(StepKind::Host {
                        text: plain("It said the task is finished."),
                    });
                }
            }
            "ran" => {
                let result = at(event, "result");
                push(StepKind::Command {
                    command: scrub.text(&opt_string(result, "command").unwrap_or_default()),
                });
                let mut output = opt_string(result, "output").unwrap_or_default();
                if at(result, "timed_out").as_bool() == Some(true) {
                    output.push_str("\n[the command timed out]");
                }
                push(StepKind::CommandResult {
                    exit_code: at(result, "exit").as_i64(),
                    output: scrub.text(&output),
                });
            }
            "retrieved" => {
                let retrieval = at(event, "retrieval");
                let kept = at(retrieval, "kept")
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|k| {
                        Some(Answer {
                            name: k.get("id")?.as_str()?.to_owned(),
                            p: k.get("relevance")?.as_f64()?,
                        })
                    })
                    .collect::<Vec<_>>();
                let expanded = at(retrieval, "expanded")
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|pair| pair.get(0)?.as_str().map(str::to_owned))
                    .collect::<Vec<_>>();
                if !kept.is_empty() || !expanded.is_empty() {
                    push(StepKind::Retrieval {
                        step: step_no,
                        kept,
                        expanded,
                    });
                }
            }
            "tested" => {
                let results: Vec<&Value> = at(event, "results")
                    .as_array()
                    .into_iter()
                    .flatten()
                    .collect();
                let failed: Vec<Text> = results
                    .iter()
                    .filter(|r| at(r, "exit").as_i64() != Some(0))
                    .map(|r| {
                        let output = opt_string(r, "output").unwrap_or_default();
                        let last = output
                            .lines()
                            .rev()
                            .find(|l| !l.trim().is_empty())
                            .unwrap_or("no output")
                            .trim()
                            .to_owned();
                        scrub.text(&format!(
                            "{}: {last}",
                            opt_string(r, "command").unwrap_or_else(|| "?".into())
                        ))
                    })
                    .collect();
                push(StepKind::Tests {
                    froze: at(event, "froze").as_bool() == Some(true),
                    passed: crate::evidence::count(results.len() - failed.len()),
                    total: crate::evidence::count(results.len()),
                    failed,
                });
            }
            "ended" => push(StepKind::Ended {
                reason: opt_string(event, "outcome/ending/reason")
                    .unwrap_or_else(|| "unknown".into()),
                steps: opt_u64(event, "outcome/steps"),
            }),
            "verified" => reward_event = Some(event),
            _ => {}
        }
    }
    let dropped = keep_ends(&mut steps, keep);

    let verifier_output = opt_string(&files.summary, "verifier_output");
    let verifier = (verifier_output.is_some() || reward_event.is_some()).then(|| {
        let output = verifier_output.as_deref().unwrap_or("");
        let tests = pytest_tests(output);
        let summary = gym::runs::pytest_summary(output);
        VerifierDetail {
            reward: opt_number(&files.summary, "reward")
                .or_else(|| reward_event.and_then(|e| opt_number(e, "reward"))),
            passed: summary.map_or_else(
                || crate::evidence::count(tests.iter().filter(|t| t.status == "passed").count()),
                |s| u32::try_from(s.passed).unwrap_or(u32::MAX),
            ),
            failed: summary.map_or_else(
                || crate::evidence::count(tests.iter().filter(|t| t.status == "failed").count()),
                |s| u32::try_from(s.failed).unwrap_or(u32::MAX),
            ),
            tests,
            output_tail: scrub.text_within(last_part(output, VERIFIER_TAIL * 4), VERIFIER_TAIL),
        }
    });

    let attempt = input.attempt;
    Ok(TraceBundle {
        schema: TRACE_BUNDLE_SCHEMA.into(),
        board: input.board.into(),
        attempt: attempt.id.clone(),
        task: attempt.task.clone(),
        sources: files.sources.clone(),
        // Microcoder's record doesn't carry the task's instruction: the
        // loop reads it inside the task's container.
        instruction: scrub.text_within("", INSTRUCTION_BOUND),
        jev: None,
        briefing: None,
        steps,
        verifier,
        outcome: TraceOutcome {
            passed: attempt.passed,
            beat: attempt.beat,
            seconds: attempt.seconds,
            cost: attempt.cost,
            bar: input.bar.clone(),
            how_it_ended: attempt.how_it_ended.clone(),
        },
        scrub: ScrubReport {
            redactions: scrub.redactions,
            truncated_fields: scrub.truncated,
            dropped_steps: u32::try_from(dropped).unwrap_or(u32::MAX),
            field_bound: u32::try_from(bound).unwrap_or(u32::MAX),
        },
    })
}

fn plain(text: &str) -> Text {
    Text {
        text: text.to_owned(),
        original_bytes: None,
    }
}

/// Test names and outcomes from pytest's `PASSED path::name` and
/// `FAILED path::name - reason` summary lines, in order, once each.
#[must_use]
pub fn pytest_tests(output: &str) -> Vec<TestResult> {
    let mut seen = std::collections::BTreeSet::new();
    let mut tests = Vec::new();
    for line in output.lines() {
        let line = line.trim();
        let (status, rest) = if let Some(rest) = line.strip_prefix("PASSED ") {
            ("passed", rest)
        } else if let Some(rest) = line.strip_prefix("FAILED ") {
            ("failed", rest)
        } else if let Some(rest) = line.strip_prefix("ERROR ") {
            ("failed", rest)
        } else {
            continue;
        };
        let id = rest.split(" - ").next().unwrap_or(rest).trim();
        let name = id.rsplit("::").next().unwrap_or(id).to_owned();
        if name.is_empty() || !seen.insert(name.clone()) {
            continue;
        }
        tests.push(TestResult {
            name,
            status: status.to_owned(),
        });
    }
    tests
}

#[cfg(test)]
mod tests {
    use super::pytest_tests;

    #[test]
    fn pytest_summary_lines_become_tests() {
        let out = "noise\nPASSED ../tests/test_outputs.py::test_a\nFAILED ../tests/test_outputs.py::test_b - AssertionError: x\nPASSED ../tests/test_outputs.py::test_a\n";
        let tests = pytest_tests(out);
        assert_eq!(tests.len(), 2);
        assert_eq!(
            (tests[0].name.as_str(), tests[0].status.as_str()),
            ("test_a", "passed")
        );
        assert_eq!(
            (tests[1].name.as_str(), tests[1].status.as_str()),
            ("test_b", "failed")
        );
    }
}
