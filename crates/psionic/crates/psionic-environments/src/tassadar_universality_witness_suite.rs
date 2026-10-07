use serde::{Deserialize, Serialize};
use sha2::Digest;

use psionic_data::{
    tassadar_universality_witness_suite_contract, TassadarUniversalityWitnessExpectation,
    TassadarUniversalityWitnessFamily, TassadarUniversalityWitnessSuiteContract,
    TASSADAR_UNIVERSALITY_WITNESS_SUITE_REPORT_REF,
    TASSADAR_UNIVERSALITY_WITNESS_SUITE_SUMMARY_REF,
};

use crate::TassadarEnvironmentError;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarUniversalityWitnessSuiteBinding {
    pub suite_ref: String,
    pub suite_version: String,
    pub exact_family_ids: Vec<TassadarUniversalityWitnessFamily>,
    pub refusal_boundary_family_ids: Vec<TassadarUniversalityWitnessFamily>,
    pub evaluation_axes: Vec<String>,
    pub report_ref: String,
    pub summary_report_ref: String,
}

impl TassadarUniversalityWitnessSuiteBinding {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let encoded =
            serde_json::to_vec(self).expect("universality witness suite binding should serialize");
        let digest = sha2::Sha256::digest(encoded.as_slice());
        hex::encode(digest)
    }

    pub fn validate(&self) -> Result<(), TassadarEnvironmentError> {
        if self.suite_ref.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingUniversalityWitnessSuiteRef);
        }
        if self.suite_version.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingUniversalityWitnessSuiteVersion);
        }
        if self.exact_family_ids.is_empty() {
            return Err(TassadarEnvironmentError::MissingUniversalityWitnessExactFamilies);
        }
        if self.refusal_boundary_family_ids.is_empty() {
            return Err(TassadarEnvironmentError::MissingUniversalityWitnessRefusalFamilies);
        }
        if self.evaluation_axes.is_empty() {
            return Err(TassadarEnvironmentError::MissingUniversalityWitnessEvaluationAxes);
        }
        if self
            .evaluation_axes
            .iter()
            .any(|evaluation_axis| evaluation_axis.trim().is_empty())
        {
            return Err(TassadarEnvironmentError::InvalidUniversalityWitnessEvaluationAxis);
        }
        if self.report_ref.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingUniversalityWitnessReportRef);
        }
        if self.summary_report_ref.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingUniversalityWitnessSummaryRef);
        }
        Ok(())
    }
}

#[must_use]
pub fn default_tassadar_universality_witness_suite_binding(
) -> TassadarUniversalityWitnessSuiteBinding {
    let contract = tassadar_universality_witness_suite_contract();
    binding_from_contract(&contract)
}

fn binding_from_contract(
    contract: &TassadarUniversalityWitnessSuiteContract,
) -> TassadarUniversalityWitnessSuiteBinding {
    TassadarUniversalityWitnessSuiteBinding {
        suite_ref: contract.suite_ref.clone(),
        suite_version: contract.version.clone(),
        exact_family_ids: contract
            .family_ids_by_expectation(TassadarUniversalityWitnessExpectation::Exact),
        refusal_boundary_family_ids: contract
            .family_ids_by_expectation(TassadarUniversalityWitnessExpectation::RefusalBoundary),
        evaluation_axes: contract.evaluation_axes.clone(),
        report_ref: String::from(TASSADAR_UNIVERSALITY_WITNESS_SUITE_REPORT_REF),
        summary_report_ref: String::from(TASSADAR_UNIVERSALITY_WITNESS_SUITE_SUMMARY_REF),
    }
}

#[cfg(test)]
mod tests {
    use super::default_tassadar_universality_witness_suite_binding;
    use psionic_data::{
        TassadarUniversalityWitnessFamily, TASSADAR_UNIVERSALITY_WITNESS_SUITE_REPORT_REF,
        TASSADAR_UNIVERSALITY_WITNESS_SUITE_SUMMARY_REF,
    };

    #[test]
    fn universality_witness_suite_binding_tracks_contract_families() {
        let binding = default_tassadar_universality_witness_suite_binding();

        binding.validate().expect("binding should validate");
        assert_eq!(binding.exact_family_ids.len(), 5);
        assert_eq!(binding.refusal_boundary_family_ids.len(), 2);
        assert!(binding
            .exact_family_ids
            .contains(&TassadarUniversalityWitnessFamily::RegisterMachine));
        assert!(binding
            .refusal_boundary_family_ids
            .contains(&TassadarUniversalityWitnessFamily::ExternalEventLoopBoundary));
        assert_eq!(
            binding.report_ref,
            TASSADAR_UNIVERSALITY_WITNESS_SUITE_REPORT_REF
        );
        assert_eq!(
            binding.summary_report_ref,
            TASSADAR_UNIVERSALITY_WITNESS_SUITE_SUMMARY_REF
        );
    }
}
