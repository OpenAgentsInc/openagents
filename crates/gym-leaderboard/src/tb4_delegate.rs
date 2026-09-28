//! The #9776 board: Coder One's Jev-briefed Fable 5.1 low delegate on 14
//! Terminal-Bench 4 tasks, two passes, against Fable 5.1 low's cheapest
//! and fastest wins.
//!
//! The board is a study descriptor ([`DESCRIPTOR`]) built by the generic
//! adapter in [`crate::study`]. This module keeps only what is particular
//! to the experiment's files: reading its `attempts.json` (written by its
//! `summarize.py` before the shared row schema existed) as
//! `openagents.gym.attempt-row.v1` rows, with the file's own per-pass
//! tallies, and reading its `tasks.json` (written by `bars.py` and frozen
//! in e0414c3356 before any run) as the bars file. The study adapter then
//! recomputes every beat against the row's recorded verdict, checks each
//! row's bar against the frozen one, and checks the counts against the
//! tallies; any disagreement is an error, not a silent choice.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::contract::{
    Attempt, Bar, Board, Cost, CostBasis, EvidenceFile, JevSummary, Phases, VerifierSummary,
};
use crate::evidence::{
    Reader, Result, array, at, count, fail, number, opt_number, opt_string, opt_u64, string,
};
use crate::study::{
    AttemptRow, FileRef, ROW_SCHEMA, RecordedTally, RowKnowledge, Rows, SubjectFields,
};

/// The board's identity.
pub const BOARD_ID: &str = "tb4-fable-delegate-repro-9776";

/// The study descriptor.
pub const DESCRIPTOR: &str =
    "bench/terminal-bench/experiments/2026-09-27-fable-delegate-repro/study.json";

/// The row shape of this experiment's `attempts.json`.
pub const ROWS_SHAPE: &str = "openagents.tb.fable_delegate_repro.attempts";

/// The bars shape of its `tasks.json`.
pub const BARS_SHAPE: &str = "openagents.tb.fable_delegate_repro.tasks.v1";

const TRACES: &str = "bench/terminal-bench/traces";

/// An attempt whose trace should be bundled.
#[derive(Clone, Debug)]
pub struct TraceJob {
    pub attempt: Attempt,
    pub bar: Bar,
    /// The retained episode directory, repository-relative.
    pub episode: String,
    /// Knowledge entry IDs written from this attempt's task.
    pub own_entries: BTreeSet<String>,
}

/// Builds the board and lists the traces to bundle.
pub fn build(reader: &Reader) -> Result<(Board, Vec<TraceJob>)> {
    crate::study::build(reader, DESCRIPTOR)
}

