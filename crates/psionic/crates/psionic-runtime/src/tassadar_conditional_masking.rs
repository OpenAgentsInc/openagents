use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const TASSADAR_CONDITIONAL_MASKING_CONTRACT_SCHEMA_VERSION: u16 = 1;

/// One bounded address-selection domain admitted by the conditional-masking lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarAddressSelectionDomain {
    /// One bounded local-slot choice inside the active frame.
    LocalSlot,
    /// One bounded frame-slot choice inside the active call stack.
    CallFrame,
    /// One bounded memory-region choice over a declared contiguous span.
    MemoryRegion,
}

impl TassadarAddressSelectionDomain {
    /// Returns the stable domain label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LocalSlot => "local_slot",
            Self::CallFrame => "call_frame",
            Self::MemoryRegion => "memory_region",
        }
    }
}

/// One conditional mask family admitted by the bounded address-selection lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarConditionalMaskKind {
    /// Restrict choices to a bounded local-slot window.
    LocalWindow,
    /// Restrict choices to the active or adjacent frame window.
    FrameWindow,
    /// Restrict choices to one contiguous memory region.
    MemoryRegionWindow,
}

/// Explicit refusal kinds for out-of-family masked-address workloads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarAddressSelectionRefusalKind {
    /// The requested address domain is unsupported.
    UnsupportedDomain,
    /// The requested mask family is unsupported.
    UnsupportedMaskKind,
    /// The requested local or frame window is too wide.
    WindowTooWide,
    /// The requested frame depth exceeds the bounded family.
    FrameDepthTooDeep,
    /// The requested memory region exceeds the bounded span.
    MemoryRegionTooWide,
}

/// Runtime-owned contract for the bounded conditional-masking and
/// address-selection lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarConditionalMaskingContract {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable contract identifier.
    pub contract_id: String,
    /// Address-selection domains admitted by the lane.
    pub supported_domains: Vec<TassadarAddressSelectionDomain>,
    /// Mask kinds admitted by the lane.
    pub supported_mask_kinds: Vec<TassadarConditionalMaskKind>,
    /// Maximum local-slot window width admitted by the lane.
    pub max_local_window_width: u16,
    /// Maximum frame depth admitted by the lane.
    pub max_frame_depth: u16,
    /// Maximum contiguous memory-region span admitted by the lane.
    pub max_memory_region_span: u16,
    /// Explicit refusal kinds for out-of-family requests.
    pub refusal_kinds: Vec<TassadarAddressSelectionRefusalKind>,
    /// Plain-language refusal boundary for the lane.
    pub refusal_boundary: String,
    /// Stable digest over the contract.
    pub contract_digest: String,
}

impl TassadarConditionalMaskingContract {
    fn new() -> Self {
        let mut contract = Self {
            schema_version: TASSADAR_CONDITIONAL_MASKING_CONTRACT_SCHEMA_VERSION,
            contract_id: String::from("tassadar.conditional_masking.contract.v1"),
            supported_domains: vec![
                TassadarAddressSelectionDomain::LocalSlot,
                TassadarAddressSelectionDomain::CallFrame,
                TassadarAddressSelectionDomain::MemoryRegion,
            ],
            supported_mask_kinds: vec![
                TassadarConditionalMaskKind::LocalWindow,
                TassadarConditionalMaskKind::FrameWindow,
                TassadarConditionalMaskKind::MemoryRegionWindow,
            ],
            max_local_window_width: 16,
            max_frame_depth: 8,
            max_memory_region_span: 64,
            refusal_kinds: vec![
                TassadarAddressSelectionRefusalKind::UnsupportedDomain,
                TassadarAddressSelectionRefusalKind::UnsupportedMaskKind,
                TassadarAddressSelectionRefusalKind::WindowTooWide,
                TassadarAddressSelectionRefusalKind::FrameDepthTooDeep,
                TassadarAddressSelectionRefusalKind::MemoryRegionTooWide,
            ],
            refusal_boundary: String::from(
                "conditional masking stays bounded to declared local-slot windows, bounded frame windows, and contiguous memory-region spans only; out-of-family address domains, wider spans, or deeper frame traversals must refuse explicitly instead of silently widening learned access",
            ),
            contract_digest: String::new(),
        };
        contract.contract_digest =
            stable_digest(b"psionic_tassadar_conditional_masking_contract|", &contract);
        contract
    }
}

/// Returns the canonical bounded conditional-masking contract.
#[must_use]
pub fn tassadar_conditional_masking_contract() -> TassadarConditionalMaskingContract {
    TassadarConditionalMaskingContract::new()
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
        tassadar_conditional_masking_contract, TassadarAddressSelectionDomain,
        TassadarAddressSelectionRefusalKind, TassadarConditionalMaskKind,
    };

    #[test]
    fn conditional_masking_contract_is_machine_legible() {
        let contract = tassadar_conditional_masking_contract();

        assert_eq!(
            contract.contract_id,
            "tassadar.conditional_masking.contract.v1"
        );
        assert!(contract
            .supported_domains
            .contains(&TassadarAddressSelectionDomain::MemoryRegion));
        assert!(contract
            .supported_mask_kinds
            .contains(&TassadarConditionalMaskKind::FrameWindow));
        assert!(contract
            .refusal_kinds
            .contains(&TassadarAddressSelectionRefusalKind::MemoryRegionTooWide));
        assert!(!contract.contract_digest.is_empty());
    }

    #[test]
    fn conditional_masking_contract_keeps_window_and_depth_boundaries_explicit() {
        let contract = tassadar_conditional_masking_contract();

        assert_eq!(contract.max_local_window_width, 16);
        assert_eq!(contract.max_frame_depth, 8);
        assert_eq!(contract.max_memory_region_span, 64);
        assert!(contract.refusal_boundary.contains("must refuse explicitly"));
    }
}
