//! Conductor GRPO NL-planner lane (Khala M7, OpenAgents issue #6015).
//!
//! This is the **M7 Conductor scaffold**: a natural-language planning
//! coordinator that decomposes a hard task across the M6 worker pool. Where the
//! TRINITY (M6) lane emits *logits* from a ~10K-param [`CoordinatorHead`] trained
//! by evolution ([`crate::SepCmaEs`]), the Conductor emits a *natural-language
//! workflow* — `model_id` / `subtasks` / `access_list` parallel lists, exactly
//! the shape from `docs/sakana/conductor.md` (Sakana AI, arXiv:2512.04388) — and
//! is trained by RL (GRPO).
//!
//! It **reuses the M6 substrate unchanged**:
//!
//! - **[`WorkerPoolBinding`]** (P5) is the pool the plan decomposes across. The
//!   plan's `model_id` entries are eligible-pool indices, validated against the
//!   binding; the Conductor can never name a worker outside the
//!   receipt-eligible set (the capability gate already filtered the binding), so
//!   the language plan cannot override the receipt gate.
//! - **[`EvalVerdictSource`]** (M6) is the reward: the executed workflow's
//!   terminal [`VerificationVerdict`] (replay-validator / verification-command
//!   verdict, never a prompted LLM judge) is the same object that releases
//!   settlement. Reward = the verdict; monetize on settlement
//!   (`docs/sakana/coordinator-as-verified-work.md`).
//! - **[`DailySpendCap`]** + **[`CoordinatorArmState`]** are the fail-closed,
//!   default-off gate: the trainer is an inert scaffold until armed, and the
//!   daily cap clamps to the owner's 10,000 sat/day ceiling.
//! - **[`crate::ShadowComparison`]** is the promotion gate the eventual paid
//!   composition demo consumes (learned vs single-model, verified-work-per-sat).
//!
//! ## TMAX stability recipe (`docs/research/tmax/synthesis.md` §5)
//!
//! The trainer scaffold adopts TMAX's agentic-RL stabilizers verbatim, because
//! the Conductor RL lane is exactly the regime they were measured in (rollouts
//! from a fast serving path, gradients from a trainer → training–inference
//! logprob mismatch):
//!
//! - **FP32 LM head** — the cheap, high-leverage fix for the logprob mismatch
//!   (high-frequency tokens like `\n` drive the worst mismatch). Modeled here by
//!   [`ConductorTrainerConfig::fp32_lm_head`] and applied in the logprob path
//!   ([`DpppoUpdate::token_is_masked`] reads FP32-projected logprobs).
//! - **DPPO over GRPO** — mask tokens where inference/training logprobs diverge
//!   (binary total-variation threshold 0.1). Modeled by
//!   [`DpppoUpdate::token_is_masked`]; masked tokens contribute no gradient.
//! - **Filter zero-std samples** — a GRPO group whose rewards are all identical
//!   has zero advantage signal and is dropped before the update
//!   ([`GrpoGroup::is_zero_std`]).
//! - Group size 32, KL β = 0, constant LR 1e-6, centered advantage — the TMAX
//!   Table-13 starting config, captured in [`ConductorTrainerConfig::tmax_table13`].
//!
//! ## What is real vs. owner/compute-gated
//!
//! **Real + tested here (CPU, no spend):**
//! - the typed [`ConductorPlan`] contract + parser + validation against the M6
//!   pool (subtasks + worker ids + access-list topology);
//! - the [`ConductorPlanner`] stepping interface (decode → parse → validate →
//!   one workflow step), driven by an injected text-generation seam so tests run
//!   without a 7B model;
//! - the [`ConductorTrainer`] GRPO/DPPO scaffold: it groups rollouts, filters
//!   zero-std groups, computes centered advantages, applies the DPPO TV-mask and
//!   the FP32-head logprob path, and produces a deterministic [`GrpoUpdateStep`]
//!   summary over a **fixture** verdict — proving the loop steps, NOT that the
//!   Conductor is good.
//!
//! **Owner / compute-gated (NOT built here, flagged precisely):**
//! - a real GRPO training run needs a 7B base policy, the FP32 head wired to a
//!   real autograd/serving split, and many H100-hours — this scaffold ships
//!   **no** model weights and **no** gradient backend, only the typed loop;
//! - the paid "khala solves crossy-road by composition, cheaper than
//!   single-model" demo additionally needs an **armed** [`EvalVerdictSource`]
//!   over the live Pylon pool (M4, #6012) + a spend-enabled buy-mode campaign +
//!   an M6 paid shadow-win — see [`ConductorReadiness`] for the exact gate list.
//!
//! Default-off discipline mirrors the M6 lanes: a fixture/offline path for
//! tests; the live training run gated behind arming + the daily cap. Nothing in
//! this module dispatches work, moves sats, or starts a training run.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::coordinator_eval_verdict_source::CoordinatorArmState;
use crate::coordinator_evolution::{
    TerminalRewardAdapter, TrajectoryOutcome, VerificationVerdict, WorkerPoolBinding,
};
use crate::coordinator_live_training::{DailySpendCap, OWNER_DAILY_CAP_MSATS};

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors raised by the Conductor lane.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ConductorError {
    /// The emitted plan did not parse into the three parallel lists.
    #[error("conductor plan parse error: {detail}")]
    PlanParse {
        /// Human-readable detail.
        detail: String,
    },
    /// The plan referenced a worker index outside the eligible pool.
    #[error(
        "plan step {step} names worker index {worker_index} but the eligible pool has {pool_len} workers"
    )]
    WorkerOutOfPool {
        /// The offending step index.
        step: usize,
        /// The named worker index.
        worker_index: usize,
        /// The eligible pool size.
        pool_len: usize,
    },
    /// The plan's access list referenced a step that is not strictly earlier.
    #[error(
        "plan step {step} access list references step {referenced}, which is not strictly earlier"
    )]
    AccessNotEarlier {
        /// The step whose access list is invalid.
        step: usize,
        /// The forward/self reference.
        referenced: usize,
    },
    /// The plan had no steps, or exceeded the step budget.
    #[error("plan has {steps} steps, outside the allowed range [1, {max_steps}]")]
    StepCountOutOfRange {
        /// The plan's step count.
        steps: usize,
        /// The configured maximum.
        max_steps: usize,
    },
    /// The three parallel lists had mismatched lengths.
    #[error(
        "plan lists have mismatched lengths: model_id={model_id}, subtasks={subtasks}, access_list={access_list}"
    )]
    MismatchedLists {
        /// `model_id` length.
        model_id: usize,
        /// `subtasks` length.
        subtasks: usize,
        /// `access_list` length.
        access_list: usize,
    },
    /// A trainer configuration value was invalid.
    #[error("invalid conductor trainer configuration: {detail}")]
    InvalidConfiguration {
        /// Human-readable detail.
        detail: String,
    },
    /// The trainer was asked to run a paid step while disarmed or over-cap.
    #[error(
        "conductor trainer is not armed for paid rollouts (state={state:?}); the scaffold stays inert until armed"
    )]
    NotArmed {
        /// The current arm state.
        state: CoordinatorArmState,
    },
}

