use std::collections::BTreeMap;

use psionic_ir::{
    TassadarSymbolicBinaryOp, TassadarSymbolicExpr, TassadarSymbolicLoweringOpcode,
    TassadarSymbolicOperand, TassadarSymbolicProgram, TassadarSymbolicProgramError,
};
use psionic_runtime::{
    TassadarCompilerToolchainIdentity, TassadarCpuReferenceRunner, TassadarExecutionRefusal,
    TassadarInstruction, TassadarOpcode, TassadarProgram, TassadarProgramArtifact,
    TassadarProgramArtifactError, TassadarProgramSourceIdentity, TassadarProgramSourceKind,
    tassadar_trace_abi_for_profile_id, tassadar_wasm_profile_for_id,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_SYMBOLIC_PROGRAM_ARTIFACT_BUNDLE_SCHEMA_VERSION: u16 = 1;
const TASSADAR_SYMBOLIC_PROGRAM_ARTIFACT_COMPILER_FAMILY: &str = "tassadar_symbolic_lowering";
const TASSADAR_SYMBOLIC_PROGRAM_ARTIFACT_COMPILER_VERSION: &str = "v1";
const TASSADAR_SYMBOLIC_PROGRAM_ARTIFACT_CLAIM_BOUNDARY: &str =
    "bounded symbolic lowering compiles straight-line symbolic programs into runnable Tassadar program artifacts and explicit execution manifests for one declared input assignment set at a time; this stays research-only compiled bounded exactness and does not imply arbitrary Wasm, loops, subroutines, or learned generalization";

/// One lowered runtime program instantiated from the bounded symbolic IR.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLoweredSymbolicProgram {
    /// Stable symbolic source-program identifier.
    pub symbolic_program_id: String,
    /// Stable digest of the symbolic program.
    pub symbolic_program_digest: String,
    /// Stable digest of the concrete input assignments used for lowering.
    pub input_assignment_digest: String,
    /// Lowered runtime-visible Tassadar program.
    pub validated_program: TassadarProgram,
}

impl TassadarLoweredSymbolicProgram {
    /// Returns a stable digest over the lowered symbolic/runtime pair.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_digest(b"tassadar_lowered_symbolic_program|", self)
    }
}

/// Lowering failure for the bounded symbolic IR.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TassadarSymbolicLoweringError {
    /// One symbolic parse/validation/evaluation check failed.
    #[error(transparent)]
    Symbolic(#[from] TassadarSymbolicProgramError),
    /// The symbolic program targeted an unknown runtime profile.
    #[error("unsupported symbolic runtime profile `{profile_id}`")]
    UnsupportedProfile {
        /// Unsupported profile identifier.
        profile_id: String,
    },
    /// The symbolic program required one opcode the selected profile does not support.
    #[error("symbolic lowering opcode `{opcode}` is unsupported by runtime profile `{profile_id}`")]
    UnsupportedLoweringOpcode {
        /// Unsupported opcode mnemonic.
        opcode: String,
        /// Runtime profile identifier.
        profile_id: String,
    },
    /// The symbolic program lowered into too many runtime locals for the selected profile.
    #[error(
        "symbolic lowering requires {required} locals, but runtime profile `{profile_id}` supports at most {max_supported}"
    )]
    TooManyLoweredLocals {
        /// Required lowered local count.
        required: usize,
        /// Runtime profile local limit.
        max_supported: usize,
        /// Runtime profile identifier.
        profile_id: String,
    },
    /// The lowered runtime program failed validation or execution on the CPU reference lane.
    #[error(transparent)]
    Execution(#[from] TassadarExecutionRefusal),
}

