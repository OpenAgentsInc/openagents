//! Jev's judgments on a targeted experiment's trials.
//!
//! [`learn`] asks about finished trials. It runs `gym runs rank`'s question
//! set ([`crate::runs_learning`], 18 yes-or-no judgments and a learning
//! Score) on each trial that has no answer for its current evidence, and
//! groups the judgments by arm. It also asks three experiment questions,
//! each only where it applies: whether escalation changed the candidate,
//! whether the effort level plausibly decided the outcome, and whether a
//! failure was a near miss. Every answer is kept under the digest of its
//! state and questions, so a trial is asked once, and the pass reports
//! what it cost.
//!
//! [`advise`] asks, advisory only, about each running trial's live tail
//! ([`crate::coder_live`]): whether the agent is looping, stalled on
//! transport, or done but still spending. It never stops anything; only the
//! early-stopping rule in code ([`crate::terminal_bench_stop`]) stops an
//! arm, and only when the scheduler applies it.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::time::Instant;

use jev::{Entry, Noul, NoulCriteria, Questions, SystemOneRequest};
use serde_json::{Value, json};

use crate::runs_learning::{self, Judge, Recorded};
use crate::terminal_bench_pulse::{Pulse, TrialFacts};

/// The experiment questions' version. Change it with any change to a
/// question's wording.
pub const EXPERIMENT_SET: &str = "experiment-pulse-v1";

/// The live questions' version.
pub const LIVE_SET: &str = "experiment-live-v1";

/// A judgment at or above this probability counts as yes.
pub const YES_AT: f64 = 0.5;

/// How many requests run at once.
const CONCURRENCY: usize = 8;

/// The most recent live events a running trial's state carries.
const LIVE_EVENTS: usize = 30;

/// One experiment question.
#[derive(Clone, Copy, Debug)]
pub struct Question {
    pub id: &'static str,
    pub tag: &'static str,
    pub ask: &'static str,
    pub yes: &'static str,
    pub no: &'static str,
}

/// The questions about a finished trial.
pub const EXPERIMENT_QUESTIONS: [Question; 3] = [
    Question {
        id: "escalation_changed",
        tag: "escalation changed the candidate",
        ask: "Did escalating to a second executor, see `coder_one.second`, change the final candidate in a way that bears on the verifier's result in `trial`, rather than leaving the outcome where the first candidate had it?",
        yes: "The second executor's candidate was kept, and it plausibly changed which of the verifier's tests pass, for better or worse.",
        no: "The first candidate was kept, or the second one changed nothing the verifier's tests look at.",
    },
    Question {
        id: "effort_mattered",
        tag: "the effort level plausibly mattered",
        ask: "Did the reasoning effort this trial ran at, see `coder_one.effort`, plausibly decide its outcome, given how the arms that ran other effort levels did on the same task in `task_results`?",
        yes: "Another effort level would likely have changed the outcome: for example, the trial failed at a low effort on a task a higher-effort arm passed, or passed because the effort was high enough.",
        no: "The outcome would likely be the same at another effort level: the arms did about the same on this task whatever their effort, or the result has nothing to do with how hard the executor reasoned.",
    },
    Question {
        id: "near_miss",
        tag: "a near miss",
        ask: "Did this trial fail by a small margin: most of the verifier's tests in `trial.verifier_tests` passed, and the failing tests in `trial.failing_tests` point at one or two details a small change would fix?",
        yes: "It failed, and the failures come down to a detail or two, such as a formula, a unit, an off-by-one, or a missed field, while the rest of the work was right.",
        no: "It failed broadly: most tests failed, or the main result was missing or wrong.",
    },
];

