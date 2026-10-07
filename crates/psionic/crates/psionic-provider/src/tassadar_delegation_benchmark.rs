use serde::{Deserialize, Serialize};

use psionic_router::TassadarInternalExternalDelegationRouteMatrix;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInternalExternalDelegationReceipt {
    pub matrix_id: String,
    pub internal_win_count: u32,
    pub cpu_reference_win_count: u32,
    pub external_sandbox_win_count: u32,
    pub hybrid_only_count: u32,
    pub internal_cost_per_correct_job_milliunits: u32,
    pub cpu_reference_cost_per_correct_job_milliunits: u32,
    pub external_sandbox_cost_per_correct_job_milliunits: u32,
    pub detail: String,
}

impl TassadarInternalExternalDelegationReceipt {
    #[must_use]
    pub fn from_matrix(matrix: &TassadarInternalExternalDelegationRouteMatrix) -> Self {
        Self {
            matrix_id: matrix.matrix_id.clone(),
            internal_win_count: matrix.internal_win_count,
            cpu_reference_win_count: matrix.cpu_reference_win_count,
            external_sandbox_win_count: matrix.external_sandbox_win_count,
            hybrid_only_count: matrix.hybrid_only_count,
            internal_cost_per_correct_job_milliunits: matrix
                .internal_cost_per_correct_job_milliunits,
            cpu_reference_cost_per_correct_job_milliunits: matrix
                .cpu_reference_cost_per_correct_job_milliunits,
            external_sandbox_cost_per_correct_job_milliunits: matrix
                .external_sandbox_cost_per_correct_job_milliunits,
            detail: format!(
                "delegation matrix `{}` currently reports wins internal={}, cpu_reference={}, external_sandbox={}, hybrid={} with cost_per_correct_job internal/cpu/external = {}/{}/{}",
                matrix.matrix_id,
                matrix.internal_win_count,
                matrix.cpu_reference_win_count,
                matrix.external_sandbox_win_count,
                matrix.hybrid_only_count,
                matrix.internal_cost_per_correct_job_milliunits,
                matrix.cpu_reference_cost_per_correct_job_milliunits,
                matrix.external_sandbox_cost_per_correct_job_milliunits,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TassadarInternalExternalDelegationReceipt;
    use psionic_router::build_tassadar_internal_external_delegation_route_matrix;

    #[test]
    fn internal_external_delegation_receipt_projects_route_matrix() {
        let matrix = build_tassadar_internal_external_delegation_route_matrix().expect("matrix");
        let receipt = TassadarInternalExternalDelegationReceipt::from_matrix(&matrix);

        assert_eq!(receipt.internal_win_count, 2);
        assert_eq!(receipt.cpu_reference_win_count, 2);
        assert_eq!(receipt.external_sandbox_win_count, 1);
        assert_eq!(receipt.hybrid_only_count, 1);
    }
}
