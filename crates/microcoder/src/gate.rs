//! What a run must get past before it may end green, and the blind oracle.
//!
//! Without these, a run ends when every frozen test passes: the model says
//! it's finished, or the tests hold for [`crate::run::Limits::green_stop`]
//! steps. On many tasks the model's own suite misses requirements the
//! task's grader checks, so a green suite ends the run with budget left.
//! Each mechanism here is off by default and turned on by its own flag, so
//! a study can pin the configuration it measured:
//!
//! - `--gate-requirements`: code quotes the task's statements, and Jev
//!   judges, per statement, whether no test checks it. Named statements
//!   send the run back to test them.
//! - `--gate-target`: Jev judges whether the task states a numeric target
//!   that no test measures; if so, the run goes back once to measure it
//!   and improve toward it.
//! - `--gate-credible`: the model is asked to say whether its solution is
//!   credible when it finishes, and Jev judges whether its recent
//!   reasoning doubts the solution; doubt sends the run back.
//! - `--adversarial N`: up to N times, while less than a fraction of the
//!   time and spend is used, the run goes back to write tests that try to
//!   break the solution on stated requirements.
//! - `--oracle`: before the loop, a separate session writes checks from the
//!   task's statement and the provided files alone, before any solution
//!   exists. Checks that already pass on the untouched workspace are
//!   dropped; the rest are frozen with the model's tests, so a failing one
//!   blocks the finish like any frozen test.

use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::json;

use crate::env::Env;
use crate::models::{
    Generate, Generated, Judge, Judgment, credible_set, relevance_set, requirements_set, target_set,
};
use crate::state::{Action, CommandResult, State, Test, cut};

/// The mechanisms a green run must get past before it ends. All off by
/// default: the loop then behaves as it did before they existed.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Gates {
    /// Jev checks each statement of the task against the tests.
    pub requirements: bool,
    /// Jev checks for a numeric target no test measures.
    pub target: bool,
    /// Jev checks the model's recent reasoning for doubt.
    pub credible: bool,
    /// Jev's probability of doubt at which the credibility check sends the
    /// run back.
    pub doubt: f64,
    /// Rounds of adversarial tests before a green run may end; 0 is off.
    pub adversarial: usize,
    /// The share of the time and spend limits after which adversarial
    /// rounds stop being asked for.
    pub budget_fraction: f64,
    /// Whether a separate session writes checks from the task alone first.
    pub oracle: bool,
    /// Steps the oracle session may take, at most.
    pub oracle_steps: usize,
}

impl Default for Gates {
    fn default() -> Self {
        Gates {
            requirements: false,
            target: false,
            credible: false,
            doubt: DOUBT,
            adversarial: 0,
            budget_fraction: 0.5,
            oracle: false,
            oracle_steps: 8,
        }
    }
}

impl Gates {
    /// Whether any end-of-run check is on.
    #[must_use]
    pub fn any_check(&self) -> bool {
        self.requirements || self.target || self.credible || self.adversarial > 0
    }
}

/// Jev's probability at which a statement is named as unchecked.
pub const UNCHECKED: f64 = 0.6;

/// Jev's probability at which the model's reasoning doubts its solution,
/// by default. Replayed on the end states of 44 green-ending runs on the
/// excluded development tasks, 0.9 sent back 16 of 28 graded failures and
/// 1 of 16 graded passes; 0.5 sent back 21 of 28 and 9 of 16.
pub const DOUBT: f64 = 0.9;

/// Jev's probability at which the task states a numeric target, and under
/// which no test measures it.
pub const TARGET: f64 = 0.6;
pub const MEASURED: f64 = 0.5;

/// Statements Jev judges per request.
pub const STATEMENTS_PER_CALL: usize = 20;

/// Statements quoted from a task, at most.
pub const STATEMENTS: usize = 60;

/// Times the requirements check may send a run back.
pub const REQUIREMENT_ROUNDS: usize = 2;

/// Times the credibility check may send a run back.
pub const CREDIBLE_ROUNDS: usize = 2;

/// Unchecked statements a note names, at most.
const NAMED: usize = 8;

/// What the gates remember across a run.
#[derive(Clone, Debug, Default)]
pub struct GateState {
    requirement_rounds: usize,
    /// Statements already named, so each is named once.
    named: Vec<String>,
    target_checked: bool,
    credible_rounds: usize,
    adversarial_rounds: usize,
}

