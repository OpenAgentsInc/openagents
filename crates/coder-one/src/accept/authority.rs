//! Authority classes: how far each acceptance test may be believed
//! (issue #9629).
//!
//! In v6 to v8 a Luna-written suite ran the loop, and its tests reversed
//! correct fixes: a guard that passed on the untouched code pinned the
//! defect, and a writer's wrong expected value stopped the loop red. The
//! 2026-09-25 assessment's answer is to rank tests, not drop them. Every
//! test gets a class from how its expected value is supported, and the
//! loop's power over the code follows the class:
//!
//! | Class | Support | Power |
//! | --- | --- | --- |
//! | [`Authority::ExecutedContract`] | The task's own command or example, run | Can hold the loop red; green is necessary, not sufficient |
//! | [`Authority::IndependentlySupported`] | Recomputed by a separate route or a reference tool | The same |
//! | [`Authority::WriterDerived`] | Luna wrote it, Jev judged it faithful, and it fails on the untouched code | Ranks candidates; never reverses an edit or stops the loop |
//! | [`Authority::Guard`] | Passed on the untouched code | Advisory: a regression marks a suspect |
//!
//! A test that fits none of these is [`Authority::Unsupported`] and has
//! no power at all.
//!
//! Code decides what code can see: whether the test passed on the
//! untouched workspace, and whether an executed-contract extractor
//! (issue #9628) supplied it. Two narrow Jev questions decide the rest:
//! whether each expected value comes from a route separate from the code
//! under test, and whether each expected value is what the task's rule
//! gives. Every probability is kept as evidence ([`Evidence`]), so a later
//! fit can reread it without asking again.
//!
//! A class holds power in a policy only once it has passed the offline
//! bar ([`PROMOTED`]); `accept validity --authority` measures that bar.

use std::collections::BTreeMap;

use futures_util::future::join_all;
use jev::{Noul, NoulCriteria, Questions};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{AcceptanceSuite, RunResult, Task};
use crate::component::jev::{self as jev_component, Ask, JevMode, USD_PER_MILLION_INPUT};
use crate::record::Recorder;

/// The schema of a classification record ([`Record`]).
pub const SCHEMA: &str = "openagents.coder-one.acceptance-authority.v1";

/// How an acceptance test's expected value is supported.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Authority {
    /// The task's own command or example, run; supplied by an
    /// executed-contract extractor (issue #9628).
    ExecutedContract,
    /// Red on the untouched code, and every expected value comes from a
    /// route separate from the code under test and is what the task's
    /// rule gives.
    IndependentlySupported,
    /// Red on the untouched code and judged faithful to the task, with no
    /// independent support for its expected values.
    WriterDerived,
    /// Green on the untouched code.
    Guard,
    /// None of the above: never run on the untouched code, or not judged
    /// faithful.
    Unsupported,
}

impl Authority {
    /// Every class, strongest first.
    pub const ALL: [Authority; 5] = [
        Authority::ExecutedContract,
        Authority::IndependentlySupported,
        Authority::WriterDerived,
        Authority::Guard,
        Authority::Unsupported,
    ];

    /// The class as records and the command line spell it.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Authority::ExecutedContract => "executed_contract",
            Authority::IndependentlySupported => "independently_supported",
            Authority::WriterDerived => "writer_derived",
            Authority::Guard => "guard",
            Authority::Unsupported => "unsupported",
        }
    }

    /// The class a word names.
    ///
    /// # Errors
    ///
    /// A message for any other word.
    pub fn parse(word: &str) -> Result<Authority, String> {
        Authority::ALL
            .into_iter()
            .find(|a| a.word() == word)
            .ok_or_else(|| {
                format!(
                    "an authority class is one of {}, not {word}",
                    Authority::ALL.map(Authority::word).join(", ")
                )
            })
    }

    /// Whether the class may ever hold the loop red: only the task's own
    /// contract and independently supported expectations.
    #[must_use]
    pub fn can_hold(self) -> bool {
        matches!(
            self,
            Authority::ExecutedContract | Authority::IndependentlySupported
        )
    }

    /// Whether the class may ever rank candidates.
    #[must_use]
    pub fn can_rank(self) -> bool {
        self.can_hold() || self == Authority::WriterDerived
    }
}

/// The classes that passed the offline bar and so may hold power in a
/// policy: none yet. In the 2026-09-25 measurement
/// (`docs/terminal-bench/2026-09-25-tiered-acceptance.md`) no held-out
/// task had graded workspaces that both pass and fail, so no class could
/// show that it separates them on tasks it wasn't fitted on. A policy
/// that names any other class under `hold` or `rank` is refused.
pub const PROMOTED: &[Authority] = &[];