/// The advisory questions about a running trial.
pub const LIVE_QUESTIONS: [Question; 3] = [
    Question {
        id: "looping",
        tag: "looping",
        ask: "Is the agent repeating the same commands, edits, or failed steps without making progress, as `recent_events` shows?",
        yes: "The recent events go around in circles: the same command, the same error, or the same fix again and again.",
        no: "Each recent step moves the work on, or there are too few events to tell.",
    },
    Question {
        id: "stalled_transport",
        tag: "stalled on transport",
        ask: "Is the trial stuck waiting on the network, a provider, or the host's copy of its log rather than working: no new event for a long time (`live.since_last_event_sec`), repeated connection, timeout, or rate-limit errors in `recent_events`, or a log copy that stopped polling (`live.since_last_poll_sec`)?",
        yes: "Nothing is happening because a connection, a provider, or the log copy is stuck.",
        no: "The agent is working, or it is quiet for a reason that isn't transport, such as a long build or test run.",
    },
    Question {
        id: "done_but_spending",
        tag: "done but still spending",
        ask: "Does the work look finished, for example the agent already reported the task done and its checks passed in `recent_events`, while the trial keeps running and spending (`live.spend_usd`)?",
        yes: "The agent looks done, but the trial keeps going and the spend keeps rising.",
        no: "The work isn't done yet, or the trial is wrapping up normally.",
    },
];

/// Where the experiment answers live: `~/.openagents/gym/pulse`.
#[must_use]
pub fn default_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(|home| PathBuf::from(home).join(".openagents/gym/pulse"))
}

fn questions(set: &[&Question]) -> Questions {
    let mut questions = Questions::new();
    for question in set {
        questions = questions.with(
            question.id,
            Noul::with_criteria(
                question.ask,
                NoulCriteria::new()
                    .when_true(question.yes)
                    .when_false(question.no),
            ),
        );
    }
    questions
}

/// The answer key: the digest of the set's name, the state, and the
/// questions exactly as the request carries them.
fn key(set: &str, state: &Value, questions: &Questions) -> String {
    atif::digest(&json!({
        "set": set,
        "state": state,
        "questions": serde_json::to_value(questions).unwrap_or(Value::Null),
    }))
}

/// One request: its key, its state, and its questions.
struct Ask {
    key: String,
    state: Value,
    questions: Questions,
}

/// What asking came back with.
#[derive(Default)]
struct Asked {
    answers: HashMap<String, Value>,
    asked: usize,
    input_tokens: u64,
    errors: Vec<String>,
}

/// Asks every request, `CONCURRENCY` at a time, or looks each up in a
/// recorded file.
fn ask_all(judge: &Judge, asks: Vec<Ask>) -> Asked {
    let mut asked = Asked::default();
    if asks.is_empty() {
        return asked;
    }
    match judge {
        Judge::Off(why) => {
            asked
                .errors
                .push(format!("{} questions wait for Jev: {why}", asks.len()));
        }
        Judge::Recorded(recorded) => {
            for ask in asks {
                asked.asked += 1;
                match recorded.entries.get(&ask.key) {
                    Some(answers) => {
                        asked.answers.insert(ask.key, answers.clone());
                    }
                    None => asked.errors.push(format!(
                        "no recorded answer for {}",
                        &ask.key[..12.min(ask.key.len())]
                    )),
                }
            }
        }
        Judge::Live(client) => {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    asked
                        .errors
                        .push(format!("cannot start a runtime: {error}"));
                    return asked;
                }
            };
            runtime.block_on(async {
                let mut pending: std::collections::VecDeque<Ask> = asks.into();
                let mut set = tokio::task::JoinSet::new();
                loop {
                    while set.len() < CONCURRENCY
                        && let Some(ask) = pending.pop_front()
                    {
                        let client = client.clone();
                        asked.asked += 1;
                        set.spawn(async move {
                            let started = Instant::now();
                            let request =
                                SystemOneRequest::new(Entry::from(ask.state), ask.questions);
                            let result = client.system_one(request).await;
                            (ask.key, result, started.elapsed())
                        });
                    }
                    let Some(joined) = set.join_next().await else {
                        break;
                    };
                    let Ok((key, result, _elapsed)) = joined else {
                        asked.errors.push("a request task stopped".to_owned());
                        continue;
                    };
                    match result {
                        Ok(response) => {
                            asked.input_tokens += response.usage.input_tokens.unwrap_or(0);
                            let answers = serde_json::from_str::<Value>(&response.raw().text())
                                .ok()
                                .and_then(|body| body.get("answers").cloned())
                                .unwrap_or(Value::Null);
                            asked.answers.insert(key, answers);
                        }
                        Err(error) => {
                            if asked.errors.len() < 5 {
                                let text = error.to_string();
                                asked
                                    .errors
                                    .push(text.lines().next().unwrap_or_default().to_owned());
                            }
                        }
                    }
                }
            });
        }
    }
    asked
}

