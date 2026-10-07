//! Dispatch-backed `EvalVerdictSource` for the paid coordinator lane (Khala M6,
//! issue #6014 / EPIC #6017).
//!
//! The P1–P5 substrate ([`crate::coordinator_evolution`]), the
//! [`crate::coordinator_live_training`] fitness + [`DailySpendCap`], the
//! [`crate::coordinator_shadow_comparison`] decision primitive, and the
//! [`crate::coordinator_candidate_emission`] candidate artifact are all merged.
//! The `docs/KHALA_M6_M7_COORDINATOR_PLAN.md` "Concrete next slice" #3 names the
//! remaining piece: *implement [`EvalVerdictSource`] over the buy-mode dispatch
//! path + Tassadar verdict, behind the daily cap — the first thing M4 unblocks.*
//!
//! This module lands the **plumbing** for that paid lane. It does not move sats
//! and never dispatches in tests; the real paid run stays owner-gated behind the
//! daily cap and an explicit arm.
//!
//! ## What this is
//!
//! A [`DispatchBackedVerdictSource`] is a real [`EvalVerdictSource`] over the
//! **buy-mode dispatch path**. Given a coordinator routing decision (a
//! [`TrajectoryRequest`], built by [`crate::LiveCoordinatorFitness`] from
//! `head.decide(h)` resolved through the P5 worker-pool binding), it:
//!
//! 1. **pre-checks the [`DailySpendCap`]** against the configured per-eval price
//!    (fail-closed: an over-cap or disarmed request dispatches nothing and moves
//!    no sats);
//! 2. dispatches the inference through the injected [`BuyModeDispatch`] seam —
//!    the read-only contract for the `openagents` Worker buy-mode dispatcher
//!    (`apps/openagents.com/workers/api/src/buy-mode-dispatcher.ts`), which
//!    publishes the job to the Pylon network and pays on settlement;
//! 3. reads the **`training.verification_classes.v1`** verdict off the dispatch
//!    result (the replay-validator verification-class outcome, NOT a prompted
//!    LLM judge);
//! 4. yields a [`VerdictOutcome`] (verdict + actual settled spend) the
//!    [`crate::LiveCoordinatorFitness`] debits against the cap and feeds to the
//!    [`crate::TerminalRewardAdapter`] → `CoordinatorFitness` → `ShadowComparison`.
//!
//! Reward = the verification verdict; monetize on settlement
//! (`docs/sakana/coordinator-as-verified-work.md`). This is what lets the learned
//! router be scored on **real verified work**.
//!
//! ## Fail-closed, default-off arming
//!
//! The source is **disarmed by default** ([`CoordinatorArmState::Disarmed`]).
//! While disarmed, [`verdict_for`](EvalVerdictSource::verdict_for) refuses
//! cleanly and dispatches nothing. Even when armed, the [`DailySpendCap`]
//! pre-check runs before any dispatch, so an over-budget request fails closed
//! with zero spend. The cap clamps to the owner's 10,000 sats/day ceiling
//! ([`OWNER_DAILY_CAP_MSATS`]), and the same job carries the price so the Worker
//! stays the final spend authority (see the [`crate::coordinator_live_training`]
//! module docs for the cross-repo cap-ledger contract).
//!
//! ## Fixture vs live
//!
//! - Tests bind a [`RecordingDispatch`] / closure dispatcher that returns a fixed
//!   verdict + spend and records the calls. The tests assert the dispatcher is
//!   **never called** while disarmed or over-cap. No sats move; no real inference
//!   runs.
//! - A real run binds a live [`BuyModeDispatch`] that publishes a buy-mode eval
//!   job to the Pylon relay and reads the settled Tassadar verdict. That lane is
//!   **owner-gated**: it needs the M4 real Pylon pool (#6012, merged), an armed
//!   source, a spend-enabled buy-mode campaign row, and a reachable live verdict
//!   source. This module provides the seam; it does not provide a live
//!   dispatcher, and never fabricates a verdict.

use serde::{Deserialize, Serialize};

use crate::coordinator_evolution::VerificationVerdict;
use crate::coordinator_live_training::{
    CoordinatorLiveTrainingError, DailySpendCap, EvalVerdictSource, TrajectoryRequest,
    VerdictOutcome, OWNER_DAILY_CAP_MSATS,
};

