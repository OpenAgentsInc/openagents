use std::collections::BTreeMap;

use psionic_core::{DType, Device, Shape, TensorData, TensorSpec};
use psionic_eval::{
    TASSADAR_BENCHMARK_ENVIRONMENT_REF, TASSADAR_REFERENCE_FIXTURE_BENCHMARK_REF,
    TassadarBenchmarkError, TassadarBenchmarkReport, TassadarReferenceFixtureSuite,
    build_tassadar_reference_fixture_suite, run_tassadar_reference_fixture_benchmark,
};
use psionic_models::TassadarExecutorFixture;
use psionic_runtime::{
    TassadarArithmeticOp, TassadarExecution, TassadarExecutionRefusal, TassadarExecutorDecodeMode,
    TassadarFixtureRunner, TassadarHaltReason, TassadarInstruction, TassadarProgram,
    TassadarTraceEvent, TassadarTraceStep, build_tassadar_execution_evidence_bundle,
    tassadar_validation_corpus, tassadar_wasm_profile_for_id,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    FixedBudgetTrainingRun, TrainingCoreError, TrainingGradientBatch, TrainingLoopBudget,
    TrainingOptimizerConfig, TrainingOptimizerResidencyPolicy, TrainingParameterClass,
    TrainingParameterGroupState, TrainingRunOutcome, TrainingStepInput, TrainingTensorBuffer,
};

const ADD_GROUP_ID: &str = "tassadar.add_kernel";
const SUB_GROUP_ID: &str = "tassadar.sub_kernel";
const MUL_GROUP_ID: &str = "tassadar.mul_kernel";
const TASSADAR_SMALL_EXECUTOR_MODEL_FAMILY: &str = "tassadar_small_executor";
const TASSADAR_SMALL_EXECUTOR_RUNNER_ID: &str = "tassadar.small_executor.v0";
const TASSADAR_SMALL_EXECUTOR_BOUNDARY_LABEL: &str = "validation_corpus_only";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ArithmeticKernelKind {
    Add,
    Sub,
    Mul,
    Lt,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct ArithmeticSupervisionExample {
    case_id: String,
    features: Vec<f32>,
    target: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct ArithmeticSupervisionSet {
    add: Vec<ArithmeticSupervisionExample>,
    sub: Vec<ArithmeticSupervisionExample>,
    mul: Vec<ArithmeticSupervisionExample>,
}

/// Typed configuration for the Phase 9B small-model Tassadar training lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSmallExecutorTrainingConfig {
    /// Stable training run identifier.
    pub run_id: String,
    /// Stable checkpoint family for the training run.
    pub checkpoint_family: String,
    /// Benchmark package version to bind into the training/eval run.
    pub benchmark_version: String,
    /// Maximum fixed-budget training steps to execute.
    pub max_steps: u64,
    /// Logical base timestamp used for replay-stable step receipts.
    pub base_time_ms: u64,
}

impl TassadarSmallExecutorTrainingConfig {
    /// Returns the canonical bounded Phase 9B training config.
    #[must_use]
    pub fn reference() -> Self {
        Self {
            run_id: String::from("tassadar-small-executor-train-v0"),
            checkpoint_family: String::from("train.tassadar.small_executor"),
            benchmark_version: String::from("train-v0"),
            max_steps: 3,
            base_time_ms: 20_000,
        }
    }
}

impl Default for TassadarSmallExecutorTrainingConfig {
    fn default() -> Self {
        Self::reference()
    }
}

/// Explicit claim scope for one trained small executor model.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSmallExecutorClaimScope {
    /// Benchmark package the model was trained and validated against.
    pub benchmark_ref: String,
    /// Environment ref bound to the benchmark package.
    pub environment_ref: String,
    /// Reference fixture model used as the authoritative baseline.
    pub reference_fixture_model_id: String,
    /// Stable ordered case ids inside the validated workload envelope.
    pub validated_case_ids: Vec<String>,
    /// Stable digest over the workload envelope.
    pub workload_scope_digest: String,
    /// Human-readable boundary label for the claim.
    pub boundary_label: String,
}

impl TassadarSmallExecutorClaimScope {
    fn new(
        suite: &TassadarReferenceFixtureSuite,
        reference_fixture_model_id: &str,
        validated_case_ids: Vec<String>,
    ) -> Self {
        let workload_scope_digest = stable_digest(
            b"tassadar_small_executor_claim_scope|",
            &(
                TASSADAR_REFERENCE_FIXTURE_BENCHMARK_REF,
                TASSADAR_BENCHMARK_ENVIRONMENT_REF,
                reference_fixture_model_id,
                suite.corpus_digest.as_str(),
                &validated_case_ids,
                TASSADAR_SMALL_EXECUTOR_BOUNDARY_LABEL,
            ),
        );
        Self {
            benchmark_ref: String::from(TASSADAR_REFERENCE_FIXTURE_BENCHMARK_REF),
            environment_ref: String::from(TASSADAR_BENCHMARK_ENVIRONMENT_REF),
            reference_fixture_model_id: String::from(reference_fixture_model_id),
            validated_case_ids,
            workload_scope_digest,
            boundary_label: String::from(TASSADAR_SMALL_EXECUTOR_BOUNDARY_LABEL),
        }
    }
}

