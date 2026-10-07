use psionic_runtime::{
    TassadarModuleExecutionError, TassadarModuleExecutionProgram, TassadarModuleTraceReplayError,
    TassadarModuleTraceV1Artifact, build_tassadar_module_trace_lineage_compatibility_receipt,
    build_tassadar_module_trace_v2_artifact, execute_tassadar_module_execution_program,
    replay_tassadar_module_trace_v2_artifact, summarize_tassadar_module_trace_footprint,
    tassadar_seeded_module_call_indirect_program,
    tassadar_seeded_module_deterministic_import_program,
    tassadar_seeded_module_global_state_program,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const TASSADAR_MODULE_TRACE_ABI_V2_TRAINING_SUITE_SCHEMA_VERSION: u16 = 1;

/// Public training-suite family for the module-trace ABI v2 lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarModuleTraceAbiV2TrainingCaseFamily {
    /// Mutable-global exactness with delta publication.
    GlobalStateParity,
    /// Frame-aware indirect-call replay.
    CallIndirectDispatch,
    /// Deterministic host-import replay.
    DeterministicImportStub,
}

/// One training-facing supervised case for the module-trace ABI v2 lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleTraceAbiV2TrainingCase {
    /// Stable case identifier.
    pub case_id: String,
    /// Case family represented by the case.
    pub family: TassadarModuleTraceAbiV2TrainingCaseFamily,
    /// Stable program identifier.
    pub program_id: String,
    /// Stable execution digest shared across v1 and v2.
    pub execution_digest: String,
    /// Legacy v1 artifact digest.
    pub trace_v1_artifact_digest: String,
    /// V2 artifact digest.
    pub trace_v2_artifact_digest: String,
    /// Shared lineage compatibility digest.
    pub lineage_compatibility_digest: String,
    /// Serialized byte count for the v1 step stream.
    pub v1_step_stream_bytes: u64,
    /// Serialized byte count for the v2 step stream.
    pub v2_step_stream_bytes: u64,
    /// Savings in basis points against v1.
    pub step_stream_reduction_bps: u32,
    /// Whether v2 replay rebuilt the exact legacy execution.
    pub replay_exact: bool,
}

/// Public training-facing suite for the module-trace ABI v2 lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleTraceAbiV2TrainingSuite {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable suite identifier.
    pub suite_id: String,
    /// Explicit claim class for the suite.
    pub claim_class: String,
    /// Ordered supervised cases.
    pub cases: Vec<TassadarModuleTraceAbiV2TrainingCase>,
    /// Stable digest over the suite.
    pub suite_digest: String,
}

impl TassadarModuleTraceAbiV2TrainingSuite {
    fn new(cases: Vec<TassadarModuleTraceAbiV2TrainingCase>) -> Self {
        let mut suite = Self {
            schema_version: TASSADAR_MODULE_TRACE_ABI_V2_TRAINING_SUITE_SCHEMA_VERSION,
            suite_id: String::from("tassadar.module_trace_abi_v2.training_suite.v1"),
            claim_class: String::from("execution_truth_learned_substrate"),
            cases,
            suite_digest: String::new(),
        };
        suite.suite_digest = stable_digest(
            b"psionic_tassadar_module_trace_abi_v2_training_suite|",
            &suite,
        );
        suite
    }
}

/// Suite build failures for the module-trace ABI v2 lane.
#[derive(Debug, Error)]
pub enum TassadarModuleTraceAbiV2TrainingSuiteError {
    /// Runtime execution failed for one seeded case.
    #[error(transparent)]
    Runtime(#[from] TassadarModuleExecutionError),
    /// Step-stream footprint serialization failed.
    #[error(transparent)]
    Serialization(#[from] serde_json::Error),
    /// V2 replay drifted from the original execution.
    #[error(transparent)]
    Replay(#[from] TassadarModuleTraceReplayError),
}

/// Builds the canonical training-facing suite for the module-trace ABI v2 lane.
pub fn build_tassadar_module_trace_abi_v2_training_suite()
-> Result<TassadarModuleTraceAbiV2TrainingSuite, TassadarModuleTraceAbiV2TrainingSuiteError> {
    Ok(TassadarModuleTraceAbiV2TrainingSuite::new(vec![
        build_case(
            "global_state_parity",
            TassadarModuleTraceAbiV2TrainingCaseFamily::GlobalStateParity,
            tassadar_seeded_module_global_state_program(),
        )?,
        build_case(
            "call_indirect_dispatch",
            TassadarModuleTraceAbiV2TrainingCaseFamily::CallIndirectDispatch,
            tassadar_seeded_module_call_indirect_program(),
        )?,
        build_case(
            "deterministic_import_stub",
            TassadarModuleTraceAbiV2TrainingCaseFamily::DeterministicImportStub,
            tassadar_seeded_module_deterministic_import_program(),
        )?,
    ]))
}

fn build_case(
    case_id: &str,
    family: TassadarModuleTraceAbiV2TrainingCaseFamily,
    program: TassadarModuleExecutionProgram,
) -> Result<TassadarModuleTraceAbiV2TrainingCase, TassadarModuleTraceAbiV2TrainingSuiteError> {
    let execution = execute_tassadar_module_execution_program(&program)?;
    let v1 = TassadarModuleTraceV1Artifact::from_execution(
        format!("tassadar://module_trace_v1/training/{case_id}"),
        &execution,
    );
    let v2 = build_tassadar_module_trace_v2_artifact(
        format!("tassadar://module_trace_v2/training/{case_id}"),
        &program,
        &execution,
    )?;
    let replayed = replay_tassadar_module_trace_v2_artifact(&program, &v2)?;
    let footprint = summarize_tassadar_module_trace_footprint(&v1, &v2)?;
    let lineage = build_tassadar_module_trace_lineage_compatibility_receipt(&v1, &v2);

    Ok(TassadarModuleTraceAbiV2TrainingCase {
        case_id: String::from(case_id),
        family,
        program_id: program.program_id,
        execution_digest: execution.execution_digest(),
        trace_v1_artifact_digest: v1.artifact_digest,
        trace_v2_artifact_digest: v2.artifact_digest,
        lineage_compatibility_digest: lineage.compatibility_digest,
        v1_step_stream_bytes: footprint.v1_step_stream_bytes,
        v2_step_stream_bytes: footprint.v2_step_stream_bytes,
        step_stream_reduction_bps: footprint.step_stream_reduction_bps,
        replay_exact: replayed == execution,
    })
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        TassadarModuleTraceAbiV2TrainingCaseFamily,
        build_tassadar_module_trace_abi_v2_training_suite,
    };

    #[test]
    fn module_trace_abi_v2_training_suite_is_machine_legible() {
        let suite = build_tassadar_module_trace_abi_v2_training_suite().expect("suite");
        assert_eq!(suite.cases.len(), 3);
        assert!(!suite.suite_digest.is_empty());
        assert!(suite.cases.iter().all(|case| case.replay_exact));
    }

    #[test]
    fn module_trace_abi_v2_training_suite_records_frame_aware_case() {
        let suite = build_tassadar_module_trace_abi_v2_training_suite().expect("suite");
        let case = suite
            .cases
            .iter()
            .find(|case| {
                case.family == TassadarModuleTraceAbiV2TrainingCaseFamily::CallIndirectDispatch
            })
            .expect("call-indirect case");

        assert!(case.step_stream_reduction_bps > 0);
        assert!(case.v2_step_stream_bytes < case.v1_step_stream_bytes);
    }
}
