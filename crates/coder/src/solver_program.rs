//! The solver as a program, replayed (#11392).
//!
//! `programs/solve-issue.json` carries the solver's hands as a NIP-PRG
//! document: intake, brief, code under a bounded `verify` clause whose
//! rejections hand back through the Tester, the final check, the land
//! operation, and the `done` judgment. This module is a second host for
//! that document. It reads nothing but the document's steps — kinds,
//! order, `requires`, the delegate executors, the `verify` bound, the
//! `hand_back` routes, the invoke operation, and the decide gate — and runs
//! them against a recorded issue run: what each hand returned, what the
//! harness checks said each round, and what the door answered.
//!
//! The recorded case also carries what the hard-coded `drive()` in
//! `coder-new`'s solver produced on the same run: its hands with rounds,
//! the last round, and the outcome. [`replay`] produces the same three from
//! the document alone, and the corpus under `fixtures/solver/solve-issue/`
//! pins that they match. The workflow lives in the document: change the
//! `verify` drives and the replay's hand-back bound moves with it, change
//! the step order and the hands change, so the match is evidence the
//! document — not this module — carries the solver.
//!
//! The replay is exact about drive()'s control flow:
//!
//! - intake blocked is `setup_failed`; a commit for the issue already on
//!   main is `already_done`; a base that could not be prepared is
//!   `setup_failed`; no briefing is `decision_failed`;
//! - each drive of the verified `code` step is one Coder turn; a Coder
//!   error ends the run `failed`;
//! - the gate reads the required checks as the door's per-requirement
//!   answers (`openagents.completion.v1` is templated per requirement), a
//!   pass read 1.0 and a failure 0.0, and the drive clears when every
//!   requirement clears `refuse_below`; no required check is `unchecked`;
//! - a failure the computer caused, not the change, ends `failed` with no
//!   Tester turn;
//! - otherwise the `hand_back` diagnoser (the Tester) answers, its route
//!   sends the next drive to the Coder or through the Briefer first, the
//!   same diagnosis on the same failing checks twice running ends
//!   `failed`, and the last drive spent ends `failed`.
//!
//! The `done` decide step answers through the door with the sidecar
//! convention (#11377): the raw answers land at `done$answers` and the
//! typed read at `done`.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::program::{Kind, Program, Step};

/// One recorded issue run: the hands' results, the checks each round, the
/// door's answers, and what `drive()` produced on it.
#[derive(Clone, Debug, Deserialize)]
pub struct Case {
    /// The case's name in the corpus.
    pub case: String,
    /// The recorded solver run this case was drawn from, when one was.
    #[serde(default)]
    pub from_run: Option<String>,
    pub issue: u64,
    #[serde(default)]
    pub intake: Intake,
    #[serde(default)]
    pub brief: Brief,
    /// The required checks, unless a drive names its own (a change that
    /// touched another crate adds that crate's checks).
    #[serde(default)]
    pub required: Vec<String>,
    /// One record per Coder turn, in order.
    #[serde(default)]
    pub drives: Vec<Drive>,
    /// The door's answers to the `done` question, per requirement.
    #[serde(default)]
    pub done: BTreeMap<String, f64>,
    /// What `drive()` produced on this run.
    pub drive: Expected,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Intake {
    /// Why intake stopped the run, when it did.
    #[serde(default)]
    pub blocked: Option<String>,
    /// The commit already on main for this issue, when there is one.
    #[serde(default)]
    pub landed_commit: Option<String>,
    /// The issue, base, or worktree could not be prepared.
    #[serde(default)]
    pub prepare_failed: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Brief {
    /// The code-brief steps stopped before a briefing existed.
    #[serde(default)]
    pub failed: bool,
}

/// One drive of the verified `code` step.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Drive {
    #[serde(default)]
    pub coder_error: Option<String>,
    /// This drive's required checks, when they differ from the case's.
    #[serde(default)]
    pub required: Option<Vec<String>>,
    /// Every check the harness ran this drive, by id.
    #[serde(default)]
    pub checks: BTreeMap<String, bool>,
    /// A failure the computer caused rather than the change.
    #[serde(default)]
    pub environment_fault: Option<String>,
    /// The Tester's verdict on this drive's failures.
    #[serde(default)]
    pub tester: Option<Tester>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Tester {
    /// `coder`, `briefer`, or `pass`.
    pub route: String,
    #[serde(default)]
    pub hypothesis: String,
}

/// What `drive()` produced: its hands with rounds, the last round, and the
/// outcome's status word.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Expected {
    pub hands: Vec<(String, usize)>,
    pub rounds: usize,
    pub outcome: String,
}

/// What the program produced on the case.
#[derive(Clone, Debug, PartialEq)]
pub struct Replay {
    pub hands: Vec<(String, usize)>,
    pub rounds: usize,
    pub outcome: String,
    /// The step outputs, decide sidecars included.
    pub produced: BTreeMap<String, Value>,
    /// The steps that ran, in order.
    pub steps: Vec<String>,
}

impl Replay {
    /// The three things `drive()` is compared on.
    #[must_use]
    pub fn expected(&self) -> Expected {
        Expected {
            hands: self.hands.clone(),
            rounds: self.rounds,
            outcome: self.outcome.clone(),
        }
    }
}

/// The hand each executor the document names is, on this host. A host
/// that does not know an executor refuses the step rather than guessing.
fn hand_of(executor: &str) -> Option<&'static str> {
    match executor {
        "solver-briefer" => Some("briefer"),
        "solver-coder" => Some("coder"),
        "solver-tester" => Some("tester"),
        _ => None,
    }
}