/// One gate's check, for the record.
#[derive(Clone, Debug, Serialize)]
pub struct Checked {
    /// `requirements`, `target`, `credible`, or `adversarial`.
    pub check: String,
    /// Jev's answers, when the check asked Jev.
    pub judgments: Vec<Judgment>,
    /// Whether it sent the run back.
    pub refused: bool,
    /// What it found: the statements named, or the reasoning that doubts.
    pub detail: Vec<String>,
}

/// The statements code quotes from a task for the requirements check:
/// each sentence outside fenced blocks, with list markers removed, and each
/// fenced block as one statement. Statements under four words, such as
/// headings, are left out.
#[must_use]
pub fn statements(task: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut fence: Option<Vec<&str>> = None;
    for line in task.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            match fence.take() {
                Some(block) => {
                    let text = block.join("\n");
                    if !text.trim().is_empty() {
                        out.push(cut(text.trim(), 600, 0));
                    }
                }
                None => fence = Some(Vec::new()),
            }
            continue;
        }
        if let Some(block) = fence.as_mut() {
            block.push(line);
            continue;
        }
        let item = trimmed
            .trim_start_matches(['-', '*', '+', '>', '#'])
            .trim_start();
        let item = strip_number(item);
        for sentence in sentences(item) {
            if sentence.split_whitespace().count() >= 4 {
                out.push(cut(&sentence, 600, 0));
            }
        }
    }
    if let Some(block) = fence {
        let text = block.join("\n");
        if !text.trim().is_empty() {
            out.push(cut(text.trim(), 600, 0));
        }
    }
    out.dedup();
    out.truncate(STATEMENTS);
    out
}

/// `text` without a leading list number such as `3.` or `3)`.
fn strip_number(text: &str) -> &str {
    let digits = text.chars().take_while(char::is_ascii_digit).count();
    if digits > 0 {
        let rest = &text[digits..];
        if let Some(stripped) = rest.strip_prefix(['.', ')']) {
            return stripped.trim_start();
        }
    }
    text
}

/// Splits a line at a sentence end: `.`, `!`, or `?` followed by a space
/// and an upper-case letter.
fn sentences(line: &str) -> Vec<String> {
    let chars: Vec<char> = line.chars().collect();
    let mut out = Vec::new();
    let mut start = 0;
    for n in 0..chars.len() {
        let ends = matches!(chars[n], '.' | '!' | '?')
            && chars.get(n + 1) == Some(&' ')
            && chars.get(n + 2).is_some_and(|c| c.is_uppercase());
        if ends {
            out.push(
                chars[start..=n]
                    .iter()
                    .collect::<String>()
                    .trim()
                    .to_string(),
            );
            start = n + 2;
        }
    }
    let rest: String = chars[start.min(chars.len())..].iter().collect();
    if !rest.trim().is_empty() {
        out.push(rest.trim().to_string());
    }
    out
}

/// The frozen tests as Jev reads them, within `room` characters.
fn tests_for_jev(state: &State, mut room: usize) -> Vec<serde_json::Value> {
    state
        .tests
        .iter()
        .map(|t| {
            let script = cut(&t.script, room.min(2_500), 0);
            room = room.saturating_sub(script.len());
            json!({"name": t.name, "script": script})
        })
        .collect()
}

/// How much of the run's limits is used, each from 0 to 1.
#[derive(Clone, Copy, Debug)]
pub struct Used {
    pub time: f64,
    pub spend: f64,
}

