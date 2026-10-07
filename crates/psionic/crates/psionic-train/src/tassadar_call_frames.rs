use psionic_runtime::{
    execute_tassadar_call_frame_program, tassadar_seeded_call_frame_direct_call_program,
    tassadar_seeded_call_frame_multi_function_program,
    tassadar_seeded_call_frame_recursion_program, tassadar_seeded_call_frame_recursive_sum_program,
    TassadarCallFrameError, TassadarCallFrameExecution, TassadarCallFrameProgram,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Public training-suite family for the bounded call-frame lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarCallFrameTrainingCaseFamily {
    DirectCallParity,
    MultiFunctionReplay,
    BoundedRecursiveExact,
    BoundedRecursionRefusal,
}

/// One training-facing supervised case for the call-frame lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCallFrameTrainingCase {
    pub case_id: String,
    pub family: TassadarCallFrameTrainingCaseFamily,
    pub program_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_return_value: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_return_value: Option<i32>,
    pub trace_step_count: usize,
    pub max_observed_frame_depth: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal_kind: Option<String>,
}

/// Public training-facing suite for the call-frame lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCallFrameTrainingSuite {
    pub schema_version: u16,
    pub suite_id: String,
    pub claim_class: String,
    pub cases: Vec<TassadarCallFrameTrainingCase>,
    pub suite_digest: String,
}

impl TassadarCallFrameTrainingSuite {
    fn new(cases: Vec<TassadarCallFrameTrainingCase>) -> Self {
        let mut suite = Self {
            schema_version: 1,
            suite_id: String::from("tassadar.call_frames.training_suite.v1"),
            claim_class: String::from("execution_truth_compiled_bounded_exactness"),
            cases,
            suite_digest: String::new(),
        };
        suite.suite_digest = stable_digest(b"psionic_tassadar_call_frame_training_suite|", &suite);
        suite
    }
}

#[derive(Debug, Error)]
pub enum TassadarCallFrameTrainingSuiteError {
    #[error(transparent)]
    Runtime(#[from] TassadarCallFrameError),
}

pub fn build_tassadar_call_frame_training_suite(
) -> Result<TassadarCallFrameTrainingSuite, TassadarCallFrameTrainingSuiteError> {
    Ok(TassadarCallFrameTrainingSuite::new(vec![
        build_exact_case(
            "direct_call_parity",
            TassadarCallFrameTrainingCaseFamily::DirectCallParity,
            tassadar_seeded_call_frame_direct_call_program(),
            Some(9),
        )?,
        build_exact_case(
            "multi_function_replay",
            TassadarCallFrameTrainingCaseFamily::MultiFunctionReplay,
            tassadar_seeded_call_frame_multi_function_program(),
            Some(25),
        )?,
        build_exact_case(
            "bounded_recursive_exact",
            TassadarCallFrameTrainingCaseFamily::BoundedRecursiveExact,
            tassadar_seeded_call_frame_recursive_sum_program(),
            Some(15),
        )?,
        build_refusal_case(
            "bounded_recursion_refusal",
            TassadarCallFrameTrainingCaseFamily::BoundedRecursionRefusal,
            tassadar_seeded_call_frame_recursion_program(),
        ),
    ]))
}

fn build_exact_case(
    case_id: &str,
    family: TassadarCallFrameTrainingCaseFamily,
    program: TassadarCallFrameProgram,
    expected_return_value: Option<i32>,
) -> Result<TassadarCallFrameTrainingCase, TassadarCallFrameTrainingSuiteError> {
    let execution = execute_tassadar_call_frame_program(&program)?;
    Ok(case_from_execution(
        case_id,
        family,
        program.program_id,
        expected_return_value,
        execution,
    ))
}

fn case_from_execution(
    case_id: &str,
    family: TassadarCallFrameTrainingCaseFamily,
    program_id: String,
    expected_return_value: Option<i32>,
    execution: TassadarCallFrameExecution,
) -> TassadarCallFrameTrainingCase {
    TassadarCallFrameTrainingCase {
        case_id: String::from(case_id),
        family,
        program_id,
        expected_return_value,
        observed_return_value: execution.returned_value,
        trace_step_count: execution.steps.len(),
        max_observed_frame_depth: execution
            .steps
            .iter()
            .map(|step| step.frame_depth_after)
            .max()
            .unwrap_or_default(),
        refusal_kind: None,
    }
}

fn build_refusal_case(
    case_id: &str,
    family: TassadarCallFrameTrainingCaseFamily,
    program: TassadarCallFrameProgram,
) -> TassadarCallFrameTrainingCase {
    match execute_tassadar_call_frame_program(&program) {
        Ok(execution) => case_from_execution(case_id, family, program.program_id, None, execution),
        Err(error) => TassadarCallFrameTrainingCase {
            case_id: String::from(case_id),
            family,
            program_id: program.program_id,
            expected_return_value: None,
            observed_return_value: None,
            trace_step_count: 0,
            max_observed_frame_depth: 0,
            refusal_kind: Some(match error {
                TassadarCallFrameError::RecursionDepthExceeded { .. } => {
                    String::from("recursion_depth_exceeded")
                }
                _ => String::from("runtime_refusal"),
            }),
        },
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
    use super::{build_tassadar_call_frame_training_suite, TassadarCallFrameTrainingCaseFamily};

    #[test]
    fn call_frame_training_suite_is_machine_legible() {
        let suite = build_tassadar_call_frame_training_suite().expect("suite");
        assert_eq!(suite.cases.len(), 4);
        assert!(!suite.suite_digest.is_empty());
    }

    #[test]
    fn call_frame_training_suite_captures_multi_function_and_refusal_cases() {
        let suite = build_tassadar_call_frame_training_suite().expect("suite");
        let multi = suite
            .cases
            .iter()
            .find(|case| case.family == TassadarCallFrameTrainingCaseFamily::MultiFunctionReplay)
            .expect("multi-function case");
        assert_eq!(multi.observed_return_value, Some(25));
        assert!(multi.max_observed_frame_depth >= 3);

        let recursive = suite
            .cases
            .iter()
            .find(|case| case.family == TassadarCallFrameTrainingCaseFamily::BoundedRecursiveExact)
            .expect("recursive exact case");
        assert_eq!(recursive.observed_return_value, Some(15));
        assert!(recursive.max_observed_frame_depth >= 6);

        let refusal = suite
            .cases
            .iter()
            .find(|case| {
                case.family == TassadarCallFrameTrainingCaseFamily::BoundedRecursionRefusal
            })
            .expect("refusal case");
        assert_eq!(
            refusal.refusal_kind.as_deref(),
            Some("recursion_depth_exceeded")
        );
    }
}
