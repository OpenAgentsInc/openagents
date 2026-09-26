//! Which runs are most worth learning from, as Jev judges them.
//!
//! Reading every run is slow, and most runs repeat what others already
//! show. This module asks Jev, once per finished run, whether the run's
//! records show something worth a person's time: a failure a small change
//! would fix, an agent or harness doing what it shouldn't, evidence against
//! a design choice, or an outcome nobody expected. Each is its own typed
//! judgment, so a ranked run says why it ranks.
//!
//! - [`evidence`] builds one run's state for Jev: the task, the outcome and
//!   the failing tests, the story [`crate::runs_story`] tells, Coder One's
//!   composition and checks, the transcript's activity, the leaderboard's
//!   pass rate on the task, and this host's other runs of it. The same
//!   records always build the same state, clipped to [`STATE_LIMIT`]. No
//!   test content goes in beyond what the verifier printed.
//! - [`questions`] is the question set, versioned as [`QUESTION_SET`] and
//!   digested by [`questions_digest`].
//! - [`Store`] keeps each answer under the digest of its state and question
//!   set, in `~/.openagents/gym/learning/`. A run is asked again only when
//!   its evidence or the questions change.
//! - [`rank`] asks about every finished run that has no answer yet;
//!   `gym runs rank` is its command line, and the Runs pane runs it in the
//!   background when the reader switches to the learning order.
//!
//! Code owns the order: [`Answer::learning`] combines the judgments by a
//! fixed rule, and the probabilities stay in the record so another rule
//! needs no new request.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::time::Instant;

use jev::{Entry, Noul, NoulCriteria, Questions, Score, SystemOneRequest};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::runs::{Agent, Catalog, Outcome, Run, clip_words, duration, read_json};
use crate::runs_story::{self, Detail};
use crate::runs_transcript::{Kind, Transcript};
use crate::terminal_bench_reference::Reference;

/// The question set's version. Change it with any change to a question's
/// wording, so old answers are never read as answers to new questions.
pub const QUESTION_SET: &str = "runs-learning-v1";

/// The evidence builder's version. Change it with any change to what
/// [`evidence`] puts in the state, so the fingerprint index stops pointing
/// runs at answers to the old state.
pub const EVIDENCE_VERSION: &str = "runs-learning-evidence-v2";

/// The Jev version the rankings pin, the same one Coder One pins.
pub const JEV_MODEL: &str = "jev-1.13.0";

/// The TypeSafe door Jev answers on.
pub const JEV_BASE_URL: &str = "https://api.typesafe.ai";

/// Jev's published rate for `jev-1.13.0`, in dollars per million input
/// tokens, retrieved 2026-09-22; Coder One prices its requests the same way.
pub const USD_PER_MILLION_INPUT: f64 = 0.042;

/// The most characters a run's state takes. Longer text is clipped, the
/// least important parts first.
pub const STATE_LIMIT: usize = 6_000;

/// A judgment at or above this probability is a reason the list shows.
pub const REASON_AT: f64 = 0.5;

/// How many requests run at once.
pub const CONCURRENCY: usize = 8;

const ANSWER_SCHEMA: &str = "openagents.gym.runs-learning.answer.v1";
const INDEX_SCHEMA: &str = "openagents.gym.runs-learning.index.v1";

/// The schema of a recorded-answer file.
pub const RECORDED_SCHEMA: &str = "openagents.gym.runs-learning.recorded.v1";

/// The kind of lesson a judgment looks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Category {
    /// A failure a small change would likely fix.
    Fruit,
    /// The agent or the harness did something it shouldn't.
    Misbehavior,
    /// The trace contradicts something the design relies on.
    Hypothesis,
    /// An outcome nobody expected.
    Surprise,
}

impl Category {
    /// The category's name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Category::Fruit => "low-hanging fruit",
            Category::Misbehavior => "flagrant misbehavior",
            Category::Hypothesis => "evidence against a design choice",
            Category::Surprise => "surprise",
        }
    }

    /// Every category, in the order a summary lists them.
    pub const ALL: [Category; 4] = [
        Category::Fruit,
        Category::Misbehavior,
        Category::Hypothesis,
        Category::Surprise,
    ];
}

/// One yes-or-no judgment the question set asks.
#[derive(Clone, Copy, Debug)]
pub struct Judgment {
    /// The question's ID in the request.
    pub id: &'static str,
    pub category: Category,
    /// The short tag the list shows when the answer is yes.
    pub tag: &'static str,
    pub ask: &'static str,
    pub yes: &'static str,
    pub no: &'static str,
}

