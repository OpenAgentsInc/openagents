//! Khala M8 head-to-head evaluation harness (OpenAgents issue #6016).
//!
//! M8's north-star is the watchable Fugu-Ultra-vs-frontier head-to-head, where
//! **`openagents/khala` solves the benchmark BY COMPOSITION, verified, cheaper
//! than a single model**. The publication-side reducer + manifest + closure
//! audit already live in the `openagents` repo (`scripts/khala-demo/`,
//! `docs/inference/khala-head-to-head-demo.md`). What was missing — and what this
//! module adds in Psionic, which owns coordinator/execution truth — is the
//! **evaluation harness that produces a lane's metrics in the first place**: it
//! runs the M7 Conductor's composed plan over the M6 [`WorkerPoolBinding`] pool
//! against a single-model baseline on a fixture task set, scores both through the
//! M6 verdict shape, and emits the deterministic, typed quality + cost report the
//! demo's "compose to win, cheaper" claim rests on.
//!
//! ## What it does
//!
//! Given a fixture [`HeadToHeadTaskSet`] (each task carries the per-arm outcome
//! both arms would produce on the offline lane — verdict + spend), the harness:
//!
//! 1. **Runs the composed arm**: for each task it builds the Conductor plan over
//!    the M6 pool (proving the language plan only ever names receipt-eligible
//!    workers — the capability gate is honored), then records the task's composed
//!    [`TrajectoryOutcome`] (verdict + total composed spend across the plan's
//!    steps).
//! 2. **Runs the single-model baseline arm**: the same task routed one-shot to a
//!    single worker, recording its [`TrajectoryOutcome`].
//! 3. **Scores both** with [`ArmOutcome`] (accepted/verified count + spend) and
//!    the [`TerminalRewardAdapter`], computing **accepted rate**,
//!    **cost-per-accepted-outcome**, and **verified-work-per-sat** — the exact
//!    metric vocabulary the openagents gateway telemetry and the demo reducer use
//!    (`costPerAcceptedOutcome*`, `verifiedRate`, verified-work-per-sat).
//! 4. **Reuses [`ShadowComparison`]** for the underlying learned-vs-baseline
//!    decision logic, then layers the M8 **win verdict**
//!    ([`HeadToHeadVerdict::ComposeToWinCheaper`]) on top: composition wins only
//!    when its verified-work-per-sat strictly beats single-model **at comparable
//!    quality** (the composed accepted rate is not materially worse than the
//!    single-model accepted rate, within [`QUALITY_PARITY_EPSILON`]).
//!
//! ## What it is NOT (default-off / fixture-only)
//!
//! - **No real model.** Both arms consume fixture [`TrajectoryOutcome`]s; the
//!   Conductor plan is produced by a [`ConductorPolicy`] fixture, never a 7B.
//! - **No Pylon dispatch, no spend.** It never dispatches a buy-mode eval job,
//!   never moves sats. The composed-arm "spend" is a fixture number used only to
//!   compute the cost metric; it is not debited against any live ledger.
//! - The **real armed run** — composed plan over the live M4 Pylon pool with a
//!   spend-enabled buy-mode campaign, scored by an *armed*
//!   [`EvalVerdictSource`](crate::EvalVerdictSource) — is the **owner gate**,
//!   flagged precisely by [`M8HeadToHeadReadiness`] (mirroring M7's
//!   [`ConductorReadiness`](crate::ConductorReadiness)). Every field is `false`
//!   in the shipped harness; flipping them is owner/compute work, not code that
//!   lands here.
//!
//! The accept/halt decision is always the replay-validator verdict carried by
//! each [`TrajectoryOutcome`], never a harness or head output — exactly as the
//! verified-work doc requires.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::coordinator_conductor::{ConductorPlan, ConductorPlanner, ConductorPolicy};
use crate::coordinator_evolution::{
    TerminalRewardAdapter, TrajectoryOutcome, VerificationVerdict, WorkerPoolBinding,
};
use crate::coordinator_shadow_comparison::{ArmOutcome, ShadowComparison};

/// Default quality-parity tolerance for the M8 win verdict. Composition is held
/// to "comparable quality" — its accepted rate may dip by at most this much
/// below the single-model accepted rate and still count as a cheaper win.
/// Tighter than the watch band on purpose: the demo claim is "cheaper at
/// comparable quality", not "cheaper at any quality".
pub const QUALITY_PARITY_EPSILON: f32 = 0.05;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors raised by the M8 head-to-head harness.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum M8HeadToHeadError {
    /// The task set was empty; there is nothing to compare.
    #[error("head-to-head task set is empty")]
    EmptyTaskSet,
    /// The single-model baseline named a worker index outside the eligible pool.
    #[error(
        "single-model baseline for task `{task_id}` names worker index {worker_index} but the eligible pool has {pool_len} workers"
    )]
    BaselineWorkerOutOfPool {
        /// The offending task id.
        task_id: String,
        /// The named worker index.
        worker_index: usize,
        /// The eligible pool size.
        pool_len: usize,
    },
    /// Building the composed plan for a task failed (parse/validation against the
    /// pool). This is surfaced as a structured error rather than a panic — a
    /// malformed plan is a real evaluation outcome (the composed arm fails the
    /// task), but a harness-level plan failure that prevents scoring is an error.
    #[error("composed plan for task `{task_id}` failed: {detail}")]
    ComposedPlanFailed {
        /// The task id whose plan failed.
        task_id: String,
        /// Human-readable detail.
        detail: String,
    },
}

// ---------------------------------------------------------------------------
// Fixture task set.
// ---------------------------------------------------------------------------

