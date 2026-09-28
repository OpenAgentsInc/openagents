//! The #9746 board: the Fable delegate's development series, 1 to 7, on
//! `sound-change-cascade`, `fin-saccr-rwa`, and `gsea-proteomics`, against
//! Fable 5.1 low's cheapest and fastest wins.
//!
//! Read from the experiment's three committed row files, written by its
//! `summarize.py` in three shapes as the arm changed: `attempts.json`
//! (series 1, no verdict), `attempts-series2-5.json`, and
//! `attempts-series6-7.json` (each row with `beat_the_bar`). The bars are
//! the ones the report used: Fable 5.1 low's cheapest and fastest winning
//! runs in `reference/fable-5.1-replays.json`, computed with `gym`'s
//! `reference_task` (the same rule as the study's `fable_reference()`) and
//! cross-checked against the #9776 bars frozen in `tasks.json` for the
//! tasks both use. Every beat is recomputed from the row's own numbers and
//! must agree with the row's `beat_the_bar` where it has one.
//!
//! The arm changed between series, so the board has one split per series
//! and pools nothing across them in its caveats.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::contract::{
    Attempt, Bar, Benchmark, Board, BoardKind, Caveat, Cost, CostBasis, EvidenceFile, JevSummary,
    Label, Miss, Phases, Provenance, Reference, Spend, Split, Subject, Tally, TaskKnowledge,
    TaskRow, TaskStatus, TraceRef, VerifierSummary,
};
use crate::evidence::{
    Reader, Result, array, at, close, count, fail, number, opt_number, opt_string, opt_u64, string,
};
use crate::tb4_delegate::TraceJob;
use crate::{THIN_MARGIN, pct};

/// The board's identity.
pub const BOARD_ID: &str = "tb4-fable-delegate-dev-9746";

const EXPERIMENT: &str = "bench/terminal-bench/experiments/2026-09-27-fable-delegate";
/// The row files, in series order.
const ROW_FILES: [&str; 3] = [
    "attempts.json",
    "attempts-series2-5.json",
    "attempts-series6-7.json",
];
const REPORT: &str = "docs/terminal-bench/2026-09-27-fable-delegate.md";
const TRACES: &str = "bench/terminal-bench/traces";
/// Fable 5.1's public trials: the reference.
pub const REPLAYS: &str = "bench/terminal-bench/reference/fable-5.1-replays.json";
/// The #9776 bars, frozen before that experiment ran; the tasks both
/// experiments use must agree.
const FROZEN_BARS: &str =
    "bench/terminal-bench/experiments/2026-09-27-fable-delegate-repro/tasks.json";
/// The first series whose briefing was tuned on `fin-saccr-rwa`'s own
/// earlier results: series 5's sentence came from series 3's verifier
/// failures, and series 7's question set from series 6's results.
const FIRST_TUNED_SERIES: u32 = 5;
const TUNED_TASK: &str = "fin-saccr-rwa";

/// The bar on `task` from the public replays: Fable 5.1 low's cheapest and
/// fastest winning runs, with `gym`'s own reference rule.
pub fn bar_of(replays: &Value, task: &str) -> Result<Bar> {
    let reference = gym::runs_beats_winner::reference_task(replays, task);
    let cheapest = reference
        .cheapest()
        .ok_or_else(|| fail!("{task}: Fable 5.1 low has no winning run with a cost"))?;
    let fastest = reference
        .fastest()
        .ok_or_else(|| fail!("{task}: Fable 5.1 low has no winning run with a time"))?;
    Ok(Bar {
        cost_usd: cheapest.cost_usd,
        seconds: fastest.seconds,
        cost_trial: Some(cheapest.id.clone()),
        time_trial: Some(fastest.id.clone()),
        deadline_seconds: None,
        reference_passes: Some(count(reference.winners.len())),
        reference_trials: Some(count(reference.attempts)),
    })
}