/// What a class rests on. Each field is what code or Jev observed; none
/// is a verdict on its own.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Evidence {
    /// The test passed on the untouched workspace; `None` when it never
    /// ran there.
    pub green_at_start: Option<bool>,
    /// Its exit code on the untouched workspace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_exit: Option<i32>,
    /// Jev's probability that it asserts only what the task states.
    #[serde(default)]
    pub faithful: Option<f64>,
    /// Where `faithful` comes from: `define`, the judgment that accepted
    /// the test, or `classify`, a judgment asked only for its class.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub faithful_from: Option<String>,
    /// Jev's probability, at definition, that it hardcodes an answer the
    /// task doesn't give.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hardcoded: Option<f64>,
    /// Jev's probability that each expected value comes from a route
    /// separate from the code under test ([`SEPARATE_ROUTE`]).
    #[serde(default)]
    pub separate_route: Option<f64>,
    /// Jev's probability that each expected value is what the task's rule
    /// gives for the test's inputs ([`EXPECTED_CORRECT`]).
    #[serde(default)]
    pub expected_correct: Option<f64>,
    /// Reference libraries and tools the test's source calls, found by
    /// code ([`reference_routes`]). Recorded, not required: a test can
    /// recompute a value with its own arithmetic.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub routes: Vec<String>,
    /// The executed-contract item the test runs, as the extractor of
    /// issue #9628 describes it: the command or example, where the task
    /// states it, and the stated result. Its presence makes the test an
    /// executed contract.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract: Option<Value>,
    /// The recorded-answer key of the classification's Jev request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jev_key: Option<String>,
}

/// One test's class and what it rests on.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Classified {
    pub class: Authority,
    /// Why, in a sentence.
    pub why: String,
    pub evidence: Evidence,
}

/// The thresholds [`classify`] reads. None is fitted: the faithfulness and
/// hardcoding bounds are `accept.define`'s own, and the two new bounds
/// are the midpoint.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Thresholds {
    pub faithful_min: f64,
    pub hardcoded_max: f64,
    pub separate_min: f64,
    pub correct_min: f64,
}

impl Default for Thresholds {
    fn default() -> Self {
        let define = super::Options::default();
        Thresholds {
            faithful_min: define.faithful_min,
            hardcoded_max: define.hardcoded_max,
            separate_min: 0.5,
            correct_min: 0.5,
        }
    }
}

/// A test's class from its evidence.
///
/// 1. An executed-contract item makes it [`Authority::ExecutedContract`].
/// 2. Green on the untouched workspace makes it a [`Authority::Guard`],
///    whatever else is true: a guard can pin the defect.
/// 3. Never run there, or not judged faithful, makes it
///    [`Authority::Unsupported`].
/// 4. Both Jev support questions at their bounds, and no sign of a
///    hardcoded answer, make it [`Authority::IndependentlySupported`].
/// 5. Anything else red on the untouched workspace and faithful is
///    [`Authority::WriterDerived`].
#[must_use]
pub fn classify(evidence: &Evidence, t: &Thresholds) -> (Authority, String) {
    if evidence.contract.is_some() {
        return (
            Authority::ExecutedContract,
            "it runs the task's own command or example".to_string(),
        );
    }
    match evidence.green_at_start {
        Some(true) => {
            return (
                Authority::Guard,
                "it passed on the untouched workspace".to_string(),
            );
        }
        None => {
            return (
                Authority::Unsupported,
                "it never ran on the untouched workspace".to_string(),
            );
        }
        Some(false) => {}
    }
    let Some(faithful) = evidence.faithful.filter(|f| *f >= t.faithful_min) else {
        return (
            Authority::Unsupported,
            match evidence.faithful {
                Some(f) => format!("Jev reads it as unfaithful to the task ({f:.2})"),
                None => "Jev never judged it faithful".to_string(),
            },
        );
    };
    let separate = evidence.separate_route.unwrap_or(0.0);
    let correct = evidence.expected_correct.unwrap_or(0.0);
    let hardcoded = evidence.hardcoded.unwrap_or(0.0);
    if separate >= t.separate_min && correct >= t.correct_min && hardcoded < t.hardcoded_max {
        return (
            Authority::IndependentlySupported,
            format!(
                "red on the untouched workspace; Jev reads its expected values as coming from \
                 a separate route ({separate:.2}) and matching the task's rule ({correct:.2})"
            ),
        );
    }
    (
        Authority::WriterDerived,
        format!(
            "red on the untouched workspace and faithful ({faithful:.2}), with no independent \
             support for its expected values (separate route {}, matches the rule {})",
            evidence
                .separate_route
                .map_or("unjudged".to_string(), |p| format!("{p:.2}")),
            evidence
                .expected_correct
                .map_or("unjudged".to_string(), |p| format!("{p:.2}"))
        ),
    )
}