/// The judgments, grouped by category. The hypotheses are the ones
/// `docs/terminal-bench/2026-09-23-what-we-have-learned.md` and
/// `docs/optimization/coder-components.md` rest on.
pub const JUDGMENTS: [Judgment; 18] = [
    Judgment {
        id: "near_miss",
        category: Category::Fruit,
        tag: "near miss",
        ask: "Did the run described in `run` fail by a small margin: most of the verifier's tests passed, and the failing tests in `run.failing_tests` point at one or two specific details a small change would fix?",
        yes: "It failed, and the failures come down to a detail or two, such as a formula, a unit, an off-by-one, or a missed field, while the rest of the work was right.",
        no: "It passed, wasn't graded, or failed broadly: most tests failed, or the main result was missing or wrong.",
    },
    Judgment {
        id: "output_slip",
        category: Category::Fruit,
        tag: "output slip",
        ask: "Did the run fail, or come close to failing, because of a wrong output path, file name, or format, or a file the task asked for that was missing, rather than because the work itself was wrong?",
        yes: "The work was largely right, but it was written to the wrong place, under the wrong name, in the wrong format, or not written at all.",
        no: "Outputs were where and what the task asked for, or the run failed for reasons unrelated to its outputs.",
    },
    Judgment {
        id: "missed_check",
        category: Category::Fruit,
        tag: "check should have fired",
        ask: "Could a simple check of the outputs and behaviors the task states in `task.instruction` have caught this run's failure before the agent stopped, while the agent's own checks, if it ran any, passed it, didn't run, or looked at the wrong thing?",
        yes: "The run failed, and a direct look at a stated output or behavior would have shown the problem, but no check the agent ran flagged it.",
        no: "The run passed, wasn't graded, or its failure is too subtle for a simple check of stated outputs to see.",
    },
    Judgment {
        id: "ignored_task",
        category: Category::Misbehavior,
        tag: "ignored the task",
        ask: "Did the agent do something other than what the task in `task.instruction` asked, or skip a requirement the task stated plainly?",
        yes: "The agent solved a different problem, changed the wrong thing, or left out something the task explicitly required.",
        no: "The agent worked on what the task asked, whether or not it succeeded.",
    },
    Judgment {
        id: "looped",
        category: Category::Misbehavior,
        tag: "looped",
        ask: "Did the agent repeat the same commands, edits, or sessions without making progress, as `activity` and `story` show?",
        yes: "The agent went around in circles: the same command or fix again and again, or rounds that changed nothing.",
        no: "Each step moved the work on, or the record shows too little activity to tell.",
    },
    Judgment {
        id: "stopped_early",
        category: Category::Misbehavior,
        tag: "stopped early",
        ask: "Did the agent stop while its result was still wrong and with most of its time limit unused, see `run.share_of_time_limit_used`, when more work could plausibly have fixed it?",
        yes: "It finished failing with most of its budget left, and the failure looks fixable with more effort.",
        no: "It passed, used most of its time, ran out of time, or failed in a way more time wouldn't fix.",
    },
    Judgment {
        id: "unearned_success",
        category: Category::Misbehavior,
        tag: "claimed unearned success",
        ask: "Did the agent report the task done or working when the verifier found it failing, or when its own evidence doesn't support the claim?",
        yes: "The agent's report says the work is complete or correct, and the verifier's result or the agent's own checks say otherwise.",
        no: "The agent's report matches the outcome, or it admitted what was left undone.",
    },
    Judgment {
        id: "needless_repair",
        category: Category::Misbehavior,
        tag: "repaired correct work",
        ask: "Did Coder One, see `coder_one`, run a repair on work that was already correct, or repair a problem that its own checks invented?",
        yes: "A repair ran although the work was already right, for example because a check looked at the wrong path or misread the task.",
        no: "No repair ran, or the repair addressed a real problem.",
    },
    Judgment {
        id: "wasted_rounds",
        category: Category::Misbehavior,
        tag: "extra rounds for nothing",
        ask: "Did Coder One escalate, start a second executor, or run persistence rounds, see `coder_one`, that changed nothing that mattered to the outcome?",
        yes: "Extra sessions ran and spent time or money, but the checks and the result came out the same as before them.",
        no: "No extra sessions ran, or they changed the outcome or the tests for the better.",
    },
    Judgment {
        id: "wasted_money",
        category: Category::Misbehavior,
        tag: "wasted money",
        ask: "Did the run spend far more than the task needed, compared with the other runs of the same task in `other_runs` and the task's difficulty, for no better result?",
        yes: "It cost several times what comparable runs cost and did no better, or spent heavily on work that added nothing.",
        no: "Its cost was in line with the task and with other runs, or the extra spend bought a better result.",
    },
    Judgment {
        id: "harness_fault",
        category: Category::Misbehavior,
        tag: "harness fault",
        ask: "Did the harness, the infrastructure, or the environment, rather than the agent, decide how this run ended: a crash, a full disk, a provider usage limit, a container that failed to start, a missing result, or a grade the agent's work didn't earn?",
        yes: "Something outside the agent's work ended the run or decided its grade.",
        no: "The agent's own work decided the outcome.",
    },
    Judgment {
        id: "h_briefing",
        category: Category::Hypothesis,
        tag: "contradicts: the briefing gives the executor what it needs",
        ask: "Does this run contradict the design assumption that Coder One's briefing gives the executor the evidence it needs, for example the executor missed a requirement, a path, or a fact that the briefing should have carried?",
        yes: "The executor lacked, or had to rediscover, something the task stated or the workspace showed, and that gap shaped the outcome.",
        no: "The run isn't a Coder One run, or nothing in it points at a gap in the briefing.",
    },
    Judgment {
        id: "h_checks",
        category: Category::Hypothesis,
        tag: "contradicts: checks catch failures",
        ask: "Does this run contradict the design assumption that Coder One's own checks catch real failures and don't flag correct work: a check passed a result the verifier failed, or a check flagged work that was correct, for example by reading the wrong path?",
        yes: "Coder One's checks and the verifier disagree: the checks missed the failure, or raised a problem that wasn't one.",
        no: "The run isn't a Coder One run, it ran no checks, or its checks agreed with the verifier.",
    },
    Judgment {
        id: "h_effort",
        category: Category::Hypothesis,
        tag: "contradicts: effort and persistence help",
        ask: "Does this run contradict the assumption that more reasoning effort, more sessions, or persistence rounds make an agent pass more tasks: heavy effort or extra rounds that spent a lot and changed no test outcome, or a lean, cheap run that passed where heavier runs of the same task in `other_runs` failed?",
        yes: "Extra effort or rounds bought nothing here, or less effort did better than more.",
        no: "Effort and rounds weren't tested here, or more of them helped.",
    },
    Judgment {
        id: "h_routing",
        category: Category::Hypothesis,
        tag: "contradicts: routing picks well",
        ask: "Does this run contradict the assumption that Coder One's routing picks the right executor, model, and effort for the task: the route in `coder_one` looks wrong for this task, for example a strong, expensive tier on an easy task or a tier that other runs in `other_runs` show fails here?",
        yes: "The chosen executor or effort was a poor fit, and another choice would likely have done better or cost much less.",
        no: "The run isn't a Coder One run, or its route looks sound.",
    },
    Judgment {
        id: "h_jev",
        category: Category::Hypothesis,
        tag: "contradicts: Jev's judgments are right",
        ask: "Does this run contradict the assumption that Jev's judgments during the run are right: its difficulty estimate, its requirement support judgments, or its judgment that the work was done disagree with what actually happened?",
        yes: "A Jev judgment recorded in the run points one way, and the outcome shows the other.",
        no: "The run records no Jev judgments, or they agree with the outcome.",
    },
    Judgment {
        id: "h_controller",
        category: Category::Hypothesis,
        tag: "contradicts: the controller adds value",
        ask: "Does this run contradict the assumption that Coder One's controller, its briefing, checks, and repair around the executor, adds value over running the executor alone: a plain Claude Code or Codex run of the same task in `other_runs` passed where Coder One failed, or Coder One's extra steps changed nothing?",
        yes: "The controller's steps didn't help here, and running the executor alone did as well or better.",
        no: "The run isn't a Coder One run, or the controller's steps visibly helped.",
    },
    Judgment {
        id: "surprise",
        category: Category::Surprise,
        tag: "surprise",
        ask: "Is this run's outcome unexpected given the task's pass rate in `leaderboard` and the other runs of it in `other_runs`: a pass on a task other agents rarely solve, or a failure on a task they usually solve?",
        yes: "The outcome goes against what the leaderboard and this host's other runs of the task predict.",
        no: "The outcome is what the leaderboard and the other runs predict, or there is nothing to compare it with.",
    },
];

/// The Score question's ID.
pub const VALUE_ID: &str = "value";

const VALUE_ASK: &str = "How much would a person improving these coding agents learn from reading this run's trace, given everything in the state?";

const VALUE_LEVELS: [&str; 5] = [
    "Nothing new: a routine pass, a failure that repeats what other runs already show, or a run that never really started.",
    "A little: a detail worth a glance, but no clear change to the agent follows from it.",
    "Some: it points at a specific weakness or cost worth noting.",
    "A lot: it shows a concrete, fixable problem, or a clear counterexample to a design assumption.",
    "The most: a flagrant failure, or a result that should change what gets built next.",
];

/// The question set every run is asked.
#[must_use]
pub fn questions() -> Questions {
    let mut questions = Questions::new();
    for judgment in &JUDGMENTS {
        questions = questions.with(
            judgment.id,
            Noul::with_criteria(
                judgment.ask,
                NoulCriteria::new()
                    .when_true(judgment.yes)
                    .when_false(judgment.no),
            ),
        );
    }
    questions.with(
        VALUE_ID,
        Score::new(
            VALUE_ASK,
            VALUE_LEVELS
                .iter()
                .map(|level| Some(Entry::from(*level)))
                .collect(),
        ),
    )
}

/// The question set as the request body carries it.
#[must_use]
pub fn questions_body() -> Value {
    serde_json::to_value(questions()).unwrap_or(Value::Null)
}

/// The question set's digest.
#[must_use]
pub fn questions_digest() -> String {
    // Every run's fingerprint names it; the set is fixed, so it is taken
    // once.
    static DIGEST: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    DIGEST
        .get_or_init(|| {
            atif::digest(&json!({ "set": QUESTION_SET, "questions": questions_body() }))
        })
        .clone()
}

/// The answer key: the digest of the state and the question set, exactly
/// as the request carries them.
#[must_use]
pub fn key(state: &Value) -> String {
    atif::digest(&json!({ "state": state, "questions": questions_body() }))
}

/// A judgment by its ID.
#[must_use]
pub fn judgment(id: &str) -> Option<&'static Judgment> {
    JUDGMENTS.iter().find(|judgment| judgment.id == id)
}

// ---------------------------------------------------------------------------
// Evidence
// ---------------------------------------------------------------------------

/// What every run's evidence compares against: this host's other runs of
/// the same task, and the leaderboard.
#[derive(Clone, Debug, Default)]
pub struct Context {
    /// Per task: each graded run's id, agent label, and whether it passed.
    by_task: HashMap<String, Vec<(String, String, bool)>>,
    reference: Option<Reference>,
}

impl Context {
    /// The context of `catalog`'s runs and `reference`'s rows.
    #[must_use]
    pub fn new(catalog: &Catalog, reference: Option<Reference>) -> Self {
        let mut by_task: HashMap<String, Vec<(String, String, bool)>> = HashMap::new();
        for run in &catalog.runs {
            if matches!(run.outcome, Outcome::Passed | Outcome::Failed) {
                by_task.entry(run.task.clone()).or_default().push((
                    run.id(),
                    run.agent_label(),
                    run.outcome == Outcome::Passed,
                ));
            }
        }
        Context { by_task, reference }
    }

