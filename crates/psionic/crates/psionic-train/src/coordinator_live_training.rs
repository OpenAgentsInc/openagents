//! Live coordinator-evolution training run (Khala M6, issue #6014 / EPIC #6017).
//!
//! This module wires the **live training run** on top of the merged P1–P5
//! substrate ([`crate::coordinator_evolution`] + the P1/P2 model primitives):
//!
//! - It drives [`forward_with_hidden`] on a *frozen* small backbone (the cs336
//!   reference stack, or a Qwen3-0.6B on the production lane) to produce the
//!   penultimate-token hidden state `h` the coordinator head reads — replacing
//!   the fixed fixture probe tensor used by the P3–P5 smoke.
//! - Each candidate's atomic evaluation runs a real `select -> role -> dispatch
//!   -> verify` trajectory over the P5 [`WorkerPoolBinding`], and the reward is
//!   a *verification verdict* ([`VerificationVerdict`]) — the Tassadar
//!   `training.verification_classes.v1` verdict on the live lane, a deterministic
//!   simulated verdict on the no-spend validation lane.
//! - Every eval that moves sats (Pylon-served worker evals) is **debited against
//!   a hard daily spend cap** ([`DailySpendCap`]). The cap mirrors the existing
//!   autonomous buy-mode ceiling in the `openagents` repo
//!   (`apps/openagents.com/workers/api/src/buy-mode-dispatcher.ts`:
//!   `spent_today_msats + amount_msats > daily_cap_msats` -> halt, `day_key`-scoped),
//!   clamped to the owner-set **10,000 sats/day** budget. It **fails closed**:
//!   when the next eval would breach the cap the run halts before spending.
//!
//! ## Cap integration point (cross-repo, exact)
//!
//! The owner budget is one shared autonomous daily ceiling. The authoritative
//! accounting lives in the `openagents` Worker, NOT here:
//!
//! - Table/columns (minified at runtime, semantics stable):
//!   `daily_cap_msats`, `spent_today_msats`, `day_key`, `per_job_cap_msats`,
//!   `spend_enabled` on the buy-mode campaign row.
//! - Enforcement: `buy-mode-dispatcher.ts` `dispatchJob` /
//!   `recordSettlement` reject when
//!   `campaign.spentTodayMsats + amountMsats > campaign.dailyCapMsats` and emit
//!   `alert.buy_mode.daily_cap_breach`, then halt the campaign.
//! - Roll-over: `day_key` (UTC date) scopes `spent_today_msats`; a new day
//!   resets the spent counter.
//!
//! On a first cross-repo run the training loop CANNOT reach that D1 row
//! synchronously, so [`DailySpendCap`] enforces a hard **local** 10,000 sat/day
//! counter with the same `spent_today` / `day_key` / `daily_cap` semantics, and
//! every Pylon eval job MUST also carry the campaign's `per_job_cap_msats` /
//! `daily_cap_msats` so the Worker is the final authority. The production wiring
//! is: each [`LiveCoordinatorFitness`] eval dispatched to Pylon is one buy-mode
//! `dispatchJob` call against the SAME campaign row, so the local cap and the
//! shared cap debit the same budget. The local cap is the fail-closed backstop,
//! never a parallel budget.

use psionic_models::{CoordinatorHead, CoordinatorHeadError};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::coordinator_evolution::{
    CoordinatorEvolutionError, CoordinatorFitness, TerminalRewardAdapter, TrajectoryOutcome,
    VerificationVerdict, WorkerPoolBinding,
};

/// The owner-set HARD budget cap: 10,000 sats/day == 10,000,000 msats/day.
/// This is a policy constant, not a tunable — the run must never exceed it.
pub const OWNER_DAILY_CAP_MSATS: u64 = 10_000_000;

