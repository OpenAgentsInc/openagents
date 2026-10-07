use psionic_runtime::{
    TassadarLinearMemoryExecutionError, TassadarLinearMemoryProgram,
    TassadarLinearMemoryTraceFootprint, TassadarMemoryAbiContract,
    execute_tassadar_linear_memory_program, summarize_tassadar_linear_memory_trace_footprint,
    tassadar_seeded_linear_memory_growth_program, tassadar_seeded_linear_memory_memcpy_program,
    tassadar_seeded_linear_memory_sign_extension_program,
    tassadar_seeded_linear_memory_width_parity_program,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const TASSADAR_MEMORY_ABI_V2_TRAINING_SUITE_SCHEMA_VERSION: u16 = 1;

/// Public training-suite family for one byte-addressed memory ABI v2 case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarMemoryAbiV2TrainingCaseFamily {
    /// Width-parity and truncation behavior.
    LoadStoreWidthParity,
    /// Signed-vs-unsigned narrow loads.
    SignExtension,
    /// `memory.size` and `memory.grow` behavior.
    MemorySizeAndGrow,
    /// Dense byte-copy trace regression coverage.
    MemcpyTraceRegression,
}

/// One training-facing supervised case for the memory ABI v2 lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMemoryAbiV2TrainingCase {
    /// Stable case identifier.
    pub case_id: String,
    /// Family represented by the case.
    pub family: TassadarMemoryAbiV2TrainingCaseFamily,
    /// Stable program identifier.
    pub program_id: String,
    /// Expected final outputs.
    pub expected_outputs: Vec<i32>,
    /// Observed final outputs under the runtime-owned executor.
    pub observed_outputs: Vec<i32>,
    /// Delta-oriented trace footprint for the case.
    pub trace_footprint: TassadarLinearMemoryTraceFootprint,
}

/// Public training-facing suite for the memory ABI v2 lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMemoryAbiV2TrainingSuite {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable suite identifier.
    pub suite_id: String,
    /// Explicit claim class for the suite.
    pub claim_class: String,
    /// Declared runtime-owned memory ABI contract.
    pub memory_abi: TassadarMemoryAbiContract,
    /// Ordered supervised cases.
    pub cases: Vec<TassadarMemoryAbiV2TrainingCase>,
    /// Stable digest over the suite.
    pub suite_digest: String,
}

impl TassadarMemoryAbiV2TrainingSuite {
    fn new(cases: Vec<TassadarMemoryAbiV2TrainingCase>) -> Self {
        let mut suite = Self {
            schema_version: TASSADAR_MEMORY_ABI_V2_TRAINING_SUITE_SCHEMA_VERSION,
            suite_id: String::from("tassadar.memory_abi_v2.training_suite.v1"),
            claim_class: String::from("execution_truth_fast_path_substrate"),
            memory_abi: TassadarMemoryAbiContract::linear_memory_v2(),
            cases,
            suite_digest: String::new(),
        };
        suite.suite_digest =
            stable_digest(b"psionic_tassadar_memory_abi_v2_training_suite|", &suite);
        suite
    }
}

/// Training-suite build failures for the memory ABI v2 lane.
#[derive(Debug, Error)]
pub enum TassadarMemoryAbiV2TrainingSuiteError {
    /// Runtime execution failed for one seeded case.
    #[error(transparent)]
    Runtime(#[from] TassadarLinearMemoryExecutionError),
}

/// Builds the canonical training-facing supervision suite for the memory ABI v2 lane.
pub fn build_tassadar_memory_abi_v2_training_suite()
-> Result<TassadarMemoryAbiV2TrainingSuite, TassadarMemoryAbiV2TrainingSuiteError> {
    let cases = vec![
        build_case(
            "width_parity",
            TassadarMemoryAbiV2TrainingCaseFamily::LoadStoreWidthParity,
            tassadar_seeded_linear_memory_width_parity_program(),
            vec![120, 22_136, 305_419_896, 255],
        )?,
        build_case(
            "sign_extension",
            TassadarMemoryAbiV2TrainingCaseFamily::SignExtension,
            tassadar_seeded_linear_memory_sign_extension_program(),
            vec![-128, 128, -128, 65_408],
        )?,
        build_case(
            "memory_size_and_grow",
            TassadarMemoryAbiV2TrainingCaseFamily::MemorySizeAndGrow,
            tassadar_seeded_linear_memory_growth_program(),
            vec![1, 1, 2, -1, 2],
        )?,
        build_case(
            "memcpy_trace_regression",
            TassadarMemoryAbiV2TrainingCaseFamily::MemcpyTraceRegression,
            tassadar_seeded_linear_memory_memcpy_program(64),
            vec![190],
        )?,
    ];
    Ok(TassadarMemoryAbiV2TrainingSuite::new(cases))
}

fn build_case(
    case_id: &str,
    family: TassadarMemoryAbiV2TrainingCaseFamily,
    program: TassadarLinearMemoryProgram,
    expected_outputs: Vec<i32>,
) -> Result<TassadarMemoryAbiV2TrainingCase, TassadarMemoryAbiV2TrainingSuiteError> {
    let execution = execute_tassadar_linear_memory_program(&program)?;
    let trace_footprint = summarize_tassadar_linear_memory_trace_footprint(&execution)?;
    Ok(TassadarMemoryAbiV2TrainingCase {
        case_id: String::from(case_id),
        family,
        program_id: program.program_id,
        expected_outputs,
        observed_outputs: execution.outputs,
        trace_footprint,
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
        TassadarMemoryAbiV2TrainingCaseFamily, build_tassadar_memory_abi_v2_training_suite,
    };

    #[test]
    fn memory_abi_v2_training_suite_is_machine_legible() {
        let suite = build_tassadar_memory_abi_v2_training_suite().expect("suite");
        assert_eq!(suite.cases.len(), 4);
        assert_eq!(
            suite.cases[0].observed_outputs,
            suite.cases[0].expected_outputs
        );
        assert!(!suite.suite_digest.is_empty());
    }

    #[test]
    fn memory_abi_v2_training_suite_captures_memcpy_trace_regression() {
        let suite = build_tassadar_memory_abi_v2_training_suite().expect("suite");
        let memcpy = suite
            .cases
            .iter()
            .find(|case| {
                case.family == TassadarMemoryAbiV2TrainingCaseFamily::MemcpyTraceRegression
            })
            .expect("memcpy case");

        assert_eq!(memcpy.expected_outputs, vec![190]);
        assert_eq!(memcpy.observed_outputs, vec![190]);
        assert!(
            memcpy.trace_footprint.delta_trace_bytes
                < memcpy.trace_footprint.equivalent_full_snapshot_trace_bytes
        );
    }
}