/// Runs the gates that are on, in order, and returns every check made and
/// the note that sends the run back, or `None` when the run may end.
/// `rationale` is the model's reason on the step that would end the run.
pub async fn check<J: Judge>(
    gates: &Gates,
    memory: &mut GateState,
    judge: &J,
    state: &State,
    used: Used,
    accept_dir: &str,
) -> (Vec<Checked>, Option<String>) {
    let mut checks = Vec::new();
    if gates.requirements && memory.requirement_rounds < REQUIREMENT_ROUNDS {
        let (checked, named) = requirements(judge, state, &memory.named).await;
        let refused = !named.is_empty();
        checks.push(Checked { refused, ..checked });
        if refused {
            memory.requirement_rounds += 1;
            memory.named.extend(named.iter().cloned());
            let list: String = named
                .iter()
                .enumerate()
                .map(|(n, s)| format!("{}. \"{}\"\n", n + 1, cut(s, 400, 0)))
                .collect();
            return (
                checks,
                Some(format!(
                    "Every frozen test passes, but Jev judged that no test checks these statements \
of the task:\n{list}For each one that sets a requirement, write a test under {accept_dir} that \
checks it, with the expected result taken from the task's statement rather than from what the \
code prints now, set freeze_tests to true to add the tests, and fix what they find. If a \
statement is already checked or sets no requirement, say which and why in the rationale."
                )),
            );
        }
    }
    if gates.target && !memory.target_checked {
        memory.target_checked = true;
        let jev_state = json!({
            "task": cut(&state.task, 6_000, 0),
            "tests": tests_for_jev(state, 9_000),
        });
        let judgment = judge.judge(&target_set(), &jev_state).await;
        let p = |id: &str| {
            judgment
                .answers
                .iter()
                .find(|(q, _)| q == id)
                .map(|(_, p)| *p)
        };
        let refused =
            p("target").is_some_and(|t| t >= TARGET) && p("measured").is_some_and(|m| m < MEASURED);
        checks.push(Checked {
            check: "target".to_string(),
            judgments: vec![judgment],
            refused,
            detail: Vec::new(),
        });
        if refused {
            return (
                checks,
                Some(format!(
                    "Every frozen test passes, but Jev judged that the task states a numeric \
target that no test measures. Write a test under {accept_dir} that measures that quantity the \
way the task states and compares it with the target, printing the measured value; set \
freeze_tests to true to add it. Then improve the solution until it passes, keeping a change \
only when every other test still passes."
                )),
            );
        }
    }
    if gates.credible && memory.credible_rounds < CREDIBLE_ROUNDS {
        let rationales: Vec<String> = state
            .actions
            .iter()
            .rev()
            .take(4)
            .rev()
            .map(|a| format!("step {}: {}", a.step, cut(&a.rationale, 1_200, 0)))
            .collect();
        let jev_state = json!({
            "task": cut(&state.task, 6_000, 0),
            "rationales": rationales,
        });
        let judgment = judge.judge(&credible_set(), &jev_state).await;
        let refused = judgment
            .answers
            .iter()
            .any(|(q, p)| q == "doubt" && *p >= gates.doubt);
        let last = state
            .actions
            .last()
            .map(|a| a.rationale.clone())
            .unwrap_or_default();
        checks.push(Checked {
            check: "credible".to_string(),
            judgments: vec![judgment],
            refused,
            detail: if refused {
                vec![last.clone()]
            } else {
                Vec::new()
            },
        });
        if refused {
            memory.credible_rounds += 1;
            return (
                checks,
                Some(format!(
                    "Every frozen test passes, but Jev judged that your own recent reasoning \
doubts the solution (latest: \"{}\"). Passing your tests isn't enough: find what your \
reasoning doubts, test it against the task's statement, and fix it. If the doubt is resolved, \
say in the rationale why the solution meets every requirement of the task.",
                    cut(&last, 500, 0)
                )),
            );
        }
    }
    if memory.adversarial_rounds < gates.adversarial
        && used.time < gates.budget_fraction
        && used.spend < gates.budget_fraction
    {
        memory.adversarial_rounds += 1;
        checks.push(Checked {
            check: "adversarial".to_string(),
            judgments: Vec::new(),
            refused: true,
            detail: vec![format!(
                "round {} of {}",
                memory.adversarial_rounds, gates.adversarial
            )],
        });
        return (
            checks,
            Some(format!(
                "Every frozen test passes, and budget is left, so try to break the solution \
before finishing (round {} of {}). Reread the task and write at least three new tests under \
{accept_dir}, each checking a stated requirement, output format, value, or edge case that no \
frozen test checks yet, with expected results derived from the task's statement or an \
independent computation, never from what the code prints now. Set freeze_tests to true to add \
them, and fix what they find.",
                memory.adversarial_rounds, gates.adversarial
            )),
        );
    }
    (checks, None)
}

/// Asks Jev, per statement of the task not named before, whether no test
/// checks it; returns the check and the statements it names.
async fn requirements<J: Judge>(
    judge: &J,
    state: &State,
    named_before: &[String],
) -> (Checked, Vec<String>) {
    let all: Vec<String> = statements(&state.task)
        .into_iter()
        .filter(|s| !named_before.contains(s))
        .collect();
    let tests = tests_for_jev(state, 9_000);
    let mut judgments = Vec::new();
    let mut named: Vec<(String, f64)> = Vec::new();
    for chunk in all.chunks(STATEMENTS_PER_CALL) {
        let set = relevance_set(&requirements_set(), chunk.len());
        let mut jev_state = json!({
            "task": cut(&state.task, 4_000, 0),
            "tests": tests,
        });
        for (n, statement) in chunk.iter().enumerate() {
            jev_state[format!("entry_{}", n + 1)] = json!(statement);
        }
        let judgment = judge.judge(&set, &jev_state).await;
        for (n, statement) in chunk.iter().enumerate() {
            let id = format!("entry_{}", n + 1);
            if let Some((_, p)) = judgment.answers.iter().find(|(q, _)| *q == id)
                && *p >= UNCHECKED
            {
                named.push((statement.clone(), *p));
            }
        }
        judgments.push(judgment);
    }
    named.sort_by(|a, b| b.1.total_cmp(&a.1));
    named.truncate(NAMED);
    let named: Vec<String> = named.into_iter().map(|(s, _)| s).collect();
    (
        Checked {
            check: "requirements".to_string(),
            judgments,
            refused: false,
            detail: named.clone(),
        },
        named,
    )
}

