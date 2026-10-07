//! Learned-coordinator evolution lane (Khala M6 / TRINITY substrate, P3–P5).
//!
//! This module lands the three remaining learning-side primitives from
//! `docs/sakana/psionic-coordinator-roadmap.md` (which lives in the
//! `openagents` repo) on top of the merged P1/P2 substrate
//! (`Cs336A1TransformerLm::forward_with_hidden` and
//! [`psionic_models::CoordinatorHead`]):
//!
//! - **P3 — separable CMA-ES optimizer** ([`SepCmaEs`]). A gradient-free
//!   trainer that samples a population of perturbed `CoordinatorHead` parameter
//!   vectors (via [`CoordinatorHead::flatten_parameters`] /
//!   [`CoordinatorHead::with_flat_parameters`]), evaluates each via the P4
//!   fitness hook, recombines by fitness-weighted mean, and updates a diagonal
//!   (separable) covariance. A [`RandomSearch`] baseline shares the same
//!   evaluation surface — the paper's control and a cheap sanity gate before
//!   trusting ES.
//! - **P4 — scalar terminal-reward adapter** ([`TerminalRewardAdapter`]) plus
//!   the atomic-evaluation hook [`CoordinatorFitness::evaluate_coordinator`],
//!   `params -> f32`. The reward is a verification *verdict*
//!   ([`VerificationVerdict`]) mapped to `verified ? 1.0 : 0.0`, with optional
//!   cost-aware shaping `reward - lambda * cost`. This mirrors
//!   `docs/sakana/coordinator-as-verified-work.md`: ACCEPT is the
//!   replay-validator verdict, never a learned head output.
//! - **P5 — typed, capability-filtered worker-pool binding**
//!   ([`WorkerPoolBinding`]). The head's `L` worker logits map onto a stable,
//!   ordered list of eligible workers, derived from a candidate set and
//!   **filtered by a receipted capability envelope** before the coordinator
//!   ever sees it. The coordinator selects *within* the eligible set; it never
//!   overrides the receipt gate.
//!
//! ## What is real vs. what is a smoke
//!
//! The optimizer, reward adapter, pool binding, and their tests are real,
//! deterministic Rust. The *fitness function* used by the CPU unit tests and
//! the local smoke is a **toy / fixture** fitness (a documented closure or a
//! fixture verdict), NOT a frontier ML result. A real coordinator training run
//! needs the live `forward_with_hidden` feature, the live worker pool, and the
//! Tassadar verdict on a budgeted batch — see
//! `docs/COORDINATOR_EVOLUTION_TRAINING.md` and the GCloud job spec under
//! `scripts/`. The CPU smoke proves the optimizer *improves a fitness*, not
//! that the coordinator is good.
//!
//! No autograd dependency: sep-CMA-ES is gradient-free, so this whole lane is
//! CPU-friendly and self-contained.

use std::collections::BTreeSet;

use psionic_models::{CoordinatorHead, CoordinatorHeadError};
use serde::{Deserialize, Serialize};
use thiserror::Error;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors raised by the coordinator evolution lane.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum CoordinatorEvolutionError {
    /// A configuration value was invalid.
    #[error("invalid evolution configuration: {detail}")]
    InvalidConfiguration {
        /// Human-readable detail.
        detail: String,
    },
    /// The parameter vector dimension was zero or mismatched.
    #[error("invalid parameter dimension: {detail}")]
    InvalidDimension {
        /// Human-readable detail.
        detail: String,
    },
    /// The fitness hook returned a non-finite value.
    #[error("fitness returned a non-finite value for a population member")]
    NonFiniteFitness,
    /// The coordinator head rejected a parameter vector.
    #[error("coordinator head error: {0}")]
    Head(#[from] CoordinatorHeadError),
    /// The worker pool binding was empty after capability filtering.
    #[error("worker pool binding is empty: {detail}")]
    EmptyWorkerPool {
        /// Human-readable detail.
        detail: String,
    },
}

// ---------------------------------------------------------------------------
// Deterministic PRNG (splitmix64 + Box–Muller).
//
// The roadmap calls out that ES fitness must be reproducible. We use a small
// internal deterministic PRNG rather than pulling a `rand` dependency into this
// crate, so a seeded run is bit-for-bit repeatable for the smoke and for tests.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `[0, 1)`.
    fn next_f64(&mut self) -> f64 {
        // 53-bit mantissa.
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// Standard-normal sample via Box–Muller.
    fn next_standard_normal(&mut self) -> f64 {
        // Avoid log(0).
        let u1 = self.next_f64().max(f64::MIN_POSITIVE);
        let u2 = self.next_f64();
        let r = (-2.0 * u1.ln()).sqrt();
        r * (std::f64::consts::TAU * u2).cos()
    }
}

// ---------------------------------------------------------------------------
// P4 — verification verdict + scalar terminal-reward adapter
// ---------------------------------------------------------------------------

/// A terminal verification verdict for one coordinated trajectory.
///
/// Per `docs/sakana/coordinator-as-verified-work.md`, the ACCEPT decision is
/// the replay-validator / verification-command verdict — a deterministic,
/// independently-recomputed result — not a prompted LLM judge. For the offline
/// smoke this is a fixture verdict; on the live lane it binds to the Tassadar
/// `training.verification_classes.v1` verdict.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationVerdict {
    /// The trajectory's output verified (replay digests matched / command
    /// passed). This is the same object that releases settlement.
    Verified,
    /// The trajectory's output failed verification.
    Rejected,
}