fn noul(answers: &Value, id: &str) -> Option<f64> {
    answers
        .pointer(&format!("/{id}/noul"))
        .and_then(Value::as_f64)
}

/// The compact composition a trial's state carries.
fn coder_one(record: &Value) -> Value {
    let second = &record["second"];
    json!({
        "effort": record["effort"].is_object().then(|| json!({
            "score": record["effort"]["score"],
            "threshold": record["effort"]["at"],
            "ran_at": record["effort"]["effort"],
            "base": record["effort"]["base"],
            "raised": record["effort"]["raised"],
            "reason": record["effort"]["reason"],
        })),
        "second": second.is_object().then(|| json!({
            "ran": second["skipped"].is_null(),
            "skipped_because": second["skipped"],
            "fired": second["fired"],
            "trigger": second["trigger"],
            "kept": second["kept"],
            "why": second["why"],
            "first_candidate_checks": second["first"],
            "second_candidate_checks": second["second"],
            "cost_usd": second["cost_usd"],
        })),
        "final_checks": {
            "verdicts": record["final_checks"]["verdicts"],
            "requirements": record["final_checks"]["requirements"],
        },
        "support": record["support"],
        "repair": record["repair"].is_object().then(|| json!({
            "ran": record["repair"]["ran"],
            "changed": record["repair"]["changed"],
            "skipped": record["repair"]["skipped"],
        })),
        "persist": record["persist"].is_object().then(|| json!({
            "rounds": record["persist"]["rounds"].as_array().map(Vec::len),
            "stopped": record["persist"]["stopped"],
            "totals": record["persist"]["totals"],
        })),
    })
}

/// A finished trial's state for the experiment questions, and the
/// questions that apply to it.
fn experiment_ask(pulse: &Pulse, trial: &TrialFacts) -> Option<(Value, Vec<&'static Question>)> {
    if !trial.graded() {
        return None;
    }
    let record = trial.composition.as_ref();
    let mut applies: Vec<&'static Question> = Vec::new();
    for question in &EXPERIMENT_QUESTIONS {
        let yes = match question.id {
            "escalation_changed" => {
                record.is_some_and(|r| r["second"].is_object() && r["second"]["skipped"].is_null())
            }
            "effort_mattered" => record.is_some_and(|r| r["effort"]["score"].is_number()),
            "near_miss" => !trial.passed(),
            _ => false,
        };
        if yes {
            applies.push(question);
        }
    }
    if applies.is_empty() {
        return None;
    }
    let task_results: Vec<Value> = pulse
        .report
        .task_cells(&trial.task)
        .iter()
        .zip(&pulse.report.arms)
        .map(|((passes, graded), arm)| {
            let effort = pulse
                .trials
                .iter()
                .filter(|t| &t.arm == arm && t.task == trial.task && t.graded())
                .filter_map(|t| t.composition.as_ref()?["effort"]["effort"].as_str())
                .collect::<Vec<_>>();
            json!({"arm": arm, "passes": passes, "graded": graded, "effort_levels": effort})
        })
        .collect();
    let state = json!({
        "experiment": {"id": pulse.report.id, "arms": pulse.report.arms},
        "trial": {
            "arm": trial.arm,
            "task": trial.task,
            "attempt": trial.attempt,
            "passed": trial.passed(),
            "reward": trial.reward,
            "verifier_tests": trial.tests.map(|(passed, total)| json!({"passed": passed, "total": total})),
            "failing_tests": trial.failing_tests.iter().take(12).collect::<Vec<_>>(),
            "cost_usd": trial.cost_usd,
        },
        "task_results": task_results,
        "coder_one": record.map(coder_one),
    });
    Some((state, applies))
}