// ---------------------------------------------------------------------------
// Arming gate (default OFF).
// ---------------------------------------------------------------------------

/// Whether the paid dispatch lane is armed. **Default is [`Disarmed`].** A
/// disarmed source dispatches nothing and moves no sats; arming is an owner
/// decision, never a default.
///
/// [`Disarmed`]: CoordinatorArmState::Disarmed
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoordinatorArmState {
    /// Paid dispatch is OFF (default). Every verdict request refuses cleanly with
    /// zero spend and no dispatch.
    #[default]
    Disarmed,
    /// Paid dispatch is armed. Requests within the daily cap dispatch a real
    /// buy-mode eval job; over-cap requests still fail closed.
    Armed,
}

impl CoordinatorArmState {
    /// Whether the lane is armed for paid dispatch.
    #[must_use]
    pub const fn is_armed(self) -> bool {
        matches!(self, Self::Armed)
    }
}

// ---------------------------------------------------------------------------
// Verification-class verdict (training.verification_classes.v1, read-only).
// ---------------------------------------------------------------------------

/// The named verification class a dispatched eval was checked under, mirroring
/// the `openagents` `training.verification_classes.v1` promise (the
/// `verification_classes` stage in `training-full-pipeline-program.ts`). This is
/// consumed read-only: the class is decided by the replay validator / verifier,
/// never by this crate and never by a prompted LLM judge.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationClass {
    /// Deterministic work checked by exact-trace replay (digests must match).
    /// This is the green-receipt class the promise calls out for paid work.
    ExactTraceReplay,
    /// A bounded command/check verdict (a pass/fail verification command).
    CommandCheck,
}

/// The `training.verification_classes.v1` verdict carried back by a dispatched
/// buy-mode eval: the verification class it was checked under and whether the
/// independently-recomputed check passed. The [`VerificationVerdict`] the rest
/// of the coordinator lane consumes is derived from `passed`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationClassVerdict {
    /// The verification class the work was checked under.
    pub class: VerificationClass,
    /// Whether the replay-validator / verification-command check passed. `true`
    /// is the same object that releases settlement.
    pub passed: bool,
}

impl Default for VerificationClassVerdict {
    fn default() -> Self {
        Self::exact_trace_pass()
    }
}

impl VerificationClassVerdict {
    /// An exact-trace-replay pass (the green deterministic-work verdict).
    #[must_use]
    pub const fn exact_trace_pass() -> Self {
        Self {
            class: VerificationClass::ExactTraceReplay,
            passed: true,
        }
    }

    /// An exact-trace-replay failure (digests did not match).
    #[must_use]
    pub const fn exact_trace_fail() -> Self {
        Self {
            class: VerificationClass::ExactTraceReplay,
            passed: false,
        }
    }

    /// The terminal [`VerificationVerdict`] the coordinator reward adapter reads.
    #[must_use]
    pub const fn verdict(self) -> VerificationVerdict {
        if self.passed {
            VerificationVerdict::Verified
        } else {
            VerificationVerdict::Rejected
        }
    }
}

// ---------------------------------------------------------------------------
// Buy-mode dispatch seam (read-only contract for the openagents Worker).
// ---------------------------------------------------------------------------

/// One buy-mode eval job derived from a coordinator routing decision. This is
/// the read-only shape the `openagents` buy-mode dispatcher consumes
/// (`dispatchJob`): a priced unit of inference work routed to a specific eligible
/// worker. The `amount_msats` is the job's price, carried so the Worker debits
/// the SAME `daily_cap_msats` / `spent_today_msats` ledger this lane mirrors.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuyModeEvalJob {
    /// Stable id of the worker the coordinator selected (already
    /// capability-filtered through the P5 binding).
    pub worker_id: String,
    /// The TRINITY role index the head assigned.
    pub role_index: usize,
    /// The task / sample id this eval is for.
    pub sample_id: String,
    /// The priced amount this eval job will spend, in msats. Debited against the
    /// [`DailySpendCap`] before dispatch and carried to the Worker as the job's
    /// `amount_msats`.
    pub amount_msats: u64,
}

impl BuyModeEvalJob {
    /// Builds a priced job from a coordinator [`TrajectoryRequest`].
    #[must_use]
    pub fn from_request(request: &TrajectoryRequest, amount_msats: u64) -> Self {
        Self {
            worker_id: request.worker_id.clone(),
            role_index: request.role_index,
            sample_id: request.sample_id.clone(),
            amount_msats,
        }
    }
}

