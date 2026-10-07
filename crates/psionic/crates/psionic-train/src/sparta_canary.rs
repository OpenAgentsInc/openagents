//! SPARTA sparse-averaging canary harness (psionic#1127, Pluralis roadmap P2.4).
//!
//! W3 standing order (openagents Tassadar research plan): no public gradients
//! into the main optimizer, ever. Sparse-averaging decentralized training is a
//! side experiment with canary evals and pre-registered kill bounds, never the
//! main run. This module is that side experiment's harness, run by the book:
//! harness before claim.
//!
//! The harness owns four surfaces:
//!
//! 1. A pre-registration contract: a typed, digest-pinned record written
//!    before any run — the grid (starting from the Pluralis published tuning:
//!    sparsity fraction 0.05, average state every 5 steps, random rotating
//!    partitions), the synchronous-averaging baseline, the eval schema
//!    (first-divergence step plus held-out eval families, never perplexity
//!    alone), and the pre-registered kill bound. Config, not constants.
//! 2. SPARTA partition mechanics owned in Rust, adapted from the Pluralis
//!    AsyncMesh `PartitionedIndexSelector` (read-only reference lane
//!    `projects/pluralis/repos/AsyncMesh`, `sparta/sparta.py`): partition each
//!    tensor's indices into `ceil(1/p)` random partitions, average exactly one
//!    rotating partition per averaging round, complete coverage over a full
//!    rotation, then re-randomize. Deterministic through a caller-passed seed
//!    and a stateless counter-based splitmix64 mix (the house seeded-
//!    determinism pattern; see `cs336_a4_data_refinery.rs`).
//! 3. A bounded, deterministic two-arm comparison runner: in-process replicas
//!    with synthetic parameter drift, sparse-averaging vs synchronous-
//!    averaging side by side at toy scale, producing per-round divergence
//!    metrics, rounds-to-coverage, and simulated bytes communicated per arm.
//!    No networking, no live training; every value is labeled synthetic.
//! 4. A typed canary outcome record (held / killed / pending plus which
//!    pre-registered bound decided it), designed to be written back to the
//!    derisking ledger entry (`docs/PSION_DERISKING_LEDGER.md`, entry 1).
//!
//! This module makes no claim about real convergence behavior. The R1/R2-scale
//! canary is gated and not claimed; the committed fixture demonstrates only
//! that the harness works end to end at toy scale.

use std::{collections::BTreeMap, fs, path::Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const SPARTA_CANARY_SCHEMA_VERSION: &str = "psionic.train.sparta_canary.v1";
pub const SPARTA_CANARY_CONTRACT_ID: &str = "psionic.train.sparta_canary.v1";
pub const SPARTA_CANARY_PREREGISTRATION_SCHEMA_VERSION: &str =
    "psionic.train.sparta_canary_preregistration.v1";
pub const SPARTA_CANARY_OUTCOME_SCHEMA_VERSION: &str = "psionic.train.sparta_canary_outcome.v1";
pub const SPARTA_CANARY_FIXTURE_PATH: &str = "fixtures/training/sparta_canary_v1.json";
pub const SPARTA_CANARY_CHECK_SCRIPT_PATH: &str = "scripts/check-sparta-canary.sh";
pub const SPARTA_CANARY_DOC_PATH: &str = "docs/PSION_SPARTA_CANARY.md";
pub const SPARTA_CANARY_DERISKING_LEDGER_PATH: &str = "docs/PSION_DERISKING_LEDGER.md";
pub const SPARTA_CANARY_DERISKING_LEDGER_ENTRY: &str = "SPARTA-class sparse parameter averaging";
pub const SPARTA_CANARY_TRAIN_SYSTEM_DOC_PATH: &str = "docs/TRAIN_SYSTEM.md";

/// The W3 standing order every pre-registration must carry verbatim.
pub const SPARTA_CANARY_STANDING_ORDER: &str = "No public gradients into the main optimizer, \
ever. Sparse-averaging decentralized training runs only as a side experiment with canary evals \
and pre-registered kill bounds; it is never the main run.";

#[derive(Debug, Error)]
pub enum SpartaCanaryError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
    #[error("sparta partition selector input is invalid: {detail}")]
    InvalidPartitionInput { detail: String },
    #[error("sparta canary pre-registration is invalid: {detail}")]
    InvalidPreRegistration { detail: String },
    #[error("sparta canary simulation config is invalid: {detail}")]
    InvalidSimulationConfig { detail: String },
    #[error("sparta canary comparison artifact is invalid: {detail}")]
    InvalidComparisonArtifact { detail: String },
    #[error("sparta canary outcome record is invalid: {detail}")]
    InvalidOutcomeRecord { detail: String },
    #[error("sparta canary contract is invalid: {detail}")]
    InvalidContract { detail: String },
}

// ---------------------------------------------------------------------------
// Counter-based seeded determinism (house pattern: stateless splitmix64 mix).
// ---------------------------------------------------------------------------

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = value;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Stateless counter-based draw: the same `(seed, stream, counter)` triple
/// always yields the same value, independent of call order. The seed is
/// caller-passed; the harness never reads ambient entropy or wall clocks.
#[must_use]
pub fn sparta_counter_rng(seed: u64, stream: u64, counter: u64) -> u64 {
    splitmix64(seed ^ splitmix64(stream ^ splitmix64(counter)))
}

/// Uniform draw in `[0, 1)` from one counter-based value.
#[must_use]
pub fn sparta_counter_unit(seed: u64, stream: u64, counter: u64) -> f64 {
    (sparta_counter_rng(seed, stream, counter) >> 11) as f64 / (1_u64 << 53) as f64
}

fn tensor_name_stream(tensor_name: &str) -> u64 {
    let mut hash = 0xCBF2_9CE4_8422_2325_u64;
    for byte in tensor_name.as_bytes() {
        hash = splitmix64(hash ^ u64::from(*byte));
    }
    hash
}

// ---------------------------------------------------------------------------
// SPARTA partition mechanics (Pluralis AsyncMesh PartitionedIndexSelector).
// ---------------------------------------------------------------------------

/// Per-tensor rotating partition state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpartaTensorPartitionState {
    pub element_count: usize,
    /// How many full rotations have been re-randomized so far.
    pub rotation_epoch: u64,
    /// Next partition to serve within the current rotation.
    pub current_partition: u32,
    /// Number of partitions in the current rotation: `min(ceil(1/p), n)`.
    pub partition_count: u32,
    /// Element index -> partition id for the current rotation epoch.
    pub assignment: Vec<u32>,
}

/// Rotating partitioned index selector, the owned equivalent of the Pluralis
/// AsyncMesh `PartitionedIndexSelector` (`sparta/sparta.py`): each tensor's
/// indices are split into `ceil(1/p)` random partitions; each averaging round
/// consumes exactly one partition; a full rotation covers every index exactly
/// once, then the partitions re-randomize for the next rotation epoch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpartaPartitionedIndexSelector {
    pub seed: u64,
    pub sparsity_fraction: f64,
    pub state: BTreeMap<String, SpartaTensorPartitionState>,
}

impl SpartaPartitionedIndexSelector {
    /// Builds a selector. The seed is caller-passed: the same seed yields the
    /// same partition schedule for the same tensors.
    pub fn new(seed: u64, sparsity_fraction: f64) -> Result<Self, SpartaCanaryError> {
        validate_sparsity_fraction(sparsity_fraction)
            .map_err(|detail| SpartaCanaryError::InvalidPartitionInput { detail })?;
        Ok(Self {
            seed,
            sparsity_fraction,
            state: BTreeMap::new(),
        })
    }

