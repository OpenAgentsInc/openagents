use std::collections::{BTreeMap, BTreeSet};

use psionic_environments::{
    EnvironmentContractError, EnvironmentPackageContract, EnvironmentPackageKey,
    EnvironmentRuntimeFamily, EnvironmentToolContract, EnvironmentToolInterface,
};
use psionic_eval::{EvalExecutionStrategyFacts, EvalRunContract, EvalRunMode, EvalRuntimeError};
use psionic_runtime::{
    DeterminismContractError, DeterminismMode, GeneratorState, RuntimeDeterminismContract,
    SamplingPolicy, SamplingStrategy, TokenSampler, TrainingCheckpointReference,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::TrainerBatch;

/// Error returned by the deterministic replay and reproducibility layer.
#[derive(Debug, Error)]
pub enum TrainingReplayError {
    /// One environment package contract failed validation.
    #[error(transparent)]
    EnvironmentContract(#[from] EnvironmentContractError),
    /// One eval contract failed validation.
    #[error(transparent)]
    EvalRuntime(#[from] EvalRuntimeError),
    /// The eval posture did not declare deterministic scheduling.
    #[error("eval posture must declare deterministic scheduling; found `{actual}`")]
    NonDeterministicEvalScheduler {
        /// Observed scheduler posture, or `missing` when absent.
        actual: String,
    },
    /// The same worker and attempt pair was repeated in replay selection rules.
    #[error(
        "deterministic sample-selection rule `{worker_id}` / attempt `{attempt_index}` was defined more than once"
    )]
    DuplicateSampleSelectionRule {
        /// Stable worker identifier.
        worker_id: String,
        /// Stable attempt index.
        attempt_index: u32,
    },
    /// One rule omitted the selected population needed for replay.
    #[error(
        "deterministic sample-selection rule `{worker_id}` / attempt `{attempt_index}` must select at least one item"
    )]
    EmptySelectedItems {
        /// Stable worker identifier.
        worker_id: String,
        /// Stable attempt index.
        attempt_index: u32,
    },
    /// The environment used by the replayable trainer batch did not match the eval posture.
    #[error("replay environment mismatch: expected `{expected}`, found `{actual}`")]
    EnvironmentEvalMismatch {
        /// Environment storage key expected by the environment pin.
        expected: String,
        /// Environment storage key observed from the eval posture.
        actual: String,
    },
}

/// Global seed contract for deterministic replay.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingReplaySeedDiscipline {
    /// Stable assignment seed used by window planning and claim derivation.
    pub assignment_seed: u64,
    /// Stable trainer seed used for any trainer-side randomized behavior.
    pub trainer_seed: u64,
    /// Stable eval seed used for held-out sample ordering or other eval randomness.
    pub eval_seed: u64,
}

impl TrainingReplaySeedDiscipline {
    /// Creates a replay-seed contract.
    #[must_use]
    pub const fn new(assignment_seed: u64, trainer_seed: u64, eval_seed: u64) -> Self {
        Self {
            assignment_seed,
            trainer_seed,
            eval_seed,
        }
    }

    /// Returns the seeded replay contract for assignment-time randomness.
    #[must_use]
    pub const fn assignment_runtime_contract(self) -> RuntimeDeterminismContract {
        RuntimeDeterminismContract::seeded(self.assignment_seed)
    }

    /// Returns the strict deterministic contract for trainer-side randomness.
    #[must_use]
    pub const fn trainer_runtime_contract(self) -> RuntimeDeterminismContract {
        RuntimeDeterminismContract::strict(self.trainer_seed)
    }

    /// Returns the strict deterministic contract for eval-time randomness.
    #[must_use]
    pub const fn eval_runtime_contract(self) -> RuntimeDeterminismContract {
        RuntimeDeterminismContract::strict(self.eval_seed)
    }

    /// Derives one stable eval-time local-device generator.
    pub fn derive_eval_local_device_generator(
        self,
        stable_device_id: impl Into<String>,
    ) -> Result<GeneratorState, DeterminismContractError> {
        self.eval_runtime_contract()
            .derive_local_device_generator(stable_device_id)
    }

    /// Derives one stable trainer-side distributed-rank generator.
    pub fn derive_trainer_distributed_rank_generator(
        self,
        replica_group: impl Into<String>,
        rank: usize,
        world_size: usize,
    ) -> Result<GeneratorState, DeterminismContractError> {
        self.trainer_runtime_contract()
            .derive_distributed_rank_generator(replica_group, rank, world_size)
    }
}

/// Scope covered by one reproducibility-semantics case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReproducibilitySemanticsScope {
    /// Global assignment-time seed discipline.
    Assignment,
    /// Trainer-side runtime determinism.
    Trainer,
    /// Eval-time runtime determinism.
    Eval,
    /// Local-device generator derivation.
    LocalDevice,
    /// Distributed-rank generator derivation.
    DistributedReplay,
    /// Checkpoint and restore of replayable RNG state.
    CheckpointRestore,
}

/// Status for one reproducibility-semantics case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReproducibilitySemanticsStatus {
    /// The case is explicitly supported.
    Supported,
    /// The case is explicitly refused.
    Refused,
}

/// One machine-readable reproducibility-semantics case.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReproducibilitySemanticsCaseResult {
    /// Stable case identifier.
    pub case_id: String,
    /// Covered scope.
    pub scope: ReproducibilitySemanticsScope,
    /// Current status for the scope.
    pub status: ReproducibilitySemanticsStatus,
    /// Stable seed discipline carried by the case.
    pub seed_discipline: TrainingReplaySeedDiscipline,
    /// Runtime determinism contract for the case when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_contract: Option<RuntimeDeterminismContract>,
    /// Derived generator state when the case proves local or distributed derivation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub derived_generator: Option<GeneratorState>,
    /// Restored determinism contract when the case proves checkpoint-state restore.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restored_contract: Option<RuntimeDeterminismContract>,
    /// Plain-language current scope boundary.
    pub bounded_scope: String,
    /// Explicit refusal when the case is intentionally unsupported.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<psionic_core::PsionicRefusal>,
}