// ---------------------------------------------------------------------------
// The Conductor plan contract (model_id / subtasks / access_list).
// ---------------------------------------------------------------------------

/// Which previous steps' outputs a step can see — the access-list topology from
/// `docs/sakana/conductor.md`. `"all"`, `[]`, or specific earlier indices. This
/// single field is what subsumes best-of-N, sequential chains, and arbitrary
/// tree topologies.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccessList {
    /// The step sees every strictly-earlier step's output.
    All,
    /// The step sees nothing (a fresh, independent attempt — best-of-N branch).
    None,
    /// The step sees exactly these earlier step indices.
    Indices(Vec<usize>),
}

impl AccessList {
    /// Validates the access list against the step's own position: every named
    /// index must be strictly earlier than `step` (no self/forward edges — the
    /// topology is a DAG over a linear ordering, the Conductor paper's scheme).
    fn validate(&self, step: usize) -> Result<(), ConductorError> {
        if let Self::Indices(indices) = self {
            for &referenced in indices {
                if referenced >= step {
                    return Err(ConductorError::AccessNotEarlier { step, referenced });
                }
            }
        }
        Ok(())
    }
}

/// One step of a Conductor workflow: which worker runs it, the natural-language
/// subtask, and which earlier outputs it can see.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConductorStep {
    /// Eligible-pool index of the worker assigned to this step (the paper's
    /// anonymous "Model k"). Validated against the [`WorkerPoolBinding`].
    pub worker_index: usize,
    /// The natural-language instruction for this step.
    pub subtask: String,
    /// Which earlier steps' outputs are visible to this step.
    pub access: AccessList,
}

/// A full Conductor plan: the parallel `model_id` / `subtasks` / `access_list`
/// lists the 7B Conductor emits after its chain-of-thought, materialized as a
/// validated step sequence. The final step's output is what is returned to the
/// user (per the paper).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConductorPlan {
    steps: Vec<ConductorStep>,
}

impl ConductorPlan {
    /// Builds a plan from already-typed steps and validates it against `pool` and
    /// `max_steps`. This is the canonical constructor; [`ConductorPlan::parse`]
    /// is a thin wrapper that builds the steps from the three parallel lists
    /// first.
    pub fn from_steps(
        steps: Vec<ConductorStep>,
        pool: &WorkerPoolBinding,
        max_steps: usize,
    ) -> Result<Self, ConductorError> {
        if steps.is_empty() || steps.len() > max_steps {
            return Err(ConductorError::StepCountOutOfRange {
                steps: steps.len(),
                max_steps,
            });
        }
        for (index, step) in steps.iter().enumerate() {
            if step.worker_index >= pool.len() {
                return Err(ConductorError::WorkerOutOfPool {
                    step: index,
                    worker_index: step.worker_index,
                    pool_len: pool.len(),
                });
            }
            step.access.validate(index)?;
        }
        Ok(Self { steps })
    }

    /// Parses a plan from the three parallel lists the Conductor emits, then
    /// validates it against the pool. The lists must be equal length.
    ///
    /// This is the deterministic, bounded-field parse that runs *after* the
    /// semantic decode (the model produces the lists; this only structures and
    /// bounds-checks them) — it does not do intent routing or keyword matching.
    pub fn parse(
        model_id: &[usize],
        subtasks: &[String],
        access_list: &[AccessList],
        pool: &WorkerPoolBinding,
        max_steps: usize,
    ) -> Result<Self, ConductorError> {
        if model_id.len() != subtasks.len() || model_id.len() != access_list.len() {
            return Err(ConductorError::MismatchedLists {
                model_id: model_id.len(),
                subtasks: subtasks.len(),
                access_list: access_list.len(),
            });
        }
        let steps = model_id
            .iter()
            .zip(subtasks.iter())
            .zip(access_list.iter())
            .map(|((worker_index, subtask), access)| ConductorStep {
                worker_index: *worker_index,
                subtask: subtask.clone(),
                access: access.clone(),
            })
            .collect();
        Self::from_steps(steps, pool, max_steps)
    }

    /// The validated steps.
    #[must_use]
    pub fn steps(&self) -> &[ConductorStep] {
        &self.steps
    }

    /// Number of steps the plan spends — the Conductor paper's inference-cost
    /// proxy (it learns to spend more steps on hard tasks, fewer on easy ones).
    #[must_use]
    pub fn step_count(&self) -> usize {
        self.steps.len()
    }

    /// The distinct eligible-pool worker indices the plan fans out across — the
    /// "compose-across-the-map" set the Verse view (M5) renders.
    #[must_use]
    pub fn worker_fanout(&self) -> BTreeSet<usize> {
        self.steps.iter().map(|s| s.worker_index).collect()
    }