/// The series and attempt number from a job name ending `9746-a1` (series
/// 1) or `9746-s7a1`.
fn series_of(job: &str) -> Result<(u32, u32)> {
    let tail = job
        .rsplit("--")
        .next()
        .and_then(|t| t.strip_prefix("9746-"))
        .ok_or_else(|| fail!("{job}: not a #9746 job"))?;
    let (series, attempt) = match tail.strip_prefix('s') {
        Some(rest) => {
            let (s, a) = rest
                .split_once('a')
                .ok_or_else(|| fail!("{job}: no attempt number"))?;
            (s.parse().ok(), a.parse().ok())
        }
        None => (Some(1), tail.strip_prefix('a').and_then(|a| a.parse().ok())),
    };
    series
        .zip(attempt)
        .ok_or_else(|| fail!("{job}: can't read the series and attempt"))
}

/// Knowledge entry IDs a briefing included, from `knowledge <id> v<n> …`.
fn included(row: &Value) -> Vec<String> {
    at(row, "briefing_knowledge/included")
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|line| {
            let mut words = line.as_str()?.split_whitespace();
            (words.next()? == "knowledge").then_some(())?;
            let id = words.next()?;
            // "knowledge omission note" is the briefing's own note.
            words.next()?.starts_with('v').then(|| id.to_owned())
        })
        .collect()
}