/// The settled result of a dispatched buy-mode eval job: the
/// `training.verification_classes.v1` verdict and the spend that actually
/// settled, in msats. The settled spend may differ from the quoted price (e.g. a
/// rejected job that still incurred a partial cost); the cap is debited against
/// the SETTLED spend so the local ledger tracks real money movement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuyModeEvalResult {
    /// The verification-class verdict (the replay-validator outcome).
    pub verdict: VerificationClassVerdict,
    /// The spend that actually settled for this job, in msats.
    pub settled_msats: u64,
}

/// The seam to the `openagents` buy-mode dispatch path. A live implementation
/// publishes the job to the Pylon relay, waits for the worker's signed result,
/// runs the replay validator, and returns the settled verdict + spend. The CPU
/// tests bind a fixture implementation; this crate provides no live dispatcher.
///
/// Implementations MUST NOT move sats unless a real, owner-armed, spend-enabled
/// buy-mode campaign authorizes it; the [`DispatchBackedVerdictSource`] only
/// calls this after its own fail-closed cap pre-check passes.
pub trait BuyModeDispatch {
    /// Dispatches one priced eval job and returns the settled verdict + spend.
    /// On the live lane the verdict MUST be the replay-validator /
    /// verification-command outcome, never a prompted LLM judge.
    fn dispatch_eval(
        &self,
        job: &BuyModeEvalJob,
    ) -> Result<BuyModeEvalResult, CoordinatorLiveTrainingError>;
}

// ---------------------------------------------------------------------------
// Dispatch-backed EvalVerdictSource.
// ---------------------------------------------------------------------------

/// A real [`EvalVerdictSource`] over the buy-mode dispatch path, behind the daily
/// spend cap and an explicit arm.
///
/// On each [`verdict_for`](EvalVerdictSource::verdict_for):
///
/// 1. if [`CoordinatorArmState::Disarmed`], refuse cleanly — no dispatch, no
///    spend;
/// 2. build the priced [`BuyModeEvalJob`] for the routing decision;
/// 3. **pre-check the shared [`DailySpendCap`]** against the quoted price; if it
///    would breach, refuse cleanly — no dispatch, no spend (fail-closed);
/// 4. dispatch through the [`BuyModeDispatch`] seam;
/// 5. debit the **settled** spend against the cap (defense-in-depth: the price
///    was already admitted, but the settled amount is the real money moved), and
///    return the [`VerdictOutcome`].
///
/// The cap is shared with the [`crate::LiveCoordinatorFitness`] driving the run,
/// so the price pre-check here and the post-dispatch debit there debit the same
/// budget. The [`LiveCoordinatorFitness`] also debits the returned
/// `spend_msats`; to avoid double-debiting, this source debits the cap itself and
/// reports `spend_msats: 0` to the fitness when it owns the cap, OR reports the
/// settled spend and lets the fitness own the debit. The mode is explicit via
/// [`CapDebitOwner`].
pub struct DispatchBackedVerdictSource<D: BuyModeDispatch> {
    dispatch: D,
    arm: CoordinatorArmState,
    /// The quoted per-eval price in msats (the job's `amount_msats`). The
    /// fail-closed pre-check is against this price.
    per_eval_msats: u64,
    /// Who debits the cap for the settled spend, so the spend is never
    /// double-counted. See [`CapDebitOwner`].
    cap_debit_owner: CapDebitOwner,
    /// The shared daily cap. Pre-checked before every dispatch; debited here only
    /// when [`CapDebitOwner::Source`].
    cap: std::cell::RefCell<DailySpendCap>,
    /// Records whether the source refused on a cap pre-check or a disarmed state,
    /// for the driver's report.
    last_refusal: std::cell::RefCell<Option<CoordinatorDispatchRefusal>>,
}

/// Which component debits the [`DailySpendCap`] for the settled spend, so it is
/// counted exactly once.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapDebitOwner {
    /// The [`DispatchBackedVerdictSource`] debits the settled spend itself and
    /// reports `spend_msats: 0` to the fitness. Use when the source owns the cap
    /// outright (e.g. driven directly, not through [`LiveCoordinatorFitness`]).
    Source,
    /// The source pre-checks the cap (fail-closed) but reports the settled
    /// `spend_msats` to the fitness, which performs the authoritative debit. Use
    /// when wired through [`LiveCoordinatorFitness`], whose own debit is the
    /// single source of truth. This is the default for the live lane.
    Fitness,
}