/// Trained bounded small-model executor for the current validation workload envelope.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarSmallExecutorModel {
    /// Stable model identifier.
    pub model_id: String,
    /// Stable training run identifier.
    pub training_run_id: String,
    /// Stable checkpoint family.
    pub checkpoint_family: String,
    /// Explicit bounded claim scope.
    pub claim_scope: TassadarSmallExecutorClaimScope,
    /// Learned add kernel weights over `[lhs, rhs]`.
    pub add_kernel: Vec<f32>,
    /// Learned sub kernel weights over `[lhs, rhs]`.
    pub sub_kernel: Vec<f32>,
    /// Learned mul kernel weight over `[lhs * rhs]`.
    pub mul_kernel: Vec<f32>,
}

impl TassadarSmallExecutorModel {
    /// Returns a stable digest over the trained model.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_digest(b"tassadar_small_executor_model|", self)
    }

    /// Executes one validated Tassadar program through the bounded learned kernels.
    pub fn execute(
        &self,
        program: &TassadarProgram,
    ) -> Result<TassadarExecution, TassadarExecutionRefusal> {
        let Some(profile) = tassadar_wasm_profile_for_id(program.profile_id.as_str()) else {
            return Err(TassadarExecutionRefusal::ProfileMismatch {
                expected: String::from("tassadar.wasm.core_i32.v1"),
                actual: program.profile_id.clone(),
            });
        };
        if program.instructions.len() > profile.max_program_len {
            return Err(TassadarExecutionRefusal::ProgramTooLong {
                instruction_count: program.instructions.len(),
                max_supported: profile.max_program_len,
            });
        }
        if program.local_count > profile.max_locals {
            return Err(TassadarExecutionRefusal::TooManyLocals {
                requested: program.local_count,
                max_supported: profile.max_locals,
            });
        }
        if program.memory_slots > profile.max_memory_slots {
            return Err(TassadarExecutionRefusal::TooManyMemorySlots {
                requested: program.memory_slots,
                max_supported: profile.max_memory_slots,
            });
        }
        if program.initial_memory.len() != program.memory_slots {
            return Err(TassadarExecutionRefusal::InitialMemoryShapeMismatch {
                expected: program.memory_slots,
                actual: program.initial_memory.len(),
            });
        }

        let trace_abi =
            psionic_runtime::tassadar_trace_abi_for_profile_id(program.profile_id.as_str())
                .expect("supported profile should have a trace ABI");
        let mut stack = Vec::new();
        let mut locals = vec![0; program.local_count];
        let mut memory = program.initial_memory.clone();
        let mut outputs = Vec::new();
        let mut steps = Vec::new();
        let mut pc = 0usize;
        let mut step_index = 0usize;
        let halt_reason = loop {
            if step_index >= profile.max_steps {
                return Err(TassadarExecutionRefusal::StepLimitExceeded {
                    max_steps: profile.max_steps,
                });
            }
            if pc >= program.instructions.len() {
                break TassadarHaltReason::FellOffEnd;
            }
            let instruction = program.instructions[pc].clone();
            let stack_before = stack.clone();
            let (event, next_pc) = match instruction.clone() {
                TassadarInstruction::I32Const { value } => {
                    stack.push(value);
                    (TassadarTraceEvent::ConstPush { value }, pc + 1)
                }
                TassadarInstruction::LocalGet { local } => {
                    let local_index = usize::from(local);
                    let value = *locals.get(local_index).ok_or(
                        TassadarExecutionRefusal::LocalOutOfRange {
                            pc,
                            local: local_index,
                            local_count: program.local_count,
                        },
                    )?;
                    stack.push(value);
                    (TassadarTraceEvent::LocalGet { local, value }, pc + 1)
                }
                TassadarInstruction::LocalSet { local } => {
                    let local_index = usize::from(local);
                    let value = stack
                        .pop()
                        .ok_or(TassadarExecutionRefusal::StackUnderflow {
                            pc,
                            needed: 1,
                            available: 0,
                        })?;
                    let destination = locals.get_mut(local_index).ok_or(
                        TassadarExecutionRefusal::LocalOutOfRange {
                            pc,
                            local: local_index,
                            local_count: program.local_count,
                        },
                    )?;
                    *destination = value;
                    (TassadarTraceEvent::LocalSet { local, value }, pc + 1)
                }
                TassadarInstruction::I32Add => {
                    let (left, right) = pop_binary_operands(&mut stack, pc)?;
                    let result = self.apply_kernel(ArithmeticKernelKind::Add, left, right);
                    stack.push(result);
                    (
                        TassadarTraceEvent::BinaryOp {
                            op: TassadarArithmeticOp::Add,
                            left,
                            right,
                            result,
                        },
                        pc + 1,
                    )
                }
                TassadarInstruction::I32Sub => {
                    let (left, right) = pop_binary_operands(&mut stack, pc)?;
                    let result = self.apply_kernel(ArithmeticKernelKind::Sub, left, right);
                    stack.push(result);
                    (
                        TassadarTraceEvent::BinaryOp {
                            op: TassadarArithmeticOp::Sub,
                            left,
                            right,
                            result,
                        },
                        pc + 1,
                    )
                }
                TassadarInstruction::I32Mul => {
                    let (left, right) = pop_binary_operands(&mut stack, pc)?;
                    let result = self.apply_kernel(ArithmeticKernelKind::Mul, left, right);
                    stack.push(result);
                    (
                        TassadarTraceEvent::BinaryOp {
                            op: TassadarArithmeticOp::Mul,
                            left,
                            right,
                            result,
                        },
                        pc + 1,
                    )
                }
                TassadarInstruction::I32Lt => {
                    let (left, right) = pop_binary_operands(&mut stack, pc)?;
                    let result = self.apply_kernel(ArithmeticKernelKind::Lt, left, right);
                    stack.push(result);
                    (
                        TassadarTraceEvent::BinaryOp {
                            op: TassadarArithmeticOp::Lt,
                            left,
                            right,
                            result,
                        },
                        pc + 1,
                    )
                }
                TassadarInstruction::I32Load { slot } => {
                    let slot_index = usize::from(slot);
                    let value = *memory.get(slot_index).ok_or(
                        TassadarExecutionRefusal::MemorySlotOutOfRange {
                            pc,
                            slot: slot_index,
                            memory_slots: program.memory_slots,
                        },
                    )?;
                    stack.push(value);
                    (TassadarTraceEvent::Load { slot, value }, pc + 1)
                }
                TassadarInstruction::I32Store { slot } => {
                    let slot_index = usize::from(slot);
                    let value = stack
                        .pop()
                        .ok_or(TassadarExecutionRefusal::StackUnderflow {
                            pc,
                            needed: 1,
                            available: 0,
                        })?;
                    let destination = memory.get_mut(slot_index).ok_or(
                        TassadarExecutionRefusal::MemorySlotOutOfRange {
                            pc,
                            slot: slot_index,
                            memory_slots: program.memory_slots,
                        },
                    )?;
                    *destination = value;
                    (TassadarTraceEvent::Store { slot, value }, pc + 1)
                }
                TassadarInstruction::BrIf { target_pc } => {
                    let condition =
                        stack
                            .pop()
                            .ok_or(TassadarExecutionRefusal::StackUnderflow {
                                pc,
                                needed: 1,
                                available: 0,
                            })?;
                    let target_pc = usize::from(target_pc);
                    if target_pc >= program.instructions.len() {
                        return Err(TassadarExecutionRefusal::InvalidBranchTarget {
                            pc,
                            target_pc,
                            instruction_count: program.instructions.len(),
                        });
                    }
                    let taken = condition != 0;
                    (
                        TassadarTraceEvent::Branch {
                            condition,
                            taken,
                            target_pc,
                        },
                        if taken { target_pc } else { pc + 1 },
                    )
                }
                TassadarInstruction::Output => {
                    let value = stack
                        .pop()
                        .ok_or(TassadarExecutionRefusal::StackUnderflow {
                            pc,
                            needed: 1,
                            available: 0,
                        })?;
                    outputs.push(value);
                    (TassadarTraceEvent::Output { value }, pc + 1)
                }
                TassadarInstruction::Return => {
                    steps.push(TassadarTraceStep {
                        step_index,
                        pc,
                        next_pc: pc + 1,
                        instruction,
                        event: TassadarTraceEvent::Return,
                        stack_before,
                        stack_after: stack.clone(),
                        locals_after: locals.clone(),
                        memory_after: memory.clone(),
                    });
                    break TassadarHaltReason::Returned;
                }
            };
            steps.push(TassadarTraceStep {
                step_index,
                pc,
                next_pc,
                instruction,
                event,
                stack_before,
                stack_after: stack.clone(),
                locals_after: locals.clone(),
                memory_after: memory.clone(),
            });
            step_index = step_index.saturating_add(1);
            pc = next_pc;
        };

        Ok(TassadarExecution {
            program_id: program.program_id.clone(),
            profile_id: program.profile_id.clone(),
            runner_id: String::from(TASSADAR_SMALL_EXECUTOR_RUNNER_ID),
            trace_abi,
            steps,
            outputs,
            final_locals: locals,
            final_memory: memory,
            final_stack: stack,
            halt_reason,
        })
    }

    fn apply_kernel(&self, kernel: ArithmeticKernelKind, left: i32, right: i32) -> i32 {
        let prediction = match kernel {
            ArithmeticKernelKind::Add => {
                self.add_kernel[0] * left as f32 + self.add_kernel[1] * right as f32
            }
            ArithmeticKernelKind::Sub => {
                self.sub_kernel[0] * left as f32 + self.sub_kernel[1] * right as f32
            }
            ArithmeticKernelKind::Mul => self.mul_kernel[0] * (left * right) as f32,
            ArithmeticKernelKind::Lt => f32::from(left < right),
        };
        prediction.round() as i32
    }
}