/// Machine-readable framework-wide reproducibility semantics report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReproducibilitySemanticsReport {
    /// Stable schema version.
    pub schema_version: u32,
    /// Versioned current-scope window.
    pub current_scope_window: String,
    /// Cases carried by the report.
    pub cases: Vec<ReproducibilitySemanticsCaseResult>,
    /// Stable digest over the report contents.
    pub report_digest: String,
}

impl ReproducibilitySemanticsReport {
    fn new(
        current_scope_window: impl Into<String>,
        cases: Vec<ReproducibilitySemanticsCaseResult>,
    ) -> Self {
        let current_scope_window = current_scope_window.into();
        let report_digest = stable_reproducibility_semantics_digest(
            current_scope_window.as_str(),
            cases.as_slice(),
        );
        Self {
            schema_version: 1,
            current_scope_window,
            cases,
            report_digest,
        }
    }

    /// Returns stable signature lines suitable for fixtures or audits.
    #[must_use]
    pub fn stable_signature_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("schema_version={}", self.schema_version),
            format!("current_scope_window={}", self.current_scope_window),
            format!("report_digest={}", self.report_digest),
        ];
        for case in &self.cases {
            lines.push(format!(
                "{}|{:?}|{:?}",
                case.case_id, case.scope, case.status
            ));
        }
        lines
    }
}

/// Builds the canonical framework-wide reproducibility semantics report over
/// the current training and runtime substrate.
#[must_use]
pub fn builtin_reproducibility_semantics_report() -> ReproducibilitySemanticsReport {
    let seed_discipline = TrainingReplaySeedDiscipline::new(41, 77, 99);
    let assignment_contract = seed_discipline.assignment_runtime_contract();
    let trainer_contract = seed_discipline.trainer_runtime_contract();
    let eval_contract = seed_discipline.eval_runtime_contract();
    let local_eval_generator = seed_discipline
        .derive_eval_local_device_generator("cuda:0")
        .expect("seeded eval contract should derive a local generator");
    let distributed_trainer_generator = seed_discipline
        .derive_trainer_distributed_rank_generator("tensor_parallel", 1, 2)
        .expect("strict trainer contract should derive a distributed generator");
    let checkpoint = TrainingCheckpointReference::new(
        "train.reproducibility",
        "stream://reproducibility/step-7",
        "manifest-7",
        "object-7",
        "trainer-a",
        1,
        "cluster-a",
        "topology-a",
        7_000,
    )
    .with_checkpoint_ref("checkpoint://reproducibility/7")
    .with_step(7);
    let sampler_policy = SamplingPolicy {
        strategy: SamplingStrategy::Sample,
        temperature: Some(0.7),
        top_k: Some(2),
        top_p: Some(0.95),
        repeat_penalty: None,
        presence_penalty: None,
        frequency_penalty: None,
        seed: None,
    };
    let mut sampler = TokenSampler::from_determinism_contract(&sampler_policy, &eval_contract)
        .expect("strict eval contract should build a seeded sampler");
    let _ = sampler.select_next_token(&[0.2, 1.4, 0.3], &[1]);
    let checkpoint_contract = RuntimeDeterminismContract {
        generator: sampler.generator_state(),
        ..eval_contract.clone()
    };
    let restored_contract = checkpoint_contract
        .checkpoint_state(checkpoint)
        .expect("strict determinism contract should checkpoint")
        .restore();
    let missing_generator_contract = RuntimeDeterminismContract {
        mode: DeterminismMode::Strict,
        algorithm_policy: trainer_contract.algorithm_policy,
        generator: None,
    };
    let missing_generator_refusal = missing_generator_contract
        .validate()
        .expect_err("strict mode without a generator should refuse")
        .refusal();
    let invalid_rank_refusal = seed_discipline
        .derive_trainer_distributed_rank_generator("tensor_parallel", 2, 2)
        .expect_err("invalid distributed rank should refuse")
        .refusal();

    ReproducibilitySemanticsReport::new(
        String::from("psionic_reproducibility_v1"),
        vec![
            supported_reproducibility_case(
                "assignment.seeded_replay_contract",
                ReproducibilitySemanticsScope::Assignment,
                seed_discipline,
                Some(assignment_contract),
                None,
                None,
                "Assignment-time randomness is bounded to one replayable seeded contract so worker-selection and claim derivation do not rely on lane-local seed math.",
            ),
            supported_reproducibility_case(
                "trainer.strict_contract",
                ReproducibilitySemanticsScope::Trainer,
                seed_discipline,
                Some(trainer_contract),
                None,
                None,
                "Trainer-side randomness is bounded to one strict runtime determinism contract seeded from the trainer replay discipline.",
            ),
            supported_reproducibility_case(
                "eval.strict_contract",
                ReproducibilitySemanticsScope::Eval,
                seed_discipline,
                Some(eval_contract.clone()),
                None,
                None,
                "Eval-time randomness is bounded to one strict runtime determinism contract seeded from the eval replay discipline.",
            ),
            supported_reproducibility_case(
                "eval.local_device_generator",
                ReproducibilitySemanticsScope::LocalDevice,
                seed_discipline,
                Some(eval_contract),
                Some(local_eval_generator),
                None,
                "Per-device eval RNG derivation is stable and machine-legible instead of being left to host-local seed math.",
            ),
            supported_reproducibility_case(
                "trainer.distributed_rank_generator",
                ReproducibilitySemanticsScope::DistributedReplay,
                seed_discipline,
                Some(seed_discipline.trainer_runtime_contract()),
                Some(distributed_trainer_generator),
                None,
                "Distributed trainer RNG derivation is stable across replica-group, rank, and world-size boundaries.",
            ),
            supported_reproducibility_case(
                "eval.checkpoint_restore",
                ReproducibilitySemanticsScope::CheckpointRestore,
                seed_discipline,
                Some(checkpoint_contract),
                None,
                Some(restored_contract),
                "Replayable eval RNG state can be checkpointed after draws and restored later without silently resetting the generator stream.",
            ),
            refused_reproducibility_case(
                "strict.missing_generator_state",
                ReproducibilitySemanticsScope::Trainer,
                seed_discipline,
                Some(missing_generator_contract),
                "Strict determinism without a generator is intentionally refused instead of degrading to best-effort randomness.",
                missing_generator_refusal,
            ),
            refused_reproducibility_case(
                "distributed.invalid_rank",
                ReproducibilitySemanticsScope::DistributedReplay,
                seed_discipline,
                Some(seed_discipline.trainer_runtime_contract()),
                "Distributed replay derivation is intentionally refused when rank and world-size bounds are invalid.",
                invalid_rank_refusal,
            ),
        ],
    )
}