/// One fixture task in the head-to-head set. Each task carries the per-arm
/// **offline** outcomes the harness scores — there is no real execution. The
/// composed arm additionally carries the prompt the fixture [`ConductorPolicy`]
/// decodes a plan for and the per-step composed spend, so the harness can prove
/// the plan is valid over the pool *and* compute the composed cost.
///
/// On the owner-gated armed lane these outcomes come from real dispatched evals
/// scored by an armed [`EvalVerdictSource`](crate::EvalVerdictSource); here they
/// are fixtures, never fabricated verdicts on a live wire.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HeadToHeadTask {
    /// Stable task id (the demo's `sample_id`).
    pub task_id: String,
    /// The natural-language task prompt the Conductor plans over (the crossy-road
    /// prompt, on the real demo).
    pub prompt: String,
    /// The composed arm's verdict for this task (replay-validator verdict).
    pub composed_verdict: VerificationVerdict,
    /// The composed arm's total spend across its plan's steps, in the reward
    /// adapter's cost units (sats on the paid lane, fixture units here).
    pub composed_cost: f32,
    /// The single-model baseline arm's verdict for this task.
    pub single_verdict: VerificationVerdict,
    /// The single-model baseline arm's spend for this task.
    pub single_cost: f32,
    /// Which eligible-pool worker the single-model baseline routes to one-shot
    /// (validated against the binding; the baseline must also respect the
    /// capability gate).
    pub single_worker_index: usize,
}

/// A fixture set of head-to-head tasks. Non-empty by construction.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HeadToHeadTaskSet {
    tasks: Vec<HeadToHeadTask>,
}

impl HeadToHeadTaskSet {
    /// Builds a task set, rejecting an empty set.
    pub fn new(tasks: Vec<HeadToHeadTask>) -> Result<Self, M8HeadToHeadError> {
        if tasks.is_empty() {
            return Err(M8HeadToHeadError::EmptyTaskSet);
        }
        Ok(Self { tasks })
    }

    /// The fixture tasks.
    #[must_use]
    pub fn tasks(&self) -> &[HeadToHeadTask] {
        &self.tasks
    }

    /// Number of tasks in the set.
    #[must_use]
    pub fn len(&self) -> usize {
        self.tasks.len()
    }

    /// Whether the set is empty (never, post-construction).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Per-arm metric (mirrors the gateway / demo-reducer cost-per-accepted shape).
// ---------------------------------------------------------------------------

/// The cost-per-accepted-outcome metric for one arm, mirroring the demo
/// reducer's per-run shape (`scripts/khala-demo/reduce-head-to-head.mjs`:
/// `accepted` count, `costPerAcceptedOutcome*`, `verifiedRate`) and the gateway's
/// cost-per-accepted-outcome telemetry. Built from an [`ArmOutcome`].
///
/// - `accepted_rate = verified / trajectories` (the demo's `verifiedRate`);
/// - `cost_per_accepted_outcome = total_cost / verified` (the inverse of
///   verified-work-per-sat; `None` when nothing was accepted — division would be
///   undefined, the demo's `"not_applicable"`);
/// - `verified_work_per_sat = verified / total_cost` (the roadmap business
///   metric; `None` on the offline lane where no sats moved).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArmCostMetric {
    /// Trajectories evaluated for this arm.
    pub trajectories: usize,
    /// Accepted (verified) trajectories.
    pub accepted: usize,
    /// Total spend across the arm, in cost units (sats on the paid lane).
    pub total_cost: f32,
    /// Accepted rate `accepted / trajectories` (the demo's `verifiedRate`).
    pub accepted_rate: f32,
    /// Cost per accepted outcome `total_cost / accepted` — the gateway's headline
    /// cost metric. `None` when nothing was accepted (undefined denominator).
    pub cost_per_accepted_outcome: Option<f32>,
    /// Verified-work-per-sat `accepted / total_cost` — the roadmap business
    /// metric. `None` on the offline lane (no sat denominator).
    pub verified_work_per_sat: Option<f32>,
}

impl ArmCostMetric {
    /// Builds the metric from an aggregated [`ArmOutcome`].
    #[must_use]
    pub fn from_arm(arm: ArmOutcome) -> Self {
        let accepted_rate = arm.verified_rate();
        let cost_per_accepted_outcome = if arm.verified > 0 && arm.total_cost > 0.0 {
            Some(arm.total_cost / arm.verified as f32)
        } else {
            None
        };
        Self {
            trajectories: arm.trajectories,
            accepted: arm.verified,
            total_cost: arm.total_cost,
            accepted_rate,
            cost_per_accepted_outcome,
            verified_work_per_sat: arm.verified_work_per_sat(),
        }
    }
}

// ---------------------------------------------------------------------------
// The head-to-head verdict.
// ---------------------------------------------------------------------------

/// The M8 win/loss verdict. This is the layer on top of [`ShadowComparison`]
/// that encodes the demo's exact claim: **"compose to win, cheaper" is true only
/// when composition's verified-work-per-sat beats single-model at comparable
/// quality**.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeadToHeadVerdict {
    /// Composition strictly beat single-model on verified-work-per-sat (cost) AND
    /// held comparable quality (accepted rate not materially below single-model).
    /// This is the only verdict that lets the demo publish "compose to win,
    /// cheaper".
    ComposeToWinCheaper,
    /// Composition was cheaper (or equal) per accepted outcome but its quality
    /// dropped below the parity tolerance — the cheaper claim is not honest
    /// because the win came from accepting fewer / worse outcomes.
    CheaperButLowerQuality,
    /// Single-model was at least as good on the cost lane — composition did not
    /// win. (Includes ties: a tie is not a win.)
    SingleModelNotBeaten,
    /// The comparison fell back to the offline (verified-rate) lane because at
    /// least one arm moved no sats, so there is no cost win to claim. The demo's
    /// cheaper-than-single-model claim requires the paid (per-sat) lane.
    NoCostLaneOffline,
}

impl HeadToHeadVerdict {
    /// Whether this verdict supports the demo's "compose to win, cheaper" claim.
    #[must_use]
    pub const fn is_compose_to_win(self) -> bool {
        matches!(self, Self::ComposeToWinCheaper)
    }
}

// ---------------------------------------------------------------------------
// The head-to-head report.
// ---------------------------------------------------------------------------