/// Where the oracle session writes its checks.
pub const ORACLE_DIR: &str = "/tmp/oracle";

/// The prefix of an oracle check's name among the frozen tests.
pub const ORACLE_PREFIX: &str = "oracle-";

/// What the oracle session is told, after [`crate::run::SYSTEM`].
pub const ORACLE_SYSTEM: &str = " In this session you don't solve the task: you write an \
independent acceptance check for it. Another session writes the solution after you, never sees \
how you worked, and can't change your check. Read the task's statement and the files it \
provides: inputs, data, examples, and any provided checker, tests, or reference program. Don't \
write or change the task's deliverables, and change nothing outside /tmp/oracle. Write bash \
scripts as /tmp/oracle/<name>.sh, one per requirement you can check with confidence, each \
exiting 0 only when its requirement is met and finishing within a minute; helper files may go \
in /tmp/oracle too. Compute every expected result by a route that doesn't depend on any \
solution: the task's own definitions and stated values, a brute-force or reference computation \
you write, the provided examples, or properties every correct output must have. Check \
substance, not only that files exist. Where the statement doesn't determine an exact value, \
check a property instead of guessing. Since the task isn't done yet, the scripts should fail \
now; run them once to be sure they run. Set finished to true once they are written.";

/// The instruction the oracle session gets in place of the user prompt.
pub const ORACLE_PROMPT: &str = "Write the independent acceptance check the system text \
describes, under /tmp/oracle. Don't solve the task.";

/// What the oracle session produced.
#[derive(Clone, Debug, Default, Serialize)]
pub struct OracleReport {
    pub steps: usize,
    /// Every check it wrote.
    pub written: Vec<String>,
    /// Checks that passed on the untouched workspace, so were dropped.
    pub trivial: Vec<String>,
    /// Checks kept, to be frozen with the model's tests.
    pub kept: Vec<String>,
    /// Why the session stopped early, when it did.
    pub stopped: Option<String>,
}

/// One oracle-session step, for the record.
#[derive(Clone, Debug, Serialize)]
pub struct OracleStep {
    pub step: usize,
    pub generated: Generated,
    pub results: Vec<CommandResult>,
}

/// Reads the `.sh` files under `dir`, each named with `prefix`.
pub async fn load_scripts<E: Env>(
    env: &E,
    dir: &str,
    prefix: &str,
    deadline: Duration,
) -> Vec<Test> {
    let listing = env
        .run(&format!("ls -1 {dir}/*.sh 2>/dev/null"), deadline)
        .await;
    let mut tests = Vec::new();
    for path in listing
        .output
        .lines()
        .map(str::trim)
        .filter(|l| l.ends_with(".sh"))
    {
        if let Some(script) = env.read(path).await {
            let name = path.rsplit('/').next().unwrap_or(path);
            tests.push(Test {
                name: format!("{prefix}{name}"),
                script,
                passed_at_freeze: None,
            });
        }
    }
    tests
}

/// The limits the oracle session runs within.
pub struct OracleLimits {
    pub steps: usize,
    /// When the whole run must end.
    pub deadline: Instant,
    /// Dollars left for the whole run.
    pub usd_left: f64,
    pub command_seconds: u64,
    pub test_seconds: u64,
}