fn supported_reproducibility_case(
    case_id: &str,
    scope: ReproducibilitySemanticsScope,
    seed_discipline: TrainingReplaySeedDiscipline,
    runtime_contract: Option<RuntimeDeterminismContract>,
    derived_generator: Option<GeneratorState>,
    restored_contract: Option<RuntimeDeterminismContract>,
    bounded_scope: &str,
) -> ReproducibilitySemanticsCaseResult {
    ReproducibilitySemanticsCaseResult {
        case_id: String::from(case_id),
        scope,
        status: ReproducibilitySemanticsStatus::Supported,
        seed_discipline,
        runtime_contract,
        derived_generator,
        restored_contract,
        bounded_scope: String::from(bounded_scope),
        refusal: None,
    }
}

fn refused_reproducibility_case(
    case_id: &str,
    scope: ReproducibilitySemanticsScope,
    seed_discipline: TrainingReplaySeedDiscipline,
    runtime_contract: Option<RuntimeDeterminismContract>,
    bounded_scope: &str,
    refusal: psionic_core::PsionicRefusal,
) -> ReproducibilitySemanticsCaseResult {
    ReproducibilitySemanticsCaseResult {
        case_id: String::from(case_id),
        scope,
        status: ReproducibilitySemanticsStatus::Refused,
        seed_discipline,
        runtime_contract,
        derived_generator: None,
        restored_contract: None,
        bounded_scope: String::from(bounded_scope),
        refusal: Some(refusal),
    }
}

fn stable_reproducibility_semantics_digest(
    current_scope_window: &str,
    cases: &[ReproducibilitySemanticsCaseResult],
) -> String {
    let mut lines = vec![format!("current_scope_window={current_scope_window}")];
    for case in cases {
        lines.push(format!("case_id={}", case.case_id));
        lines.push(format!("scope={:?}", case.scope));
        lines.push(format!("status={:?}", case.status));
        lines.push(format!(
            "seed_discipline={}/{}/{}",
            case.seed_discipline.assignment_seed,
            case.seed_discipline.trainer_seed,
            case.seed_discipline.eval_seed
        ));
        lines.push(format!("bounded_scope={}", case.bounded_scope));
        if let Some(contract) = &case.runtime_contract {
            lines.push(format!("contract_mode={:?}", contract.mode));
            lines.push(format!(
                "contract_algorithm_policy={:?}",
                contract.algorithm_policy
            ));
            if let Some(generator) = &contract.generator {
                lines.push(format!("contract_generator_seed={}", generator.seed));
                lines.push(format!("contract_generator_draws={}", generator.draws));
                lines.push(format!("contract_generator_scope={:?}", generator.scope));
            }
        }
        if let Some(generator) = &case.derived_generator {
            lines.push(format!("derived_seed={}", generator.seed));
            lines.push(format!("derived_scope={:?}", generator.scope));
        }
        if let Some(restored) = &case.restored_contract {
            lines.push(format!("restored_mode={:?}", restored.mode));
            if let Some(generator) = &restored.generator {
                lines.push(format!("restored_seed={}", generator.seed));
                lines.push(format!("restored_draws={}", generator.draws));
                lines.push(format!("restored_scope={:?}", generator.scope));
            }
        }
        if let Some(refusal) = &case.refusal {
            lines.push(format!("refusal_code={:?}", refusal.code));
            lines.push(format!("refusal_scope={:?}", refusal.scope));
            lines.push(format!("refusal_detail={}", refusal.detail));
            if let Some(subject) = &refusal.subject {
                lines.push(format!("refusal_subject={subject}"));
            }
        }
    }
    lines.sort();
    let mut hasher = Sha256::new();
    for line in lines {
        hasher.update(line.as_bytes());
        hasher.update(b"\n");
    }
    hex::encode(hasher.finalize())
}

/// One deterministic sample-selection rule bound to replay state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeterministicSampleSelectionRule {
    /// Stable worker identifier.
    pub worker_id: String,
    /// Stable attempt index under the same assignment.
    pub attempt_index: u32,
    /// Stable assignment digest the rule is bound to.
    pub assignment_digest: String,
    /// Stable digest over the candidate population from which selection occurred.
    pub population_digest: String,
    /// Stable selected item identifiers in replay order.
    pub selected_item_ids: Vec<String>,
    /// Derived sample-selection seed.
    pub sample_selection_seed: u64,
    /// Stable digest over the complete selection rule.
    pub selection_digest: String,
}

impl DeterministicSampleSelectionRule {
    /// Derives a sample-selection rule from an assignment seed plus selection inputs.
    pub fn derive(
        assignment_seed: u64,
        assignment_digest: impl Into<String>,
        worker_id: impl Into<String>,
        attempt_index: u32,
        population: Vec<String>,
        selected_item_ids: Vec<String>,
    ) -> Result<Self, TrainingReplayError> {
        let assignment_digest = assignment_digest.into();
        let worker_id = worker_id.into();
        if selected_item_ids.is_empty() {
            return Err(TrainingReplayError::EmptySelectedItems {
                worker_id,
                attempt_index,
            });
        }
        let population_digest = stable_string_list_digest(
            b"psionic_replay_selection_population|",
            population.as_slice(),
        );
        let sample_selection_seed = stable_sample_selection_seed(
            assignment_seed,
            assignment_digest.as_str(),
            worker_id.as_str(),
            attempt_index,
        );
        let selection_digest = stable_selection_digest(
            assignment_digest.as_str(),
            worker_id.as_str(),
            attempt_index,
            sample_selection_seed,
            population_digest.as_str(),
            selected_item_ids.as_slice(),
        );
        Ok(Self {
            worker_id,
            attempt_index,
            assignment_digest,
            population_digest,
            selected_item_ids,
            sample_selection_seed,
            selection_digest,
        })
    }
}