/// The deterministic, typed head-to-head report: composed vs single-model on
/// quality (accepted rate), on cost (per accepted outcome and
/// verified-work-per-sat), and the win/loss verdict. This is the Psionic-side
/// evaluation artifact the publication-side demo reducer consumes (it produces
/// the per-lane metrics the manifest carries).
///
/// Deterministic: the same task set + pool + policy + reward adapter → the same
/// report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HeadToHeadReport {
    /// Number of tasks evaluated.
    pub task_count: usize,
    /// The composed (Conductor over the M6 pool) arm's cost metric.
    pub composed: ArmCostMetric,
    /// The single-model baseline arm's cost metric.
    pub single_model: ArmCostMetric,
    /// The underlying shadow comparison (composed = learned arm, single-model =
    /// heuristic/baseline arm). Carries the lane, the per-arm lane metric, the
    /// confidence band, and the promote/hold/rollback recommendation.
    pub shadow: ShadowComparison,
    /// Whether composition held comparable quality (composed accepted rate is not
    /// more than [`QUALITY_PARITY_EPSILON`] below single-model's).
    pub quality_comparable: bool,
    /// The M8 win/loss verdict.
    pub verdict: HeadToHeadVerdict,
    /// The distinct eligible-pool worker indices the composed arm fanned out
    /// across (the "compose-across-the-map" set the Verse view renders), unioned
    /// over all task plans.
    pub composed_worker_fanout: Vec<usize>,
    /// Human-readable summary for receipts and logs.
    pub summary: String,
}

impl HeadToHeadReport {
    /// Whether the report supports publishing "compose to win, cheaper".
    #[must_use]
    pub fn composition_wins(&self) -> bool {
        self.verdict.is_compose_to_win()
    }
}

// ---------------------------------------------------------------------------
// The harness.
// ---------------------------------------------------------------------------

/// The M8 head-to-head evaluation harness. **Default-off / fixture-only.** It
/// runs the composed (Conductor) arm vs the single-model baseline arm over a
/// fixture task set and emits a deterministic [`HeadToHeadReport`]. It dispatches
/// no work, moves no sats, and starts no training. The live armed run is the
/// owner gate ([`M8HeadToHeadReadiness`]).
pub struct M8HeadToHeadHarness<P: ConductorPolicy> {
    planner: ConductorPlanner<P>,
    reward: TerminalRewardAdapter,
    quality_parity_epsilon: f32,
}

impl<P: ConductorPolicy> M8HeadToHeadHarness<P> {
    /// Builds a harness over a Conductor planner (composed arm) and a reward
    /// adapter. Uses the default [`QUALITY_PARITY_EPSILON`].
    #[must_use]
    pub fn new(planner: ConductorPlanner<P>, reward: TerminalRewardAdapter) -> Self {
        Self {
            planner,
            reward,
            quality_parity_epsilon: QUALITY_PARITY_EPSILON,
        }
    }

    /// Builds a harness with an explicit quality-parity tolerance.
    #[must_use]
    pub fn with_quality_parity_epsilon(
        planner: ConductorPlanner<P>,
        reward: TerminalRewardAdapter,
        quality_parity_epsilon: f32,
    ) -> Self {
        Self {
            planner,
            reward,
            quality_parity_epsilon,
        }
    }

    /// The reward adapter in force (offline on the fixture lane, cost-aware on the
    /// paid lane).
    #[must_use]
    pub fn reward(&self) -> TerminalRewardAdapter {
        self.reward
    }

    /// The eligible worker pool both arms are bound to.
    #[must_use]
    pub fn pool(&self) -> &WorkerPoolBinding {
        self.planner.pool()
    }

    /// Runs the head-to-head over `task_set` and emits the report.
    ///
    /// For each task it builds the composed plan over the pool (proving the
    /// language plan only names receipt-eligible workers), validates the
    /// single-model baseline's worker index against the same pool, then scores
    /// both arms' fixture outcomes. It dispatches nothing and moves no sats.
    pub fn run(&self, task_set: &HeadToHeadTaskSet) -> Result<HeadToHeadReport, M8HeadToHeadError> {
        let pool_len = self.pool().len();

        let mut composed_outcomes: Vec<TrajectoryOutcome> = Vec::with_capacity(task_set.len());
        let mut single_outcomes: Vec<TrajectoryOutcome> = Vec::with_capacity(task_set.len());
        let mut fanout: std::collections::BTreeSet<usize> = std::collections::BTreeSet::new();

        for task in task_set.tasks() {
            // Composed arm: build + validate the plan over the M6 pool. This is
            // where the capability gate is honored — a plan naming a worker
            // outside the eligible binding is a harness-level failure, never a
            // silently scored outcome.
            let plan: ConductorPlan = self.planner.plan(&task.prompt).map_err(|err| {
                M8HeadToHeadError::ComposedPlanFailed {
                    task_id: task.task_id.clone(),
                    detail: err.to_string(),
                }
            })?;
            fanout.extend(plan.worker_fanout());

            // Single-model baseline: it must also respect the capability gate.
            if task.single_worker_index >= pool_len {
                return Err(M8HeadToHeadError::BaselineWorkerOutOfPool {
                    task_id: task.task_id.clone(),
                    worker_index: task.single_worker_index,
                    pool_len,
                });
            }

            composed_outcomes.push(TrajectoryOutcome {
                verdict: task.composed_verdict,
                cost: task.composed_cost,
            });
            single_outcomes.push(TrajectoryOutcome {
                verdict: task.single_verdict,
                cost: task.single_cost,
            });
        }

        let composed_arm = ArmOutcome::from_outcomes(&composed_outcomes);
        let single_arm = ArmOutcome::from_outcomes(&single_outcomes);
        let composed = ArmCostMetric::from_arm(composed_arm);
        let single_model = ArmCostMetric::from_arm(single_arm);

        // Reuse the M6 shadow-comparison decision logic: composed = "learned",
        // single-model = "heuristic/baseline".
        let shadow = ShadowComparison::compare(&composed_outcomes, &single_outcomes);

        // Quality parity: composed accepted rate must not be materially below the
        // single-model accepted rate.
        let quality_comparable =
            composed.accepted_rate >= single_model.accepted_rate - self.quality_parity_epsilon;

        let verdict = Self::verdict_for(&shadow, quality_comparable);

        let composed_worker_fanout: Vec<usize> = fanout.into_iter().collect();

        let summary = Self::summarize(
            task_set.len(),
            &composed,
            &single_model,
            quality_comparable,
            verdict,
        );

        Ok(HeadToHeadReport {
            task_count: task_set.len(),
            composed,
            single_model,
            shadow,
            quality_comparable,
            verdict,
            composed_worker_fanout,
            summary,
        })
    }

