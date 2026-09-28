//! The TB4 knowledge-assisted Microcoder board: the "one shared fact"
//! result in `docs/coder/beat-fable-showcase.md` and
//! `docs/coder/cheapest-verified-passes.md`.
//!
//! The board is computed with the Gym's own `beats-winner` rule
//! ([`gym::runs_beats_winner::beats_winner`]) over the retained records in
//! `bench/terminal-bench/microcoder-runs/coderos-4080`, read with `gym`'s
//! own reader and the checkout's knowledge provenance, against Fable 5.1
//! low's public winning runs. Every task the rule finds an in-sample,
//! knowledge-assisted claim on is a task row. Its runs split in two: the
//! claim's own group (`with the entry`: graded runs that used an entry
//! written from the task) and the task's other graded runs (`before the
//! entry`). Each attempt's beat (a pass with known cost below the cheapest
//! winning run) is recomputed here and must agree with the claim's own
//! counts and pass list; any disagreement refuses to build.
//!
//! Runs whose directory two runs wrote (`mixed`) and runs with no summary
//! aren't graded, as the Gym leaves them out, and the board says how many.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde_json::Value;

use crate::contract::{
    Attempt, Bar, Benchmark, Board, BoardKind, Caveat, Cost, CostBasis, Label, Miss, Provenance,
    Reference, Spend, Split, Subject, Tally, TaskKnowledge, TaskRow, TaskStatus, TraceRef,
    VerifierSummary,
};
use crate::evidence::{Reader, Result, at, count, fail, opt_number};
use crate::{THIN_MARGIN, pct, usd};
use gym::runs::{Agent, Outcome, Run};
use gym::runs_microcoder::{CostBasis as RunBasis, Knowledge, Sample};

/// The board's identity.
pub const BOARD_ID: &str = "tb4-microcoder-shared-fact";

/// The retained records the rule reads.
pub const RUNS: &str = "bench/terminal-bench/microcoder-runs/coderos-4080";
const SHOWCASE: &str = "docs/coder/beat-fable-showcase.md";
const ESSAY: &str = "docs/coder/cheapest-verified-passes.md";
const RESULTS: &str = "docs/terminal-bench/tb4-results.md";

/// The two splits every task has.
pub const BEFORE: &str = "before the entry";
pub const WITH: &str = "with the entry";

/// A graded run the rule may make a claim about, as `gym` decides.
fn claimable(run: &Run) -> bool {
    run.agent == Agent::Microcoder
        && matches!(run.outcome, Outcome::Passed | Outcome::Failed)
        && run.microcoder.as_deref().is_some_and(|m| !m.mixed)
}

fn in_sample_group(run: &Run) -> bool {
    run.microcoder
        .as_deref()
        .is_some_and(|m| m.knowledge_assisted && m.sample == Some(Sample::InSample))
}

fn number(highlight: &gym::runs_highlights::Highlight, label: &str) -> Option<u32> {
    highlight
        .numbers
        .iter()
        .find(|n| n.label == label)
        .map(|n| n.value.round() as u32)
}