/// One digest-bound expected execution manifest paired with a symbolic artifact
/// bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSymbolicProgramExecutionManifest {
    /// Stable symbolic program identifier.
    pub symbolic_program_id: String,
    /// Stable input-assignment digest for the manifest.
    pub input_assignment_digest: String,
    /// Final outputs expected from the lowered runtime artifact.
    pub expected_outputs: Vec<i32>,
    /// Final memory image expected from the lowered runtime artifact.
    pub expected_final_memory: Vec<i32>,
    /// Stable digest over the expected execution fields.
    pub execution_digest: String,
}

impl TassadarSymbolicProgramExecutionManifest {
    fn new(
        symbolic_program_id: impl Into<String>,
        input_assignment_digest: impl Into<String>,
        expected_outputs: Vec<i32>,
        expected_final_memory: Vec<i32>,
    ) -> Self {
        let mut manifest = Self {
            symbolic_program_id: symbolic_program_id.into(),
            input_assignment_digest: input_assignment_digest.into(),
            expected_outputs,
            expected_final_memory,
            execution_digest: String::new(),
        };
        manifest.execution_digest =
            stable_digest(b"tassadar_symbolic_program_execution_manifest|", &manifest);
        manifest
    }
}

/// First public artifact format produced by the bounded symbolic compiler lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSymbolicProgramArtifactBundle {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable bundle identifier.
    pub bundle_id: String,
    /// Coarse claim class for the artifact bundle.
    pub claim_class: String,
    /// Plain-language boundary for the bundle.
    pub claim_boundary: String,
    /// Stable symbolic program identifier.
    pub symbolic_program_id: String,
    /// Stable symbolic program digest.
    pub symbolic_program_digest: String,
    /// Stable symbolic input-assignment digest.
    pub input_assignment_digest: String,
    /// Ordered lowering opcode requirements.
    pub required_lowering_opcodes: Vec<TassadarSymbolicLoweringOpcode>,
    /// Source identity bound into the produced runtime artifact.
    pub source_identity: TassadarProgramSourceIdentity,
    /// Toolchain identity bound into the produced runtime artifact.
    pub toolchain_identity: TassadarCompilerToolchainIdentity,
    /// Runnable runtime-facing program artifact.
    pub program_artifact: TassadarProgramArtifact,
    /// Digest-bound expected execution manifest for the lowered artifact.
    pub execution_manifest: TassadarSymbolicProgramExecutionManifest,
    /// Stable digest over the bundle.
    pub bundle_digest: String,
}

impl TassadarSymbolicProgramArtifactBundle {
    fn new(
        bundle_id: impl Into<String>,
        claim_class: impl Into<String>,
        symbolic_program_id: impl Into<String>,
        symbolic_program_digest: impl Into<String>,
        input_assignment_digest: impl Into<String>,
        required_lowering_opcodes: Vec<TassadarSymbolicLoweringOpcode>,
        source_identity: TassadarProgramSourceIdentity,
        toolchain_identity: TassadarCompilerToolchainIdentity,
        program_artifact: TassadarProgramArtifact,
        execution_manifest: TassadarSymbolicProgramExecutionManifest,
    ) -> Self {
        let mut bundle = Self {
            schema_version: TASSADAR_SYMBOLIC_PROGRAM_ARTIFACT_BUNDLE_SCHEMA_VERSION,
            bundle_id: bundle_id.into(),
            claim_class: claim_class.into(),
            claim_boundary: String::from(TASSADAR_SYMBOLIC_PROGRAM_ARTIFACT_CLAIM_BOUNDARY),
            symbolic_program_id: symbolic_program_id.into(),
            symbolic_program_digest: symbolic_program_digest.into(),
            input_assignment_digest: input_assignment_digest.into(),
            required_lowering_opcodes,
            source_identity,
            toolchain_identity,
            program_artifact,
            execution_manifest,
            bundle_digest: String::new(),
        };
        bundle.bundle_digest =
            stable_digest(b"tassadar_symbolic_program_artifact_bundle|", &bundle);
        bundle
    }
}