/// Reference libraries and tools a test can recompute an expected value
/// with. Array libraries that every test uses to build inputs aren't on
/// the list: calling one says nothing about where an expectation came
/// from.
pub const REFERENCE_ROUTES: [&str; 16] = [
    "scipy",
    "sklearn",
    "statistics",
    "fractions",
    "decimal",
    "hashlib",
    "difflib",
    "sympy",
    "networkx",
    "sqlite3",
    "sha256sum",
    "md5sum",
    "openssl",
    "jq ",
    "bc ",
    "cmp ",
];

/// The reference routes `source` calls, in list order.
#[must_use]
pub fn reference_routes(source: &str) -> Vec<String> {
    let code: String = source
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    REFERENCE_ROUTES
        .iter()
        .filter(|r| code.contains(**r))
        .map(|r| r.trim().to_string())
        .collect()
}

/// Does every expected value come from a route separate from the code
/// under test?
pub const SEPARATE_ROUTE: &str = "The acceptance test in `test` was written before the task in \
`task` was solved. For every expected value it asserts, does the value come from a source other \
than the code under test and other than the writer's own choice: a value or worked example the \
task states, a reference library or tool that computes the same quantity, or the test's own \
computation of the rule the task states?";

/// Is every expected value what the task's rule gives?
pub const EXPECTED_CORRECT: &str = "The acceptance test in `test` checks a solution to the task \
in `task`. Work out, for the inputs the test builds, what the task's stated rules and the \
standard definition of any method it names give. Is every expected value and every bound the \
test asserts consistent with that?";

/// The classification question set; with `faithful`, the faithfulness
/// question too, for a test `accept.define` never judged.
#[must_use]
pub fn questions(faithful: bool) -> Questions {
    let noul = |text: &str, yes: &str, no: &str| {
        Noul::with_criteria(text, NoulCriteria::new().when_true(yes).when_false(no))
    };
    let mut questions = Questions::new()
        .with(
            "separate_route",
            noul(
                SEPARATE_ROUTE,
                "every expected value is stated by the task or recomputed independently",
                "some expected value is the writer's own choice or copied from the code under test",
            ),
        )
        .with(
            "expected_correct",
            noul(
                EXPECTED_CORRECT,
                "every expected value and bound follows from the task's rule",
                "some expected value or bound contradicts the task's rule",
            ),
        );
    if faithful {
        questions = questions.with(
            "faithful",
            noul(
                super::verify::FAITHFUL_GENERAL,
                "every assertion follows from the task's words, examples, or rules",
                "some assertion checks something the task doesn't state or imply",
            ),
        );
    }
    questions
}

/// The evidence a frozen suite already holds for test `id`: its run on
/// the untouched workspace, and `accept.define`'s Jev answers.
#[must_use]
pub fn recorded(suite: &AcceptanceSuite, id: &str) -> Evidence {
    let start = suite
        .start
        .as_ref()
        .and_then(|s| s.tests.iter().find(|t| t.id == id));
    let judged = &suite.detail["judged"]["tests"][id];
    let faithful = judged["faithful"].as_f64();
    Evidence {
        green_at_start: start.map(|r| r.green),
        start_exit: start.and_then(|r| r.exit),
        faithful,
        faithful_from: faithful.map(|_| "define".to_string()),
        hardcoded: judged["hardcoded"].as_f64(),
        routes: suite
            .tests
            .iter()
            .find(|t| t.id == id)
            .map(|t| reference_routes(&t.source))
            .unwrap_or_default(),
        ..Evidence::default()
    }
}

/// Classifies every test of `suite` by code alone: what [`recorded`]
/// holds, with `contract` naming the tests an executed-contract extractor
/// supplied. A test already classified keeps its class.
pub fn classify_by_code(
    suite: &mut AcceptanceSuite,
    contract: &BTreeMap<String, Value>,
    t: &Thresholds,
) {
    let ids: Vec<String> = suite.tests.iter().map(|t| t.id.clone()).collect();
    for id in ids {
        if suite.authority.contains_key(&id) {
            continue;
        }
        let mut evidence = recorded(suite, &id);
        evidence.contract = contract.get(&id).cloned();
        let (class, why) = classify(&evidence, t);
        suite.authority.insert(
            id,
            Classified {
                class,
                why,
                evidence,
            },
        );
    }
}