    /// Resolves the plan's worker indices to stable worker ids through the pool
    /// binding, in step order. Returns `None` if any index is out of range
    /// (cannot happen for a validated plan, but the API stays total).
    #[must_use]
    pub fn resolve_worker_ids(&self, pool: &WorkerPoolBinding) -> Option<Vec<String>> {
        self.steps
            .iter()
            .map(|step| pool.resolve(step.worker_index).map(|w| w.worker_id.clone()))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// The Conductor planner stepping interface.
// ---------------------------------------------------------------------------

/// The text-generation seam the planner decodes from. A real run binds this to
/// a 7B base policy served from a fast inference path; tests bind a deterministic
/// fixture generator. Keeping it a trait means the planner + parser + validation
/// are fully testable on CPU with **no** model weights.
///
/// The generator returns the Conductor's raw natural-language output for a task
/// (the chain-of-thought + the three parallel lists). The planner does not
/// interpret it semantically beyond the bounded-field parse — the *language* is
/// the model's job.
pub trait ConductorPolicy {
    /// Generate the raw Conductor output (CoT + parallel lists) for one task,
    /// given the anonymized pool size `pool_len` (workers are "Model 0..L").
    fn generate_plan(
        &self,
        task_prompt: &str,
        pool_len: usize,
    ) -> Result<ConductorRawPlan, ConductorError>;
}

/// The raw three-list output a [`ConductorPolicy`] produces, before validation.
/// Separating "what the model said" from "what validated" keeps the parse seam
/// honest: a malformed model output is a parse/format failure (reward 0 in the
/// GRPO format condition), not a panic.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConductorRawPlan {
    /// `model_id` list — anonymized worker ordinals, one per step.
    pub model_id: Vec<usize>,
    /// `subtasks` list — the natural-language instruction per step.
    pub subtasks: Vec<String>,
    /// `access_list` list — the topology per step.
    pub access_list: Vec<AccessList>,
}

/// The Conductor planner: decode a raw plan from the policy, parse + validate it
/// against the M6 pool, and expose the validated workflow for stepping.
///
/// This is the M7 analogue of `CoordinatorHead::decide` — but where `decide`
/// emits one `(worker, role)` argmax, the planner emits a whole multi-step
/// workflow in natural language.
pub struct ConductorPlanner<P: ConductorPolicy> {
    policy: P,
    pool: WorkerPoolBinding,
    max_steps: usize,
}

impl<P: ConductorPolicy> ConductorPlanner<P> {
    /// Builds a planner over a policy and an M6 worker-pool binding. `max_steps`
    /// caps the workflow length (the paper's 5-step limit; the Conductor learns
    /// to use ~3 on average).
    pub fn new(
        policy: P,
        pool: WorkerPoolBinding,
        max_steps: usize,
    ) -> Result<Self, ConductorError> {
        if max_steps == 0 {
            return Err(ConductorError::InvalidConfiguration {
                detail: String::from("max_steps must be non-zero"),
            });
        }
        Ok(Self {
            policy,
            pool,
            max_steps,
        })
    }

    /// The eligible pool the planner decomposes across.
    #[must_use]
    pub fn pool(&self) -> &WorkerPoolBinding {
        &self.pool
    }

    /// Plan one task: decode → parse → validate against the pool. Returns a
    /// validated [`ConductorPlan`] or a structured [`ConductorError`] (a parse
    /// or validation failure is the GRPO format-condition reward-0 signal, not a
    /// crash).
    pub fn plan(&self, task_prompt: &str) -> Result<ConductorPlan, ConductorError> {
        let raw = self.policy.generate_plan(task_prompt, self.pool.len())?;
        ConductorPlan::parse(
            &raw.model_id,
            &raw.subtasks,
            &raw.access_list,
            &self.pool,
            self.max_steps,
        )
    }

    /// One planning *step* in the M7 plan→implement→verify→refine loop: produce a
    /// plan and a [`PlanStepOutcome`] describing the next action over the pool
    /// (which worker the next subtask routes to, what it can see). This is the
    /// stepping interface the Verse fan-out view (M5) and the executor consume;
    /// it dispatches nothing itself.
    pub fn step(
        &self,
        task_prompt: &str,
        step_index: usize,
    ) -> Result<PlanStepOutcome, ConductorError> {
        let plan = self.plan(task_prompt)?;
        let step = plan
            .steps()
            .get(step_index)
            .ok_or(ConductorError::StepCountOutOfRange {
                steps: step_index,
                max_steps: plan.step_count(),
            })?;
        let worker_id = self
            .pool
            .resolve(step.worker_index)
            .map(|w| w.worker_id.clone())
            .ok_or(ConductorError::WorkerOutOfPool {
                step: step_index,
                worker_index: step.worker_index,
                pool_len: self.pool.len(),
            })?;
        Ok(PlanStepOutcome {
            step_index,
            worker_index: step.worker_index,
            worker_id,
            subtask: step.subtask.clone(),
            access: step.access.clone(),
            is_final: step_index + 1 == plan.step_count(),
        })
    }
}

/// The outcome of one planner step: the resolved next action over the pool. The
/// executor turns this into an inference dispatch (priced, capped) on the live
/// lane; on the offline lane it is inspected by tests and the Verse view.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanStepOutcome {
    /// Index of this step in the workflow.
    pub step_index: usize,
    /// Eligible-pool index of the worker the subtask routes to.
    pub worker_index: usize,
    /// Stable id of that worker (resolved through the P5 binding).
    pub worker_id: String,
    /// The natural-language subtask.
    pub subtask: String,
    /// The earlier outputs this step can see.
    pub access: AccessList,
    /// Whether this is the final step (its output returns to the user).
    pub is_final: bool,
}

// ---------------------------------------------------------------------------
// TMAX stability recipe + GRPO/DPPO trainer scaffold (inert until armed).
// ---------------------------------------------------------------------------