/// Builds the board. `repro` is the #9776 board, whose result the
/// same-task caveat cites.
pub fn build(reader: &Reader, repro: &Board) -> Result<Board> {
    let (replays, replays_file) = reader.json(crate::tb4_delegate_dev::REPLAYS)?;
    let (_, manifest_file) = reader.bytes(&format!("{RUNS}/{}", gym::runs_microcoder::MANIFEST))?;
    let mut evidence = vec![manifest_file, replays_file];
    let knowledge = Knowledge::read(&reader.root().join("knowledge"));
    // Graded or not, from the retained copy; `now` far ahead, so a run
    // without a summary reads as stopped, not running.
    let runs =
        gym::runs_microcoder::read_all(&[reader.root().join(RUNS)], &knowledge, i64::MAX / 4);
    for run in &runs {
        if let Some(m) = run.microcoder.as_deref()
            && !m.digest_mismatches.is_empty()
        {
            return Err(fail!(
                "{RUNS}/{}: {} doesn't match the manifest",
                m.name,
                m.digest_mismatches.join(" and ")
            ));
        }
    }

    let answers = HashMap::new();
    let marks = gym::runs_marks::Marks::default();
    let highlights = gym::runs_beats_winner::beats_winner(&gym::runs_highlights::Inputs {
        runs: &runs,
        answers: &answers,
        reference: None,
        marks: &marks,
        fable: Some(&replays),
    });
    let claims: Vec<&gym::runs_highlights::Highlight> = highlights
        .iter()
        .filter(|h| {
            h.detail.as_ref().is_some_and(|d| {
                at(d, "labels/in_sample").as_bool() == Some(true)
                    && at(d, "labels/knowledge_assisted").as_bool() == Some(true)
            })
        })
        .collect();
    if claims.is_empty() {
        return Err(fail!("the beats-winner rule found no in-sample claim"));
    }

    let mut attempts = Vec::new();
    let mut tasks = Vec::new();
    let mut entries_by_task: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut left_out: BTreeMap<String, (u32, u32)> = BTreeMap::new();
    let mut faster = 0_u32;
    for claim in &claims {
        let task = claim
            .task
            .clone()
            .ok_or_else(|| fail!("{}: a claim with no task", claim.key))?;
        let reference = gym::runs_beats_winner::reference_task(&replays, &task);
        let bar = crate::tb4_delegate_dev::bar_of(&replays, &task)?;
        let cheapest = bar.cost_usd.ok_or_else(|| fail!("{task}: no cost bar"))?;
        let mine: Vec<&Run> = runs.iter().filter(|r| r.task == task).collect();
        let model = mine
            .iter()
            .copied()
            .filter(|r| claimable(r) && in_sample_group(r))
            .find_map(|r| r.model.clone());
        let graded: Vec<&Run> = mine
            .iter()
            .copied()
            .filter(|r| claimable(r) && r.model == model)
            .collect();
        let mixed = count(
            mine.iter()
                .filter(|r| r.microcoder.as_deref().is_some_and(|m| m.mixed))
                .count(),
        );
        let ungraded = count(
            mine.iter()
                .filter(|r| matches!(r.outcome, Outcome::NotGraded(_)))
                .count(),
        );
        left_out.insert(task.clone(), (mixed, ungraded));

        let mut task_attempts = Vec::new();
        for run in &graded {
            let with = in_sample_group(run);
            let (summary, summary_file) =
                reader.json(&format!("{RUNS}/{}/summary.json", run.trial))?;
            let (_, events_file) = reader.bytes(&format!("{RUNS}/{}/events.jsonl", run.trial))?;
            evidence.push(summary_file);
            evidence.push(events_file);
            let m = run.microcoder.as_deref().expect("a Microcoder run");
            if with {
                for e in m.entries.iter().filter(|e| e.names_task) {
                    entries_by_task
                        .entry(task.clone())
                        .or_default()
                        .insert(e.id.clone());
                }
            }
            let a = attempt(run, &summary, &task, with, &bar, cheapest)?;
            if a.passed && a.time_ratio.is_some_and(|t| t < 1.0) && with {
                faster += 1;
            }
            task_attempts.push(a);
        }

        // The claim's own numbers must be what this board counts.
        let with_set: Vec<&Attempt> = task_attempts.iter().filter(|a| a.series == WITH).collect();
        let passes = count(with_set.iter().filter(|a| a.passed).count());
        let beats = count(with_set.iter().filter(|a| a.beat).count());
        for (label, got) in [
            ("graded", count(with_set.len())),
            ("passes", passes),
            ("cheaper_passes", beats),
            ("reference_attempts", count(reference.attempts)),
            ("reference_winning_runs", count(reference.winners.len())),
        ] {
            if number(claim, label) != Some(got) {
                return Err(fail!(
                    "{task}: counted {label} {got}, the beats-winner claim says {:?}",
                    number(claim, label)
                ));
            }
        }
        let pass_ids: BTreeSet<String> = with_set
            .iter()
            .filter(|a| a.passed)
            .map(|a| format!("{}/{}", gym::runs_microcoder::JOB, a.id))
            .collect();
        let claimed: BTreeSet<String> = claim.runs.iter().cloned().collect();
        if pass_ids != claimed {
            return Err(fail!(
                "{task}: the passes {pass_ids:?} aren't the claim's {claimed:?}"
            ));
        }

        let own = entries_by_task.get(&task).map_or(0, BTreeSet::len);
        let passes_all = count(task_attempts.iter().filter(|a| a.passed).count());
        let beats_all = count(task_attempts.iter().filter(|a| a.beat).count());
        tasks.push(TaskRow {
            task: task.clone(),
            bar,
            knowledge: TaskKnowledge::Own {
                candidates: count(own),
            },
            attempts: task_attempts.iter().map(|a| a.id.clone()).collect(),
            passes: passes_all,
            beats: beats_all,
            status: if beats_all > 0 {
                TaskStatus::Beat
            } else if passes_all > 0 {
                TaskStatus::PassedWithoutBeat
            } else {
                TaskStatus::NeverPassed
            },
        });
        attempts.extend(task_attempts);
    }
    for id in entries_by_task.values().flatten().collect::<BTreeSet<_>>() {
        evidence.push(reader.bytes(&format!("knowledge/{id}.md"))?.1);
    }
    for rel in [SHOWCASE, ESSAY, RESULTS] {
        evidence.push(reader.bytes(rel)?.1);
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
    let totals = tally_of(&|_| true);
    let mut splits = vec![
        Split {
            name: BEFORE.into(),
            tally: tally_of(&|a| a.series == BEFORE),
        },
        Split {
            name: WITH.into(),
            tally: tally_of(&|a| a.series == WITH),
        },
    ];
    for t in &tasks {
        for series in [BEFORE, WITH] {
            splits.push(Split {
                name: format!("{}, {series}", t.task),
                tally: tally_of(&|a| a.task == t.task && a.series == series),
            });
        }
    }

    let reported_usd: f64 = attempts.iter().filter_map(|a| a.cost.known()).sum();
    let bound = |upper: bool| -> f64 {
        attempts
            .iter()
            .filter_map(|a| match a.cost {
                Cost::Unknown {
                    lower_bound_usd,
                    upper_bound_usd,
                } => {
                    if upper {
                        upper_bound_usd
                    } else {
                        lower_bound_usd
                    }
                }
                Cost::Reported { .. } => None,
            })
            .sum()
    };
    let bases: BTreeSet<CostBasis> = attempts.iter().filter_map(|a| a.cost_basis).collect();
    let basis = match bases.len() {
        1 => *bases.iter().next().expect("one"),
        _ => CostBasis::Mixed,
    };
    let spend = Spend {
        basis,
        reported_usd,
        estimated_lower_bound_usd: (totals.cost_unknown > 0).then(|| bound(false)),
        estimated_upper_bound_usd: (totals.cost_unknown > 0).then(|| bound(true)),
    };

    let split = |task: &str, series: &str| {
        splits
            .iter()
            .find(|s| s.name == format!("{task}, {series}"))
            .map(|s| s.tally)
            .unwrap_or_default()
    };
    let per_task: Vec<String> = tasks
        .iter()
        .map(|t| {
            let (b, w) = (split(&t.task, BEFORE), split(&t.task, WITH));
            format!(
                "{} {} of {} (before, {} of {})",
                t.task, w.beats, w.attempts, b.beats, b.attempts
            )
        })
        .collect();
    let with = splits[1].tally;
    let before = splits[0].tally;
    let headline = format!(
        "In-sample: with an entry written from the task, Microcoder beat Fable 5.1 low's cheapest winning run on {} of {} graded runs of {} TB4 tasks, and on {} of {} before the entry: {}.",
        with.beats,
        with.attempts,
        tasks.len(),
        before.beats,
        before.attempts,
        per_task.join("; ")
    );

    let caveats = caveats(
        &attempts,
        &tasks,
        &entries_by_task,
        &left_out,
        faster,
        repro,
        spend.basis,
    );
    let mut labels = vec![
        Label::KnowledgeAssisted,
        Label::InSample,
        Label::FewAttempts,
        Label::ReferenceOtherConditions,
    ];
    if bases.contains(&CostBasis::ListPrice) {
        labels.push(Label::ListPrice);
    }
    if totals.cost_unknown > 0 {
        labels.push(Label::CostBound);
    }
    labels.sort();
    let effort: BTreeSet<String> = runs
        .iter()
        .filter(|r| attempts.iter().any(|a| a.id == r.trial))
        .filter_map(|r| r.microcoder.as_deref().and_then(|m| m.effort.clone()))
        .collect();
    let models: BTreeSet<String> = runs
        .iter()
        .filter(|r| attempts.iter().any(|a| a.id == r.trial))
        .filter_map(|r| r.model.clone())
        .collect();

    Ok(Board {
        id: BOARD_ID.into(),
        title: "Microcoder with one shared fact, TB4 (in-sample)".into(),
        benchmark: Benchmark {
            name: "Terminal-Bench".into(),
            version: "4.0".into(),
        },
        kind: BoardKind::CostBelowCheapestWin,
        question: "Given one precise, cited fact from the shared knowledge base, does the cheap Microcoder loop pass TB4 tasks it failed before, for less than Fable 5.1 low's cheapest winning run?".into(),
        headline,
        provenance: Provenance {
            issues: vec![9843, 9684, 9680],
            report: SHOWCASE.into(),
            frozen_commit: None,
            evidence,
        },
        subject: Subject {
            agent: "Microcoder".into(),
            arm: "knowledge base on or candidates, entries from the NIP-KB relay".into(),
            model: models.into_iter().collect::<Vec<_>>().join(", "),
            effort: (!effort.is_empty()).then(|| effort.into_iter().collect::<Vec<_>>().join(", ")),
            artifact: None,
        },
        reference: Reference {
            name: "Fable 5.1 low".into(),
            rule: "A beat is a pass whose known total cost (model, Jev, and embeddings) is below Fable 5.1 low's cheapest winning run on the task: the Gym's beats-winner rule. Time is shown against its fastest win but isn't part of the rule. Unknown cost can't beat.".into(),
            conditions: "Fable 5.1 low's public Claude Code trials, on another host with each task's full timeout; its time is the trial's wall time and its cost the public record's.".into(),
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
    })
}

fn attempt(
    run: &Run,
    summary: &Value,
    task: &str,
    with: bool,
    bar: &Bar,
    cheapest: f64,
) -> Result<Attempt> {
    let m = run.microcoder.as_deref().expect("a Microcoder run");
    let passed = run.outcome == Outcome::Passed;
    let cost = match run.cost_usd {
        Some(usd) => Cost::Reported { usd },
        None => Cost::Unknown {
            lower_bound_usd: opt_number(summary, "outcome/known_usd"),
            upper_bound_usd: opt_number(summary, "outcome/usd_upper"),
        },
    };
    let mut misses = Vec::new();
    if !passed {
        misses.push(Miss::Failed);
    }
    match cost.known() {
        None => misses.push(Miss::CostUnknown),
        Some(c) if c >= cheapest => misses.push(Miss::Cost),
        Some(_) => {}
    }
    let beat = misses.is_empty();
    let seconds = run.agent_ms.map(|ms| ms as f64 / 1000.0);
    let (cost_ratio, cost_ratio_is_bound) = match cost {
        Cost::Reported { usd } => (Some(usd / cheapest), false),
        Cost::Unknown {
            lower_bound_usd, ..
        } => (lower_bound_usd.map(|l| l / cheapest), true),
    };
    let time_ratio = seconds.zip(bar.seconds).map(|(s, b)| s / b);

    let mut labels = Vec::new();
    if m.knowledge_assisted {
        labels.push(Label::KnowledgeAssisted);
    } else {
        labels.push(Label::KnowledgeOff);
    }
    match m.sample {
        Some(Sample::InSample) => labels.push(Label::InSample),
        Some(Sample::OutOfSample) => labels.push(Label::OutOfSample),
        _ => {}
    }
    let cost_basis = match m.cost_basis {
        Some(RunBasis::ListPrice) => {
            labels.push(Label::ListPrice);
            Some(CostBasis::ListPrice)
        }
        Some(RunBasis::Billed) => Some(CostBasis::Billed),
        _ => None,
    };
    if cost.known().is_none() {
        labels.push(Label::CostBound);
    }
    let thin = beat && 1.0 - cost_ratio.unwrap_or(1.0) < THIN_MARGIN;
    if thin {
        labels.push(Label::ThinMargin);
    }
    labels.sort();
    let mut caveats = Vec::new();
    if beat {
        if labels.contains(&Label::InSample) {
            caveats.push("same_task".to_owned());
        }
        if thin {
            caveats.push("thin_margin".to_owned());
        }
    }

    Ok(Attempt {
        id: run.trial.clone(),
        task: task.to_owned(),
        series: if with { WITH } else { BEFORE }.into(),
        trial: format!("{RUNS}/{}", run.trial),
        reward: run.reward,
        passed,
        seconds,
        phases: None,
        cost,
        cost_ratio,
        cost_ratio_is_bound,
        time_ratio,
        beat,
        misses,
        labels,
        how_it_ended: m.ending.clone(),
        jev: None,
        verifier: run.tests.map(|t| VerifierSummary {
            summary: Some(format!("{} of {} tests passed", t.passed, t.total)),
            failed_tests: Vec::new(),
        }),
        trace: passed.then(|| TraceRef {
            path: crate::trace_path(BOARD_ID, &run.trial),
            sha256: String::new(),
            bytes: 0,
        }),
        cost_basis,
        caveats,
    })
}

fn caveats(
    attempts: &[Attempt],
    tasks: &[TaskRow],
    entries: &BTreeMap<String, BTreeSet<String>>,
    left_out: &BTreeMap<String, (u32, u32)>,
    faster: u32,
    repro: &Board,
    basis: CostBasis,
) -> Vec<Caveat> {
    let beats: Vec<&Attempt> = attempts.iter().filter(|a| a.beat).collect();
    let in_sample = beats
        .iter()
        .filter(|a| a.labels.contains(&Label::InSample))
        .count();
    let naming: Vec<String> = entries
        .iter()
        .map(|(task, ids)| {
            format!(
                "{task}: {}",
                ids.iter().cloned().collect::<Vec<_>>().join(", ")
            )
        })
        .collect();
    let repro_own = repro
        .attempts
        .iter()
        .filter(|a| a.beat && a.labels.contains(&Label::InSample))
        .count();
    let mut out = vec![Caveat {
        code: "same_task".into(),
        text: format!(
            "In-sample: {in_sample} of {} beats used an entry written from the task it helped ({}). That shows a cheap loop plus the right fact passing; it doesn't show the knowledge transfers to work its author never saw. When a delegate was briefed from the same knowledge base on {} more tasks (#9776), it beat Fable 5.1 low's bar on {} of {} attempts, and {} of those beats used knowledge written from the same task: the reproduction didn't hold.",
            beats.len(),
            naming.join("; "),
            repro.tasks.len(),
            repro.totals.beats,
            repro.totals.attempts,
            repro_own,
        ),
    }];
    let thin: Vec<String> = beats
        .iter()
        .filter(|a| a.labels.contains(&Label::ThinMargin))
        .map(|a| format!("{} by {}", a.id, pct(1.0 - a.cost_ratio.unwrap_or(1.0))))
        .collect();
    if !thin.is_empty() {
        out.push(Caveat {
            code: "thin_margin".into(),
            text: format!(
                "{} of {} beats came in under the bar by less than {}: {}.",
                thin.len(),
                beats.len(),
                pct(THIN_MARGIN),
                thin.join(", ")
            ),
        });
    }
    let (mixed, ungraded) = left_out
        .values()
        .fold((0, 0), |(m, u), (a, b)| (m + a, u + b));
    out.push(Caveat {
        code: "retained_records_only".into(),
        text: format!(
            "The board counts only the retained, graded records the Gym reads: {mixed} run directories that two runs wrote and {ungraded} that stopped before grading are left out on these {} tasks. The showcase's before-and-after table also counts runs whose records aren't retained or are mixed, so its counts differ from these.",
            tasks.len()
        ),
    });
    let with_passes = attempts
        .iter()
        .filter(|a| a.series == WITH && a.passed)
        .count();
    out.push(Caveat {
        code: "time_basis".into(),
        text: format!(
            "Time isn't part of the rule. Microcoder's time is its loop's own, without starting the container or grading; Fable 5.1 low's is the public trial's wall time, which includes both. {faster} of {with_passes} passes with the entry finished faster than its fastest win."
        ),
    });
    let bars: Vec<String> = tasks
        .iter()
        .map(|t| {
            format!(
                "{} {}",
                t.task,
                t.bar.cost_usd.map_or_else(|| "unknown".into(), usd)
            )
        })
        .collect();
    out.push(Caveat {
        code: "cost_basis".into(),
        text: format!(
            "Microcoder's cost is the model plus Jev and the knowledge base's embeddings, {}; Fable 5.1 low's cheapest wins ({}) are the public record's reported cost.",
            match basis {
                CostBasis::Mixed => {
                    let n = |b: CostBasis| attempts.iter().filter(|a| a.cost_basis == Some(b)).count();
                    format!(
                        "billed by OpenRouter on {} runs and GPT-6 Luna's list price for the tokens the Codex login reported on {} (each row says which)",
                        n(CostBasis::Billed),
                        n(CostBasis::ListPrice)
                    )
                }
                CostBasis::Billed => "as OpenRouter billed it".to_owned(),
                _ => "at list price".to_owned(),
            },
            bars.join(", ")
        ),
    });
    out.push(Caveat {
        code: "few_attempts".into(),
        text: "A handful of runs per task, not pre-registered: the counts describe these runs and don't estimate a pass rate.".into(),
    });
    out
}