/// What classifying a suite cost.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Spent {
    pub jev_requests: usize,
    pub jev_usd: f64,
}

/// Classifies every test of `suite` and records the classes in
/// `suite.authority`, replacing any earlier ones. Jev is asked only about
/// the tests code can't settle: those red on the untouched workspace and
/// not supplied by the extractor. Six requests run at a time.
pub async fn classify_suite(
    suite: &mut AcceptanceSuite,
    task: &Task,
    contract: &BTreeMap<String, Value>,
    jev: &JevMode,
    recorder: &Recorder,
    t: &Thresholds,
) -> Spent {
    let mut spent = Spent::default();
    let instruction = crate::judge::clip(task.instruction.trim(), 6_000);
    let mut evidence: BTreeMap<String, Evidence> = suite
        .tests
        .iter()
        .map(|test| {
            let mut e = recorded(suite, &test.id);
            e.contract = contract.get(&test.id).cloned();
            (test.id.clone(), e)
        })
        .collect();
    let asked: Vec<&super::Test> = suite
        .tests
        .iter()
        .filter(|test| {
            let e = &evidence[&test.id];
            e.contract.is_none() && e.green_at_start == Some(false)
        })
        .collect();
    for chunk in asked.chunks(6) {
        let asks = chunk.iter().map(|test| {
            let need_faithful = evidence[&test.id].faithful.is_none();
            let state = json!({
                "task": instruction,
                "test": {
                    "id": test.id,
                    "kind": test.kind,
                    "what": test.what,
                    "source": crate::judge::clip(
                        &super::verify::with_helpers(&suite.dir, &test.source),
                        6_000
                    ),
                },
            });
            jev_component::ask(
                jev,
                recorder,
                Ask {
                    component: super::DEFINE_COMPONENT,
                    name: "jev_accept_authority",
                    id: format!("jev-accept-authority-{}", test.id),
                    state,
                    questions: questions(need_faithful),
                    parent: None,
                    deadline: None,
                },
            )
        });
        for (answer, test) in join_all(asks).await.into_iter().zip(chunk) {
            if answer.how == "live" {
                spent.jev_requests += 1;
                spent.jev_usd += answer
                    .input_tokens
                    .map_or(0.0, |n| n as f64 * USD_PER_MILLION_INPUT / 1_000_000.0);
            }
            let e = evidence.get_mut(&test.id).expect("every test has evidence");
            e.separate_route = answer.noul("separate_route");
            e.expected_correct = answer.noul("expected_correct");
            if e.faithful.is_none() {
                e.faithful = answer.noul("faithful");
                e.faithful_from = e.faithful.map(|_| "classify".to_string());
            }
            e.jev_key = Some(answer.key.clone());
        }
    }
    suite.authority = evidence
        .into_iter()
        .map(|(id, evidence)| {
            let (class, why) = classify(&evidence, t);
            (
                id,
                Classified {
                    class,
                    why,
                    evidence,
                },
            )
        })
        .collect();
    spent
}

/// Classes for the suites of an offline measurement, keyed by suite
/// digest, as `accept classify` writes them and `accept validity
/// --authority` reads them.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Record {
    pub schema: String,
    pub thresholds: Option<Thresholds>,
    pub suites: BTreeMap<String, SuiteClasses>,
    pub jev_requests: usize,
    pub jev_usd: f64,
}

/// One suite's classes.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SuiteClasses {
    pub task: String,
    /// Where the suite record was read.
    pub record: String,
    pub tests: BTreeMap<String, Classified>,
    pub jev_requests: usize,
    pub jev_usd: f64,
}

impl Record {
    /// Reads a classification record.
    ///
    /// # Errors
    ///
    /// A message when the file doesn't read or has another schema.
    pub fn load(path: &std::path::Path) -> Result<Record, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let record: Record = serde_json::from_str(&text)
            .map_err(|e| format!("{} is not a classification record: {e}", path.display()))?;
        if record.schema != SCHEMA {
            return Err(format!("{} has schema {}", path.display(), record.schema));
        }
        Ok(record)
    }

    /// The class of test `id` in the suite with `digest`.
    #[must_use]
    pub fn class(&self, digest: &str, id: &str) -> Option<Authority> {
        self.suites
            .get(digest)
            .and_then(|s| s.tests.get(id))
            .map(|c| c.class)
    }
}

