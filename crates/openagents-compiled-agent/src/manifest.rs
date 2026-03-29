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