    /// Returns the indices of the next rotating partition for `tensor_name`
    /// and advances the rotation. Mirrors the Pluralis `get_indices` flow:
    /// (re)randomize when the rotation is exhausted, serve the current
    /// partition, then advance the partition cursor.
    pub fn select_indices(
        &mut self,
        tensor_name: &str,
        element_count: usize,
    ) -> Result<Vec<usize>, SpartaCanaryError> {
        if element_count == 0 {
            return Err(SpartaCanaryError::InvalidPartitionInput {
                detail: format!("tensor `{tensor_name}` has zero elements"),
            });
        }
        let seed = self.seed;
        let sparsity_fraction = self.sparsity_fraction;
        let state = match self.state.get_mut(tensor_name) {
            Some(state) => {
                if state.element_count != element_count {
                    return Err(SpartaCanaryError::InvalidPartitionInput {
                        detail: format!(
                            "tensor `{tensor_name}` changed element count from {} to {element_count}",
                            state.element_count
                        ),
                    });
                }
                if state.current_partition >= state.partition_count {
                    let epoch = state.rotation_epoch + 1;
                    *state = build_tensor_partition_state(
                        seed,
                        tensor_name,
                        epoch,
                        element_count,
                        sparsity_fraction,
                    );
                }
                state
            }
            None => {
                let built = build_tensor_partition_state(
                    seed,
                    tensor_name,
                    0,
                    element_count,
                    sparsity_fraction,
                );
                self.state.insert(tensor_name.to_string(), built);
                self.state
                    .get_mut(tensor_name)
                    .expect("state was just inserted")
            }
        };
        let partition = state.current_partition;
        let indices: Vec<usize> = state
            .assignment
            .iter()
            .enumerate()
            .filter_map(|(index, assigned)| (*assigned == partition).then_some(index))
            .collect();
        state.current_partition += 1;
        Ok(indices)
    }

    /// Partitions in one full rotation for a tensor of `element_count`
    /// elements: `min(ceil(1/p), n)`.
    #[must_use]
    pub fn partitions_per_rotation(&self, element_count: usize) -> u32 {
        partition_count_for(self.sparsity_fraction, element_count)
    }
}

fn validate_sparsity_fraction(sparsity_fraction: f64) -> Result<(), String> {
    if !sparsity_fraction.is_finite() || sparsity_fraction <= 0.0 || sparsity_fraction > 1.0 {
        return Err(format!(
            "sparsity_fraction must be finite and in (0, 1], got {sparsity_fraction}"
        ));
    }
    Ok(())
}

fn partition_count_for(sparsity_fraction: f64, element_count: usize) -> u32 {
    let raw = (1.0 / sparsity_fraction).ceil() as usize;
    raw.min(element_count).max(1) as u32
}

/// Builds one rotation epoch's random partition assignment: a seeded
/// Fisher-Yates shuffle of the index range, then `shuffled position %
/// partition_count`, the owned analogue of the Pluralis
/// `rand(n).argsort() % num_partitions` assignment.
fn build_tensor_partition_state(
    seed: u64,
    tensor_name: &str,
    rotation_epoch: u64,
    element_count: usize,
    sparsity_fraction: f64,
) -> SpartaTensorPartitionState {
    let partition_count = partition_count_for(sparsity_fraction, element_count);
    let stream = tensor_name_stream(tensor_name) ^ splitmix64(rotation_epoch);
    let mut permutation: Vec<usize> = (0..element_count).collect();
    // Seeded Fisher-Yates with counter-based draws: deterministic in
    // (seed, tensor_name, rotation_epoch).
    for position in (1..element_count).rev() {
        let draw = sparta_counter_rng(seed, stream, position as u64);
        let swap_with = (draw % (position as u64 + 1)) as usize;
        permutation.swap(position, swap_with);
    }
    let mut assignment = vec![0_u32; element_count];
    for (shuffled_position, element_index) in permutation.iter().enumerate() {
        assignment[*element_index] = (shuffled_position % partition_count as usize) as u32;
    }
    SpartaTensorPartitionState {
        element_count,
        rotation_epoch,
        current_partition: 0,
        partition_count,
        assignment,
    }
}

// ---------------------------------------------------------------------------
// Pre-registration contract: written before any run, digest-pinned.
// ---------------------------------------------------------------------------

/// Partition strategy named in the pre-registered grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpartaPartitionStrategy {
    /// Random rotating partitions: `ceil(1/p)` partitions per rotation, one
    /// partition averaged per round, re-randomized each rotation.
    RandomRotatingPartitions,
}

/// The comparison baseline arm. Synchronous full-parameter averaging is the
/// only admitted baseline: the canary is judged against it, never against
/// nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpartaCanaryBaselineArm {
    SynchronousAveraging,
}

/// One grid point. The first committed point starts from the Pluralis
/// published tuning (sparsity 0.05, average state every 5 steps); that is
/// their tuning, not ours, and the grid is config, not constants.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpartaCanaryGridPoint {
    pub sparsity_fraction: f64,
    pub average_state_every_steps: u32,
    pub partition_strategy: SpartaPartitionStrategy,
}

impl SpartaCanaryGridPoint {
    pub fn validate(&self) -> Result<(), SpartaCanaryError> {
        validate_sparsity_fraction(self.sparsity_fraction)
            .map_err(|detail| SpartaCanaryError::InvalidPreRegistration { detail })?;
        if self.average_state_every_steps == 0 {
            return Err(SpartaCanaryError::InvalidPreRegistration {
                detail: String::from("average_state_every_steps must be at least 1"),
            });
        }
        Ok(())
    }
}

/// Eval schema for the canary. Per the research plan, perplexity alone never
/// decides the canary: the schema must name the first-divergence-step metric
/// and at least one held-out eval family that is not a perplexity surface.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpartaCanaryEvalSchema {
    /// Named first-divergence metric, e.g. first step where the sparse arm's
    /// held-out eval trails the baseline beyond the bound.
    pub first_divergence_step_metric: String,
    /// Held-out eval families that decide the canary alongside divergence.
    pub held_out_eval_families: Vec<String>,
    /// Additional tracked metrics; may include perplexity, never alone.
    pub additional_tracked_metrics: Vec<String>,
}

impl SpartaCanaryEvalSchema {
    pub fn validate(&self) -> Result<(), SpartaCanaryError> {
        let invalid = |detail: String| SpartaCanaryError::InvalidPreRegistration { detail };
        if self.first_divergence_step_metric.trim().is_empty() {
            return Err(invalid(String::from(
                "first_divergence_step_metric must be named",
            )));
        }
        if self.held_out_eval_families.is_empty() {
            return Err(invalid(String::from(
                "held_out_eval_families must be nonempty; perplexity alone never decides the canary",
            )));
        }
        if self
            .held_out_eval_families
            .iter()
            .any(|family| family.trim().is_empty())
        {
            return Err(invalid(String::from(
                "held_out_eval_families must not contain empty names",
            )));
        }
        if self
            .held_out_eval_families
            .iter()
            .all(|family| family.to_ascii_lowercase().contains("perplexity"))
        {
            return Err(invalid(String::from(
                "held-out eval families cannot all be perplexity surfaces; perplexity alone never decides the canary",
            )));
        }
        Ok(())
    }
}

/// Pre-registered kill bound. Both clauses are written before any run:
/// the canary is killed if held-out evals degrade beyond the bound vs the
/// synchronous baseline, or if the communications savings are immaterial.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpartaCanaryKillBound {
    /// Kill if held-out eval degradation vs baseline exceeds this fraction.
    pub max_eval_degradation_fraction_vs_baseline: f64,
    /// Kill if simulated/measured comms savings vs baseline fall below this
    /// materiality floor.
    pub min_comms_savings_fraction_floor: f64,
}