/// The TMAX Table-13 starting config for the Conductor GRPO/DPPO lane
/// (`docs/research/tmax/synthesis.md` §5). These are the documented stabilizers
/// that transfer to *any* agentic-RL policy whose rollouts come from a fast
/// serving path and gradients from a trainer.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConductorTrainerConfig {
    /// FP32 LM head — the cheap, high-leverage fix for the training–inference
    /// logprob mismatch. When `true`, the logprob path uses FP32-projected
    /// logits (modeled here as exact-precision logprobs).
    pub fp32_lm_head: bool,
    /// DPPO total-variation mask threshold. Tokens whose inference/training
    /// logprob TV distance exceeds this are masked out of the update (binary
    /// 0.1 in TMAX). `0.0` disables masking (plain GRPO).
    pub dppo_tv_threshold: f32,
    /// Whether to filter GRPO groups whose rewards have zero standard deviation
    /// (no advantage signal). TMAX: always on.
    pub filter_zero_std: bool,
    /// GRPO group size (rollouts per prompt). TMAX: 32.
    pub group_size: usize,
    /// KL regularization coefficient β. TMAX: 0 (no KL).
    pub kl_beta: f32,
    /// Constant learning rate. TMAX: 1e-6.
    pub learning_rate: f32,
    /// Whether advantages are centered (mean-subtracted within group). TMAX: on.
    pub center_advantage: bool,
}

impl ConductorTrainerConfig {
    /// The TMAX §5 / Table-13 starting config: FP32 head ON, DPPO TV mask 0.1,
    /// zero-std filtering ON, group size 32, KL β 0, LR 1e-6, centered advantage.
    #[must_use]
    pub const fn tmax_table13() -> Self {
        Self {
            fp32_lm_head: true,
            dppo_tv_threshold: 0.1,
            filter_zero_std: true,
            group_size: 32,
            kl_beta: 0.0,
            learning_rate: 1e-6,
            center_advantage: true,
        }
    }

    fn validate(self) -> Result<Self, ConductorError> {
        if self.group_size == 0 {
            return Err(ConductorError::InvalidConfiguration {
                detail: String::from("group_size must be non-zero"),
            });
        }
        if !(0.0..=1.0).contains(&self.dppo_tv_threshold) {
            return Err(ConductorError::InvalidConfiguration {
                detail: format!(
                    "dppo_tv_threshold {} must be in [0, 1]",
                    self.dppo_tv_threshold
                ),
            });
        }
        if !self.learning_rate.is_finite() || self.learning_rate <= 0.0 {
            return Err(ConductorError::InvalidConfiguration {
                detail: format!(
                    "learning_rate {} must be finite and positive",
                    self.learning_rate
                ),
            });
        }
        Ok(self)
    }
}

impl Default for ConductorTrainerConfig {
    fn default() -> Self {
        Self::tmax_table13()
    }
}

/// One rollout in a GRPO group: the Conductor produced a plan for a prompt, the
/// plan was executed, and the [`EvalVerdictSource`](crate::EvalVerdictSource)
/// returned a terminal verdict + spend. The per-token logprob pair (inference
/// vs training) drives the DPPO mask.
///
/// On the fixture lane the verdict is a fixture [`TrajectoryOutcome`]; on the
/// owner-gated paid lane it is the live replay-validator verdict.
#[derive(Clone, Debug, PartialEq)]
pub struct ConductorRollout {
    /// The prompt id this rollout is for (rollouts with the same prompt id form
    /// a GRPO group).
    pub prompt_id: String,
    /// The terminal outcome (verdict + spend) the reward is computed from.
    pub outcome: TrajectoryOutcome,
    /// Per-token `(inference_logprob, training_logprob)` pairs. The DPPO mask
    /// drops tokens whose TV distance exceeds the threshold. Empty is allowed
    /// (a format-failure rollout contributes only its reward).
    pub token_logprobs: Vec<(f32, f32)>,
}

/// A GRPO group: all rollouts for one prompt. The group-relative advantage is
/// each rollout's reward minus the group mean (centered, no value network).
#[derive(Clone, Debug, PartialEq)]
pub struct GrpoGroup {
    rewards: Vec<f32>,
}

impl GrpoGroup {
    fn from_rewards(rewards: Vec<f32>) -> Self {
        Self { rewards }
    }

    fn mean(&self) -> f32 {
        if self.rewards.is_empty() {
            return 0.0;
        }
        self.rewards.iter().sum::<f32>() / self.rewards.len() as f32
    }

    /// Population standard deviation of the group's rewards.
    fn std(&self) -> f32 {
        if self.rewards.len() < 2 {
            return 0.0;
        }
        let mean = self.mean();
        let var = self
            .rewards
            .iter()
            .map(|r| {
                let d = r - mean;
                d * d
            })
            .sum::<f32>()
            / self.rewards.len() as f32;
        var.sqrt()
    }

    /// Whether the group has zero reward spread — a TMAX zero-std sample with no
    /// advantage signal, filtered before the update.
    #[must_use]
    pub fn is_zero_std(&self) -> bool {
        self.std() <= f32::EPSILON
    }

    /// Centered group-relative advantages (reward − group mean). This is the
    /// GRPO advantage (no value network).
    fn centered_advantages(&self) -> Vec<f32> {
        let mean = self.mean();
        self.rewards.iter().map(|r| r - mean).collect()
    }
}

/// The DPPO update masker: decides which tokens contribute gradient. A token is
/// masked when the inference/training logprob total-variation distance exceeds
/// the threshold (TMAX binary 0.1), which is the dominant source of training
/// collapse. With an FP32 head the divergence shrinks, so fewer tokens mask.
#[derive(Clone, Copy, Debug)]
pub struct DpppoUpdate {
    tv_threshold: f32,
    fp32_lm_head: bool,
}

impl DpppoUpdate {
    fn new(config: &ConductorTrainerConfig) -> Self {
        Self {
            tv_threshold: config.dppo_tv_threshold,
            fp32_lm_head: config.fp32_lm_head,
        }
    }

    /// Whether a token is masked out of the DPPO update. `inference_logprob` is
    /// the logprob the rollout was sampled under (fast serving path);
    /// `training_logprob` is the trainer's recomputed logprob. The FP32-head
    /// path is modeled by collapsing the training-side rounding error so the
    /// effective divergence is the genuine policy gap, not a precision artifact.
    #[must_use]
    pub fn token_is_masked(&self, inference_logprob: f32, training_logprob: f32) -> bool {
        if self.tv_threshold <= 0.0 {
            // Plain GRPO: never mask.
            return false;
        }
        // FP32 head: the training-side logprob is treated as exact, so the
        // measured divergence is the real inference/training gap. A BF16 head
        // would add a precision floor; we model that by not collapsing it.
        let training = if self.fp32_lm_head {
            training_logprob
        } else {
            // Coarse precision floor for a non-FP32 head (illustrative).
            (training_logprob * 256.0).round() / 256.0
        };
        // Total variation between two Bernoulli-ish logprob points is |Δp|;
        // approximate with the probability gap from the logprobs.
        let p_inf = inference_logprob.exp().clamp(0.0, 1.0);
        let p_train = training.exp().clamp(0.0, 1.0);
        (p_inf - p_train).abs() > self.tv_threshold
    }

