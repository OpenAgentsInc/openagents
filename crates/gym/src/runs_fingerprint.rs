//! A trajectory's strategy, as numbers: a fingerprint.
//!
//! [`crate::runs_phases`] places every step in a phase. A [`Fingerprint`]
//! summarizes the placed steps: how long the agent looked before its first
//! edit, how often it tested, where in the run it verified, how often it
//! repeated a failed step, how many files it touched, whether it ran the
//! task's own example before editing, and the phase sequence, compressed.
//!
//! Fingerprints cover local runs from the Runs catalog and the public
//! Fable 5.1 trajectories head-to-head replay reads, through the same
//! [`crate::runs_replay`] loader. `gym runs fingerprint RUN` prints one;
//! `gym runs fingerprints --task T` prints a task's, and
//! [`crate::runs_moves`] compares them.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;

use serde::Serialize;
use serde_json::{Value, json};

use crate::runs::{Agent, Catalog, Outcome, Sources, duration, money};
use crate::runs_learning::Judge;
use crate::runs_phases::{
    self, ActionKind, Labeling, Phase, QUESTION_SET, RULES_VERSION, Step, StepStore, YES,
};
use crate::runs_replay::{Replay, Source};

/// The moves report `gym runs moves` keeps, under the fingerprint
/// directory.
pub const MOVES_FILE: &str = "moves.json";

/// The fingerprint record's schema.
pub const SCHEMA: &str = "openagents.gym.runs-fingerprint.v1";

/// The Luna baseline subset: the 14 TB4 tasks of
/// `docs/terminal-bench/2026-09-24-luna-tb4-baseline.md` (#9583).
pub const LUNA_SUBSET: [&str; 14] = [
    "legacy-utility-triage",
    "mvcc-lsm-compaction",
    "heat-pump-warranty",
    "ks-solver-cpp",
    "wal-recovery-ordering",
    "cad-model",
    "nextjs-performance",
    "embedding-drift-monitor",
    "fin-saccr-rwa",
    "sound-change-cascade",
    "wdm-design",
    "shadow-relay",
    "uefi-bootkit",
    "coq-block-bound",
];

/// Which family of agent a trajectory belongs to: `fable`, `luna`,
/// `coder-one`, `claude-code`, `codex`, or `other`.
#[must_use]
pub fn group(source: &Source) -> &'static str {
    match source {
        Source::Public { .. } => "fable",
        Source::Local(run) => {
            let luna = [run.model.as_deref(), run.variant.as_deref(), Some(&run.job)]
                .into_iter()
                .flatten()
                .any(|text| text.to_lowercase().contains("luna"));
            if luna {
                "luna"
            } else {
                match run.agent {
                    Agent::CoderOne => "coder-one",
                    Agent::ClaudeCode => "claude-code",
                    Agent::Codex => "codex",
                    _ => "other",
                }
            }
        }
    }
}

/// The agent and its configuration, in words.
#[must_use]
pub fn arm(source: &Source) -> String {
    match source {
        Source::Public { trial, .. } => format!("Fable 5.1 {}", trial.effort),
        Source::Local(run) => run.agent_label(),
    }
}

/// Whether the verifier passed it, when it graded it.
#[must_use]
pub fn passed(source: &Source) -> Option<bool> {
    match source {
        Source::Public { trial, .. } => trial.reward.map(|r| r >= 1.0),
        Source::Local(run) => match run.outcome {
            Outcome::Passed => Some(true),
            Outcome::Failed => Some(false),
            _ => None,
        },
    }
}

fn outcome_word(source: &Source) -> &'static str {
    match source {
        Source::Public { trial, .. } => match trial.reward {
            Some(r) if r >= 1.0 => "passed",
            Some(_) => "failed",
            None => "not graded",
        },
        Source::Local(run) => run.outcome.word(),
    }
}

fn cost(source: &Source) -> Option<f64> {
    match source {
        Source::Public { trial, .. } => trial.cost_usd,
        Source::Local(run) => run.cost_usd,
    }
}

/// Where verification fell in the run.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Verification {
    /// Test and verify steps.
    pub total: usize,
    /// How many fell in each quarter of the steps.
    pub quarters: [usize; 4],
    pub before_first_edit: usize,
    pub after_last_edit: usize,
    /// The first test or verify step after the last edit.
    pub after_last_edit_step: Option<usize>,
    /// The share of verification in the last quarter.
    pub last_quarter_share: Option<f64>,
    /// The share of edit runs followed by a test or verify step before the
    /// next edit.
    pub edits_checked_share: Option<f64>,
}