/// Builds the board and lists the traces to bundle.
pub fn build(reader: &Reader) -> Result<(Board, Vec<TraceJob>)> {
    let mut evidence = Vec::new();
    let mut rows = Vec::new();
    for file in ROW_FILES {
        let (doc, entry) = reader.json(&format!("{EXPERIMENT}/{file}"))?;
        evidence.push(entry);
        let list = doc
            .as_array()
            .ok_or_else(|| fail!("{file}: not a list of rows"))?;
        rows.extend(list.iter().cloned().map(|r| (file, r)));
    }
    let (replays, replays_file) = reader.json(REPLAYS)?;
    let (frozen, frozen_file) = reader.json(FROZEN_BARS)?;
    evidence.push(replays_file);
    evidence.push(frozen_file);
    let knowledge = gym::runs_microcoder::Knowledge::read(&reader.root().join("knowledge"));

    let mut bars: BTreeMap<String, Bar> = BTreeMap::new();
    let mut task_order: Vec<String> = Vec::new();
    let mut attempts = Vec::new();
    let mut jobs = Vec::new();
    let mut own_by_task: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut knowledge_files: BTreeSet<String> = BTreeSet::new();
    let mut jev_files: Vec<EvidenceFile> = Vec::new();
    let mut versions: Vec<(u32, String)> = Vec::new();
    let mut arms: Vec<(u32, String)> = Vec::new();
    let mut model = None;
    for (file, row) in &rows {
        let task = string(row, "trial")?
            .split("__")
            .next()
            .map(str::to_owned)
            .ok_or_else(|| fail!("{file}: a row without a trial"))?;
        let job = string(row, "job")?;
        let (series, number) = series_of(&job)?;
        if !bars.contains_key(&task) {
            let bar = bar_of(&replays, &task)?;
            check_frozen(&frozen, &task, &bar)?;
            bars.insert(task.clone(), bar);
            task_order.push(task.clone());
        }
        let bar = bars[&task].clone();
        let arm = job
            .split("--")
            .nth(1)
            .ok_or_else(|| fail!("{job}: no arm in the job name"))?
            .to_owned();
        if !arms.iter().any(|(s, _)| *s == series) {
            arms.push((series, arm));
        }
        let this_model = (
            string(row, "delegate/model")?,
            opt_string(row, "delegate/effort"),
        );
        match &model {
            None => model = Some(this_model),
            Some(m) if *m != this_model => {
                return Err(fail!("{job}: delegate {this_model:?} differs from {m:?}"));
            }
            Some(_) => {}
        }

        // Knowledge the briefing included, and which of it was written
        // from this task.
        let entries = included(row);
        let own: BTreeSet<String> = entries
            .iter()
            .filter(|id| {
                knowledge.written_from.get(*id).is_some_and(|from| {
                    from.iter()
                        .filter(|w| w.as_str() != "reference")
                        .any(|w| knowledge::evidence::task_of(w) == task)
                })
            })
            .cloned()
            .collect();
        for id in &entries {
            if knowledge.written_from.contains_key(id) {
                knowledge_files.insert(format!("knowledge/{id}.md"));
            }
        }
        own_by_task
            .entry(task.clone())
            .or_default()
            .extend(own.iter().cloned());

        let trial = string(row, "trial")?;
        let episode = format!("{TRACES}/{job}/{trial}.episode");
        let (trajectory, _) = reader.json(&format!("{episode}/trajectory.atif.json"))?;
        if let Some(v) = opt_string(&trajectory, "agent/version") {
            versions.push((series, v));
        }
        let jev_rel = format!("{episode}/artifacts/briefing-jev.json");
        let jev = if reader.exists(&jev_rel) {
            let (record, entry) = reader.json(&jev_rel)?;
            jev_files.push(entry);
            Some(jev_summary(&record, &own)?)
        } else {
            None
        };
        let attempt = attempt(row, &task, series, number, &bar, &entries, &own, jev)?;
        jobs.push(TraceJob {
            attempt: attempt.clone(),
            bar,
            episode,
            own_entries: own,
        });
        attempts.push(attempt);
    }
    for rel in &knowledge_files {
        evidence.push(reader.bytes(rel)?.1);
    }
    evidence.extend(jev_files);
    let (_, report_file) = reader.bytes(REPORT)?;
    evidence.push(report_file);
    let (model, effort) = model.ok_or_else(|| fail!("no rows"))?;
    arms.sort();
    versions.sort();
    let mut artifacts: Vec<String> = Vec::new();
    for (_, v) in versions {
        if !artifacts.contains(&v) {
            artifacts.push(v);
        }
    }

    let tasks: Vec<TaskRow> = task_order
        .iter()
        .map(|task| {
            let mine: Vec<&Attempt> = attempts.iter().filter(|a| &a.task == task).collect();
            let passes = count(mine.iter().filter(|a| a.passed).count());
            let beats = count(mine.iter().filter(|a| a.beat).count());
            let own = own_by_task.get(task).map_or(0, BTreeSet::len);
            let assisted = mine
                .iter()
                .any(|a| a.labels.contains(&Label::KnowledgeAssisted));
            TaskRow {
                task: task.clone(),
                bar: bars[task].clone(),
                knowledge: if own > 0 {
                    TaskKnowledge::Own {
                        candidates: count(own),
                    }
                } else if assisted {
                    TaskKnowledge::OtherTasksOnly
                } else {
                    TaskKnowledge::Off
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

    let tally_of = |filter: &dyn Fn(&Attempt) -> bool| {
        let set: Vec<&Attempt> = attempts.iter().filter(|a| filter(a)).collect();
        Tally {
            attempts: count(set.len()),
            passes: count(set.iter().filter(|a| a.passed).count()),
            beats: count(set.iter().filter(|a| a.beat).count()),
            // No attempt on #9746 was a fault; the rows are results only.
            faults: 0,
            cost_unknown: count(set.iter().filter(|a| a.cost.known().is_none()).count()),
        }
    };
    let totals = tally_of(&|_| true);
    let series_list: BTreeSet<String> = attempts.iter().map(|a| a.series.clone()).collect();
    let mut series_list: Vec<String> = series_list.into_iter().collect();
    series_list.sort_by_key(|s| s.trim_start_matches("series ").parse::<u32>().unwrap_or(0));
    let splits: Vec<Split> = series_list
        .iter()
        .map(|name| Split {
            name: name.clone(),
            tally: tally_of(&|a| &a.series == name),
        })
        .collect();

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

    let beats: Vec<&Attempt> = attempts.iter().filter(|a| a.beat).collect();
    let beat_names: Vec<String> = beats.iter().map(|a| label_of(&a.id)).collect();
    let headline = format!(
        "Beat Fable 5.1 low's cheapest and fastest win on {} of {} attempts across {} series ({}); passed {} of {}.",
        totals.beats,
        totals.attempts,
        splits.len(),
        if beat_names.is_empty() {
            "none".to_owned()
        } else {
            beat_names.join(" and ")
        },
        totals.passes,
        totals.attempts,
    );
    let caveats = caveats(&attempts, &splits, &totals);
    let mut labels = vec![
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

    let board = Board {
        id: BOARD_ID.into(),
        title: "Fable delegate development, series 1 to 7".into(),
        benchmark: Benchmark {
            name: "Terminal-Bench".into(),
            version: "4.0".into(),
        },
        kind: BoardKind::BeatCheapestAndFastestWin,
        question: "Can Coder One use Jev to prepare a Terminal-Bench 4 task, hand it to Fable 5.1, and get a passing result that is both cheaper and faster than Fable 5.1 low's own winning runs on the same task?".into(),
        headline,
        provenance: Provenance {
            issues: vec![9746, 9680],
            report: REPORT.into(),
            frozen_commit: None,
            evidence,
        },
        subject: Subject {
            agent: "Coder One".into(),
            arm: arms
                .iter()
                .map(|(s, arm)| format!("{arm} (series {s})"))
                .collect::<Vec<_>>()
                .join(", "),
            model,
            effort,
            artifact: (!artifacts.is_empty()).then(|| artifacts.join(", ")),
        },
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

/// `s7a1` from `fin-saccr-rwa.s7a1`.
fn label_of(id: &str) -> String {
    id.rsplit('.').next().unwrap_or(id).to_owned()
}

/// A bar computed from the replays must agree with the bar #9776 froze
/// for the same task, when it froze one.
fn check_frozen(frozen: &Value, task: &str, bar: &Bar) -> Result<()> {
    let Some(row) = array(frozen, "tasks")?
        .iter()
        .find(|t| at(t, "task").as_str() == Some(task))
    else {
        return Ok(());
    };
    let cost = number(row, "bar_cost_usd")?;
    let seconds = number(row, "bar_seconds")?;
    // The frozen seconds come from microsecond timestamps; `gym`'s from
    // milliseconds.
    if !close(cost, bar.cost_usd.unwrap_or(f64::NAN))
        || (seconds - bar.seconds.unwrap_or(f64::NAN)).abs() > 0.01
    {
        return Err(fail!(
            "{task}: the replays' bar ({:?}, {:?}) differs from the bar #9776 froze ({cost}, {seconds})",
            bar.cost_usd,
            bar.seconds
        ));
    }
    Ok(())
}

fn jev_summary(record: &Value, own: &BTreeSet<String>) -> Result<JevSummary> {
    let candidates = array(record, "candidates")?;
    let kept: Vec<&Value> = candidates
        .iter()
        .filter(|c| at(c, "kept").as_bool() == Some(true))
        .collect();
    let kept_own = kept
        .iter()
        .filter(|c| opt_string(c, "id").is_some_and(|id| own.contains(&id)))
        .count();
    let requirements = array(record, "requirements")?;
    Ok(JevSummary {
        // Series 6's records predate the field.
        question_set: opt_string(record, "question_set").unwrap_or_else(|| "not recorded".into()),
        outcome: string(record, "outcome")?,
        milliseconds: opt_u64(record, "milliseconds"),
        candidates: count(candidates.len()),
        kept: count(kept.len()),
        kept_own: count(kept_own),
        requirements: count(requirements.len()),
        flagged: count(
            requirements
                .iter()
                .filter(|r| at(r, "flagged").as_bool() == Some(true))
                .count(),
        ),
    })
}

#[allow(clippy::too_many_arguments)]
fn attempt(
    row: &Value,
    task: &str,
    series: u32,
    number: u32,
    bar: &Bar,
    entries: &[String],
    own: &BTreeSet<String>,
    jev: Option<JevSummary>,
) -> Result<Attempt> {
    let label = format!("s{series}a{number}");
    let reward = opt_number(row, "reward");
    let passed = reward.is_some_and(|r| r >= 1.0);
    let seconds = opt_number(row, "trial_seconds");
    let answered = at(row, "delegate/total_cost_usd").is_number();
    let cost = if answered {
        // Series 2 on add the knowledge search's charge; series 1 had none.
        let usd = opt_number(row, "cost/total_with_search_usd")
            .or_else(|| opt_number(row, "cost/total_usd"))
            .ok_or_else(|| fail!("{task} {label}: an answered delegate with no total"))?;
        Cost::Reported { usd }
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
    // Series 1's rows were written without a bar; the rest record their
    // verdict, which must be this one.
    if let Some(recorded) = row.get("beat_the_bar")
        && recorded.as_bool() != Some(beat)
    {
        return Err(fail!(
            "{task} {label}: recomputed beat {beat} but the row says {recorded}"
        ));
    }
    let (cost_ratio, cost_ratio_is_bound) = match cost {
        Cost::Reported { usd } => (Some(usd / cost_bar), false),
        Cost::Unknown {
            lower_bound_usd, ..
        } => (lower_bound_usd.map(|l| l / cost_bar), true),
    };
    let time_ratio = seconds.map(|s| s / time_bar);

    let mut labels = Vec::new();
    if !entries.is_empty() {
        labels.push(Label::KnowledgeAssisted);
        if !own.is_empty() {
            labels.push(Label::InSample);
        }
    }
    if cost.known().is_none() {
        labels.push(Label::CostBound);
    }
    let thin = beat
        && (1.0 - cost_ratio.unwrap_or(1.0) < THIN_MARGIN
            || 1.0 - time_ratio.unwrap_or(1.0) < THIN_MARGIN);
    if thin {
        labels.push(Label::ThinMargin);
    }
    labels.sort();
    let tuned = task == TUNED_TASK && series >= FIRST_TUNED_SERIES;
    let mut caveats = Vec::new();
    if beat {
        if labels.contains(&Label::InSample) {
            caveats.push("in_sample".to_owned());
        }
        if tuned {
            caveats.push("tuned_on_task".to_owned());
        }
        if thin {
            caveats.push("thin_margin".to_owned());
        }
        if jev.is_none() {
            caveats.push("no_jev_decision".to_owned());
        }
    }

    let deadline = opt_u64(row, "delegate/deadline_sec");
    let delegate_seconds = opt_number(row, "agent_sec/delegate");
    let jev_calls = opt_u64(row, "explore/jev_calls").unwrap_or(0);
    let mut how = match (string(row, "delegate/status")?.as_str(), deadline) {
        ("answered", _) => format!(
            "The delegate answered after {:.1} s.",
            delegate_seconds.unwrap_or(f64::NAN)
        ),
        ("timed_out", Some(d)) => {
            format!("The delegate ran to its {d} s deadline; its cost is unknown.")
        }
        (other, _) => format!("The delegate ended as {other}."),
    };
    if jev.is_none() {
        how.push_str(&format!(
            " Jev made no briefing decision ({jev_calls} Jev call{} in all).",
            if jev_calls == 1 { "" } else { "s" }
        ));
    }

    Ok(Attempt {
        id: format!("{task}.{label}"),
        task: task.to_owned(),
        series: format!("series {series}"),
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
        how_it_ended: Some(how),
        jev,
        verifier: Some(VerifierSummary {
            summary: None,
            failed_tests: Vec::new(),
        }),
        trace: Some(TraceRef {
            path: crate::trace_path(BOARD_ID, &format!("{task}.{label}")),
            sha256: String::new(),
            bytes: 0,
        }),
        cost_basis: None,
        caveats,
    })
}

fn caveats(attempts: &[Attempt], splits: &[Split], totals: &Tally) -> Vec<Caveat> {
    let beats: Vec<&Attempt> = attempts.iter().filter(|a| a.beat).collect();
    let in_sample = beats
        .iter()
        .filter(|a| a.labels.contains(&Label::InSample))
        .count();
    let tuned: Vec<String> = beats
        .iter()
        .filter(|a| a.caveats.iter().any(|c| c == "tuned_on_task"))
        .map(|a| label_of(&a.id))
        .collect();
    let without_jev: Vec<String> = beats
        .iter()
        .filter(|a| a.jev.is_none())
        .map(|a| label_of(&a.id))
        .collect();
    let with_jev: Vec<String> = beats
        .iter()
        .filter(|a| a.jev.is_some())
        .map(|a| label_of(&a.id))
        .collect();
    let mut out = vec![Caveat {
        code: "in_sample".into(),
        text: format!(
            "{in_sample} of {} beats briefed the delegate with knowledge written from Coder's earlier runs on the same task. The board says nothing about tasks Coder hasn't seen; #9776 tried the winning arm on 14 more.",
            beats.len()
        ),
    }];
    out.push(Caveat {
        code: "tuned_on_task".into(),
        text: format!(
            "From series {FIRST_TUNED_SERIES} on, the briefing was tuned on {TUNED_TASK}'s own earlier results: series 5's sentence on the delta sign rule after reading series 3's verifier failures, and series 7's question set after reading series 6's. {} of {} beats came from tuned series{}.",
            tuned.len(),
            beats.len(),
            if tuned.is_empty() {
                String::new()
            } else {
                format!(" ({})", tuned.join(", "))
            }
        ),
    });
    if !without_jev.is_empty() {
        out.push(Caveat {
            code: "no_jev_decision".into(),
            text: format!(
                "{} beat the bar with no Jev decision: code built its briefing from the instruction and a lexically ranked knowledge selection, so it isn't a Jev win.{}",
                without_jev.join(", "),
                if with_jev.is_empty() {
                    String::new()
                } else {
                    format!(" Jev decided the briefing in {}.", with_jev.join(", "))
                }
            ),
        });
    }
    let thin: Vec<String> = beats
        .iter()
        .filter(|a| a.labels.contains(&Label::ThinMargin))
        .map(|a| {
            format!(
                "{} by {}",
                label_of(&a.id),
                pct((1.0 - a.cost_ratio.unwrap_or(1.0)).min(1.0 - a.time_ratio.unwrap_or(1.0)))
            )
        })
        .collect();
    if !thin.is_empty() {
        out.push(Caveat {
            code: "thin_margin".into(),
            text: format!(
                "{} of {} beats came in under a bar by less than {}: {}.",
                thin.len(),
                beats.len(),
                pct(THIN_MARGIN),
                thin.join(", ")
            ),
        });
    }
    out.push(Caveat {
        code: "arm_changed".into(),
        text: format!(
            "The arm changed between series, so each series is its own split and none is pooled with another. Series sizes: {}.",
            splits
                .iter()
                .map(|s| format!("{} of {} beat in {}", s.tally.beats, s.tally.attempts, s.name))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    });
    out.push(Caveat {
        code: "cost_unknown".into(),
        text: format!(
            "{} of {} delegates ran to their deadline, so their cost is unknown and they count as misses. Their lower-bound estimates price the stream's usage at list price and leave out the call in flight.",
            totals.cost_unknown, totals.attempts
        ),
    });
    out.push(Caveat {
        code: "list_price".into(),
        text: "Every cost is a list-price figure on a subscription login, like the reference's, not a bill.".into(),
    });
    out.push(Caveat {
        code: "reference_conditions".into(),
        text: "The reference ran Claude Code 2.1.273 on another host with open network and each task's full timeout. These attempts ran Claude Code 2.1.280 inside Coder One, with a delegate deadline set per series.".into(),
    });
    out
}