impl VerificationVerdict {
    /// `1.0` for `Verified`, `0.0` for `Rejected` — the clean-Bernoulli reward
    /// sep-CMA-ES is built for.
    #[must_use]
    pub const fn base_reward(self) -> f32 {
        match self {
            Self::Verified => 1.0,
            Self::Rejected => 0.0,
        }
    }
}

/// One atomic-evaluation outcome: a verdict plus the spend it incurred (sats or
/// abstract cost units). Spend is logged separately so fitness can be
/// `reward - lambda * cost` ("verified-work-per-sat" rather than raw pass
/// rate) without conflating the two signals.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrajectoryOutcome {
    /// The terminal verification verdict for the trajectory.
    pub verdict: VerificationVerdict,
    /// Spend incurred by the trajectory (same units as the adapter's cost
    /// coefficient denominator; e.g. sats). Must be finite and non-negative.
    pub cost: f32,
}

impl TrajectoryOutcome {
    /// A zero-cost verdict (the offline module-eval lane, where no workers move
    /// sats).
    #[must_use]
    pub const fn offline(verdict: VerificationVerdict) -> Self {
        Self {
            verdict,
            cost: 0.0,
        }
    }
}

/// Maps verification outcomes to a scalar fitness contribution.
///
/// `fitness(outcome) = base_reward(verdict) - cost_coefficient * cost`.
///
/// With `cost_coefficient = 0` this is the pure verified/rejected reward (the
/// offline lane). With `cost_coefficient > 0` it becomes the cost-aware
/// "verified-work-per-sat-spent" objective the verified-work doc calls the
/// actual business objective.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TerminalRewardAdapter {
    /// `lambda` in `reward - lambda * cost`. Zero on the offline lane.
    pub cost_coefficient: f32,
}

impl TerminalRewardAdapter {
    /// Pure verified/rejected reward, no cost shaping (offline lane).
    #[must_use]
    pub const fn offline() -> Self {
        Self {
            cost_coefficient: 0.0,
        }
    }

    /// Cost-aware reward (`reward - lambda * cost`).
    #[must_use]
    pub const fn cost_aware(cost_coefficient: f32) -> Self {
        Self { cost_coefficient }
    }

    /// Scalar fitness contribution for one trajectory outcome.
    #[must_use]
    pub fn scalar(&self, outcome: TrajectoryOutcome) -> f32 {
        outcome.verdict.base_reward() - self.cost_coefficient * outcome.cost
    }

    /// Mean scalar reward over a batch of trajectory outcomes (the
    /// `evaluate_coordinator` aggregate). Returns `0.0` for an empty batch.
    #[must_use]
    pub fn mean_scalar(&self, outcomes: &[TrajectoryOutcome]) -> f32 {
        if outcomes.is_empty() {
            return 0.0;
        }
        let sum: f32 = outcomes.iter().map(|outcome| self.scalar(*outcome)).sum();
        sum / outcomes.len() as f32
    }
}

// ---------------------------------------------------------------------------
// P5 — typed, capability-filtered worker-pool binding
// ---------------------------------------------------------------------------

/// One worker the coordinator's `L` worker logits can index.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerPoolMember {
    /// Stable worker identifier (endpoint id / node id / model id).
    pub worker_id: String,
    /// Whether this is an open-network worker or a frontier-LLM endpoint —
    /// frontier endpoints are first-class pool members per the roadmap.
    pub kind: WorkerKind,
    /// The capabilities this worker has a *receipted* envelope for. The
    /// coordinator may only select a worker whose receipted capabilities cover
    /// the trajectory's required capability.
    pub receipted_capabilities: BTreeSet<String>,
}

/// Whether a pool member is an open-network worker or a frontier endpoint.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerKind {
    /// Open-network / contributor worker.
    Open,
    /// Frontier-LLM endpoint (first-class pool member).
    Frontier,
}

/// A stable, ordered list of the `L` eligible workers the head's worker logits
/// map onto, after capability filtering.
///
/// Construction is the capability gate: a candidate set is filtered down to the
/// workers whose receipted capabilities cover the required capability, then
/// sorted into a stable order (by `worker_id`). The coordinator selects an
/// index into this filtered list; it can never name a worker outside the
/// receipt-eligible set, so the head cannot override the receipt gate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerPoolBinding {
    workers: Vec<WorkerPoolMember>,
    required_capability: String,
}