/// Sats-to-msats conversion (1 sat = 1000 msats).
pub const MSATS_PER_SAT: u64 = 1_000;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors raised by the live coordinator-training lane.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum CoordinatorLiveTrainingError {
    /// The daily spend cap would be breached by the next eval. The run fails
    /// closed: it halts rather than spending past the cap.
    #[error(
        "daily spend cap breach: spent {spent_msats} msats + {requested_msats} msats would exceed cap {cap_msats} msats (day_key {day_key})"
    )]
    DailyCapBreach {
        /// Sats already spent today (in msats).
        spent_msats: u64,
        /// The eval spend that would breach the cap (in msats).
        requested_msats: u64,
        /// The hard daily ceiling (in msats).
        cap_msats: u64,
        /// The UTC day key the spend is scoped to.
        day_key: String,
    },
    /// A configured per-eval cost exceeds the whole daily cap — config error.
    #[error("per-eval cost {per_eval_msats} msats exceeds the daily cap {cap_msats} msats")]
    PerEvalExceedsDailyCap {
        /// The per-eval spend (in msats).
        per_eval_msats: u64,
        /// The daily ceiling (in msats).
        cap_msats: u64,
    },
    /// A coordinator-head error surfaced from the head materialization.
    #[error("coordinator head error: {0}")]
    Head(#[from] CoordinatorHeadError),
    /// An underlying evolution-lane error.
    #[error("coordinator evolution error: {0}")]
    Evolution(#[from] CoordinatorEvolutionError),
    /// The verdict source rejected the trajectory request.
    #[error("verdict source error: {detail}")]
    VerdictSource {
        /// Human-readable detail.
        detail: String,
    },
}

// ---------------------------------------------------------------------------
// Hard daily spend cap (mirrors openagents buy-mode `spent_today_msats`).
// ---------------------------------------------------------------------------

/// A hard, fail-closed daily spend cap, scoped by `day_key` (UTC date), in
/// msats. This is the local backstop for the shared autonomous buy-mode
/// ceiling described in the module docs; it has identical
/// `spent_today / day_key / daily_cap` semantics.
///
/// Interior mutability is deliberately NOT used: callers debit explicitly via
/// [`DailySpendCap::try_debit`] before each eval, so the cap is checked
/// *before* any sats move (fail-closed). The cap clamps its ceiling to
/// [`OWNER_DAILY_CAP_MSATS`] so it can never be configured above the owner
/// budget.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DailySpendCap {
    cap_msats: u64,
    spent_today_msats: u64,
    day_key: String,
}

impl DailySpendCap {
    /// Builds a cap for `day_key` with `requested_cap_msats`, clamped down to
    /// the owner's hard [`OWNER_DAILY_CAP_MSATS`] ceiling. A request above the
    /// owner cap is silently clamped (never raised) — the owner cap is a hard
    /// upper bound.
    #[must_use]
    pub fn for_day(day_key: impl Into<String>, requested_cap_msats: u64) -> Self {
        Self {
            cap_msats: requested_cap_msats.min(OWNER_DAILY_CAP_MSATS),
            spent_today_msats: 0,
            day_key: day_key.into(),
        }
    }

    /// A cap pinned exactly at the owner's 10,000 sat/day ceiling.
    #[must_use]
    pub fn owner_default(day_key: impl Into<String>) -> Self {
        Self::for_day(day_key, OWNER_DAILY_CAP_MSATS)
    }

    /// The hard daily ceiling in msats (already clamped to the owner cap).
    #[must_use]
    pub const fn cap_msats(&self) -> u64 {
        self.cap_msats
    }

    /// Msats spent so far today.
    #[must_use]
    pub const fn spent_today_msats(&self) -> u64 {
        self.spent_today_msats
    }

    /// Remaining budget today, in msats.
    #[must_use]
    pub const fn remaining_msats(&self) -> u64 {
        self.cap_msats.saturating_sub(self.spent_today_msats)
    }

    /// The UTC day key this cap is scoped to.
    #[must_use]
    pub fn day_key(&self) -> &str {
        &self.day_key
    }

    /// Whether `amount_msats` can be spent without breaching the cap. This is
    /// the exact predicate the openagents Worker uses
    /// (`spent_today_msats + amount_msats <= daily_cap_msats`).
    #[must_use]
    pub const fn can_spend(&self, amount_msats: u64) -> bool {
        // Use checked add so an overflow can never wrap below the cap.
        match self.spent_today_msats.checked_add(amount_msats) {
            Some(total) => total <= self.cap_msats,
            None => false,
        }
    }