/// Retries.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Retries {
    /// Commands identical to an earlier failed one or to the one before.
    pub repeated_failed: usize,
    /// Jev's retries among the steps it placed, not counted above.
    pub by_jev: usize,
    /// Steps that failed.
    pub failed_steps: usize,
    /// The first repeated step.
    pub first_step: Option<usize>,
}

/// How steps were placed, and Jev's Nouls on the steps it placed.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Placement {
    pub by_rules: usize,
    pub by_jev: usize,
    pub unplaced: usize,
    pub checks_assumption: usize,
    pub uses_evidence: usize,
    pub retry: usize,
}

/// One trajectory's fingerprint.
#[derive(Clone, Debug, Serialize)]
pub struct Fingerprint {
    pub schema: &'static str,
    pub run: String,
    pub task: String,
    pub group: &'static str,
    pub arm: String,
    pub outcome: &'static str,
    pub passed: Option<bool>,
    pub cost_usd: Option<f64>,
    pub steps: usize,
    pub duration_ms: u64,
    pub first_edit_step: Option<usize>,
    pub first_edit_ms: Option<u64>,
    /// Steps before the first edit, as a share of all steps.
    pub first_edit_share: Option<f64>,
    pub tests: usize,
    pub first_test_step: Option<usize>,
    pub test_rate: f64,
    pub tests_per_edit: Option<f64>,
    pub verification: Verification,
    pub retries: Retries,
    pub files_touched: usize,
    pub files: Vec<String>,
    /// Whether it ran a test, a script, or a program before its first edit.
    pub ran_before_edit: bool,
    pub ran_before_edit_step: Option<usize>,
    /// Whether the task's words name something to run: a command or a
    /// script in backticks, or an example.
    pub task_has_example: bool,
    pub ran_example_before_edit: bool,
    pub example_step: Option<usize>,
    pub phases: BTreeMap<&'static str, usize>,
    /// Runs of phases: `O3 R5 E1 T2`; `?` is unplaced.
    pub sequence: String,
    /// How many separate runs of edits there were.
    pub edit_runs: usize,
    /// Checks Coder One's controller ran, which the verification counts
    /// leave out.
    pub controller_checks: usize,
    pub placement: Placement,
    pub rules: &'static str,
    pub questions: &'static str,
}

/// Commands and scripts the task's words name, for the example check.
#[must_use]
pub fn example_tokens(task: &str) -> Vec<String> {
    const PROGRAMS: [&str; 16] = [
        "python", "python3", "./", "bash", "sh ", "node", "make", "cargo", "npm", "pytest", "go ",
        "java", "ruby", "curl", "uv ", "npx",
    ];
    const SCRIPTS: [&str; 9] = [
        ".py", ".sh", ".js", ".ts", ".rb", ".pl", ".jl", ".R", ".mjs",
    ];
    let mut tokens: Vec<String> = Vec::new();
    for (index, span) in task.split('`').enumerate() {
        if index % 2 == 0 || span.is_empty() || span.len() > 200 {
            continue;
        }
        let span = span.trim();
        let command = span.contains(' ') && PROGRAMS.iter().any(|p| span.starts_with(p));
        let script = !span.contains(' ') && SCRIPTS.iter().any(|ext| span.ends_with(ext));
        let example =
            span.to_lowercase().contains("example") || span.to_lowercase().contains("sample");
        if command {
            tokens.push(span.split_whitespace().collect::<Vec<_>>().join(" "));
        } else if script || example {
            let name = span.rsplit('/').next().unwrap_or(span).to_owned();
            if !name.is_empty() {
                tokens.push(name);
            }
        }
    }
    tokens.sort();
    tokens.dedup();
    tokens
}

/// Compresses a phase sequence into runs: `O3 R5 E1`.
#[must_use]
pub fn compress(steps: &[Step]) -> String {
    let mut runs: Vec<(char, usize)> = Vec::new();
    for step in steps {
        let letter = step.phase.map_or('?', Phase::letter);
        match runs.last_mut() {
            Some((last, count)) if *last == letter => *count += 1,
            _ => runs.push((letter, 1)),
        }
    }
    let mut words: Vec<String> = runs
        .iter()
        .take(80)
        .map(|(letter, count)| format!("{letter}{count}"))
        .collect();
    if runs.len() > 80 {
        words.push(format!("… {} more runs", runs.len() - 80));
    }
    words.join(" ")
}