    /// This host's other graded runs of `run`'s task, by agent.
    #[must_use]
    pub fn others(&self, run: &Run) -> Value {
        let mut arms: BTreeMap<&str, (u64, u64)> = BTreeMap::new();
        for (id, label, passed) in self.by_task.get(&run.task).into_iter().flatten() {
            if *id == run.id() {
                continue;
            }
            let arm = arms.entry(label.as_str()).or_default();
            arm.0 += u64::from(*passed);
            arm.1 += 1;
        }
        let mut arms: Vec<(&str, (u64, u64))> = arms.into_iter().collect();
        // The arms with the most runs say the most.
        arms.sort_by(|a, b| b.1.1.cmp(&a.1.1).then(a.0.cmp(b.0)));
        Value::Array(
            arms.into_iter()
                .take(8)
                .map(|(label, (passed, runs))| {
                    json!({ "agent": label, "passed": passed, "runs": runs })
                })
                .collect(),
        )
    }

    /// The leaderboard's record on `task`, or null when it has none.
    #[must_use]
    pub fn leaderboard(&self, task: &str) -> Value {
        let Some(reference) = &self.reference else {
            return Value::Null;
        };
        let rows = reference.task(task);
        let trials: u64 = rows.iter().map(|(_, result)| result.trials).sum();
        if rows.is_empty() || trials == 0 {
            return Value::Null;
        }
        let passes: u64 = rows.iter().map(|(_, result)| result.successes).sum();
        let best = rows
            .iter()
            .filter(|(_, result)| result.trials > 0)
            .map(|(_, result)| result.successes as f64 / result.trials as f64)
            .fold(0.0_f64, f64::max);
        json!({
            "rows": rows.len(),
            "pass_rate": round2(passes as f64 / trials as f64),
            "rows_that_ever_pass": rows.iter().filter(|(_, result)| result.successes > 0).count(),
            "best_row_pass_rate": round2(best),
        })
    }
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

/// A run's evidence: the state Jev reads. Only a finished run has one.
#[must_use]
pub fn evidence(detail: &Detail, context: &Context) -> Value {
    let run = &detail.run;
    // A finished run's story reads the same whenever it is told.
    let now = run.ended_ms.or(run.started_ms).unwrap_or(0);
    let story: Map<String, Value> = runs_story::summary(detail, now)
        .into_iter()
        .filter(|paragraph| paragraph.heading != "What the task asked")
        .map(|paragraph| (paragraph.heading, Value::String(paragraph.text)))
        .collect();
    let mut state = json!({
        "task": {
            "name": run.task,
            "asks": run.ask,
            "instruction": detail.instruction.as_deref().map(|text| clip_words(&text.split_whitespace().collect::<Vec<_>>().join(" "), 700)),
            "category": run.category,
            "expert_hours": run.expert_hours,
            "time_limit": time_limit_sec(detail).map(|sec| duration((sec * 1000.0) as u64)),
        },
        "run": run_facts(detail),
        "story": story,
        "coder_one": coder_one(detail),
        "activity": activity(&detail.transcript),
        "leaderboard": context.leaderboard(&run.task),
        "other_runs": context.others(run),
    });
    fit(&mut state);
    state
}

fn time_limit_sec(detail: &Detail) -> Option<f64> {
    detail.run.time_limit_sec.or_else(|| {
        detail
            .composition
            .as_ref()?
            .pointer("/horizon/episode_deadline_sec")?
            .as_f64()
    })
}

fn run_facts(detail: &Detail) -> Value {
    let run = &detail.run;
    let limit = time_limit_sec(detail);
    let used = match (run.agent_ms, limit) {
        (Some(agent), Some(limit)) if limit > 0.0 => {
            Some(format!("{:.1}%", agent as f64 / (limit * 10.0)))
        }
        _ => None,
    };
    json!({
        "agent": run.agent_label(),
        "batch": run.batch,
        "outcome": run.outcome.word(),
        "why_not_graded": match &run.outcome {
            Outcome::NotGraded(why) => Some(why.clone()),
            _ => None,
        },
        "tests": run.tests.map(|t| format!("{} of {} passed", t.passed, t.total)),
        "failing_tests": detail.failures.iter().take(6).map(|failure| match &failure.why {
            Some(why) => format!("{}: {why}", failure.name),
            None => failure.name.clone(),
        }).collect::<Vec<_>>(),
        "cost_usd": run.cost_usd.map(round2),
        "agent_time": run.agent_ms.map(duration),
        "share_of_time_limit_used": used,
        "notes": run.notes,
    })
}

/// Coder One's own record of what it composed: the route, each session,
/// the checks and what they flagged, support, repair, and extra rounds.
fn coder_one(detail: &Detail) -> Value {
    if detail.run.agent != Agent::CoderOne {
        return Value::Null;
    }
    let Some(composition) = &detail.composition else {
        return Value::Null;
    };
    let tier = |tier: &Value| {
        let part = |key: &str| tier.get(key).and_then(Value::as_str).unwrap_or_default();
        format!("{} {} {}", part("agent"), part("model"), part("effort"))
            .trim()
            .to_owned()
    };
    let sessions: Vec<Value> = composition
        .get("branches")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|branch| {
            json!({
                "role": branch.get("role"),
                "executor": tier(branch.get("tier").unwrap_or(&Value::Null)),
                "minutes": branch.get("milliseconds").and_then(Value::as_u64).map(|ms| round2(ms as f64 / 60_000.0)),
                "usd": branch.get("usd").and_then(Value::as_f64).map(round2),
                "status": branch.get("status"),
            })
        })
        .collect();
    let difficulty = composition.pointer("/route/profile/difficulty").cloned();
    let repair = composition.get("repair").filter(|r| r.is_object()).map(|repair| {
        json!({
            "ran": repair.get("ran"),
            "changed": repair.get("changed"),
            "rechecked": repair.pointer("/recheck/summary/verdicts"),
            "said": repair.pointer("/session/result").and_then(Value::as_str).map(|text| clip_words(text, 300)),
        })
    });
    let rounds: Vec<Value> = composition
        .pointer("/persist/rounds")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|round| {
            json!({
                "usd": round.get("cost_usd").and_then(Value::as_f64).map(round2),
                "changed_files": round.get("changed"),
                "checks_before": round.get("before"),
                "checks_after": round.get("after"),
            })
        })
        .collect();
    json!({
        "route": {
            "judged_difficulty": difficulty,
            "reason": composition.pointer("/route/reason"),
        },
        "sessions": sessions,
        "checks": flagged_checks(detail),
        "final_checks": composition.pointer("/final_checks/verdicts"),
        "support": composition.get("support").map(|support| json!({
            "judged": support.get("judged"),
            "supported": support.get("supported"),
            "contradicted": support.get("contradicted"),
            "unresolved": support.get("unresolved"),
        })),
        "repair": repair,
        "second_executor": composition.pointer("/second/skipped").map(|why| json!({"skipped": why})).or_else(|| composition.get("second").filter(|s| s.is_object()).map(|_| json!("ran"))),
        "escalated": composition.get("escalated"),
        "persistence_rounds": rounds,
    })
}

/// Each check Coder One ran after the first session, and for every check
/// that didn't pass, what it expected, where it looked, and what it saw.
fn flagged_checks(detail: &Detail) -> Value {
    let Some(episode) = detail.run.files.episode.as_deref() else {
        return Value::Null;
    };
    let Some(checks) = read_json(&episode.join("verification/checks.json")) else {
        return Value::Null;
    };
    let scenarios: HashMap<&str, &Value> = checks
        .get("scenarios")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|s| Some((s.get("id")?.as_str()?, s)))
        .collect();
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut flagged = Vec::new();
    for verdict in checks
        .get("verdicts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let word = verdict
            .get("verdict")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        *counts.entry(word.to_owned()).or_default() += 1;
        if word == "passed" || flagged.len() >= 4 {
            continue;
        }
        let scenario = verdict
            .get("scenario")
            .and_then(Value::as_str)
            .and_then(|id| scenarios.get(id));
        let looked_at = scenario
            .and_then(|s| s.pointer("/params/resolved"))
            .and_then(Value::as_str);
        let task_says = scenario
            .and_then(|s| s.pointer("/spans/0/text"))
            .and_then(Value::as_str);
        let mut check = json!({
            "verdict": word,
            "expected": scenario.and_then(|s| s.pointer("/expected/statement")),
            "looked_at": looked_at,
            "task_says": task_says.map(|text| clip_words(text, 200)),
            "observed": verdict.get("observations").map(|o| clip_words(&o.to_string(), 200)),
        });
        if let (Some(looked_at), Some(task_says)) = (looked_at, task_says)
            && let Some(note) = wrong_place(looked_at, task_says, &reports(detail))
        {
            check["mismatch"] = Value::String(note);
        }
        flagged.push(check);
    }
    json!({ "verdicts": counts, "not_passed": flagged })
}