    /// Attempts to debit `amount_msats`. Fails closed (no mutation) if the
    /// debit would breach the cap.
    pub fn try_debit(&mut self, amount_msats: u64) -> Result<(), CoordinatorLiveTrainingError> {
        if amount_msats > self.cap_msats {
            return Err(CoordinatorLiveTrainingError::PerEvalExceedsDailyCap {
                per_eval_msats: amount_msats,
                cap_msats: self.cap_msats,
            });
        }
        if !self.can_spend(amount_msats) {
            return Err(CoordinatorLiveTrainingError::DailyCapBreach {
                spent_msats: self.spent_today_msats,
                requested_msats: amount_msats,
                cap_msats: self.cap_msats,
                day_key: self.day_key.clone(),
            });
        }
        self.spent_today_msats += amount_msats;
        Ok(())
    }

    /// Rolls the cap over to a new `day_key`, resetting the spent counter (the
    /// Worker does this implicitly when `day_key` changes).
    pub fn roll_over(&mut self, new_day_key: impl Into<String>) {
        self.day_key = new_day_key.into();
        self.spent_today_msats = 0;
    }
}

// ---------------------------------------------------------------------------
// Verdict source: where the terminal verdict comes from.
// ---------------------------------------------------------------------------

/// One coordinated trajectory request: the worker the head selected and the
/// role it assigned, resolved against the P5 binding. The verdict source uses
/// this to decide / look up the terminal verdict and the spend it incurred.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrajectoryRequest {
    /// The eligible-pool index the head selected (already capability-filtered).
    pub worker_index: usize,
    /// Stable id of the selected worker.
    pub worker_id: String,
    /// The TRINITY role index the head assigned.
    pub role_index: usize,
    /// The task / sample id this trajectory is for (stable across candidates so
    /// fitness is comparable).
    pub sample_id: String,
}

/// One verdict outcome: the terminal verification verdict plus the spend the
/// trajectory incurred, in msats. The spend is debited against the
/// [`DailySpendCap`] before this is accepted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VerdictOutcome {
    /// The terminal verification verdict.
    pub verdict: VerificationVerdict,
    /// Spend the trajectory incurred, in msats (0 on the no-spend lane).
    pub spend_msats: u64,
}

/// The seam where the offline / simulated / live lanes plug in. A real run
/// binds this to the Pylon dispatch path (fan out the trajectory as an eval
/// job) and the Tassadar replay-validator verdict; the validation smoke binds
/// a deterministic, no-spend implementation.
pub trait EvalVerdictSource {
    /// Returns the terminal verdict and incurred spend for one trajectory.
    /// Implementations on the live lane MUST be the replay-validator /
    /// verification-command verdict, never a prompted LLM judge.
    fn verdict_for(
        &self,
        request: &TrajectoryRequest,
    ) -> Result<VerdictOutcome, CoordinatorLiveTrainingError>;
}

/// A deterministic, **no-spend** verdict source for the validation pass. It
/// declares one "correct" eligible-worker index per sample: a trajectory
/// Verifies iff the head routed that sample to its correct worker. Spend is
/// always zero, so this proves the loop + cap *enforcement* without moving any
/// sats.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimulatedVerdictSource {
    /// `(sample_id, correct_worker_index)` pairs.
    correct_worker_for_sample: Vec<(String, usize)>,
}

impl SimulatedVerdictSource {
    /// Builds a simulated source from `(sample_id, correct_worker_index)` pairs.
    #[must_use]
    pub fn new(correct_worker_for_sample: Vec<(String, usize)>) -> Self {
        Self {
            correct_worker_for_sample,
        }
    }
}

impl EvalVerdictSource for SimulatedVerdictSource {
    fn verdict_for(
        &self,
        request: &TrajectoryRequest,
    ) -> Result<VerdictOutcome, CoordinatorLiveTrainingError> {
        let correct = self
            .correct_worker_for_sample
            .iter()
            .find(|(sample_id, _)| sample_id == &request.sample_id)
            .map(|(_, index)| *index)
            .ok_or_else(|| CoordinatorLiveTrainingError::VerdictSource {
                detail: format!("no simulated verdict for sample `{}`", request.sample_id),
            })?;
        let verdict = if request.worker_index == correct {
            VerificationVerdict::Verified
        } else {
            VerificationVerdict::Rejected
        };
        // No-spend lane: zero msats. Proves cap enforcement without spending.
        Ok(VerdictOutcome {
            verdict,
            spend_msats: 0,
        })
    }
}