impl WorkerPoolBinding {
    /// Builds a binding from a candidate set, filtering to workers whose
    /// receipted capability envelope covers `required_capability`, then sorting
    /// by `worker_id` for a stable logit-to-worker mapping.
    pub fn from_candidates(
        candidates: impl IntoIterator<Item = WorkerPoolMember>,
        required_capability: impl Into<String>,
    ) -> Result<Self, CoordinatorEvolutionError> {
        let required_capability = required_capability.into();
        let mut workers: Vec<WorkerPoolMember> = candidates
            .into_iter()
            .filter(|worker| worker.receipted_capabilities.contains(&required_capability))
            .collect();
        workers.sort_by(|a, b| a.worker_id.cmp(&b.worker_id));
        // De-dup by worker_id (stable: first wins after sort).
        workers.dedup_by(|a, b| a.worker_id == b.worker_id);
        if workers.is_empty() {
            return Err(CoordinatorEvolutionError::EmptyWorkerPool {
                detail: format!(
                    "no candidate worker has a receipted envelope for capability `{required_capability}`"
                ),
            });
        }
        Ok(Self {
            workers,
            required_capability,
        })
    }

    /// Number of eligible workers `L` — this is the `num_workers` the
    /// coordinator head must be configured with.
    #[must_use]
    pub fn len(&self) -> usize {
        self.workers.len()
    }

    /// Whether the binding is empty (it never is post-construction, but the
    /// API contract benefits from the explicit method).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.workers.is_empty()
    }

    /// The capability all eligible workers are receipted for.
    #[must_use]
    pub fn required_capability(&self) -> &str {
        &self.required_capability
    }

    /// Resolves a coordinator worker logit index to a pool member. Returns
    /// `None` if the index is out of range (e.g. a head sized for a larger
    /// pool than the capability-filtered binding).
    #[must_use]
    pub fn resolve(&self, worker_index: usize) -> Option<&WorkerPoolMember> {
        self.workers.get(worker_index)
    }

    /// The ordered eligible workers.
    #[must_use]
    pub fn workers(&self) -> &[WorkerPoolMember] {
        &self.workers
    }
}

// ---------------------------------------------------------------------------
// P4 — atomic evaluation hook
// ---------------------------------------------------------------------------

/// The fitness hook P3 calls: load the head with `params`, run the coordinated
/// trajectory batch end-to-end, and return mean scalar reward.
///
/// This is the seam where the offline (fixture-verdict) and live (Tassadar
/// verdict over a budgeted worker batch) lanes plug in. The smoke and the unit
/// tests use a closure / fixture implementation; the GCloud job binds a live
/// implementation that drives the existing rollout coordinator.
pub trait CoordinatorFitness {
    /// Evaluate one flat coordinator-head parameter vector and return its mean
    /// scalar reward. Implementations must be deterministic given the same
    /// params (required for stable ES fitness).
    fn evaluate_coordinator(&self, params: &[f32]) -> Result<f32, CoordinatorEvolutionError>;
}

/// A closure-backed fitness, convenient for the CPU smoke and tests.
pub struct ClosureFitness<F>
where
    F: Fn(&[f32]) -> Result<f32, CoordinatorEvolutionError>,
{
    evaluate: F,
}

impl<F> ClosureFitness<F>
where
    F: Fn(&[f32]) -> Result<f32, CoordinatorEvolutionError>,
{
    /// Wraps a closure as a [`CoordinatorFitness`].
    pub fn new(evaluate: F) -> Self {
        Self { evaluate }
    }
}

impl<F> CoordinatorFitness for ClosureFitness<F>
where
    F: Fn(&[f32]) -> Result<f32, CoordinatorEvolutionError>,
{
    fn evaluate_coordinator(&self, params: &[f32]) -> Result<f32, CoordinatorEvolutionError> {
        (self.evaluate)(params)
    }
}

/// A fixture atomic-evaluation that materializes the head with `params` and
/// runs a caller-supplied verdict-and-cost function per batch item, then
/// aggregates with a [`TerminalRewardAdapter`]. This is the offline lane used
/// by the smoke: no live workers, deterministic fixture verdicts, but the head
/// is really instantiated so the parameter seam is exercised end-to-end.
pub struct FixtureCoordinatorEval<V>
where
    V: Fn(&CoordinatorHead) -> Vec<TrajectoryOutcome>,
{
    seed_head: CoordinatorHead,
    reward: TerminalRewardAdapter,
    verdicts: V,
}

impl<V> FixtureCoordinatorEval<V>
where
    V: Fn(&CoordinatorHead) -> Vec<TrajectoryOutcome>,
{
    /// Builds a fixture evaluation around a seed head (its config is reused to
    /// rebuild the head from each parameter vector), a reward adapter, and a
    /// verdict function that maps a materialized head to a batch of outcomes.
    pub fn new(seed_head: CoordinatorHead, reward: TerminalRewardAdapter, verdicts: V) -> Self {
        Self {
            seed_head,
            reward,
            verdicts,
        }
    }
}

