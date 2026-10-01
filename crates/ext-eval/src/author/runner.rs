//! Runs of the draft: the one-run try and the full run, as the interview
//! reads them.
//!
//! The interview never runs anything itself. In a terminal, `openagents ext
//! eval init` asks the [`Runner`] on the person's `y`; in chat, a tap sends
//! the phone's own signed request to the hosted runner or the connected
//! computer, and the result comes back as a record. Either way the
//! interview reads a [`Tried`], built from the engine's [`Evaluation`], so
//! every number it states comes from a report.

use std::collections::BTreeSet;
use std::path::Path;

use nostr::contracts::{ArtifactRef, parse_artifact};

use crate::case::Kind;
use crate::evaluate::{Evaluation, Verdict};
use crate::record::Arm;

use super::catalog::Tool;

/// The runs per arm a try uses.
pub const TRY_RUNS: u32 = 1;
/// The runs per arm the full run uses.
pub const FULL_RUNS: u32 = 3;

/// One test in a result.
#[derive(Clone, Debug, PartialEq)]
pub struct CaseTried {
    /// The test's id.
    pub id: String,
    /// Whether the tool should help.
    pub kind: Kind,
    /// Whether it passed with the tool; `None` when unknown.
    pub with: Option<bool>,
    /// Whether it passed without the tool; `None` when unknown or not run.
    pub without: Option<bool>,
    /// The checks that failed with the tool, as `name: why`, from the first
    /// run.
    pub failing: Vec<String>,
}

/// A result the interview reads: a try or a full run.
#[derive(Clone, Debug, PartialEq)]
pub struct Tried {
    /// Runs per arm.
    pub runs: u32,
    /// Tests passed with the tool.
    pub with: u64,
    /// Tests passed without it; `None` with no baseline.
    pub without: Option<u64>,
    /// Tests in the set.
    pub total: u64,
    /// The gate's verdict.
    pub verdict: Verdict,
    /// The report, which a publish offer names.
    pub report: Option<ArtifactRef>,
    /// Each test.
    pub cases: Vec<CaseTried>,
}

impl Tried {
    /// The result an evaluation reports.
    #[must_use]
    pub fn from_evaluation(evaluation: &Evaluation, runs: u32) -> Self {
        let scores = &evaluation.scores;
        let cases = scores
            .cases
            .iter()
            .map(|case| {
                let failing = evaluation
                    .runs
                    .iter()
                    .find(|run| run.case == case.id && run.arm == Arm::Subject)
                    .map(|run| {
                        run.graders
                            .iter()
                            .filter(|g| !g.passed)
                            .map(|g| format!("{}: {}", g.name, g.explanation))
                            .collect()
                    })
                    .unwrap_or_default();
                CaseTried {
                    id: case.id.clone(),
                    kind: case.kind,
                    with: case.subject.passed,
                    without: case.baseline.as_ref().and_then(|b| b.passed),
                    failing,
                }
            })
            .collect();
        Self {
            runs,
            with: scores.subject.cases_passed as u64,
            without: scores.baseline.as_ref().map(|b| b.cases_passed as u64),
            total: scores.cases.len() as u64,
            verdict: evaluation.verdict,
            report: parse_artifact(&evaluation.report_ref.value()).ok(),
            cases,
        }
    }

    /// The result as the model reads it: the counts and each test, no
    /// more.
    #[must_use]
    pub fn state(&self) -> serde_json::Value {
        serde_json::json!({
            "runs_per_side": self.runs,
            "tests": self.total,
            "passed_with_the_tool": self.with,
            "passed_without_the_tool": self.without,
            "verdict": self.verdict.plain(),
            "each_test": self.cases.iter().map(|c| serde_json::json!({
                "test": c.id,
                "kind": c.kind.word(),
                "passed_with_the_tool": c.with,
                "passed_without_the_tool": c.without,
                "failed_checks_with_the_tool": c.failing,
            })).collect::<Vec<_>>(),
        })
    }