/// Tool-version and contract pin surfaced by replay truth.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayToolPin {
    /// Stable tool name.
    pub tool_name: String,
    /// Tool interface family.
    pub interface: EnvironmentToolInterface,
    /// Stable digest over the full tool contract.
    pub contract_digest: String,
    /// Stable version label or fallback contract-version identifier.
    pub version_label: String,
}

/// Reproducible environment package and tool pin.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayEnvironmentPin {
    /// Stable environment package identity.
    pub package_key: EnvironmentPackageKey,
    /// Stable digest over the full environment package contract.
    pub package_digest: String,
    /// Runtime family surfaced by the package entrypoint.
    pub runtime_family: EnvironmentRuntimeFamily,
    /// Stable digest over the execution entrypoint contract.
    pub entrypoint_digest: String,
    /// Versioned tool pins bound to the environment package.
    pub tool_pins: Vec<ReplayToolPin>,
}

impl ReplayEnvironmentPin {
    /// Builds one reproducible environment pin from an environment package.
    pub fn from_package(
        package: &EnvironmentPackageContract,
        tool_versions: &BTreeMap<String, String>,
    ) -> Result<Self, TrainingReplayError> {
        package.validate()?;
        let package_digest = package.stable_digest();
        let entrypoint_digest = digest_json_value(&package.execution);
        let tool_pins = package
            .tools
            .iter()
            .map(|tool| replay_tool_pin(tool, tool_versions))
            .collect();
        Ok(Self {
            package_key: package.key.clone(),
            package_digest,
            runtime_family: package.execution.runtime_family,
            entrypoint_digest,
            tool_pins,
        })
    }
}

/// Reproducible eval posture attached to replay truth.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReproducibleEvalPosture {
    /// Stable digest over the eval run contract.
    pub eval_run_digest: String,
    /// Stable environment package identity.
    pub environment: EnvironmentPackageKey,
    /// Eval mode.
    pub mode: EvalRunMode,
    /// Optional policy revision reference.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_revision_id: Option<String>,
    /// Optional benchmark package storage key.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub benchmark_package_key: Option<String>,
    /// Expected sample count when pinned.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_sample_count: Option<u64>,
    /// Declared scheduler posture.
    pub scheduler_posture: String,
    /// Stable digest over the execution-strategy declaration.
    pub strategy_digest: String,
    /// Stable eval seed used by replay.
    pub eval_seed: u64,
    /// Stable digest over the ordered eval sample identifiers.
    pub sample_ordering_digest: String,
}

impl ReproducibleEvalPosture {
    /// Builds a reproducible eval posture from a validated eval contract.
    pub fn from_eval_contract(
        contract: &EvalRunContract,
        strategy: EvalExecutionStrategyFacts,
        eval_seed: u64,
        sample_order: Vec<String>,
    ) -> Result<Self, TrainingReplayError> {
        contract.validate()?;
        let scheduler_posture = strategy
            .scheduler_posture
            .clone()
            .unwrap_or_else(|| String::from("missing"));
        if scheduler_posture != "deterministic" {
            return Err(TrainingReplayError::NonDeterministicEvalScheduler {
                actual: scheduler_posture,
            });
        }
        Ok(Self {
            eval_run_digest: contract.stable_digest(),
            environment: contract.environment.clone(),
            mode: contract.mode,
            policy_revision_id: contract.policy_revision_id.clone(),
            benchmark_package_key: contract
                .benchmark_package
                .as_ref()
                .map(|package| package.storage_key()),
            expected_sample_count: contract.expected_sample_count,
            scheduler_posture: String::from("deterministic"),
            strategy_digest: digest_json_value(&strategy),
            eval_seed,
            sample_ordering_digest: stable_string_list_digest(
                b"psionic_eval_sample_order|",
                sample_order.as_slice(),
            ),
        })
    }
}

/// High-level replay verification result.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayVerificationDisposition {
    /// All deterministic replay inputs matched.
    Match,
    /// One or more deterministic replay inputs drifted.
    Drifted,
}

/// Reason-code family for replay verification drift.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayVerificationSignalKind {
    /// Trainer batch digest mismatch.
    TrainerBatchDigest,
    /// Assignment seed mismatch.
    AssignmentSeed,
    /// Trainer seed mismatch.
    TrainerSeed,
    /// Eval seed mismatch.
    EvalSeed,
    /// Environment key mismatch.
    EnvironmentKey,
    /// Environment package digest mismatch.
    EnvironmentPackageDigest,
    /// Tool contract digest mismatch.
    ToolContractDigest,
    /// Tool version mismatch.
    ToolVersion,
    /// Eval run digest mismatch.
    EvalRunDigest,
    /// Eval scheduler posture mismatch.
    EvalSchedulerPosture,
    /// Eval sample ordering mismatch.
    EvalSampleOrderingDigest,
    /// Sample-selection seed mismatch.
    SampleSelectionSeed,
    /// Sample-selection digest mismatch.
    SampleSelectionDigest,
}

/// One mismatch signal surfaced during replay verification.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingReplayVerificationSignal {
    /// Reason-code family.
    pub kind: ReplayVerificationSignalKind,
    /// Stable subject of the comparison when one exists.
    pub subject: String,
    /// Expected value from the baseline receipt.
    pub expected: String,
    /// Observed value from the candidate receipt.
    pub observed: String,
}

/// Verification receipt comparing two replay receipts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingReplayVerificationReceipt {
    /// Overall verification outcome.
    pub disposition: ReplayVerificationDisposition,
    /// Baseline replay digest.
    pub expected_replay_digest: String,
    /// Candidate replay digest.
    pub observed_replay_digest: String,
    /// Drift signals surfaced by the comparison.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub signals: Vec<TrainingReplayVerificationSignal>,
}