    /// The fraction of a rollout's tokens that survive the DPPO mask (contribute
    /// gradient). `1.0` when there are no tokens (nothing to mask).
    #[must_use]
    pub fn unmasked_fraction(&self, token_logprobs: &[(f32, f32)]) -> f32 {
        if token_logprobs.is_empty() {
            return 1.0;
        }
        let kept = token_logprobs
            .iter()
            .filter(|(inf, train)| !self.token_is_masked(*inf, *train))
            .count();
        kept as f32 / token_logprobs.len() as f32
    }
}

/// One deterministic GRPO/DPPO update-step summary over a batch of rollouts.
/// This is what the fixture test asserts on: it proves the loop groups,
/// zero-std-filters, centers advantages, and DPPO-masks — it does **not** apply
/// a gradient (no autograd backend ships in this scaffold).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GrpoUpdateStep {
    /// Number of GRPO groups seen.
    pub groups: usize,
    /// Number of groups dropped by the zero-std filter.
    pub zero_std_filtered: usize,
    /// Number of rollouts that contributed advantage (in surviving groups).
    pub contributing_rollouts: usize,
    /// Mean absolute centered advantage over contributing rollouts (the update
    /// signal magnitude; 0 means nothing to learn from this batch).
    pub mean_abs_advantage: f32,
    /// Mean DPPO-unmasked token fraction over contributing rollouts (1.0 = no
    /// tokens masked; lower = DPPO suppressed more divergent tokens).
    pub mean_unmasked_fraction: f32,
    /// Mean reward over all rollouts (the verified-work signal; reward = the
    /// verification verdict via the [`TerminalRewardAdapter`]).
    pub mean_reward: f32,
}

/// The Conductor GRPO/DPPO trainer scaffold. **Inert until armed.** It computes
/// a deterministic [`GrpoUpdateStep`] from a batch of rollouts using the TMAX
/// recipe, but applies no gradient and dispatches no work. The paid rollout
/// collection (which moves sats) is gated behind [`CoordinatorArmState::Armed`]
/// + the [`DailySpendCap`]; on the fixture lane rollouts are supplied directly.
pub struct ConductorTrainer {
    config: ConductorTrainerConfig,
    reward: TerminalRewardAdapter,
    arm_state: CoordinatorArmState,
    cap: DailySpendCap,
}

impl ConductorTrainer {
    /// Builds a trainer. **Defaults to disarmed**: the caller must explicitly
    /// [`arm`](Self::arm) before any paid rollout is permitted, and even then
    /// the daily cap clamps to the owner ceiling.
    pub fn new(
        config: ConductorTrainerConfig,
        reward: TerminalRewardAdapter,
        cap: DailySpendCap,
    ) -> Result<Self, ConductorError> {
        let config = config.validate()?;
        Ok(Self {
            config,
            reward,
            arm_state: CoordinatorArmState::Disarmed,
            cap,
        })
    }

    /// A trainer with the TMAX Table-13 config, the offline reward adapter, and a
    /// disarmed owner-ceiling cap for `day_key`. The default-off scaffold.
    pub fn offline_scaffold(day_key: impl Into<String>) -> Result<Self, ConductorError> {
        Self::new(
            ConductorTrainerConfig::tmax_table13(),
            TerminalRewardAdapter::offline(),
            DailySpendCap::owner_default(day_key),
        )
    }

    /// The TMAX recipe config in force.
    #[must_use]
    pub fn config(&self) -> ConductorTrainerConfig {
        self.config
    }

    /// The current arm state (default [`CoordinatorArmState::Disarmed`]).
    #[must_use]
    pub fn arm_state(&self) -> CoordinatorArmState {
        self.arm_state
    }

    /// The daily spend cap snapshot.
    #[must_use]
    pub fn cap(&self) -> &DailySpendCap {
        &self.cap
    }

    /// Arms the trainer for paid rollouts. This is the **owner decision** — never
    /// a default. Arming alone does not start a run; every paid rollout still
    /// pre-checks the cap. The owner ceiling remains a hard upper bound.
    pub fn arm(&mut self) {
        self.arm_state = CoordinatorArmState::Armed;
    }

    /// Disarms the trainer (returns to the inert default).
    pub fn disarm(&mut self) {
        self.arm_state = CoordinatorArmState::Disarmed;
    }

    /// Guards a *paid* rollout collection: refuses cleanly while disarmed, and
    /// pre-checks the daily cap for `amount_msats` before any spend. This is the
    /// fail-closed seam the live executor must call before dispatching priced
    /// inference. It moves no sats itself (the caller debits via the returned
    /// guard); it only enforces the gate.
    ///
    /// Returns the remaining budget after a successful guard so the executor can
    /// stop before breaching the cap.
    pub fn guard_paid_rollout(&self, amount_msats: u64) -> Result<u64, ConductorError> {
        if !self.arm_state.is_armed() {
            return Err(ConductorError::NotArmed {
                state: self.arm_state,
            });
        }
        if !self.cap.can_spend(amount_msats) || amount_msats > OWNER_DAILY_CAP_MSATS {
            return Err(ConductorError::NotArmed {
                state: self.arm_state,
            });
        }
        Ok(self.cap.remaining_msats().saturating_sub(amount_msats))
    }