    /// The M8 win verdict from the shadow comparison + quality parity. The
    /// "compose to win, cheaper" verdict fires ONLY when the comparison ran on the
    /// paid (verified-work-per-sat) lane, composition strictly won that lane, and
    /// quality stayed comparable.
    fn verdict_for(shadow: &ShadowComparison, quality_comparable: bool) -> HeadToHeadVerdict {
        use crate::coordinator_shadow_comparison::ComparisonLane;
        match shadow.lane {
            // Offline lane: no sat denominator -> no cost win to claim.
            ComparisonLane::VerifiedRate => HeadToHeadVerdict::NoCostLaneOffline,
            ComparisonLane::VerifiedWorkPerSat => {
                if !shadow.learned_wins {
                    // Composition did not strictly beat single-model on cost
                    // (includes ties).
                    HeadToHeadVerdict::SingleModelNotBeaten
                } else if quality_comparable {
                    HeadToHeadVerdict::ComposeToWinCheaper
                } else {
                    HeadToHeadVerdict::CheaperButLowerQuality
                }
            }
        }
    }

    fn summarize(
        task_count: usize,
        composed: &ArmCostMetric,
        single_model: &ArmCostMetric,
        quality_comparable: bool,
        verdict: HeadToHeadVerdict,
    ) -> String {
        let fmt_cpao = |m: &ArmCostMetric| match m.cost_per_accepted_outcome {
            Some(v) => format!("{v:.6}"),
            None => String::from("n/a"),
        };
        let fmt_vws = |m: &ArmCostMetric| match m.verified_work_per_sat {
            Some(v) => format!("{v:.6}"),
            None => String::from("n/a (offline)"),
        };
        format!(
            "head-to-head over {task_count} task(s): composed accepted {}/{} (rate {:.4}, \
             cost/accepted {}, vws {}) vs single-model accepted {}/{} (rate {:.4}, \
             cost/accepted {}, vws {}); quality_comparable={quality_comparable} -> {verdict:?}",
            composed.accepted,
            composed.trajectories,
            composed.accepted_rate,
            fmt_cpao(composed),
            fmt_vws(composed),
            single_model.accepted,
            single_model.trajectories,
            single_model.accepted_rate,
            fmt_cpao(single_model),
            fmt_vws(single_model),
        )
    }
}

// ---------------------------------------------------------------------------
// Readiness gate: what stands between this fixture harness and the armed run.
// ---------------------------------------------------------------------------

/// The precise gate list between this fixture harness and the M8 armed run — the
/// live, spend-enabled head-to-head whose result the demo publishes. Mirrors
/// M7's [`ConductorReadiness`](crate::ConductorReadiness): every field is `false`
/// in the shipped harness; flipping them is owner / compute work, NOT code that
/// lands here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct M8HeadToHeadReadiness {
    /// The composed arm runs a real Conductor policy (a trained 7B over the M6
    /// pool), not a fixture plan (compute; M7 `policy_backend_wired` +
    /// `training_run_executed`).
    pub composed_arm_live: bool,
    /// The single-model baseline runs a real frontier endpoint, not a fixture
    /// outcome.
    pub single_model_baseline_live: bool,
    /// The [`EvalVerdictSource`](crate::EvalVerdictSource) is **armed** over the
    /// live Pylon pool (M4, #6012) with a spend-enabled buy-mode campaign, so the
    /// per-arm outcomes are real verified evals (owner).
    pub paid_verdict_source_armed: bool,
    /// A paid head-to-head run has been recorded where composition's
    /// verified-work-per-sat beat single-model at comparable quality under the M2
    /// rubric (the #6016 Done-when proof; owner + M4 + M6).
    pub paid_compose_to_win_recorded: bool,
    /// The demo manifest's closure audit returns `canClose: true` for a live
    /// manifest (publication-side, owner) — the openagents-repo reducer's bar.
    pub demo_closure_audit_passes: bool,
}

impl M8HeadToHeadReadiness {
    /// The shipped fixture-harness state: everything ahead is owner/compute-gated.
    #[must_use]
    pub const fn fixture() -> Self {
        Self {
            composed_arm_live: false,
            single_model_baseline_live: false,
            paid_verdict_source_armed: false,
            paid_compose_to_win_recorded: false,
            demo_closure_audit_passes: false,
        }
    }

    /// Whether the M8 armed Done-when bar is met (all gates green).
    #[must_use]
    pub const fn armed_done_when_met(&self) -> bool {
        self.composed_arm_live
            && self.single_model_baseline_live
            && self.paid_verdict_source_armed
            && self.paid_compose_to_win_recorded
            && self.demo_closure_audit_passes
    }
}

// ---------------------------------------------------------------------------
// Owner-armed real-run preflight: bounded, explicit, no dispatch.
// ---------------------------------------------------------------------------

/// Explicit owner/cap inputs required before the real M8 head-to-head may leave
/// fixture mode. This is a *preflight* contract only: it does not hold provider
/// credentials, does not dispatch jobs, and does not move sats.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct M8RealRunPreflightOptions {
    /// The owner deliberately approved a real paid M8 comparison for this run.
    pub owner_confirmed: bool,
    /// Human-readable approval reference, issue/comment/runbook id, or operator
    /// handoff id tying the spend decision to a reviewable record.
    pub approval_ref: Option<String>,
    /// Spend ceiling for the real run, in millisats. Must be positive.
    pub spend_cap_msats: u64,
    /// Hard cap on paid eval calls. Must cover both arms for every task.
    pub max_paid_evals: usize,
}

impl M8RealRunPreflightOptions {
    /// Safe default: no owner approval, no approval ref, no spend cap, no eval
    /// cap. A preflight built from this can never dispatch.
    #[must_use]
    pub const fn disarmed() -> Self {
        Self {
            owner_confirmed: false,
            approval_ref: None,
            spend_cap_msats: 0,
            max_paid_evals: 0,
        }
    }
}

/// Reasons the real paid M8 comparison may not be dispatched yet.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M8RealRunDispatchBlocker {
    /// No explicit owner approval was supplied.
    OwnerApprovalMissing,
    /// The approval record is absent or blank.
    ApprovalRefMissing,
    /// The spend cap is zero.
    SpendCapMissing,
    /// The paid eval cap is zero.
    PaidEvalCapMissing,
    /// The paid eval cap does not cover both arms for every task.
    PaidEvalCapTooLow {
        /// Required paid eval calls (`task_count * 2`).
        required: usize,
        /// Provided paid eval cap.
        provided: usize,
    },
    /// The composed arm is still fixture/offline.
    ComposedArmNotLive,
    /// The single-model baseline is still fixture/offline.
    SingleModelBaselineNotLive,
    /// The paid verdict source is not armed over the live Pylon pool.
    PaidVerdictSourceNotArmed,
}

