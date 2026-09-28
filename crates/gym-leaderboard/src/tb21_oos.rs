//! The #9683 board: Microcoder on 65 held-out Terminal-Bench 2.1 tasks,
//! knowledge base off, against Fable 5 xhigh's cost per trial.
//!
//! Read from the committed round report `t1-report.json` (written by the
//! study's `report.py` from the 127 retained run records). The cost-win
//! rule is recomputed from each run's numbers and must agree with the
//! report's verdict; the totals must agree with the report's totals.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::contract::{
    Attempt, Bar, Benchmark, Board, BoardKind, Caveat, Cost, CostBasis, Label, Miss, Provenance,
    Reference, Spend, Split, Subject, Tally, TaskKnowledge, TaskRow, TaskStatus,
};
use crate::evidence::{
    Reader, Result, array, at, count, fail, number, opt_number, opt_string, opt_u64, string,
};
use crate::{median, pct, usd};

/// The board's identity.
pub const BOARD_ID: &str = "tb21-oos-microcoder-9683";

const STUDY: &str = "bench/terminal-bench/studies/2026-09-26-out-of-sample";
const RUNS: &str = "bench/terminal-bench/microcoder-runs/coderos-4080-tb21";
const REPORT: &str = "docs/terminal-bench/2026-09-26-tb21-oos-results.md";
const PRE_REGISTRATION: &str = "docs/terminal-bench/2026-09-26-tb21-oos-study.md";
const CONFIRMED: &str = "Confirmed out-of-sample win";
const NOT_CONFIRMED: &str = "Not confirmed";