    /// The headline in plain words, from the record.
    #[must_use]
    pub fn headline(&self) -> String {
        match self.without {
            Some(without) => format!(
                "With the plugin, Coder passed {} of {} tests; without it, {} of {}.",
                self.with, self.total, without, self.total
            ),
            None => format!(
                "With the plugin, Coder passed {} of {} tests.",
                self.with, self.total
            ),
        }
    }
}

/// What a runner is asked to run.
#[derive(Clone, Copy, Debug)]
pub struct RunRequest<'a> {
    /// The tool.
    pub tool: &'a Tool,
    /// The eval directory holding the draft's cases, written out.
    pub eval_dir: &'a Path,
    /// The extension's directory, for a tool on this computer.
    pub extension: Option<&'a Path>,
    /// Runs per arm.
    pub runs: u32,
}

/// Runs a suite in both arms and grades it: `openagents ext eval run`
/// (#9934) in a terminal, or a fake in tests.
pub trait Runner {
    /// Runs `request` and reads its result.
    ///
    /// # Errors
    ///
    /// Why nothing ran, in plain words.
    fn run(&self, request: &RunRequest<'_>) -> Result<Tried, String>;
}

/// No runner on this computer: every run is refused with the command to
/// run instead.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoRunner;

impl Runner for NoRunner {
    fn run(&self, request: &RunRequest<'_>) -> Result<Tried, String> {
        Err(format!(
            "this openagents can't run tests from the interview yet; after the test set is written, run `openagents plugin test run <plugin> --runs {}`",
            request.runs
        ))
    }
}

/// Test runners: the real engine over scripted runs.
pub mod fake {
    use std::cell::RefCell;

    use serde_json::{Value, json};

    use super::{BTreeSet, RunRequest, Runner, Tried};
    use crate::ArtifactRef as EngineArtifact;
    use crate::artifact::JSON;
    use crate::case::{Case, LoadOptions};
    use crate::discover::Suite;
    use crate::door::fake::FakeDecisionDoor;
    use crate::door::{DecisionAnswer, Doors};
    use crate::evaluate::{evaluate, load_gate};
    use crate::record::{Arm, RunOutcome, RunRecord};
    use crate::report::{ArmSetup, Identity};
    use crate::score::Plan;
    use crate::trajectory::Trajectory;

    /// The final message a scripted run that did the task ends with. The
    /// fake decision door says yes to it and no to anything else.
    pub const GOOD: &str = "Done: the task is complete and correct.";
    /// The final message a scripted run that missed ends with.
    pub const BAD: &str = "Not sure; this may be incomplete.";

    /// What one scripted run does: its final message and the operations it
    /// called.
    pub type Script = Box<dyn Fn(&Case, Arm, u32) -> (String, Vec<String>)>;

    /// A runner that loads the written suite with the engine, scripts each
    /// run, grades it with a fake decision door, and concludes with the
    /// real gate. It records every request.
    pub struct FakeRunner {
        script: Script,
        requests: RefCell<Vec<(String, u32)>>,
    }

    impl FakeRunner {
        /// A runner whose runs do what `script` says.
        #[must_use]
        pub fn scripted(script: Script) -> Self {
            Self {
                script,
                requests: RefCell::new(Vec::new()),
            }
        }

        /// The usual tool: it helps on tests where it should (the task is
        /// done with it and missed without it), and stays out of the way
        /// elsewhere (both sides do the task, and it never runs).
        #[must_use]
        pub fn helpful(operations: Vec<String>) -> Self {
            Self::scripted(Box::new(move |case, arm, _| {
                let fire = case.kind == crate::case::Kind::ShouldFire;
                match (fire, arm) {
                    (true, Arm::Subject) => (GOOD.into(), operations.clone()),
                    (true, Arm::Baseline) => (BAD.into(), Vec::new()),
                    (false, _) => (GOOD.into(), Vec::new()),
                }
            }))
        }

        /// The eval directories and runs per arm it was asked to run.
        #[must_use]
        pub fn requests(&self) -> Vec<(String, u32)> {
            self.requests.borrow().clone()
        }
    }

