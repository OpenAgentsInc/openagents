//! Verifying a written suite: red first by code, then Jev's faithfulness,
//! hardcoding, triviality, and coverage judgments.
//!
//! Every judgment is a Noul. The thresholds live in [`super::Options`];
//! none is calibrated yet, so the record keeps every probability and a
//! later fit can reread them without asking again.

use std::collections::BTreeMap;
use std::path::Path;

use futures_util::future::join_all;
use jev::{Noul, NoulCriteria, Questions};
use serde_json::{Value, json};

use super::runner::Runner;
use super::{Coverage, Gap, Inputs, Options, Rejected, Test, TestRun, read_tests, sha256};
use crate::component::jev::{self as jev_component, Ask, Asked, JevMode, USD_PER_MILLION_INPUT};
use crate::record::Recorder;

/// Does the test assert only what the task states?
pub const FAITHFUL: &str = "The acceptance test in `test` was written before the task in `task` was solved, to check the requirements in `requirements`. Does every assertion in the test follow from what the task states, from its examples, or from its rules, with nothing the task doesn't ask for, and does every input the test builds follow the input format the task describes?";

/// Does the test hardcode an answer the task doesn't give?
pub const HARDCODED: &str = "The acceptance test in `test` checks the requirements in `requirements` of the task in `task`. A test may build its own small inputs and expect the output the task's rules give for them, or compute the expected output with its own implementation of the task's rules; that is not hardcoding. Does the test instead expect a specific value for a case the task's words, examples, and rules don't determine, such as a guessed answer for the task's own data or a choice the task leaves open?";

/// Could the test pass without its requirement met?
pub const TRIVIAL: &str = "Does the acceptance test in `test` check almost nothing of substance about the requirements in `requirements` of the task in `task`, for example only that a file or a name exists, that a command runs, or that some output appears, so that nearly any attempt would pass it?";

/// Does the requirement keep something already true?
pub const KEEPS: &str = "Read the requirements in `requirements` of the task in `task`. Do they ask to keep something that is already true of the existing workspace, such as leaving a file unchanged or keeping existing behavior working, rather than to create, fix, or change something?";

/// Do the requirement's tests decide it?
pub const DECIDES: &str = "The acceptance tests in `tests` were written to check the requirement in `requirement` of the task in `task`. If a solution met the rest of the task but not this requirement, would at least one of these tests fail?";

/// Do the requirement's tests check its rule exactly, not a
/// simplification? A cheap model's commonest failure is reading the fact
/// that decides a test and then applying a simpler rule.
pub const EXACT: &str = "The task in `task` states the requirement in `requirement`, and `facts` lists the exact formats, edge cases, units, and rules the suite's writer found for it. Do the tests in `tests` check this requirement's rule exactly as the task states it, including those facts, rather than a simpler or more lenient version of it?";

/// Does the test assert only what the task states, counting the standard
/// definition of a method the task names as stated?
pub const FAITHFUL_STANDARD: &str = "The acceptance test in `test` was written before the task in `task` was solved, to check the requirements in `requirements`. Count as stated by the task its words, its examples, its rules, and the standard definition and textbook properties of any method, statistic, estimator, metric, algorithm, or format the task names, even where the code's comments defend something else. Does every assertion in the test follow from what the task states in that sense, with nothing the task doesn't ask for, and does every input the test builds follow the input format the task describes?";

/// [`FAITHFUL_STANDARD`] without the words of any one task's defect: the
/// standard definition of a well-known method the task names counts as
/// stated.
pub const FAITHFUL_GENERAL: &str = "The acceptance test in `test` was written before the task in `task` was solved, to check the requirements in `requirements`. Count as stated by the task its words, its examples, its rules, and the standard definition of any well-known method, algorithm, protocol, or format the task names. Does every assertion in the test follow from what the task states in that sense, with nothing the task doesn't ask for, and does every input the test builds follow the input format the task describes?";

/// The faithfulness question `options` ask.
#[must_use]
pub fn faithful_question(options: &super::Options) -> &'static str {
    match (options.standard_methods, options.general) {
        (true, true) => FAITHFUL_GENERAL,
        (true, false) => FAITHFUL_STANDARD,
        (false, _) => FAITHFUL,
    }
}