/// Builds the board.
pub fn build(reader: &Reader) -> Result<Board> {
    let (report, report_file) = reader.json(&format!("{STUDY}/t1-report.json"))?;
    let mut evidence = vec![report_file];
    for rel in [REPORT, PRE_REGISTRATION] {
        evidence.push(reader.bytes(rel)?.1);
    }

    let reference = at(&report, "reference");
    let bar_of = |task: &str| -> Bar {
        let r = at(reference, task);
        Bar {
            cost_usd: opt_number(r, "usd_per_trial"),
            seconds: None,
            cost_trial: None,
            time_trial: None,
            deadline_seconds: None,
            reference_passes: opt_u64(r, "successes").map(|n| n as u32),
            reference_trials: opt_u64(r, "trials").map(|n| n as u32),
        }
    };

    // Runs in start order within each task: the first is the screen, the
    // rest are confirmations. The record name ends in its start time.
    let mut by_task: BTreeMap<String, Vec<&Value>> = BTreeMap::new();
    for run in array(&report, "runs")? {
        by_task.entry(string(run, "task")?).or_default().push(run);
    }
    let mut attempts = Vec::new();
    for (task, runs) in &mut by_task {
        runs.sort_by_key(|r| started(r));
        for (i, run) in runs.iter().enumerate() {
            attempts.push(attempt(run, task, i == 0, &bar_of(task))?);
        }
    }

    let mut tasks = Vec::new();
    let mut confirmed = 0_u32;
    for verdict in array(&report, "verdicts")? {
        let task = string(verdict, "task")?;
        let text = string(verdict, "verdict")?;
        let mine: Vec<&Attempt> = attempts.iter().filter(|a| a.task == task).collect();
        let passes = count(mine.iter().filter(|a| a.passed).count());
        let beats = count(mine.iter().filter(|a| a.beat).count());
        let recorded_wins = opt_u64(verdict, "cost_wins").map(|n| n as u32);
        if recorded_wins != Some(beats) {
            return Err(fail!(
                "{task}: counted {beats} cost wins, the report says {recorded_wins:?}"
            ));
        }
        let status = if text == CONFIRMED {
            confirmed += 1;
            TaskStatus::Confirmed
        } else if text == NOT_CONFIRMED {
            TaskStatus::NotConfirmed
        } else if passes > 0 {
            TaskStatus::PassedWithoutBeat
        } else {
            TaskStatus::NeverPassed
        };
        tasks.push(TaskRow {
            task: task.clone(),
            bar: bar_of(&task),
            knowledge: TaskKnowledge::Off,
            attempts: mine.iter().map(|a| a.id.clone()).collect(),
            passes,
            beats,
            status,
        });
    }

    let tally_of = |filter: &dyn Fn(&Attempt) -> bool| {
        let set: Vec<&Attempt> = attempts.iter().filter(|a| filter(a)).collect();
        Tally {
            attempts: count(set.len()),
            passes: count(set.iter().filter(|a| a.passed).count()),
            beats: count(set.iter().filter(|a| a.beat).count()),
            faults: 0,
            cost_unknown: count(set.iter().filter(|a| a.cost.known().is_none()).count()),
        }
    };
    let mut totals = tally_of(&|_| true);
    totals.faults = opt_u64(&report, "totals/faults").unwrap_or(0) as u32;
    for (key, got) in [
        ("graded", totals.attempts),
        ("passes", totals.passes),
        ("cost_wins", totals.beats),
        ("confirmed_wins", confirmed),
    ] {
        let want = opt_u64(&report, &format!("totals/{key}")).map(|n| n as u32);
        if want != Some(got) {
            return Err(fail!(
                "counted {key} {got}, the report's totals say {want:?}"
            ));
        }
    }
    let splits = vec![
        Split {
            name: "first runs".into(),
            tally: tally_of(&|a| a.series == "first run"),
        },
        Split {
            name: "confirmation runs".into(),
            tally: tally_of(&|a| a.series == "confirmation"),
        },
    ];

    let reported_usd: f64 = attempts.iter().filter_map(|a| a.cost.known()).sum();
    let upper: f64 = attempts
        .iter()
        .filter_map(|a| match a.cost {
            Cost::Unknown {
                upper_bound_usd, ..
            } => upper_bound_usd,
            Cost::Reported { .. } => None,
        })
        .sum();
    let spend = Spend {
        basis: CostBasis::ListPrice,
        reported_usd,
        estimated_lower_bound_usd: None,
        estimated_upper_bound_usd: (totals.cost_unknown > 0).then_some(reported_usd + upper),
    };

    let pass_ratios: Vec<f64> = attempts
        .iter()
        .filter(|a| a.passed)
        .filter_map(|a| a.cost_ratio)
        .collect();
    let pass_costs: Vec<f64> = attempts
        .iter()
        .filter(|a| a.passed)
        .filter_map(|a| a.cost.known())
        .collect();
    let (ref_passes, ref_trials) = tasks.iter().fold((0, 0), |(p, t), row| {
        (
            p + row.bar.reference_passes.unwrap_or(0),
            t + row.bar.reference_trials.unwrap_or(0),
        )
    });
    let first = &splits[0].tally;
    let headline = format!(
        "{confirmed} confirmed wins on {} held-out tasks; the median pass cost {} of Fable 5 xhigh's cost per trial ({}); first runs passed {} of {} tasks ({}), against Fable 5 xhigh's {} of trials.",
        tasks.len(),
        pct(median(&pass_ratios).unwrap_or(f64::NAN)),
        usd(median(&pass_costs).unwrap_or(f64::NAN)),
        first.passes,
        first.attempts,
        pct(f64::from(first.passes) / f64::from(first.attempts.max(1))),
        pct(f64::from(ref_passes) / f64::from(ref_trials.max(1))),
    );

    let model = model(reader, &attempts)?;
    let caveats = vec![
        Caveat {
            code: "reliability".into(),
            text: format!(
                "Reliability is the gap: first runs passed {} of {} tasks, and Fable 5 xhigh passed {ref_passes} of {ref_trials} trials on the same tasks. Microcoder is far cheaper where it passes; it isn't a drop-in replacement.",
                first.passes, first.attempts
            ),
        },
        Caveat {
            code: "older_benchmark".into(),
            text: "Terminal-Bench 2.1 is older and easier than Terminal-Bench 4. Microcoder hasn't yet passed a pre-registered held-out TB4 task.".into(),
        },
        Caveat {
            code: "reference_mean".into(),
            text: "The bar is Fable 5 xhigh's mean cost per trial from the TB2.1 leaderboard, not a per-run winning cost, and its source priced only part of the row.".into(),
        },
        Caveat {
            code: "no_reasoning".into(),
            text: "On the Codex login, GPT-6 Luna did no reasoning on any step of these runs.".into(),
        },
        Caveat {
            code: "time_basis".into(),
            text: "Seconds are Microcoder's loop time, not whole-trial time, and time isn't part of the win rule.".into(),
        },
        Caveat {
            code: "list_price".into(),
            text: "Every cost is list price on reported tokens on a subscription login, not a bill.".into(),
        },
    ];

    Ok(Board {
        id: BOARD_ID.into(),
        title: "Microcoder on 65 held-out TB2.1 tasks".into(),
        benchmark: Benchmark {
            name: "Terminal-Bench".into(),
            version: "2.1".into(),
        },
        kind: BoardKind::CostBelowReferencePerTrial,
        question: "With the knowledge base off, does Microcoder pass held-out TB2.1 tasks for less than Fable 5 xhigh's cost per trial, confirmed on at least 2 of 3 runs?".into(),
        headline,
        provenance: Provenance {
            issues: vec![9683, 9680],
            report: REPORT.into(),
            frozen_commit: None,
            evidence,
        },
        subject: Subject {
            agent: "Microcoder".into(),
            arm: "off".into(),
            model: model.0,
            effort: model.1,
            artifact: opt_string(&report, "meta")
                .and_then(|m| m.split_whitespace().skip_while(|w| *w != "commit").nth(1).map(|c| format!("microcoder {c}"))),
        },
        reference: Reference {
            name: "Fable 5 xhigh".into(),
            rule: "A win is a pass whose cost is below Fable 5 xhigh's mean cost per trial on the task. A task is a confirmed win when at least 2 of its 3 runs win.".into(),
            conditions: "The TB2.1 leaderboard's Fable 5 xhigh row, on its own hosts.".into(),
        },
        labels: {
            let mut l = vec![
                Label::PreRegistered,
                Label::OutOfSample,
                Label::KnowledgeOff,
                Label::ListPrice,
                Label::ReferenceOtherConditions,
            ];
            if totals.cost_unknown > 0 {
                l.push(Label::CostBound);
            }
            l.sort();
            l
        },
        caveats,
        totals,
        splits,
        spend,
        tasks,
        attempts,
    })
}