/// Canonical deterministic replay receipt for one trainer batch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrainingReplayReceipt {
    /// Replayable trainer batch.
    pub trainer_batch: TrainerBatch,
    /// Global replay-seed discipline.
    pub seed_discipline: TrainingReplaySeedDiscipline,
    /// Deterministic sample-selection rules.
    pub sample_selection_rules: Vec<DeterministicSampleSelectionRule>,
    /// Pinned environment package and tools.
    pub environment_pin: ReplayEnvironmentPin,
    /// Reproducible eval posture.
    pub eval_posture: ReproducibleEvalPosture,
    /// Stable digest over the replay contract.
    pub replay_digest: String,
}

impl TrainingReplayReceipt {
    /// Creates and validates a deterministic replay receipt.
    pub fn new(
        trainer_batch: TrainerBatch,
        seed_discipline: TrainingReplaySeedDiscipline,
        mut sample_selection_rules: Vec<DeterministicSampleSelectionRule>,
        environment_pin: ReplayEnvironmentPin,
        eval_posture: ReproducibleEvalPosture,
    ) -> Result<Self, TrainingReplayError> {
        if environment_pin.package_key != eval_posture.environment {
            return Err(TrainingReplayError::EnvironmentEvalMismatch {
                expected: environment_pin.package_key.storage_key(),
                actual: eval_posture.environment.storage_key(),
            });
        }

        sample_selection_rules.sort_by(|left, right| {
            left.worker_id
                .cmp(&right.worker_id)
                .then(left.attempt_index.cmp(&right.attempt_index))
        });
        let mut subjects = BTreeSet::new();
        for rule in &sample_selection_rules {
            if !subjects.insert((rule.worker_id.clone(), rule.attempt_index)) {
                return Err(TrainingReplayError::DuplicateSampleSelectionRule {
                    worker_id: rule.worker_id.clone(),
                    attempt_index: rule.attempt_index,
                });
            }
        }

        let replay_digest = stable_replay_digest(
            trainer_batch.batch_digest.as_str(),
            &seed_discipline,
            sample_selection_rules.as_slice(),
            &environment_pin,
            &eval_posture,
        );
        Ok(Self {
            trainer_batch,
            seed_discipline,
            sample_selection_rules,
            environment_pin,
            eval_posture,
            replay_digest,
        })
    }

    /// Compares another replay receipt against this one and surfaces drift signals.
    #[must_use]
    pub fn verify_against(&self, observed: &Self) -> TrainingReplayVerificationReceipt {
        let mut signals = Vec::new();
        compare_signal(
            &mut signals,
            ReplayVerificationSignalKind::TrainerBatchDigest,
            "trainer_batch",
            self.trainer_batch.batch_digest.as_str(),
            observed.trainer_batch.batch_digest.as_str(),
        );
        compare_signal(
            &mut signals,
            ReplayVerificationSignalKind::AssignmentSeed,
            "seeds",
            self.seed_discipline.assignment_seed.to_string().as_str(),
            observed
                .seed_discipline
                .assignment_seed
                .to_string()
                .as_str(),
        );
        compare_signal(
            &mut signals,
            ReplayVerificationSignalKind::TrainerSeed,
            "seeds",
            self.seed_discipline.trainer_seed.to_string().as_str(),
            observed.seed_discipline.trainer_seed.to_string().as_str(),
        );
        compare_signal(
            &mut signals,
            ReplayVerificationSignalKind::EvalSeed,
            "seeds",
            self.seed_discipline.eval_seed.to_string().as_str(),
            observed.seed_discipline.eval_seed.to_string().as_str(),
        );
        compare_signal(
            &mut signals,
            ReplayVerificationSignalKind::EnvironmentKey,
            "environment",
            self.environment_pin.package_key.storage_key().as_str(),
            observed.environment_pin.package_key.storage_key().as_str(),
        );
        compare_signal(
            &mut signals,
            ReplayVerificationSignalKind::EnvironmentPackageDigest,
            "environment",
            self.environment_pin.package_digest.as_str(),
            observed.environment_pin.package_digest.as_str(),
        );
        compare_signal(
            &mut signals,
            ReplayVerificationSignalKind::EvalRunDigest,
            "eval",
            self.eval_posture.eval_run_digest.as_str(),
            observed.eval_posture.eval_run_digest.as_str(),
        );
        compare_signal(
            &mut signals,
            ReplayVerificationSignalKind::EvalSchedulerPosture,
            "eval",
            self.eval_posture.scheduler_posture.as_str(),
            observed.eval_posture.scheduler_posture.as_str(),
        );
        compare_signal(
            &mut signals,
            ReplayVerificationSignalKind::EvalSampleOrderingDigest,
            "eval",
            self.eval_posture.sample_ordering_digest.as_str(),
            observed.eval_posture.sample_ordering_digest.as_str(),
        );

        compare_tool_pins(
            &mut signals,
            &self.environment_pin.tool_pins,
            &observed.environment_pin.tool_pins,
        );
        compare_sample_selection_rules(
            &mut signals,
            self.sample_selection_rules.as_slice(),
            observed.sample_selection_rules.as_slice(),
        );

        TrainingReplayVerificationReceipt {
            disposition: if signals.is_empty() {
                ReplayVerificationDisposition::Match
            } else {
                ReplayVerificationDisposition::Drifted
            },
            expected_replay_digest: self.replay_digest.clone(),
            observed_replay_digest: observed.replay_digest.clone(),
            signals,
        }
    }
}

fn replay_tool_pin(
    tool: &EnvironmentToolContract,
    tool_versions: &BTreeMap<String, String>,
) -> ReplayToolPin {
    let contract_digest = digest_json_value(tool);
    let version_label = tool_versions
        .get(tool.tool_name.as_str())
        .cloned()
        .unwrap_or_else(|| format!("contract_digest:{contract_digest}"));
    ReplayToolPin {
        tool_name: tool.tool_name.clone(),
        interface: tool.interface,
        contract_digest,
        version_label,
    }
}