/// Does at least one test fail on a module's current behavior?
#[must_use]
pub fn module_question(index: usize) -> String {
    format!(
        "The task in `task` asks for a fix across several modules. `modules[{index}]` is one of \
         them, with its current source, and `tests` are the acceptance tests written for the \
         task. Does at least one test in `tests` fail on the current behavior of the module in \
         `modules[{index}]`, because of a defect in that module?"
    )
}

/// The per-test question set; with `standard`, the faithfulness question
/// counts the standard definition of a method the task names as stated.
#[must_use]
pub fn test_questions(standard: bool) -> Questions {
    test_questions_with(if standard {
        FAITHFUL_STANDARD
    } else {
        FAITHFUL
    })
}

/// The per-test question set with `faithful` as the faithfulness question.
#[must_use]
pub fn test_questions_with(faithful: &str) -> Questions {
    let noul = |text: &str, yes: &str, no: &str| {
        Noul::with_criteria(text, NoulCriteria::new().when_true(yes).when_false(no))
    };
    Questions::new()
        .with(
            "faithful",
            noul(
                faithful,
                "every assertion follows from the task's words, examples, or rules",
                "some assertion checks something the task doesn't state or imply",
            ),
        )
        .with(
            "hardcoded",
            noul(
                HARDCODED,
                "the test expects a specific answer the task doesn't give or let you derive",
                "every expected value is stated by the task, derived from its rules, or a property any correct answer has",
            ),
        )
        .with(
            "trivial",
            noul(
                TRIVIAL,
                "the test could pass with the requirements unmet",
                "the test fails whenever the requirements are unmet",
            ),
        )
        .with(
            "keeps",
            noul(
                KEEPS,
                "the requirements ask to keep something already true",
                "the requirements ask to create, fix, or change something",
            ),
        )
}

/// The per-requirement question set.
#[must_use]
pub fn coverage_questions() -> Questions {
    Questions::new().with(
        "decides",
        Noul::with_criteria(
            DECIDES,
            NoulCriteria::new()
                .when_true("a solution missing this requirement fails at least one of the tests")
                .when_false(
                    "a solution missing this requirement could pass every one of the tests",
                ),
        ),
    )
    .with(
        "exact",
        Noul::with_criteria(
            EXACT,
            NoulCriteria::new()
                .when_true("the tests check the stated rule exactly, so a simplified rule fails them")
                .when_false("a solution applying a simplified or lenient version of the rule could pass the tests"),
        ),
    )
}

/// A digest of every question's wording, for the record.
#[must_use]
pub fn question_digest() -> String {
    sha256(
        [FAITHFUL, HARDCODED, TRIVIAL, KEEPS, DECIDES]
            .join("\n")
            .as_bytes(),
    )
}

/// Jev's answers already given in earlier rounds, by the digest of the
/// state and the question set, so an unchanged test isn't asked twice.
#[derive(Default)]
pub struct Cache(BTreeMap<String, Asked>);

/// What one verification found.
#[derive(Clone, Debug, Default)]
pub struct Verified {
    pub tests: Vec<Test>,
    /// Each test's run on the untouched workspace.
    pub start: Vec<TestRun>,
    pub start_ms: u64,
    /// Header problems: a test and why.
    pub headers: Vec<(String, String)>,
    pub rejected: Vec<Rejected>,
    pub coverage: Vec<Coverage>,
    pub gaps: Vec<Gap>,
    /// Per test: Jev's answers, the start run, and the reasons.
    pub judged: Value,
    /// The problems as the next writing session reads them.
    pub messages: Vec<String>,
    pub jev_requests: usize,
    pub jev_usd: f64,
    /// Per test, Jev's doubts that didn't reject it, with
    /// `rewrite: "hard"`.
    pub notes: BTreeMap<String, Vec<String>>,
}

impl Verified {
    /// The problems for the next round; empty when the suite is accepted.
    #[must_use]
    pub fn problems(&self) -> Vec<String> {
        self.messages.clone()
    }
}

/// Whether a test's source can't fail: nothing but existence checks,
/// `true`, `exit 0`, and setup.
#[must_use]
pub fn statically_trivial(source: &str) -> bool {
    let lines: Vec<&str> = source
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    if lines.is_empty() {
        return true;
    }
    let harmless = |line: &str| {
        let line = line.trim_end_matches(';').trim();
        [
            "test -e ", "test -f ", "test -d ", "test -s ", "[ -e ", "[ -f ", "[ -d ", "[ -s ",
            "cd ", "set -", "export ",
        ]
        .iter()
        .any(|prefix| line.starts_with(prefix))
            || ["true", ":", "exit 0", "exit"].contains(&line)
    };
    lines.iter().all(|line| harmless(line))
}