/// One arm's `runs-learning-v1` judgments.
#[derive(Clone, Debug, PartialEq)]
pub struct ArmJudgments {
    pub arm: String,
    pub answered: usize,
    /// Per judgment: its id, tag, how many trials were yes, and the mean
    /// probability, in the question set's order.
    pub judgments: Vec<(&'static str, &'static str, usize, f64)>,
    /// Per experiment question: its id, tag, trials it applied to, and
    /// how many were yes.
    pub experiment: Vec<(&'static str, &'static str, usize, usize)>,
}

/// What [`learn`] found.
#[derive(Clone, Debug)]
pub struct Learning {
    pub judge: &'static str,
    pub rank: runs_learning::Report,
    /// Finished trials `gym runs` found for the experiment.
    pub runs: usize,
    pub arms: Vec<ArmJudgments>,
    /// Experiment-question requests sent, answered, and cached.
    pub asked: usize,
    pub answered: usize,
    pub cached: usize,
    pub input_tokens: u64,
    pub errors: Vec<String>,
    /// Per trial: the job and its experiment answers.
    pub per_trial: Vec<(String, BTreeMap<String, f64>)>,
}

impl Learning {
    /// Requests sent to Jev in this pass.
    #[must_use]
    pub fn asked(&self) -> usize {
        self.rank.asked + self.asked
    }

    /// What this pass cost at Jev's published input rate.
    #[must_use]
    pub fn cost_usd(&self) -> f64 {
        self.rank.cost_usd
            + self.input_tokens as f64 * runs_learning::USD_PER_MILLION_INPUT / 1_000_000.0
    }

    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "judge": self.judge,
            "questions": [runs_learning::QUESTION_SET, EXPERIMENT_SET],
            "runs": self.runs,
            "rank": self.rank,
            "experiment_questions": {
                "asked": self.asked,
                "answered": self.answered,
                "cached": self.cached,
                "input_tokens": self.input_tokens,
            },
            "cost_usd": self.cost_usd(),
            "usd_per_million_input": runs_learning::USD_PER_MILLION_INPUT,
            "errors": self.errors,
            "by_arm": self.arms.iter().map(|a| json!({
                "arm": a.arm,
                "answered": a.answered,
                "judgments": a.judgments.iter().map(|(id, tag, yes, mean)| json!({
                    "id": id, "tag": tag, "yes": yes, "mean_p": mean,
                })).collect::<Vec<_>>(),
                "experiment": a.experiment.iter().map(|(id, tag, applied, yes)| json!({
                    "id": id, "tag": tag, "applied": applied, "yes": yes,
                })).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
            "trials": self.per_trial.iter().map(|(job, answers)| json!({"job": job, "answers": answers})).collect::<Vec<_>>(),
        })
    }

    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        let mut lines = vec![format!(
            "Jev on finished trials ({}): {} runs; runs-learning-v1 asked {}, {} answered, {} cached; experiment questions asked {}, {} answered, {} cached; cost ${:.4}",
            self.judge,
            self.runs,
            self.rank.asked,
            self.rank.answered,
            self.rank.cached,
            self.asked,
            self.answered,
            self.cached,
            self.cost_usd()
        )];
        for error in self.rank.errors.iter().chain(&self.errors).take(5) {
            lines.push(format!("  ! {error}"));
        }
        for arm in &self.arms {
            lines.push(format!("  {} · {} trials judged", arm.arm, arm.answered));
            let mut top: Vec<&(&str, &str, usize, f64)> =
                arm.judgments.iter().filter(|j| j.2 > 0).collect();
            top.sort_by(|a, b| b.2.cmp(&a.2).then(b.3.total_cmp(&a.3)));
            for (_, tag, yes, mean) in top.into_iter().take(6) {
                lines.push(format!(
                    "    {yes:>3} of {:<3} {tag} (mean p {mean:.2})",
                    arm.answered
                ));
            }
            for (_, tag, applied, yes) in arm.experiment.iter().filter(|e| e.2 > 0) {
                lines.push(format!("    {yes:>3} of {applied:<3} {tag}"));
            }
        }
        lines
    }
}