fn compare_tool_pins(
    signals: &mut Vec<TrainingReplayVerificationSignal>,
    expected: &[ReplayToolPin],
    observed: &[ReplayToolPin],
) {
    let expected = expected
        .iter()
        .map(|tool| (tool.tool_name.as_str(), tool))
        .collect::<BTreeMap<_, _>>();
    let observed = observed
        .iter()
        .map(|tool| (tool.tool_name.as_str(), tool))
        .collect::<BTreeMap<_, _>>();
    let names = expected
        .keys()
        .chain(observed.keys())
        .copied()
        .collect::<BTreeSet<_>>();
    for tool_name in names {
        let expected_tool = expected.get(tool_name);
        let observed_tool = observed.get(tool_name);
        compare_signal(
            signals,
            ReplayVerificationSignalKind::ToolContractDigest,
            tool_name,
            expected_tool
                .map(|tool| tool.contract_digest.as_str())
                .unwrap_or("<missing>"),
            observed_tool
                .map(|tool| tool.contract_digest.as_str())
                .unwrap_or("<missing>"),
        );
        compare_signal(
            signals,
            ReplayVerificationSignalKind::ToolVersion,
            tool_name,
            expected_tool
                .map(|tool| tool.version_label.as_str())
                .unwrap_or("<missing>"),
            observed_tool
                .map(|tool| tool.version_label.as_str())
                .unwrap_or("<missing>"),
        );
    }
}

fn compare_sample_selection_rules(
    signals: &mut Vec<TrainingReplayVerificationSignal>,
    expected: &[DeterministicSampleSelectionRule],
    observed: &[DeterministicSampleSelectionRule],
) {
    let expected = expected
        .iter()
        .map(|rule| ((rule.worker_id.as_str(), rule.attempt_index), rule))
        .collect::<BTreeMap<_, _>>();
    let observed = observed
        .iter()
        .map(|rule| ((rule.worker_id.as_str(), rule.attempt_index), rule))
        .collect::<BTreeMap<_, _>>();
    let subjects = expected
        .keys()
        .chain(observed.keys())
        .copied()
        .collect::<BTreeSet<_>>();
    for (worker_id, attempt_index) in subjects {
        let subject = format!("{worker_id}:{attempt_index}");
        let expected_rule = expected.get(&(worker_id, attempt_index));
        let observed_rule = observed.get(&(worker_id, attempt_index));
        compare_signal(
            signals,
            ReplayVerificationSignalKind::SampleSelectionSeed,
            subject.as_str(),
            expected_rule
                .map(|rule| rule.sample_selection_seed.to_string())
                .unwrap_or_else(|| String::from("<missing>"))
                .as_str(),
            observed_rule
                .map(|rule| rule.sample_selection_seed.to_string())
                .unwrap_or_else(|| String::from("<missing>"))
                .as_str(),
        );
        compare_signal(
            signals,
            ReplayVerificationSignalKind::SampleSelectionDigest,
            subject.as_str(),
            expected_rule
                .map(|rule| rule.selection_digest.as_str())
                .unwrap_or("<missing>"),
            observed_rule
                .map(|rule| rule.selection_digest.as_str())
                .unwrap_or("<missing>"),
        );
    }
}

fn compare_signal(
    signals: &mut Vec<TrainingReplayVerificationSignal>,
    kind: ReplayVerificationSignalKind,
    subject: &str,
    expected: &str,
    observed: &str,
) {
    if expected != observed {
        signals.push(TrainingReplayVerificationSignal {
            kind,
            subject: String::from(subject),
            expected: String::from(expected),
            observed: String::from(observed),
        });
    }
}

