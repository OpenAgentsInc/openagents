//! The #9776 board: Coder One's Jev-briefed Fable 5.1 low delegate on 14
//! Terminal-Bench 4 tasks, two passes, against Fable 5.1 low's cheapest
//! and fastest wins.
//!
//! Read from the committed `attempts.json` (written by that experiment's
//! `summarize.py`) and `tasks.json` (written by `bars.py` and frozen in
//! e0414c3356 before any run). The beat rule is recomputed here from the
//! row's own numbers and must agree with the row's recorded verdict, and
//! the per-pass tallies must agree with the file's own tallies; any
//! disagreement is an error, not a silent choice between them.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::contract::{
    Attempt, Bar, Benchmark, Board, BoardKind, Caveat, Cost, CostBasis, JevSummary, Label, Miss,
    Phases, Provenance, Reference, Spend, Split, Subject, Tally, TaskKnowledge, TaskRow,
    TaskStatus, TraceRef, VerifierSummary,
};
use crate::evidence::{
    Reader, Result, array, at, close, count, fail, number, opt_number, opt_string, opt_u64, string,
};
use crate::{THIN_MARGIN, pct};

/// The board's identity.
pub const BOARD_ID: &str = "tb4-fable-delegate-repro-9776";

const EXPERIMENT: &str = "bench/terminal-bench/experiments/2026-09-27-fable-delegate-repro";
const REPORT: &str = "docs/terminal-bench/2026-09-27-fable-delegate-repro.md";
const TRACES: &str = "bench/terminal-bench/traces";
/// The commit that froze the bars, deadlines, note, and candidates.
const FROZEN: &str = "e0414c335603f7e2aa755074abae9b7c686335a4";

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
    let (attempts_doc, attempts_file) = reader.json(&format!("{EXPERIMENT}/attempts.json"))?;
    let (tasks_doc, tasks_file) = reader.json(&format!("{EXPERIMENT}/tasks.json"))?;
    let mut evidence = vec![attempts_file, tasks_file];

    let mut bars: BTreeMap<String, (u32, Bar)> = BTreeMap::new();
    for task in array(&tasks_doc, "tasks")? {
        let name = string(task, "task")?;
        let order = opt_u64(task, "order").unwrap_or(0) as u32;
        let bar = Bar {
            cost_usd: Some(number(task, "bar_cost_usd")?),
            seconds: Some(number(task, "bar_seconds")?),
            cost_trial: opt_string(task, "cheapest_win/id"),
            time_trial: opt_string(task, "fastest_win/id"),
            deadline_seconds: opt_u64(task, "delegate_deadline_sec").map(|d| d as u32),
            reference_passes: opt_u64(task, "fable_low_passes").map(|n| n as u32),
            reference_trials: opt_u64(task, "fable_low_attempts").map(|n| n as u32),
        };
        bars.insert(name, (order, bar));
    }

    let rows = array(&attempts_doc, "attempts")?;
    let mut attempts = Vec::new();
    let mut jobs = Vec::new();
    let mut faults_by_pass: BTreeMap<u64, u32> = BTreeMap::new();
    let mut own_by_task: BTreeMap<String, u32> = BTreeMap::new();
    let mut subject: Option<Subject> = None;
    for row in rows {
        let task = string(row, "task")?;
        let pass = opt_u64(row, "pass").ok_or_else(|| fail!("{task}: no pass"))?;
        if string(row, "kind")? == "fault" {
            *faults_by_pass.entry(pass).or_default() += 1;
            continue;
        }
        let (_, bar) = bars
            .get(&task)
            .ok_or_else(|| fail!("{task}: no bar in tasks.json"))?;
        let bar = bar.clone();
        check_bar(row, &task, &bar)?;
        let own_entries: BTreeSet<String> = array(row, "candidates")?
            .iter()
            .filter(|c| at(c, "written_from_this_task").as_bool() == Some(true))
            .filter_map(|c| opt_string(c, "id"))
            .collect();
        own_by_task.insert(task.clone(), count(own_entries.len()));
        let attempt = attempt(row, &task, pass, &bar)?;
        let job = string(row, "job")?;
        let trial = string(row, "trial")?;
        let episode = format!("{TRACES}/{job}/{trial}.episode");
        if subject.is_none() {
            subject = Some(self::subject(reader, row, &job, &trial)?);
        }
        jobs.push(TraceJob {
            attempt: attempt.clone(),
            bar,
            episode,
            own_entries,
        });
        attempts.push(attempt);
    }
    let subject = subject.ok_or_else(|| fail!("no attempts"))?;

    // Tasks in the issue's order.
    let mut order: Vec<(&String, &(u32, Bar))> = bars.iter().collect();
    order.sort_by_key(|(_, (o, _))| *o);
    let tasks: Vec<TaskRow> = order
        .into_iter()
        .map(|(task, (_, bar))| {
            let mine: Vec<&Attempt> = attempts.iter().filter(|a| &a.task == task).collect();
            let passes = count(mine.iter().filter(|a| a.passed).count());
            let beats = count(mine.iter().filter(|a| a.beat).count());
            let own = own_by_task.get(task).copied().unwrap_or(0);
            TaskRow {
                task: task.clone(),
                bar: bar.clone(),
                knowledge: if own > 0 {
                    TaskKnowledge::Own { candidates: own }
                } else {
                    TaskKnowledge::OtherTasksOnly
                },
                attempts: mine.iter().map(|a| a.id.clone()).collect(),
                passes,
                beats,
                status: if beats > 0 {
                    TaskStatus::Beat
                } else if passes > 0 {
                    TaskStatus::PassedWithoutBeat
                } else {
                    TaskStatus::NeverPassed
                },
            }
        })
        .collect();

    let own_tasks: Vec<&str> = tasks
        .iter()
        .filter(|t| matches!(t.knowledge, TaskKnowledge::Own { .. }))
        .map(|t| t.task.as_str())
        .collect();
    let tally_of = |filter: &dyn Fn(&Attempt) -> bool, faults: u32| {
        let set: Vec<&Attempt> = attempts.iter().filter(|a| filter(a)).collect();
        Tally {
            attempts: count(set.len()),
            passes: count(set.iter().filter(|a| a.passed).count()),
            beats: count(set.iter().filter(|a| a.beat).count()),
            faults,
            cost_unknown: count(set.iter().filter(|a| a.cost.known().is_none()).count()),
        }
    };
    let total_faults: u32 = faults_by_pass.values().sum();
    let totals = tally_of(&|_| true, total_faults);
    let mut splits = Vec::new();
    for pass in [1_u64, 2] {
        let series = format!("pass {pass}");
        let tally = tally_of(
            &|a| a.series == series,
            faults_by_pass.get(&pass).copied().unwrap_or(0),
        );
        check_tallies(&attempts_doc, pass, &tally, &attempts, &series)?;
        splits.push(Split {
            name: series.clone(),
            tally,
        });
    }
    splits.push(Split {
        name: "tasks with their own knowledge".into(),
        tally: tally_of(&|a| own_tasks.contains(&a.task.as_str()), 0),
    });
    splits.push(Split {
        name: "tasks without their own knowledge".into(),
        tally: tally_of(&|a| !own_tasks.contains(&a.task.as_str()), 0),
    });

    let reported_usd: f64 = attempts.iter().filter_map(|a| a.cost.known()).sum();
    let lower: f64 = attempts
        .iter()
        .filter_map(|a| match a.cost {
            Cost::Unknown {
                lower_bound_usd, ..
            } => lower_bound_usd,
            Cost::Reported { .. } => None,
        })
        .sum();
    let spend = Spend {
        basis: CostBasis::ListPrice,
        reported_usd,
        estimated_lower_bound_usd: (totals.cost_unknown > 0).then_some(lower),
        estimated_upper_bound_usd: None,
    };

    let caveats = caveats(&attempts, &tasks, &splits, &totals);
    let headline = format!(
        "Beat Fable 5.1 low's cheapest and fastest win on {} of {} attempts ({}: {} of {}, {}: {} of {}); passed {} of {}.",
        totals.beats,
        totals.attempts,
        splits[0].name,
        splits[0].tally.beats,
        splits[0].tally.attempts,
        splits[1].name,
        splits[1].tally.beats,
        splits[1].tally.attempts,
        totals.passes,
        totals.attempts,
    );
    let mut labels = vec![
        Label::PreRegistered,
        Label::KnowledgeAssisted,
        Label::ListPrice,
        Label::FewAttempts,
        Label::ReferenceOtherConditions,
    ];
    if totals.cost_unknown > 0 {
        labels.push(Label::CostBound);
    }
    if attempts.iter().any(|a| a.labels.contains(&Label::InSample)) {
        labels.push(Label::InSample);
    }
    labels.sort();

    let (_, report_file) = reader.bytes(REPORT)?;
    evidence.push(report_file);

    let board = Board {
        id: BOARD_ID.into(),
        title: "Jev-briefed Fable delegate, 14 more tasks".into(),
        benchmark: Benchmark {
            name: "Terminal-Bench".into(),
            version: "4.0".into(),
        },
        kind: BoardKind::BeatCheapestAndFastestWin,
        question: "Does s7a1's frozen arm, with nothing changed, beat Fable 5.1 low's cheapest and fastest wins on the other TB4 tasks it could be tried on?".into(),
        headline,
        provenance: Provenance {
            issues: vec![9776, 9746, 9680],
            report: REPORT.into(),
            frozen_commit: Some(FROZEN.into()),
            evidence,
        },
        subject,
        reference: Reference {
            name: "Fable 5.1 low".into(),
            rule: "A beat is reward 1, known total cost below Fable 5.1 low's cheapest win on the task, and whole-trial time below its fastest win. Unknown cost can't beat.".into(),
            conditions: "Public Claude Code runs on another host, with open network and each task's full timeout.".into(),
        },
        labels,
        caveats,
        totals,
        splits,
        spend,
        tasks,
        attempts,
        reference_rows: Vec::new(),
        snapshot: None,
    };
    Ok((board, jobs))
}