/// Why a red test at the start is red for a reason of its own: a missing
/// interpreter or test framework, a syntax error, or a runner failure.
#[must_use]
pub fn broken_reason(run: &TestRun) -> Option<String> {
    let out = &run.output;
    if out.starts_with("[runner]") {
        return Some(out.clone());
    }
    for tool in [
        "python3", "python", "pytest", "node", "bun", "perl", "ruby", "jq", "bc", "awk",
    ] {
        for form in [
            format!("{tool}: not found"),
            format!("{tool}: command not found"),
        ] {
            if out.contains(&form) {
                return Some(format!("the workspace has no `{tool}`"));
            }
        }
    }
    if out.contains("No module named 'pytest'") {
        return Some("the workspace has no pytest".to_string());
    }
    for marker in [
        "SyntaxError",
        "IndentationError",
        "Syntax error",
        "syntax error",
    ] {
        if out.contains(marker) {
            return Some(format!("its own code has a syntax error ({marker})"));
        }
    }
    None
}

/// A test's source followed by every helper file under the suite
/// directory it names, such as `lib/check.py`, so a judgment reads what
/// the test runs.
#[must_use]
pub fn with_helpers(dir: &Path, source: &str) -> String {
    let mut out = source.to_string();
    for path in super::digest_files(dir).keys() {
        if path.starts_with(super::TESTS_DIR)
            || path.starts_with(super::REJECTED_DIR)
            || super::HARNESS.contains(&path.as_str())
            || !source.contains(path.as_str())
        {
            continue;
        }
        if let Ok(text) = std::fs::read_to_string(dir.join(path)) {
            out.push_str(&format!("\n# ---- {path} ----\n{text}"));
        }
    }
    out
}

/// The lines of `facts` that name `id`, such as `R3: dates are UTC`.
#[must_use]
pub fn facts_for(facts: &str, id: &str) -> Vec<String> {
    facts
        .lines()
        .map(|l| l.trim().trim_start_matches(['-', '*', ' ']))
        .filter(|l| {
            l.strip_prefix(id)
                .is_some_and(|rest| rest.starts_with([':', ' ', ',', ')']))
                || l.contains(&format!("({id})"))
        })
        .map(|l| crate::judge::clip(l, 400))
        .take(12)
        .collect()
}

fn requirement_state(inputs: &Inputs<'_>, ids: &[String]) -> Value {
    Value::Array(
        inputs
            .requirements
            .requirements
            .iter()
            .filter(|r| ids.contains(&r.id))
            .map(|r| {
                json!({
                    "id": r.id,
                    "kind": r.kind.word(),
                    "text": r.text.split_whitespace().collect::<Vec<_>>().join(" "),
                })
            })
            .collect(),
    )
}

async fn ask_cached(
    jev: &JevMode,
    recorder: &Recorder,
    cache: &Cache,
    name: &'static str,
    id: String,
    state: Value,
    questions: Questions,
) -> (String, Option<Asked>) {
    let key = sha256(format!("{name}\n{state}").as_bytes());
    if let Some(hit) = cache.0.get(&key) {
        return (key, Some(hit.clone()));
    }
    let asked = jev_component::ask(
        jev,
        recorder,
        Ask {
            component: super::DEFINE_COMPONENT,
            name,
            id,
            state,
            questions,
            parent: None,
            deadline: None,
        },
    )
    .await;
    (key, Some(asked))
}

fn usd(asked: &Asked) -> f64 {
    asked
        .input_tokens
        .map_or(0.0, |t| t as f64 * USD_PER_MILLION_INPUT / 1_000_000.0)
}

