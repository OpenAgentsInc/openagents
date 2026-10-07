use serde::{Deserialize, Serialize};

use psionic_runtime::TassadarQuantizationEnvelopePosture;
use psionic_serve::TassadarServedQuantizationTruthEnvelope;

/// Provider-facing deployment truth envelope for the served executor lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarDeploymentTruthEnvelope {
    /// Runtime backend currently serving the executor lane.
    pub runtime_backend: String,
    /// Active backend family exported by the provider.
    pub active_backend_family: psionic_runtime::TassadarQuantizationBackendFamily,
    /// Exact publication envelope ids for the active backend family.
    pub exact_envelope_ids: Vec<String>,
    /// Constrained publication envelope ids for the active backend family.
    pub constrained_envelope_ids: Vec<String>,
    /// Refused publication envelope ids for the active backend family.
    pub refused_envelope_ids: Vec<String>,
    /// Stable validation refs anchoring the lane.
    pub validation_refs: Vec<String>,
    /// Stable publication digest for the lane.
    pub publication_digest: String,
}

impl TassadarDeploymentTruthEnvelope {
    /// Builds a provider-facing deployment truth envelope from the served publication.
    pub fn from_served_quantization_truth_envelope(
        served: &TassadarServedQuantizationTruthEnvelope,
    ) -> Result<Self, TassadarDeploymentTruthEnvelopeError> {
        if served.active_envelopes.is_empty() {
            return Err(TassadarDeploymentTruthEnvelopeError::MissingActiveEnvelope {
                runtime_backend: served.runtime_backend.clone(),
            });
        }
        if let Some(receipt) = served
            .active_envelopes
            .iter()
            .find(|receipt| receipt.benchmark_refs.is_empty())
        {
            return Err(
                TassadarDeploymentTruthEnvelopeError::MissingBenchmarkEvidence {
                    envelope_id: receipt.envelope_id.clone(),
                },
            );
        }
        let exact_envelope_ids = served
            .active_envelopes
            .iter()
            .filter(|receipt| {
                receipt.publication_posture == TassadarQuantizationEnvelopePosture::PublishExact
            })
            .map(|receipt| receipt.envelope_id.clone())
            .collect::<Vec<_>>();
        let constrained_envelope_ids = served
            .active_envelopes
            .iter()
            .filter(|receipt| {
                receipt.publication_posture
                    == TassadarQuantizationEnvelopePosture::PublishConstrained
            })
            .map(|receipt| receipt.envelope_id.clone())
            .collect::<Vec<_>>();
        let refused_envelope_ids = served
            .active_envelopes
            .iter()
            .filter(|receipt| {
                receipt.publication_posture
                    == TassadarQuantizationEnvelopePosture::RefusePublication
            })
            .map(|receipt| receipt.envelope_id.clone())
            .collect::<Vec<_>>();
        Ok(Self {
            runtime_backend: served.runtime_backend.clone(),
            active_backend_family: served.active_backend_family,
            exact_envelope_ids,
            constrained_envelope_ids,
            refused_envelope_ids,
            validation_refs: served.publication.validation_refs.clone(),
            publication_digest: served.publication.publication_digest.clone(),
        })
    }
}

/// Provider-facing deployment-envelope failure.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarDeploymentTruthEnvelopeError {
    /// The served backend family did not export any active envelopes.
    MissingActiveEnvelope {
        /// Runtime backend label.
        runtime_backend: String,
    },
    /// One active envelope omitted its benchmark lineage.
    MissingBenchmarkEvidence {
        /// Stable envelope identifier.
        envelope_id: String,
    },
}

#[cfg(test)]
mod tests {
    use super::{TassadarDeploymentTruthEnvelope, TassadarDeploymentTruthEnvelopeError};
    use psionic_serve::build_tassadar_served_quantization_truth_envelope;

    #[test]
    fn deployment_truth_envelope_projects_served_publication() {
        let served =
            build_tassadar_served_quantization_truth_envelope("metal").expect("metal envelope");
        let envelope = TassadarDeploymentTruthEnvelope::from_served_quantization_truth_envelope(
            &served,
        )
        .expect("provider envelope");

        assert_eq!(envelope.runtime_backend, "metal");
        assert!(envelope
            .constrained_envelope_ids
            .contains(&String::from("metal_served_bf16_cast")));
        assert!(envelope
            .constrained_envelope_ids
            .contains(&String::from("metal_served_fp8_block")));
    }

    #[test]
    fn deployment_truth_envelope_rejects_missing_benchmark_refs() {
        let mut served =
            build_tassadar_served_quantization_truth_envelope("cpu").expect("cpu envelope");
        served.active_envelopes[0].benchmark_refs.clear();

        let err = TassadarDeploymentTruthEnvelope::from_served_quantization_truth_envelope(
            &served,
        )
        .expect_err("benchmark refs should be required");
        assert_eq!(
            err,
            TassadarDeploymentTruthEnvelopeError::MissingBenchmarkEvidence {
                envelope_id: String::from("cpu_reference_fp32_dense"),
            }
        );
    }
}