/// The operations this host runs for an `invoke` step.
const OPERATIONS: &[&str] = &["open-pull-request"];

struct Host<'a> {
    program: &'a Program,
    case: &'a Case,
    hands: Vec<(String, usize)>,
    round: usize,
    produced: BTreeMap<String, Value>,
    steps: Vec<String>,
    /// The last drive's required checks and results, for the final check.
    last: Option<(Vec<String>, BTreeMap<String, bool>)>,
}

/// Why the run ended before its last step.
struct End(String);

/// Runs the document's steps against the recorded case.
#[must_use]
pub fn replay(program: &Program, case: &Case) -> Replay {
    let mut host = Host {
        program,
        case,
        hands: Vec::new(),
        round: 0,
        produced: BTreeMap::new(),
        steps: Vec::new(),
        last: None,
    };
    let outcome = match host.run() {
        Ok(outcome) | Err(End(outcome)) => outcome,
    };
    Replay {
        hands: host.hands,
        rounds: host.round,
        outcome,
        produced: host.produced,
        steps: host.steps,
    }
}

impl Host<'_> {
    fn run(&mut self) -> Result<String, End> {
        let mut outcome = String::from("refused");
        for step in &self.program.steps {
            // The host refuses a step while any output it names is empty
            // (#11377): the document's order is enforced, not assumed.
            for name in &step.requires {
                if self.produced.get(name).is_none_or(Value::is_null) {
                    return Err(End("refused".into()));
                }
            }
            self.steps.push(step.name.clone());
            let output = match step.kind {
                Kind::Query => self.query(step)?,
                Kind::Delegate => match step.verify() {
                    Ok(Some(_)) => self.verified(step)?,
                    Ok(None) => self.delegate(step, 0)?,
                    Err(_) => return Err(End("refused".into())),
                },
                Kind::Check => {
                    let (status, value) = self.check()?;
                    outcome = status;
                    value
                }
                Kind::Invoke => self.invoke(step)?,
                Kind::Decide => self.decide(step)?,
                Kind::Program | Kind::Module => return Err(End("refused".into())),
            };
            self.produced.insert(step.name.clone(), output);
        }
        Ok(outcome)
    }

    fn query(&mut self, step: &Step) -> Result<Value, End> {
        if step
            .source
            .as_deref()
            .is_some_and(|source| source != crate::source::REQUEST)
        {
            return Err(End("refused".into()));
        }
        self.hands.push(("intaker".into(), 0));
        let intake = &self.case.intake;
        if intake.blocked.is_some() {
            return Err(End("setup_failed".into()));
        }
        if intake.landed_commit.is_some() {
            return Err(End("already_done".into()));
        }
        if intake.prepare_failed {
            return Err(End("setup_failed".into()));
        }
        Ok(json!({"issue": self.case.issue}))
    }

    fn executor_hand(step: &Step) -> Result<&'static str, End> {
        step.rest
            .get("executor")
            .and_then(Value::as_str)
            .and_then(hand_of)
            .ok_or_else(|| End("refused".into()))
    }

    fn delegate(&mut self, step: &Step, round: usize) -> Result<Value, End> {
        let hand = Self::executor_hand(step)?;
        if hand == "briefer" && self.case.brief.failed {
            return Err(End("decision_failed".into()));
        }
        self.hands.push((hand.into(), round));
        Ok(json!(hand))
    }

    /// A delegate step under its `verify` clause, with the `hand_back`
    /// routes the binding declares: drive()'s Coder and Tester rounds.
    fn verified(&mut self, step: &Step) -> Result<Value, End> {
        let verify = step
            .verify()
            .ok()
            .flatten()
            .ok_or_else(|| End("refused".into()))?;
        let hand_back = step
            .rest
            .get("hand_back")
            .and_then(Value::as_object)
            .ok_or_else(|| End("refused".into()))?;
        let diagnoser = hand_back
            .get("diagnoser")
            .and_then(Value::as_str)
            .and_then(hand_of)
            .ok_or_else(|| End("refused".into()))?;
        let routes = hand_back
            .get("routes")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let hand = Self::executor_hand(step)?;
        let mut earlier: Option<(String, Vec<String>, String)> = None;
        let mut drives = Vec::new();
        for drive in 1..=verify.drives {
            let round = usize::try_from(drive - 1).unwrap_or(usize::MAX);
            self.round = round;
            self.hands.push((hand.into(), round));
            let Some(record) = self.case.drives.get(round) else {
                // The recording ran out before the document's bound: the
                // case does not cover this run.
                return Err(End("refused".into()));
            };
            if record.coder_error.is_some() {
                return Err(End("failed".into()));
            }
            let required = record
                .required
                .clone()
                .unwrap_or_else(|| self.case.required.clone());
            self.last = Some((required.clone(), record.checks.clone()));
            // The door's answers, one per requirement: a deterministic
            // check reads 1.0 when it passed and 0.0 otherwise.
            let answers: Map<String, Value> = required
                .iter()
                .map(|id| {
                    let read = match record.checks.get(id) {
                        Some(true) => 1.0,
                        _ => 0.0,
                    };
                    (id.clone(), json!({"noul": read}))
                })
                .collect();
            drives.push(json!({"drive": drive, "answers": answers}));
            self.produced
                .insert(format!("{}$verify", step.name), json!({"drives": drives}));
            let cleared = required
                .iter()
                .all(|id| answers[id]["noul"].as_f64().unwrap_or(0.0) >= verify.refuse_below);
            if required.is_empty() || cleared {
                return Ok(json!({"drive": drive}));
            }
            if record.environment_fault.is_some() {
                return Err(End("failed".into()));
            }
            self.hands.push((diagnoser.into(), round));
            let failed: Vec<String> = record
                .checks
                .iter()
                .filter(|(_, ok)| !**ok)
                .map(|(id, _)| id.clone())
                .collect();
            let tester = record
                .tester
                .as_ref()
                .ok_or_else(|| End("refused".into()))?;
            let first_failing = required
                .iter()
                .find(|id| record.checks.get(*id) != Some(&true))
                .cloned()
                .unwrap_or_default();
            let judged = match record.checks.get(&first_failing) {
                None => format!("The required check {first_failing} has no result."),
                Some(_) => format!("The required check {first_failing} failed."),
            };
            let (route, hypothesis) = match tester.route.as_str() {
                "pass" | "coder" if failed.is_empty() => ("coder", judged),
                "pass" => (
                    "coder",
                    format!("The harness's final checks failed: {}.", failed.join(", ")),
                ),
                "briefer" => ("briefer", tester.hypothesis.clone()),
                _ => ("coder", tester.hypothesis.clone()),
            };
            let record_key = (
                route.to_string(),
                failed.clone(),
                hypothesis.trim().to_string(),
            );
            if earlier.as_ref().is_some_and(|before| {
                before.0 == record_key.0
                    && before.1 == record_key.1
                    && before.2.eq_ignore_ascii_case(&record_key.2)
            }) {
                return Err(End("failed".into()));
            }
            earlier = Some(record_key);
            if drive >= verify.drives {
                return Err(End("failed".into()));
            }
            if route == "briefer" {
                let target = routes
                    .get("briefer")
                    .and_then(Value::as_str)
                    .and_then(|name| self.program.steps.iter().find(|step| step.name == name))
                    .ok_or_else(|| End("refused".into()))?;
                self.delegate(target, round + 1)?;
            }
        }
        Err(End("failed".into()))
    }

    /// The final check: the last drive's required checks.
    fn check(&mut self) -> Result<(String, Value), End> {
        let Some((required, checks)) = self.last.clone() else {
            return Err(End("refused".into()));
        };
        if required.is_empty() {
            return Err(End("unchecked".into()));
        }
        if required.iter().all(|id| checks.get(id) == Some(&true)) {
            Ok(("passed".into(), json!(required)))
        } else {
            Err(End("failed".into()))
        }
    }

    fn invoke(&mut self, step: &Step) -> Result<Value, End> {
        match step.rest.get("operation").and_then(Value::as_str) {
            Some(operation) if OPERATIONS.contains(&operation) => {
                Ok(json!({"operation": operation, "issue": self.case.issue}))
            }
            _ => Err(End("refused".into())),
        }
    }

    /// A decide step through the door: the raw answers at `<name>$answers`,
    /// the typed read (the weakest requirement) at `<name>`, and a read
    /// under the step's `refuse_below` refuses.
    fn decide(&mut self, step: &Step) -> Result<Value, End> {
        if step.question.is_none() || self.case.done.is_empty() {
            return Err(End("refused".into()));
        }
        let answers: Map<String, Value> = self
            .case
            .done
            .iter()
            .map(|(id, read)| (id.clone(), json!({"noul": read})))
            .collect();
        self.produced
            .insert(format!("{}$answers", step.name), Value::Object(answers));
        let read = self.case.done.values().copied().fold(1.0_f64, f64::min);
        let floor = step
            .bounds
            .get("refuse_below")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        if read < floor {
            return Err(End("refused".into()));
        }
        Ok(json!(read))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn solve_issue() -> Program {
        Program::load(&root().join("programs/solve-issue.json"))
            .expect("the repository's solve-issue reads")
    }

    fn corpus() -> Vec<Case> {
        let dir = root().join("fixtures/solver/solve-issue");
        let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
            .expect("the solver corpus")
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
            .collect();
        paths.sort();
        paths
            .iter()
            .map(|path| {
                let text = std::fs::read_to_string(path).unwrap();
                serde_json::from_str(&text)
                    .unwrap_or_else(|why| panic!("{}: {why}", path.display()))
            })
            .collect()
    }

    /// Every recorded case: the document, replayed, produces the hands,
    /// rounds, and outcome `drive()` produced on the same run.
    #[test]
    fn the_solve_issue_program_matches_drive_on_the_corpus() {
        let program = solve_issue();
        let cases = corpus();
        assert!(cases.len() >= 8, "the corpus covers every end drive() has");
        let mut outcomes: Vec<&str> = cases
            .iter()
            .map(|case| case.drive.outcome.as_str())
            .collect();
        outcomes.sort_unstable();
        outcomes.dedup();
        for end in [
            "already_done",
            "decision_failed",
            "failed",
            "passed",
            "setup_failed",
            "unchecked",
        ] {
            assert!(outcomes.contains(&end), "no case ends {end}");
        }
        for case in &cases {
            let replay = replay(&program, case);
            assert_eq!(replay.expected(), case.drive, "case {}", case.case);
        }
    }

    /// A pass reaches `done` through the door, and the decide step lands
    /// its raw answers in the sidecar beside the typed read.
    #[test]
    fn done_answers_through_the_door_with_the_sidecar() {
        let program = solve_issue();
        let case = corpus()
            .into_iter()
            .find(|case| case.case == "clean-pass")
            .unwrap();
        let replay = replay(&program, &case);
        assert_eq!(
            replay.steps,
            ["intake", "brief", "code", "check", "land", "done"]
        );
        assert!(replay.produced["done$answers"].is_object());
        assert!(replay.produced["done"].as_f64().unwrap() >= 0.7);
        assert!(replay.produced["code$verify"]["drives"].is_array());
    }

    /// The bound lives in the document: one fewer drive ends a case that
    /// needed its last hand-back one round early, failed.
    #[test]
    fn the_hand_back_bound_is_the_documents() {
        let mut program = solve_issue();
        let case = corpus()
            .into_iter()
            .find(|case| case.case == "hand-back-twice-then-pass")
            .unwrap();
        assert_eq!(replay(&program, &case).outcome, "passed");
        let code = program
            .steps
            .iter_mut()
            .find(|step| step.name == "code")
            .unwrap();
        code.rest
            .get_mut("verify")
            .and_then(Value::as_object_mut)
            .unwrap()
            .insert("drives".into(), json!(2));
        let short = replay(&program, &case);
        assert_eq!(short.outcome, "failed");
        assert_eq!(short.rounds, 1);
    }

    /// The order lives in the document too: a step whose required output
    /// is missing refuses, and an executor the host does not know refuses.
    #[test]
    fn the_host_refuses_what_the_document_does_not_support() {
        let case = corpus()
            .into_iter()
            .find(|case| case.case == "clean-pass")
            .unwrap();
        let mut program = solve_issue();
        program.steps.retain(|step| step.name != "brief");
        assert_eq!(replay(&program, &case).outcome, "refused");

        let mut program = solve_issue();
        let brief = program
            .steps
            .iter_mut()
            .find(|step| step.name == "brief")
            .unwrap();
        brief.rest.insert("executor".into(), json!("someone-else"));
        assert_eq!(replay(&program, &case).outcome, "refused");
    }
}