/// Reads a pulse store; a missing file is empty.
fn open_store(dir: Option<&Path>) -> Recorded {
    dir.map(|dir| dir.join("answers.json"))
        .filter(|path| path.is_file())
        .and_then(|path| Recorded::load(&path).ok())
        .unwrap_or_else(Recorded::empty)
}

/// Asks Jev about the experiment's finished trials that have no answer
/// yet: `gym runs rank`'s set and the experiment questions.
///
/// # Errors
/// When the pulse store can't be written.
pub fn learn(
    pulse: &Pulse,
    judge: &Judge,
    jobs: &Path,
    reference: bool,
    dir: Option<PathBuf>,
) -> Result<Learning, String> {
    // Every run on the host, so each trial's state can say how other runs
    // of the task went.
    let sources = crate::runs::Sources {
        jobs: Some(jobs.to_path_buf()),
        ..crate::runs::Sources::standard()
    };
    learn_with(
        pulse,
        judge,
        sources,
        reference,
        dir,
        runs_learning::default_dir(),
    )
}

/// [`learn`] with the runs `gym runs` reads and the runs-learning store's
/// directory named.
///
/// # Errors
/// When the pulse store can't be written.
pub fn learn_with(
    pulse: &Pulse,
    judge: &Judge,
    sources: crate::runs::Sources,
    reference: bool,
    dir: Option<PathBuf>,
    learning_dir: Option<PathBuf>,
) -> Result<Learning, String> {
    let catalog = crate::runs::Catalog::load(sources);
    let context = runs_learning::Context::new(
        &catalog,
        reference
            .then(crate::terminal_bench_reference::Reference::checked)
            .flatten(),
    );
    let arm_of: HashMap<&str, &str> = pulse
        .trials
        .iter()
        .map(|t| (t.job.as_str(), t.arm.as_str()))
        .collect();
    let runs: Vec<crate::runs::Run> = catalog
        .runs
        .iter()
        .filter(|run| arm_of.contains_key(run.job.as_str()))
        .cloned()
        .collect();
    let mut store = runs_learning::Store::open(learning_dir);
    let rank = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("cannot start a runtime: {error}"))?
        .block_on(runs_learning::rank(
            &runs, &context, &mut store, judge, None, None,
        ));
    // The 18 judgments by arm.
    let mut arms: Vec<ArmJudgments> = pulse
        .report
        .arms
        .iter()
        .map(|arm| ArmJudgments {
            arm: arm.clone(),
            answered: 0,
            judgments: runs_learning::JUDGMENTS
                .iter()
                .map(|j| (j.id, j.tag, 0, 0.0))
                .collect(),
            experiment: EXPERIMENT_QUESTIONS
                .iter()
                .map(|q| (q.id, q.tag, 0, 0))
                .collect(),
        })
        .collect();
    for run in runs.iter().filter(|run| runs_learning::rankable(run)) {
        let Some(answer) = store.lookup(&runs_learning::fingerprint(run, &context)) else {
            continue;
        };
        let Some(arm) = arm_of
            .get(run.job.as_str())
            .and_then(|arm| arms.iter_mut().find(|a| a.arm == *arm))
        else {
            continue;
        };
        arm.answered += 1;
        for judgment in &mut arm.judgments {
            let p = answer.nouls.get(judgment.0).copied().unwrap_or(0.0);
            judgment.2 += usize::from(p >= YES_AT);
            judgment.3 += p;
        }
    }
    for arm in &mut arms {
        if arm.answered > 0 {
            for judgment in &mut arm.judgments {
                judgment.3 /= arm.answered as f64;
            }
        }
    }
    // The experiment questions, asked once per state.
    let mut stored = open_store(dir.as_deref());
    let mut asks = Vec::new();
    let mut keyed: Vec<(&TrialFacts, String, Vec<&'static Question>)> = Vec::new();
    let mut cached = 0;
    for trial in &pulse.trials {
        let Some((state, applies)) = experiment_ask(pulse, trial) else {
            continue;
        };
        let set = questions(&applies);
        let key = key(EXPERIMENT_SET, &state, &set);
        if stored.entries.contains_key(&key) {
            cached += 1;
        } else {
            asks.push(Ask {
                key: key.clone(),
                state,
                questions: set,
            });
        }
        keyed.push((trial, key, applies));
    }
    let asked = ask_all(judge, asks);
    let answered = asked.answers.len();
    for (key, answers) in &asked.answers {
        stored.entries.insert(key.clone(), answers.clone());
    }
    if answered > 0
        && let Some(dir) = &dir
    {
        std::fs::create_dir_all(dir)
            .map_err(|error| format!("cannot create {}: {error}", dir.display()))?;
        stored.save(&dir.join("answers.json"))?;
    }
    let mut per_trial = Vec::new();
    for (trial, key, applies) in keyed {
        let Some(answers) = stored.entries.get(&key) else {
            continue;
        };
        let mut found = BTreeMap::new();
        for question in applies {
            if let Some(p) = noul(answers, question.id) {
                found.insert(question.id.to_owned(), p);
                if let Some(arm) = arms.iter_mut().find(|a| a.arm == trial.arm)
                    && let Some(entry) = arm.experiment.iter_mut().find(|e| e.0 == question.id)
                {
                    entry.2 += 1;
                    entry.3 += usize::from(p >= YES_AT);
                }
            }
        }
        per_trial.push((trial.job.clone(), found));
    }
    Ok(Learning {
        judge: judge.word(),
        rank,
        runs: runs.len(),
        arms,
        asked: asked.asked,
        answered,
        cached,
        input_tokens: asked.input_tokens,
        errors: asked.errors,
        per_trial,
    })
}