/// A fingerprint from placed steps.
#[must_use]
pub fn fingerprint(source: &Source, steps: &[Step], task: &str, duration_ms: u64) -> Fingerprint {
    let total = steps.len();
    let is = |step: &Step, phase: Phase| step.phase == Some(phase);
    let first_edit = steps.iter().position(|s| is(s, Phase::Edit));
    let last_edit = steps.iter().rposition(|s| is(s, Phase::Edit));
    let start_ms = steps.first().map_or(0, |s| s.elapsed_ms);
    let edits = steps.iter().filter(|s| is(s, Phase::Edit)).count();
    let tests = steps.iter().filter(|s| is(s, Phase::Test)).count();
    // Coder One's controller checks count apart: they are the harness's
    // verification, not the agent's.
    let controller = |s: &Step| matches!(s.kind, ActionKind::Check);
    let verifying = |s: &Step| (is(s, Phase::Test) || is(s, Phase::Verify)) && !controller(s);
    let mut verification = Verification::default();
    for (index, step) in steps.iter().enumerate() {
        if !verifying(step) {
            continue;
        }
        verification.total += 1;
        verification.quarters[(index * 4 / total.max(1)).min(3)] += 1;
        if first_edit.is_none_or(|first| index < first) {
            verification.before_first_edit += 1;
        }
        if last_edit.is_some_and(|last| index > last) {
            verification.after_last_edit += 1;
            verification.after_last_edit_step.get_or_insert(step.n);
        }
    }
    verification.last_quarter_share = (verification.total > 0)
        .then(|| verification.quarters[3] as f64 / verification.total as f64);
    // Edit runs, and whether each is checked before the next.
    let mut edit_runs = 0usize;
    let mut checked = 0usize;
    let mut in_edit = false;
    let mut pending_check = false;
    for step in steps {
        if is(step, Phase::Edit) {
            if !in_edit {
                edit_runs += 1;
                pending_check = true;
            }
            in_edit = true;
        } else {
            in_edit = false;
            if pending_check && verifying(step) {
                checked += 1;
                pending_check = false;
            }
        }
    }
    verification.edits_checked_share = (edit_runs > 0).then(|| checked as f64 / edit_runs as f64);
    let mut retries = Retries::default();
    let mut placement = Placement::default();
    for step in steps {
        if step.failed == Some(true) {
            retries.failed_steps += 1;
        }
        if step.repeats_failed {
            retries.repeated_failed += 1;
            retries.first_step.get_or_insert(step.n);
        }
        match step.by {
            "rule" => placement.by_rules += 1,
            "jev" => placement.by_jev += 1,
            _ => placement.unplaced += 1,
        }
        if let Some(answer) = &step.jev {
            placement.checks_assumption += usize::from(answer.checks_assumption >= YES);
            placement.uses_evidence += usize::from(answer.uses_evidence >= YES);
            if answer.retry >= YES {
                placement.retry += 1;
                if !step.repeats_failed {
                    retries.by_jev += 1;
                    retries.first_step.get_or_insert(step.n);
                }
            }
        }
    }
    let mut files: Vec<String> = steps
        .iter()
        .filter(|s| is(s, Phase::Edit))
        .flat_map(|s| s.writes.iter().cloned())
        .filter(|path| !path.is_empty())
        .collect();
    files.sort();
    files.dedup();
    // A relative path is the same file as an absolute one that ends in it.
    let absolute: Vec<String> = files
        .iter()
        .filter(|p| p.starts_with('/'))
        .cloned()
        .collect();
    files.retain(|path| {
        path.starts_with('/')
            || !absolute
                .iter()
                .any(|full| full.ends_with(&format!("/{}", path.trim_start_matches("./"))))
    });
    let before = &steps[..first_edit.unwrap_or(total)];
    let ran = before.iter().find(|s| s.executes || is(s, Phase::Test));
    let tokens = example_tokens(task);
    let example = before.iter().find(|s| {
        (s.executes || is(s, Phase::Test))
            && tokens.iter().any(|token| {
                s.input
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
                    .contains(token.as_str())
            })
    });
    let mut phases: BTreeMap<&'static str, usize> = BTreeMap::new();
    for step in steps {
        *phases
            .entry(step.phase.map_or("unplaced", Phase::name))
            .or_default() += 1;
    }
    Fingerprint {
        schema: SCHEMA,
        run: source.id(),
        task: source.task().to_owned(),
        group: group(source),
        arm: arm(source),
        outcome: outcome_word(source),
        passed: passed(source),
        cost_usd: cost(source),
        steps: total,
        duration_ms,
        first_edit_step: first_edit.map(|i| steps[i].n),
        first_edit_ms: first_edit.map(|i| steps[i].elapsed_ms.saturating_sub(start_ms)),
        first_edit_share: first_edit.map(|i| i as f64 / total.max(1) as f64),
        tests,
        first_test_step: steps.iter().find(|s| is(s, Phase::Test)).map(|s| s.n),
        test_rate: tests as f64 / total.max(1) as f64,
        tests_per_edit: (edits > 0).then(|| tests as f64 / edits as f64),
        verification,
        retries,
        files_touched: files.len(),
        files: files.into_iter().take(20).collect(),
        ran_before_edit: ran.is_some(),
        ran_before_edit_step: ran.map(|s| s.n),
        task_has_example: !tokens.is_empty(),
        ran_example_before_edit: example.is_some(),
        example_step: example.map(|s| s.n),
        phases,
        sequence: compress(steps),
        edit_runs,
        controller_checks: steps.iter().filter(|s| controller(s)).count(),
        placement,
        rules: RULES_VERSION,
        questions: QUESTION_SET,
    }
}

