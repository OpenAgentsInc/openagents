//! The extension evaluation engine.
//!
//! An extension eval measures whether an extension changes what Coder does:
//! it runs a suite of cases with the extension admitted (the `subject` arm)
//! and with it absent (the `baseline` arm), grades each run, scores both
//! arms, and reports the change. `docs/extensions/evaluation.md` is the
//! specification; the NIP-EVAL *Extension evaluation profile* is the wire
//! contract of the report.
//!
//! This crate is the pure part: everything that reads a suite and a set of
//! finished runs and produces a verdict and a report. It spawns nothing.
//! The runner (`openagents ext eval`, the hosted runner) runs each case and
//! hands the engine one [`RunRecord`] per attempt.
//!
//! # The flow
//!
//! 1. [`Suite::load`] discovers and validates the cases under the eval
//!    directory ([`eval_dir`] resolves it).
//! 2. [`Plan::attempts`] lists the runs to make.
//! 3. The runner makes them and builds [`RunRecord`]s.
//! 4. [`evaluate`] grades every run ([`grade_all`]), scores the arms
//!    ([`Scores`]), asks the Gym gate `ext-eval-v1` for the verdict, and
//!    builds `report.json`, its artifacts, and `report.html`
//!    ([`Evaluation`]); [`Evaluation::write`] puts them in a results
//!    directory.
//!
//! # Graders and doors
//!
//! Structural graders are deterministic checks over bounded fields of the
//! trajectory and the created files. `decision` graders ask Jev a typed
//! question ([`JevDoor`]); `judge` graders ask the chat door for PASS or
//! FAIL; `receipt` graders replay Wasm invocation receipts. Each door is a
//! trait ([`DecisionDoor`], [`JudgeDoor`], [`Replayer`]) with a fake in
//! [`door::fake`], so tests make no network call. Nothing here routes on
//! keywords.

pub mod artifact;
pub mod case;
pub mod discover;
pub mod door;
pub mod evaluate;
pub mod glob;
pub mod grade;
pub mod grader;
pub mod html;
pub mod record;
pub mod report;
pub mod score;
pub mod trajectory;

pub use artifact::ArtifactRef;
pub use case::{Case, CaseError, CaseFiles, Grant, Kind, LoadOptions, RunConfig, RunFailure};
pub use discover::{Filter, Suite, eval_dir};
pub use door::{
    DecisionAnswer, DecisionDoor, Doors, JevDoor, JudgeDoor, ReplayVerdict, Replayer, RunKey,
};
pub use evaluate::{EvalError, Evaluation, GATE_ID, Verdict, conclude, evaluate, load_gate};
pub use grade::{GraderResult, Vote, grade_run};
pub use grader::{ArmRule, Check, DecisionQuestion, Focus, Grader, Match};
pub use record::{Arm, RunOutcome, RunRecord, run_path};
pub use report::{ArmSetup, DoorNames, Identity, validate};
pub use score::{GradedRun, Plan, PlanError, Scores, grade_all};
pub use trajectory::Trajectory;