/// One running trial's advice.
#[derive(Clone, Debug, PartialEq)]
pub struct Advice {
    pub job: String,
    pub arm: String,
    /// What code sees without a model: a stale log, a stopped copy.
    pub mechanical: Vec<String>,
    /// Each live question's probability of yes.
    pub answers: BTreeMap<String, f64>,
    /// Why a trial wasn't judged.
    pub note: Option<String>,
}

/// What [`advise`] found.
#[derive(Clone, Debug, PartialEq)]
pub struct Advisory {
    pub judge: &'static str,
    pub trials: Vec<Advice>,
    pub asked: usize,
    pub input_tokens: u64,
    pub errors: Vec<String>,
}

impl Advisory {
    #[must_use]
    pub fn cost_usd(&self) -> f64 {
        self.input_tokens as f64 * runs_learning::USD_PER_MILLION_INPUT / 1_000_000.0
    }

    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "judge": self.judge,
            "questions": LIVE_SET,
            "advisory": "nothing is stopped by these answers",
            "asked": self.asked,
            "input_tokens": self.input_tokens,
            "cost_usd": self.cost_usd(),
            "errors": self.errors,
            "trials": self.trials.iter().map(|t| json!({
                "job": t.job,
                "arm": t.arm,
                "mechanical": t.mechanical,
                "answers": t.answers,
                "flags": t.answers.iter().filter(|(_, p)| **p >= YES_AT).map(|(id, _)| id).collect::<Vec<_>>(),
                "note": t.note,
            })).collect::<Vec<_>>(),
        })
    }

    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        let mut lines = vec![format!(
            "Running trials, advisory only; nothing is stopped ({}): {} judged, cost ${:.4}",
            self.judge,
            self.asked,
            self.cost_usd()
        )];
        if self.trials.is_empty() {
            lines.push("  no trial is running".to_owned());
        }
        for error in self.errors.iter().take(3) {
            lines.push(format!("  ! {error}"));
        }
        for trial in &self.trials {
            let flags: Vec<String> = LIVE_QUESTIONS
                .iter()
                .filter_map(|q| {
                    let p = *trial.answers.get(q.id)?;
                    (p >= YES_AT).then(|| format!("{} (p {p:.2})", q.tag))
                })
                .collect();
            let said = if let Some(note) = &trial.note {
                note.clone()
            } else if flags.is_empty() {
                "no flag".to_owned()
            } else {
                format!("FLAG {}", flags.join(", "))
            };
            lines.push(format!("  {}: {said}", trial.job));
            for mechanical in &trial.mechanical {
                lines.push(format!("    {mechanical}"));
            }
        }
        lines
    }
}

