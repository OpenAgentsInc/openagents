//! The `beats-winner` highlight rule: a Microcoder pass that cost less
//! than a reference agent's cheapest winning run on the same task, or
//! finished faster than its fastest winning run.
//!
//! The reference is Claude Code with Fable 5.1 at low effort: its public
//! trials in `bench/terminal-bench/reference/fable-5.1-replays.json` (or the
//! replay cache's copy), the ones that passed. Code computes every number
//! from the records; no model writes one.
//!
//! Microcoder's runs of a task are grouped by model and by the labels a
//! claim must carry: whether they were knowledge-assisted and whether they
//! were in-sample (an entry they used was written from this task). One
//! claim per group states how many of the group's graded runs passed, how
//! many passes beat the cheapest winning run and by how much, and how many
//! beat the fastest. The labels, the provider, and the cost basis are part
//! of the claim's text and of its JSON, so the claim can't be quoted
//! without them. Failures in the group count in its denominator.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};

use crate::runs::{Agent, Outcome, Run, duration};
use crate::runs_analysis::usd;
use crate::runs_highlights::{Highlight, Inputs, Number, Rule, key, mark_caveats};
use crate::runs_microcoder::{CostBasis, Microcoder, Sample, provider_name};
use crate::terminal_bench::timestamp_ms;

/// The reference agent's model.
pub const REFERENCE_MODEL: &str = "Fable 5.1";

/// The reference agent's reasoning effort.
pub const REFERENCE_EFFORT: &str = "low";

/// The reference in words.
pub const REFERENCE: &str = "Fable 5.1 low";

/// One winning reference run.
#[derive(Clone, Debug, PartialEq)]
pub struct Winner {
    pub id: String,
    pub cost_usd: Option<f64>,
    /// The public trial's wall time.
    pub seconds: Option<f64>,
}

/// The reference agent's attempts on one task.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReferenceTask {
    pub attempts: usize,
    pub winners: Vec<Winner>,
}

impl ReferenceTask {
    /// The cheapest winning run with a cost.
    #[must_use]
    pub fn cheapest(&self) -> Option<&Winner> {
        self.winners
            .iter()
            .filter(|w| w.cost_usd.is_some())
            .min_by(|a, b| {
                a.cost_usd
                    .unwrap_or(f64::MAX)
                    .total_cmp(&b.cost_usd.unwrap_or(f64::MAX))
                    .then(a.id.cmp(&b.id))
            })
    }

    /// The fastest winning run with a time.
    #[must_use]
    pub fn fastest(&self) -> Option<&Winner> {
        self.winners
            .iter()
            .filter(|w| w.seconds.is_some())
            .min_by(|a, b| {
                a.seconds
                    .unwrap_or(f64::MAX)
                    .total_cmp(&b.seconds.unwrap_or(f64::MAX))
                    .then(a.id.cmp(&b.id))
            })
    }
}

/// The reference agent's attempts on `task`, from the public replay
/// manifest.
#[must_use]
pub fn reference_task(manifest: &Value, task: &str) -> ReferenceTask {
    let mut reference = ReferenceTask::default();
    for trial in manifest["trials"].as_array().into_iter().flatten() {
        if trial["task"] != task
            || trial["model"] != REFERENCE_MODEL
            || trial["effort"] != REFERENCE_EFFORT
        {
            continue;
        }
        reference.attempts += 1;
        if !trial["reward"].as_f64().is_some_and(|r| r >= 1.0) {
            continue;
        }
        let seconds = timestamp_ms(trial["started_at"].as_str().unwrap_or_default())
            .zip(timestamp_ms(
                trial["finished_at"].as_str().unwrap_or_default(),
            ))
            .map(|(start, end)| (end - start) as f64 / 1000.0);
        reference.winners.push(Winner {
            id: trial["id"].as_str().unwrap_or_default().to_owned(),
            cost_usd: trial["cost_usd"].as_f64(),
            seconds,
        });
    }
    reference
}