    /// Computes one deterministic GRPO/DPPO update-step summary from a batch of
    /// rollouts grouped by `prompt_id`. This is the **offline / fixture** path:
    /// it requires no arming because it consumes already-collected rollouts (the
    /// caller is responsible for not having spent without arming). It groups,
    /// zero-std-filters, centers advantages, and DPPO-masks per the TMAX recipe.
    ///
    /// Deterministic: same rollouts → same [`GrpoUpdateStep`].
    #[must_use]
    pub fn update_step(&self, rollouts: &[ConductorRollout]) -> GrpoUpdateStep {
        // Group rollouts by prompt_id, preserving first-seen order for
        // determinism.
        let mut group_order: Vec<String> = Vec::new();
        let mut grouped: std::collections::BTreeMap<String, Vec<&ConductorRollout>> =
            std::collections::BTreeMap::new();
        for rollout in rollouts {
            if !grouped.contains_key(&rollout.prompt_id) {
                group_order.push(rollout.prompt_id.clone());
            }
            grouped
                .entry(rollout.prompt_id.clone())
                .or_default()
                .push(rollout);
        }

        let dppo = DpppoUpdate::new(&self.config);

        let mut zero_std_filtered = 0usize;
        let mut contributing_rollouts = 0usize;
        let mut abs_adv_sum = 0.0f32;
        let mut unmasked_sum = 0.0f32;
        let mut reward_sum = 0.0f32;
        let mut reward_count = 0usize;

        for prompt_id in &group_order {
            let members = &grouped[prompt_id];
            let rewards: Vec<f32> = members
                .iter()
                .map(|r| self.reward.scalar(r.outcome))
                .collect();
            for &reward in &rewards {
                reward_sum += reward;
                reward_count += 1;
            }
            let group = GrpoGroup::from_rewards(rewards);
            if self.config.filter_zero_std && group.is_zero_std() {
                zero_std_filtered += 1;
                continue;
            }
            let advantages = if self.config.center_advantage {
                group.centered_advantages()
            } else {
                group.rewards.clone()
            };
            for (rollout, advantage) in members.iter().zip(advantages.iter()) {
                contributing_rollouts += 1;
                abs_adv_sum += advantage.abs();
                unmasked_sum += dppo.unmasked_fraction(&rollout.token_logprobs);
            }
        }

        let mean_abs_advantage = if contributing_rollouts == 0 {
            0.0
        } else {
            abs_adv_sum / contributing_rollouts as f32
        };
        let mean_unmasked_fraction = if contributing_rollouts == 0 {
            1.0
        } else {
            unmasked_sum / contributing_rollouts as f32
        };
        let mean_reward = if reward_count == 0 {
            0.0
        } else {
            reward_sum / reward_count as f32
        };

        GrpoUpdateStep {
            groups: group_order.len(),
            zero_std_filtered,
            contributing_rollouts,
            mean_abs_advantage,
            mean_unmasked_fraction,
            mean_reward,
        }
    }
}

// ---------------------------------------------------------------------------
// Readiness gate: what stands between this scaffold and the Done-when demo.
// ---------------------------------------------------------------------------

/// The precise gate list between this M7 scaffold and the issue-#6015 "Done
/// when": `openagents/khala` solving the crossy-road task by composition,
/// beating single-model cost at comparable quality (verified by the M2 rubric).
///
/// Every field is `false` in the shipped scaffold; flipping them is owner /
/// compute work, NOT code that lands here. This is the honest "what's left".
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConductorReadiness {
    /// A 7B base policy + FP32-head + autograd/serving split is wired (compute).
    pub policy_backend_wired: bool,
    /// A real GRPO training run has been executed and the loop converges
    /// (compute / H100-hours).
    pub training_run_executed: bool,
    /// The [`EvalVerdictSource`](crate::EvalVerdictSource) is **armed** over the
    /// live Pylon pool (M4, #6012) with a spend-enabled buy-mode campaign
    /// (owner).
    pub paid_verdict_source_armed: bool,
    /// An M6 paid shadow-win (verified-work-per-sat) over single-model has been
    /// recorded by [`ShadowComparison`](crate::ShadowComparison) (owner + M6).
    pub paid_shadow_win_recorded: bool,
    /// The crossy-road composition beats single-model cost at comparable quality
    /// under the M2 rubric (the Done-when proof).
    pub crossy_road_composition_verified: bool,
}

impl ConductorReadiness {
    /// The shipped scaffold state: everything ahead is owner/compute-gated.
    #[must_use]
    pub const fn scaffold() -> Self {
        Self {
            policy_backend_wired: false,
            training_run_executed: false,
            paid_verdict_source_armed: false,
            paid_shadow_win_recorded: false,
            crossy_road_composition_verified: false,
        }
    }