/// Every executor report the transcript kept, joined.
fn reports(detail: &Detail) -> String {
    detail
        .transcript
        .sessions
        .iter()
        .filter_map(|session| session.report.as_deref())
        .collect::<Vec<_>>()
        .join("\n")
}

/// When a check looked for a file somewhere other than where the task,
/// or the executor's own report, puts it: a fact code can see, stated
/// plainly so Jev needn't compare paths itself.
fn wrong_place(looked_at: &str, task_says: &str, reports: &str) -> Option<String> {
    let (dir, name) = looked_at.rsplit_once('/')?;
    let dir = if dir.is_empty() { "/" } else { dir };
    let named: Vec<String> = task_says
        .split(|c: char| c.is_whitespace() || c == '`' || c == '"' || c == '\'')
        .map(|word| word.trim_end_matches(['.', ',', ';', ':', ')']))
        .filter(|word| word.starts_with('/') && word.len() > 1)
        .map(|word| word.trim_end_matches('/').to_owned())
        .collect();
    let elsewhere = named.iter().find(|path| {
        path.as_str() != dir
            && path.as_str() != looked_at
            && !looked_at.starts_with(&format!("{path}/"))
    })?;
    let mut note = format!("The check looked in {dir}, but the task names {elsewhere}.");
    let written = reports
        .split(|c: char| c.is_whitespace() || c == '`' || c == '"' || c == '\'')
        .map(|word| word.trim_end_matches(['.', ',', ';', ':', ')']))
        .find(|word| {
            word.starts_with('/') && word.ends_with(&format!("/{name}")) && *word != looked_at
        });
    if let Some(written) = written {
        note.push_str(&format!(" The executor's report says it wrote {written}."));
    }
    Some(note)
}

/// What the transcript shows of the work: how many commands and edits,
/// how many failed, and the command repeated most.
fn activity(transcript: &Transcript) -> Value {
    let mut commands = 0;
    let mut failed = 0;
    let mut edits = 0;
    let mut repeats: HashMap<String, usize> = HashMap::new();
    for block in &transcript.blocks {
        match &block.kind {
            Kind::Command {
                command, failed: f, ..
            } => {
                commands += 1;
                failed += usize::from(*f);
                *repeats
                    .entry(command.trim().chars().take(160).collect())
                    .or_default() += 1;
            }
            Kind::Edit { .. } => edits += 1,
            _ => {}
        }
    }
    let most = repeats
        .into_iter()
        .filter(|(_, times)| *times > 1)
        .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)));
    json!({
        "sessions": transcript.sessions.len(),
        "commands": commands,
        "failed_commands": failed,
        "edits": edits,
        "most_repeated_command": most.map(|(command, times)| json!({"times": times, "command": clip_words(&command, 120)})),
        "monitor_looks_and_concerns": [transcript.monitor.0, transcript.monitor.1],
    })
}

/// Clips `state` until it fits [`STATE_LIMIT`]: long story paragraphs
/// first, then the check details, then the other runs.
fn fit(state: &mut Value) {
    let size = |state: &Value| state.to_string().chars().count();
    for limit in [900, 600, 400, 250] {
        if size(state) <= STATE_LIMIT {
            return;
        }
        if let Some(story) = state.get_mut("story").and_then(Value::as_object_mut) {
            for text in story.values_mut() {
                if let Some(s) = text.as_str() {
                    *text = Value::String(clip_words(s, limit));
                }
            }
        }
    }
    if size(state) > STATE_LIMIT
        && let Some(instruction) = state.pointer_mut("/task/instruction")
        && let Some(text) = instruction.as_str()
    {
        *instruction = Value::String(clip_words(text, 300));
    }
    if size(state) > STATE_LIMIT
        && let Some(said) = state.pointer_mut("/coder_one/repair/said")
    {
        *said = Value::Null;
    }
    if size(state) > STATE_LIMIT
        && let Some(list) = state
            .pointer_mut("/coder_one/checks/not_passed")
            .and_then(Value::as_array_mut)
    {
        list.truncate(1);
    }
    if size(state) > STATE_LIMIT
        && let Some(list) = state
            .pointer_mut("/run/failing_tests")
            .and_then(Value::as_array_mut)
    {
        list.truncate(3);
    }
    if size(state) > STATE_LIMIT
        && let Some(list) = state.get_mut("other_runs").and_then(Value::as_array_mut)
    {
        list.truncate(3);
    }
}

/// A cheap fingerprint of everything [`evidence`] reads: the run's own
/// facts, the size and time of its record files, the comparison context,
/// and both versions. Equal fingerprints mean the evidence needn't be
/// built again.
#[must_use]
pub fn fingerprint(run: &Run, context: &Context) -> String {
    let files = &run.files;
    let episode = files.episode.as_deref();
    let verifier = files.verifier.as_deref();
    let candidates: [(&str, Option<PathBuf>); 10] = [
        ("result", files.result.clone()),
        ("trajectory", files.trajectory.clone()),
        ("native", files.native.clone()),
        ("attempt", files.attempt.clone()),
        ("live", files.live.clone()),
        ("ctrf", verifier.map(|dir| dir.join("ctrf.json"))),
        ("stdout", verifier.map(|dir| dir.join("test-stdout.txt"))),
        ("manifest", episode.map(|dir| dir.join("manifest.json"))),
        (
            "composition",
            episode.map(|dir| dir.join("artifacts/composition.json")),
        ),
        ("log", episode.map(|dir| dir.join("episode.atif.jsonl"))),
    ];
    let stamps: Map<String, Value> = candidates
        .into_iter()
        .filter_map(|(name, path)| {
            let meta = std::fs::metadata(path?).ok()?;
            let modified = meta
                .modified()
                .ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |elapsed| elapsed.as_millis());
            Some((name.to_owned(), json!([meta.len(), modified.to_string()])))
        })
        .collect();
    atif::digest(&json!({
        "evidence": EVIDENCE_VERSION,
        "questions": questions_digest(),
        "run": run.id(),
        "outcome": run.outcome.word(),
        "reward": run.reward,
        "ended": run.ended_ms,
        "files": stamps,
        "others": context.others(run),
        "leaderboard": context.leaderboard(&run.task),
    }))
}

/// Whether a run can be ranked: it has finished.
#[must_use]
pub fn rankable(run: &Run) -> bool {
    // Microcoder's records aren't in the evidence state Jev reads yet, so
    // a judgment of one would rest on its outcome alone.
    run.outcome != Outcome::Running && run.agent != crate::runs::Agent::Microcoder
}

// ---------------------------------------------------------------------------
// Answers and the store
// ---------------------------------------------------------------------------

/// Jev's answer about one run, as the store keeps it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Answer {
    pub schema: String,
    /// The digest of the state and the question set.
    pub key: String,
    /// The run it was asked about, `job/trial`.
    pub run: String,
    pub questions: String,
    pub questions_digest: String,
    pub model: String,
    /// Each judgment's probability of yes, by ID.
    pub nouls: BTreeMap<String, f64>,
    /// The Score's probability-weighted level, from 0 to 4.
    pub value: Option<f64>,
    pub value_confidence: Option<f64>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub milliseconds: Option<u64>,
    /// `live`, or `recorded` for an answer replayed from a file.
    pub source: String,
    /// The state Jev read, so an answer can always be explained.
    pub state: Value,
}

