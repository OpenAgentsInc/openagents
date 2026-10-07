use psionic_ir::{
    TASSADAR_SYMBOLIC_PROGRAM_CLAIM_CLASS, TASSADAR_SYMBOLIC_PROGRAM_LANGUAGE_ID,
    TassadarSymbolicProgramExample, tassadar_symbolic_program_examples,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Model-facing machine-legible suite of bounded symbolic executor IR examples.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSymbolicProgramSuite {
    /// Stable symbolic IR language identifier.
    pub language_id: String,
    /// Coarse claim class for the suite.
    pub claim_class: String,
    /// Ordered seeded examples.
    pub programs: Vec<TassadarSymbolicProgramExample>,
    /// Stable digest over the suite.
    pub suite_digest: String,
}

impl TassadarSymbolicProgramSuite {
    fn new(programs: Vec<TassadarSymbolicProgramExample>) -> Self {
        let mut suite = Self {
            language_id: String::from(TASSADAR_SYMBOLIC_PROGRAM_LANGUAGE_ID),
            claim_class: String::from(TASSADAR_SYMBOLIC_PROGRAM_CLAIM_CLASS),
            programs,
            suite_digest: String::new(),
        };
        suite.suite_digest = stable_digest(&suite);
        suite
    }
}

/// Returns the canonical bounded symbolic program suite for the Tassadar lane.
#[must_use]
pub fn tassadar_symbolic_program_suite() -> TassadarSymbolicProgramSuite {
    TassadarSymbolicProgramSuite::new(tassadar_symbolic_program_examples())
}

fn stable_digest<T: Serialize>(value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"tassadar_symbolic_program_suite|");
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::tassadar_symbolic_program_suite;

    #[test]
    fn symbolic_program_suite_is_machine_legible() {
        let suite = tassadar_symbolic_program_suite();
        assert_eq!(suite.language_id, "tassadar.symbolic_executor_ir.v1");
        assert_eq!(suite.claim_class, "compiled_bounded_exactness");
        assert_eq!(suite.programs.len(), 5);

        let encoded = serde_json::to_value(&suite).expect("suite should serialize");
        let case_ids = encoded["programs"]
            .as_array()
            .expect("programs array")
            .iter()
            .filter_map(|program| program["case_id"].as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            case_ids,
            vec![
                "addition_pair",
                "parity_two_bits",
                "memory_accumulator",
                "finite_state_counter",
                "stack_machine_add_step",
            ]
        );
    }

    #[test]
    fn symbolic_program_suite_examples_match_seeded_truth() {
        let suite = tassadar_symbolic_program_suite();
        for example in suite.programs {
            let execution = example
                .program
                .evaluate(&example.input_assignments)
                .expect("seeded program should evaluate");
            assert_eq!(execution.outputs, example.expected_outputs);
            assert_eq!(execution.final_memory, example.expected_final_memory);
        }
    }
}