    /// Whether the Done-when bar is met (all gates green).
    #[must_use]
    pub const fn done_when_met(&self) -> bool {
        self.policy_backend_wired
            && self.training_run_executed
            && self.paid_verdict_source_armed
            && self.paid_shadow_win_recorded
            && self.crossy_road_composition_verified
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coordinator_evolution::{WorkerKind, WorkerPoolMember};

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

    /// A fixture policy that replays a fixed raw plan, so the planner +
    /// parser + validation are exercised with no model weights.
    struct FixturePolicy {
        raw: ConductorRawPlan,
    }

    impl ConductorPolicy for FixturePolicy {
        fn generate_plan(
            &self,
            _task_prompt: &str,
            _pool_len: usize,
        ) -> Result<ConductorRawPlan, ConductorError> {
            Ok(self.raw.clone())
        }
    }

    // ---- Plan contract ----------------------------------------------------

    #[test]
    fn plan_parses_and_validates_against_pool() {
        let pool = pool(4);
        // plan -> implement -> verify -> refine, a 4-step crossy-road workflow.
        let plan = ConductorPlan::parse(
            &[0, 1, 2, 1],
            &[
                "plan the crossy-road solution".to_string(),
                "implement the movement logic".to_string(),
                "verify against the rubric".to_string(),
                "refine using the verifier feedback".to_string(),
            ],
            &[
                AccessList::None,
                AccessList::Indices(vec![0]),
                AccessList::Indices(vec![1]),
                AccessList::All,
            ],
            &pool,
            5,
        )
        .expect("valid plan");
        assert_eq!(plan.step_count(), 4);
        assert_eq!(plan.worker_fanout(), [0usize, 1, 2].into_iter().collect());
        let ids = plan.resolve_worker_ids(&pool).expect("ids");
        assert_eq!(
            ids,
            vec!["worker-00", "worker-01", "worker-02", "worker-01"]
        );
    }

    #[test]
    fn plan_rejects_worker_outside_pool() {
        let pool = pool(2);
        let err = ConductorPlan::parse(
            &[0, 5],
            &["a".to_string(), "b".to_string()],
            &[AccessList::None, AccessList::None],
            &pool,
            5,
        )
        .unwrap_err();
        assert_eq!(
            err,
            ConductorError::WorkerOutOfPool {
                step: 1,
                worker_index: 5,
                pool_len: 2,
            }
        );
    }

    #[test]
    fn plan_rejects_forward_or_self_access_edge() {
        let pool = pool(2);
        // Step 0 cannot reference step 0 (self) or any later step.
        let err = ConductorPlan::parse(
            &[0, 1],
            &["a".to_string(), "b".to_string()],
            &[AccessList::Indices(vec![1]), AccessList::None],
            &pool,
            5,
        )
        .unwrap_err();
        assert_eq!(
            err,
            ConductorError::AccessNotEarlier {
                step: 0,
                referenced: 1,
            }
        );
    }

    #[test]
    fn plan_rejects_mismatched_lists() {
        let pool = pool(2);
        let err = ConductorPlan::parse(
            &[0, 1],
            &["a".to_string()],
            &[AccessList::None, AccessList::None],
            &pool,
            5,
        )
        .unwrap_err();
        assert_eq!(
            err,
            ConductorError::MismatchedLists {
                model_id: 2,
                subtasks: 1,
                access_list: 2,
            }
        );
    }

    #[test]
    fn plan_rejects_step_count_out_of_range() {
        let pool = pool(2);
        let empty = ConductorPlan::from_steps(vec![], &pool, 5).unwrap_err();
        assert_eq!(
            empty,
            ConductorError::StepCountOutOfRange {
                steps: 0,
                max_steps: 5
            }
        );
        let over = ConductorPlan::parse(
            &[0, 0, 0],
            &["a".to_string(), "b".to_string(), "c".to_string()],
            &[AccessList::None, AccessList::None, AccessList::None],
            &pool,
            2,
        )
        .unwrap_err();
        assert_eq!(
            over,
            ConductorError::StepCountOutOfRange {
                steps: 3,
                max_steps: 2
            }
        );
    }

    // ---- Planner stepping interface --------------------------------------

    #[test]
    fn planner_emits_valid_plan_over_pool() {
        let policy = FixturePolicy {
            raw: ConductorRawPlan {
                model_id: vec![0, 1, 2],
                subtasks: vec!["plan".into(), "implement".into(), "verify".into()],
                access_list: vec![
                    AccessList::None,
                    AccessList::Indices(vec![0]),
                    AccessList::All,
                ],
            },
        };
        let planner = ConductorPlanner::new(policy, pool(3), 5).expect("planner");
        let plan = planner.plan("solve crossy-road").expect("plan");
        assert_eq!(plan.step_count(), 3);
        assert_eq!(plan.worker_fanout().len(), 3);
    }

    #[test]
    fn planner_step_resolves_worker_and_flags_final() {
        let policy = FixturePolicy {
            raw: ConductorRawPlan {
                model_id: vec![0, 2],
                subtasks: vec!["plan".into(), "implement".into()],
                access_list: vec![AccessList::None, AccessList::Indices(vec![0])],
            },
        };
        let planner = ConductorPlanner::new(policy, pool(3), 5).expect("planner");
        let first = planner.step("task", 0).expect("step 0");
        assert_eq!(first.worker_id, "worker-00");
        assert!(!first.is_final);
        let last = planner.step("task", 1).expect("step 1");
        assert_eq!(last.worker_id, "worker-02");
        assert!(last.is_final);
    }

    #[test]
    fn planner_surfaces_parse_failure_as_error_not_panic() {
        // A malformed (mismatched) raw plan is a format-condition failure, the
        // GRPO reward-0 signal — surfaced as an Err, never a panic.
        let policy = FixturePolicy {
            raw: ConductorRawPlan {
                model_id: vec![0, 1],
                subtasks: vec!["only one".into()],
                access_list: vec![AccessList::None, AccessList::None],
            },
        };
        let planner = ConductorPlanner::new(policy, pool(2), 5).expect("planner");
        assert!(matches!(
            planner.plan("task"),
            Err(ConductorError::MismatchedLists { .. })
        ));
    }

    // ---- TMAX recipe config ----------------------------------------------

    #[test]
    fn tmax_table13_config_is_the_documented_recipe() {
        let c = ConductorTrainerConfig::tmax_table13();
        assert!(c.fp32_lm_head);
        assert!((c.dppo_tv_threshold - 0.1).abs() < 1e-9);
        assert!(c.filter_zero_std);
        assert_eq!(c.group_size, 32);
        assert_eq!(c.kl_beta, 0.0);
        assert!((c.learning_rate - 1e-6).abs() < 1e-12);
        assert!(c.center_advantage);
        assert_eq!(ConductorTrainerConfig::default(), c);
    }

    #[test]
    fn trainer_rejects_invalid_config() {
        let mut bad = ConductorTrainerConfig::tmax_table13();
        bad.group_size = 0;
        assert!(matches!(
            ConductorTrainer::new(
                bad,
                TerminalRewardAdapter::offline(),
                DailySpendCap::owner_default("2026-06-23")
            ),
            Err(ConductorError::InvalidConfiguration { .. })
        ));
    }

    // ---- DPPO mask + FP32 head -------------------------------------------

    #[test]
    fn dppo_masks_divergent_tokens_and_keeps_aligned_ones() {
        let dppo = DpppoUpdate::new(&ConductorTrainerConfig::tmax_table13());
        // Aligned token: inference and training logprobs near-equal -> kept.
        assert!(!dppo.token_is_masked(-0.01, -0.01));
        // Divergent token: large probability gap -> masked (DPPO suppresses).
        // p_inf = exp(-0.01) ~ 0.99, p_train = exp(-3.0) ~ 0.05; |Δ| ~ 0.94 > 0.1.
        assert!(dppo.token_is_masked(-0.01, -3.0));
    }

    #[test]
    fn plain_grpo_threshold_zero_never_masks() {
        let mut cfg = ConductorTrainerConfig::tmax_table13();
        cfg.dppo_tv_threshold = 0.0;
        let dppo = DpppoUpdate::new(&cfg);
        assert!(!dppo.token_is_masked(-0.01, -5.0));
        assert!((dppo.unmasked_fraction(&[(-0.01, -5.0), (-0.1, -2.0)]) - 1.0).abs() < 1e-6);
    }

    // ---- GRPO group / zero-std filter ------------------------------------

    #[test]
    fn zero_std_group_is_flagged() {
        let all_same = GrpoGroup::from_rewards(vec![1.0, 1.0, 1.0, 1.0]);
        assert!(all_same.is_zero_std());
        let mixed = GrpoGroup::from_rewards(vec![1.0, 0.0, 1.0, 0.0]);
        assert!(!mixed.is_zero_std());
    }

    // ---- Trainer update step (fixture, deterministic, no spend) -----------

    fn rollout(prompt: &str, verified: bool, logprobs: Vec<(f32, f32)>) -> ConductorRollout {
        ConductorRollout {
            prompt_id: prompt.to_string(),
            outcome: TrajectoryOutcome::offline(if verified {
                VerificationVerdict::Verified
            } else {
                VerificationVerdict::Rejected
            }),
            token_logprobs: logprobs,
        }
    }

    #[test]
    fn update_step_filters_zero_std_and_centers_advantage() {
        let trainer = ConductorTrainer::offline_scaffold("2026-06-23").expect("trainer");
        let rollouts = vec![
            // Group A: mixed rewards -> contributes (non-zero std).
            rollout("A", true, vec![(-0.01, -0.01), (-0.02, -2.0)]),
            rollout("A", false, vec![(-0.01, -0.01)]),
            // Group B: all verified -> zero std -> filtered.
            rollout("B", true, vec![(-0.01, -0.01)]),
            rollout("B", true, vec![(-0.01, -0.01)]),
        ];
        let step = trainer.update_step(&rollouts);
        assert_eq!(step.groups, 2);
        assert_eq!(step.zero_std_filtered, 1);
        // Only group A's two rollouts contribute.
        assert_eq!(step.contributing_rollouts, 2);
        // Centered advantages for [1.0, 0.0] -> [+0.5, -0.5]; mean abs = 0.5.
        assert!((step.mean_abs_advantage - 0.5).abs() < 1e-6);
        // mean reward over all 4 rollouts = (1+0+1+1)/4 = 0.75.
        assert!((step.mean_reward - 0.75).abs() < 1e-6);
        // Group A second rollout fully aligned; first has one divergent token.
        assert!(step.mean_unmasked_fraction > 0.0 && step.mean_unmasked_fraction <= 1.0);
    }

    #[test]
    fn update_step_is_deterministic() {
        let trainer = ConductorTrainer::offline_scaffold("2026-06-23").expect("trainer");
        let rollouts = vec![
            rollout("A", true, vec![(-0.01, -0.01)]),
            rollout("A", false, vec![(-0.01, -2.0)]),
        ];
        assert_eq!(
            trainer.update_step(&rollouts),
            trainer.update_step(&rollouts)
        );
    }

    // ---- Default-off / fail-closed arming --------------------------------

    #[test]
    fn trainer_is_disarmed_by_default_and_refuses_paid_rollouts() {
        let trainer = ConductorTrainer::offline_scaffold("2026-06-23").expect("trainer");
        assert_eq!(trainer.arm_state(), CoordinatorArmState::Disarmed);
        // A paid rollout is refused while disarmed (no spend, no dispatch).
        assert!(matches!(
            trainer.guard_paid_rollout(1_000),
            Err(ConductorError::NotArmed { .. })
        ));
    }

    #[test]
    fn armed_trainer_still_fails_closed_over_cap() {
        let mut trainer = ConductorTrainer::new(
            ConductorTrainerConfig::tmax_table13(),
            TerminalRewardAdapter::cost_aware(0.001),
            DailySpendCap::for_day("2026-06-23", 5_000),
        )
        .expect("trainer");
        trainer.arm();
        assert_eq!(trainer.arm_state(), CoordinatorArmState::Armed);
        // Within cap: guard passes and reports remaining budget.
        let remaining = trainer.guard_paid_rollout(3_000).expect("within cap");
        assert_eq!(remaining, 5_000 - 3_000);
        // Over cap: fails closed even while armed.
        assert!(matches!(
            trainer.guard_paid_rollout(6_000),
            Err(ConductorError::NotArmed { .. })
        ));
    }

    #[test]
    fn owner_ceiling_is_a_hard_upper_bound_on_paid_guard() {
        let mut trainer = ConductorTrainer::new(
            ConductorTrainerConfig::tmax_table13(),
            TerminalRewardAdapter::offline(),
            // Request above the owner ceiling is clamped down to it.
            DailySpendCap::for_day("2026-06-23", OWNER_DAILY_CAP_MSATS + 50_000),
        )
        .expect("trainer");
        trainer.arm();
        assert_eq!(trainer.cap().cap_msats(), OWNER_DAILY_CAP_MSATS);
        // A request above the owner ceiling fails closed.
        assert!(matches!(
            trainer.guard_paid_rollout(OWNER_DAILY_CAP_MSATS + 1),
            Err(ConductorError::NotArmed { .. })
        ));
    }

    // ---- Readiness gate ---------------------------------------------------

    #[test]
    fn scaffold_readiness_is_all_gates_owner_compute_gated() {
        let r = ConductorReadiness::scaffold();
        assert!(!r.done_when_met());
        assert!(!r.policy_backend_wired);
        assert!(!r.training_run_executed);
        assert!(!r.paid_verdict_source_armed);
        assert!(!r.paid_shadow_win_recorded);
        assert!(!r.crossy_road_composition_verified);
    }
}