// ---------------------------------------------------------------------------
// Live coordinator fitness.
// ---------------------------------------------------------------------------

/// One evaluation sample: the prompt token ids fed to the frozen backbone and
/// a stable sample id. The backbone's `forward_with_hidden` turns the tokens
/// into the hidden state `h` the head routes on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvalSample {
    /// Stable sample id (used by the verdict source and for comparable fitness).
    pub sample_id: String,
    /// Prompt token ids fed to the frozen backbone.
    pub token_ids: Vec<usize>,
}

/// The live atomic-evaluation hook. Given a candidate head parameter vector it:
///
/// 1. materializes a real [`CoordinatorHead`] from `params`;
/// 2. for each [`EvalSample`], computes the backbone hidden state via the
///    injected `hidden_for_tokens` closure (a frozen `forward_with_hidden`),
///    runs `head.decide(h)`, resolves the worker through the P5 binding;
/// 3. asks the [`EvalVerdictSource`] for the verdict + spend, **debits the
///    spend against the [`DailySpendCap`] first (fail-closed)**;
/// 4. aggregates with a [`TerminalRewardAdapter`] (cost-aware on the live lane).
///
/// The backbone is injected as a closure so the validation smoke can use a real
/// small frozen LM and the production lane can swap in Qwen3-0.6B without
/// changing the loop. The closure MUST be deterministic (frozen weights) so ES
/// fitness is stable.
pub struct LiveCoordinatorFitness<H, S>
where
    H: Fn(&[usize]) -> Result<Vec<f32>, CoordinatorLiveTrainingError>,
    S: EvalVerdictSource,
{
    seed_head: CoordinatorHead,
    pool: WorkerPoolBinding,
    reward: TerminalRewardAdapter,
    samples: Vec<EvalSample>,
    hidden_for_tokens: H,
    verdicts: S,
    /// Shared cap. `RefCell` so the immutable `evaluate_coordinator` signature
    /// (required by the optimizer) can still debit; the cap is the single
    /// authority and is checked before every spend.
    cap: std::cell::RefCell<DailySpendCap>,
    /// Records whether the run halted on a cap breach so the driver can report.
    halted_on_cap: std::cell::Cell<bool>,
}

impl<H, S> LiveCoordinatorFitness<H, S>
where
    H: Fn(&[usize]) -> Result<Vec<f32>, CoordinatorLiveTrainingError>,
    S: EvalVerdictSource,
{
    /// Builds a live fitness. `seed_head`'s config must have `num_workers ==
    /// pool.len()` so worker logits index the eligible set exactly.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        seed_head: CoordinatorHead,
        pool: WorkerPoolBinding,
        reward: TerminalRewardAdapter,
        samples: Vec<EvalSample>,
        hidden_for_tokens: H,
        verdicts: S,
        cap: DailySpendCap,
    ) -> Result<Self, CoordinatorLiveTrainingError> {
        if seed_head.config().num_workers != pool.len() {
            return Err(CoordinatorLiveTrainingError::VerdictSource {
                detail: format!(
                    "head num_workers {} != eligible pool size {}",
                    seed_head.config().num_workers,
                    pool.len()
                ),
            });
        }
        Ok(Self {
            seed_head,
            pool,
            reward,
            samples,
            hidden_for_tokens,
            verdicts,
            cap: std::cell::RefCell::new(cap),
            halted_on_cap: std::cell::Cell::new(false),
        })
    }

    /// A snapshot of the cap after a run (spent / remaining / day_key).
    #[must_use]
    pub fn cap_snapshot(&self) -> DailySpendCap {
        self.cap.borrow().clone()
    }

    /// Whether the run halted because the cap was hit.
    #[must_use]
    pub fn halted_on_cap(&self) -> bool {
        self.halted_on_cap.get()
    }
}