/// One trajectory, loaded and placed.
pub struct Loaded {
    pub source: Source,
    pub steps: Vec<Step>,
    pub task: String,
    pub duration_ms: u64,
}

/// Loads and places a trajectory's steps by the rules.
///
/// # Errors
///
/// Returns why the transcript can't be read.
pub fn load(source: &Source) -> Result<Loaded, String> {
    let replay = Replay::load(source)?;
    let fallback = match source {
        Source::Local(run) => run
            .task_path
            .as_ref()
            .and_then(|path| std::fs::read_to_string(path.join("instruction.md")).ok()),
        Source::Public { .. } => None,
    };
    let task = runs_phases::task_of(&replay, fallback);
    Ok(Loaded {
        source: source.clone(),
        steps: runs_phases::steps(&replay),
        task,
        duration_ms: replay.duration_ms,
    })
}

/// Every trajectory: local runs, then the public Fable ones.
#[must_use]
pub fn all_sources(sources: Sources) -> Vec<Source> {
    let catalog = Catalog::load(sources);
    let (local, public, _) = crate::runs_replay::sources(&catalog);
    local.into_iter().chain(public).collect()
}

/// Loads `sources`, asks Jev about the steps the rules leave unless
/// `judge` is off, and returns the fingerprints with the labeling report
/// and any load failures.
pub fn build(
    sources: &[Source],
    store: &mut StepStore,
    judge: &Judge,
    limit: Option<usize>,
) -> (Vec<(Fingerprint, Loaded)>, Labeling, Vec<String>) {
    let mut loaded = Vec::new();
    let mut errors = Vec::new();
    for source in sources {
        match load(source) {
            Ok(one) => loaded.push(one),
            Err(error) => errors.push(format!(
                "{}: {}",
                source.id(),
                error.lines().next().unwrap_or_default()
            )),
        }
    }
    let pending: Vec<(String, Value)> = loaded
        .iter()
        .flat_map(|one| runs_phases::pending(&one.steps, &one.task))
        .collect();
    let labeling = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime.block_on(runs_phases::label(pending, store, judge, limit)),
        Err(error) => Labeling {
            errors: vec![format!("cannot start a runtime: {error}")],
            ..Labeling::default()
        },
    };
    let out = loaded
        .into_iter()
        .map(|mut one| {
            runs_phases::apply(&mut one.steps, &one.task, store);
            (
                fingerprint(&one.source, &one.steps, &one.task, one.duration_ms),
                one,
            )
        })
        .collect();
    (out, labeling, errors)
}