impl Answer {
    /// Reads Jev's answers object into an answer.
    #[must_use]
    pub fn from_answers(run: &str, key: String, state: Value, answers: &Value) -> Self {
        let nouls = JUDGMENTS
            .iter()
            .filter_map(|judgment| {
                answers
                    .pointer(&format!("/{}/noul", judgment.id))
                    .and_then(Value::as_f64)
                    .map(|p| (judgment.id.to_owned(), p))
            })
            .collect();
        Answer {
            schema: ANSWER_SCHEMA.to_owned(),
            key,
            run: run.to_owned(),
            questions: QUESTION_SET.to_owned(),
            questions_digest: questions_digest(),
            model: JEV_MODEL.to_owned(),
            nouls,
            value: answers
                .pointer(&format!("/{VALUE_ID}/score"))
                .and_then(Value::as_f64),
            value_confidence: answers
                .pointer(&format!("/{VALUE_ID}/confidence"))
                .and_then(Value::as_f64),
            input_tokens: None,
            output_tokens: None,
            milliseconds: None,
            source: String::new(),
            state,
        }
    }

    /// The learning value the list orders by, from 0 to 1: half Jev's
    /// overall Score, half the strongest reason once each reason is
    /// weighed by how rare it is, so a lesson every failure shares doesn't
    /// outrank one only this run shows.
    #[must_use]
    pub fn learning(&self, rarity: &Rarity) -> f64 {
        let value = self
            .value
            .map_or(0.0, |value| (value / 4.0).clamp(0.0, 1.0));
        let strongest = self
            .nouls
            .iter()
            .map(|(id, p)| p * rarity.weight(id))
            .fold(0.0_f64, f64::max);
        0.5 * value + 0.5 * strongest
    }

    /// The reasons: every judgment at or above [`REASON_AT`], the most
    /// telling first, weighed by `rarity`.
    #[must_use]
    pub fn reasons(&self, rarity: &Rarity) -> Vec<(&'static Judgment, f64)> {
        let mut reasons: Vec<(&'static Judgment, f64)> = self
            .all()
            .into_iter()
            .filter(|(_, p)| *p >= REASON_AT)
            .collect();
        reasons.sort_by(|a, b| {
            (b.1 * rarity.weight(b.0.id)).total_cmp(&(a.1 * rarity.weight(a.0.id)))
        });
        reasons
    }

    /// Every judgment's probability, highest first; ties keep the set's
    /// order.
    #[must_use]
    pub fn all(&self) -> Vec<(&'static Judgment, f64)> {
        let mut all: Vec<(&'static Judgment, f64)> = JUDGMENTS
            .iter()
            .filter_map(|judgment| Some((judgment, *self.nouls.get(judgment.id)?)))
            .collect();
        all.sort_by(|a, b| b.1.total_cmp(&a.1));
        all
    }

    /// The top reasons as the list's short tags.
    #[must_use]
    pub fn tags(&self, rarity: &Rarity, most: usize) -> Vec<&'static str> {
        self.reasons(rarity)
            .into_iter()
            .take(most)
            .map(|(judgment, _)| judgment.tag)
            .collect()
    }

    /// A category's probability: the strongest of its judgments.
    #[must_use]
    pub fn category(&self, category: Category) -> f64 {
        JUDGMENTS
            .iter()
            .filter(|judgment| judgment.category == category)
            .filter_map(|judgment| self.nouls.get(judgment.id))
            .copied()
            .fold(0.0_f64, f64::max)
    }

    /// The answer as `gym runs --json` shows it.
    #[must_use]
    pub fn to_json(&self, rarity: &Rarity) -> Value {
        json!({
            "learning": round2(self.learning(rarity)),
            "value": self.value,
            "reasons": self.reasons(rarity).iter().map(|(j, p)| json!({
                "id": j.id,
                "tag": j.tag,
                "category": j.category.name(),
                "probability": p,
                "share_of_runs": round2(1.0 - rarity.weight(j.id)),
            })).collect::<Vec<_>>(),
            "categories": Category::ALL.iter().map(|c| (c.name().to_owned(), json!(round2(self.category(*c))))).collect::<Map<String, Value>>(),
            "judgments": self.nouls,
            "every_judgment": JUDGMENTS.iter().filter_map(|j| {
                let p = *self.nouls.get(j.id)?;
                Some(json!({
                    "id": j.id,
                    "tag": j.tag,
                    "category": j.category.name(),
                    "probability": p,
                    "reason": p >= REASON_AT,
                }))
            }).collect::<Vec<_>>(),
            "reason_at": REASON_AT,
            "questions": self.questions,
            "key": self.key,
        })
    }
}

/// How common each reason is across a set of answers: the share of them
/// that give it. A reason nearly every failure gives, such as an agent
/// reporting success it didn't earn, says less about one run than a
/// reason few runs give.
#[derive(Clone, Debug, Default)]
pub struct Rarity {
    share: HashMap<String, f64>,
}

impl Rarity {
    /// Every reason weighed alike.
    #[must_use]
    pub fn none() -> Self {
        Rarity::default()
    }

    /// The shares across `answers`.
    #[must_use]
    pub fn of<'a>(answers: impl IntoIterator<Item = &'a Answer>) -> Self {
        let mut counts: HashMap<String, usize> = HashMap::new();
        let mut total = 0usize;
        for answer in answers {
            total += 1;
            for (id, p) in &answer.nouls {
                if *p >= REASON_AT {
                    *counts.entry(id.clone()).or_default() += 1;
                }
            }
        }
        if total == 0 {
            return Rarity::none();
        }
        Rarity {
            share: counts
                .into_iter()
                .map(|(id, count)| (id, count as f64 / total as f64))
                .collect(),
        }
    }

    /// A reason's weight: one less the share of answers that give it.
    #[must_use]
    pub fn weight(&self, id: &str) -> f64 {
        1.0 - self.share.get(id).copied().unwrap_or(0.0)
    }
}

/// Where the rankings are kept: `~/.openagents/gym/learning`.
#[must_use]
pub fn default_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(|home| PathBuf::from(home).join(".openagents/gym/learning"))
}

/// The answers, keyed by the digest of state and questions, and an index
/// from each run's fingerprint to its answer's key.
#[derive(Clone, Debug, Default)]
pub struct Store {
    /// Where the store lives; `None` keeps it in memory.
    pub dir: Option<PathBuf>,
    index: BTreeMap<String, String>,
    answers: HashMap<String, Answer>,
}

#[derive(Serialize, Deserialize)]
struct Index {
    schema: String,
    fingerprints: BTreeMap<String, String>,
}

impl Store {
    /// Reads the store under `dir`; a missing directory is an empty store.
    #[must_use]
    pub fn open(dir: Option<PathBuf>) -> Self {
        let mut store = Store {
            dir,
            ..Store::default()
        };
        let Some(dir) = store.dir.clone() else {
            return store;
        };
        if let Some(index) = read_json(&dir.join("index.json"))
            .and_then(|value| serde_json::from_value::<Index>(value).ok())
            .filter(|index| index.schema == INDEX_SCHEMA)
        {
            store.index = index.fingerprints;
        }
        for entry in std::fs::read_dir(dir.join("answers"))
            .into_iter()
            .flatten()
            .flatten()
        {
            if let Some(answer) = read_json(&entry.path())
                .and_then(|value| serde_json::from_value::<Answer>(value).ok())
                .filter(|answer| answer.schema == ANSWER_SCHEMA)
            {
                store.answers.insert(answer.key.clone(), answer);
            }
        }
        store
    }

    /// The answer a run's fingerprint points at.
    #[must_use]
    pub fn lookup(&self, fingerprint: &str) -> Option<&Answer> {
        self.answers.get(self.index.get(fingerprint)?)
    }

    /// The answer under `key`.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Answer> {
        self.answers.get(key)
    }

    /// How many answers the store holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.answers.len()
    }

    /// Whether the store holds no answers.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.answers.is_empty()
    }

    /// Points `fingerprint` at `key`.
    pub fn point(&mut self, fingerprint: String, key: String) {
        self.index.insert(fingerprint, key);
    }

    /// Keeps `answer`, writing it to disk when the store has a directory.
    ///
    /// # Errors
    ///
    /// Returns a message when the file can't be written.
    pub fn insert(&mut self, answer: Answer) -> Result<(), String> {
        if let Some(dir) = &self.dir {
            let dir = dir.join("answers");
            std::fs::create_dir_all(&dir)
                .map_err(|error| format!("cannot create {}: {error}", dir.display()))?;
            let text = serde_json::to_string_pretty(&answer).map_err(|e| e.to_string())?;
            write_atomic(&dir.join(format!("{}.json", answer.key)), &text)?;
        }
        self.answers.insert(answer.key.clone(), answer);
        Ok(())
    }

    /// Writes the fingerprint index.
    ///
    /// # Errors
    ///
    /// Returns a message when the file can't be written.
    pub fn save_index(&self) -> Result<(), String> {
        let Some(dir) = &self.dir else {
            return Ok(());
        };
        std::fs::create_dir_all(dir)
            .map_err(|error| format!("cannot create {}: {error}", dir.display()))?;
        let index = Index {
            schema: INDEX_SCHEMA.to_owned(),
            fingerprints: self.index.clone(),
        };
        let text = serde_json::to_string_pretty(&index).map_err(|e| e.to_string())?;
        write_atomic(&dir.join("index.json"), &text)
    }
}