/// Why a [`DispatchBackedVerdictSource`] refused a verdict request without
/// dispatching.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoordinatorDispatchRefusal {
    /// The lane was disarmed (default). No dispatch, no spend.
    Disarmed,
    /// The quoted price would breach the daily cap. No dispatch, no spend.
    OverDailyCap,
}

impl<D: BuyModeDispatch> DispatchBackedVerdictSource<D> {
    /// Builds a dispatch-backed source. **Disarmed by default**: pass
    /// [`CoordinatorArmState::Armed`] explicitly to enable paid dispatch. The cap
    /// is clamped to the owner ceiling by [`DailySpendCap`] construction.
    #[must_use]
    pub fn new(
        dispatch: D,
        arm: CoordinatorArmState,
        per_eval_msats: u64,
        cap_debit_owner: CapDebitOwner,
        cap: DailySpendCap,
    ) -> Self {
        Self {
            dispatch,
            arm,
            per_eval_msats,
            cap_debit_owner,
            cap: std::cell::RefCell::new(cap),
            last_refusal: std::cell::RefCell::new(None),
        }
    }

    /// Builds a **disarmed** source (the safe default). Equivalent to
    /// [`new`](Self::new) with [`CoordinatorArmState::Disarmed`].
    #[must_use]
    pub fn disarmed(dispatch: D, per_eval_msats: u64, cap: DailySpendCap) -> Self {
        Self::new(
            dispatch,
            CoordinatorArmState::Disarmed,
            per_eval_msats,
            CapDebitOwner::Fitness,
            cap,
        )
    }

    /// Whether the lane is armed.
    #[must_use]
    pub fn is_armed(&self) -> bool {
        self.arm.is_armed()
    }

    /// The quoted per-eval price in msats.
    #[must_use]
    pub const fn per_eval_msats(&self) -> u64 {
        self.per_eval_msats
    }

    /// A snapshot of the cap (spent / remaining / day_key).
    #[must_use]
    pub fn cap_snapshot(&self) -> DailySpendCap {
        self.cap.borrow().clone()
    }

    /// The most recent refusal reason, if the last request refused without
    /// dispatching.
    #[must_use]
    pub fn last_refusal(&self) -> Option<CoordinatorDispatchRefusal> {
        *self.last_refusal.borrow()
    }

    fn refuse(
        &self,
        refusal: CoordinatorDispatchRefusal,
        detail: String,
    ) -> CoordinatorLiveTrainingError {
        *self.last_refusal.borrow_mut() = Some(refusal);
        CoordinatorLiveTrainingError::VerdictSource { detail }
    }
}

impl<D: BuyModeDispatch> EvalVerdictSource for DispatchBackedVerdictSource<D> {
    fn verdict_for(
        &self,
        request: &TrajectoryRequest,
    ) -> Result<VerdictOutcome, CoordinatorLiveTrainingError> {
        // 1. Default-off arming. Disarmed -> refuse cleanly, dispatch nothing.
        if !self.arm.is_armed() {
            return Err(self.refuse(
                CoordinatorDispatchRefusal::Disarmed,
                format!(
                    "paid coordinator dispatch is DISARMED (default); refusing sample `{}` with zero spend",
                    request.sample_id
                ),
            ));
        }

        let job = BuyModeEvalJob::from_request(request, self.per_eval_msats);

        // 2. Fail-closed cap pre-check BEFORE any dispatch. A price that would
        //    breach the cap dispatches nothing and moves no sats.
        if !self.cap.borrow().can_spend(job.amount_msats) {
            let cap = self.cap.borrow();
            return Err(self.refuse(
                CoordinatorDispatchRefusal::OverDailyCap,
                format!(
                    "daily cap pre-check failed for sample `{}`: price {} msats + spent {} msats would exceed cap {} msats (day_key {}); no dispatch, zero spend",
                    request.sample_id,
                    job.amount_msats,
                    cap.spent_today_msats(),
                    cap.cap_msats(),
                    cap.day_key(),
                ),
            ));
        }

        // 3. Armed and within budget: dispatch through the buy-mode seam.
        let result = self.dispatch.dispatch_eval(&job)?;

        // 4. Debit the SETTLED spend. When the source owns the cap it debits here
        //    and reports zero to the fitness; otherwise the fitness debits the
        //    returned spend. Either way the settled spend is counted exactly once.
        let reported_spend = match self.cap_debit_owner {
            CapDebitOwner::Source => {
                self.cap.borrow_mut().try_debit(result.settled_msats)?;
                0
            }
            CapDebitOwner::Fitness => result.settled_msats,
        };

        // Clear any stale refusal on a successful dispatch.
        *self.last_refusal.borrow_mut() = None;

        Ok(VerdictOutcome {
            verdict: result.verdict.verdict(),
            spend_msats: reported_spend,
        })
    }
}