/// The text `gym runs fingerprint` prints.
#[must_use]
pub fn render(print: &Fingerprint, steps: &[Step]) -> Vec<String> {
    let mut out = vec![
        format!("Fingerprint · {}", print.run),
        format!(
            "{} · {} · {} · {}",
            print.task,
            print.arm,
            print.outcome,
            print.cost_usd.map_or("cost unknown".to_owned(), money)
        ),
        format!(
            "{} steps over {} · first edit {}",
            print.steps,
            duration(print.duration_ms),
            match (
                print.first_edit_step,
                print.first_edit_ms,
                print.first_edit_share
            ) {
                (Some(step), Some(ms), Some(share)) => format!(
                    "at step {step} ({:.0}% of steps), {} in",
                    share * 100.0,
                    duration(ms)
                ),
                _ => "never".to_owned(),
            }
        ),
        format!(
            "Tests: {} ({:.2} a step{}) · test or verify steps in each quarter of the run: {}, {}, {}, {}, and {} after the last edit{}",
            print.tests,
            print.test_rate,
            print
                .tests_per_edit
                .map_or(String::new(), |t| format!(", {t:.1} an edit")),
            print.verification.quarters[0],
            print.verification.quarters[1],
            print.verification.quarters[2],
            print.verification.quarters[3],
            print.verification.after_last_edit,
            print
                .verification
                .edits_checked_share
                .map_or(String::new(), |s| format!(
                    " · {:.0}% of edit runs checked before the next",
                    s * 100.0
                ))
        ),
        format!(
            "Retries: {} repeated failed commands, {} more by Jev · {} failed steps",
            print.retries.repeated_failed, print.retries.by_jev, print.retries.failed_steps
        ),
        format!(
            "Files touched: {}{}",
            print.files_touched,
            if print.files.is_empty() {
                String::new()
            } else {
                format!(" ({})", print.files.join(", "))
            }
        ),
        format!(
            "Ran something before the first edit: {} · ran the task's example before editing: {}",
            print
                .ran_before_edit_step
                .map_or("no".to_owned(), |n| format!("yes, step {n}")),
            if print.task_has_example {
                print
                    .example_step
                    .map_or("no".to_owned(), |n| format!("yes, step {n}"))
            } else {
                "the task names no example".to_owned()
            }
        ),
        format!(
            "Phases: {}",
            print
                .phases
                .iter()
                .map(|(phase, count)| format!("{phase} {count}"))
                .collect::<Vec<_>>()
                .join(" · ")
        ),
        format!("Sequence: {}", print.sequence),
        format!(
            "Placed: {} by rules, {} by Jev, {} unplaced · Jev on its steps: checks an assumption {}, uses earlier evidence {}, retry {}",
            print.placement.by_rules,
            print.placement.by_jev,
            print.placement.unplaced,
            print.placement.checks_assumption,
            print.placement.uses_evidence,
            print.placement.retry
        ),
    ];
    if !steps.is_empty() {
        out.push(String::new());
        out.push("Steps".to_owned());
        for step in steps {
            out.push(format!(
                "{:>5}  {}  {:<8} {:<4}  {}{}",
                step.n,
                clock(step.elapsed_ms),
                step.phase.map_or("?", Phase::name),
                step.by,
                crate::runs_transcript::first_line(&format!("{} {}", step.tool, step.input))
                    .chars()
                    .take(110)
                    .collect::<String>(),
                match (step.failed, step.repeats_failed) {
                    (Some(true), true) => "   (failed, repeated)",
                    (Some(true), false) => "   (failed)",
                    (_, true) => "   (repeated)",
                    _ => "",
                }
            ));
        }
    }
    out
}

fn clock(ms: u64) -> String {
    let seconds = ms / 1000;
    format!(
        "{:02}:{:02}:{:02}",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60
    )
}

fn labeling_line(labeling: &Labeling, judge: &Judge) -> String {
    format!(
        "Jev ({}): {} unplaced steps, {} already answered, {} asked, {} answered, {} failed, {} input tokens, ${:.4}{}",
        judge.word(),
        labeling.considered,
        labeling.cached,
        labeling.asked,
        labeling.answered,
        labeling.failed,
        labeling.input_tokens,
        labeling.cost_usd,
        labeling
            .errors
            .first()
            .map_or(String::new(), |e| format!(" · {e}"))
    )
}