impl<V> CoordinatorFitness for FixtureCoordinatorEval<V>
where
    V: Fn(&CoordinatorHead) -> Vec<TrajectoryOutcome>,
{
    fn evaluate_coordinator(&self, params: &[f32]) -> Result<f32, CoordinatorEvolutionError> {
        let head = self.seed_head.with_flat_parameters(params.to_vec())?;
        let outcomes = (self.verdicts)(&head);
        Ok(self.reward.mean_scalar(&outcomes))
    }
}

// ---------------------------------------------------------------------------
// P3 — separable CMA-ES + random-search baseline
// ---------------------------------------------------------------------------

/// Configuration for the separable CMA-ES optimizer.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SepCmaEsConfig {
    /// Problem dimension (the head's flat parameter count).
    pub dimension: usize,
    /// Population size `lambda` (number of samples per generation).
    pub population_size: usize,
    /// Number of generations to run.
    pub generations: usize,
    /// Initial global step size `sigma`.
    pub initial_sigma: f64,
    /// Deterministic seed (reproducible fitness per the roadmap).
    pub seed: u64,
}

impl SepCmaEsConfig {
    /// A small, CPU-friendly default for a given dimension.
    #[must_use]
    pub fn smoke(dimension: usize) -> Self {
        Self {
            dimension,
            population_size: 16,
            generations: 40,
            initial_sigma: 0.5,
            seed: 0xC007_D1A6,
        }
    }

    fn validate(&self) -> Result<(), CoordinatorEvolutionError> {
        if self.dimension == 0 {
            return Err(CoordinatorEvolutionError::InvalidDimension {
                detail: String::from("dimension must be non-zero"),
            });
        }
        if self.population_size < 2 {
            return Err(CoordinatorEvolutionError::InvalidConfiguration {
                detail: String::from("population_size must be >= 2"),
            });
        }
        if self.generations == 0 {
            return Err(CoordinatorEvolutionError::InvalidConfiguration {
                detail: String::from("generations must be non-zero"),
            });
        }
        if !(self.initial_sigma.is_finite() && self.initial_sigma > 0.0) {
            return Err(CoordinatorEvolutionError::InvalidConfiguration {
                detail: String::from("initial_sigma must be finite and positive"),
            });
        }
        Ok(())
    }
}

/// The result of an evolution run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EvolutionOutcome {
    /// Best parameter vector found (maximizing fitness).
    pub best_parameters: Vec<f32>,
    /// Best fitness value found.
    pub best_fitness: f32,
    /// Fitness of the initial mean at generation 0 (the improvement baseline).
    pub initial_fitness: f32,
    /// Best fitness per generation (length == generations), for plotting /
    /// monotonicity assertions.
    pub fitness_history: Vec<f32>,
    /// Total number of fitness evaluations consumed (the budget actually
    /// spent — every eval may run a real worker on the live lane).
    pub evaluations: usize,
}

impl EvolutionOutcome {
    /// Whether the run improved over its starting point.
    #[must_use]
    pub fn improved(&self) -> bool {
        self.best_fitness > self.initial_fitness
    }
}

/// Separable (diagonal-covariance) CMA-ES.
///
/// This is a deliberately compact, dependency-free sep-CMA-ES that maximizes
/// fitness. It maintains a mean `m`, a per-coordinate standard deviation
/// `c` (the diagonal of the covariance), and a global step size `sigma`. Each
/// generation it samples `lambda` candidates `m + sigma * c .* z`, evaluates
/// them, recombines the best `mu` by rank weights, and updates `m`, `c`, and
/// `sigma`. The recombination/adaptation follows the standard sep-CMA-ES
/// scheme (Ros & Hansen, 2008) reduced to the parts that matter at our scale.
#[derive(Clone, Debug)]
pub struct SepCmaEs {
    config: SepCmaEsConfig,
}

impl SepCmaEs {
    /// Builds an optimizer from config.
    pub fn new(config: SepCmaEsConfig) -> Result<Self, CoordinatorEvolutionError> {
        config.validate()?;
        Ok(Self { config })
    }