fn write_atomic(path: &Path, text: &str) -> Result<(), String> {
    let temporary = path.with_extension(format!("tmp{}", std::process::id()));
    std::fs::write(&temporary, format!("{text}\n"))
        .and_then(|()| std::fs::rename(&temporary, path))
        .map_err(|error| format!("cannot write {}: {error}", path.display()))
}

/// Every run's answer, when it has one, by run id.
#[must_use]
pub fn answers<'a>(
    catalog: &'a Catalog,
    store: &'a Store,
    context: &Context,
) -> HashMap<String, &'a Answer> {
    catalog
        .runs
        .iter()
        .filter(|run| rankable(run))
        .filter_map(|run| Some((run.id(), store.lookup(&fingerprint(run, context))?)))
        .collect()
}

/// `runs` in the learning order: ranked runs by learning value, highest
/// first, then the rest in the order given.
#[must_use]
pub fn order<'a>(
    runs: Vec<&'a Run>,
    answers: &HashMap<String, &Answer>,
    rarity: &Rarity,
) -> Vec<&'a Run> {
    let mut indexed: Vec<(usize, &Run, Option<f64>)> = runs
        .into_iter()
        .enumerate()
        .map(|(index, run)| {
            (
                index,
                run,
                answers.get(&run.id()).map(|a| a.learning(rarity)),
            )
        })
        .collect();
    indexed.sort_by(|a, b| match (a.2, b.2) {
        (Some(x), Some(y)) => y.total_cmp(&x).then(a.0.cmp(&b.0)),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.0.cmp(&b.0),
    });
    indexed.into_iter().map(|(_, run, _)| run).collect()
}

// ---------------------------------------------------------------------------
// Asking Jev
// ---------------------------------------------------------------------------

/// Recorded answers, keyed by [`key`].
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Recorded {
    pub schema: String,
    pub entries: BTreeMap<String, Value>,
}

impl Recorded {
    /// An empty set.
    #[must_use]
    pub fn empty() -> Self {
        Recorded {
            schema: RECORDED_SCHEMA.to_owned(),
            entries: BTreeMap::new(),
        }
    }

    /// Reads a recorded-answer file.
    ///
    /// # Errors
    ///
    /// Returns a message when the file doesn't read or has another schema.
    pub fn load(path: &Path) -> Result<Self, String> {
        let value =
            read_json(path).ok_or_else(|| format!("{} is not a JSON file", path.display()))?;
        let recorded: Recorded = serde_json::from_value(value)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        if recorded.schema != RECORDED_SCHEMA {
            return Err(format!(
                "{} is not a {RECORDED_SCHEMA} file",
                path.display()
            ));
        }
        Ok(recorded)
    }

    /// Writes the set, keys sorted.
    ///
    /// # Errors
    ///
    /// Returns a message when the file can't be written.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        write_atomic(path, &text)
    }
}

/// Where answers come from.
#[derive(Clone)]
pub enum Judge {
    /// The hosted service.
    Live(jev::Client),
    /// Answers recorded earlier, for tests and replays.
    Recorded(Recorded),
    /// No Jev, and why.
    Off(String),
}

impl Judge {
    /// Hosted Jev with the operator's TypeSafe key: `TYPESAFE_API_KEY`, or
    /// `api_key` in `~/.openagents/jev.json`. `GYM_JEV=off` turns it off.
    /// The key is never printed; a message names where it was looked for.
    #[must_use]
    pub fn from_environment() -> Self {
        if std::env::var("GYM_JEV").is_ok_and(|value| value.trim() == "off") {
            return Judge::Off("GYM_JEV=off turns Jev off".to_owned());
        }
        let key = std::env::var("TYPESAFE_API_KEY")
            .ok()
            .map(|key| key.trim().to_owned())
            .filter(|key| !key.is_empty())
            .or_else(|| {
                let path = PathBuf::from(std::env::var_os("HOME")?).join(".openagents/jev.json");
                read_json(&path)?
                    .get("api_key")?
                    .as_str()
                    .map(|key| key.trim().to_owned())
                    .filter(|key| !key.is_empty())
            });
        let Some(key) = key else {
            return Judge::Off(
                "no TypeSafe key: set TYPESAFE_API_KEY or put `api_key` in ~/.openagents/jev.json"
                    .to_owned(),
            );
        };
        match jev::Client::new(
            jev::Config::new()
                .api_key(key)
                .base_url(JEV_BASE_URL)
                .default_model(JEV_MODEL),
        ) {
            Ok(client) => Judge::Live(client),
            Err(error) => Judge::Off(format!("cannot build the Jev client: {error}")),
        }
    }

    /// Why Jev can't answer, or `None` when it can.
    #[must_use]
    pub fn unavailable(&self) -> Option<&str> {
        match self {
            Judge::Off(why) => Some(why),
            _ => None,
        }
    }

    /// The mode's word: `live`, `recorded`, or `off`.
    #[must_use]
    pub fn word(&self) -> &'static str {
        match self {
            Judge::Live(_) => "live",
            Judge::Recorded(_) => "recorded",
            Judge::Off(_) => "off",
        }
    }
}

/// What one ranking pass did.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Report {
    /// Finished runs considered.
    pub considered: usize,
    /// Runs whose answer was already in the store.
    pub cached: usize,
    /// Requests sent to Jev, or looked up in a recorded file.
    pub asked: usize,
    pub answered: usize,
    pub failed: usize,
    /// Runs still going, left for when they finish.
    pub running: usize,
    pub input_tokens: u64,
    pub output_tokens: u64,
    /// What the answered requests cost at [`USD_PER_MILLION_INPUT`].
    pub cost_usd: f64,
    /// The first few failures, in words.
    pub errors: Vec<String>,
}

