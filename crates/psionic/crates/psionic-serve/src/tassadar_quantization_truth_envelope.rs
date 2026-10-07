use serde::{Deserialize, Serialize};
use thiserror::Error;

use psionic_models::{
    tassadar_quantization_truth_envelope_publication,
    TassadarQuantizationTruthEnvelopePublication,
};
use psionic_runtime::{
    TassadarQuantizationBackendFamily, TassadarQuantizationTruthEnvelopeReceipt,
};

use crate::EXECUTOR_TRACE_PRODUCT_ID;

/// Served projection of the quantization deployment envelope lane for one runtime backend.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarServedQuantizationTruthEnvelope {
    /// Served product identifier.
    pub product_id: String,
    /// Runtime backend currently serving the executor lane.
    pub runtime_backend: String,
    /// Backend family selected for the current runtime backend.
    pub active_backend_family: TassadarQuantizationBackendFamily,
    /// Repo-facing publication anchoring the lane.
    pub publication: TassadarQuantizationTruthEnvelopePublication,
    /// Deployment envelopes active for the current backend family.
    pub active_envelopes: Vec<TassadarQuantizationTruthEnvelopeReceipt>,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
}

/// Served quantization-envelope publication failure.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TassadarServedQuantizationTruthEnvelopeError {
    /// The runtime backend is not mapped into the current envelope publication.
    #[error("unknown Tassadar runtime backend `{runtime_backend}` for quantization truth envelope")]
    UnknownRuntimeBackend {
        /// Runtime backend label.
        runtime_backend: String,
    },
    /// The selected backend family had no published envelopes.
    #[error("missing active quantization truth envelope for backend family `{backend_family}`")]
    MissingActiveEnvelope {
        /// Stable backend-family label.
        backend_family: String,
    },
}

/// Returns the served quantization deployment envelope for one runtime backend.
pub fn build_tassadar_served_quantization_truth_envelope(
    runtime_backend: &str,
) -> Result<TassadarServedQuantizationTruthEnvelope, TassadarServedQuantizationTruthEnvelopeError>
{
    let active_backend_family = match runtime_backend {
        "cpu" => TassadarQuantizationBackendFamily::CpuReference,
        "metal" => TassadarQuantizationBackendFamily::MetalServed,
        "cuda" => TassadarQuantizationBackendFamily::CudaServed,
        _ => {
            return Err(
                TassadarServedQuantizationTruthEnvelopeError::UnknownRuntimeBackend {
                    runtime_backend: String::from(runtime_backend),
                },
            );
        }
    };
    let publication = tassadar_quantization_truth_envelope_publication();
    let active_envelopes = publication
        .envelope_receipts
        .iter()
        .filter(|receipt| receipt.backend_family == active_backend_family)
        .cloned()
        .collect::<Vec<_>>();
    if active_envelopes.is_empty() {
        return Err(
            TassadarServedQuantizationTruthEnvelopeError::MissingActiveEnvelope {
                backend_family: String::from(active_backend_family.as_str()),
            },
        );
    }
    Ok(TassadarServedQuantizationTruthEnvelope {
        product_id: String::from(EXECUTOR_TRACE_PRODUCT_ID),
        runtime_backend: String::from(runtime_backend),
        active_backend_family,
        publication,
        active_envelopes,
        claim_boundary: String::from(
            "the served executor lane publishes only the deployment envelopes anchored to the current runtime backend family, keeping exact, constrained, and refused backend and quantization posture explicit instead of assuming backend-invariant exactness",
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_served_quantization_truth_envelope,
        TassadarServedQuantizationTruthEnvelopeError,
    };
    use psionic_runtime::{TassadarQuantizationBackendFamily, TassadarQuantizationEnvelopePosture};

    #[test]
    fn served_quantization_truth_envelope_tracks_active_backend_family() {
        let envelope =
            build_tassadar_served_quantization_truth_envelope("cpu").expect("cpu envelope");

        assert_eq!(
            envelope.active_backend_family,
            TassadarQuantizationBackendFamily::CpuReference
        );
        assert_eq!(envelope.active_envelopes.len(), 1);
        assert_eq!(
            envelope.active_envelopes[0].publication_posture,
            TassadarQuantizationEnvelopePosture::PublishExact
        );
    }

    #[test]
    fn served_quantization_truth_envelope_rejects_unknown_backend() {
        let err = build_tassadar_served_quantization_truth_envelope("rocm")
            .expect_err("rocm should stay unknown");

        assert_eq!(
            err,
            TassadarServedQuantizationTruthEnvelopeError::UnknownRuntimeBackend {
                runtime_backend: String::from("rocm"),
            }
        );
    }
}