impl SpartaCanaryKillBound {
    pub fn validate(&self) -> Result<(), SpartaCanaryError> {
        let invalid = |detail: String| SpartaCanaryError::InvalidPreRegistration { detail };
        if !self.max_eval_degradation_fraction_vs_baseline.is_finite()
            || self.max_eval_degradation_fraction_vs_baseline <= 0.0
            || self.max_eval_degradation_fraction_vs_baseline >= 1.0
        {
            return Err(invalid(String::from(
                "max_eval_degradation_fraction_vs_baseline must be finite and in (0, 1)",
            )));
        }
        if !self.min_comms_savings_fraction_floor.is_finite()
            || self.min_comms_savings_fraction_floor <= 0.0
            || self.min_comms_savings_fraction_floor >= 1.0
        {
            return Err(invalid(String::from(
                "min_comms_savings_fraction_floor must be finite and in (0, 1)",
            )));
        }
        Ok(())
    }
}

/// The pre-registration record: written and digest-pinned before any run.
/// A run whose grid, baseline, eval schema, or kill bound does not match a
/// committed pre-registration digest is not the pre-registered canary.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpartaCanaryPreRegistration {
    pub schema_version: String,
    pub preregistration_id: String,
    /// Must carry `SPARTA_CANARY_STANDING_ORDER` verbatim.
    pub standing_order: String,
    pub grid: Vec<SpartaCanaryGridPoint>,
    pub baseline_arm: SpartaCanaryBaselineArm,
    pub eval_schema: SpartaCanaryEvalSchema,
    pub kill_bound: SpartaCanaryKillBound,
    /// Caller-passed registration timestamp (RFC 3339); the harness never
    /// reads wall clocks.
    pub registered_at: String,
    pub preregistration_digest: String,
}

impl SpartaCanaryPreRegistration {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.preregistration_digest.clear();
        stable_digest(b"psionic_sparta_canary_preregistration|", &clone)
    }

    pub fn validate(&self) -> Result<(), SpartaCanaryError> {
        let invalid = |detail: String| SpartaCanaryError::InvalidPreRegistration { detail };
        if self.schema_version != SPARTA_CANARY_PREREGISTRATION_SCHEMA_VERSION {
            return Err(invalid(format!(
                "schema_version must stay `{SPARTA_CANARY_PREREGISTRATION_SCHEMA_VERSION}`"
            )));
        }
        if self.preregistration_id.trim().is_empty() {
            return Err(invalid(String::from("preregistration_id must be nonempty")));
        }
        if self.standing_order != SPARTA_CANARY_STANDING_ORDER {
            return Err(invalid(String::from(
                "standing_order must carry the W3 standing order verbatim: no public gradients into the main optimizer, ever",
            )));
        }
        if self.grid.is_empty() {
            return Err(invalid(String::from("grid must carry at least one point")));
        }
        for point in &self.grid {
            point.validate()?;
        }
        self.eval_schema.validate()?;
        self.kill_bound.validate()?;
        if self.registered_at.trim().is_empty() {
            return Err(invalid(String::from(
                "registered_at must carry the caller-passed registration timestamp",
            )));
        }
        if self.preregistration_digest != self.stable_digest() {
            return Err(invalid(String::from(
                "preregistration_digest does not match the stable digest; the pinned grid drifted",
            )));
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Bounded deterministic two-arm comparison runner.
// ---------------------------------------------------------------------------

/// One synthetic toy tensor in the bounded simulation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpartaCanarySyntheticTensorSpec {
    pub tensor_name: String,
    pub element_count: usize,
}

/// Bounded simulation config: small tensors, in-process replicas, no
/// networking. Every field is caller-passed; nothing reads ambient state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpartaCanarySimulationConfig {
    pub replica_count: usize,
    pub tensor_specs: Vec<SpartaCanarySyntheticTensorSpec>,
    pub total_rounds: u32,
    /// Averaging happens on rounds where `(round_index + 1) %
    /// average_state_every_rounds == 0`.
    pub average_state_every_rounds: u32,
    pub sparsity_fraction: f64,
    /// Scale of the per-replica synthetic drift injected each round.
    pub drift_scale: f64,
    pub seed: u64,
}

impl SpartaCanarySimulationConfig {
    pub fn validate(&self) -> Result<(), SpartaCanaryError> {
        let invalid = |detail: String| SpartaCanaryError::InvalidSimulationConfig { detail };
        if self.replica_count < 2 {
            return Err(invalid(String::from(
                "replica_count must be at least 2 to measure inter-replica divergence",
            )));
        }
        if self.tensor_specs.is_empty() {
            return Err(invalid(String::from("tensor_specs must be nonempty")));
        }
        let mut seen = std::collections::BTreeSet::new();
        for spec in &self.tensor_specs {
            if spec.tensor_name.trim().is_empty() || spec.element_count == 0 {
                return Err(invalid(format!(
                    "tensor spec `{}` must carry a name and at least one element",
                    spec.tensor_name
                )));
            }
            if !seen.insert(spec.tensor_name.as_str()) {
                return Err(invalid(format!(
                    "duplicate tensor name `{}`",
                    spec.tensor_name
                )));
            }
        }
        if self.total_rounds == 0 || self.average_state_every_rounds == 0 {
            return Err(invalid(String::from(
                "total_rounds and average_state_every_rounds must be at least 1",
            )));
        }
        validate_sparsity_fraction(self.sparsity_fraction).map_err(invalid)?;
        if !self.drift_scale.is_finite() || self.drift_scale <= 0.0 {
            return Err(invalid(String::from(
                "drift_scale must be finite and positive",
            )));
        }
        Ok(())
    }
}

/// Per-round metrics for one arm.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpartaCanaryArmRoundMetrics {
    pub averaged_this_round: bool,
    /// Mean pairwise L2 distance between replica parameter vectors, summed
    /// over tensors, after this round's drift and (possibly) averaging.
    /// Quantized to nano-units (1e-9) so the committed artifact survives
    /// JSON round-trips exactly and stays digest-friendly.
    pub mean_pairwise_l2_divergence_nano: u64,
    /// Simulated bytes exchanged this round (elements averaged * 8 bytes *
    /// replica_count); zero on non-averaging rounds.
    pub simulated_bytes_communicated: u64,
}

/// Quantizes a nonnegative metric to nano-units (1e-9) for exact JSON
/// round-trips and digest stability.
#[must_use]
pub fn quantize_metric_nano(value: f64) -> u64 {
    (value * 1e9).round() as u64
}

/// One simulated round, both arms side by side.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpartaCanaryRoundComparison {
    pub round_index: u32,
    pub sparse_arm: SpartaCanaryArmRoundMetrics,
    pub synchronous_arm: SpartaCanaryArmRoundMetrics,
}

/// Whether the artifact carries bounded synthetic values or a real run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpartaCanaryMeasurementStatus {
    /// Values come from the bounded in-process simulation; synthetic only.
    SyntheticBounded,
    /// Values come from a real gated canary run (none exist; R1/R2-gated).
    Measured,
}

/// The two-arm comparison artifact the runner produces.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpartaCanaryComparisonArtifact {
    pub comparison_id: String,
    pub config: SpartaCanarySimulationConfig,
    pub per_round: Vec<SpartaCanaryRoundComparison>,
    /// Averaging rounds needed for the sparse arm to touch every index once:
    /// the max partition count over the simulated tensors.
    pub averaging_rounds_to_full_coverage: u32,
    pub total_sparse_bytes_communicated: u64,
    pub total_synchronous_bytes_communicated: u64,
    pub measurement_status: SpartaCanaryMeasurementStatus,
    pub detail: String,
}