/// Verifies the suite in `dir`: reads its tests, runs them on the
/// untouched workspace, asks Jev about each test and each requirement,
/// and decides what to reject and what's missing.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub async fn verify<R: Runner>(
    inputs: &Inputs<'_>,
    dir: &Path,
    known: &[String],
    runner: &R,
    jev: &JevMode,
    recorder: &Recorder,
    options: &Options,
    cache: &mut Cache,
    round: u32,
    modules: &[String],
) -> Verified {
    let (tests, headers) = read_tests(dir, known);
    let started = std::time::Instant::now();
    let start = runner.run_all(&tests, dir, inputs.workspace).await;
    let start_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let task = crate::judge::clip(inputs.task.instruction.trim(), 6_000);
    let mut out = Verified {
        tests: tests.clone(),
        start: start.clone(),
        start_ms,
        headers: headers.clone(),
        ..Verified::default()
    };

    // Per-test judgments, a few requests at a time.
    let mut answers: BTreeMap<String, Asked> = BTreeMap::new();
    let judgeable: Vec<&Test> = tests
        .iter()
        .filter(|t| !t.requirements.is_empty())
        .collect();
    for chunk in judgeable.chunks(options.jev_parallel.max(1)) {
        let asks = chunk.iter().map(|test| {
            let state = json!({
                "task": task,
                "requirements": requirement_state(inputs, &test.requirements),
                "test": {
                    "id": test.id,
                    "kind": test.kind,
                    "what": test.what,
                    "source": crate::judge::clip(&with_helpers(dir, &test.source), 6_000),
                },
            });
            ask_cached(
                jev,
                recorder,
                cache,
                "jev_accept_test",
                format!("jev-accept-test-{round}-{}", test.id),
                state,
                test_questions_with(faithful_question(options)),
            )
        });
        for ((key, asked), test) in join_all(asks).await.into_iter().zip(chunk) {
            if let Some(asked) = asked {
                if asked.how == "live" {
                    out.jev_requests += 1;
                    out.jev_usd += usd(&asked);
                }
                if asked.answered() {
                    cache.0.insert(key, asked.clone());
                }
                answers.insert(test.id.clone(), asked);
            }
        }
    }

    // Reasons per test.
    let mut judged = serde_json::Map::new();
    let mut messages = Vec::new();
    // With `rewrite: "hard"`, only what code checks sends the suite back:
    // Jev's doubts become notes on the test, which the edit sessions read.
    let hard = options.rewrite == super::Rewrite::Hard;
    if !hard {
        for (id, why) in &headers {
            messages.push(format!("{id}: {why}. Name the requirement IDs it decides."));
        }
    }
    for (index, test) in tests.iter().enumerate() {
        let run = start.iter().find(|r| r.id == test.id);
        let asked = answers.get(&test.id);
        let p = |q: &str| asked.and_then(|a| a.noul(q));
        let (faithful, hardcoded, trivial, keeps) =
            (p("faithful"), p("hardcoded"), p("trivial"), p("keeps"));
        let mut reasons: Vec<String> = Vec::new();
        let mut notes: Vec<String> = Vec::new();
        if test.requirements.is_empty() {
            reasons.push("no_requirement".to_string());
        }
        if index >= options.max_tests {
            reasons.push("over_bound".to_string());
            if !hard {
                messages.push(format!(
                    "{} is over the bound of {} tests: fold it into another test or delete it.",
                    test.id, options.max_tests
                ));
            }
        }
        let green_at_start = run.is_some_and(|r| r.green);
        if green_at_start && options.guards {
            notes.push(
                "A guard: it passes on the untouched workspace, so it checks behavior that \
                 already works, and it must stay green."
                    .to_string(),
            );
        } else if green_at_start && keeps.is_none_or(|k| k < options.keeps_min) {
            reasons.push("green_at_start".to_string());
            messages.push(format!(
                "{} passes on the untouched workspace, but its requirements ({}) ask for a change: \
                 make it fail until the work is done.",
                test.id,
                test.requirements.join(", ")
            ));
        }
        if let Some(why) = run.filter(|r| !r.green).and_then(broken_reason) {
            reasons.push("broken".to_string());
            messages.push(format!(
                "{} fails for a reason of its own ({why}), not because the work is missing: \
                 fix the test.\n{}",
                test.id,
                crate::judge::clip_tail(&run.map(|r| r.output.clone()).unwrap_or_default(), 600)
            ));
        }
        if faithful.is_some_and(|f| f < options.faithful_min) {
            if hard {
                notes.push(format!(
                    "Jev reads it as possibly asserting something the task doesn't state ({:.2} \
                     that it asserts only what the task states); where it disagrees with the task, \
                     follow the task.",
                    faithful.unwrap_or_default()
                ));
            } else {
                reasons.push("unfaithful".to_string());
                messages.push(format!(
                    "{} may assert something the task doesn't state (Jev: {:.2} that it asserts \
                     only what the task states): keep only assertions the task's words, \
                     examples, or rules support.",
                    test.id,
                    faithful.unwrap_or_default()
                ));
            }
        }
        if hardcoded.is_some_and(|h| h >= options.hardcoded_max) {
            if hard {
                notes.push(format!(
                    "Jev reads it as possibly expecting a value the task doesn't give ({:.2}).",
                    hardcoded.unwrap_or_default()
                ));
            } else {
                reasons.push("hardcoded".to_string());
                messages.push(format!(
                    "{} may hardcode an answer the task doesn't give (Jev: {:.2}): compute the \
                     expected value from the task's rules, or check a property any correct \
                     answer has.",
                    test.id,
                    hardcoded.unwrap_or_default()
                ));
            }
        }
        let static_trivial = statically_trivial(&test.source);
        let jev_trivial = trivial.is_some_and(|t| t >= options.trivial_max);
        if hard && jev_trivial && !static_trivial {
            notes.push(format!(
                "Jev reads it as possibly passing without its requirements met ({:.2}).",
                trivial.unwrap_or_default()
            ));
        }
        if static_trivial || (jev_trivial && !hard) {
            reasons.push("trivial".to_string());
            messages.push(format!(
                "{} could pass without its requirements met{}: make it check the behavior itself.",
                test.id,
                if static_trivial {
                    " (it only checks existence or can't fail)".to_string()
                } else {
                    format!(" (Jev: {:.2})", trivial.unwrap_or_default())
                }
            ));
        }
        if !notes.is_empty() {
            out.notes.insert(test.id.clone(), notes.clone());
        }
        judged.insert(
            test.id.clone(),
            json!({
                "requirements": test.requirements,
                "faithful": faithful,
                "hardcoded": hardcoded,
                "trivial": trivial,
                "static_trivial": static_trivial,
                "keeps": keeps,
                "green_at_start": green_at_start,
                "start_exit": run.and_then(|r| r.exit),
                "jev": asked.map(|a| a.how),
                "key": asked.map(|a| a.key.clone()),
                "reasons": reasons,
                "notes": notes,
            }),
        );
        if !reasons.is_empty() {
            out.rejected.push(Rejected {
                id: test.id.clone(),
                requirements: test.requirements.clone(),
                reasons,
            });
        }
    }

    // Coverage per requirement, over the tests that survived.
    let surviving: Vec<&Test> = tests
        .iter()
        .filter(|t| !out.rejected.iter().any(|r| r.id == t.id))
        .collect();
    let decidable = super::decidable(inputs.requirements);
    let facts = std::fs::read_to_string(dir.join(super::FACTS)).unwrap_or_default();
    if facts.trim().is_empty() && !tests.is_empty() && !hard {
        messages.push(format!(
            "{} is missing or empty: list the decisive facts there first, one per line as \
             `R3: fact`, and encode each as a test.",
            super::FACTS
        ));
    }
    let mut coverage_answers: BTreeMap<String, Asked> = BTreeMap::new();
    let with_tests: Vec<_> = decidable
        .iter()
        .filter(|r| surviving.iter().any(|t| t.requirements.contains(&r.id)))
        .collect();
    for chunk in with_tests.chunks(options.jev_parallel.max(1)) {
        let asks = chunk.iter().map(|requirement| {
            let mine: Vec<Value> = surviving
                .iter()
                .filter(|t| t.requirements.contains(&requirement.id))
                .map(|t| {
                    json!({
                        "id": t.id,
                        "what": t.what,
                        "source": crate::judge::clip(&t.source, 2_500),
                    })
                })
                .collect();
            let state = json!({
                "task": task,
                "requirement": {
                    "id": requirement.id,
                    "kind": requirement.kind.word(),
                    "text": requirement.text.split_whitespace().collect::<Vec<_>>().join(" "),
                },
                "tests": mine,
                "facts": facts_for(&facts, &requirement.id),
            });
            ask_cached(
                jev,
                recorder,
                cache,
                "jev_accept_coverage",
                format!("jev-accept-coverage-{round}-{}", requirement.id),
                state,
                coverage_questions(),
            )
        });
        for ((key, asked), requirement) in join_all(asks).await.into_iter().zip(chunk) {
            if let Some(asked) = asked {
                if asked.how == "live" {
                    out.jev_requests += 1;
                    out.jev_usd += usd(&asked);
                }
                if asked.answered() {
                    cache.0.insert(key, asked.clone());
                }
                coverage_answers.insert(requirement.id.clone(), asked);
            }
        }
    }
    let mut coverage_judged = serde_json::Map::new();
    let mut untested: Vec<String> = Vec::new();
    for requirement in &decidable {
        let mine: Vec<String> = surviving
            .iter()
            .filter(|t| t.requirements.contains(&requirement.id))
            .map(|t| t.id.clone())
            .collect();
        let decides = coverage_answers
            .get(&requirement.id)
            .and_then(|a| a.noul("decides"));
        let exact = coverage_answers
            .get(&requirement.id)
            .and_then(|a| a.noul("exact"));
        let decided = decides.is_none_or(|d| d >= options.decides_min);
        let covered = !mine.is_empty() && decided && exact.is_none_or(|e| e >= options.exact_min);
        let only_guards = options.guards
            && !mine.is_empty()
            && mine
                .iter()
                .all(|id| start.iter().any(|r| r.id == *id && r.green));
        if mine.is_empty() {
            out.gaps.push(super::Gap {
                requirement: requirement.id.clone(),
                why: "no accepted test names it".to_string(),
            });
            untested.push(requirement.id.clone());
        } else if only_guards {
            out.gaps.push(super::Gap {
                requirement: requirement.id.clone(),
                why: "only guards: every test of it passes on the untouched workspace".to_string(),
            });
        } else if decided && !covered {
            out.gaps.push(super::Gap {
                requirement: requirement.id.clone(),
                why: format!(
                    "Jev reads its tests as checking a simpler rule than the task states ({:.2})",
                    exact.unwrap_or_default()
                ),
            });
            let note = format!(
                "Jev reads {}'s tests as possibly checking a simpler rule than the task states \
                 ({:.2} that they check it exactly): hold the code to the task's exact rule, not \
                 to a simpler reading of this test.",
                requirement.id,
                exact.unwrap_or_default()
            );
            if hard {
                for id in &mine {
                    out.notes.entry(id.clone()).or_default().push(note.clone());
                }
            } else {
                messages.push(format!(
                    "{}'s tests ({}) may check a simplified version of its rule (Jev: {:.2} that \
                     they check it exactly): add a test that fails for the simpler reading, using \
                     the exact format, edge case, unit, or rule the task states.",
                    requirement.id,
                    mine.join(", "),
                    exact.unwrap_or_default()
                ));
            }
        } else if !covered {
            out.gaps.push(super::Gap {
                requirement: requirement.id.clone(),
                why: format!(
                    "Jev reads its tests as not deciding it ({:.2})",
                    decides.unwrap_or_default()
                ),
            });
            if !hard {
                messages.push(format!(
                    "{}'s tests ({}) may not decide it (Jev: {:.2} that a solution missing it \
                     fails one): add a test that fails exactly when {} is not met.",
                    requirement.id,
                    mine.join(", "),
                    decides.unwrap_or_default(),
                    requirement.id
                ));
            }
        }
        coverage_judged.insert(
            requirement.id.clone(),
            json!({
                "decides": decides,
                "exact": exact,
                "jev": coverage_answers.get(&requirement.id).map(|a| a.how),
                "key": coverage_answers.get(&requirement.id).map(|a| a.key.clone()),
            }),
        );
        out.coverage.push(Coverage {
            id: requirement.id.clone(),
            kind: requirement.kind.word().to_string(),
            tests: mine,
            decides,
            exact,
            covered,
        });
    }
    // Each output the task names, and the untouched workspace lacks, needs
    // a test that checks it where the task names it: a suite that only
    // writes to its scratch never sees the deliverable.
    let outputs = super::named_outputs(&inputs.task.instruction, inputs.workspace, None);
    let unchecked: Vec<&String> = outputs
        .iter()
        .filter(|output| {
            !surviving
                .iter()
                .any(|t| with_helpers(dir, &t.source).contains(output.as_str()))
        })
        .collect();
    if !unchecked.is_empty() && !tests.is_empty() {
        messages.push(format!(
            "No test checks {}, the output{} the task names, where the task names {}: add a test \
             that runs the program the way the task says and checks what it wrote there.",
            unchecked
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", "),
            if unchecked.len() == 1 { "" } else { "s" },
            if unchecked.len() == 1 { "it" } else { "them" }
        ));
        for output in &unchecked {
            out.gaps.push(super::Gap {
                requirement: (*output).clone(),
                why: "no test checks this output where the task names it".to_string(),
            });
        }
    }
    // The inventory: each module needs a test that names it, or a waiver.
    let waived = super::waived(&facts, modules);
    let mut unnamed: Vec<String> = Vec::new();
    let mut inventory_rows: Vec<Value> = Vec::new();
    let mut named: Vec<(String, Vec<String>)> = Vec::new();
    for module in modules {
        let naming: Vec<String> = surviving
            .iter()
            .filter(|t| {
                !crate::micro::parallel::files_named(
                    &with_helpers(dir, &t.source),
                    std::slice::from_ref(module),
                )
                .is_empty()
            })
            .map(|t| t.id.clone())
            .collect();
        let waive = waived.contains(module);
        if naming.is_empty() && !waive {
            unnamed.push(module.clone());
            out.gaps.push(super::Gap {
                requirement: module.clone(),
                why: "no test names this inventory module, and facts.md doesn't waive it"
                    .to_string(),
            });
        }
        if !naming.is_empty() {
            named.push((module.clone(), naming.clone()));
        }
        inventory_rows.push(json!({ "module": module, "tests": naming, "waived": waive }));
    }
    if !unnamed.is_empty() {
        messages.push(format!(
            "No test names {} and facts.md doesn't waive {}: for each, add a test that fails on \
             its current behavior and names the module, or a `{} path: why` line in facts.md.",
            unnamed.join(", "),
            if unnamed.len() == 1 { "it" } else { "them" },
            super::WAIVE
        ));
    }
    // One Jev request: does a test fail on each named module's current
    // behavior? A doubt becomes a note on the tests that name it.
    if !named.is_empty() {
        let state = json!({
            "task": task,
            "modules": named.iter().map(|(module, _)| json!({
                "path": module,
                "source": crate::judge::clip(
                    &std::fs::read_to_string(inputs.workspace.join(module)).unwrap_or_default(),
                    2_500
                ),
            })).collect::<Vec<_>>(),
            "tests": surviving.iter().map(|t| json!({
                "id": t.id,
                "what": t.what,
                "source": crate::judge::clip(&t.source, 1_500),
            })).collect::<Vec<_>>(),
        });
        let mut questions = Questions::new();
        for index in 0..named.len() {
            questions = questions.with(
                format!("module_{index}"),
                Noul::with_criteria(
                    module_question(index).as_str(),
                    NoulCriteria::new()
                        .when_true("a test fails because of a defect in this module")
                        .when_false("no test would fail because of a defect in this module"),
                ),
            );
        }
        let (key, asked) = ask_cached(
            jev,
            recorder,
            cache,
            "jev_accept_inventory",
            format!("jev-accept-inventory-{round}"),
            state,
            questions,
        )
        .await;
        if let Some(asked) = asked {
            if asked.how == "live" {
                out.jev_requests += 1;
                out.jev_usd += usd(&asked);
            }
            if asked.answered() {
                cache.0.insert(key, asked.clone());
            }
            for (index, (module, naming)) in named.iter().enumerate() {
                let p = asked.noul(&format!("module_{index}"));
                if let Some(row) = inventory_rows
                    .iter_mut()
                    .find(|row| row["module"] == module.as_str())
                {
                    row["fails_on_current"] = json!(p);
                }
                if p.is_some_and(|p| !crate::decision::VERIFY_FAILS_ON_CURRENT.yes(p)) {
                    let note = format!(
                        "Jev doubts any test fails on the current behavior of {module} ({:.2}): \
                         read that module for its defect.",
                        p.unwrap_or_default()
                    );
                    for id in naming {
                        out.notes.entry(id.clone()).or_default().push(note.clone());
                    }
                }
            }
        }
    }
    if !untested.is_empty() && !hard {
        messages.push(format!(
            "No accepted test decides {}: write tests that fail until each is met, or fix the \
             rejected tests above that name them.",
            untested.join(", ")
        ));
    }
    if tests.is_empty() {
        messages.insert(
            0,
            "The suite has no tests: write them under tests/ as tests/T1.sh and so on.".to_string(),
        );
    }
    out.judged =
        json!({ "tests": judged, "coverage": coverage_judged, "inventory": inventory_rows });
    out.messages = messages;
    out
}