const USAGE: &str = "usage:
  gym runs fingerprint RUN [--json] [--no-jev] [--no-steps]
  gym runs fingerprints [--task T]... [--subset] [--agent A] [--outcome passed|failed]
                        [--json] [--no-jev] [--jev-limit N]
  gym runs moves [--task T]... [--json] [--no-jev] [--jev-limit N] [--min-tasks N]
  gym runs moves --cached [--json]

`fingerprint` places each step of one run in a phase (orient, read, plan,
edit, build, test, verify, finish) and prints the run's fingerprint and its
steps. RUN is a local run, `job/trial` or a job with one trial, or a public
Fable trial ID. Rules place most steps; Jev places the rest, and its
answers are kept in ~/.openagents/gym/fingerprints/step-answers.jsonl.

`fingerprints` prints the fingerprints of every trajectory of the tasks
given; `--subset` is the Luna baseline subset of #9583. `--agent` is
fable, luna, coder-one, claude-code, or codex.

`moves` compares fingerprints per task: Fable winners with Fable losers,
all winners with all losers, and Fable winners with Luna and with Coder
One. A difference that repeats across tasks is a candidate move, with its
task count, effect size, and citations. The default tasks are the subset.

Each `moves` run keeps its report in ~/.openagents/gym/fingerprints/moves.json;
`--cached` prints that report without reading any trajectory.

--no-jev (or GYM_JEV=off) reads stored answers and asks nothing new.";

/// Parsed arguments shared by the three commands.
struct Args {
    json: bool,
    judge: Judge,
    limit: Option<usize>,
    tasks: Vec<String>,
    agent: Option<String>,
    outcome: Option<bool>,
    min_tasks: usize,
    steps: bool,
    cached: bool,
    run: Option<String>,
    sources: Sources,
    dir: Option<PathBuf>,
}

fn parse(args: &[String]) -> Result<Args, String> {
    let mut parsed = Args {
        json: false,
        judge: Judge::Off(String::new()),
        limit: None,
        tasks: Vec::new(),
        agent: None,
        outcome: None,
        min_tasks: 3,
        steps: true,
        cached: false,
        run: None,
        sources: Sources::standard(),
        dir: runs_phases::default_dir(),
    };
    let mut no_jev = false;
    let mut index = 1;
    let value = |index: usize| args.get(index + 1).cloned().ok_or_else(|| USAGE.to_owned());
    while index < args.len() {
        match args[index].as_str() {
            "--json" => parsed.json = true,
            "--no-jev" => no_jev = true,
            "--no-steps" => parsed.steps = false,
            "--cached" => parsed.cached = true,
            "--subset" => parsed
                .tasks
                .extend(LUNA_SUBSET.iter().map(|t| (*t).to_owned())),
            "--task" => {
                parsed.tasks.push(value(index)?);
                index += 1;
            }
            "--agent" => {
                parsed.agent = Some(value(index)?.to_lowercase());
                index += 1;
            }
            "--outcome" => {
                parsed.outcome = match value(index)?.as_str() {
                    "passed" => Some(true),
                    "failed" => Some(false),
                    other => return Err(format!("unknown outcome {other}\n\n{USAGE}")),
                };
                index += 1;
            }
            "--jev-limit" => {
                parsed.limit = Some(
                    value(index)?
                        .parse()
                        .map_err(|_| format!("--jev-limit needs a number\n\n{USAGE}"))?,
                );
                index += 1;
            }
            "--min-tasks" => {
                parsed.min_tasks = value(index)?
                    .parse()
                    .map_err(|_| format!("--min-tasks needs a number\n\n{USAGE}"))?;
                index += 1;
            }
            "--dir" => {
                parsed.dir = Some(PathBuf::from(value(index)?));
                index += 1;
            }
            "--jobs-dir" => {
                parsed.sources.jobs = Some(PathBuf::from(value(index)?));
                index += 1;
            }
            "--traces-dir" => {
                parsed.sources.traces = Some(PathBuf::from(value(index)?));
                index += 1;
            }
            "--help" | "-h" => return Err(USAGE.to_owned()),
            other
                if parsed.run.is_none() && !other.starts_with('-') && args[0] == "fingerprint" =>
            {
                parsed.run = Some(other.to_owned());
            }
            other => return Err(format!("unknown argument {other}\n\n{USAGE}")),
        }
        index += 1;
    }
    parsed.judge = if no_jev {
        Judge::Off("--no-jev turns Jev off".to_owned())
    } else {
        Judge::from_environment()
    };
    parsed.tasks.sort();
    parsed.tasks.dedup();
    Ok(parsed)
}