/// Exactness failure surfaced during trained-model comparison.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarSmallExecutorExactnessFailure {
    /// Final outputs diverged from the handcrafted reference lane.
    FinalOutputMismatch,
    /// The append-only trace diverged from the handcrafted reference lane.
    TraceMismatch,
    /// The terminal halt reason diverged from the handcrafted reference lane.
    HaltMismatch,
}

/// Per-case comparison between the trained small model and the handcrafted reference lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSmallExecutorEvalCaseReport {
    /// Stable benchmark case identifier.
    pub case_id: String,
    /// Stable program identifier.
    pub program_id: String,
    /// Final-output exactness in basis points.
    pub final_output_exactness_bps: u32,
    /// Step-trace exactness in basis points.
    pub step_exactness_bps: u32,
    /// Halt exactness in basis points.
    pub halt_exactness_bps: u32,
    /// Aggregate score in basis points.
    pub score_bps: u32,
    /// Stable reference behavior digest.
    pub reference_behavior_digest: String,
    /// Stable trained-model behavior digest.
    pub trained_behavior_digest: String,
    /// Stable reference trace artifact digest.
    pub reference_trace_artifact_digest: String,
    /// Stable reference trace proof digest.
    pub reference_trace_proof_digest: String,
    /// Stable reference proof-bundle digest.
    pub reference_proof_bundle_digest: String,
    /// Exactness failures that remained after evaluation.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exactness_failures: Vec<TassadarSmallExecutorExactnessFailure>,
}