/// The bar a row carries must be the frozen one.
fn check_bar(row: &Value, task: &str, bar: &Bar) -> Result<()> {
    let row_cost = number(row, "bar/cost_usd")?;
    let row_seconds = number(row, "bar/seconds")?;
    if !close(row_cost, bar.cost_usd.unwrap_or(f64::NAN))
        || !close(row_seconds, bar.seconds.unwrap_or(f64::NAN))
    {
        return Err(fail!(
            "{task}: attempts.json's bar ({row_cost}, {row_seconds}) differs from tasks.json"
        ));
    }
    Ok(())
}

fn attempt(row: &Value, task: &str, pass: u64, bar: &Bar) -> Result<Attempt> {
    let label = string(row, "attempt")?;
    let reward = opt_number(row, "reward");
    let passed = reward.is_some_and(|r| r >= 1.0);
    let seconds = opt_number(row, "trial_seconds");
    let answered = at(row, "delegate/total_cost_usd").is_number();
    let cost = if answered {
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
    let cost_bar = bar.cost_usd.ok_or_else(|| fail!("{task}: no cost bar"))?;
    let time_bar = bar.seconds.ok_or_else(|| fail!("{task}: no time bar"))?;
    let mut misses = Vec::new();
    if !passed {
        misses.push(Miss::Failed);
    }
    match cost.known() {
        None => misses.push(Miss::CostUnknown),
        Some(c) if c >= cost_bar => misses.push(Miss::Cost),
        Some(_) => {}
    }
    if seconds.is_none_or(|s| s >= time_bar) {
        misses.push(Miss::Time);
    }
    let beat = misses.is_empty();
    let recorded = at(row, "beat_the_bar").as_bool();
    if recorded != Some(beat) {
        return Err(fail!(
            "{task} {label}: recomputed beat {beat} but attempts.json says {recorded:?}"
        ));
    }
    let (cost_ratio, cost_ratio_is_bound) = match cost {
        Cost::Reported { usd } => (Some(usd / cost_bar), false),
        Cost::Unknown {
            lower_bound_usd, ..
        } => (lower_bound_usd.map(|l| l / cost_bar), true),
    };
    let time_ratio = seconds.map(|s| s / time_bar);

    let candidates = array(row, "candidates")?;
    let kept: Vec<&Value> = candidates
        .iter()
        .filter(|c| at(c, "fate").as_str() == Some("kept"))
        .collect();
    let kept_own = kept
        .iter()
        .filter(|c| at(c, "written_from_this_task").as_bool() == Some(true))
        .count();
    let requirements = array(row, "jev/requirements")?;
    let flagged = requirements
        .iter()
        .filter(|r| at(r, "flagged").as_bool() == Some(true))
        .count();
    let jev = JevSummary {
        question_set: string(row, "jev/question_set")?,
        outcome: string(row, "jev/outcome")?,
        milliseconds: opt_u64(row, "jev/milliseconds"),
        candidates: count(candidates.len()),
        kept: count(kept.len()),
        kept_own: count(kept_own),
        requirements: count(requirements.len()),
        flagged: count(flagged),
    };

    let mut labels = Vec::new();
    if !kept.is_empty() {
        labels.push(Label::KnowledgeAssisted);
        if kept_own > 0 {
            labels.push(Label::InSample);
        }
    }
    if cost.known().is_none() {
        labels.push(Label::CostBound);
    }
    if beat {
        let cost_margin = 1.0 - cost_ratio.unwrap_or(1.0);
        let time_margin = 1.0 - time_ratio.unwrap_or(1.0);
        if cost_margin < THIN_MARGIN || time_margin < THIN_MARGIN {
            labels.push(Label::ThinMargin);
        }
    }
    labels.sort();

    let deadline = opt_u64(row, "delegate/deadline_sec");
    let delegate_seconds = opt_number(row, "agent_sec/delegate");
    let how_it_ended = match (string(row, "delegate/status")?.as_str(), deadline) {
        ("answered", _) => Some(format!(
            "The delegate answered after {:.1} s.",
            delegate_seconds.unwrap_or(f64::NAN)
        )),
        ("timed_out", Some(d)) => Some(format!(
            "The delegate ran to its {d} s deadline; its cost is unknown."
        )),
        (other, _) => Some(format!("The delegate ended as {other}.")),
    };

    Ok(Attempt {
        id: format!("{task}.{label}"),
        task: task.to_owned(),
        series: format!("pass {pass}"),
        trial: string(row, "trial_id")?,
        reward,
        passed,
        seconds,
        phases: Some(Phases {
            environment_setup: opt_number(row, "phases_sec/environment_setup"),
            agent_setup: opt_number(row, "phases_sec/agent_setup"),
            agent_execution: opt_number(row, "phases_sec/agent_execution"),
            verifier: opt_number(row, "phases_sec/verifier"),
        }),
        cost,
        cost_ratio,
        cost_ratio_is_bound,
        time_ratio,
        beat,
        misses,
        labels,
        how_it_ended,
        jev: Some(jev),
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
        trace: Some(TraceRef {
            path: crate::trace_path(BOARD_ID, &format!("{task}.{label}")),
            sha256: String::new(),
            bytes: 0,
        }),
        cost_basis: None,
        caveats: Vec::new(),
    })
}

/// The file's own per-pass tallies must match what this crate counts.
fn check_tallies(
    doc: &Value,
    pass: u64,
    tally: &Tally,
    attempts: &[Attempt],
    series: &str,
) -> Result<()> {
    let recorded = at(doc, &format!("tallies/pass{pass}"));
    let want = |key: &str| opt_u64(recorded, key).map(|n| n as u32);
    for (key, got) in [
        ("results", tally.attempts),
        ("passes", tally.passes),
        ("beats", tally.beats),
        ("faults", tally.faults),
        ("unknown_cost_attempts", tally.cost_unknown),
    ] {
        if want(key) != Some(got) {
            return Err(fail!(
                "pass {pass}: counted {key} {got}, attempts.json tallies say {:?}",
                want(key)
            ));
        }
    }
    let known: f64 = attempts
        .iter()
        .filter(|a| a.series == series)
        .filter_map(|a| match a.cost {
            Cost::Reported { .. } => a.cost.known(),
            Cost::Unknown { .. } => None,
        })
        .sum();
    if let Some(recorded_known) = opt_number(recorded, "known_cost_usd") {
        // The file rounds to four places and counts the delegate only.
        if (known - recorded_known).abs() > 0.01 {
            return Err(fail!(
                "pass {pass}: known cost {known} differs from the tallies' {recorded_known}"
            ));
        }
    }
    Ok(())
}

fn subject(reader: &Reader, row: &Value, job: &str, trial: &str) -> Result<Subject> {
    // `tb4--<arm>--<task>--<suffix>`.
    let arm = job
        .split("--")
        .nth(1)
        .ok_or_else(|| fail!("{job}: no arm in the job name"))?
        .to_owned();
    let (trajectory, _) = reader.json(&format!(
        "{TRACES}/{job}/{trial}.episode/trajectory.atif.json"
    ))?;
    Ok(Subject {
        agent: "Coder One".into(),
        arm,
        model: string(row, "delegate/model")?,
        effort: opt_string(row, "delegate/effort"),
        artifact: opt_string(&trajectory, "agent/version"),
    })
}

fn caveats(
    attempts: &[Attempt],
    tasks: &[TaskRow],
    splits: &[Split],
    totals: &Tally,
) -> Vec<Caveat> {
    let mut out = Vec::new();
    let beats: Vec<&Attempt> = attempts.iter().filter(|a| a.beat).collect();
    let in_sample = beats
        .iter()
        .filter(|a| a.labels.contains(&Label::InSample))
        .count();
    let own_less: Vec<&TaskRow> = tasks
        .iter()
        .filter(|t| !matches!(t.knowledge, TaskKnowledge::Own { .. }))
        .collect();
    let own_less_beats: u32 = own_less.iter().map(|t| t.beats).sum();
    out.push(Caveat {
        code: "in_sample".into(),
        text: format!(
            "{in_sample} of {} beats kept knowledge written from earlier runs on the same task. The {} tasks without their own knowledge beat the bar {} times. The board says nothing about tasks Coder hasn't seen.",
            beats.len(),
            own_less.len(),
            own_less_beats
        ),
    });
    let thin: Vec<String> = beats
        .iter()
        .filter(|a| a.labels.contains(&Label::ThinMargin))
        .map(|a| format!("{} by {}", a.task, pct(1.0 - a.cost_ratio.unwrap_or(1.0))))
        .collect();
    if !thin.is_empty() {
        out.push(Caveat {
            code: "thin_margin".into(),
            text: format!(
                "{} of {} beats came in under a bar by less than {}: {}. Claude Code's list-price figure and the reference's cost field can differ by more than that.",
                thin.len(),
                beats.len(),
                pct(THIN_MARGIN),
                thin.join(", ")
            ),
        });
    }
    out.push(Caveat {
        code: "cost_unknown".into(),
        text: format!(
            "{} of {} delegates ran to their deadline, so their cost is unknown and they count as misses. Their lower-bound estimates price the stream's usage at list price and leave out the call in flight.",
            totals.cost_unknown, totals.attempts
        ),
    });
    let per_pass: Vec<String> = splits
        .iter()
        .filter(|s| s.name.starts_with("pass "))
        .map(|s| format!("{} of {} in {}", s.tally.beats, s.tally.attempts, s.name))
        .collect();
    out.push(Caveat {
        code: "few_attempts".into(),
        text: format!(
            "Two attempts per task can't estimate a per-task beat rate, and the passes disagree: {}.",
            per_pass.join(", ")
        ),
    });
    out.push(Caveat {
        code: "list_price".into(),
        text: "Every cost is a list-price figure on a subscription login, like the reference's, not a bill.".into(),
    });
    out.push(Caveat {
        code: "reference_conditions".into(),
        text: "The reference ran Claude Code 2.1.273 on another host with open network and each task's full timeout. These attempts ran Claude Code 2.1.280 inside Coder One, with only PyPI added to the allowlist and a deadline the delegate wasn't told.".into(),
    });
    out
}