/// The public manifest `gym runs analyze` reads, parsed once.
#[must_use]
pub fn default_manifest() -> Option<&'static Value> {
    static MANIFEST: std::sync::OnceLock<Option<Value>> = std::sync::OnceLock::new();
    MANIFEST
        .get_or_init(|| crate::runs::read_json(&crate::runs_analysis::fable_manifest_path()))
        .as_ref()
}

/// `1/28 of it` for a large ratio, `64% of it` for a small one.
fn fraction(part: f64, whole: f64) -> String {
    let ratio = whole / part;
    if ratio >= 1.5 {
        format!("1/{:.0}", ratio)
    } else {
        format!("{:.0}%", part / whole * 100.0)
    }
}

fn seconds_text(seconds: f64) -> String {
    duration((seconds * 1000.0).round() as u64)
}

/// A run's Microcoder record, when it is one a claim may be about: graded,
/// and not a directory two runs wrote.
fn claimable(run: &Run) -> Option<&Microcoder> {
    let m = run.microcoder.as_deref()?;
    (run.agent == Agent::Microcoder
        && matches!(run.outcome, Outcome::Passed | Outcome::Failed)
        && !m.mixed)
        .then_some(m)
}

/// The provider and cost-basis label of a set of runs: `OpenRouter,
/// billed cost`, or each with its count when they differ.
fn providers_label(runs: &[&Run]) -> (String, Vec<Value>) {
    let mut counts: BTreeMap<(Option<String>, CostBasis), usize> = BTreeMap::new();
    for run in runs {
        let m = run.microcoder.as_deref().expect("a Microcoder run");
        *counts
            .entry((
                m.provider.clone(),
                m.cost_basis.unwrap_or(CostBasis::Unknown),
            ))
            .or_default() += 1;
    }
    let json = counts
        .iter()
        .map(|((provider, basis), n)| {
            json!({"provider": provider, "cost_basis": basis.word(), "runs": n})
        })
        .collect();
    let text = if counts.len() == 1 {
        let ((provider, basis), _) = counts.iter().next().expect("one");
        format!("{}, {}", provider_name(provider.as_deref()), basis.phrase())
    } else {
        counts
            .iter()
            .map(|((provider, basis), n)| {
                format!(
                    "{}, {} ({n} run{})",
                    provider_name(provider.as_deref()),
                    basis.phrase(),
                    if *n == 1 { "" } else { "s" }
                )
            })
            .collect::<Vec<_>>()
            .join("; ")
    };
    (text, json)
}

/// Claimable runs by task, then by model, knowledge-assisted, and sample.
type Tasks<'a> = BTreeMap<String, BTreeMap<(String, bool, Sample), Vec<&'a Run>>>;

/// The rule.
#[must_use]
pub fn beats_winner(inputs: &Inputs<'_>) -> Vec<Highlight> {
    let Some(manifest) = inputs.fable else {
        return Vec::new();
    };
    // Graded, claimable Microcoder runs by task, then by model and labels.
    let mut tasks: Tasks<'_> = BTreeMap::new();
    for run in inputs.runs {
        let Some(m) = claimable(run) else { continue };
        tasks
            .entry(run.task.clone())
            .or_default()
            .entry((
                run.model.clone().unwrap_or_else(|| "unknown".to_owned()),
                m.knowledge_assisted,
                m.sample.unwrap_or(Sample::NoKnowledge),
            ))
            .or_default()
            .push(run);
    }
    let mut claims = Vec::new();
    for (task, groups) in &tasks {
        let reference = reference_task(manifest, task);
        let (cheapest, fastest) = (reference.cheapest(), reference.fastest());
        if cheapest.is_none() && fastest.is_none() {
            continue;
        }
        for ((model, assisted, sample), graded) in groups {
            let passes: Vec<&Run> = graded
                .iter()
                .copied()
                .filter(|run| run.outcome == Outcome::Passed)
                .collect();
            let cheaper: Vec<(&Run, f64)> = cheapest
                .and_then(|w| w.cost_usd)
                .map(|limit| {
                    passes
                        .iter()
                        .filter_map(|run| run.cost_usd.filter(|c| *c < limit).map(|c| (*run, c)))
                        .collect()
                })
                .unwrap_or_default();
            let faster: Vec<(&Run, f64)> = fastest
                .and_then(|w| w.seconds)
                .map(|limit| {
                    passes
                        .iter()
                        .filter_map(|run| {
                            let seconds = run.agent_ms? as f64 / 1000.0;
                            (seconds < limit).then_some((*run, seconds))
                        })
                        .collect()
                })
                .unwrap_or_default();
            if cheaper.is_empty() && faster.is_empty() {
                continue;
            }
            claims.push(claim(
                inputs, task, model, *assisted, *sample, graded, &passes, &cheaper, &faster,
                &reference, &tasks,
            ));
        }
    }
    claims
}