fn stable_replay_digest(
    trainer_batch_digest: &str,
    seed_discipline: &TrainingReplaySeedDiscipline,
    sample_selection_rules: &[DeterministicSampleSelectionRule],
    environment_pin: &ReplayEnvironmentPin,
    eval_posture: &ReproducibleEvalPosture,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_training_replay_truth|");
    hasher.update(trainer_batch_digest.as_bytes());
    hasher.update(stable_json_bytes(seed_discipline));
    hasher.update(environment_pin.package_key.storage_key().as_bytes());
    hasher.update(environment_pin.package_digest.as_bytes());
    hasher.update(environment_pin.entrypoint_digest.as_bytes());
    for tool in &environment_pin.tool_pins {
        hasher.update(tool.tool_name.as_bytes());
        hasher.update(tool.contract_digest.as_bytes());
        hasher.update(tool.version_label.as_bytes());
    }
    hasher.update(eval_posture.eval_run_digest.as_bytes());
    hasher.update(eval_posture.scheduler_posture.as_bytes());
    hasher.update(eval_posture.strategy_digest.as_bytes());
    hasher.update(eval_posture.eval_seed.to_string().as_bytes());
    hasher.update(eval_posture.sample_ordering_digest.as_bytes());
    for rule in sample_selection_rules {
        hasher.update(rule.selection_digest.as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn stable_sample_selection_seed(
    assignment_seed: u64,
    assignment_digest: &str,
    worker_id: &str,
    attempt_index: u32,
) -> u64 {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_rollout_sample_selection_seed|");
    hasher.update(assignment_seed.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(assignment_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(worker_id.as_bytes());
    hasher.update(b"|");
    hasher.update(attempt_index.to_string().as_bytes());
    let digest = hasher.finalize();
    let mut bytes = [0_u8; 8];
    bytes.copy_from_slice(&digest[..8]);
    u64::from_le_bytes(bytes)
}

fn stable_selection_digest(
    assignment_digest: &str,
    worker_id: &str,
    attempt_index: u32,
    sample_selection_seed: u64,
    population_digest: &str,
    selected_item_ids: &[String],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_replay_selection_rule|");
    hasher.update(assignment_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(worker_id.as_bytes());
    hasher.update(b"|");
    hasher.update(attempt_index.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(sample_selection_seed.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(population_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(stable_string_list_digest(
        b"psionic_replay_selected_items|",
        selected_item_ids,
    ));
    hex::encode(hasher.finalize())
}

fn stable_string_list_digest(prefix: &[u8], items: &[String]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    for item in items {
        hasher.update(item.as_bytes());
        hasher.update(b"|");
    }
    hex::encode(hasher.finalize())
}

fn digest_json_value(value: &impl Serialize) -> String {
    hex::encode(Sha256::digest(stable_json_bytes(value)))
}

fn stable_json_bytes(value: &impl Serialize) -> Vec<u8> {
    serde_json::to_vec(value).expect("stable JSON serialization failed")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::{
        PolicyRevision, RolloutArtifact, RolloutProofKind, RolloutProofReference, RolloutSample,
        RolloutTerminationReason,
    };
    use psionic_environments::{
        EnvironmentExecutionEntrypoint, EnvironmentPackageFamily, EnvironmentStateMode,
        EnvironmentToolContract,
    };

    use super::*;

    #[test]
    fn replay_truth_receipt_is_machine_legible_and_verifiable(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let package = sample_environment_package();
        let batch = sample_trainer_batch(&package.key)?;
        let seed_discipline = TrainingReplaySeedDiscipline::new(41, 77, 99);
        let environment_pin =
            ReplayEnvironmentPin::from_package(&package, &sample_tool_versions("tool@1"))?;
        let eval_posture = ReproducibleEvalPosture::from_eval_contract(
            &sample_eval_contract(&package.key),
            deterministic_eval_strategy(),
            seed_discipline.eval_seed,
            vec![String::from("sample-1"), String::from("sample-2")],
        )?;
        let sample_selection_rules = vec![
            DeterministicSampleSelectionRule::derive(
                seed_discipline.assignment_seed,
                "assignment-a",
                "worker-a",
                0,
                vec![String::from("task-a"), String::from("task-b")],
                vec![String::from("task-a")],
            )?,
            DeterministicSampleSelectionRule::derive(
                seed_discipline.assignment_seed,
                "assignment-b",
                "worker-b",
                0,
                vec![String::from("task-c"), String::from("task-d")],
                vec![String::from("task-c")],
            )?,
        ];

        let receipt = TrainingReplayReceipt::new(
            batch.clone(),
            seed_discipline,
            sample_selection_rules,
            environment_pin.clone(),
            eval_posture.clone(),
        )?;
        assert!(!receipt.replay_digest.is_empty());
        assert_eq!(
            receipt.environment_pin.package_digest,
            package.stable_digest()
        );
        assert_eq!(receipt.environment_pin.tool_pins[0].version_label, "tool@1");

        let verification = receipt.verify_against(&TrainingReplayReceipt::new(
            batch,
            seed_discipline,
            receipt.sample_selection_rules.clone(),
            environment_pin,
            eval_posture,
        )?);
        assert_eq!(
            verification.disposition,
            ReplayVerificationDisposition::Match
        );
        assert!(verification.signals.is_empty());
        Ok(())
    }

    #[test]
    fn replay_truth_verification_detects_seed_tool_and_order_drift(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let package = sample_environment_package();
        let batch = sample_trainer_batch(&package.key)?;
        let baseline_seed_discipline = TrainingReplaySeedDiscipline::new(41, 77, 99);
        let baseline_receipt = TrainingReplayReceipt::new(
            batch.clone(),
            baseline_seed_discipline,
            vec![DeterministicSampleSelectionRule::derive(
                baseline_seed_discipline.assignment_seed,
                "assignment-a",
                "worker-a",
                0,
                vec![String::from("task-a"), String::from("task-b")],
                vec![String::from("task-a")],
            )?],
            ReplayEnvironmentPin::from_package(&package, &sample_tool_versions("tool@1"))?,
            ReproducibleEvalPosture::from_eval_contract(
                &sample_eval_contract(&package.key),
                deterministic_eval_strategy(),
                baseline_seed_discipline.eval_seed,
                vec![String::from("sample-1"), String::from("sample-2")],
            )?,
        )?;

        let drifted_seed_discipline = TrainingReplaySeedDiscipline::new(41, 78, 99);
        let drifted_receipt = TrainingReplayReceipt::new(
            batch,
            drifted_seed_discipline,
            vec![DeterministicSampleSelectionRule::derive(
                drifted_seed_discipline.assignment_seed,
                "assignment-a",
                "worker-a",
                0,
                vec![String::from("task-a"), String::from("task-b")],
                vec![String::from("task-b")],
            )?],
            ReplayEnvironmentPin::from_package(&package, &sample_tool_versions("tool@2"))?,
            ReproducibleEvalPosture::from_eval_contract(
                &sample_eval_contract(&package.key),
                deterministic_eval_strategy(),
                drifted_seed_discipline.eval_seed,
                vec![String::from("sample-2"), String::from("sample-1")],
            )?,
        )?;

        let verification = baseline_receipt.verify_against(&drifted_receipt);
        assert_eq!(
            verification.disposition,
            ReplayVerificationDisposition::Drifted
        );
        assert!(verification
            .signals
            .iter()
            .any(|signal| signal.kind == ReplayVerificationSignalKind::TrainerSeed));
        assert!(verification
            .signals
            .iter()
            .any(|signal| signal.kind == ReplayVerificationSignalKind::ToolVersion));
        assert!(verification.signals.iter().any(|signal| {
            signal.kind == ReplayVerificationSignalKind::EvalSampleOrderingDigest
        }));
        assert!(verification
            .signals
            .iter()
            .any(|signal| { signal.kind == ReplayVerificationSignalKind::SampleSelectionDigest }));
        Ok(())
    }

    #[test]
    fn reproducible_eval_posture_requires_deterministic_scheduler(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let package = sample_environment_package();
        let error = ReproducibleEvalPosture::from_eval_contract(
            &sample_eval_contract(&package.key),
            EvalExecutionStrategyFacts {
                strategy_label: String::from("validator"),
                runtime_family: Some(String::from("sandbox")),
                scheduler_posture: Some(String::from("adaptive")),
            },
            99,
            vec![String::from("sample-1")],
        )
        .expect_err("non-deterministic scheduler should fail");
        assert!(matches!(
            error,
            TrainingReplayError::NonDeterministicEvalScheduler { actual }
                if actual == "adaptive"
        ));
        Ok(())
    }

    #[test]
    fn reproducibility_semantics_report_tracks_seeded_runtime_and_replay_cases() {
        let report = builtin_reproducibility_semantics_report();
        assert_eq!(report.schema_version, 1);
        assert_eq!(report.current_scope_window, "psionic_reproducibility_v1");
        assert!(report
            .stable_signature_lines()
            .iter()
            .any(|line| line.starts_with("report_digest=")));

        let trainer_case = report
            .cases
            .iter()
            .find(|case| case.case_id == "trainer.strict_contract")
            .expect("missing trainer strict contract case");
        assert_eq!(
            trainer_case.status,
            ReproducibilitySemanticsStatus::Supported
        );
        assert_eq!(
            trainer_case
                .runtime_contract
                .as_ref()
                .map(|contract| contract.mode),
            Some(DeterminismMode::Strict)
        );

        let distributed_case = report
            .cases
            .iter()
            .find(|case| case.case_id == "trainer.distributed_rank_generator")
            .expect("missing distributed replay case");
        assert_eq!(
            distributed_case.status,
            ReproducibilitySemanticsStatus::Supported
        );
        assert_eq!(
            distributed_case
                .derived_generator
                .as_ref()
                .map(|generator| generator.draws),
            Some(0)
        );

        let checkpoint_case = report
            .cases
            .iter()
            .find(|case| case.case_id == "eval.checkpoint_restore")
            .expect("missing checkpoint restore case");
        assert_eq!(
            checkpoint_case.status,
            ReproducibilitySemanticsStatus::Supported
        );
        assert_eq!(
            checkpoint_case
                .restored_contract
                .as_ref()
                .and_then(|contract| contract.generator.as_ref())
                .map(|generator| generator.draws),
            Some(1)
        );

        let missing_generator_case = report
            .cases
            .iter()
            .find(|case| case.case_id == "strict.missing_generator_state")
            .expect("missing strict missing-generator refusal");
        assert_eq!(
            missing_generator_case.status,
            ReproducibilitySemanticsStatus::Refused
        );
        assert_eq!(
            missing_generator_case
                .refusal
                .as_ref()
                .map(|refusal| refusal.code),
            Some(psionic_core::PsionicRefusalCode::UnsupportedBackendCapability)
        );

        let invalid_rank_case = report
            .cases
            .iter()
            .find(|case| case.case_id == "distributed.invalid_rank")
            .expect("missing invalid-rank refusal");
        assert_eq!(
            invalid_rank_case.status,
            ReproducibilitySemanticsStatus::Refused
        );
        assert_eq!(
            invalid_rank_case
                .refusal
                .as_ref()
                .map(|refusal| refusal.code),
            Some(psionic_core::PsionicRefusalCode::TopologyMismatch)
        );

        let seed_discipline = TrainingReplaySeedDiscipline::new(41, 77, 99);
        let eval_generator = seed_discipline
            .derive_eval_local_device_generator("cuda:0")
            .expect("seeded eval contract should derive a local generator");
        assert_eq!(eval_generator.draws, 0);
        let distributed_generator = seed_discipline
            .derive_trainer_distributed_rank_generator("tensor_parallel", 1, 2)
            .expect("strict trainer contract should derive a distributed generator");
        assert_eq!(distributed_generator.draws, 0);
    }

    fn sample_environment_package() -> EnvironmentPackageContract {
        EnvironmentPackageContract::new(
            EnvironmentPackageKey::new("weather.agent", "1.0.0"),
            EnvironmentPackageFamily::Agentic,
            "Weather Agent",
            EnvironmentExecutionEntrypoint {
                runtime_family: EnvironmentRuntimeFamily::MultiTurnDialog,
                entrypoint: String::from("weather.run"),
                args: vec![String::from("--local")],
                sandbox_profile_ref: None,
                max_turns: 4,
                state_mode: EnvironmentStateMode::SessionPersistent,
                time_budget_ms: Some(5_000),
            },
        )
        .with_tools(vec![EnvironmentToolContract {
            tool_name: String::from("get_weather"),
            interface: EnvironmentToolInterface::NativeFunction,
            description: String::from("Fetch the weather for one city"),
            args_schema: json!({"type": "object", "required": ["city"]}),
            result_schema: Some(json!({"type": "object"})),
        }])
    }

    fn sample_eval_contract(environment: &EnvironmentPackageKey) -> EvalRunContract {
        EvalRunContract::new(
            "eval-weather-1",
            EvalRunMode::OfflineHeldOut,
            environment.clone(),
        )
        .with_expected_sample_count(2)
    }

    fn deterministic_eval_strategy() -> EvalExecutionStrategyFacts {
        EvalExecutionStrategyFacts {
            strategy_label: String::from("validator"),
            runtime_family: Some(String::from("sandbox")),
            scheduler_posture: Some(String::from("deterministic")),
        }
    }

    fn sample_tool_versions(version: &str) -> BTreeMap<String, String> {
        BTreeMap::from([(String::from("get_weather"), String::from(version))])
    }

    fn sample_trainer_batch(
        environment: &EnvironmentPackageKey,
    ) -> Result<TrainerBatch, Box<dyn std::error::Error>> {
        let target_revision =
            PolicyRevision::new("weather.policy", "rev-2", "target-policy-digest", 2_000)
                .with_revision_number(2);
        let source_revision =
            PolicyRevision::new("weather.policy", "rev-1", "source-policy-digest", 1_000)
                .with_revision_number(1);
        let rollout_a = RolloutArtifact::new(
            "rollout-a",
            "worker-a",
            environment.clone(),
            "task-a",
            source_revision.clone(),
            vec![RolloutSample::new(1, -0.2, 0.8, 0.6)],
            RolloutTerminationReason::Completed,
            vec![RolloutProofReference::new(
                RolloutProofKind::ExecutionProof,
                "proof-a",
                "proof://a",
            )],
            3_000,
        )?;
        let rollout_b = RolloutArtifact::new(
            "rollout-b",
            "worker-b",
            environment.clone(),
            "task-b",
            source_revision,
            vec![RolloutSample::new(2, -0.3, 0.5, 0.4)],
            RolloutTerminationReason::Completed,
            vec![RolloutProofReference::new(
                RolloutProofKind::ExecutionProof,
                "proof-b",
                "proof://b",
            )],
            3_100,
        )?;
        Ok(TrainerBatch::assemble(
            "trainer-batch-1",
            target_revision,
            vec![rollout_a, rollout_b],
            4_000,
        )?)
    }
}
