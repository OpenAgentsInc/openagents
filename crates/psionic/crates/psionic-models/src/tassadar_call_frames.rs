use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const TASSADAR_CALL_FRAME_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_call_frame_report.json";

/// Machine-legible publication status for the bounded call-frame lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarCallFramePublicationStatus {
    /// Landed as a repo-backed public substrate surface.
    Implemented,
}

/// Public model-facing publication for the bounded call-frame lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCallFramePublication {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable publication identifier.
    pub publication_id: String,
    /// Repo status vocabulary value for the lane.
    pub status: TassadarCallFramePublicationStatus,
    /// Explicit claim class for the lane.
    pub claim_class: String,
    /// Whether direct multi-function calls are supported.
    pub supports_direct_calls: bool,
    /// Whether bounded exact recursion succeeds below the declared depth cap.
    pub supports_bounded_exact_recursion: bool,
    /// Whether the lane carries explicit in-frame conditional control.
    pub supports_conditional_control: bool,
    /// Whether full frame-stack snapshots are carried in the trace.
    pub traces_include_frame_stack: bool,
    /// Maximum bounded call depth before recursion is refused.
    pub max_call_depth: u32,
    /// Stable target surfaces implementing the lane.
    pub target_surfaces: Vec<String>,
    /// Stable validation refs for the lane.
    pub validation_refs: Vec<String>,
    /// Explicit support boundaries that remain out of scope.
    pub support_boundaries: Vec<String>,
    /// Stable digest over the publication.
    pub publication_digest: String,
}

impl TassadarCallFramePublication {
    fn new() -> Self {
        let mut publication = Self {
            schema_version: 1,
            publication_id: String::from("tassadar.call_frames.publication.v1"),
            status: TassadarCallFramePublicationStatus::Implemented,
            claim_class: String::from("execution_truth_compiled_bounded_exactness"),
            supports_direct_calls: true,
            supports_bounded_exact_recursion: true,
            supports_conditional_control: true,
            traces_include_frame_stack: true,
            max_call_depth: 8,
            target_surfaces: vec![
                String::from("crates/psionic-runtime"),
                String::from("crates/psionic-models"),
                String::from("crates/psionic-train"),
                String::from("crates/psionic-eval"),
            ],
            validation_refs: vec![String::from(TASSADAR_CALL_FRAME_REPORT_REF)],
            support_boundaries: vec![
                String::from(
                    "supports direct function calls with 0 or 1 return values under one bounded frame stack plus conditional in-frame control; call_indirect and import boundaries remain separate work",
                ),
                String::from(
                    "exact recursion is supported below explicit max_call_depth and refuses once that cap would be exceeded",
                ),
                String::from(
                    "this publication does not claim arbitrary Wasm closure, tail calls, host imports, or learned-lane generalization",
                ),
            ],
            publication_digest: String::new(),
        };
        publication.publication_digest =
            stable_digest(b"psionic_tassadar_call_frame_publication|", &publication);
        publication
    }
}

/// Returns the canonical public publication for the bounded call-frame lane.
#[must_use]
pub fn tassadar_call_frame_publication() -> TassadarCallFramePublication {
    TassadarCallFramePublication::new()
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{tassadar_call_frame_publication, TassadarCallFramePublicationStatus};

    #[test]
    fn call_frame_publication_is_machine_legible() {
        let publication = tassadar_call_frame_publication();
        assert_eq!(
            publication.status,
            TassadarCallFramePublicationStatus::Implemented
        );
        assert!(publication.supports_direct_calls);
        assert!(publication.supports_bounded_exact_recursion);
        assert!(publication.supports_conditional_control);
        assert!(publication.traces_include_frame_stack);
        assert_eq!(
            publication.validation_refs,
            vec![String::from(super::TASSADAR_CALL_FRAME_REPORT_REF)]
        );
    }
}