/// Artifact-bundle assembly failure for the bounded symbolic compiler lane.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TassadarSymbolicArtifactBundleError {
    /// One symbolic parse, evaluation, or lowering step failed.
    #[error(transparent)]
    Symbolic(#[from] TassadarSymbolicProgramError),
    /// Lowering into the runtime artifact surface failed.
    #[error(transparent)]
    Lowering(#[from] TassadarSymbolicLoweringError),
    /// No trace ABI is currently published for the targeted runtime profile.
    #[error("no trace ABI is published for symbolic runtime profile `{profile_id}`")]
    UnsupportedTraceAbi {
        /// Unsupported runtime profile identifier.
        profile_id: String,
    },
    /// Final program-artifact assembly failed validation.
    #[error(transparent)]
    ProgramArtifact(#[from] TassadarProgramArtifactError),
}

/// Lowers one bounded symbolic program into a concrete runtime program over the
/// supplied input assignments.
pub fn lower_tassadar_symbolic_program(
    program: &TassadarSymbolicProgram,
    input_assignments: &BTreeMap<String, i32>,
) -> Result<TassadarLoweredSymbolicProgram, TassadarSymbolicLoweringError> {
    program.validate()?;
    let Some(profile) = tassadar_wasm_profile_for_id(program.profile_id.as_str()) else {
        return Err(TassadarSymbolicLoweringError::UnsupportedProfile {
            profile_id: program.profile_id.clone(),
        });
    };

    for opcode in program.required_lowering_opcodes() {
        let runtime_opcode = runtime_opcode(opcode);
        if !profile.supports(runtime_opcode) {
            return Err(TassadarSymbolicLoweringError::UnsupportedLoweringOpcode {
                opcode: runtime_opcode.mnemonic().to_string(),
                profile_id: profile.profile_id.clone(),
            });
        }
    }

    let required_locals = program.required_lowered_locals();
    if required_locals > profile.max_locals {
        return Err(TassadarSymbolicLoweringError::TooManyLoweredLocals {
            required: required_locals,
            max_supported: profile.max_locals,
            profile_id: profile.profile_id.clone(),
        });
    }

    let local_map = symbolic_local_map(program);
    let initial_memory = program.initial_memory_image(input_assignments)?;
    let mut instructions = Vec::new();

    for statement in &program.statements {
        match statement {
            psionic_ir::TassadarSymbolicStatement::Let { name, expr } => {
                lower_expr(program, expr, &local_map, &mut instructions)?;
                instructions.push(TassadarInstruction::LocalSet {
                    local: *local_map
                        .get(name.as_str())
                        .expect("validated symbolic binding should have one lowered local"),
                });
            }
            psionic_ir::TassadarSymbolicStatement::Store { slot, value } => {
                lower_operand(program, value, &local_map, &mut instructions)?;
                instructions.push(TassadarInstruction::I32Store { slot: *slot });
            }
            psionic_ir::TassadarSymbolicStatement::Output { value } => {
                lower_operand(program, value, &local_map, &mut instructions)?;
                instructions.push(TassadarInstruction::Output);
            }
        }
    }
    instructions.push(TassadarInstruction::Return);

    let input_assignment_digest =
        stable_digest(b"tassadar_symbolic_input_assignments|", input_assignments);
    let lowered_program_id = format!(
        "{}.lowered.{}",
        program.program_id,
        &input_assignment_digest[..12]
    );
    let validated_program = TassadarProgram::new(
        lowered_program_id,
        &profile,
        required_locals,
        program.memory_slots,
        instructions,
    )
    .with_initial_memory(initial_memory);

    let runner = TassadarCpuReferenceRunner::for_program(&validated_program)?;
    let _execution = runner.execute(&validated_program)?;

    Ok(TassadarLoweredSymbolicProgram {
        symbolic_program_id: program.program_id.clone(),
        symbolic_program_digest: program.stable_digest(),
        input_assignment_digest,
        validated_program,
    })
}

/// Lowers one bounded symbolic program into a digest-bound runtime artifact plus
/// explicit execution manifest for the supplied input assignments.
pub fn compile_tassadar_symbolic_program_to_artifact_bundle(
    program: &TassadarSymbolicProgram,
    input_assignments: &BTreeMap<String, i32>,
) -> Result<TassadarSymbolicProgramArtifactBundle, TassadarSymbolicArtifactBundleError> {
    let lowered = lower_tassadar_symbolic_program(program, input_assignments)?;
    let profile = tassadar_wasm_profile_for_id(program.profile_id.as_str()).ok_or_else(|| {
        TassadarSymbolicLoweringError::UnsupportedProfile {
            profile_id: program.profile_id.clone(),
        }
    })?;
    let trace_abi = tassadar_trace_abi_for_profile_id(program.profile_id.as_str()).ok_or_else(
        || TassadarSymbolicArtifactBundleError::UnsupportedTraceAbi {
            profile_id: program.profile_id.clone(),
        },
    )?;
    let required_lowering_opcodes = program.required_lowering_opcodes();
    let source_identity = TassadarProgramSourceIdentity::new(
        TassadarProgramSourceKind::SymbolicProgram,
        program.program_id.clone(),
        program.stable_digest(),
    );
    let toolchain_identity = symbolic_toolchain_identity(
        program.profile_id.as_str(),
        required_lowering_opcodes.as_slice(),
    );
    let artifact_id = format!(
        "{}.symbolic_artifact.{}.v1",
        program.program_id,
        &lowered.input_assignment_digest[..12]
    );
    let program_artifact = TassadarProgramArtifact::new(
        artifact_id,
        source_identity.clone(),
        toolchain_identity.clone(),
        &profile,
        &trace_abi,
        lowered.validated_program,
    )?;
    let execution = program.evaluate(input_assignments)?;
    let execution_manifest = TassadarSymbolicProgramExecutionManifest::new(
        program.program_id.clone(),
        lowered.input_assignment_digest.clone(),
        execution.outputs,
        execution.final_memory,
    );
    let bundle_id = format!(
        "{}.symbolic_bundle.{}.v1",
        program.program_id,
        &lowered.input_assignment_digest[..12]
    );
    Ok(TassadarSymbolicProgramArtifactBundle::new(
        bundle_id,
        psionic_ir::TASSADAR_SYMBOLIC_PROGRAM_CLAIM_CLASS,
        program.program_id.clone(),
        program.stable_digest(),
        lowered.input_assignment_digest,
        required_lowering_opcodes,
        source_identity,
        toolchain_identity,
        program_artifact,
        execution_manifest,
    ))
}

fn symbolic_toolchain_identity(
    profile_id: &str,
    required_lowering_opcodes: &[TassadarSymbolicLoweringOpcode],
) -> TassadarCompilerToolchainIdentity {
    let mut pipeline_features = vec![
        String::from("bounded_symbolic_ir"),
        String::from("compiled_bounded_exactness"),
        String::from("research_only"),
        String::from("runnable_runtime_artifact"),
        String::from("trace_generator_ready"),
    ];
    for opcode in required_lowering_opcodes {
        pipeline_features.push(format!("opcode:{}", opcode.mnemonic()));
    }
    TassadarCompilerToolchainIdentity::new(
        TASSADAR_SYMBOLIC_PROGRAM_ARTIFACT_COMPILER_FAMILY,
        TASSADAR_SYMBOLIC_PROGRAM_ARTIFACT_COMPILER_VERSION,
        profile_id,
    )
    .with_pipeline_features(pipeline_features)
}

fn symbolic_local_map(program: &TassadarSymbolicProgram) -> BTreeMap<&str, u8> {
    let mut local_map = BTreeMap::new();
    let mut next_local = 0_u8;
    for statement in &program.statements {
        if let psionic_ir::TassadarSymbolicStatement::Let { name, .. } = statement {
            local_map.insert(name.as_str(), next_local);
            next_local = next_local.saturating_add(1);
        }
    }
    local_map
}

fn lower_expr(
    program: &TassadarSymbolicProgram,
    expr: &TassadarSymbolicExpr,
    local_map: &BTreeMap<&str, u8>,
    instructions: &mut Vec<TassadarInstruction>,
) -> Result<(), TassadarSymbolicLoweringError> {
    match expr {
        TassadarSymbolicExpr::Operand { operand } => {
            lower_operand(program, operand, local_map, instructions)?;
        }
        TassadarSymbolicExpr::Binary { op, left, right } => {
            lower_operand(program, left, local_map, instructions)?;
            lower_operand(program, right, local_map, instructions)?;
            instructions.push(match op {
                TassadarSymbolicBinaryOp::Add => TassadarInstruction::I32Add,
                TassadarSymbolicBinaryOp::Sub => TassadarInstruction::I32Sub,
                TassadarSymbolicBinaryOp::Mul => TassadarInstruction::I32Mul,
                TassadarSymbolicBinaryOp::Lt => TassadarInstruction::I32Lt,
            });
        }
    }
    Ok(())
}

fn lower_operand(
    program: &TassadarSymbolicProgram,
    operand: &TassadarSymbolicOperand,
    local_map: &BTreeMap<&str, u8>,
    instructions: &mut Vec<TassadarInstruction>,
) -> Result<(), TassadarSymbolicLoweringError> {
    match operand {
        TassadarSymbolicOperand::Name { name } => {
            if let Some(local) = local_map.get(name.as_str()) {
                instructions.push(TassadarInstruction::LocalGet { local: *local });
            } else if let Some(slot) = program.input_slot(name.as_str()) {
                instructions.push(TassadarInstruction::I32Load { slot });
            } else {
                return Err(
                    TassadarSymbolicProgramError::UnknownValue { name: name.clone() }.into(),
                );
            }
        }
        TassadarSymbolicOperand::Const { value } => {
            instructions.push(TassadarInstruction::I32Const { value: *value });
        }
        TassadarSymbolicOperand::MemorySlot { slot } => {
            instructions.push(TassadarInstruction::I32Load { slot: *slot });
        }
    }
    Ok(())
}

fn runtime_opcode(opcode: TassadarSymbolicLoweringOpcode) -> TassadarOpcode {
    match opcode {
        TassadarSymbolicLoweringOpcode::I32Const => TassadarOpcode::I32Const,
        TassadarSymbolicLoweringOpcode::I32Load => TassadarOpcode::I32Load,
        TassadarSymbolicLoweringOpcode::I32Store => TassadarOpcode::I32Store,
        TassadarSymbolicLoweringOpcode::LocalGet => TassadarOpcode::LocalGet,
        TassadarSymbolicLoweringOpcode::LocalSet => TassadarOpcode::LocalSet,
        TassadarSymbolicLoweringOpcode::I32Add => TassadarOpcode::I32Add,
        TassadarSymbolicLoweringOpcode::I32Sub => TassadarOpcode::I32Sub,
        TassadarSymbolicLoweringOpcode::I32Mul => TassadarOpcode::I32Mul,
        TassadarSymbolicLoweringOpcode::I32Lt => TassadarOpcode::I32Lt,
        TassadarSymbolicLoweringOpcode::Output => TassadarOpcode::Output,
        TassadarSymbolicLoweringOpcode::Return => TassadarOpcode::Return,
    }
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use psionic_ir::tassadar_symbolic_program_examples;
    use psionic_runtime::run_tassadar_exact_parity;

    use super::{
        TassadarSymbolicArtifactBundleError, TassadarSymbolicLoweringError,
        compile_tassadar_symbolic_program_to_artifact_bundle, lower_tassadar_symbolic_program,
    };

    #[test]
    fn symbolic_programs_lower_and_match_cpu_reference_truth() {
        for example in tassadar_symbolic_program_examples() {
            let lowered =
                lower_tassadar_symbolic_program(&example.program, &example.input_assignments)
                    .expect("example should lower");
            let parity = run_tassadar_exact_parity(&lowered.validated_program)
                .expect("lowered example should execute");

            parity.require_exact().expect("fixture parity should hold");
            assert_eq!(parity.reference.outputs, example.expected_outputs);
            assert_eq!(parity.reference.final_memory, example.expected_final_memory);
        }
    }

    #[test]
    fn symbolic_lowering_refuses_profiles_that_do_not_support_required_opcodes() {
        let mut example = tassadar_symbolic_program_examples()
            .into_iter()
            .find(|example| example.case_id == "parity_two_bits")
            .expect("parity example");
        example.program.profile_id = String::from("tassadar.wasm.core_i32.v1");

        let error = lower_tassadar_symbolic_program(&example.program, &example.input_assignments)
            .expect_err("lt should stay explicit as unsupported on core_i32_v1");

        assert_eq!(
            error,
            TassadarSymbolicLoweringError::UnsupportedLoweringOpcode {
                opcode: String::from("i32.lt"),
                profile_id: String::from("tassadar.wasm.core_i32.v1"),
            }
        );
    }

    #[test]
    fn symbolic_lowering_requires_declared_input_assignments() {
        let example = tassadar_symbolic_program_examples()
            .into_iter()
            .next()
            .expect("addition example");
        let error = lower_tassadar_symbolic_program(&example.program, &BTreeMap::new())
            .expect_err("missing input assignments should stay explicit");

        assert_eq!(
            error,
            TassadarSymbolicLoweringError::Symbolic(
                psionic_ir::TassadarSymbolicProgramError::MissingInputAssignment {
                    input: String::from("lhs"),
                }
            )
        );
    }

    #[test]
    fn symbolic_program_artifact_bundles_cover_required_bounded_families() {
        let examples = tassadar_symbolic_program_examples();
        let case_ids = examples
            .iter()
            .map(|example| example.case_id.as_str())
            .collect::<Vec<_>>();
        assert!(case_ids.contains(&"addition_pair"));
        assert!(case_ids.contains(&"parity_two_bits"));
        assert!(case_ids.contains(&"finite_state_counter"));
        assert!(case_ids.contains(&"stack_machine_add_step"));

        for example in examples {
            let bundle = compile_tassadar_symbolic_program_to_artifact_bundle(
                &example.program,
                &example.input_assignments,
            )
            .expect("example should compile into an artifact bundle");
            bundle
                .program_artifact
                .validate_internal_consistency()
                .expect("artifact bundle should stay digest-bound");
            assert_eq!(bundle.claim_class, "compiled_bounded_exactness");
            assert_eq!(
                bundle.program_artifact.source_identity.source_kind,
                psionic_runtime::TassadarProgramSourceKind::SymbolicProgram
            );
            assert_eq!(
                bundle.execution_manifest.expected_outputs,
                example.expected_outputs
            );
            assert_eq!(
                bundle.execution_manifest.expected_final_memory,
                example.expected_final_memory
            );
        }
    }

    #[test]
    fn symbolic_artifact_bundle_refuses_profiles_without_a_trace_abi() {
        let mut example = tassadar_symbolic_program_examples()
            .into_iter()
            .find(|example| example.case_id == "addition_pair")
            .expect("addition example");
        example.program.profile_id = String::from("tassadar.wasm.unknown_profile.v9");

        let error = compile_tassadar_symbolic_program_to_artifact_bundle(
            &example.program,
            &example.input_assignments,
        )
        .expect_err("unknown profile should stay explicit");

        assert_eq!(
            error,
            TassadarSymbolicArtifactBundleError::Lowering(
                TassadarSymbolicLoweringError::UnsupportedProfile {
                    profile_id: String::from("tassadar.wasm.unknown_profile.v9"),
                }
            )
        );
    }
}