impl SpartaCanaryComparisonArtifact {
    pub fn validate(&self) -> Result<(), SpartaCanaryError> {
        let invalid = |detail: String| SpartaCanaryError::InvalidComparisonArtifact { detail };
        if self.comparison_id.trim().is_empty() {
            return Err(invalid(String::from("comparison_id must be nonempty")));
        }
        self.config.validate()?;
        if self.measurement_status == SpartaCanaryMeasurementStatus::SyntheticBounded
            && !self.detail.to_ascii_lowercase().contains("synthetic")
        {
            return Err(invalid(String::from(
                "synthetic bounded artifacts must say so in their detail",
            )));
        }
        // Replay the bounded simulation from the committed config and refuse
        // any artifact whose recorded values drift from what the runner
        // deterministically produces.
        let replayed = run_sparta_canary_comparison(
            &self.comparison_id,
            self.config.clone(),
            self.measurement_status,
            &self.detail,
        )?;
        if replayed != *self {
            return Err(invalid(String::from(
                "artifact values drifted from the deterministic replay of its own config",
            )));
        }
        Ok(())
    }
}

struct ReplicaSet {
    /// replica -> tensor_name -> values
    replicas: Vec<BTreeMap<String, Vec<f64>>>,
}

impl ReplicaSet {
    fn seeded(config: &SpartaCanarySimulationConfig) -> Self {
        let mut replicas = Vec::with_capacity(config.replica_count);
        for _ in 0..config.replica_count {
            let mut tensors = BTreeMap::new();
            for spec in &config.tensor_specs {
                let stream = tensor_name_stream(&spec.tensor_name) ^ splitmix64(0x5EED);
                let values: Vec<f64> = (0..spec.element_count)
                    .map(|element| {
                        (sparta_counter_unit(config.seed, stream, element as u64) - 0.5) * 2.0
                    })
                    .collect();
                tensors.insert(spec.tensor_name.clone(), values);
            }
            // Every replica starts from the identical seeded state.
            replicas.push(tensors);
        }
        Self { replicas }
    }

    /// Injects deterministic per-replica drift for one round. The drift is
    /// keyed by (replica, round, tensor, element), so the two arms receive
    /// identical drift streams and stay comparable.
    fn apply_drift(&mut self, config: &SpartaCanarySimulationConfig, round_index: u32) {
        for (replica_index, tensors) in self.replicas.iter_mut().enumerate() {
            for spec in &config.tensor_specs {
                let stream = tensor_name_stream(&spec.tensor_name)
                    ^ splitmix64(
                        0xD81F ^ (((replica_index as u64) << 32) | u64::from(round_index)),
                    );
                let values = tensors
                    .get_mut(&spec.tensor_name)
                    .expect("tensor was seeded");
                for (element, value) in values.iter_mut().enumerate() {
                    let unit = sparta_counter_unit(config.seed, stream, element as u64);
                    *value += config.drift_scale * (unit - 0.5) * 2.0;
                }
            }
        }
    }

    /// Averages the given indices of one tensor across all replicas in place.
    /// Returns the number of elements averaged.
    fn average_indices(&mut self, tensor_name: &str, indices: &[usize]) -> usize {
        let replica_count = self.replicas.len() as f64;
        for index in indices {
            let sum: f64 = self
                .replicas
                .iter()
                .map(|tensors| tensors[tensor_name][*index])
                .sum();
            let mean = sum / replica_count;
            for tensors in &mut self.replicas {
                tensors.get_mut(tensor_name).expect("tensor exists")[*index] = mean;
            }
        }
        indices.len()
    }

    /// Mean pairwise L2 distance between replica parameter vectors, summed
    /// over tensors.
    fn mean_pairwise_l2_divergence(&self) -> f64 {
        let replica_count = self.replicas.len();
        let mut pair_count = 0_u64;
        let mut total = 0.0_f64;
        for a in 0..replica_count {
            for b in (a + 1)..replica_count {
                let mut squared = 0.0_f64;
                for (tensor_name, values_a) in &self.replicas[a] {
                    let values_b = &self.replicas[b][tensor_name];
                    for (value_a, value_b) in values_a.iter().zip(values_b) {
                        let delta = value_a - value_b;
                        squared += delta * delta;
                    }
                }
                total += squared.sqrt();
                pair_count += 1;
            }
        }
        total / pair_count as f64
    }
}

/// Runs the bounded deterministic two-arm comparison: sparse rotating-
/// partition averaging vs synchronous full averaging over in-process replicas
/// with identical synthetic drift. This demonstrates the harness end to end at
/// toy scale; it makes no claim about real convergence behavior.
pub fn run_sparta_canary_comparison(
    comparison_id: &str,
    config: SpartaCanarySimulationConfig,
    measurement_status: SpartaCanaryMeasurementStatus,
    detail: &str,
) -> Result<SpartaCanaryComparisonArtifact, SpartaCanaryError> {
    config.validate()?;
    if comparison_id.trim().is_empty() {
        return Err(SpartaCanaryError::InvalidComparisonArtifact {
            detail: String::from("comparison_id must be nonempty"),
        });
    }

    let mut sparse_replicas = ReplicaSet::seeded(&config);
    let mut sync_replicas = ReplicaSet::seeded(&config);
    let mut selector = SpartaPartitionedIndexSelector::new(config.seed, config.sparsity_fraction)?;

    let mut per_round = Vec::with_capacity(config.total_rounds as usize);
    let mut total_sparse_bytes = 0_u64;
    let mut total_sync_bytes = 0_u64;

    for round_index in 0..config.total_rounds {
        sparse_replicas.apply_drift(&config, round_index);
        sync_replicas.apply_drift(&config, round_index);

        let averaging_round = (round_index + 1) % config.average_state_every_rounds == 0;
        let mut sparse_elements = 0_usize;
        let mut sync_elements = 0_usize;
        if averaging_round {
            for spec in &config.tensor_specs {
                let indices = selector.select_indices(&spec.tensor_name, spec.element_count)?;
                sparse_elements += sparse_replicas.average_indices(&spec.tensor_name, &indices);
                let all_indices: Vec<usize> = (0..spec.element_count).collect();
                sync_elements += sync_replicas.average_indices(&spec.tensor_name, &all_indices);
            }
        }
        let sparse_bytes = (sparse_elements as u64) * 8 * config.replica_count as u64;
        let sync_bytes = (sync_elements as u64) * 8 * config.replica_count as u64;
        total_sparse_bytes += sparse_bytes;
        total_sync_bytes += sync_bytes;

        per_round.push(SpartaCanaryRoundComparison {
            round_index,
            sparse_arm: SpartaCanaryArmRoundMetrics {
                averaged_this_round: averaging_round,
                mean_pairwise_l2_divergence_nano: quantize_metric_nano(
                    sparse_replicas.mean_pairwise_l2_divergence(),
                ),
                simulated_bytes_communicated: sparse_bytes,
            },
            synchronous_arm: SpartaCanaryArmRoundMetrics {
                averaged_this_round: averaging_round,
                mean_pairwise_l2_divergence_nano: quantize_metric_nano(
                    sync_replicas.mean_pairwise_l2_divergence(),
                ),
                simulated_bytes_communicated: sync_bytes,
            },
        });
    }

    let averaging_rounds_to_full_coverage = config
        .tensor_specs
        .iter()
        .map(|spec| selector.partitions_per_rotation(spec.element_count))
        .max()
        .unwrap_or(1);

    Ok(SpartaCanaryComparisonArtifact {
        comparison_id: comparison_id.to_string(),
        config,
        per_round,
        averaging_rounds_to_full_coverage,
        total_sparse_bytes_communicated: total_sparse_bytes,
        total_synchronous_bytes_communicated: total_sync_bytes,
        measurement_status,
        detail: detail.to_string(),
    })
}