impl<H, S> CoordinatorFitness for LiveCoordinatorFitness<H, S>
where
    H: Fn(&[usize]) -> Result<Vec<f32>, CoordinatorLiveTrainingError>,
    S: EvalVerdictSource,
{
    fn evaluate_coordinator(&self, params: &[f32]) -> Result<f32, CoordinatorEvolutionError> {
        use psionic_core::Shape;
        use psionic_nn::NnTensor;

        let head = self.seed_head.with_flat_parameters(params.to_vec())?;
        let hidden_dim = head.config().hidden_dim;
        let mut outcomes: Vec<TrajectoryOutcome> = Vec::with_capacity(self.samples.len());

        for sample in &self.samples {
            // 1. Frozen backbone hidden state for this sample.
            let hidden_values = (self.hidden_for_tokens)(&sample.token_ids).map_err(|error| {
                CoordinatorEvolutionError::InvalidConfiguration {
                    detail: format!("backbone hidden failed for `{}`: {error}", sample.sample_id),
                }
            })?;
            if hidden_values.len() != hidden_dim {
                return Err(CoordinatorEvolutionError::InvalidDimension {
                    detail: format!(
                        "backbone hidden width {} != head hidden_dim {}",
                        hidden_values.len(),
                        hidden_dim
                    ),
                });
            }
            let hidden = NnTensor::f32(Shape::new(vec![1, hidden_dim]), hidden_values)
                .map_err(|error| CoordinatorEvolutionError::InvalidConfiguration {
                    detail: error.to_string(),
                })?;

            // 2. Head decision -> worker via the P5 binding.
            let decisions = head
                .decide(&hidden)
                .map_err(CoordinatorEvolutionError::Head)?;
            let decision = &decisions[0];
            let worker = self.pool.resolve(decision.worker_index).ok_or_else(|| {
                CoordinatorEvolutionError::EmptyWorkerPool {
                    detail: format!(
                        "head selected worker index {} outside the {}-worker eligible pool",
                        decision.worker_index,
                        self.pool.len()
                    ),
                }
            })?;
            let request = TrajectoryRequest {
                worker_index: decision.worker_index,
                worker_id: worker.worker_id.clone(),
                role_index: decision.role_index,
                sample_id: sample.sample_id.clone(),
            };

            // 3. Verdict + spend. DEBIT FIRST (fail-closed): if the spend would
            //    breach the cap we stop the whole eval here and surface the
            //    breach so the driver halts the run.
            let outcome = self.verdicts.verdict_for(&request).map_err(|error| {
                CoordinatorEvolutionError::InvalidConfiguration {
                    detail: format!("verdict source: {error}"),
                }
            })?;
            if outcome.spend_msats > 0 {
                let debit = self.cap.borrow_mut().try_debit(outcome.spend_msats);
                if let Err(error) = debit {
                    // Fail closed: never spend past the cap. Mark the run halted
                    // and surface the breach as a fitness error so the optimizer
                    // and driver stop immediately.
                    self.halted_on_cap.set(true);
                    return Err(CoordinatorEvolutionError::InvalidConfiguration {
                        detail: format!("HALT_DAILY_CAP: {error}"),
                    });
                }
            }

            outcomes.push(TrajectoryOutcome {
                verdict: outcome.verdict,
                // Cost in the reward adapter's units (sats); convert from msats.
                cost: outcome.spend_msats as f32 / MSATS_PER_SAT as f32,
            });
        }

        Ok(self.reward.mean_scalar(&outcomes))
    }
}

// ---------------------------------------------------------------------------
// Run report.
// ---------------------------------------------------------------------------

/// A structured report for a bounded live (or simulated) coordinator run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LiveRunReport {
    /// Whether this was the no-spend simulated validation pass or a real run.
    pub lane: LiveRunLane,
    /// Fitness of the seed (zero/initial) head before optimization.
    pub initial_fitness: f32,
    /// Best fitness the optimizer reached.
    pub best_fitness: f32,
    /// Whether the optimizer improved over the start.
    pub improved: bool,
    /// Total fitness evaluations consumed (the eval budget actually spent).
    pub evaluations: usize,
    /// Msats spent (0 on the simulated lane).
    pub spent_msats: u64,
    /// The hard daily cap in msats.
    pub cap_msats: u64,
    /// Whether the run halted because the cap was hit (fail-closed).
    pub halted_on_cap: bool,
    /// The day key the spend was scoped to.
    pub day_key: String,
}

impl LiveRunReport {
    /// Whether the run stayed within the owner cap (always true unless a bug
    /// let a debit through — defense-in-depth assertion for the driver).
    #[must_use]
    pub const fn within_cap(&self) -> bool {
        self.spent_msats <= self.cap_msats && self.cap_msats <= OWNER_DAILY_CAP_MSATS
    }
}

