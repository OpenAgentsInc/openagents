use serde::{Deserialize, Serialize};

/// Lifecycle state for a compiled module artifact.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModulePromotionState {
    /// Promoted artifact that may become product authority.
    Promoted,
    /// Candidate artifact under evaluation only.
    Candidate,
}

/// Stable manifest for a compiled module artifact.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledModuleManifest {
    /// Human-readable module slot name.
    pub module_name: String,
    /// Stable signature name for this slot.
    pub signature_name: String,
    /// Implementation family such as `rule_v1`, `qwen`, or `psion`.
    pub implementation_family: String,
    /// Unique implementation label inside the family.
    pub implementation_label: String,
    /// Semantic or build version of the compiled module.
    pub version: String,
    /// Current lifecycle state of the module.
    pub promotion_state: ModulePromotionState,
    /// Minimum confidence this module claims as acceptable authority.
    pub confidence_floor: f32,
    /// Stable artifact id when the module comes from a promoted artifact contract.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_id: Option<String>,
    /// Stable artifact digest for receipt lineage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_digest: Option<String>,
    /// Compatibility contract for the consuming graph.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compatibility_version: Option<String>,
    /// Default learned row id that admitted this artifact when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub row_id: Option<String>,
    /// Rollback target artifact id when the current artifact is promoted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rollback_artifact_id: Option<String>,
}

impl CompiledModuleManifest {
    /// Stable opaque identifier for storage and receipts.
    #[must_use]
    pub fn manifest_id(&self) -> String {
        format!(
            "{}:{}:{}:{}",
            self.module_name, self.implementation_family, self.implementation_label, self.version
        )
    }
}