// ---------------------------------------------------------------------------
// Kill-bound evaluation and outcome recording.
// ---------------------------------------------------------------------------

/// Canary outcome states.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpartaCanaryOutcomeStatus {
    /// The canary passed every pre-registered bound on a real gated run.
    Held,
    /// A pre-registered bound was tripped; the mechanism stays out.
    Killed,
    /// No real gated canary run has produced a decision.
    Pending,
}

/// Which pre-registered bound decided a kill.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpartaCanaryDecidingBound {
    /// Held-out eval degradation vs the synchronous baseline exceeded the
    /// pre-registered maximum.
    EvalDegradationBound,
    /// Communications savings fell below the pre-registered materiality
    /// floor.
    CommsSavingsMaterialityFloor,
}

/// Observed outcome values fed to the kill-bound evaluation. These come from
/// a real gated canary run; the bounded simulation never produces them.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpartaCanaryObservedOutcome {
    /// Held-out eval degradation vs the synchronous baseline, as a fraction
    /// (positive = worse than baseline).
    pub eval_degradation_fraction_vs_baseline: f64,
    /// Communications savings vs the synchronous baseline, as a fraction of
    /// baseline bytes.
    pub comms_savings_fraction_vs_baseline: f64,
}

/// Evaluates one observed outcome against the pre-registered kill bound.
/// The eval-degradation clause is checked first; either tripped clause kills.
/// A killed canary still produces an outcome record and ledger write-back.
pub fn evaluate_sparta_kill_bound(
    bound: &SpartaCanaryKillBound,
    observed: &SpartaCanaryObservedOutcome,
) -> Result<(SpartaCanaryOutcomeStatus, Option<SpartaCanaryDecidingBound>), SpartaCanaryError> {
    bound.validate()?;
    if !observed.eval_degradation_fraction_vs_baseline.is_finite()
        || !observed.comms_savings_fraction_vs_baseline.is_finite()
    {
        return Err(SpartaCanaryError::InvalidOutcomeRecord {
            detail: String::from("observed outcome values must be finite"),
        });
    }
    if observed.eval_degradation_fraction_vs_baseline
        > bound.max_eval_degradation_fraction_vs_baseline
    {
        return Ok((
            SpartaCanaryOutcomeStatus::Killed,
            Some(SpartaCanaryDecidingBound::EvalDegradationBound),
        ));
    }
    if observed.comms_savings_fraction_vs_baseline < bound.min_comms_savings_fraction_floor {
        return Ok((
            SpartaCanaryOutcomeStatus::Killed,
            Some(SpartaCanaryDecidingBound::CommsSavingsMaterialityFloor),
        ));
    }
    Ok((SpartaCanaryOutcomeStatus::Held, None))
}

/// Typed canary outcome record, designed to be written back to the derisking
/// ledger entry for SPARTA-class sparse parameter averaging.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpartaCanaryOutcomeRecord {
    pub schema_version: String,
    pub outcome_id: String,
    /// Digest of the pre-registration this outcome answers.
    pub preregistration_digest: String,
    pub status: SpartaCanaryOutcomeStatus,
    /// The pre-registered bound that decided a kill; `None` for held (every
    /// bound passed) and pending (no decision exists).
    pub decided_by: Option<SpartaCanaryDecidingBound>,
    /// Observed values behind a held/killed decision; absent while pending.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_outcome: Option<SpartaCanaryObservedOutcome>,
    pub derisking_ledger_path: String,
    pub derisking_ledger_entry: String,
    /// Caller-passed recording timestamp (RFC 3339).
    pub recorded_at: String,
    pub detail: String,
}