/// Runs the oracle session before the loop: a separate generation, with its
/// own system text and state, that sees the task and the environment but no
/// solution. Returns the checks to freeze with the model's tests, the
/// report, and each step for the record. `spend` receives each step's cost.
pub async fn write_oracle<E: Env, G: Generate>(
    env: &E,
    generator: &G,
    state: &State,
    limits: &OracleLimits,
    mut on_step: impl FnMut(&OracleStep),
    spend: &mut crate::run::Spend,
) -> (Vec<Test>, OracleReport) {
    let system = format!("{}{ORACLE_SYSTEM}", crate::run::SYSTEM);
    let mut sub = State {
        environment: state.environment.clone(),
        task: state.task.clone(),
        ..State::default()
    };
    let mut report = OracleReport::default();
    let mut bad = 0usize;
    let mut used = 0.0f64;
    let _ = env
        .run(
            &format!("mkdir -p {ORACLE_DIR}"),
            Duration::from_secs(limits.command_seconds),
        )
        .await;
    for step in 1..=limits.steps {
        if Instant::now() >= limits.deadline {
            report.stopped = Some("the run's time limit".to_string());
            break;
        }
        if used >= limits.usd_left {
            report.stopped = Some("the run's spend limit".to_string());
            break;
        }
        report.steps = step;
        let left = limits.steps - step;
        if left == 0 {
            sub.notes.push(format!(
                "This is the session's last step (step {step} of {}): write the check scripts \
under {ORACLE_DIR} in this step's commands and set finished to true. Checks that aren't written \
now are lost.",
                limits.steps
            ));
        } else if left == 1 {
            sub.notes.push(format!(
                "Step {step} of {}: one step is left after this one. Write the check scripts \
under {ORACLE_DIR} now, and use the last step only to fix them.",
                limits.steps
            ));
        } else if step == 1 {
            sub.notes.push(format!(
                "The session has {} steps. Read what you need in the first few, then write the \
scripts; a check that isn't written by the last step is lost.",
                limits.steps
            ));
        }
        let text = crate::run::prompt(&sub, ORACLE_PROMPT, "None in this session.", None, false);
        let generated = generator.generate(&system, &text).await;
        spend.generated(&generated, &format!("oracle step {step} model"));
        used += generated.known_usd;
        let action = match &generated.action {
            Ok(action) => {
                bad = 0;
                sub.notes.clear();
                action.clone()
            }
            Err(error) => {
                on_step(&OracleStep {
                    step,
                    generated: generated.clone(),
                    results: Vec::new(),
                });
                bad += 1;
                if bad >= 3 {
                    report.stopped = Some(format!("replies didn't match the format: {error}"));
                    break;
                }
                sub.notes.push(format!(
                    "Step {step}'s reply couldn't be used ({}); reply with the JSON object the \
format asks for.",
                    cut(error, 300, 0)
                ));
                continue;
            }
        };
        let mut results = Vec::new();
        let mut skipped = Vec::new();
        let mut failed = false;
        for command in &action.commands {
            if failed {
                skipped.push(command.clone());
                continue;
            }
            let result = env
                .run(command, Duration::from_secs(limits.command_seconds))
                .await;
            failed = !result.ok();
            results.push(result);
        }
        on_step(&OracleStep {
            step,
            generated,
            results: results.clone(),
        });
        sub.actions.push(Action {
            step,
            rationale: action.rationale.clone(),
            results,
            skipped,
        });
        let paths: Vec<String> = if action.view.is_empty() {
            sub.files.iter().map(|(path, _)| path.clone()).collect()
        } else {
            action.view.clone()
        };
        sub.files = crate::run::read_view(env, &paths).await;
        if action.finished {
            break;
        }
    }
    let deadline = Duration::from_secs(limits.command_seconds);
    let mut tests = load_scripts(env, ORACLE_DIR, ORACLE_PREFIX, deadline).await;
    report.written = tests.iter().map(|t| t.name.clone()).collect();
    let mut kept = Vec::new();
    for mut test in tests.drain(..) {
        let result = env
            .run(&test.script, Duration::from_secs(limits.test_seconds))
            .await;
        if result.ok() {
            report.trivial.push(test.name.clone());
        } else {
            test.passed_at_freeze = Some(false);
            report.kept.push(test.name.clone());
            kept.push(test);
        }
    }
    (kept, report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statements_are_sentences_items_and_fenced_blocks() {
        let task = "# Heading here\n\nWrite the report to /app/out.csv. It must have three \
columns: id, score, and rank.\n\n- Scores are rounded to 2 decimals.\n2. Rank ties by id \
ascending order.\n\n```\nid,score,rank\n1,0.50,1\n```\nShort line.";
        let got = statements(task);
        assert_eq!(
            got,
            [
                "Write the report to /app/out.csv.",
                "It must have three columns: id, score, and rank.",
                "Scores are rounded to 2 decimals.",
                "Rank ties by id ascending order.",
                "id,score,rank\n1,0.50,1",
            ]
        );
    }

    #[test]
    fn decimals_and_paths_dont_split_a_sentence() {
        let got = sentences("Keep 0.5 of the mass. Values like e.g. x.y stay whole.");
        assert_eq!(
            got,
            ["Keep 0.5 of the mass.", "Values like e.g. x.y stay whole."]
        );
    }
}