#[allow(clippy::too_many_arguments)]
fn claim(
    inputs: &Inputs<'_>,
    task: &str,
    model: &str,
    assisted: bool,
    sample: Sample,
    graded: &[&Run],
    passes: &[&Run],
    cheaper: &[(&Run, f64)],
    faster: &[(&Run, f64)],
    reference: &ReferenceTask,
    tasks: &Tasks<'_>,
) -> Highlight {
    let cheapest = reference.cheapest();
    let fastest = reference.fastest();
    let (providers, providers_json) = providers_label(graded);
    let labels = format!(
        "{} · {} · {providers}",
        sample.label(),
        if assisted {
            "knowledge-assisted"
        } else {
            "not knowledge-assisted"
        },
    );
    let agent = format!("Microcoder · {}", crate::runs_transcript::model_name(model));
    let mut numbers = vec![
        Number::count("passes", passes.len()),
        Number::count("graded", graded.len()),
        Number::count("reference_attempts", reference.attempts),
        Number::count("reference_winning_runs", reference.winners.len()),
        Number::count("cheaper_passes", cheaper.len()),
        Number::count("faster_passes", faster.len()),
    ];
    let mut text = format!(
        "[{labels}] {agent} passed {task} in {} of {} graded runs.",
        passes.len(),
        graded.len()
    );
    if let Some(limit) = cheapest.and_then(|w| w.cost_usd) {
        numbers.push(Number::new("reference_cheapest_usd", limit, usd(limit)));
        if cheaper.is_empty() {
            text.push_str(&format!(
                " None of its passes cost less than {REFERENCE}'s cheapest winning run ({}).",
                usd(limit)
            ));
        } else {
            let low = cheaper.iter().map(|(_, c)| *c).fold(f64::MAX, f64::min);
            let high = cheaper.iter().map(|(_, c)| *c).fold(0.0, f64::max);
            numbers.push(Number::new("cheaper_min_usd", low, usd(low)));
            numbers.push(Number::new("cheaper_max_usd", high, usd(high)));
            numbers.push(Number::new(
                "cheaper_min_fraction",
                low / limit,
                fraction(low, limit),
            ));
            text.push_str(&format!(
                " {} cost less than {REFERENCE}'s cheapest winning run ({}): {}.",
                if passes.len() == 1 {
                    "Its one pass".to_owned()
                } else {
                    format!("{} of the {} passes", cheaper.len(), passes.len())
                },
                usd(limit),
                if cheaper.len() == 1 {
                    format!("{}, {} of it", usd(low), fraction(low, limit))
                } else {
                    format!(
                        "from {} to {}, {} to {} of it",
                        usd(low),
                        usd(high),
                        fraction(low, limit),
                        fraction(high, limit)
                    )
                }
            ));
        }
    }
    if let Some(limit) = fastest.and_then(|w| w.seconds) {
        numbers.push(Number::new(
            "reference_fastest_seconds",
            limit,
            seconds_text(limit),
        ));
        if faster.is_empty() {
            text.push_str(&format!(
                " None finished faster than its fastest winning run ({}).",
                seconds_text(limit)
            ));
        } else {
            let best = faster.iter().map(|(_, s)| *s).fold(f64::MAX, f64::min);
            numbers.push(Number::new("faster_min_seconds", best, seconds_text(best)));
            text.push_str(&format!(
                " {} finished faster than its fastest winning run ({}){}.",
                faster.len(),
                seconds_text(limit),
                if faster.len() == 1 {
                    format!(", in {}", seconds_text(best))
                } else {
                    format!(", the fastest in {}", seconds_text(best))
                }
            ));
        }
    }
    let unmeasured = passes.iter().filter(|run| run.cost_usd.is_none()).count();
    if unmeasured > 0 {
        numbers.push(Number::count("passes_without_cost", unmeasured));
        text.push_str(&format!(
            " {unmeasured} of the passes recorded no cost, so no cost claim covers them."
        ));
    }

    // Caveats, from the data.
    let mut caveats = Vec::new();
    let naming: BTreeSet<String> = graded
        .iter()
        .flat_map(|run| {
            run.microcoder
                .as_deref()
                .map(|m| {
                    m.entries
                        .iter()
                        .filter(|e| e.names_task)
                        .map(|e| e.id.clone())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        })
        .collect();
    match sample {
        Sample::InSample => caveats.push(format!(
            "In-sample: {} {} written from runs of {task}, so these passes show the entries work on a task they were written for, not that they generalize.",
            naming.iter().cloned().collect::<Vec<_>>().join(", "),
            if naming.len() == 1 { "was" } else { "were" },
        )),
        Sample::Unknown => caveats.push(
            "Sample unknown: some entries these runs used aren't in the checkout's knowledge/, so whether they were written from this task can't be checked.".to_owned(),
        ),
        _ => {}
    }
    caveats.push(format!(
        "Microcoder's time is its loop's own, from the first step to the finish, without starting the container or grading; {REFERENCE}'s is the public trial's wall time, which includes both."
    ));
    caveats.push(format!(
        "Microcoder's cost is the model plus Jev and the knowledge base's embeddings; {REFERENCE}'s is the public record's reported cost."
    ));
    if graded.iter().any(|run| {
        run.microcoder
            .as_deref()
            .is_some_and(|m| m.cost_basis == Some(CostBasis::ListPrice))
    }) {
        caveats.push(
            "A list-price cost is GPT-6 Luna's list price for the tokens the Codex login reported; the login bills a subscription, so it isn't a bill.".to_owned(),
        );
    }
    if graded.iter().any(|run| {
        run.microcoder
            .as_deref()
            .is_some_and(|m| m.cost_basis == Some(CostBasis::Unknown))
    }) {
        caveats.push("Some of these runs' cost basis is unknown.".to_owned());
    }
    if graded.len() > passes.len() {
        caveats.push(format!(
            "{} of the {} graded runs with these labels failed.",
            graded.len() - passes.len(),
            graded.len()
        ));
    }
    if assisted && let Some(groups) = tasks.get(task) {
        let without: Vec<&Run> = groups
            .iter()
            .filter(|((m, a, _), _)| m == model && !a)
            .flat_map(|(_, runs)| runs.iter().copied())
            .collect();
        if !without.is_empty() {
            let passed = without
                .iter()
                .filter(|run| run.outcome == Outcome::Passed)
                .count();
            numbers.push(Number::count("passes_without_knowledge", passed));
            numbers.push(Number::count("graded_without_knowledge", without.len()));
            caveats.push(format!(
                "Without knowledge, {agent} passed {task} in {passed} of {} graded runs.",
                without.len()
            ));
        }
    }
    let left_out = inputs
        .runs
        .iter()
        .filter(|run| run.agent == Agent::Microcoder && run.task == task)
        .filter(|run| run.microcoder.as_deref().is_some_and(|m| m.mixed))
        .count();
    if left_out > 0 {
        caveats.push(if left_out == 1 {
            format!("1 run directory of {task} holds two runs' records and is left out.")
        } else {
            format!("{left_out} run directories of {task} hold two runs' records and are left out.")
        });
    }
    let sample_size = cheaper
        .iter()
        .chain(faster)
        .map(|(run, _)| run.id())
        .collect::<BTreeSet<_>>()
        .len();
    if sample_size == 1 {
        caveats.push("One pass: an anecdote, not a benchmark result.".to_owned());
    }
    let commits: BTreeSet<String> = passes
        .iter()
        .filter_map(|run| {
            let m = run.microcoder.as_deref()?;
            let commit = m.commit.as_deref()?;
            Some(if m.commit_source.as_deref() == Some("record") {
                commit.to_owned()
            } else {
                format!("{commit} (attributed)")
            })
        })
        .collect();
    if !commits.is_empty() {
        caveats.push(format!(
            "Commits of the passes: {}.",
            commits.into_iter().collect::<Vec<_>>().join(", ")
        ));
    }

    let runs: Vec<String> = passes.iter().map(|run| run.id()).collect();
    caveats.extend(mark_caveats(inputs.marks, &runs));
    let strength = {
        let cost = cheapest
            .and_then(|w| w.cost_usd)
            .zip(cheaper.iter().map(|(_, c)| *c).reduce(f64::min))
            .map_or(0.0, |(limit, low)| (limit / low).log2() / 5.0);
        let time = fastest
            .and_then(|w| w.seconds)
            .zip(faster.iter().map(|(_, s)| *s).reduce(f64::min))
            .map_or(0.0, |(limit, best)| (limit / best).log2());
        cost.max(time).clamp(0.0, 1.0)
    };
    let pass_json: Vec<Value> = passes
        .iter()
        .map(|run| {
            let m = run.microcoder.as_deref().expect("a Microcoder run");
            json!({
                "run": run.id(),
                "cost_usd": run.cost_usd,
                "cost_basis": m.cost_basis.map(CostBasis::word),
                "provider": m.provider,
                "seconds": run.agent_ms.map(|ms| ms as f64 / 1000.0),
                "cheaper": cheaper.iter().any(|(r, _)| r.id() == run.id()),
                "faster": faster.iter().any(|(r, _)| r.id() == run.id()),
                "retrieval": m.retrieval,
                "commit": m.commit,
                "commit_source": m.commit_source,
                "entries": m.entries.iter().map(|e| json!({
                    "id": e.id, "digest": e.digest, "version": e.version, "names_task": e.names_task,
                })).collect::<Vec<_>>(),
                "digests": m.digests,
            })
        })
        .collect();
    let winner = |w: &Winner| json!({"id": w.id, "cost_usd": w.cost_usd, "seconds": w.seconds});
    Highlight {
        key: key(
            Rule::BeatsWinner,
            &[task, model, &assisted.to_string(), sample.word()],
        ),
        rule: Rule::BeatsWinner,
        claim: text,
        task: Some(task.to_owned()),
        runs,
        numbers,
        sample: sample_size,
        n1: sample_size == 1,
        caveats,
        strength,
        score: 0.0,
        detail: Some(json!({
            "labels": {
                "text": labels,
                "sample": sample.word(),
                "in_sample": sample == Sample::InSample,
                "knowledge_assisted": assisted,
                "providers": providers_json,
                "entries_naming_task": naming,
            },
            "agent": agent,
            "reference": {
                "agent": "Claude Code",
                "model": REFERENCE_MODEL,
                "effort": REFERENCE_EFFORT,
                "attempts": reference.attempts,
                "winning": reference.winners.iter().map(winner).collect::<Vec<_>>(),
                "cheapest": cheapest.map(winner),
                "fastest": fastest.map(winner),
                "time_basis": "public trial wall time",
            },
            "time_basis": "Microcoder loop time",
            "passes": pass_json,
        })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runs_marks::Marks;
    use crate::runs_microcoder::{Knowledge, read_all};
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn runs() -> Vec<Run> {
        let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/microcoder");
        read_all(
            &[fixtures.join("runs")],
            &Knowledge::read(&fixtures.join("knowledge")),
            i64::MAX / 4,
        )
    }

    fn manifest() -> Value {
        let trial = |id: &str, effort: &str, reward: f64, cost: f64, start: &str, end: &str| {
            json!({"id": id, "task": "drift-check", "model": "Fable 5.1", "effort": effort,
                   "reward": reward, "cost_usd": cost, "started_at": start, "finished_at": end})
        };
        json!({"trials": [
            trial("w1", "low", 1.0, 0.74, "2026-09-17T05:00:00Z", "2026-09-17T05:03:43Z"),
            trial("w2", "low", 1.0, 0.92, "2026-09-17T05:36:29Z", "2026-09-17T05:39:00Z"),
            trial("l1", "low", 0.0, 0.10, "2026-09-17T05:00:00Z", "2026-09-17T05:01:00Z"),
            // A cheaper max-effort win doesn't move the low reference.
            trial("m1", "max", 1.0, 0.01, "2026-09-17T05:00:00Z", "2026-09-17T05:00:30Z"),
        ]})
    }

    fn claims(runs: &[Run], fable: Option<&Value>) -> Vec<Highlight> {
        let answers = HashMap::new();
        let marks = Marks::open(None);
        beats_winner(&Inputs {
            runs,
            answers: &answers,
            reference: None,
            marks: &marks,
            fable,
        })
    }

    #[test]
    fn a_cheaper_pass_is_claimed_with_its_labels() {
        let runs = runs();
        let manifest = manifest();
        let claims = claims(&runs, Some(&manifest));
        // drift-check in-sample and knowledge-assisted; sound-shift has no
        // reference, and the mixed directory is left out.
        assert_eq!(claims.len(), 1, "{claims:#?}");
        let claim = &claims[0];
        assert_eq!(claim.rule, Rule::BeatsWinner);
        assert!(
            claim.claim.starts_with("[in-sample · knowledge-assisted · the Codex login, list-price cost (1 run); OpenRouter, billed cost (3 runs)] Microcoder · GPT-6 Luna passed drift-check in 3 of 4 graded runs."),
            "{}",
            claim.claim
        );
        assert!(
            claim.claim.contains("3 of the 3 passes cost less than Fable 5.1 low's cheapest winning run ($0.74): from $0.0165 to $0.0491, 1/45 to 1/15 of it."),
            "{}",
            claim.claim
        );
        // 141.5 s against the fastest winning run's 151 s.
        assert!(
            claim
                .claim
                .contains("1 finished faster than its fastest winning run (2m 31s), in 2m 21s."),
            "{}",
            claim.claim
        );
        let detail = claim.detail.as_ref().unwrap();
        assert_eq!(detail["labels"]["in_sample"], true);
        assert_eq!(detail["labels"]["knowledge_assisted"], true);
        assert_eq!(detail["reference"]["cheapest"]["id"], "w1");
        assert_eq!(detail["reference"]["winning"].as_array().unwrap().len(), 2);
        assert_eq!(detail["passes"].as_array().unwrap().len(), 3);
        assert!(claim.caveats.iter().any(|c| {
            c.starts_with("In-sample: statistics.mmd was written from runs of drift-check")
        }));
        assert!(claim.caveats.iter().any(|c| c.contains("list price")));
        assert!(
            claim
                .caveats
                .iter()
                .any(|c| c == "1 of the 4 graded runs with these labels failed.")
        );
        assert!(
            claim
                .caveats
                .iter()
                .any(|c| c.contains("holds two runs' records"))
        );
        assert!(claim.caveats.iter().any(|c| c.contains("loop's own")));
        let json = claim.to_json();
        assert_eq!(json["rule"], "beats-winner");
        assert_eq!(json["detail"]["labels"]["sample"], "in_sample");
    }

    #[test]
    fn no_manifest_or_no_win_makes_no_claim() {
        let runs = runs();
        assert!(claims(&runs, None).is_empty());
        let losing = json!({"trials": [
            {"id": "x", "task": "drift-check", "model": "Fable 5.1", "effort": "low", "reward": 1.0,
             "cost_usd": 0.001, "started_at": "2026-09-17T05:00:00Z", "finished_at": "2026-09-17T05:00:10Z"}
        ]});
        assert!(claims(&runs, Some(&losing)).is_empty());
    }

    #[test]
    fn reference_reads_low_winners_only() {
        let reference = reference_task(&manifest(), "drift-check");
        assert_eq!(reference.attempts, 3);
        assert_eq!(reference.winners.len(), 2);
        assert_eq!(reference.cheapest().unwrap().id, "w1");
        assert_eq!(reference.fastest().unwrap().seconds, Some(151.0));
    }
}