impl SpartaCanaryOutcomeRecord {
    pub fn validate(&self) -> Result<(), SpartaCanaryError> {
        let invalid = |detail: String| SpartaCanaryError::InvalidOutcomeRecord { detail };
        if self.schema_version != SPARTA_CANARY_OUTCOME_SCHEMA_VERSION {
            return Err(invalid(format!(
                "schema_version must stay `{SPARTA_CANARY_OUTCOME_SCHEMA_VERSION}`"
            )));
        }
        if self.outcome_id.trim().is_empty() || self.recorded_at.trim().is_empty() {
            return Err(invalid(String::from(
                "outcome_id and recorded_at must be nonempty",
            )));
        }
        if self.preregistration_digest.len() != 64
            || !self
                .preregistration_digest
                .chars()
                .all(|c| c.is_ascii_hexdigit())
        {
            return Err(invalid(String::from(
                "preregistration_digest must be a 64-character SHA256 hex digest",
            )));
        }
        if self.derisking_ledger_path != SPARTA_CANARY_DERISKING_LEDGER_PATH {
            return Err(invalid(String::from("derisking_ledger_path drifted")));
        }
        if self.derisking_ledger_entry != SPARTA_CANARY_DERISKING_LEDGER_ENTRY {
            return Err(invalid(String::from("derisking_ledger_entry drifted")));
        }
        match (self.status, self.decided_by, self.observed_outcome) {
            (SpartaCanaryOutcomeStatus::Pending, None, None) => {}
            (SpartaCanaryOutcomeStatus::Pending, _, _) => {
                return Err(invalid(String::from(
                    "pending outcomes carry no deciding bound and no observed values",
                )));
            }
            (SpartaCanaryOutcomeStatus::Killed, Some(_), Some(_)) => {}
            (SpartaCanaryOutcomeStatus::Killed, _, _) => {
                return Err(invalid(String::from(
                    "killed outcomes must name the deciding bound and the observed values",
                )));
            }
            (SpartaCanaryOutcomeStatus::Held, None, Some(_)) => {}
            (SpartaCanaryOutcomeStatus::Held, _, _) => {
                return Err(invalid(String::from(
                    "held outcomes carry observed values and no deciding bound (every bound passed)",
                )));
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Canonical contract: pre-registration + bounded demo + pending outcome.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpartaCanaryAuthorityPaths {
    pub fixture_path: String,
    pub check_script_path: String,
    pub reference_doc_path: String,
    pub derisking_ledger_path: String,
    pub train_system_doc_path: String,
}

/// Canonical SPARTA canary harness contract.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpartaCanaryContract {
    pub schema_version: String,
    pub contract_id: String,
    pub preregistration: SpartaCanaryPreRegistration,
    pub comparison_artifact: SpartaCanaryComparisonArtifact,
    pub outcome_record: SpartaCanaryOutcomeRecord,
    pub authority_paths: SpartaCanaryAuthorityPaths,
    pub claim_boundary: String,
    pub contract_digest: String,
}

impl SpartaCanaryContract {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.contract_digest.clear();
        stable_digest(b"psionic_sparta_canary_contract|", &clone)
    }

    pub fn validate(&self) -> Result<(), SpartaCanaryError> {
        let invalid = |detail: String| SpartaCanaryError::InvalidContract { detail };
        if self.schema_version != SPARTA_CANARY_SCHEMA_VERSION {
            return Err(invalid(format!(
                "schema_version must stay `{SPARTA_CANARY_SCHEMA_VERSION}` but was `{}`",
                self.schema_version
            )));
        }
        if self.contract_id != SPARTA_CANARY_CONTRACT_ID {
            return Err(invalid(String::from("contract_id drifted")));
        }
        if self.authority_paths.fixture_path != SPARTA_CANARY_FIXTURE_PATH
            || self.authority_paths.check_script_path != SPARTA_CANARY_CHECK_SCRIPT_PATH
            || self.authority_paths.reference_doc_path != SPARTA_CANARY_DOC_PATH
            || self.authority_paths.derisking_ledger_path != SPARTA_CANARY_DERISKING_LEDGER_PATH
            || self.authority_paths.train_system_doc_path != SPARTA_CANARY_TRAIN_SYSTEM_DOC_PATH
        {
            return Err(invalid(String::from("authority paths drifted")));
        }
        self.preregistration.validate()?;
        self.comparison_artifact.validate()?;
        self.outcome_record.validate()?;
        if self.outcome_record.preregistration_digest != self.preregistration.preregistration_digest
        {
            return Err(invalid(String::from(
                "outcome record is not bound to this contract's pre-registration digest",
            )));
        }
        // Until a real gated canary run exists, the artifact stays synthetic
        // and the outcome stays pending: a bounded toy simulation never
        // decides the canary.
        if self.comparison_artifact.measurement_status != SpartaCanaryMeasurementStatus::Measured
            && self.outcome_record.status != SpartaCanaryOutcomeStatus::Pending
        {
            return Err(invalid(String::from(
                "a synthetic bounded artifact cannot carry a held or killed outcome; only a real gated run decides",
            )));
        }
        if self.comparison_artifact.measurement_status != SpartaCanaryMeasurementStatus::Measured
            && !self
                .claim_boundary
                .to_ascii_lowercase()
                .contains("synthetic")
        {
            return Err(invalid(String::from(
                "claim_boundary must state that the comparison values are synthetic until a real gated run lands",
            )));
        }
        if self.contract_digest != self.stable_digest() {
            return Err(invalid(String::from(
                "contract_digest does not match the stable digest",
            )));
        }
        Ok(())
    }
}

static SPARTA_CANARY_CONTRACT_CACHE: std::sync::OnceLock<SpartaCanaryContract> =
    std::sync::OnceLock::new();

pub fn canonical_sparta_canary_contract() -> Result<SpartaCanaryContract, SpartaCanaryError> {
    if let Some(contract) = SPARTA_CANARY_CONTRACT_CACHE.get() {
        return Ok(contract.clone());
    }

    // Grid starting from the Pluralis published tuning (sparsity fraction
    // 0.05, average state every 5 steps, random rotating partitions). That is
    // their tuning, not ours; the grid is a pre-registered config input.
    let grid = vec![
        SpartaCanaryGridPoint {
            sparsity_fraction: 0.05,
            average_state_every_steps: 5,
            partition_strategy: SpartaPartitionStrategy::RandomRotatingPartitions,
        },
        SpartaCanaryGridPoint {
            sparsity_fraction: 0.10,
            average_state_every_steps: 5,
            partition_strategy: SpartaPartitionStrategy::RandomRotatingPartitions,
        },
        SpartaCanaryGridPoint {
            sparsity_fraction: 0.05,
            average_state_every_steps: 10,
            partition_strategy: SpartaPartitionStrategy::RandomRotatingPartitions,
        },
    ];
    let mut preregistration = SpartaCanaryPreRegistration {
        schema_version: String::from(SPARTA_CANARY_PREREGISTRATION_SCHEMA_VERSION),
        preregistration_id: String::from("sparta_canary_preregistration.fixture.v1"),
        standing_order: String::from(SPARTA_CANARY_STANDING_ORDER),
        grid,
        baseline_arm: SpartaCanaryBaselineArm::SynchronousAveraging,
        eval_schema: SpartaCanaryEvalSchema {
            first_divergence_step_metric: String::from(
                "first_step_where_sparse_arm_held_out_eval_trails_synchronous_baseline_beyond_bound",
            ),
            held_out_eval_families: vec![
                String::from("held_out_family.reasoning_short_answer"),
                String::from("held_out_family.code_completion_bounded"),
            ],
            additional_tracked_metrics: vec![
                String::from("held_out_perplexity_tracked_never_deciding_alone"),
                String::from("per_round_parameter_divergence_between_replicas"),
            ],
        },
        kill_bound: SpartaCanaryKillBound {
            max_eval_degradation_fraction_vs_baseline: 0.02,
            min_comms_savings_fraction_floor: 0.5,
        },
        registered_at: String::from("2026-06-12T00:00:00Z"),
        preregistration_digest: String::new(),
    };
    preregistration.preregistration_digest = preregistration.stable_digest();
    preregistration.validate()?;

    // Bounded toy-scale demo config. The sparsity fraction here (0.25 ->
    // 4 partitions per rotation) is a toy walkthrough input chosen so the
    // committed fixture shows a complete rotation; it is not the
    // pre-registered grid and decides nothing.
    let simulation_config = SpartaCanarySimulationConfig {
        replica_count: 3,
        tensor_specs: vec![
            SpartaCanarySyntheticTensorSpec {
                tensor_name: String::from("toy.embedding"),
                element_count: 24,
            },
            SpartaCanarySyntheticTensorSpec {
                tensor_name: String::from("toy.linear"),
                element_count: 40,
            },
        ],
        total_rounds: 10,
        average_state_every_rounds: 2,
        sparsity_fraction: 0.25,
        drift_scale: 0.01,
        seed: 0x1127,
    };
    let comparison_artifact = run_sparta_canary_comparison(
        "sparta_canary_comparison.fixture.v1",
        simulation_config,
        SpartaCanaryMeasurementStatus::SyntheticBounded,
        "Synthetic bounded toy-scale demo: in-process replicas with seeded synthetic drift, \
sparse rotating-partition averaging vs synchronous full averaging. Demonstrates the harness \
end to end only; no claim about real convergence behavior.",
    )?;

    let outcome_record = SpartaCanaryOutcomeRecord {
        schema_version: String::from(SPARTA_CANARY_OUTCOME_SCHEMA_VERSION),
        outcome_id: String::from("sparta_canary_outcome.fixture.v1"),
        preregistration_digest: preregistration.preregistration_digest.clone(),
        status: SpartaCanaryOutcomeStatus::Pending,
        decided_by: None,
        observed_outcome: None,
        derisking_ledger_path: String::from(SPARTA_CANARY_DERISKING_LEDGER_PATH),
        derisking_ledger_entry: String::from(SPARTA_CANARY_DERISKING_LEDGER_ENTRY),
        recorded_at: String::from("2026-06-12T00:00:00Z"),
        detail: String::from(
            "Pending: the harness exists and the pre-registered bounds are pinned, but no real \
gated canary run has produced a held/killed decision. The bounded synthetic demo decides \
nothing. A killed canary still produces this record and the ledger write-back.",
        ),
    };

    let mut contract = SpartaCanaryContract {
        schema_version: String::from(SPARTA_CANARY_SCHEMA_VERSION),
        contract_id: String::from(SPARTA_CANARY_CONTRACT_ID),
        preregistration,
        comparison_artifact,
        outcome_record,
        authority_paths: SpartaCanaryAuthorityPaths {
            fixture_path: String::from(SPARTA_CANARY_FIXTURE_PATH),
            check_script_path: String::from(SPARTA_CANARY_CHECK_SCRIPT_PATH),
            reference_doc_path: String::from(SPARTA_CANARY_DOC_PATH),
            derisking_ledger_path: String::from(SPARTA_CANARY_DERISKING_LEDGER_PATH),
            train_system_doc_path: String::from(SPARTA_CANARY_TRAIN_SYSTEM_DOC_PATH),
        },
        claim_boundary: String::from(
            "This contract freezes the SPARTA canary harness shape: a digest-pinned \
pre-registration (grid, synchronous baseline, eval schema that never lets perplexity decide \
alone, kill bound), owned rotating-partition mechanics, a bounded deterministic two-arm \
comparison runner, and a pending outcome record bound to the pre-registration digest. Every \
comparison value is synthetic toy-scale output of the in-process simulation. No real canary \
has run; the R1/R2-scale canary is gated and not claimed, and the W3 standing order stands: \
no public gradients into the main optimizer, ever.",
        ),
        contract_digest: String::new(),
    };
    contract.contract_digest = contract.stable_digest();
    contract.validate()?;
    let _ = SPARTA_CANARY_CONTRACT_CACHE.set(contract.clone());
    Ok(contract)
}

pub fn write_sparta_canary_contract(
    output_path: impl AsRef<Path>,
) -> Result<(), SpartaCanaryError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| SpartaCanaryError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    let contract = canonical_sparta_canary_contract()?;
    let bytes = serde_json::to_vec_pretty(&contract)?;
    fs::write(output_path, bytes).map_err(|error| SpartaCanaryError::Write {
        path: output_path.display().to_string(),
        error,
    })
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(
        serde_json::to_vec(value)
            .expect("stable digest serialization must succeed for sparta canary records"),
    );
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toy_config() -> SpartaCanarySimulationConfig {
        SpartaCanarySimulationConfig {
            replica_count: 3,
            tensor_specs: vec![
                SpartaCanarySyntheticTensorSpec {
                    tensor_name: String::from("test.alpha"),
                    element_count: 16,
                },
                SpartaCanarySyntheticTensorSpec {
                    tensor_name: String::from("test.beta"),
                    element_count: 9,
                },
            ],
            total_rounds: 6,
            average_state_every_rounds: 2,
            sparsity_fraction: 0.25,
            drift_scale: 0.02,
            seed: 7,
        }
    }

    #[test]
    fn canonical_sparta_canary_contract_is_valid() -> Result<(), SpartaCanaryError> {
        let contract = canonical_sparta_canary_contract()?;
        contract.validate()
    }

    #[test]
    fn canonical_contract_round_trips_through_json() -> Result<(), SpartaCanaryError> {
        let contract = canonical_sparta_canary_contract()?;
        let bytes = serde_json::to_vec(&contract)?;
        let decoded: SpartaCanaryContract = serde_json::from_slice(&bytes)?;
        assert_eq!(contract, decoded);
        decoded.validate()
    }

    #[test]
    fn committed_fixture_round_trips_and_matches_canonical(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .ok_or("missing repo root")?;
        let fixture_path = repo_root.join(SPARTA_CANARY_FIXTURE_PATH);
        let bytes = fs::read(&fixture_path)?;
        let committed: SpartaCanaryContract = serde_json::from_slice(&bytes)?;
        committed.validate()?;
        assert_eq!(committed, canonical_sparta_canary_contract()?);
        Ok(())
    }

    #[test]
    fn partition_rotation_covers_all_indices_exactly_once() -> Result<(), SpartaCanaryError> {
        let mut selector = SpartaPartitionedIndexSelector::new(42, 0.3)?;
        let element_count = 23;
        let partitions = selector.partitions_per_rotation(element_count);
        assert_eq!(partitions, 4, "ceil(1/0.3) = 4 partitions");
        let mut seen = vec![0_u32; element_count];
        for _ in 0..partitions {
            for index in selector.select_indices("test.tensor", element_count)? {
                seen[index] += 1;
            }
        }
        assert!(
            seen.iter().all(|count| *count == 1),
            "one full rotation must cover every index exactly once, got {seen:?}"
        );
        // The next rotation re-randomizes but still covers everything once.
        let mut second = vec![0_u32; element_count];
        for _ in 0..partitions {
            for index in selector.select_indices("test.tensor", element_count)? {
                second[index] += 1;
            }
        }
        assert!(second.iter().all(|count| *count == 1));
        Ok(())
    }

    #[test]
    fn partition_count_clamps_to_element_count() -> Result<(), SpartaCanaryError> {
        // ceil(1/0.05) = 20 partitions, but a 3-element tensor can carry at
        // most 3 nonempty partitions (Pluralis: min(ceil(1/p), numel)).
        let selector = SpartaPartitionedIndexSelector::new(1, 0.05)?;
        assert_eq!(selector.partitions_per_rotation(3), 3);
        Ok(())
    }

    #[test]
    fn partition_schedule_is_deterministic_under_same_seed() -> Result<(), SpartaCanaryError> {
        let mut first = SpartaPartitionedIndexSelector::new(99, 0.2)?;
        let mut second = SpartaPartitionedIndexSelector::new(99, 0.2)?;
        for _ in 0..12 {
            assert_eq!(
                first.select_indices("test.alpha", 17)?,
                second.select_indices("test.alpha", 17)?
            );
            assert_eq!(
                first.select_indices("test.beta", 8)?,
                second.select_indices("test.beta", 8)?
            );
        }
        // A different seed produces a different schedule somewhere in the
        // first rotation.
        let mut third = SpartaPartitionedIndexSelector::new(100, 0.2)?;
        let mut diverged = false;
        let mut fourth = SpartaPartitionedIndexSelector::new(99, 0.2)?;
        for _ in 0..5 {
            if third.select_indices("test.alpha", 17)? != fourth.select_indices("test.alpha", 17)? {
                diverged = true;
            }
        }
        assert!(diverged, "different seeds must produce different schedules");
        Ok(())
    }

    #[test]
    fn sparse_round_moves_only_the_selected_partition() -> Result<(), SpartaCanaryError> {
        let config = toy_config();
        let mut replicas = ReplicaSet::seeded(&config);
        // Drift once so replicas actually differ before averaging.
        replicas.apply_drift(&config, 0);
        let before: Vec<BTreeMap<String, Vec<f64>>> = replicas.replicas.clone();
        let mut selector =
            SpartaPartitionedIndexSelector::new(config.seed, config.sparsity_fraction)?;
        let indices = selector.select_indices("test.alpha", 16)?;
        assert!(!indices.is_empty() && indices.len() < 16);
        replicas.average_indices("test.alpha", &indices);
        for (replica_index, tensors) in replicas.replicas.iter().enumerate() {
            for (element, value) in tensors["test.alpha"].iter().enumerate() {
                let untouched = before[replica_index]["test.alpha"][element];
                if indices.contains(&element) {
                    // Selected indices must equal the cross-replica mean.
                    let mean: f64 = before.iter().map(|t| t["test.alpha"][element]).sum::<f64>()
                        / config.replica_count as f64;
                    assert_eq!(*value, mean);
                } else {
                    assert_eq!(
                        *value, untouched,
                        "index {element} outside the selected partition must not move"
                    );
                }
            }
            // The other tensor is untouched entirely.
            assert_eq!(tensors["test.beta"], before[replica_index]["test.beta"]);
        }
        Ok(())
    }

    #[test]
    fn comparison_runner_produces_both_arms_metrics() -> Result<(), SpartaCanaryError> {
        let artifact = run_sparta_canary_comparison(
            "test.comparison",
            toy_config(),
            SpartaCanaryMeasurementStatus::SyntheticBounded,
            "synthetic test run",
        )?;
        assert_eq!(artifact.per_round.len(), 6);
        let averaging_rounds: Vec<&SpartaCanaryRoundComparison> = artifact
            .per_round
            .iter()
            .filter(|round| round.sparse_arm.averaged_this_round)
            .collect();
        assert_eq!(averaging_rounds.len(), 3, "every 2nd of 6 rounds averages");
        for round in &artifact.per_round {
            assert_eq!(
                round.sparse_arm.averaged_this_round,
                round.synchronous_arm.averaged_this_round
            );
            if round.sparse_arm.averaged_this_round {
                assert!(
                    round.sparse_arm.simulated_bytes_communicated
                        < round.synchronous_arm.simulated_bytes_communicated,
                    "sparse rounds must communicate fewer simulated bytes"
                );
                // Synchronous full averaging collapses inter-replica
                // divergence; sparse leaves the unselected partitions apart.
                assert!(
                    round.synchronous_arm.mean_pairwise_l2_divergence_nano
                        <= round.sparse_arm.mean_pairwise_l2_divergence_nano
                );
            } else {
                assert_eq!(round.sparse_arm.simulated_bytes_communicated, 0);
                assert_eq!(round.synchronous_arm.simulated_bytes_communicated, 0);
            }
        }
        assert!(
            artifact.total_sparse_bytes_communicated
                < artifact.total_synchronous_bytes_communicated
        );
        assert_eq!(artifact.averaging_rounds_to_full_coverage, 4);
        // The runner is deterministic: replaying the config reproduces it.
        let replayed = run_sparta_canary_comparison(
            "test.comparison",
            toy_config(),
            SpartaCanaryMeasurementStatus::SyntheticBounded,
            "synthetic test run",
        )?;
        assert_eq!(artifact, replayed);
        artifact.validate()
    }

    #[test]
    fn preregistration_digest_pins_the_grid() -> Result<(), SpartaCanaryError> {
        let contract = canonical_sparta_canary_contract()?;
        let mut tampered = contract.preregistration.clone();
        tampered.grid[0].sparsity_fraction = 0.5;
        assert_ne!(
            tampered.stable_digest(),
            contract.preregistration.preregistration_digest,
            "changing the grid must change the pre-registration digest"
        );
        // A tampered grid with a stale digest is refused.
        let error = tampered
            .validate()
            .expect_err("stale digest over a changed grid must be refused");
        assert!(matches!(
            error,
            SpartaCanaryError::InvalidPreRegistration { .. }
        ));
        Ok(())
    }

    #[test]
    fn preregistration_refuses_perplexity_alone() -> Result<(), SpartaCanaryError> {
        let contract = canonical_sparta_canary_contract()?;
        let mut tampered = contract.preregistration.clone();
        tampered.eval_schema.held_out_eval_families =
            vec![String::from("held_out_perplexity_only")];
        tampered.preregistration_digest = tampered.stable_digest();
        let error = tampered
            .validate()
            .expect_err("perplexity alone can never decide the canary");
        assert!(matches!(
            error,
            SpartaCanaryError::InvalidPreRegistration { .. }
        ));
        Ok(())
    }

    #[test]
    fn preregistration_requires_standing_order_verbatim() -> Result<(), SpartaCanaryError> {
        let contract = canonical_sparta_canary_contract()?;
        let mut tampered = contract.preregistration.clone();
        tampered.standing_order = String::from("sparse averaging seems fine for the main run");
        tampered.preregistration_digest = tampered.stable_digest();
        let error = tampered
            .validate()
            .expect_err("a weakened standing order must be refused");
        assert!(matches!(
            error,
            SpartaCanaryError::InvalidPreRegistration { .. }
        ));
        Ok(())
    }

    #[test]
    fn kill_bound_evaluation_decides_both_sides() -> Result<(), SpartaCanaryError> {
        let bound = SpartaCanaryKillBound {
            max_eval_degradation_fraction_vs_baseline: 0.02,
            min_comms_savings_fraction_floor: 0.5,
        };
        // Inside both bounds: held, no deciding bound.
        let (status, decided) = evaluate_sparta_kill_bound(
            &bound,
            &SpartaCanaryObservedOutcome {
                eval_degradation_fraction_vs_baseline: 0.01,
                comms_savings_fraction_vs_baseline: 0.7,
            },
        )?;
        assert_eq!(status, SpartaCanaryOutcomeStatus::Held);
        assert_eq!(decided, None);
        // Eval degradation past the bound: killed by the degradation bound.
        let (status, decided) = evaluate_sparta_kill_bound(
            &bound,
            &SpartaCanaryObservedOutcome {
                eval_degradation_fraction_vs_baseline: 0.03,
                comms_savings_fraction_vs_baseline: 0.7,
            },
        )?;
        assert_eq!(status, SpartaCanaryOutcomeStatus::Killed);
        assert_eq!(
            decided,
            Some(SpartaCanaryDecidingBound::EvalDegradationBound)
        );
        // Comms savings below the materiality floor: killed by the floor.
        let (status, decided) = evaluate_sparta_kill_bound(
            &bound,
            &SpartaCanaryObservedOutcome {
                eval_degradation_fraction_vs_baseline: 0.01,
                comms_savings_fraction_vs_baseline: 0.3,
            },
        )?;
        assert_eq!(status, SpartaCanaryOutcomeStatus::Killed);
        assert_eq!(
            decided,
            Some(SpartaCanaryDecidingBound::CommsSavingsMaterialityFloor)
        );
        // Exactly at the bounds: held (kill requires exceeding the bound).
        let (status, _) = evaluate_sparta_kill_bound(
            &bound,
            &SpartaCanaryObservedOutcome {
                eval_degradation_fraction_vs_baseline: 0.02,
                comms_savings_fraction_vs_baseline: 0.5,
            },
        )?;
        assert_eq!(status, SpartaCanaryOutcomeStatus::Held);
        Ok(())
    }

    #[test]
    fn killed_outcome_record_requires_deciding_bound() -> Result<(), SpartaCanaryError> {
        let contract = canonical_sparta_canary_contract()?;
        let mut record = contract.outcome_record.clone();
        record.status = SpartaCanaryOutcomeStatus::Killed;
        let error = record
            .validate()
            .expect_err("killed without a deciding bound must be refused");
        assert!(matches!(
            error,
            SpartaCanaryError::InvalidOutcomeRecord { .. }
        ));
        record.decided_by = Some(SpartaCanaryDecidingBound::EvalDegradationBound);
        record.observed_outcome = Some(SpartaCanaryObservedOutcome {
            eval_degradation_fraction_vs_baseline: 0.03,
            comms_savings_fraction_vs_baseline: 0.7,
        });
        record.validate()?;
        Ok(())
    }

    #[test]
    fn contract_refuses_decided_outcome_on_synthetic_artifact() -> Result<(), SpartaCanaryError> {
        let mut contract = canonical_sparta_canary_contract()?;
        contract.outcome_record.status = SpartaCanaryOutcomeStatus::Held;
        contract.outcome_record.observed_outcome = Some(SpartaCanaryObservedOutcome {
            eval_degradation_fraction_vs_baseline: 0.0,
            comms_savings_fraction_vs_baseline: 0.9,
        });
        contract.contract_digest = contract.stable_digest();
        let error = contract
            .validate()
            .expect_err("a toy synthetic demo can never hold the canary");
        assert!(matches!(error, SpartaCanaryError::InvalidContract { .. }));
        Ok(())
    }

    #[test]
    fn artifact_refuses_tampered_metrics() -> Result<(), SpartaCanaryError> {
        let contract = canonical_sparta_canary_contract()?;
        let mut artifact = contract.comparison_artifact.clone();
        artifact.total_sparse_bytes_communicated = 1;
        let error = artifact
            .validate()
            .expect_err("tampered totals must fail the deterministic replay");
        assert!(matches!(
            error,
            SpartaCanaryError::InvalidComparisonArtifact { .. }
        ));
        Ok(())
    }

    #[test]
    fn counter_rng_is_order_independent() {
        let a = sparta_counter_rng(5, 9, 100);
        let _ = sparta_counter_rng(5, 9, 7);
        let b = sparta_counter_rng(5, 9, 100);
        assert_eq!(a, b);
        assert_ne!(sparta_counter_rng(5, 9, 100), sparta_counter_rng(5, 9, 101));
        assert_ne!(sparta_counter_rng(5, 9, 100), sparta_counter_rng(6, 9, 100));
    }
}