fn minutes(ms: u64) -> String {
    format!("{}m{:02}s", ms / 60_000, (ms % 60_000) / 1000)
}

/// Asks Jev, advisory only, about each running trial's live tail.
///
/// # Errors
/// Never today; the signature leaves room for a store.
pub fn advise(pulse: &Pulse, judge: &Judge) -> Result<Advisory, String> {
    let now = crate::coder_live::now_ms();
    let sources = crate::coder_live::Sources::default();
    let mut trials = Vec::new();
    let mut keys: Vec<Option<String>> = Vec::new();
    let mut asks = Vec::new();
    let set: Vec<&Question> = LIVE_QUESTIONS.iter().collect();
    for trial in pulse.trials.iter().filter(|t| t.state == "running") {
        let mut advice = Advice {
            job: trial.job.clone(),
            arm: trial.arm.clone(),
            mechanical: Vec::new(),
            answers: BTreeMap::new(),
            note: None,
        };
        let live = trial
            .trial_dir
            .as_deref()
            .and_then(|dir| crate::coder_live::read_trial(dir, now, &sources));
        let Some(live) = live else {
            advice.note = Some(
                "no live tail to read: not a Coder One trial, or its copy hasn't started"
                    .to_owned(),
            );
            trials.push(advice);
            keys.push(None);
            continue;
        };
        let age = live.age_ms(now);
        let poll = live.poll_age_ms(now);
        if live.state == crate::coder_live::State::Stale {
            advice.mechanical.push(format!(
                "no event for {}",
                age.map_or_else(|| "a while".to_owned(), minutes)
            ));
        }
        if let Some(poll) = poll.filter(|ms| *ms > 10 * 60_000) {
            advice
                .mechanical
                .push(format!("the log copy last polled {} ago", minutes(poll)));
        }
        let events: Vec<Value> = live
            .timeline
            .as_ref()
            .map(|timeline| {
                let skip = timeline.events.len().saturating_sub(LIVE_EVENTS);
                timeline
                    .events
                    .iter()
                    .skip(skip)
                    .map(|event| {
                        json!({
                            "offset_sec": event.offset_ms / 1000,
                            "kind": event.kind,
                            "summary": crate::runs::clip_words(&event.summary, 200),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let elapsed = trial
            .started_at
            .as_deref()
            .and_then(crate::terminal_bench::timestamp_ms)
            .and_then(|start| u64::try_from(start).ok())
            .map(|start| now.saturating_sub(start) / 1000);
        let state = json!({
            "trial": {
                "arm": trial.arm,
                "task": trial.task,
                "attempt": trial.attempt,
                "elapsed_sec": elapsed,
            },
            "live": {
                "state": live.state.word(),
                "since_last_event_sec": age.map(|ms| ms / 1000),
                "since_last_poll_sec": poll.map(|ms| ms / 1000),
                "tail_state": live.tail.as_ref().and_then(|tail| tail["state"].as_str()),
                "current_component": live.current().map(|entry| entry.component.clone()),
                "spend_usd": live.spend(),
            },
            "recent_events": events,
        });
        let questions = questions(&set);
        let key = key(LIVE_SET, &state, &questions);
        asks.push(Ask {
            key: key.clone(),
            state,
            questions,
        });
        keys.push(Some(key));
        trials.push(advice);
    }
    let asked = ask_all(judge, asks);
    for (advice, key) in trials.iter_mut().zip(keys) {
        let Some(key) = key else {
            continue;
        };
        match asked.answers.get(&key) {
            Some(answers) => {
                for question in &LIVE_QUESTIONS {
                    if let Some(p) = noul(answers, question.id) {
                        advice.answers.insert(question.id.to_owned(), p);
                    }
                }
            }
            None => advice.note = Some("Jev didn't answer".to_owned()),
        }
    }
    Ok(Advisory {
        judge: judge.word(),
        trials,
        asked: asked.asked,
        input_tokens: asked.input_tokens,
        errors: asked.errors,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal_bench_pulse::fixture;

    #[test]
    fn experiment_questions_are_asked_once_where_they_apply_and_grouped_by_arm() {
        let temp = tempfile::tempdir().unwrap();
        let (experiments, jobs) = fixture::write_experiment(temp.path());
        let pulse = Pulse::load(&experiments.join("x/status.json"), Some(&jobs), None).unwrap();
        // Recorded answers: yes to every question that applies.
        let mut recorded = Recorded::empty();
        let mut applied = Vec::new();
        for trial in &pulse.trials {
            if let Some((state, applies)) = experiment_ask(&pulse, trial) {
                let answers: serde_json::Map<String, Value> = applies
                    .iter()
                    .map(|q| (q.id.to_owned(), json!({"noul": 0.9})))
                    .collect();
                applied.push((
                    trial.job.clone(),
                    applies.iter().map(|q| q.id).collect::<Vec<_>>(),
                ));
                recorded.entries.insert(
                    key(EXPERIMENT_SET, &state, &questions(&applies)),
                    Value::Object(answers),
                );
            }
        }
        assert_eq!(
            applied,
            vec![
                ("tb4--cand--t1--x-r1".to_owned(), vec!["effort_mattered"]),
                ("tb4--base--t1--x-r2".to_owned(), vec!["near_miss"]),
                (
                    "tb4--cand--t1--x-r2".to_owned(),
                    vec!["escalation_changed", "effort_mattered", "near_miss"]
                ),
            ]
        );
        let sources = || crate::runs::Sources {
            jobs: Some(jobs.clone()),
            traces: None,
            tasks: Vec::new(),
            index: None,
        };
        let pulse_dir = temp.path().join("pulse");
        let learning_dir = temp.path().join("learning");
        let first = learn_with(
            &pulse,
            &Judge::Recorded(recorded),
            sources(),
            false,
            Some(pulse_dir.clone()),
            Some(learning_dir.clone()),
        )
        .unwrap();
        assert_eq!((first.asked, first.answered, first.cached), (3, 3, 0));
        let cand = first.arms.iter().find(|a| a.arm == "cand").unwrap();
        let effort = cand
            .experiment
            .iter()
            .find(|e| e.0 == "effort_mattered")
            .unwrap();
        assert_eq!((effort.2, effort.3), (2, 2));
        assert!(
            first
                .lines()
                .iter()
                .any(|l| l.contains("2 of 2   the effort level plausibly mattered"))
        );
        // The second pass reads the kept answers and asks nothing.
        let again = learn_with(
            &pulse,
            &Judge::Off("off".to_owned()),
            sources(),
            false,
            Some(pulse_dir),
            Some(learning_dir),
        )
        .unwrap();
        assert_eq!((again.asked, again.cached), (0, 3));
        assert_eq!(again.per_trial.len(), 3);
        assert_eq!(again.to_json()["experiment_questions"]["cached"], 3);
    }

    #[test]
    fn a_running_trial_without_a_live_tail_is_noted_not_judged() {
        let temp = tempfile::tempdir().unwrap();
        let (experiments, jobs) = fixture::write_experiment(temp.path());
        let pulse = Pulse::load(&experiments.join("x/status.json"), Some(&jobs), None).unwrap();
        let advisory = advise(&pulse, &Judge::Off("off".to_owned())).unwrap();
        assert_eq!(advisory.trials.len(), 1);
        assert_eq!(advisory.asked, 0);
        assert!(
            advisory.trials[0]
                .note
                .as_deref()
                .unwrap()
                .contains("no live tail")
        );
        assert!(advisory.lines()[0].contains("advisory only; nothing is stopped"));
    }
}