    /// Runs the optimizer against a fitness hook, seeding the mean at
    /// `initial_parameters` (e.g. a zero head). Maximizes fitness.
    pub fn optimize<F: CoordinatorFitness>(
        &self,
        fitness: &F,
        initial_parameters: &[f32],
    ) -> Result<EvolutionOutcome, CoordinatorEvolutionError> {
        let n = self.config.dimension;
        if initial_parameters.len() != n {
            return Err(CoordinatorEvolutionError::InvalidDimension {
                detail: format!(
                    "initial_parameters has {} entries, expected dimension {}",
                    initial_parameters.len(),
                    n
                ),
            });
        }

        let lambda = self.config.population_size;
        let mu = lambda / 2; // parents = half the population.

        // Rank-based recombination weights (log-decreasing), normalized.
        let weights = recombination_weights(mu);
        let mu_eff = mu_effective(&weights);

        // Learning rates (sep-CMA-ES scaling; cc/c1/cmu damped by 1/N as in
        // the separable variant).
        let n_f = n as f64;
        let cc = 4.0 / (n_f + 4.0);
        let cs = (mu_eff + 2.0) / (n_f + mu_eff + 3.0);
        let c1 = 2.0 / ((n_f + 1.3).powi(2) + mu_eff);
        let cmu_raw = 2.0 * (mu_eff - 2.0 + 1.0 / mu_eff) / ((n_f + 2.0).powi(2) + mu_eff);
        let cmu = cmu_raw.max(0.0).min(1.0 - c1);
        // Separable speedup factor (Ros & Hansen): scale c1/cmu by (N+2)/3.
        let sep_factor = (n_f + 2.0) / 3.0;
        let c1 = (c1 * sep_factor).min(1.0);
        let cmu = (cmu * sep_factor).min(1.0 - c1);
        let damps = 1.0 + 2.0 * ((mu_eff - 1.0) / (n_f + 1.0)).sqrt().max(0.0) + cs;
        let chi_n =
            n_f.sqrt() * (1.0 - 1.0 / (4.0 * n_f) + 1.0 / (21.0 * n_f * n_f));

        let mut mean: Vec<f64> = initial_parameters.iter().map(|&v| v as f64).collect();
        // Diagonal of C (variances), start at 1.
        let mut c_diag: Vec<f64> = vec![1.0; n];
        // Evolution paths.
        let mut p_sigma: Vec<f64> = vec![0.0; n];
        let mut p_c: Vec<f64> = vec![0.0; n];
        let mut sigma = self.config.initial_sigma;

        let mut rng = SplitMix64::new(self.config.seed);

        let initial_fitness = fitness.evaluate_coordinator(initial_parameters)?;
        ensure_finite(initial_fitness)?;
        let mut best_parameters = initial_parameters.to_vec();
        let mut best_fitness = initial_fitness;
        let mut evaluations = 1_usize;
        let mut fitness_history = Vec::with_capacity(self.config.generations);

        for _generation in 0..self.config.generations {
            // 1. Sample lambda candidates.
            let mut samples: Vec<(f64, Vec<f64>, Vec<f64>)> = Vec::with_capacity(lambda);
            for _ in 0..lambda {
                let mut z = vec![0.0_f64; n];
                let mut candidate = vec![0.0_f64; n];
                for i in 0..n {
                    z[i] = rng.next_standard_normal();
                    // x = m + sigma * sqrt(c_i) * z_i (separable: diagonal std).
                    candidate[i] = mean[i] + sigma * c_diag[i].sqrt() * z[i];
                }
                let candidate_f32: Vec<f32> = candidate.iter().map(|&v| v as f32).collect();
                let f = fitness.evaluate_coordinator(&candidate_f32)?;
                ensure_finite(f)?;
                evaluations += 1;
                samples.push((f as f64, candidate, z));

                if f > best_fitness {
                    best_fitness = f;
                    best_parameters = candidate_f32;
                }
            }

            // 2. Sort by fitness, descending (maximize).
            samples.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

            // 3. Recombine the best mu into the new mean and weighted z.
            let old_mean = mean.clone();
            let mut new_mean = vec![0.0_f64; n];
            let mut weighted_z = vec![0.0_f64; n];
            for (k, weight) in weights.iter().enumerate() {
                let (_, ref cand, ref z) = samples[k];
                for i in 0..n {
                    new_mean[i] += weight * cand[i];
                    weighted_z[i] += weight * z[i];
                }
            }
            mean = new_mean;

            // 4. Update evolution paths.
            // p_sigma uses the isotropic step (weighted_z); separable variant
            // keeps it diagonal.
            let cs_factor = (cs * (2.0 - cs) * mu_eff).sqrt();
            for i in 0..n {
                p_sigma[i] = (1.0 - cs) * p_sigma[i] + cs_factor * weighted_z[i];
            }
            let p_sigma_norm = l2_norm(&p_sigma);

            let cc_factor = (cc * (2.0 - cc) * mu_eff).sqrt();
            for i in 0..n {
                // Step the mean actually moved, in units of sigma*sqrt(c).
                let mean_step = if sigma * c_diag[i].sqrt() > f64::MIN_POSITIVE {
                    (mean[i] - old_mean[i]) / (sigma * c_diag[i].sqrt())
                } else {
                    0.0
                };
                p_c[i] = (1.0 - cc) * p_c[i] + cc_factor * mean_step;
            }

            // 5. Update the diagonal covariance.
            for i in 0..n {
                // Rank-one term from p_c, rank-mu term from the selected zs.
                let mut rank_mu = 0.0_f64;
                for (k, weight) in weights.iter().enumerate() {
                    let z_i = samples[k].2[i];
                    rank_mu += weight * z_i * z_i;
                }
                c_diag[i] = (1.0 - c1 - cmu) * c_diag[i]
                    + c1 * p_c[i] * p_c[i]
                    + cmu * rank_mu;
                // Guard against collapse / explosion.
                c_diag[i] = c_diag[i].clamp(1e-12, 1e12);
            }

            // 6. Update the global step size.
            sigma *= ((cs / damps) * (p_sigma_norm / chi_n - 1.0)).exp();
            sigma = sigma.clamp(1e-12, 1e12);

            fitness_history.push(best_fitness);
        }

        Ok(EvolutionOutcome {
            best_parameters,
            best_fitness,
            initial_fitness,
            fitness_history,
            evaluations,
        })
    }
}

