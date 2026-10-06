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
//!    ([`Scores`]), asks the Gym gate `ext-eval-v2` for the verdict, and
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
//!
//! # The runner
//!
//! With the default `runner` feature, the crate also runs suites:
//! [`run::run_suite`] makes every planned run as one confined `coder -p`
//! turn ([`sandbox`], [`child`], [`proxy`], [`arms`]), grades them with the
//! live doors ([`live`], [`replay`], and [`JevDoor`]), and writes the
//! results directory. [`publish`] builds the suite release and the `3189`
//! result, [`blob`] moves suite bytes over Blossom, [`check`] reruns a
//! published result, and [`trust`] and [`signal`] hold the operator's
//! trust and stop requests. `docs/extensions/evaluation.md`, *The run
//! sandbox*, is the specification.

pub mod artifact;
pub mod author;
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
pub mod study;
pub mod trajectory;
pub mod workspace;

#[cfg(feature = "runner")]
pub mod arms;
#[cfg(feature = "runner")]
pub mod blob;
#[cfg(feature = "runner")]
pub mod check;
#[cfg(feature = "runner")]
pub mod child;
#[cfg(feature = "runner")]
pub mod live;
#[cfg(feature = "runner")]
pub mod proxy;
#[cfg(feature = "runner")]
pub mod publish;
#[cfg(feature = "runner")]
pub mod replay;
#[cfg(feature = "runner")]
pub mod run;
#[cfg(feature = "runner")]
pub mod sandbox;
#[cfg(feature = "runner")]
pub mod signal;
#[cfg(feature = "runner")]
pub mod trust;

pub use artifact::ArtifactRef;
pub use case::{Case, CaseError, CaseFiles, Grant, Kind, LoadOptions, RunConfig, RunFailure};
pub use discover::{Filter, Suite, eval_dir};
pub use door::{
    DecisionAnswer, DecisionDoor, Doors, JevDoor, JudgeDoor, ReplayVerdict, Replayer, RunKey,
};
pub use evaluate::{EvalError, Evaluation, GATE_ID, Verdict, conclude, evaluate, load_gate, notes};
pub use grade::{GraderResult, Vote, grade_run};
pub use grader::{ArmRule, Check, DecisionQuestion, Focus, Grader, Match};
pub use record::{Arm, RunOutcome, RunRecord, run_path};
pub use report::{ArmSetup, DoorNames, Identity, validate};
pub use score::{GradedRun, Plan, PlanError, Scores, grade_all};
pub use trajectory::Trajectory;