/// A run record's start, from the millisecond stamp that ends its name.
fn started(run: &Value) -> u64 {
    at(run, "record")
        .as_str()
        .and_then(|r| r.rsplit('-').next())
        .and_then(|s| s.parse().ok())
        .unwrap_or(u64::MAX)
}

fn attempt(run: &Value, task: &str, first: bool, bar: &Bar) -> Result<Attempt> {
    let record = string(run, "record")?;
    let reward = opt_number(run, "reward");
    let passed = string(run, "kind")? == "pass";
    let cost = match opt_number(run, "usd") {
        Some(usd) => Cost::Reported { usd },
        None => Cost::Unknown {
            lower_bound_usd: opt_number(run, "known_usd"),
            upper_bound_usd: opt_number(run, "usd_upper"),
        },
    };
    let bar_usd = number(run, "bar_usd")?;
    if bar.cost_usd.is_none_or(|b| (b - bar_usd).abs() > 1e-9) {
        return Err(fail!("{record}: its bar {bar_usd} isn't the reference's"));
    }
    let mut misses = Vec::new();
    if !passed {
        misses.push(Miss::Failed);
    }
    match cost.known() {
        None => misses.push(Miss::CostUnknown),
        Some(c) if c >= bar_usd => misses.push(Miss::Cost),
        Some(_) => {}
    }
    let beat = misses.is_empty();
    if at(run, "cost_win").as_bool() != Some(beat) {
        return Err(fail!(
            "{record}: recomputed cost win {beat} disagrees with the report"
        ));
    }
    let mut labels = vec![Label::KnowledgeOff, Label::OutOfSample];
    if cost.known().is_none() {
        labels.push(Label::CostBound);
    }
    labels.sort();
    Ok(Attempt {
        id: record.clone(),
        task: task.to_owned(),
        series: if first { "first run" } else { "confirmation" }.into(),
        trial: format!("{RUNS}/{record}"),
        reward,
        passed,
        seconds: opt_number(run, "seconds"),
        phases: None,
        cost,
        cost_ratio: opt_number(run, "cost_ratio"),
        cost_ratio_is_bound: at(run, "cost_ratio_is_bound").as_bool() == Some(true),
        time_ratio: None,
        beat,
        misses,
        labels,
        how_it_ended: opt_string(run, "ending"),
        jev: None,
        verifier: None,
        trace: None,
    })
}

/// The model and effort every run record names; they must agree.
fn model(reader: &Reader, attempts: &[Attempt]) -> Result<(String, Option<String>)> {
    let mut seen: Option<(String, Option<String>)> = None;
    for a in attempts {
        let rel = format!("{}/summary.json", a.trial);
        if !reader.exists(&rel) {
            return Err(fail!("{rel} is missing"));
        }
        let (summary, _) = reader.json(&rel)?;
        let here = (string(&summary, "model")?, opt_string(&summary, "effort"));
        match &seen {
            None => seen = Some(here),
            Some(prev) if *prev != here => {
                return Err(fail!("{rel}: model {here:?} differs from {prev:?}"));
            }
            Some(_) => {}
        }
    }
    seen.ok_or_else(|| fail!("no runs"))
}