/// Random-search baseline over the same fitness surface — the paper's control
/// and a cheap sanity gate before trusting ES. Samples isotropic Gaussian
/// candidates around the initial mean at a fixed scale and keeps the best.
#[derive(Clone, Debug)]
pub struct RandomSearch {
    /// Problem dimension.
    pub dimension: usize,
    /// Number of samples to draw (the eval budget).
    pub samples: usize,
    /// Gaussian sampling scale around the initial mean.
    pub scale: f64,
    /// Deterministic seed.
    pub seed: u64,
}

impl RandomSearch {
    /// Runs random search against a fitness hook, maximizing fitness.
    pub fn optimize<F: CoordinatorFitness>(
        &self,
        fitness: &F,
        initial_parameters: &[f32],
    ) -> Result<EvolutionOutcome, CoordinatorEvolutionError> {
        let n = self.dimension;
        if n == 0 {
            return Err(CoordinatorEvolutionError::InvalidDimension {
                detail: String::from("dimension must be non-zero"),
            });
        }
        if initial_parameters.len() != n {
            return Err(CoordinatorEvolutionError::InvalidDimension {
                detail: format!(
                    "initial_parameters has {} entries, expected dimension {}",
                    initial_parameters.len(),
                    n
                ),
            });
        }
        let mut rng = SplitMix64::new(self.seed);
        let initial_fitness = fitness.evaluate_coordinator(initial_parameters)?;
        ensure_finite(initial_fitness)?;
        let mut best_parameters = initial_parameters.to_vec();
        let mut best_fitness = initial_fitness;
        let mut fitness_history = Vec::with_capacity(self.samples);
        let mut evaluations = 1_usize;

        for _ in 0..self.samples {
            let candidate: Vec<f32> = (0..n)
                .map(|i| {
                    (initial_parameters[i] as f64 + self.scale * rng.next_standard_normal()) as f32
                })
                .collect();
            let f = fitness.evaluate_coordinator(&candidate)?;
            ensure_finite(f)?;
            evaluations += 1;
            if f > best_fitness {
                best_fitness = f;
                best_parameters = candidate;
            }
            fitness_history.push(best_fitness);
        }

        Ok(EvolutionOutcome {
            best_parameters,
            best_fitness,
            initial_fitness,
            fitness_history,
            evaluations,
        })
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn ensure_finite(value: f32) -> Result<(), CoordinatorEvolutionError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(CoordinatorEvolutionError::NonFiniteFitness)
    }
}

fn l2_norm(values: &[f64]) -> f64 {
    values.iter().map(|&v| v * v).sum::<f64>().sqrt()
}

/// Log-decreasing rank weights for the best `mu` parents, normalized to sum 1.
fn recombination_weights(mu: usize) -> Vec<f64> {
    let mu = mu.max(1);
    let raw: Vec<f64> = (0..mu)
        .map(|k| ((mu as f64 + 0.5).ln()) - ((k + 1) as f64).ln())
        .collect();
    let sum: f64 = raw.iter().sum();
    if sum <= 0.0 {
        // Degenerate (mu == 1): single full-weight parent.
        return vec![1.0; mu];
    }
    raw.into_iter().map(|w| w / sum).collect()
}