/// Whether the word is one of these commands.
#[must_use]
pub fn handles(word: &str) -> bool {
    matches!(word, "fingerprint" | "fingerprints" | "moves")
}

fn find(sources: Vec<Source>, name: &str) -> Result<Source, String> {
    let mut matches: Vec<Source> = sources
        .into_iter()
        .filter(|source| match source {
            Source::Local(run) => run.id() == name || run.job == name || run.trial == name,
            Source::Public { trial, .. } => trial.id == name,
        })
        .collect();
    match matches.len() {
        0 => Err(format!("no run named {name}")),
        1 => Ok(matches.remove(0)),
        n => Err(format!("{name} names {n} runs; use job/trial")),
    }
}

/// `gym runs fingerprint`, `gym runs fingerprints`, and `gym runs moves`.
///
/// # Errors
///
/// Returns the usage text when the arguments don't parse, and a message
/// when a run isn't found or output can't be written.
pub fn command(args: &[String], out: &mut impl Write) -> Result<i32, String> {
    let parsed = match parse(args) {
        Ok(parsed) => parsed,
        Err(message) if message == USAGE => {
            writeln!(out, "{USAGE}").map_err(|e| e.to_string())?;
            return Ok(0);
        }
        Err(message) => return Err(message),
    };
    let write =
        |out: &mut dyn Write, text: &str| writeln!(out, "{text}").map_err(|e| e.to_string());
    if args[0] == "moves" && parsed.cached {
        let path = parsed
            .dir
            .as_ref()
            .map(|dir| dir.join(MOVES_FILE))
            .ok_or_else(|| "no fingerprint directory: HOME is not set".to_owned())?;
        let value = crate::runs::read_json(&path).ok_or_else(|| {
            format!(
                "no moves report at {}; run `gym runs moves` first",
                path.display()
            )
        })?;
        if parsed.json {
            write(
                out,
                &serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?,
            )?;
        } else {
            for one in value
                .pointer("/report/candidates")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                write(
                    out,
                    &format!(
                        "{} · {} {} {}: {} of {} tasks, mean delta {:+.2}",
                        one["comparison"].as_str().unwrap_or("?"),
                        one["a"].as_str().unwrap_or("?"),
                        if one["direction"] == "more" {
                            "does more:"
                        } else {
                            "does less:"
                        },
                        one["what"].as_str().unwrap_or("?"),
                        one["tasks_agreeing"],
                        one["tasks_with_data"],
                        one["mean_delta"].as_f64().unwrap_or(0.0)
                    ),
                )?;
            }
        }
        return Ok(0);
    }
    let mut store = StepStore::open(parsed.dir.clone());
    let sources = all_sources(parsed.sources.clone());
    match args[0].as_str() {
        "fingerprint" => {
            let name = parsed.run.clone().ok_or_else(|| USAGE.to_owned())?;
            let source = find(sources, &name)?;
            let (mut built, labeling, errors) =
                build(&[source], &mut store, &parsed.judge, parsed.limit);
            if let Some(error) = errors.first() {
                return Err(error.clone());
            }
            let (print, loaded) = built.remove(0);
            if parsed.json {
                let value = json!({
                    "fingerprint": print,
                    "steps": loaded.steps,
                    "jev": labeling,
                });
                write(
                    out,
                    &serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?,
                )?;
            } else {
                for line in render(&print, if parsed.steps { &loaded.steps } else { &[] }) {
                    write(out, &line)?;
                }
                write(out, "")?;
                write(out, &labeling_line(&labeling, &parsed.judge))?;
            }
            Ok(0)
        }
        "fingerprints" => {
            let chosen: Vec<Source> = sources
                .into_iter()
                .filter(|s| parsed.tasks.is_empty() || parsed.tasks.iter().any(|t| t == s.task()))
                .filter(|s| parsed.agent.as_deref().is_none_or(|a| group(s) == a))
                .filter(|s| parsed.outcome.is_none_or(|o| passed(s) == Some(o)))
                .collect();
            let (built, labeling, errors) = build(&chosen, &mut store, &parsed.judge, parsed.limit);
            let prints: Vec<&Fingerprint> = built.iter().map(|(p, _)| p).collect();
            if parsed.json {
                let value = json!({
                    "schema": "openagents.gym.runs-fingerprints.v1",
                    "tasks": parsed.tasks,
                    "agent": parsed.agent,
                    "count": prints.len(),
                    "fingerprints": prints,
                    "jev": labeling,
                    "unreadable": errors,
                });
                write(
                    out,
                    &serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?,
                )?;
            } else {
                write(
                    out,
                    &format!(
                        "{:<44} {:<18} {:<7} {:>5} {:>10} {:>5} {:>12} {:>7} {:>7}  sequence",
                        "run",
                        "agent",
                        "outcome",
                        "steps",
                        "first edit",
                        "tests",
                        "checks after",
                        "retries",
                        "example"
                    ),
                )?;
                for print in &prints {
                    write(
                        out,
                        &format!(
                            "{:<44} {:<18} {:<7} {:>5} {:>10} {:>5} {:>12} {:>7} {:>7}  {}",
                            print.run.chars().take(44).collect::<String>(),
                            print.arm.chars().take(18).collect::<String>(),
                            print.outcome,
                            print.steps,
                            print
                                .first_edit_step
                                .map_or("-".to_owned(), |n| n.to_string()),
                            print.tests,
                            print.verification.after_last_edit,
                            print.retries.repeated_failed + print.retries.by_jev,
                            if !print.task_has_example {
                                "-"
                            } else if print.ran_example_before_edit {
                                "yes"
                            } else {
                                "no"
                            },
                            print.sequence.chars().take(60).collect::<String>()
                        ),
                    )?;
                }
                write(out, "")?;
                write(out, &format!("{} fingerprints", prints.len()))?;
                write(
                    out,
                    "first edit: the step of the first edit. checks after: test or verify steps after the last edit. retries: repeated failed commands and retries Jev found. example: whether the run ran the task's example before its first edit.",
                )?;
                write(out, &labeling_line(&labeling, &parsed.judge))?;
                for error in errors.iter().take(5) {
                    write(out, &format!("unreadable: {error}"))?;
                }
            }
            Ok(0)
        }
        _ => {
            let tasks: Vec<String> = if parsed.tasks.is_empty() {
                LUNA_SUBSET.iter().map(|t| (*t).to_owned()).collect()
            } else {
                parsed.tasks.clone()
            };
            let chosen: Vec<Source> = sources
                .into_iter()
                .filter(|s| tasks.iter().any(|t| t == s.task()))
                .collect();
            let (built, labeling, errors) = build(&chosen, &mut store, &parsed.judge, parsed.limit);
            let prints: Vec<Fingerprint> = built.into_iter().map(|(p, _)| p).collect();
            let report = crate::runs_moves::moves(&prints, &tasks, parsed.min_tasks);
            let value = json!({
                "schema": "openagents.gym.runs-moves.v1",
                "tasks": tasks,
                "fingerprints": prints.len(),
                "rules": RULES_VERSION,
                "questions": QUESTION_SET,
                "report": report,
                "jev": labeling,
                "unreadable": errors,
            });
            if let Some(dir) = &parsed.dir {
                // The report is a Gym record `coder-one ask` reads with
                // `--cached`; failing to keep it doesn't fail the command.
                let _ = std::fs::create_dir_all(dir).and_then(|()| {
                    std::fs::write(
                        dir.join(MOVES_FILE),
                        serde_json::to_string_pretty(&value).unwrap_or_default(),
                    )
                });
            }
            if parsed.json {
                write(
                    out,
                    &serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?,
                )?;
            } else {
                for line in crate::runs_moves::render(&report) {
                    write(out, &line)?;
                }
                write(out, "")?;
                write(
                    out,
                    &format!("{} fingerprints over {} tasks", prints.len(), tasks.len()),
                )?;
                write(out, &labeling_line(&labeling, &parsed.judge))?;
            }
            Ok(0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_task_s_example_is_a_command_or_script_in_backticks() {
        let task = "Run `python3 /app/run_example.py --n 3` and fix `src/lib.py`. Output goes to `/app/out.json`. See `tests/test_x.py`.";
        let tokens = example_tokens(task);
        assert!(tokens.contains(&"python3 /app/run_example.py --n 3".to_owned()));
        assert!(tokens.contains(&"lib.py".to_owned()));
        assert!(tokens.contains(&"test_x.py".to_owned()));
        assert!(!tokens.iter().any(|t| t.contains("out.json")));
    }
}