/// What a run of a classified suite says, class by class: the power each
/// class has over the loop under a policy's `hold` and `rank` lists.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Powers {
    /// Red tests of a holding class: while any is red, the loop can't
    /// stop on a finish.
    pub held: Vec<String>,
    /// Green and total tests of the holding classes.
    pub hold_green: usize,
    pub hold_total: usize,
    /// Green and total tests of the ranking classes that don't hold:
    /// these only break ties between candidates.
    pub rank_green: usize,
    pub rank_total: usize,
    /// Guards red in this run: suspects, never orders.
    pub suspects: Vec<String>,
    /// Red tests with no power in this policy, by class word.
    pub unpowered_red: BTreeMap<String, Vec<String>>,
}

impl Powers {
    /// The powers of `result`'s tests under `hold` and `rank`. A test
    /// without a class counts as [`Authority::Unsupported`].
    #[must_use]
    pub fn of(
        suite: &AcceptanceSuite,
        result: &RunResult,
        hold: &[Authority],
        rank: &[Authority],
    ) -> Powers {
        let mut out = Powers::default();
        for run in &result.tests {
            let class = suite
                .authority
                .get(&run.id)
                .map_or(Authority::Unsupported, |c| c.class);
            if class.can_hold() && hold.contains(&class) {
                out.hold_total += 1;
                if run.green {
                    out.hold_green += 1;
                } else {
                    out.held.push(run.id.clone());
                }
            } else if class.can_rank() && rank.contains(&class) {
                out.rank_total += 1;
                out.rank_green += usize::from(run.green);
            } else if class == Authority::Guard && !run.green {
                out.suspects.push(run.id.clone());
            } else if !run.green {
                out.unpowered_red
                    .entry(class.word().to_string())
                    .or_default()
                    .push(run.id.clone());
            }
        }
        out
    }

    /// The share of ranking tests green, or 0 with none.
    #[must_use]
    pub fn rank_fraction(&self) -> f64 {
        if self.rank_total == 0 {
            0.0
        } else {
            self.rank_green as f64 / self.rank_total as f64
        }
    }
}

/// Whether a candidate with `(held, score)` beats one with
/// `(best_held, best_score)` on the evidence that may reverse an edit: the
/// holding classes first, fewer red is better, then the frozen score.
/// Ranking-only tests never enter this comparison.
#[must_use]
pub fn beats(held: usize, score: f64, best_held: usize, best_score: f64) -> bool {
    held < best_held || (held == best_held && score > best_score)
}

/// Whether a candidate is at least as good as the best so far, with the
/// ranking-only classes breaking a tie on the holding classes and the
/// score. `strict` keeps an earlier candidate on a full tie.
#[must_use]
pub fn ranks_at_least(
    (held, score, rank): (usize, f64, f64),
    (best_held, best_score, best_rank): (usize, f64, f64),
    strict: bool,
) -> bool {
    if beats(held, score, best_held, best_score) {
        return true;
    }
    if held != best_held || (score - best_score).abs() > f64::EPSILON {
        return false;
    }
    if strict {
        rank > best_rank
    } else {
        rank >= best_rank
    }
}

/// The lines a session reads about the tiered suite's last run: holding
/// tests as what must pass, guards as suspects, and nothing about the
/// ranking-only tests, which can't order a change.
#[must_use]
pub fn brief_lines(suite: &AcceptanceSuite, result: &RunResult, powers: &Powers) -> Vec<String> {
    let mut lines = Vec::new();
    let what = |id: &str| {
        suite
            .tests
            .iter()
            .find(|t| t.id == id)
            .map(|t| t.what.clone())
            .unwrap_or_default()
    };
    if powers.hold_total > 0 {
        lines.push(format!(
            "The task's own contract checks: {} of {} pass. The task isn't done while one \
             fails, and passing them all doesn't show it done.",
            powers.hold_green, powers.hold_total
        ));
        for id in &powers.held {
            let output = result
                .tests
                .iter()
                .find(|t| t.id == *id)
                .map(|t| crate::judge::clip_tail(t.output.trim(), 600))
                .unwrap_or_default();
            lines.push(format!("{id} fails: {}\n{output}", what(id)));
        }
    }
    for id in &powers.suspects {
        lines.push(format!(
            "{id} passed on the untouched code and fails now: {}. It may pin the defect the \
             task asks you to fix, or it may mark a regression. Decide from the task's words; \
             don't restore old behavior only to make it pass.",
            what(id)
        ));
    }
    lines
}