/// Aggregate evaluation report for one trained small executor model.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSmallExecutorEvalReport {
    /// Benchmark package identity used for training and validation.
    pub benchmark_ref: String,
    /// Environment ref bound to the validation package.
    pub environment_ref: String,
    /// Stable corpus digest for the validated workload envelope.
    pub corpus_digest: String,
    /// Stable scope digest carried by the trained model.
    pub workload_scope_digest: String,
    /// Stable baseline score from the handcrafted reference benchmark.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_baseline_score_bps: Option<u32>,
    /// Stable baseline summary digest from the handcrafted reference benchmark.
    pub reference_baseline_summary_digest: String,
    /// Aggregate exactness score across the bounded workload envelope.
    pub aggregate_score_bps: u32,
    /// Aggregate final-output exactness.
    pub final_output_exactness_bps: u32,
    /// Aggregate step-trace exactness.
    pub step_exactness_bps: u32,
    /// Aggregate halt exactness.
    pub halt_exactness_bps: u32,
    /// Number of cases that remained fully exact.
    pub exact_case_count: u32,
    /// Whether the trained model stayed exact over the whole validated envelope.
    pub full_envelope_exact: bool,
    /// Per-case exactness comparison reports.
    pub case_reports: Vec<TassadarSmallExecutorEvalCaseReport>,
}

/// Receipt for one honest small-model Tassadar training run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarSmallExecutorTrainingReceipt {
    /// Training configuration used for the run.
    pub config: TassadarSmallExecutorTrainingConfig,
    /// Stable package key string for the benchmark suite.
    pub benchmark_ref: String,
    /// Stable environment ref for the benchmark suite.
    pub environment_ref: String,
    /// Training outcome emitted by the fixed-budget training core.
    pub training_outcome: TrainingRunOutcome,
    /// Trained bounded small-model executor.
    pub trained_model: TassadarSmallExecutorModel,
    /// Proof-aware exactness evaluation against the handcrafted reference lane.
    pub evaluation: TassadarSmallExecutorEvalReport,
    /// Stable digest over the receipt.
    pub receipt_digest: String,
}