/// Reasons the M8 publication Done-when is not closed yet, even if dispatch is
/// otherwise allowed. These are post-run evidence gates, not pre-dispatch gates.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum M8RealRunPublicationBlocker {
    /// No paid `ComposeToWinCheaper` result has been recorded yet.
    PaidComposeToWinNotRecorded,
    /// The openagents demo closure audit has not passed for a live manifest.
    DemoClosureAuditNotPassed,
}

/// Pure preflight for the real M8 head-to-head lane. It separates launch
/// blockers (owner/cap/live-arm gates) from publication blockers (post-run
/// evidence gates) so an operator can see exactly what remains without
/// accidentally dispatching work.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct M8RealRunPreflight {
    /// Number of tasks in the head-to-head set.
    pub task_count: usize,
    /// Paid eval calls needed to run both arms for every task.
    pub planned_paid_evals: usize,
    /// The supplied spend ceiling in millisats.
    pub spend_cap_msats: u64,
    /// The supplied paid eval cap.
    pub max_paid_evals: usize,
    /// Blockers that prevent dispatching the real paid comparison.
    pub dispatch_blockers: Vec<M8RealRunDispatchBlocker>,
    /// Blockers that prevent publishing/closing M8 Done-when.
    pub publication_blockers: Vec<M8RealRunPublicationBlocker>,
}

impl M8RealRunPreflight {
    /// Builds the real-run preflight from the non-empty task set, readiness gates,
    /// and explicit owner/cap inputs. This performs no dispatch and moves no sats.
    #[must_use]
    pub fn new(
        task_set: &HeadToHeadTaskSet,
        readiness: M8HeadToHeadReadiness,
        options: M8RealRunPreflightOptions,
    ) -> Self {
        let task_count = task_set.len();
        let planned_paid_evals = task_count.saturating_mul(2);
        let mut dispatch_blockers = Vec::new();
        let mut publication_blockers = Vec::new();

        if !options.owner_confirmed {
            dispatch_blockers.push(M8RealRunDispatchBlocker::OwnerApprovalMissing);
        }
        if options
            .approval_ref
            .as_deref()
            .map(str::trim)
            .unwrap_or_default()
            .is_empty()
        {
            dispatch_blockers.push(M8RealRunDispatchBlocker::ApprovalRefMissing);
        }
        if options.spend_cap_msats == 0 {
            dispatch_blockers.push(M8RealRunDispatchBlocker::SpendCapMissing);
        }
        if options.max_paid_evals == 0 {
            dispatch_blockers.push(M8RealRunDispatchBlocker::PaidEvalCapMissing);
        } else if options.max_paid_evals < planned_paid_evals {
            dispatch_blockers.push(M8RealRunDispatchBlocker::PaidEvalCapTooLow {
                required: planned_paid_evals,
                provided: options.max_paid_evals,
            });
        }
        if !readiness.composed_arm_live {
            dispatch_blockers.push(M8RealRunDispatchBlocker::ComposedArmNotLive);
        }
        if !readiness.single_model_baseline_live {
            dispatch_blockers.push(M8RealRunDispatchBlocker::SingleModelBaselineNotLive);
        }
        if !readiness.paid_verdict_source_armed {
            dispatch_blockers.push(M8RealRunDispatchBlocker::PaidVerdictSourceNotArmed);
        }

        if !readiness.paid_compose_to_win_recorded {
            publication_blockers.push(M8RealRunPublicationBlocker::PaidComposeToWinNotRecorded);
        }
        if !readiness.demo_closure_audit_passes {
            publication_blockers.push(M8RealRunPublicationBlocker::DemoClosureAuditNotPassed);
        }

        Self {
            task_count,
            planned_paid_evals,
            spend_cap_msats: options.spend_cap_msats,
            max_paid_evals: options.max_paid_evals,
            dispatch_blockers,
            publication_blockers,
        }
    }

    /// Whether the real paid comparison is allowed to dispatch.
    #[must_use]
    pub fn can_dispatch_real_eval(&self) -> bool {
        self.dispatch_blockers.is_empty()
    }