/// Asks Jev about every finished run in `runs` that has no answer for its
/// current evidence, keeps the answers in `store`, and, when `record` is
/// given, adds each answer to it. `limit` bounds the requests.
pub async fn rank(
    runs: &[Run],
    context: &Context,
    store: &mut Store,
    judge: &Judge,
    limit: Option<usize>,
    mut record: Option<&mut Recorded>,
) -> Report {
    let mut report = Report::default();
    let mut todo: Vec<(String, String, String, Value)> = Vec::new();
    for run in runs {
        if !rankable(run) {
            report.running += 1;
            continue;
        }
        report.considered += 1;
        let print = fingerprint(run, context);
        if let Some(answer) = store.lookup(&print) {
            report.cached += 1;
            if let Some(record) = record.as_deref_mut() {
                record
                    .entries
                    .insert(answer.key.clone(), answers_of(answer));
            }
            continue;
        }
        let state = evidence(&Detail::load(run), context);
        let key = key(&state);
        // An answer to the state without its command text, when the full
        // state was refused, counts too.
        let found = [key.clone(), self::key(&without_commands(&state))]
            .into_iter()
            .find(|key| store.get(key).is_some());
        if let Some(found) = found {
            report.cached += 1;
            if let (Some(record), Some(answer)) = (record.as_deref_mut(), store.get(&found)) {
                record.entries.insert(found.clone(), answers_of(answer));
            }
            store.point(print, found);
            continue;
        }
        todo.push((run.id(), print, key, state));
    }
    if let Some(limit) = limit {
        todo.truncate(limit);
    }
    let fail = |report: &mut Report, message: String| {
        report.failed += 1;
        if report.errors.len() < 5 {
            // A refusal can carry a whole HTML page; its first line says
            // enough.
            let first = message.lines().next().unwrap_or_default();
            report.errors.push(clip_words(first, 200));
        }
    };
    match judge {
        Judge::Off(why) => {
            if !todo.is_empty() {
                fail(
                    &mut report,
                    format!("{} runs wait for Jev: {why}", todo.len()),
                );
                report.failed = todo.len();
            }
        }
        Judge::Recorded(recorded) => {
            for (run, print, key, state) in todo {
                report.asked += 1;
                match recorded.entries.get(&key) {
                    Some(answers) => {
                        let mut answer = Answer::from_answers(&run, key.clone(), state, answers);
                        answer.source = "recorded".to_owned();
                        if let Some(record) = record.as_deref_mut() {
                            record.entries.insert(key.clone(), answers.clone());
                        }
                        keep(store, &mut report, print, answer);
                    }
                    None => fail(
                        &mut report,
                        format!("{run}: no recorded answer for this evidence"),
                    ),
                }
            }
        }
        Judge::Live(client) => {
            let mut pending: std::collections::VecDeque<(String, String, String, Value)> =
                todo.into();
            let mut retried = std::collections::HashSet::new();
            let mut set = tokio::task::JoinSet::new();
            loop {
                while set.len() < CONCURRENCY
                    && let Some((run, print, key, state)) = pending.pop_front()
                {
                    let client = client.clone();
                    report.asked += 1;
                    set.spawn(async move {
                        let started = Instant::now();
                        let request =
                            SystemOneRequest::new(Entry::from(state.clone()), questions());
                        let result = client.system_one(request).await;
                        let ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
                        (run, print, key, state, result, ms)
                    });
                }
                let Some(joined) = set.join_next().await else {
                    break;
                };
                let Ok((run, print, key, state, result, ms)) = joined else {
                    fail(&mut report, "a request task stopped".to_owned());
                    continue;
                };
                match result {
                    Ok(response) => {
                        let answers = serde_json::from_str::<Value>(&response.raw().text())
                            .ok()
                            .and_then(|body| body.get("answers").cloned())
                            .unwrap_or(Value::Null);
                        let mut answer = Answer::from_answers(&run, key.clone(), state, &answers);
                        answer.model.clone_from(&response.model);
                        answer.input_tokens = response.usage.input_tokens;
                        answer.output_tokens = response.usage.output_tokens;
                        answer.milliseconds = Some(ms);
                        answer.source = "live".to_owned();
                        report.input_tokens += response.usage.input_tokens.unwrap_or(0);
                        report.output_tokens += response.usage.output_tokens.unwrap_or(0);
                        if let Some(record) = record.as_deref_mut() {
                            record.entries.insert(key, answers);
                        }
                        keep(store, &mut report, print, answer);
                    }
                    // The door's firewall can refuse a state for the shell
                    // text quoted in it; ask once more without it.
                    Err(jev::Error::Api(api))
                        if api.status == 403 && retried.insert(run.clone()) =>
                    {
                        let state = without_commands(&state);
                        pending.push_back((run, print, self::key(&state), state));
                    }
                    Err(error) => fail(&mut report, format!("{run}: {error}")),
                }
            }
        }
    }
    report.cost_usd = report.input_tokens as f64 * USD_PER_MILLION_INPUT / 1_000_000.0;
    if let Err(error) = store.save_index() {
        fail(&mut report, error);
    }
    report
}

/// `state` without the shell text it quotes: the most repeated command.
fn without_commands(state: &Value) -> Value {
    let mut state = state.clone();
    if let Some(command) = state.pointer_mut("/activity/most_repeated_command/command") {
        *command = Value::String("(left out)".to_owned());
    }
    state
}

fn keep(store: &mut Store, report: &mut Report, print: String, answer: Answer) {
    if answer.nouls.len() < JUDGMENTS.len() {
        report.failed += 1;
        if report.errors.len() < 5 {
            report
                .errors
                .push(format!("{}: the answer was missing judgments", answer.run));
        }
        return;
    }
    let key = answer.key.clone();
    match store.insert(answer) {
        Ok(()) => {
            report.answered += 1;
            store.point(print, key);
        }
        Err(error) => {
            report.failed += 1;
            report.errors.push(error);
        }
    }
}

/// The answers object a stored answer came from, for a recorded file.
fn answers_of(answer: &Answer) -> Value {
    let mut answers: Map<String, Value> = answer
        .nouls
        .iter()
        .map(|(id, p)| (id.clone(), json!({ "noul": p })))
        .collect();
    if let Some(value) = answer.value {
        answers.insert(
            VALUE_ID.to_owned(),
            json!({ "score": value, "confidence": answer.value_confidence }),
        );
    }
    Value::Object(answers)
}

/// Runs [`rank`] on a thread of its own, with a runtime of its own, and
/// sends the report when it is done. The store under `dir` is written as
/// answers arrive; the caller reads it again when the report comes.
pub fn spawn(
    runs: Vec<Run>,
    context: Context,
    judge: Judge,
    dir: Option<PathBuf>,
) -> std::sync::mpsc::Receiver<Report> {
    let (send, receive) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let report = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime.block_on(async {
                let mut store = Store::open(dir);
                rank(&runs, &context, &mut store, &judge, None, None).await
            }),
            Err(error) => Report {
                errors: vec![format!("cannot start a runtime: {error}")],
                ..Report::default()
            },
        };
        let _ = send.send(report);
    });
    receive
}

// ---------------------------------------------------------------------------
// Reading
// ---------------------------------------------------------------------------

/// The lines a run's summary shows under "Worth learning from": the
/// overall value, then every judgment's probability, highest first.
#[must_use]
pub fn summary_lines(answer: &Answer, rarity: &Rarity) -> Vec<String> {
    let mut lines = vec![format!(
        "Learning value {:.2} of 1{}.",
        answer.learning(rarity),
        answer
            .value
            .map(|value| format!(
                "; Jev's overall score {value:.1} of 4: {}",
                VALUE_LEVELS[(value.round() as usize).min(4)]
                    .split(':')
                    .next()
                    .unwrap_or_default()
                    .to_lowercase()
            ))
            .unwrap_or_default()
    )];
    for (judgment, p) in answer.all() {
        lines.push(format!(
            "{p:.2}  {}{}",
            judgment.tag,
            if matches!(judgment.category, Category::Hypothesis | Category::Surprise) {
                String::new()
            } else {
                format!(" ({})", judgment.category.name())
            }
        ));
    }
    lines
}