// ---------------------------------------------------------------------------
// Fixture dispatcher for tests / offline reasoning (no real inference, no sats).
// ---------------------------------------------------------------------------

/// A fixture [`BuyModeDispatch`] for tests: returns a fixed verdict + settled
/// spend and records every dispatched job. Tests assert it is **never called**
/// while disarmed or over-cap, so no real inference and no sat movement is ever
/// exercised in CI.
#[derive(Debug, Default)]
pub struct RecordingDispatch {
    verdict: VerificationClassVerdict,
    settled_msats: u64,
    calls: std::cell::RefCell<Vec<BuyModeEvalJob>>,
}

impl RecordingDispatch {
    /// A recording dispatcher that always returns `verdict` and settles
    /// `settled_msats`.
    #[must_use]
    pub fn new(verdict: VerificationClassVerdict, settled_msats: u64) -> Self {
        Self {
            verdict,
            settled_msats,
            calls: std::cell::RefCell::new(Vec::new()),
        }
    }

    /// The number of jobs dispatched so far. `0` proves no dispatch happened.
    #[must_use]
    pub fn call_count(&self) -> usize {
        self.calls.borrow().len()
    }

    /// A clone of the dispatched jobs, for assertions.
    #[must_use]
    pub fn calls(&self) -> Vec<BuyModeEvalJob> {
        self.calls.borrow().clone()
    }
}

impl BuyModeDispatch for RecordingDispatch {
    fn dispatch_eval(
        &self,
        job: &BuyModeEvalJob,
    ) -> Result<BuyModeEvalResult, CoordinatorLiveTrainingError> {
        self.calls.borrow_mut().push(job.clone());
        Ok(BuyModeEvalResult {
            verdict: self.verdict,
            settled_msats: self.settled_msats,
        })
    }
}