    /// Whether M8's publication Done-when can be closed.
    #[must_use]
    pub fn publishable_done_when(&self) -> bool {
        self.can_dispatch_real_eval() && self.publication_blockers.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coordinator_conductor::{AccessList, ConductorRawPlan};
    use crate::coordinator_evolution::{WorkerKind, WorkerPoolMember};

    // ---- Fixtures ---------------------------------------------------------

    fn pool(n: usize) -> WorkerPoolBinding {
        let members: Vec<WorkerPoolMember> = (0..n)
            .map(|i| WorkerPoolMember {
                worker_id: format!("worker-{i:02}"),
                kind: if i == 0 {
                    WorkerKind::Frontier
                } else {
                    WorkerKind::Open
                },
                receipted_capabilities: ["code".to_string()].into_iter().collect(),
            })
            .collect();
        WorkerPoolBinding::from_candidates(members, "code").expect("pool")
    }

    /// A fixture policy that emits a fixed 3-step plan (plan -> implement ->
    /// verify) fanning out across workers 0, 1, 2. No model weights.
    struct FixturePolicy;

    impl ConductorPolicy for FixturePolicy {
        fn generate_plan(
            &self,
            _task_prompt: &str,
            _pool_len: usize,
        ) -> Result<ConductorRawPlan, crate::coordinator_conductor::ConductorError> {
            Ok(ConductorRawPlan {
                model_id: vec![0, 1, 2],
                subtasks: vec!["plan".into(), "implement".into(), "verify".into()],
                access_list: vec![
                    AccessList::None,
                    AccessList::Indices(vec![0]),
                    AccessList::All,
                ],
            })
        }
    }

    /// A policy that names a worker outside a 2-worker pool, to prove the harness
    /// surfaces an invalid composed plan as an error (capability gate honored).
    struct OutOfPoolPolicy;

    impl ConductorPolicy for OutOfPoolPolicy {
        fn generate_plan(
            &self,
            _task_prompt: &str,
            _pool_len: usize,
        ) -> Result<ConductorRawPlan, crate::coordinator_conductor::ConductorError> {
            Ok(ConductorRawPlan {
                model_id: vec![0, 9],
                subtasks: vec!["plan".into(), "implement".into()],
                access_list: vec![AccessList::None, AccessList::None],
            })
        }
    }

    fn harness(reward: TerminalRewardAdapter) -> M8HeadToHeadHarness<FixturePolicy> {
        let planner = ConductorPlanner::new(FixturePolicy, pool(3), 5).expect("planner");
        M8HeadToHeadHarness::new(planner, reward)
    }

    fn task(
        id: &str,
        composed_verdict: VerificationVerdict,
        composed_cost: f32,
        single_verdict: VerificationVerdict,
        single_cost: f32,
    ) -> HeadToHeadTask {
        HeadToHeadTask {
            task_id: id.to_string(),
            prompt: "build a crossy-road game with three.js".to_string(),
            composed_verdict,
            composed_cost,
            single_verdict,
            single_cost,
            single_worker_index: 0,
        }
    }

    use VerificationVerdict::{Rejected, Verified};

    // ---- ArmCostMetric math ----------------------------------------------

    #[test]
    fn arm_cost_metric_matches_gateway_shape() {
        // 3 accepted of 4, total cost 40 sats.
        let arm = ArmOutcome {
            trajectories: 4,
            verified: 3,
            total_cost: 40.0,
        };
        let m = ArmCostMetric::from_arm(arm);
        assert_eq!(m.trajectories, 4);
        assert_eq!(m.accepted, 3);
        assert!((m.accepted_rate - 0.75).abs() < 1e-6);
        // cost-per-accepted-outcome = 40 / 3.
        assert!((m.cost_per_accepted_outcome.unwrap() - (40.0 / 3.0)).abs() < 1e-6);
        // verified-work-per-sat = 3 / 40.
        assert!((m.verified_work_per_sat.unwrap() - (3.0 / 40.0)).abs() < 1e-6);
    }

    #[test]
    fn arm_cost_metric_no_accepted_has_no_cost_per_accepted() {
        let arm = ArmOutcome {
            trajectories: 2,
            verified: 0,
            total_cost: 20.0,
        };
        let m = ArmCostMetric::from_arm(arm);
        assert_eq!(m.accepted, 0);
        assert!((m.accepted_rate - 0.0).abs() < 1e-6);
        // No accepted outcome -> cost-per-accepted is undefined (the demo's
        // "not_applicable").
        assert_eq!(m.cost_per_accepted_outcome, None);
        // Sats DID move (cost 20) but produced zero verified work, so
        // verified-work-per-sat is a meaningful 0.0 (burned sats, got nothing),
        // not None. None is reserved for the offline lane (no sat denominator).
        assert_eq!(m.verified_work_per_sat, Some(0.0));
    }

    #[test]
    fn arm_cost_metric_offline_has_no_sat_denominator() {
        let arm = ArmOutcome {
            trajectories: 2,
            verified: 1,
            total_cost: 0.0,
        };
        let m = ArmCostMetric::from_arm(arm);
        assert_eq!(m.cost_per_accepted_outcome, None);
        assert_eq!(m.verified_work_per_sat, None);
        assert!((m.accepted_rate - 0.5).abs() < 1e-6);
    }

    // ---- Empty / invalid task sets ---------------------------------------

    #[test]
    fn empty_task_set_is_rejected() {
        assert_eq!(
            HeadToHeadTaskSet::new(vec![]).unwrap_err(),
            M8HeadToHeadError::EmptyTaskSet
        );
    }

    #[test]
    fn invalid_composed_plan_surfaces_as_error_not_panic() {
        // OutOfPoolPolicy names worker 9 in a 2-worker pool -> the composed plan
        // fails validation; the harness surfaces it as a structured error.
        let planner = ConductorPlanner::new(OutOfPoolPolicy, pool(2), 5).expect("planner");
        let h = M8HeadToHeadHarness::new(planner, TerminalRewardAdapter::cost_aware(0.001));
        let set = HeadToHeadTaskSet::new(vec![task("t0", Verified, 10.0, Verified, 10.0)]).unwrap();
        assert!(matches!(
            h.run(&set),
            Err(M8HeadToHeadError::ComposedPlanFailed { .. })
        ));
    }

    #[test]
    fn single_model_baseline_must_respect_pool() {
        let h = harness(TerminalRewardAdapter::cost_aware(0.001));
        let mut t = task("t0", Verified, 10.0, Verified, 10.0);
        t.single_worker_index = 7; // pool has 3 workers.
        let set = HeadToHeadTaskSet::new(vec![t]).unwrap();
        assert_eq!(
            h.run(&set).unwrap_err(),
            M8HeadToHeadError::BaselineWorkerOutOfPool {
                task_id: "t0".to_string(),
                worker_index: 7,
                pool_len: 3,
            }
        );
    }

    // ---- The win verdict: compose to win, cheaper -------------------------

    #[test]
    fn compose_to_win_cheaper_fires_on_paid_lane_win_at_comparable_quality() {
        // Composed: 4/4 accepted, cheap (10 sats each => 40 sats; vws = 4/40 = 0.1).
        // Single:   4/4 accepted, pricey (30 sats each => 120 sats; vws = 4/120 ~ 0.033).
        // Composition wins on cost AND quality is equal -> compose-to-win.
        let h = harness(TerminalRewardAdapter::cost_aware(0.001));
        let set = HeadToHeadTaskSet::new(vec![
            task("t0", Verified, 10.0, Verified, 30.0),
            task("t1", Verified, 10.0, Verified, 30.0),
            task("t2", Verified, 10.0, Verified, 30.0),
            task("t3", Verified, 10.0, Verified, 30.0),
        ])
        .unwrap();
        let report = h.run(&set).expect("report");
        assert_eq!(report.task_count, 4);
        assert!((report.composed.accepted_rate - 1.0).abs() < 1e-6);
        assert!((report.single_model.accepted_rate - 1.0).abs() < 1e-6);
        // composed vws 0.1 > single vws ~0.0333.
        assert!(
            report.composed.verified_work_per_sat.unwrap()
                > report.single_model.verified_work_per_sat.unwrap()
        );
        assert!(report.quality_comparable);
        assert_eq!(report.verdict, HeadToHeadVerdict::ComposeToWinCheaper);
        assert!(report.composition_wins());
        // The composed arm fanned out across all 3 workers (plan uses 0,1,2).
        assert_eq!(report.composed_worker_fanout, vec![0, 1, 2]);
    }

    #[test]
    fn cheaper_but_lower_quality_does_not_win() {
        // Composed cheaper per-sat BUT accepts far fewer outcomes: the per-sat win
        // comes from skipping work, not from being better. Must NOT be a win.
        // Composed: 2/4 accepted at 5 sats each (only verified ones cost) -> vws high.
        // Single:   4/4 accepted at 30 sats each -> vws lower, but full quality.
        let h = harness(TerminalRewardAdapter::cost_aware(0.001));
        let set = HeadToHeadTaskSet::new(vec![
            task("t0", Verified, 5.0, Verified, 30.0),
            task("t1", Verified, 5.0, Verified, 30.0),
            task("t2", Rejected, 5.0, Verified, 30.0),
            task("t3", Rejected, 5.0, Verified, 30.0),
        ])
        .unwrap();
        let report = h.run(&set).expect("report");
        // composed vws = 2/20 = 0.1 ; single vws = 4/120 ~ 0.0333 -> composed wins per-sat.
        assert!(
            report.composed.verified_work_per_sat.unwrap()
                > report.single_model.verified_work_per_sat.unwrap()
        );
        // BUT composed accepted rate 0.5 vs single 1.0 -> quality not comparable.
        assert!(!report.quality_comparable);
        assert_eq!(report.verdict, HeadToHeadVerdict::CheaperButLowerQuality);
        assert!(!report.composition_wins());
    }

    #[test]
    fn single_model_not_beaten_when_composition_is_pricier() {
        // Composed pricier per accepted outcome -> single-model not beaten.
        let h = harness(TerminalRewardAdapter::cost_aware(0.001));
        let set = HeadToHeadTaskSet::new(vec![
            task("t0", Verified, 50.0, Verified, 10.0),
            task("t1", Verified, 50.0, Verified, 10.0),
        ])
        .unwrap();
        let report = h.run(&set).expect("report");
        // composed vws = 2/100 = 0.02 ; single vws = 2/20 = 0.10 -> single better.
        assert!(
            report.composed.verified_work_per_sat.unwrap()
                < report.single_model.verified_work_per_sat.unwrap()
        );
        assert_eq!(report.verdict, HeadToHeadVerdict::SingleModelNotBeaten);
        assert!(!report.composition_wins());
    }

    #[test]
    fn tie_on_cost_is_not_a_win() {
        // Identical per-sat metrics -> tie -> not a win.
        let h = harness(TerminalRewardAdapter::cost_aware(0.001));
        let set = HeadToHeadTaskSet::new(vec![
            task("t0", Verified, 10.0, Verified, 10.0),
            task("t1", Verified, 10.0, Verified, 10.0),
        ])
        .unwrap();
        let report = h.run(&set).expect("report");
        assert!(
            (report.composed.verified_work_per_sat.unwrap()
                - report.single_model.verified_work_per_sat.unwrap())
            .abs()
                < 1e-6
        );
        assert_eq!(report.verdict, HeadToHeadVerdict::SingleModelNotBeaten);
    }

    #[test]
    fn offline_lane_has_no_cost_win() {
        // No sats moved in either arm (fixture offline lane). The compare falls
        // back to verified rate; there is no cost win to claim even if composed
        // accepts more.
        let h = harness(TerminalRewardAdapter::offline());
        let set = HeadToHeadTaskSet::new(vec![
            task("t0", Verified, 0.0, Verified, 0.0),
            task("t1", Verified, 0.0, Rejected, 0.0),
        ])
        .unwrap();
        let report = h.run(&set).expect("report");
        assert_eq!(report.composed.verified_work_per_sat, None);
        assert_eq!(report.single_model.verified_work_per_sat, None);
        assert_eq!(report.verdict, HeadToHeadVerdict::NoCostLaneOffline);
        assert!(!report.composition_wins());
    }

    #[test]
    fn quality_parity_epsilon_allows_a_small_quality_dip() {
        // Composed accepts slightly fewer (0.9 vs 0.95 single) but within epsilon
        // 0.05, and wins on cost -> still a compose-to-win.
        let h = M8HeadToHeadHarness::with_quality_parity_epsilon(
            ConductorPlanner::new(FixturePolicy, pool(3), 5).expect("planner"),
            TerminalRewardAdapter::cost_aware(0.001),
            0.05,
        );
        // Composed: 18/20 accepted (0.90), cheap. Single: 19/20 accepted (0.95), pricey.
        let mut tasks = Vec::new();
        for i in 0..20 {
            let composed_v = if i < 18 { Verified } else { Rejected };
            let single_v = if i < 19 { Verified } else { Rejected };
            tasks.push(task(&format!("t{i}"), composed_v, 5.0, single_v, 30.0));
        }
        let set = HeadToHeadTaskSet::new(tasks).unwrap();
        let report = h.run(&set).expect("report");
        // 0.90 >= 0.95 - 0.05 -> comparable.
        assert!(report.quality_comparable);
        // composed vws = 18/100 = 0.18 ; single vws = 19/570 ~ 0.0333 -> composed wins.
        assert!(
            report.composed.verified_work_per_sat.unwrap()
                > report.single_model.verified_work_per_sat.unwrap()
        );
        assert_eq!(report.verdict, HeadToHeadVerdict::ComposeToWinCheaper);
    }

    // ---- Determinism ------------------------------------------------------

    #[test]
    fn report_is_deterministic() {
        let h = harness(TerminalRewardAdapter::cost_aware(0.001));
        let set = HeadToHeadTaskSet::new(vec![
            task("t0", Verified, 10.0, Verified, 30.0),
            task("t1", Rejected, 10.0, Verified, 30.0),
        ])
        .unwrap();
        let a = h.run(&set).expect("a");
        let b = h.run(&set).expect("b");
        assert_eq!(a, b);
    }

    #[test]
    fn report_serializes_round_trip() {
        let h = harness(TerminalRewardAdapter::cost_aware(0.001));
        let set = HeadToHeadTaskSet::new(vec![task("t0", Verified, 10.0, Verified, 30.0)]).unwrap();
        let report = h.run(&set).expect("report");
        let json = serde_json::to_string(&report).expect("ser");
        let back: HeadToHeadReport = serde_json::from_str(&json).expect("de");
        assert_eq!(report, back);
    }

    // ---- Default-off / readiness gate ------------------------------------

    #[test]
    fn harness_dispatches_nothing_and_moves_no_sats() {
        // The harness has no dispatch seam at all: running it only consumes
        // fixture outcomes. This test documents the invariant by construction —
        // a successful run produces a report without any BuyModeDispatch.
        let h = harness(TerminalRewardAdapter::cost_aware(0.001));
        let set = HeadToHeadTaskSet::new(vec![task("t0", Verified, 10.0, Verified, 30.0)]).unwrap();
        let report = h.run(&set).expect("report");
        // The composed "spend" is a fixture cost metric, not a debited ledger.
        assert!(report.composed.total_cost > 0.0);
    }

    #[test]
    fn readiness_is_all_owner_compute_gated_in_the_fixture_harness() {
        let r = M8HeadToHeadReadiness::fixture();
        assert!(!r.armed_done_when_met());
        assert!(!r.composed_arm_live);
        assert!(!r.single_model_baseline_live);
        assert!(!r.paid_verdict_source_armed);
        assert!(!r.paid_compose_to_win_recorded);
        assert!(!r.demo_closure_audit_passes);
    }

    #[test]
    fn readiness_done_when_requires_all_gates() {
        let r = M8HeadToHeadReadiness {
            composed_arm_live: true,
            single_model_baseline_live: true,
            paid_verdict_source_armed: true,
            paid_compose_to_win_recorded: true,
            demo_closure_audit_passes: true,
        };
        assert!(r.armed_done_when_met());
    }

    // ---- Owner-armed real-run preflight -----------------------------------

    fn live_ready_before_publication() -> M8HeadToHeadReadiness {
        M8HeadToHeadReadiness {
            composed_arm_live: true,
            single_model_baseline_live: true,
            paid_verdict_source_armed: true,
            paid_compose_to_win_recorded: false,
            demo_closure_audit_passes: false,
        }
    }

    fn preflight_task_set(n: usize) -> HeadToHeadTaskSet {
        let tasks = (0..n)
            .map(|i| task(&format!("t{i}"), Verified, 10.0, Verified, 30.0))
            .collect();
        HeadToHeadTaskSet::new(tasks).expect("preflight task set")
    }

    #[test]
    fn real_run_preflight_is_disarmed_by_default_and_moves_nothing() {
        let set = preflight_task_set(2);
        let preflight = M8RealRunPreflight::new(
            &set,
            M8HeadToHeadReadiness::fixture(),
            M8RealRunPreflightOptions::disarmed(),
        );

        assert_eq!(preflight.task_count, 2);
        assert_eq!(preflight.planned_paid_evals, 4);
        assert!(!preflight.can_dispatch_real_eval());
        assert!(!preflight.publishable_done_when());
        assert_eq!(
            preflight.dispatch_blockers,
            vec![
                M8RealRunDispatchBlocker::OwnerApprovalMissing,
                M8RealRunDispatchBlocker::ApprovalRefMissing,
                M8RealRunDispatchBlocker::SpendCapMissing,
                M8RealRunDispatchBlocker::PaidEvalCapMissing,
                M8RealRunDispatchBlocker::ComposedArmNotLive,
                M8RealRunDispatchBlocker::SingleModelBaselineNotLive,
                M8RealRunDispatchBlocker::PaidVerdictSourceNotArmed,
            ]
        );
        assert_eq!(
            preflight.publication_blockers,
            vec![
                M8RealRunPublicationBlocker::PaidComposeToWinNotRecorded,
                M8RealRunPublicationBlocker::DemoClosureAuditNotPassed,
            ]
        );
    }

    #[test]
    fn real_run_preflight_requires_eval_cap_for_both_arms() {
        let set = preflight_task_set(3);
        let preflight = M8RealRunPreflight::new(
            &set,
            live_ready_before_publication(),
            M8RealRunPreflightOptions {
                owner_confirmed: true,
                approval_ref: Some("openagents/psionic#1142 owner approval".to_string()),
                spend_cap_msats: 1_000_000,
                max_paid_evals: 5,
            },
        );

        assert_eq!(preflight.planned_paid_evals, 6);
        assert!(!preflight.can_dispatch_real_eval());
        assert_eq!(
            preflight.dispatch_blockers,
            vec![M8RealRunDispatchBlocker::PaidEvalCapTooLow {
                required: 6,
                provided: 5,
            }]
        );
    }

    #[test]
    fn real_run_preflight_allows_dispatch_before_publication_done_when() {
        let set = preflight_task_set(3);
        let preflight = M8RealRunPreflight::new(
            &set,
            live_ready_before_publication(),
            M8RealRunPreflightOptions {
                owner_confirmed: true,
                approval_ref: Some("openagents/psionic#1142 owner approval".to_string()),
                spend_cap_msats: 1_000_000,
                max_paid_evals: 6,
            },
        );

        assert!(preflight.can_dispatch_real_eval());
        assert!(!preflight.publishable_done_when());
        assert!(preflight.dispatch_blockers.is_empty());
        assert_eq!(
            preflight.publication_blockers,
            vec![
                M8RealRunPublicationBlocker::PaidComposeToWinNotRecorded,
                M8RealRunPublicationBlocker::DemoClosureAuditNotPassed,
            ]
        );
    }

    #[test]
    fn real_run_preflight_done_when_requires_recorded_win_and_closure_audit() {
        let set = preflight_task_set(1);
        let preflight = M8RealRunPreflight::new(
            &set,
            M8HeadToHeadReadiness {
                composed_arm_live: true,
                single_model_baseline_live: true,
                paid_verdict_source_armed: true,
                paid_compose_to_win_recorded: true,
                demo_closure_audit_passes: true,
            },
            M8RealRunPreflightOptions {
                owner_confirmed: true,
                approval_ref: Some("openagents/psionic#1142 closeout".to_string()),
                spend_cap_msats: 1_000_000,
                max_paid_evals: 2,
            },
        );

        assert!(preflight.can_dispatch_real_eval());
        assert!(preflight.publishable_done_when());
        assert!(preflight.dispatch_blockers.is_empty());
        assert!(preflight.publication_blockers.is_empty());
    }
}