/// The report in words, for `gym runs rank`.
#[must_use]
pub fn report_lines(report: &Report, judge: &Judge) -> Vec<String> {
    let mut lines = vec![format!(
        "Ranked {} finished runs: asked Jev ({}) about {}, {} answered, {} failed; {} came from the cache.",
        report.considered,
        judge.word(),
        report.asked,
        report.answered,
        report.failed,
        report.cached
    )];
    if report.running > 0 {
        lines.push(format!(
            "Runs still running, to rank after they finish: {}.",
            report.running
        ));
    }
    lines.push(format!(
        "Cost: ${:.4} for {} input tokens at ${USD_PER_MILLION_INPUT} per million{}.",
        report.cost_usd,
        report.input_tokens,
        if report.asked > 0 && report.answered > 0 {
            format!(
                ", about ${:.5} a request",
                report.cost_usd / report.answered as f64
            )
        } else {
            String::new()
        }
    ));
    if let Some(why) = judge.unavailable() {
        lines.push(format!("Jev is off: {why}."));
    }
    for error in &report.errors {
        lines.push(format!("  {error}"));
    }
    lines
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// The recorded answers for the fixture runs, asked live once.
    pub(crate) fn recorded() -> Recorded {
        Recorded::load(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/runs-learning/recorded.json"),
        )
        .expect("the recorded answers")
    }

    fn block_on<F: std::future::Future>(future: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("a runtime")
            .block_on(future)
    }

    #[test]
    fn the_question_set_is_valid_and_versioned() {
        let questions = questions();
        questions.validate().expect("a valid question set");
        assert_eq!(questions.len(), JUDGMENTS.len() + 1);
        assert_eq!(questions_digest().len(), 64);
        for category in Category::ALL {
            assert!(
                JUDGMENTS.iter().any(|j| j.category == category),
                "{category:?}"
            );
        }
        // The tags the issue names read as written.
        for tag in [
            "near miss",
            "stopped early",
            "contradicts: checks catch failures",
        ] {
            assert!(JUDGMENTS.iter().any(|j| j.tag == tag), "{tag}");
        }
    }

    #[test]
    fn evidence_is_deterministic_bounded_and_pathless() {
        let (dir, sources) = crate::runs::fixture_sources();
        let catalog = Catalog::load(sources);
        let context = Context::new(&catalog, None);
        for run in catalog.runs.iter().filter(|run| rankable(run)) {
            let state = evidence(&Detail::load(run), &context);
            let text = state.to_string();
            assert!(text.chars().count() <= STATE_LIMIT + 200, "{}", run.task);
            assert!(!text.contains(&dir.path().display().to_string()), "{text}");
            assert_eq!(state, evidence(&Detail::load(run), &context));
        }
        let wal = catalog
            .runs
            .iter()
            .find(|run| run.task == "wal-recovery-ordering")
            .unwrap();
        let state = evidence(&Detail::load(wal), &context);
        assert_eq!(state["run"]["tests"], "95 of 97 passed");
        assert_eq!(state["run"]["failing_tests"].as_array().unwrap().len(), 2);
        assert!(
            state["story"]["Why it likely failed"].is_string(),
            "{state}"
        );
    }

    #[test]
    fn recorded_answers_rank_the_fixture_runs_and_a_second_pass_asks_nothing() {
        let (_dir, sources) = crate::runs::fixture_sources();
        let catalog = Catalog::load(sources);
        let context = Context::new(&catalog, None);
        let store_dir = tempfile::tempdir().unwrap();
        let judge = Judge::Recorded(recorded());
        let mut store = Store::open(Some(store_dir.path().to_path_buf()));
        let first = block_on(rank(
            &catalog.runs,
            &context,
            &mut store,
            &judge,
            None,
            None,
        ));
        assert_eq!(first.considered, 4, "{first:?}");
        assert_eq!(first.running, 1);
        assert_eq!(first.asked, 4, "{first:?}");
        assert_eq!(first.answered, 4, "{first:?}");
        assert_eq!(first.failed, 0, "{first:?}");

        // Unchanged evidence: nothing is asked, even from a store read
        // afresh from disk.
        let mut store = Store::open(Some(store_dir.path().to_path_buf()));
        let second = block_on(rank(
            &catalog.runs,
            &context,
            &mut store,
            &judge,
            None,
            None,
        ));
        assert_eq!(second.asked, 0, "{second:?}");
        assert_eq!(second.cached, 4);

        // The learning order puts ranked runs first and the running run
        // last.
        let answers = answers(&catalog, &store, &context);
        assert_eq!(answers.len(), 4);
        let rarity = Rarity::of(answers.values().copied());
        let ordered = order(catalog.runs.iter().collect(), &answers, &rarity);
        assert_eq!(ordered.last().unwrap().outcome, Outcome::Running);
        let values: Vec<f64> = ordered
            .iter()
            .filter_map(|run| answers.get(&run.id()).map(|a| a.learning(&rarity)))
            .collect();
        assert!(values.windows(2).all(|w| w[0] >= w[1]), "{values:?}");
        // The near miss is a reason on the 95-of-97 failure.
        let wal = catalog
            .runs
            .iter()
            .find(|run| run.task == "wal-recovery-ordering")
            .unwrap();
        let tags = answers[&wal.id()].tags(&rarity, 5);
        assert!(tags.contains(&"near miss"), "{tags:?}");
    }

    #[test]
    fn changed_evidence_is_asked_again_and_only_that_run() {
        let (dir, sources) = crate::runs::fixture_sources();
        let catalog = Catalog::load(sources.clone());
        let context = Context::new(&catalog, None);
        let mut store = Store::open(None);
        let judge = Judge::Recorded(recorded());
        block_on(rank(
            &catalog.runs,
            &context,
            &mut store,
            &judge,
            None,
            None,
        ));
        // The verifier's report changes under one run.
        let ctrf = std::fs::read_dir(
            dir.path()
                .join("jobs/tb4--claude-code-opus--wal-recovery-ordering"),
        )
        .unwrap()
        .flatten()
        .map(|entry| entry.path().join("verifier/ctrf.json"))
        .find(|path| path.is_file())
        .unwrap();
        let text = std::fs::read_to_string(&ctrf).unwrap();
        std::fs::write(
            &ctrf,
            text.replacen("test_p37_higher_lsn", "test_p38_higher_lsn", 1),
        )
        .unwrap();
        let catalog = Catalog::load(sources);
        let context = Context::new(&catalog, None);
        let report = block_on(rank(
            &catalog.runs,
            &context,
            &mut store,
            &judge,
            None,
            None,
        ));
        assert_eq!(report.cached, 3, "{report:?}");
        // Only the changed run is asked, and no answer was recorded for
        // its new evidence.
        assert_eq!(report.asked, 1, "{report:?}");
        assert_eq!(report.failed, 1, "{report:?}");
        assert!(
            report.errors[0].contains("wal-recovery-ordering"),
            "{report:?}"
        );
    }

    #[test]
    fn a_check_that_looked_in_the_wrong_place_says_so() {
        let note = wrong_place(
            "/app/TB3_Conf_Answers.csv",
            "Provide your answers as a CSV file named `TB3_Conf_Answers.csv` and save it inside `/results/`.",
            "I wrote `/results/TB3_Conf_Answers.csv` with the header `Answers`.",
        );
        assert_eq!(
            note.as_deref(),
            Some(
                "The check looked in /app, but the task names /results. The executor's report says it wrote /results/TB3_Conf_Answers.csv."
            )
        );
        assert_eq!(
            wrong_place("/results/out.csv", "Save it as `/results/out.csv`.", ""),
            None
        );
        assert_eq!(wrong_place("/app/out.csv", "Write out.csv.", ""), None);
    }

    #[test]
    fn without_jev_nothing_is_asked_and_the_report_says_why() {
        let (_dir, sources) = crate::runs::fixture_sources();
        let catalog = Catalog::load(sources);
        let context = Context::new(&catalog, None);
        let mut store = Store::open(None);
        let judge = Judge::Off("no TypeSafe key".to_owned());
        let report = block_on(rank(
            &catalog.runs,
            &context,
            &mut store,
            &judge,
            None,
            None,
        ));
        assert_eq!(report.asked, 0);
        assert_eq!(report.failed, 4);
        let lines = report_lines(&report, &judge).join("\n");
        assert!(lines.contains("Jev is off: no TypeSafe key"), "{lines}");
        assert!(lines.contains("Cost: $0.0000"), "{lines}");
    }

    #[test]
    fn an_answer_reads_as_a_value_reasons_and_tags() {
        let answers = json!({
            "near_miss": {"noul": 0.9},
            "stopped_early": {"noul": 0.7},
            "h_checks": {"noul": 0.6},
            "value": {"score": 3.0, "confidence": 0.8},
        });
        let mut all = answers.clone();
        for judgment in &JUDGMENTS {
            if all.get(judgment.id).is_none() {
                all[judgment.id] = json!({"noul": 0.1});
            }
        }
        let answer = Answer::from_answers("job/trial", "k".to_owned(), Value::Null, &all);
        let none = Rarity::none();
        assert!((answer.learning(&none) - (0.5 * 0.75 + 0.5 * 0.9)).abs() < 1e-9);
        assert_eq!(
            answer.tags(&none, 3),
            vec![
                "near miss",
                "stopped early",
                "contradicts: checks catch failures"
            ]
        );
        assert!((answer.category(Category::Fruit) - 0.9).abs() < 1e-9);
        // A reason every run gives says less than one few runs give.
        let mut common = answer.clone();
        common.nouls.insert("stopped_early".to_owned(), 0.1);
        common.nouls.insert("h_checks".to_owned(), 0.1);
        let rarity = Rarity::of([&answer, &common, &common, &common]);
        assert_eq!(
            answer.tags(&rarity, 3),
            vec![
                "stopped early",
                "contradicts: checks catch failures",
                "near miss"
            ]
        );
        assert!(answer.learning(&rarity) < answer.learning(&none));
        let lines = summary_lines(&answer, &none);
        assert!(
            lines[0].contains("Learning value 0.82 of 1; Jev's overall score 3.0 of 4: a lot."),
            "{lines:?}"
        );
        assert!(
            lines[1].starts_with("0.90  near miss (low-hanging fruit)"),
            "{lines:?}"
        );
        assert_eq!(answers_of(&answer)["near_miss"]["noul"], 0.9);
    }
}