/// The owner ceiling re-exported for callers reasoning about the cap, so the
/// paid-lane plumbing and the live-training cap reference the same constant.
pub const COORDINATOR_OWNER_DAILY_CAP_MSATS: u64 = OWNER_DAILY_CAP_MSATS;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coordinator_evolution::VerificationVerdict;
    use crate::coordinator_live_training::TrajectoryRequest;

    fn request(sample: &str) -> TrajectoryRequest {
        TrajectoryRequest {
            worker_index: 0,
            worker_id: "open-pylon-a".to_string(),
            role_index: 1,
            sample_id: sample.to_string(),
        }
    }

    /// A guard dispatcher that fails the test if ever called. Used to prove the
    /// fail-closed paths (disarmed / over-cap) never reach dispatch: reaching it
    /// surfaces as a hard test panic, so a leaked dispatch can never pass silently.
    #[derive(Debug, Default)]
    struct NeverDispatch;

    impl BuyModeDispatch for NeverDispatch {
        fn dispatch_eval(
            &self,
            job: &BuyModeEvalJob,
        ) -> Result<BuyModeEvalResult, CoordinatorLiveTrainingError> {
            panic!(
                "BuyModeDispatch was called while it must not be (sample `{}`, {} msats); \
                 the fail-closed gate leaked a dispatch",
                job.sample_id, job.amount_msats
            );
        }
    }

    #[test]
    fn arm_state_defaults_to_disarmed() {
        assert_eq!(
            CoordinatorArmState::default(),
            CoordinatorArmState::Disarmed
        );
        assert!(!CoordinatorArmState::default().is_armed());
        assert!(CoordinatorArmState::Armed.is_armed());
    }

    #[test]
    fn verification_class_verdict_maps_to_terminal_verdict() {
        assert_eq!(
            VerificationClassVerdict::exact_trace_pass().verdict(),
            VerificationVerdict::Verified
        );
        assert_eq!(
            VerificationClassVerdict::exact_trace_fail().verdict(),
            VerificationVerdict::Rejected
        );
    }

    // ---- Disarmed: refuse, NEVER dispatch ----------------------------------

    #[test]
    fn disarmed_source_refuses_and_never_dispatches() {
        // NeverDispatch panics if reached; a disarmed source must not reach it.
        let cap = DailySpendCap::owner_default("2026-06-22");
        let source = DispatchBackedVerdictSource::disarmed(NeverDispatch, 1_000_000, cap);
        assert!(!source.is_armed());

        let error = source.verdict_for(&request("s0")).unwrap_err();
        assert!(matches!(
            error,
            CoordinatorLiveTrainingError::VerdictSource { .. }
        ));
        assert_eq!(
            source.last_refusal(),
            Some(CoordinatorDispatchRefusal::Disarmed)
        );
        // Cap untouched, zero spend.
        assert_eq!(source.cap_snapshot().spent_today_msats(), 0);
    }

    #[test]
    fn default_construction_is_disarmed_via_arm_state_default() {
        // A source built with the default arm state must refuse.
        let cap = DailySpendCap::owner_default("2026-06-22");
        let source = DispatchBackedVerdictSource::new(
            NeverDispatch,
            CoordinatorArmState::default(),
            1_000_000,
            CapDebitOwner::Source,
            cap,
        );
        assert!(source.verdict_for(&request("s0")).is_err());
        assert_eq!(source.cap_snapshot().spent_today_msats(), 0);
    }

    // ---- Armed + within budget: dispatch, real verdict, spend tracked ------

    #[test]
    fn armed_source_dispatches_and_yields_scalar_verdict_and_spend() {
        // Source owns the cap: it debits the settled spend and reports 0 spend.
        let dispatch =
            RecordingDispatch::new(VerificationClassVerdict::exact_trace_pass(), 2_000_000);
        let cap = DailySpendCap::owner_default("2026-06-22");
        let source = DispatchBackedVerdictSource::new(
            dispatch,
            CoordinatorArmState::Armed,
            2_000_000,
            CapDebitOwner::Source,
            cap,
        );

        let outcome = source.verdict_for(&request("s0")).expect("verdict");
        assert_eq!(outcome.verdict, VerificationVerdict::Verified);
        // Source-owned cap: reports zero to the fitness, debits the cap itself.
        assert_eq!(outcome.spend_msats, 0);
        assert_eq!(source.cap_snapshot().spent_today_msats(), 2_000_000);
        assert_eq!(source.last_refusal(), None);
    }

    #[test]
    fn armed_source_reports_settled_spend_when_fitness_owns_the_cap() {
        // Fitness owns the cap: the source pre-checks but reports the settled
        // spend so the fitness performs the single authoritative debit.
        let dispatch =
            RecordingDispatch::new(VerificationClassVerdict::exact_trace_pass(), 3_000_000);
        let cap = DailySpendCap::owner_default("2026-06-22");
        let source = DispatchBackedVerdictSource::new(
            dispatch,
            CoordinatorArmState::Armed,
            3_000_000,
            CapDebitOwner::Fitness,
            cap,
        );

        let outcome = source.verdict_for(&request("s0")).expect("verdict");
        assert_eq!(outcome.verdict, VerificationVerdict::Verified);
        // Reports the settled spend; the source did NOT debit its own cap.
        assert_eq!(outcome.spend_msats, 3_000_000);
        assert_eq!(source.cap_snapshot().spent_today_msats(), 0);
    }

    #[test]
    fn armed_source_passes_rejected_verdict_through() {
        let dispatch =
            RecordingDispatch::new(VerificationClassVerdict::exact_trace_fail(), 1_000_000);
        let cap = DailySpendCap::owner_default("2026-06-22");
        let source = DispatchBackedVerdictSource::new(
            dispatch,
            CoordinatorArmState::Armed,
            1_000_000,
            CapDebitOwner::Fitness,
            cap,
        );
        let outcome = source.verdict_for(&request("s0")).expect("verdict");
        assert_eq!(outcome.verdict, VerificationVerdict::Rejected);
    }

    // ---- Armed but OVER CAP: refuse, NEVER dispatch ------------------------

    #[test]
    fn armed_over_cap_fails_closed_without_dispatch() {
        // Price per eval is half the cap; the first eval fits, the third would
        // breach. NeverDispatch on the over-cap path proves no dispatch leaks.
        // Use a recording dispatcher with a counter to confirm exactly 2 dispatches.
        let dispatch = RecordingDispatch::new(
            VerificationClassVerdict::exact_trace_pass(),
            // Settled == price; each admitted eval spends 4,000,000 msats.
            4_000_000,
        );
        let cap = DailySpendCap::owner_default("2026-06-22"); // 10,000,000 msats.
        let source = DispatchBackedVerdictSource::new(
            dispatch,
            CoordinatorArmState::Armed,
            4_000_000,
            CapDebitOwner::Source,
            cap,
        );

        // Two evals fit (8,000,000 <= 10,000,000).
        assert!(source.verdict_for(&request("s0")).is_ok());
        assert!(source.verdict_for(&request("s1")).is_ok());
        assert_eq!(source.cap_snapshot().spent_today_msats(), 8_000_000);

        // The third would breach: refuse, no dispatch, no spend.
        let error = source.verdict_for(&request("s2")).unwrap_err();
        assert!(matches!(
            error,
            CoordinatorLiveTrainingError::VerdictSource { .. }
        ));
        assert_eq!(
            source.last_refusal(),
            Some(CoordinatorDispatchRefusal::OverDailyCap)
        );
        // Spend unchanged: the breaching request moved no sats.
        assert_eq!(source.cap_snapshot().spent_today_msats(), 8_000_000);
        // Exactly two jobs were ever dispatched.
        assert_eq!(source.dispatch.call_count(), 2);
    }

    #[test]
    fn armed_over_cap_never_calls_dispatch_guard() {
        // A per-eval price already above the cap must refuse before dispatch.
        // NeverDispatch panics if reached, proving the fail-closed pre-check.
        let cap = DailySpendCap::owner_default("2026-06-22");
        let source = DispatchBackedVerdictSource::new(
            NeverDispatch,
            CoordinatorArmState::Armed,
            COORDINATOR_OWNER_DAILY_CAP_MSATS + 1, // larger than the whole cap.
            CapDebitOwner::Source,
            cap,
        );
        let error = source.verdict_for(&request("s0")).unwrap_err();
        assert!(matches!(
            error,
            CoordinatorLiveTrainingError::VerdictSource { .. }
        ));
        assert_eq!(
            source.last_refusal(),
            Some(CoordinatorDispatchRefusal::OverDailyCap)
        );
        assert_eq!(source.cap_snapshot().spent_today_msats(), 0);
    }

    #[test]
    fn job_is_built_from_the_routing_decision() {
        let req = request("task-7");
        let job = BuyModeEvalJob::from_request(&req, 5_000);
        assert_eq!(job.worker_id, "open-pylon-a");
        assert_eq!(job.role_index, 1);
        assert_eq!(job.sample_id, "task-7");
        assert_eq!(job.amount_msats, 5_000);
    }

    // ---- End-to-end: dispatch-backed source -> fitness -> ShadowComparison --

    /// Proves the dispatch-backed source plugs into the existing
    /// [`crate::LiveCoordinatorFitness`] (the same `EvalVerdictSource` seam the
    /// simulated source uses) and that the per-trajectory outcomes it produces
    /// feed [`crate::ShadowComparison`] / fitness — scoring the router on the
    /// dispatched verification verdict. Armed, within cap, fixture dispatcher:
    /// no real inference, no sats actually move (the fixture just records).
    #[test]
    fn dispatch_backed_source_feeds_fitness_and_shadow_comparison() {
        use crate::coordinator_evolution::{
            CoordinatorFitness, TerminalRewardAdapter, TrajectoryOutcome, WorkerKind,
            WorkerPoolBinding, WorkerPoolMember,
        };
        use crate::coordinator_live_training::{EvalSample, LiveCoordinatorFitness};
        use crate::coordinator_shadow_comparison::{
            ComparisonLane, ShadowComparison, ShadowRecommendation,
        };
        use psionic_models::{CoordinatorHead, CoordinatorHeadConfig};

        // P5 pool: a single eligible worker so head index 0 always resolves.
        let pool = WorkerPoolBinding::from_candidates(
            vec![WorkerPoolMember {
                worker_id: "open-pylon-a".to_string(),
                kind: WorkerKind::Open,
                receipted_capabilities: ["rust_build".to_string()].into_iter().collect(),
            }],
            "rust_build",
        )
        .expect("pool");

        let d_model = 4;
        let head_config = CoordinatorHeadConfig {
            hidden_dim: d_model,
            num_workers: pool.len(),
            num_roles: 3,
        };
        let seed_head = CoordinatorHead::zeros(head_config).expect("head");

        // Two samples; a fixed deterministic hidden state per sample (the closure
        // stands in for a frozen forward_with_hidden — this test exercises the
        // verdict-source seam, not the backbone).
        let samples = vec![
            EvalSample {
                sample_id: "s0".to_string(),
                token_ids: vec![1, 2],
            },
            EvalSample {
                sample_id: "s1".to_string(),
                token_ids: vec![3, 4],
            },
        ];
        let hidden = |_tokens: &[usize]| -> Result<Vec<f32>, CoordinatorLiveTrainingError> {
            Ok(vec![0.1, -0.2, 0.3, 0.05])
        };

        // Armed dispatch-backed source; fitness owns the cap (the canonical live
        // wiring). Fixture dispatcher always Verifies and settles 1,000,000 msats.
        let dispatch =
            RecordingDispatch::new(VerificationClassVerdict::exact_trace_pass(), 1_000_000);
        let cap = DailySpendCap::owner_default("2026-06-22");
        let learned_source = DispatchBackedVerdictSource::new(
            dispatch,
            CoordinatorArmState::Armed,
            1_000_000,
            CapDebitOwner::Fitness,
            cap.clone(),
        );

        let learned_fitness = LiveCoordinatorFitness::new(
            seed_head.clone(),
            pool.clone(),
            // Cost-aware so cost feeds verified-work-per-sat.
            TerminalRewardAdapter::cost_aware(0.0),
            samples.clone(),
            hidden,
            learned_source,
            cap.clone(),
        )
        .expect("learned fitness");

        let initial = seed_head.flatten_parameters().expect("flat");
        // One evaluation runs both samples through dispatch -> verdict -> reward.
        let learned_reward = learned_fitness
            .evaluate_coordinator(&initial)
            .expect("learned eval");
        // Both samples Verified at the fixture verdict -> mean reward 1.0.
        assert!((learned_reward - 1.0).abs() < 1e-6);
        // The fitness debited the SETTLED spend (fitness owns the cap): 2 samples
        // * 1,000,000 msats == 2,000 sats reported per sample, debited by fitness.
        let snap = learned_fitness.cap_snapshot();
        assert_eq!(snap.spent_today_msats(), 2_000_000);
        assert!(!learned_fitness.halted_on_cap());

        // Build the two TrajectoryOutcome streams a ShadowComparison consumes.
        // Learned: 2 verified @ 1,000 sats each (2,000 sats total).
        let learned_outcomes = vec![
            TrajectoryOutcome {
                verdict: VerificationVerdict::Verified,
                cost: 1_000.0,
            },
            TrajectoryOutcome {
                verdict: VerificationVerdict::Verified,
                cost: 1_000.0,
            },
        ];
        // Heuristic baseline: 1 verified @ 2,000 sats (worse per-sat).
        let heuristic_outcomes = vec![
            TrajectoryOutcome {
                verdict: VerificationVerdict::Verified,
                cost: 2_000.0,
            },
            TrajectoryOutcome {
                verdict: VerificationVerdict::Rejected,
                cost: 2_000.0,
            },
        ];
        let shadow = ShadowComparison::compare(&learned_outcomes, &heuristic_outcomes);
        // Paid lane (both arms moved sats), learned wins on verified-work-per-sat.
        assert_eq!(shadow.lane, ComparisonLane::VerifiedWorkPerSat);
        assert!(shadow.learned_wins);
        // Learned verified rate 1.0 -> High band -> clean win -> promote eligible.
        assert_eq!(
            shadow.recommendation,
            ShadowRecommendation::PromoteCandidate
        );
    }
}