/// The experiment's `attempts.json` as shared rows, with its tallies.
pub fn rows(reader: &Reader, rel: &str) -> Result<(Rows, EvidenceFile)> {
    let (doc, file) = reader.json(rel)?;
    let mut rows = Vec::new();
    let mut subject: Option<SubjectFields> = None;
    for row in array(&doc, "attempts")? {
        let task = string(row, "task")?;
        let pass = opt_u64(row, "pass").ok_or_else(|| fail!("{task}: no pass"))?;
        let series = format!("pass {pass}");
        if string(row, "kind")? == "fault" {
            rows.push(AttemptRow {
                id: format!("{task}.fault.p{pass}"),
                task,
                series,
                trial: opt_string(row, "trial_id").unwrap_or_default(),
                fault: true,
                reward: None,
                seconds: None,
                phases: None,
                cost: None,
                cost_basis: None,
                bar: None,
                verdict: None,
                knowledge: RowKnowledge::default(),
                jev: None,
                how_it_ended: None,
                verifier: None,
                episode: None,
                subject: None,
            });
            continue;
        }
        let label = string(row, "attempt")?;
        let job = string(row, "job")?;
        let trial = string(row, "trial")?;
        let episode = format!("{TRACES}/{job}/{trial}.episode");
        if subject.is_none() {
            subject = Some(subject_of(reader, row, &job, &episode)?);
        }
        let candidates = array(row, "candidates")?;
        let ids = |keep: &dyn Fn(&Value) -> bool| -> Vec<String> {
            candidates
                .iter()
                .filter(|c| keep(c))
                .filter_map(|c| opt_string(c, "id"))
                .collect()
        };
        let kept = ids(&|c| at(c, "fate").as_str() == Some("kept"));
        let own = ids(&|c| at(c, "written_from_this_task").as_bool() == Some(true));
        let own_set: BTreeSet<&String> = own.iter().collect();
        let requirements = array(row, "jev/requirements")?;
        let jev = JevSummary {
            question_set: string(row, "jev/question_set")?,
            outcome: string(row, "jev/outcome")?,
            milliseconds: opt_u64(row, "jev/milliseconds"),
            candidates: count(candidates.len()),
            kept: count(kept.len()),
            kept_own: count(kept.iter().filter(|k| own_set.contains(k)).count()),
            requirements: count(requirements.len()),
            flagged: count(
                requirements
                    .iter()
                    .filter(|r| at(r, "flagged").as_bool() == Some(true))
                    .count(),
            ),
        };
        let cost = if at(row, "delegate/total_cost_usd").is_number() {
            Cost::Reported {
                usd: number(row, "cost/total_with_search_usd")?,
            }
        } else {
            let estimate = opt_number(row, "delegate/estimate_usd/total_lower_bound");
            let known = opt_number(row, "cost/known_lower_bound_usd").unwrap_or(0.0)
                + opt_number(row, "cost/knowledge_search_usd").unwrap_or(0.0);
            Cost::Unknown {
                lower_bound_usd: estimate.map(|e| e + known),
                upper_bound_usd: None,
            }
        };
        let deadline = opt_u64(row, "delegate/deadline_sec");
        let delegate_seconds = opt_number(row, "agent_sec/delegate");
        let how_it_ended = match (string(row, "delegate/status")?.as_str(), deadline) {
            ("answered", _) => format!(
                "The delegate answered after {:.1} s.",
                delegate_seconds.unwrap_or(f64::NAN)
            ),
            ("timed_out", Some(d)) => {
                format!("The delegate ran to its {d} s deadline; its cost is unknown.")
            }
            (other, _) => format!("The delegate ended as {other}."),
        };
        rows.push(AttemptRow {
            id: format!("{task}.{label}"),
            task,
            series,
            trial: string(row, "trial_id")?,
            fault: false,
            reward: opt_number(row, "reward"),
            seconds: opt_number(row, "trial_seconds"),
            phases: Some(Phases {
                environment_setup: opt_number(row, "phases_sec/environment_setup"),
                agent_setup: opt_number(row, "phases_sec/agent_setup"),
                agent_execution: opt_number(row, "phases_sec/agent_execution"),
                verifier: opt_number(row, "phases_sec/verifier"),
            }),
            cost: Some(cost),
            cost_basis: Some(CostBasis::ListPrice),
            bar: Some(Bar {
                cost_usd: Some(number(row, "bar/cost_usd")?),
                seconds: Some(number(row, "bar/seconds")?),
                cost_trial: None,
                time_trial: None,
                deadline_seconds: None,
                reference_passes: None,
                reference_trials: None,
            }),
            verdict: Some(
                at(row, "beat_the_bar")
                    .as_bool()
                    .ok_or_else(|| fail!("{rel}: {label} records no verdict"))?,
            ),
            knowledge: RowKnowledge { kept, own },
            jev: Some(jev),
            how_it_ended: Some(how_it_ended),
            verifier: Some(VerifierSummary {
                summary: opt_string(row, "verifier/summary"),
                failed_tests: array(row, "verifier/failed")
                    .map(|f| {
                        f.iter()
                            .filter_map(|t| t.as_str().map(str::to_owned))
                            .collect()
                    })
                    .unwrap_or_default(),
            }),
            episode: Some(episode),
            subject: None,
        });
    }
    if let (Some(first), Some(s)) = (rows.iter_mut().find(|r| !r.fault), subject) {
        first.subject = Some(s);
    }
    let mut tallies = BTreeMap::new();
    if let Some(recorded) = at(&doc, "tallies").as_object() {
        for (key, t) in recorded {
            let Some(pass) = key.strip_prefix("pass") else {
                continue;
            };
            let want = |k: &str| {
                opt_u64(t, k)
                    .map(|n| n as u32)
                    .ok_or_else(|| fail!("{rel}: tallies/{key} has no {k}"))
            };
            tallies.insert(
                format!("pass {pass}"),
                RecordedTally {
                    attempts: want("results")?,
                    passes: want("passes")?,
                    beats: want("beats")?,
                    faults: want("faults")?,
                    cost_unknown: want("unknown_cost_attempts")?,
                    known_cost_usd: opt_number(t, "known_cost_usd"),
                },
            );
        }
    }
    Ok((
        Rows {
            schema: ROW_SCHEMA.into(),
            rows,
            tallies,
        },
        file,
    ))
}

/// The frozen `tasks.json` as bars, in the issue's task order.
pub fn bars(reader: &Reader, file: &FileRef) -> Result<(Vec<(String, Bar)>, EvidenceFile)> {
    let (doc, entry) = reader.json(&file.path)?;
    let shape = file
        .shape
        .clone()
        .or_else(|| opt_string(&doc, "schema"))
        .unwrap_or_default();
    if shape != BARS_SHAPE {
        return Err(fail!("{}: unknown bars shape {shape:?}", file.path));
    }
    let mut bars: Vec<(u32, String, Bar)> = Vec::new();
    for task in array(&doc, "tasks")? {
        bars.push((
            opt_u64(task, "order").unwrap_or(0) as u32,
            string(task, "task")?,
            Bar {
                cost_usd: Some(number(task, "bar_cost_usd")?),
                seconds: Some(number(task, "bar_seconds")?),
                cost_trial: opt_string(task, "cheapest_win/id"),
                time_trial: opt_string(task, "fastest_win/id"),
                deadline_seconds: opt_u64(task, "delegate_deadline_sec").map(|d| d as u32),
                reference_passes: opt_u64(task, "fable_low_passes").map(|n| n as u32),
                reference_trials: opt_u64(task, "fable_low_attempts").map(|n| n as u32),
            },
        ));
    }
    bars.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    Ok((
        bars.into_iter().map(|(_, task, bar)| (task, bar)).collect(),
        entry,
    ))
}

fn subject_of(reader: &Reader, row: &Value, job: &str, episode: &str) -> Result<SubjectFields> {
    // `tb4--<arm>--<task>--<suffix>`.
    let arm = job
        .split("--")
        .nth(1)
        .ok_or_else(|| fail!("{job}: no arm in the job name"))?
        .to_owned();
    let (trajectory, _) = reader.json(&format!("{episode}/trajectory.atif.json"))?;
    Ok(SubjectFields {
        agent: None,
        arm: Some(arm),
        model: Some(string(row, "delegate/model")?),
        effort: opt_string(row, "delegate/effort"),
        artifact: opt_string(&trajectory, "agent/version"),
    })
}