impl TassadarSmallExecutorTrainingReceipt {
    fn new(
        config: TassadarSmallExecutorTrainingConfig,
        training_outcome: TrainingRunOutcome,
        trained_model: TassadarSmallExecutorModel,
        evaluation: TassadarSmallExecutorEvalReport,
    ) -> Self {
        let mut receipt = Self {
            config,
            benchmark_ref: String::from(TASSADAR_REFERENCE_FIXTURE_BENCHMARK_REF),
            environment_ref: String::from(TASSADAR_BENCHMARK_ENVIRONMENT_REF),
            training_outcome,
            trained_model,
            evaluation,
            receipt_digest: String::new(),
        };
        receipt.receipt_digest =
            stable_digest(b"tassadar_small_executor_training_receipt|", &receipt);
        receipt
    }
}

/// Tassadar small-model training or evaluation failure.
#[derive(Debug, Error)]
pub enum TassadarSmallExecutorTrainingError {
    /// Benchmark package or environment build failed.
    #[error(transparent)]
    Benchmark(#[from] TassadarBenchmarkError),
    /// Fixed-budget training core failed.
    #[error(transparent)]
    TrainingCore(#[from] TrainingCoreError),
    /// The trained model could not execute one validated program.
    #[error(transparent)]
    ExecutionRefusal(#[from] TassadarExecutionRefusal),
    /// The benchmark suite and validation corpus no longer align.
    #[error(
        "Tassadar training suite artifact `{artifact_id}` does not match validation case `{case_id}`"
    )]
    CaseMismatch {
        artifact_id: String,
        case_id: String,
    },
}

/// Trains the bounded Phase 9B small-model executor through the fixed-budget training core.
pub fn train_tassadar_small_executor(
    config: &TassadarSmallExecutorTrainingConfig,
) -> Result<TassadarSmallExecutorTrainingReceipt, TassadarSmallExecutorTrainingError> {
    let suite = build_tassadar_reference_fixture_suite(config.benchmark_version.as_str())?;
    let reference_benchmark =
        run_tassadar_reference_fixture_benchmark(config.benchmark_version.as_str())?;
    let supervision = collect_supervision_examples()?;
    let mut run = FixedBudgetTrainingRun::new(
        config.run_id.clone(),
        config.checkpoint_family.clone(),
        TrainingLoopBudget::new(config.max_steps, config.max_steps.max(1), 1)?,
        build_parameter_groups(&supervision)?,
    )?;

    let mut receipts = Vec::new();
    for step_index in 0..config.max_steps {
        let batch = training_batch_for_step(&run, &supervision, step_index)?;
        if batch.loss <= 1e-9 && step_index > 0 {
            break;
        }
        receipts.push(run.apply_step(TrainingStepInput::new(
            batch,
            config.base_time_ms + step_index * 20,
            config.base_time_ms + step_index * 20 + 10,
        ))?);
    }
    let training_outcome = TrainingRunOutcome {
        receipts,
        summary: run.summary(),
    };
    let trained_model = trained_model_from_run(&run, &suite, &reference_benchmark);
    let evaluation =
        evaluate_tassadar_small_executor(&trained_model, &suite, &reference_benchmark)?;
    Ok(TassadarSmallExecutorTrainingReceipt::new(
        config.clone(),
        training_outcome,
        trained_model,
        evaluation,
    ))
}

/// Evaluates one trained small executor against the handcrafted reference lane.
pub fn evaluate_tassadar_small_executor(
    trained_model: &TassadarSmallExecutorModel,
    suite: &TassadarReferenceFixtureSuite,
    reference_benchmark: &TassadarBenchmarkReport,
) -> Result<TassadarSmallExecutorEvalReport, TassadarSmallExecutorTrainingError> {
    let fixture = TassadarExecutorFixture::new();
    let descriptor = fixture.descriptor();
    let reference_runner = TassadarFixtureRunner::new();
    let corpus = tassadar_validation_corpus();
    let mut case_reports = Vec::new();

    for (case, artifact) in corpus.into_iter().zip(suite.artifacts.iter()) {
        if artifact.validated_program.program_id != case.program.program_id {
            return Err(TassadarSmallExecutorTrainingError::CaseMismatch {
                artifact_id: artifact.artifact_id.clone(),
                case_id: case.case_id,
            });
        }
        let reference_execution = reference_runner.execute(&artifact.validated_program)?;
        let trained_execution = trained_model.execute(&artifact.validated_program)?;
        let evidence = build_tassadar_execution_evidence_bundle(
            format!("{}-reference-proof", case.case_id),
            artifact.artifact_digest.clone(),
            "tassadar_small_executor_training",
            descriptor.model.model_id.clone(),
            descriptor.stable_digest(),
            vec![suite.benchmark_package.key.storage_key()],
            artifact,
            TassadarExecutorDecodeMode::ReferenceLinear,
            &reference_execution,
        );
        let final_output_exactness_bps =
            u32::from(trained_execution.outputs == reference_execution.outputs) * 10_000;
        let step_exactness_bps =
            u32::from(trained_execution.steps == reference_execution.steps) * 10_000;
        let halt_exactness_bps =
            u32::from(trained_execution.halt_reason == reference_execution.halt_reason) * 10_000;
        let score_bps = (final_output_exactness_bps + step_exactness_bps + halt_exactness_bps) / 3;
        let mut exactness_failures = Vec::new();
        if final_output_exactness_bps != 10_000 {
            exactness_failures.push(TassadarSmallExecutorExactnessFailure::FinalOutputMismatch);
        }
        if step_exactness_bps != 10_000 {
            exactness_failures.push(TassadarSmallExecutorExactnessFailure::TraceMismatch);
        }
        if halt_exactness_bps != 10_000 {
            exactness_failures.push(TassadarSmallExecutorExactnessFailure::HaltMismatch);
        }
        case_reports.push(TassadarSmallExecutorEvalCaseReport {
            case_id: case.case_id,
            program_id: artifact.validated_program.program_id.clone(),
            final_output_exactness_bps,
            step_exactness_bps,
            halt_exactness_bps,
            score_bps,
            reference_behavior_digest: reference_execution.behavior_digest(),
            trained_behavior_digest: trained_execution.behavior_digest(),
            reference_trace_artifact_digest: evidence.trace_artifact.artifact_digest.clone(),
            reference_trace_proof_digest: evidence.trace_proof.proof_digest.clone(),
            reference_proof_bundle_digest: evidence.proof_bundle.stable_digest(),
            exactness_failures,
        });
    }

    let case_count = case_reports.len().max(1) as u32;
    let aggregate_score_bps =
        case_reports.iter().map(|case| case.score_bps).sum::<u32>() / case_count;
    let final_output_exactness_bps = case_reports
        .iter()
        .map(|case| case.final_output_exactness_bps)
        .sum::<u32>()
        / case_count;
    let step_exactness_bps = case_reports
        .iter()
        .map(|case| case.step_exactness_bps)
        .sum::<u32>()
        / case_count;
    let halt_exactness_bps = case_reports
        .iter()
        .map(|case| case.halt_exactness_bps)
        .sum::<u32>()
        / case_count;
    let exact_case_count = case_reports
        .iter()
        .filter(|case| case.exactness_failures.is_empty())
        .count() as u32;

    Ok(TassadarSmallExecutorEvalReport {
        benchmark_ref: String::from(TASSADAR_REFERENCE_FIXTURE_BENCHMARK_REF),
        environment_ref: String::from(TASSADAR_BENCHMARK_ENVIRONMENT_REF),
        corpus_digest: suite.corpus_digest.clone(),
        workload_scope_digest: trained_model.claim_scope.workload_scope_digest.clone(),
        reference_baseline_score_bps: reference_benchmark.aggregate_summary.aggregate_score_bps,
        reference_baseline_summary_digest: reference_benchmark
            .aggregate_summary
            .summary_digest
            .clone(),
        aggregate_score_bps,
        final_output_exactness_bps,
        step_exactness_bps,
        halt_exactness_bps,
        exact_case_count,
        full_envelope_exact: exact_case_count as usize == case_reports.len(),
        case_reports,
    })
}

fn collect_supervision_examples()
-> Result<ArithmeticSupervisionSet, TassadarSmallExecutorTrainingError> {
    let runner = TassadarFixtureRunner::new();
    let mut add = Vec::new();
    let mut sub = Vec::new();
    let mut mul = Vec::new();
    for case in tassadar_validation_corpus() {
        let execution = runner.execute(&case.program)?;
        for step in execution.steps {
            if let TassadarTraceEvent::BinaryOp {
                op,
                left,
                right,
                result,
            } = step.event
            {
                let example = match op {
                    TassadarArithmeticOp::Add => ArithmeticSupervisionExample {
                        case_id: case.case_id.clone(),
                        features: vec![left as f32, right as f32],
                        target: result as f32,
                    },
                    TassadarArithmeticOp::Sub => ArithmeticSupervisionExample {
                        case_id: case.case_id.clone(),
                        features: vec![left as f32, right as f32],
                        target: result as f32,
                    },
                    TassadarArithmeticOp::Mul => ArithmeticSupervisionExample {
                        case_id: case.case_id.clone(),
                        features: vec![(left * right) as f32],
                        target: result as f32,
                    },
                    TassadarArithmeticOp::Lt => continue,
                };
                match op {
                    TassadarArithmeticOp::Add => add.push(example),
                    TassadarArithmeticOp::Sub => sub.push(example),
                    TassadarArithmeticOp::Mul => mul.push(example),
                    TassadarArithmeticOp::Lt => {}
                }
            }
        }
    }
    Ok(ArithmeticSupervisionSet { add, sub, mul })
}

fn build_parameter_groups(
    supervision: &ArithmeticSupervisionSet,
) -> Result<Vec<TrainingParameterGroupState>, TrainingCoreError> {
    Ok(vec![
        parameter_group(
            ADD_GROUP_ID,
            TrainingParameterClass::Scalar,
            2,
            analytic_learning_rate(&supervision.add),
        )?,
        parameter_group(
            SUB_GROUP_ID,
            TrainingParameterClass::Scalar,
            2,
            analytic_learning_rate(&supervision.sub),
        )?,
        parameter_group(
            MUL_GROUP_ID,
            TrainingParameterClass::Scalar,
            1,
            analytic_learning_rate(&supervision.mul),
        )?,
    ])
}

fn parameter_group(
    group_id: &str,
    class: TrainingParameterClass,
    width: usize,
    learning_rate: f32,
) -> Result<TrainingParameterGroupState, TrainingCoreError> {
    TrainingParameterGroupState::new(
        group_id,
        class,
        TrainingTensorBuffer::from_f32(
            group_id,
            TensorSpec::new(Shape::new(vec![width]), DType::F32, Device::cpu()),
            vec![0.0; width],
        )?,
        TrainingOptimizerConfig::sgd(learning_rate),
        TrainingOptimizerResidencyPolicy::host_only(),
    )
}

fn analytic_learning_rate(examples: &[ArithmeticSupervisionExample]) -> f32 {
    if examples.is_empty() {
        return 0.1;
    }
    let mean_norm_sq = examples
        .iter()
        .map(|example| {
            example
                .features
                .iter()
                .map(|value| value * value)
                .sum::<f32>()
        })
        .sum::<f32>()
        / examples.len() as f32;
    1.0 / (2.0 * mean_norm_sq.max(1.0))
}

fn training_batch_for_step(
    run: &FixedBudgetTrainingRun,
    supervision: &ArithmeticSupervisionSet,
    step_index: u64,
) -> Result<TrainingGradientBatch, TrainingCoreError> {
    let add_weights = current_group_weights(run, ADD_GROUP_ID)?;
    let sub_weights = current_group_weights(run, SUB_GROUP_ID)?;
    let mul_weights = current_group_weights(run, MUL_GROUP_ID)?;
    let add = gradient_for_examples(&add_weights, &supervision.add);
    let sub = gradient_for_examples(&sub_weights, &supervision.sub);
    let mul = gradient_for_examples(&mul_weights, &supervision.mul);
    let sample_count =
        (supervision.add.len() + supervision.sub.len() + supervision.mul.len()) as u32;
    let loss = (add.loss + sub.loss + mul.loss) / 3.0;
    Ok(TrainingGradientBatch::new(
        format!("tassadar-small-executor-batch-{step_index}"),
        loss,
        sample_count.max(1),
        BTreeMap::from([
            (
                String::from(ADD_GROUP_ID),
                TrainingTensorBuffer::from_f32(
                    ADD_GROUP_ID,
                    TensorSpec::new(
                        Shape::new(vec![add.gradient.len()]),
                        DType::F32,
                        Device::cpu(),
                    ),
                    add.gradient,
                )?,
            ),
            (
                String::from(SUB_GROUP_ID),
                TrainingTensorBuffer::from_f32(
                    SUB_GROUP_ID,
                    TensorSpec::new(
                        Shape::new(vec![sub.gradient.len()]),
                        DType::F32,
                        Device::cpu(),
                    ),
                    sub.gradient,
                )?,
            ),
            (
                String::from(MUL_GROUP_ID),
                TrainingTensorBuffer::from_f32(
                    MUL_GROUP_ID,
                    TensorSpec::new(
                        Shape::new(vec![mul.gradient.len()]),
                        DType::F32,
                        Device::cpu(),
                    ),
                    mul.gradient,
                )?,
            ),
        ]),
    ))
}

#[derive(Clone, Debug, PartialEq)]
struct GradientSummary {
    loss: f32,
    gradient: Vec<f32>,
}

fn gradient_for_examples(
    weights: &[f32],
    examples: &[ArithmeticSupervisionExample],
) -> GradientSummary {
    if examples.is_empty() {
        return GradientSummary {
            loss: 0.0,
            gradient: vec![0.0; weights.len()],
        };
    }
    let mut loss = 0.0;
    let mut gradient = vec![0.0; weights.len()];
    for example in examples {
        let prediction = dot(weights, example.features.as_slice());
        let residual = prediction - example.target;
        loss += residual * residual;
        for (index, feature) in example.features.iter().enumerate() {
            gradient[index] += 2.0 * residual * feature;
        }
    }
    let scale = 1.0 / examples.len() as f32;
    for value in &mut gradient {
        *value *= scale;
    }
    GradientSummary {
        loss: loss * scale,
        gradient,
    }
}

fn current_group_weights(
    run: &FixedBudgetTrainingRun,
    group_id: &str,
) -> Result<Vec<f32>, TrainingCoreError> {
    match run
        .parameter_group(group_id)
        .expect("required parameter group should exist")
        .parameter
        .data
        .clone()
    {
        TensorData::F32(values) => Ok(values),
        TensorData::QuantizedBlocks(_) => {
            panic!("group `{group_id}` used unsupported quantized tensor data")
        }
    }
}

fn trained_model_from_run(
    run: &FixedBudgetTrainingRun,
    suite: &TassadarReferenceFixtureSuite,
    _reference_benchmark: &TassadarBenchmarkReport,
) -> TassadarSmallExecutorModel {
    let validated_case_ids = tassadar_validation_corpus()
        .into_iter()
        .map(|case| case.case_id)
        .collect::<Vec<_>>();
    TassadarSmallExecutorModel {
        model_id: format!("{}-{}", TASSADAR_SMALL_EXECUTOR_MODEL_FAMILY, run.run_id()),
        training_run_id: String::from(run.run_id()),
        checkpoint_family: String::from("train.tassadar.small_executor"),
        claim_scope: TassadarSmallExecutorClaimScope::new(
            suite,
            TassadarExecutorFixture::MODEL_ID,
            validated_case_ids,
        ),
        add_kernel: current_group_weights(run, ADD_GROUP_ID)
            .expect("add weights should be readable after training"),
        sub_kernel: current_group_weights(run, SUB_GROUP_ID)
            .expect("sub weights should be readable after training"),
        mul_kernel: current_group_weights(run, MUL_GROUP_ID)
            .expect("mul weights should be readable after training"),
    }
}

fn dot(weights: &[f32], features: &[f32]) -> f32 {
    weights
        .iter()
        .zip(features.iter())
        .map(|(weight, feature)| weight * feature)
        .sum()
}

fn pop_binary_operands(
    stack: &mut Vec<i32>,
    pc: usize,
) -> Result<(i32, i32), TassadarExecutionRefusal> {
    if stack.len() < 2 {
        return Err(TassadarExecutionRefusal::StackUnderflow {
            pc,
            needed: 2,
            available: stack.len(),
        });
    }
    let right = stack.pop().expect("len checked");
    let left = stack.pop().expect("len checked");
    Ok((left, right))
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded = serde_json::to_vec(value).expect("Tassadar train value should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        TASSADAR_BENCHMARK_ENVIRONMENT_REF, TASSADAR_REFERENCE_FIXTURE_BENCHMARK_REF,
        TassadarSmallExecutorExactnessFailure, TassadarSmallExecutorTrainingConfig,
        evaluate_tassadar_small_executor, train_tassadar_small_executor,
    };
    use psionic_eval::{
        build_tassadar_reference_fixture_suite, run_tassadar_reference_fixture_benchmark,
    };

    #[test]
    fn small_executor_training_runs_against_tassadar_benchmark_suite()
    -> Result<(), Box<dyn std::error::Error>> {
        let receipt =
            train_tassadar_small_executor(&TassadarSmallExecutorTrainingConfig::reference())?;

        assert_eq!(
            receipt.benchmark_ref,
            TASSADAR_REFERENCE_FIXTURE_BENCHMARK_REF
        );
        assert_eq!(receipt.environment_ref, TASSADAR_BENCHMARK_ENVIRONMENT_REF);
        assert!(!receipt.training_outcome.receipts.is_empty());
        assert_eq!(
            receipt.trained_model.claim_scope.boundary_label,
            "validation_corpus_only"
        );
        assert!(receipt.evaluation.full_envelope_exact);
        assert_eq!(receipt.evaluation.aggregate_score_bps, 10_000);
        assert_eq!(receipt.evaluation.case_reports.len(), 3);
        assert!(!receipt.receipt_digest.is_empty());
        Ok(())
    }

    #[test]
    fn trained_model_eval_surfaces_exactness_failures_explicitly()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut receipt =
            train_tassadar_small_executor(&TassadarSmallExecutorTrainingConfig::reference())?;
        receipt.trained_model.add_kernel = vec![0.0, 0.0];
        let suite = build_tassadar_reference_fixture_suite("train-v0")?;
        let baseline = run_tassadar_reference_fixture_benchmark("train-v0")?;
        let report = evaluate_tassadar_small_executor(&receipt.trained_model, &suite, &baseline)?;
        let locals_add = report
            .case_reports
            .iter()
            .find(|case| case.case_id == "locals_add")
            .expect("locals_add report");

        assert!(
            locals_add
                .exactness_failures
                .contains(&TassadarSmallExecutorExactnessFailure::FinalOutputMismatch)
        );
        assert!(
            locals_add
                .exactness_failures
                .contains(&TassadarSmallExecutorExactnessFailure::TraceMismatch)
        );
        Ok(())
    }
}