/// Effective selection mass `mu_eff = 1 / sum(w_i^2)`.
fn mu_effective(weights: &[f64]) -> f64 {
    let denom: f64 = weights.iter().map(|&w| w * w).sum();
    if denom <= 0.0 {
        1.0
    } else {
        1.0 / denom
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use psionic_models::{CoordinatorHead, CoordinatorHeadConfig};

    // ---- P4 reward adapter -------------------------------------------------

    #[test]
    fn verdict_maps_to_bernoulli_reward() {
        assert_eq!(VerificationVerdict::Verified.base_reward(), 1.0);
        assert_eq!(VerificationVerdict::Rejected.base_reward(), 0.0);
    }

    #[test]
    fn offline_adapter_is_pure_pass_rate() {
        let adapter = TerminalRewardAdapter::offline();
        let outcomes = vec![
            TrajectoryOutcome::offline(VerificationVerdict::Verified),
            TrajectoryOutcome::offline(VerificationVerdict::Rejected),
            TrajectoryOutcome::offline(VerificationVerdict::Verified),
            TrajectoryOutcome::offline(VerificationVerdict::Verified),
        ];
        // 3 of 4 verified.
        assert!((adapter.mean_scalar(&outcomes) - 0.75).abs() < 1e-6);
    }

    #[test]
    fn cost_aware_adapter_penalizes_spend() {
        let adapter = TerminalRewardAdapter::cost_aware(0.001);
        let cheap = TrajectoryOutcome {
            verdict: VerificationVerdict::Verified,
            cost: 100.0,
        };
        let pricey = TrajectoryOutcome {
            verdict: VerificationVerdict::Verified,
            cost: 800.0,
        };
        // Both verified; the cheaper one scores higher (verified-work-per-sat).
        assert!(adapter.scalar(cheap) > adapter.scalar(pricey));
        assert!((adapter.scalar(cheap) - (1.0 - 0.1)).abs() < 1e-6);
    }

    #[test]
    fn empty_batch_is_zero_reward() {
        assert_eq!(TerminalRewardAdapter::offline().mean_scalar(&[]), 0.0);
    }

    // ---- P5 worker-pool binding -------------------------------------------

    fn member(id: &str, kind: WorkerKind, caps: &[&str]) -> WorkerPoolMember {
        WorkerPoolMember {
            worker_id: id.to_string(),
            kind,
            receipted_capabilities: caps.iter().map(|c| c.to_string()).collect(),
        }
    }

    #[test]
    fn binding_filters_by_receipted_capability_and_orders_stably() {
        let candidates = vec![
            member("zeta", WorkerKind::Open, &["rust_build"]),
            member("alpha", WorkerKind::Frontier, &["rust_build", "python"]),
            member("mid", WorkerKind::Open, &["python"]), // filtered out.
        ];
        let binding = WorkerPoolBinding::from_candidates(candidates, "rust_build").expect("binding");
        // Only alpha + zeta have rust_build; sorted by id.
        assert_eq!(binding.len(), 2);
        assert_eq!(binding.resolve(0).expect("0").worker_id, "alpha");
        assert_eq!(binding.resolve(1).expect("1").worker_id, "zeta");
        assert!(binding.resolve(2).is_none());
        assert_eq!(binding.required_capability(), "rust_build");
    }

    #[test]
    fn binding_dedups_by_worker_id() {
        let candidates = vec![
            member("dup", WorkerKind::Open, &["cap"]),
            member("dup", WorkerKind::Frontier, &["cap"]),
        ];
        let binding = WorkerPoolBinding::from_candidates(candidates, "cap").expect("binding");
        assert_eq!(binding.len(), 1);
    }

    #[test]
    fn binding_rejects_when_no_worker_is_eligible() {
        let candidates = vec![member("a", WorkerKind::Open, &["other"])];
        let error = WorkerPoolBinding::from_candidates(candidates, "needed").unwrap_err();
        assert!(matches!(
            error,
            CoordinatorEvolutionError::EmptyWorkerPool { .. }
        ));
    }

    // ---- P3 optimizer: toy fitness improvement (CPU smoke) -----------------

    /// Toy fitness: negative squared distance to a fixed target vector. The
    /// global maximum is 0 at `params == target`. This is a *smoke* fitness,
    /// not an ML result — it proves the optimizer drives parameters toward a
    /// better fitness.
    fn toy_fitness(target: Vec<f32>) -> ClosureFitness<impl Fn(&[f32]) -> Result<f32, CoordinatorEvolutionError>> {
        ClosureFitness::new(move |params: &[f32]| {
            let sq: f32 = params
                .iter()
                .zip(target.iter())
                .map(|(p, t)| {
                    let d = p - t;
                    d * d
                })
                .sum();
            Ok(-sq)
        })
    }

    #[test]
    fn sep_cma_es_improves_toy_fitness() {
        let target = vec![1.0_f32, -2.0, 0.5, 3.0];
        let fitness = toy_fitness(target.clone());
        let config = SepCmaEsConfig {
            dimension: target.len(),
            population_size: 20,
            generations: 80,
            initial_sigma: 1.0,
            seed: 42,
        };
        let optimizer = SepCmaEs::new(config).expect("optimizer");
        let initial = vec![0.0_f32; target.len()];
        let outcome = optimizer.optimize(&fitness, &initial).expect("optimize");

        // It must improve over the zero start, and get meaningfully close.
        assert!(outcome.improved(), "expected ES to improve over the start");
        assert!(
            outcome.best_fitness > -0.05,
            "expected near-optimal fitness, got {}",
            outcome.best_fitness
        );
        // Fitness history must be monotonically non-decreasing (best-so-far).
        for window in outcome.fitness_history.windows(2) {
            assert!(window[1] >= window[0] - 1e-6);
        }
        // Best params should be close to target.
        for (p, t) in outcome.best_parameters.iter().zip(target.iter()) {
            assert!((p - t).abs() < 0.2, "param {p} not close to target {t}");
        }
    }

    #[test]
    fn sep_cma_es_is_deterministic_given_seed() {
        let target = vec![0.3_f32, -0.7, 1.2];
        let config = SepCmaEsConfig {
            dimension: target.len(),
            population_size: 12,
            generations: 30,
            initial_sigma: 0.8,
            seed: 7,
        };
        let optimizer = SepCmaEs::new(config).expect("optimizer");
        let initial = vec![0.0_f32; target.len()];
        let a = optimizer
            .optimize(&toy_fitness(target.clone()), &initial)
            .expect("a");
        let b = optimizer
            .optimize(&toy_fitness(target.clone()), &initial)
            .expect("b");
        assert_eq!(a.best_fitness, b.best_fitness);
        assert_eq!(a.best_parameters, b.best_parameters);
        assert_eq!(a.evaluations, b.evaluations);
    }

    #[test]
    fn sep_cma_es_beats_random_search_on_toy() {
        // Under a tiny eval budget, ES should reach at least as good a fitness
        // as random search (the paper's control). Equal budgets.
        let target = vec![1.0_f32, -1.0, 2.0, -2.0, 0.5];
        let n = target.len();
        let initial = vec![0.0_f32; n];

        let es = SepCmaEs::new(SepCmaEsConfig {
            dimension: n,
            population_size: 16,
            generations: 30,
            initial_sigma: 1.0,
            seed: 99,
        })
        .expect("es");
        let es_out = es.optimize(&toy_fitness(target.clone()), &initial).expect("es");

        let rs = RandomSearch {
            dimension: n,
            // Match ES budget: ~ population_size * generations.
            samples: 16 * 30,
            scale: 1.0,
            seed: 99,
        };
        let rs_out = rs.optimize(&toy_fitness(target.clone()), &initial).expect("rs");

        assert!(
            es_out.best_fitness >= rs_out.best_fitness,
            "ES ({}) should be >= random search ({}) at equal budget",
            es_out.best_fitness,
            rs_out.best_fitness
        );
        assert!(es_out.improved());
        assert!(rs_out.improved());
    }

    // ---- P3 x P2: optimize a real CoordinatorHead via the fixture eval ------

    #[test]
    fn sep_cma_es_improves_real_head_via_fixture_eval() {
        // A real CoordinatorHead is materialized for each candidate. The
        // fixture verdict prefers a head whose worker-0 logit dominates for a
        // probe hidden state, so fitness rises as the head learns to route to
        // worker 0. This exercises the full P2<->P3<->P4 seam on CPU.
        use psionic_core::Shape;
        use psionic_nn::NnTensor;

        let config = CoordinatorHeadConfig {
            hidden_dim: 4,
            num_workers: 3,
            num_roles: 3,
        };
        let seed_head = CoordinatorHead::zeros(config).expect("seed head");
        let reward = TerminalRewardAdapter::offline();

        // Verdict: Verified iff the head routes a fixed probe hidden state to
        // worker index 0 (the "correct" worker for this fixture batch).
        let eval = FixtureCoordinatorEval::new(seed_head.clone(), reward, |head: &CoordinatorHead| {
            let probe = NnTensor::f32(Shape::new(vec![1, 4]), vec![1.0, 0.5, -0.5, 0.25])
                .expect("probe");
            let decisions = head.decide(&probe).expect("decide");
            let verdict = if decisions[0].worker_index == 0 {
                VerificationVerdict::Verified
            } else {
                VerificationVerdict::Rejected
            };
            vec![TrajectoryOutcome::offline(verdict)]
        });

        let dimension = config.parameter_count();
        let optimizer = SepCmaEs::new(SepCmaEsConfig {
            dimension,
            population_size: 24,
            generations: 60,
            initial_sigma: 0.5,
            seed: 2026,
        })
        .expect("optimizer");

        let initial = seed_head.flatten_parameters().expect("flat");
        let outcome = optimizer.optimize(&eval, &initial).expect("optimize");

        // The zero head ties on worker logits (argmax -> index 0 by tie-break),
        // so the start may already verify; the key claim is the optimizer never
        // regresses and can reach a verified (1.0) head.
        assert!(outcome.best_fitness >= outcome.initial_fitness - 1e-6);
        assert!(
            (outcome.best_fitness - 1.0).abs() < 1e-6,
            "expected a verified head (fitness 1.0), got {}",
            outcome.best_fitness
        );

        // Confirm the best head actually routes the probe to worker 0.
        let best_head = seed_head
            .with_flat_parameters(outcome.best_parameters.clone())
            .expect("best head");
        let probe = NnTensor::f32(Shape::new(vec![1, 4]), vec![1.0, 0.5, -0.5, 0.25])
            .expect("probe");
        let decisions = best_head.decide(&probe).expect("decide");
        assert_eq!(decisions[0].worker_index, 0);
    }
}