    fn trajectory(case: &Case, message: &str, operations: &[String]) -> Trajectory {
        let mut steps = vec![json!({
            "step_id": 1,
            "timestamp": "2026-09-29T12:00:00Z",
            "source": "user",
            "message": case.prompt,
        })];
        for (index, operation) in operations.iter().enumerate() {
            steps.push(json!({
                "step_id": steps.len() + 1,
                "timestamp": "2026-09-29T12:00:01Z",
                "source": "agent",
                "message": "Looking first.",
                "model_name": "fake-model",
                "tool_calls": [{
                    "tool_call_id": format!("call-{index}"),
                    "function_name": operation,
                    "arguments": {},
                }],
            }));
        }
        steps.push(json!({
            "step_id": steps.len() + 1,
            "timestamp": "2026-09-29T12:00:02Z",
            "source": "agent",
            "message": message,
            "model_name": "fake-model",
        }));
        let count = steps.len();
        let document = json!({
            "schema_version": "ATIF-v1.8",
            "session_id": format!("fake-{}", case.name),
            "trajectory_id": format!("fake-{}", case.name),
            "agent": {"name": "coder", "version": "0.1.0", "model_name": "fake-model", "extra": {"door": "fake-door"}},
            "steps": steps,
            "final_metrics": {"total_prompt_tokens": 0, "total_completion_tokens": 0, "total_steps": count},
        });
        Trajectory::from_bytes(&serde_json::to_vec(&document).unwrap_or_default())
            .unwrap_or_else(|error| panic!("the fake trajectory is valid: {error}"))
    }

    const KEY: &str = "5be6446aef0a9a6b1f2c3d4e5f6071829a4b5c6d7e8f90112233445566778899";

    fn setup(name: &str) -> ArmSetup {
        ArmSetup {
            definition: json!({
                "id": format!("{KEY}:{name}/{name}"),
                "artifact": EngineArtifact::of(name.as_bytes(), JSON, Some("openagents.ext-package.v1")).value(),
            }),
            lock: EngineArtifact::of(name.as_bytes(), JSON, Some("openagents.lock.v1")),
            door: "fake-door".into(),
            run: json!({}),
        }
    }

    impl Runner for FakeRunner {
        fn run(&self, request: &RunRequest<'_>) -> Result<Tried, String> {
            self.requests
                .borrow_mut()
                .push((request.eval_dir.display().to_string(), request.runs));
            let suite = Suite::load(request.eval_dir, LoadOptions::default())
                .map_err(|error| error.to_string())?;
            let plan = Plan {
                baseline: true,
                runs: Some(request.runs),
                extension_operations: request
                    .tool
                    .operations
                    .iter()
                    .cloned()
                    .collect::<BTreeSet<_>>(),
            };
            let mut records = Vec::new();
            for (name, arm, attempt) in plan.attempts(&suite) {
                let case = suite.case(&name).ok_or("a planned case is missing")?;
                let (message, operations) = (self.script)(case, arm, attempt);
                let mut record = RunRecord::new(&name, arm, attempt, RunOutcome::Completed);
                record.trajectory = Some(trajectory(case, &message, &operations));
                records.push(record);
            }
            let door = FakeDecisionDoor::answering(|state, _| {
                let run: &Value = &state["run"];
                Ok(DecisionAnswer::Noul(if run.as_str() == Some(GOOD) {
                    0.9
                } else {
                    0.1
                }))
            });
            let identity = Identity {
                author: KEY.into(),
                package: "draft".into(),
                component: "eval-suite".into(),
                evaluator: KEY.into(),
                subject: setup("subject"),
                baseline: Some(setup("baseline")),
                started_at: 1_790_000_000,
                ended_at: 1_790_000_100,
                requester: None,
                suite_release: None,
                environment: None,
                defaults: None,
                partial: None,
            };
            let (gate, bytes) = load_gate().map_err(|error| error.to_string())?;
            let evaluation = evaluate(
                &suite,
                &plan,
                records,
                &identity,
                (&gate, &bytes),
                Doors {
                    decision: Some(&door),
                    ..Doors::default()
                },
            )
            .map_err(|error| error.to_string())?;
            Ok(Tried::from_evaluation(&evaluation, request.runs))
        }
    }
}