/// Which lane a [`LiveRunReport`] describes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LiveRunLane {
    /// No-spend simulated validation (proves the loop + cap enforcement).
    SimulatedNoSpend,
    /// Real bounded run (Pylon-served evals, sats moved, cap-debited).
    BoundedReal,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coordinator_evolution::{SepCmaEs, SepCmaEsConfig};
    use psionic_core::{Shape, TensorData};
    use psionic_models::{
        CoordinatorHead, CoordinatorHeadConfig, Cs336A1ReferenceConfig, Cs336A1TransformerLm,
    };
    use psionic_nn::ModuleStateLoadMode;

    // ---- DailySpendCap: fail-closed ----------------------------------------

    #[test]
    fn cap_clamps_to_owner_ceiling() {
        let cap = DailySpendCap::for_day("2026-06-22", OWNER_DAILY_CAP_MSATS * 100);
        assert_eq!(cap.cap_msats(), OWNER_DAILY_CAP_MSATS);
    }

    #[test]
    fn cap_debits_until_the_owner_ceiling_then_fails_closed() {
        let mut cap = DailySpendCap::owner_default("2026-06-22");
        // 10,000 sats == 10,000,000 msats. Spend in 1000-sat (1,000,000 msat)
        // chunks; the 11th must fail closed.
        for _ in 0..10 {
            cap.try_debit(1_000_000).expect("within cap");
        }
        assert_eq!(cap.spent_today_msats(), OWNER_DAILY_CAP_MSATS);
        assert_eq!(cap.remaining_msats(), 0);
        let breach = cap.try_debit(1).unwrap_err();
        assert!(matches!(
            breach,
            CoordinatorLiveTrainingError::DailyCapBreach { .. }
        ));
        // The failed debit did NOT mutate the counter (fail-closed).
        assert_eq!(cap.spent_today_msats(), OWNER_DAILY_CAP_MSATS);
    }

    #[test]
    fn cap_rejects_per_eval_larger_than_the_whole_cap() {
        let mut cap = DailySpendCap::owner_default("2026-06-22");
        let error = cap.try_debit(OWNER_DAILY_CAP_MSATS + 1).unwrap_err();
        assert!(matches!(
            error,
            CoordinatorLiveTrainingError::PerEvalExceedsDailyCap { .. }
        ));
    }

    #[test]
    fn cap_predicate_matches_worker_semantics() {
        // spent + amount <= cap  (the exact buy-mode-dispatcher predicate).
        let cap = DailySpendCap::owner_default("2026-06-22");
        assert!(cap.can_spend(OWNER_DAILY_CAP_MSATS));
        assert!(!cap.can_spend(OWNER_DAILY_CAP_MSATS + 1));
    }

    #[test]
    fn cap_rolls_over_to_a_new_day() {
        let mut cap = DailySpendCap::owner_default("2026-06-22");
        cap.try_debit(5_000_000).expect("spend");
        cap.roll_over("2026-06-23");
        assert_eq!(cap.day_key(), "2026-06-23");
        assert_eq!(cap.spent_today_msats(), 0);
    }

    // ---- SimulatedVerdictSource --------------------------------------------

    #[test]
    fn simulated_source_verifies_only_the_correct_worker() {
        let source = SimulatedVerdictSource::new(vec![("s0".to_string(), 1)]);
        let correct = source
            .verdict_for(&TrajectoryRequest {
                worker_index: 1,
                worker_id: "w1".to_string(),
                role_index: 0,
                sample_id: "s0".to_string(),
            })
            .expect("verdict");
        assert_eq!(correct.verdict, VerificationVerdict::Verified);
        assert_eq!(correct.spend_msats, 0);

        let wrong = source
            .verdict_for(&TrajectoryRequest {
                worker_index: 0,
                worker_id: "w0".to_string(),
                role_index: 0,
                sample_id: "s0".to_string(),
            })
            .expect("verdict");
        assert_eq!(wrong.verdict, VerificationVerdict::Rejected);
    }

    // ---- Live fitness over a REAL frozen backbone (no-spend) ---------------

    /// Builds a tiny deterministic frozen cs336 backbone (hidden_dim == d_model).
    /// Weights are fixed (a simple ramp), so `forward_with_hidden` is
    /// deterministic — required for stable ES fitness.
    fn frozen_backbone(d_model: usize, vocab: usize) -> Cs336A1TransformerLm {
        let config = Cs336A1ReferenceConfig {
            vocab_size: vocab,
            context_length: 8,
            d_model,
            num_layers: 1,
            num_heads: 1,
            d_ff: d_model * 2,
        };
        let mut model =
            Cs336A1TransformerLm::new("frozen_backbone", config, 10_000.0, 1e-5).expect("model");
        let mut weights = model.state_dict();
        // Deterministic, well-conditioned fixed weights: a small ramp keeps the
        // hidden state finite and input-dependent without any randomness. RMSNorm
        // scales stay at 1.0 so they do not zero the hidden state.
        for (index, (path, entry)) in weights.entries.iter_mut().enumerate() {
            let len = entry.spec.shape().element_count();
            let values: Vec<f32> = if path.contains("ln") || path.ends_with("norm.weight") {
                vec![1.0; len]
            } else {
                let base = ((index % 7) as f32) * 0.01 + 0.01;
                (0..len).map(|i| base + (i % 5) as f32 * 0.003).collect()
            };
            entry.data = TensorData::F32(values);
        }
        model
            .load_state_dict(&weights, ModuleStateLoadMode::Strict)
            .expect("load frozen weights");
        model
    }

    #[test]
    fn live_fitness_drives_a_real_backbone_and_optimizes_no_spend() {
        let d_model = 8;
        let vocab = 16;
        let backbone = frozen_backbone(d_model, vocab);

        // P5 pool: 3 eligible workers (the head's worker logits index these).
        use crate::coordinator_evolution::{WorkerKind, WorkerPoolMember};
        let candidates = vec![
            WorkerPoolMember {
                worker_id: "alpha".to_string(),
                kind: WorkerKind::Frontier,
                receipted_capabilities: ["rust_build".to_string()].into_iter().collect(),
            },
            WorkerPoolMember {
                worker_id: "beta".to_string(),
                kind: WorkerKind::Open,
                receipted_capabilities: ["rust_build".to_string()].into_iter().collect(),
            },
            WorkerPoolMember {
                worker_id: "gamma".to_string(),
                kind: WorkerKind::Open,
                receipted_capabilities: ["rust_build".to_string()].into_iter().collect(),
            },
        ];
        let pool = WorkerPoolBinding::from_candidates(candidates, "rust_build").expect("pool");

        let head_config = CoordinatorHeadConfig {
            hidden_dim: d_model,
            num_workers: pool.len(),
            num_roles: 3,
        };
        let seed_head = CoordinatorHead::zeros(head_config).expect("seed head");

        // Two samples, each must route to worker index 2 ("gamma") to Verify.
        let samples = vec![
            EvalSample {
                sample_id: "s0".to_string(),
                token_ids: vec![1, 2, 3],
            },
            EvalSample {
                sample_id: "s1".to_string(),
                token_ids: vec![4, 5, 6],
            },
        ];
        let source = SimulatedVerdictSource::new(vec![
            ("s0".to_string(), 2),
            ("s1".to_string(), 2),
        ]);

        // Frozen backbone hidden closure: forward_with_hidden -> [1, d_model].
        let hidden = move |tokens: &[usize]| -> Result<Vec<f32>, CoordinatorLiveTrainingError> {
            let (_, h) = backbone
                .forward_with_hidden(Shape::new(vec![1, tokens.len()]), tokens)
                .map_err(|error| CoordinatorLiveTrainingError::VerdictSource {
                    detail: error.to_string(),
                })?;
            h.as_f32_slice()
                .map(<[f32]>::to_vec)
                .map_err(|error| CoordinatorLiveTrainingError::VerdictSource {
                    detail: error.to_string(),
                })
        };

        let cap = DailySpendCap::owner_default("2026-06-22");
        let fitness = LiveCoordinatorFitness::new(
            seed_head.clone(),
            pool,
            TerminalRewardAdapter::offline(),
            samples,
            hidden,
            source,
            cap,
        )
        .expect("fitness");

        let dimension = head_config.parameter_count();
        let optimizer = SepCmaEs::new(SepCmaEsConfig {
            dimension,
            population_size: 24,
            generations: 60,
            initial_sigma: 0.5,
            seed: 0xD1CE_2026,
        })
        .expect("optimizer");

        let initial = seed_head.flatten_parameters().expect("flat");
        let outcome = optimizer.optimize(&fitness, &initial).expect("optimize");

        // The optimizer must drive a real CoordinatorHead, reading REAL frozen
        // backbone hidden states, to a verified routing (fitness 1.0).
        assert!(outcome.improved(), "expected ES to improve over the start");
        assert!(
            (outcome.best_fitness - 1.0).abs() < 1e-6,
            "expected a verified head (1.0), got {}",
            outcome.best_fitness
        );
        // No-spend lane: cap untouched, never halted.
        let snap = fitness.cap_snapshot();
        assert_eq!(snap.spent_today_msats(), 0);
        assert!(!fitness.halted_on_cap());
    }

    // ---- Live fitness FAILS CLOSED when a paid eval would breach the cap ----

    /// A verdict source that always Verifies but charges a fixed per-eval spend.
    struct PaidVerdictSource {
        per_eval_msats: u64,
    }
    impl EvalVerdictSource for PaidVerdictSource {
        fn verdict_for(
            &self,
            _request: &TrajectoryRequest,
        ) -> Result<VerdictOutcome, CoordinatorLiveTrainingError> {
            Ok(VerdictOutcome {
                verdict: VerificationVerdict::Verified,
                spend_msats: self.per_eval_msats,
            })
        }
    }

    #[test]
    fn live_fitness_fails_closed_at_the_cap() {
        let d_model = 4;
        let backbone = frozen_backbone(d_model, 8);
        use crate::coordinator_evolution::{WorkerKind, WorkerPoolMember};
        let pool = WorkerPoolBinding::from_candidates(
            vec![WorkerPoolMember {
                worker_id: "only".to_string(),
                kind: WorkerKind::Open,
                receipted_capabilities: ["cap".to_string()].into_iter().collect(),
            }],
            "cap",
        )
        .expect("pool");
        let head_config = CoordinatorHeadConfig {
            hidden_dim: d_model,
            num_workers: 1,
            num_roles: 3,
        };
        let seed_head = CoordinatorHead::zeros(head_config).expect("head");
        // One sample. Each eval charges 4,000,000 msats (4,000 sats); with the
        // 10,000,000 msat cap, only 2 evals fit, the 3rd must fail closed.
        let samples = vec![EvalSample {
            sample_id: "s0".to_string(),
            token_ids: vec![1, 2],
        }];
        let hidden = move |tokens: &[usize]| -> Result<Vec<f32>, CoordinatorLiveTrainingError> {
            let (_, h) = backbone
                .forward_with_hidden(Shape::new(vec![1, tokens.len()]), tokens)
                .map_err(|e| CoordinatorLiveTrainingError::VerdictSource {
                    detail: e.to_string(),
                })?;
            h.as_f32_slice()
                .map(<[f32]>::to_vec)
                .map_err(|e| CoordinatorLiveTrainingError::VerdictSource {
                    detail: e.to_string(),
                })
        };
        let cap = DailySpendCap::owner_default("2026-06-22");
        let fitness = LiveCoordinatorFitness::new(
            seed_head.clone(),
            pool,
            TerminalRewardAdapter::cost_aware(0.0),
            samples,
            hidden,
            PaidVerdictSource {
                per_eval_msats: 4_000_000,
            },
            cap,
        )
        .expect("fitness");

        let initial = seed_head.flatten_parameters().expect("flat");
        // First two evals fit (8,000,000 msats <= 10,000,000). The third breaches.
        assert!(fitness.evaluate_coordinator(&initial).is_ok());
        assert!(fitness.evaluate_coordinator(&initial).is_ok());
        let breach = fitness.evaluate_coordinator(&initial).unwrap_err();
        // Surfaced as a halt-on-cap error.
        assert!(format!("{breach}").contains("HALT_DAILY_CAP"));
        assert!(fitness.halted_on_cap());
        // Never spent past the cap.
        let snap = fitness.cap_snapshot();
        assert!(snap.spent_today_msats() <= OWNER_DAILY_CAP_MSATS);
        assert_eq!(snap.spent_today_msats(), 8_000_000);
    }
}
