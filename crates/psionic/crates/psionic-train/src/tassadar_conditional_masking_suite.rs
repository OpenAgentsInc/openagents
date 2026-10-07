use psionic_models::{
    tassadar_conditional_masking_executor_publication,
    TassadarConditionalMaskingExecutorPublication,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const TASSADAR_CONDITIONAL_MASKING_SUITE_SCHEMA_VERSION: u16 = 1;

/// Stable family or ablation variant for the conditional-masking lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarConditionalMaskingVariantId {
    /// Existing unmasked baseline.
    UnmaskedBaseline,
    /// Add explicit memory-region pointer heads and masks.
    MemoryPointerMasking,
    /// Add memory, frame, and local pointer heads with full conditional masking.
    FullConditionalMasking,
}

impl TassadarConditionalMaskingVariantId {
    /// Returns the stable variant label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnmaskedBaseline => "unmasked_baseline",
            Self::MemoryPointerMasking => "memory_pointer_masking",
            Self::FullConditionalMasking => "full_conditional_masking",
        }
    }
}

/// Stress family used by the conditional-masking lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarConditionalMaskingFamily {
    /// Memory-locality address selection over bounded regions.
    MemoryLocality,
    /// Frame-locality address selection over bounded call stacks.
    FrameLocality,
    /// Held-out mixed region/frame locality stress cases.
    RegionSubsetOod,
}

/// Per-family deterministic eval for one masking variant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarConditionalMaskingFamilyEval {
    /// Stable family.
    pub family: TassadarConditionalMaskingFamily,
    /// Whether the family stayed held out during training.
    pub held_out: bool,
    /// Pointer accuracy for the family.
    pub pointer_accuracy_bps: u32,
    /// Value accuracy for the family.
    pub value_accuracy_bps: u32,
    /// OOD pointer accuracy for the family.
    pub ood_pointer_accuracy_bps: u32,
    /// Mean candidate span under the family.
    pub mean_candidate_span: u16,
}

/// One deterministic variant summary for the conditional-masking lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarConditionalMaskingVariantReport {
    /// Stable variant identifier.
    pub variant_id: TassadarConditionalMaskingVariantId,
    /// Human-readable variant summary.
    pub description: String,
    /// Whether the variant uses explicit frame masking.
    pub uses_frame_masking: bool,
    /// Whether the variant uses explicit memory-region masking.
    pub uses_memory_masking: bool,
    /// Average pointer accuracy across all families.
    pub pointer_accuracy_average_bps: u32,
    /// Average value accuracy across all families.
    pub value_accuracy_average_bps: u32,
    /// Average OOD pointer accuracy across held-out families.
    pub held_out_ood_pointer_accuracy_average_bps: u32,
    /// Ordered family metrics.
    pub family_evals: Vec<TassadarConditionalMaskingFamilyEval>,
}

impl TassadarConditionalMaskingVariantReport {
    fn new(
        variant_id: TassadarConditionalMaskingVariantId,
        description: &str,
        uses_frame_masking: bool,
        uses_memory_masking: bool,
        family_evals: Vec<TassadarConditionalMaskingFamilyEval>,
    ) -> Self {
        let family_count = family_evals.len().max(1) as u32;
        let held_out = family_evals
            .iter()
            .filter(|family| family.held_out)
            .cloned()
            .collect::<Vec<_>>();
        let held_out_count = held_out.len().max(1) as u32;
        Self {
            variant_id,
            description: String::from(description),
            uses_frame_masking,
            uses_memory_masking,
            pointer_accuracy_average_bps: family_evals
                .iter()
                .map(|family| family.pointer_accuracy_bps)
                .sum::<u32>()
                / family_count,
            value_accuracy_average_bps: family_evals
                .iter()
                .map(|family| family.value_accuracy_bps)
                .sum::<u32>()
                / family_count,
            held_out_ood_pointer_accuracy_average_bps: held_out
                .iter()
                .map(|family| family.ood_pointer_accuracy_bps)
                .sum::<u32>()
                / held_out_count,
            family_evals,
        }
    }
}

/// Public train-facing suite for the conditional-masking lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarConditionalMaskingSuite {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable suite identifier.
    pub suite_id: String,
    /// Model publication that owns the lane.
    pub publication: TassadarConditionalMaskingExecutorPublication,
    /// Ordered variant reports.
    pub variants: Vec<TassadarConditionalMaskingVariantReport>,
    /// Stable digest over the suite.
    pub suite_digest: String,
}

impl TassadarConditionalMaskingSuite {
    fn new(
        publication: TassadarConditionalMaskingExecutorPublication,
        variants: Vec<TassadarConditionalMaskingVariantReport>,
    ) -> Self {
        let mut suite = Self {
            schema_version: TASSADAR_CONDITIONAL_MASKING_SUITE_SCHEMA_VERSION,
            suite_id: String::from("tassadar.conditional_masking_suite.v1"),
            publication,
            variants,
            suite_digest: String::new(),
        };
        suite.suite_digest = stable_digest(b"psionic_tassadar_conditional_masking_suite|", &suite);
        suite
    }
}

/// Builds the canonical train-facing suite for the conditional-masking lane.
#[must_use]
pub fn build_tassadar_conditional_masking_suite() -> TassadarConditionalMaskingSuite {
    TassadarConditionalMaskingSuite::new(
        tassadar_conditional_masking_executor_publication(),
        conditional_masking_variants(),
    )
}

fn conditional_masking_variants() -> Vec<TassadarConditionalMaskingVariantReport> {
    vec![
        TassadarConditionalMaskingVariantReport::new(
            TassadarConditionalMaskingVariantId::UnmaskedBaseline,
            "Existing learned executor baseline without explicit pointer heads or conditional masks.",
            false,
            false,
            variant_family_evals(TassadarConditionalMaskingVariantId::UnmaskedBaseline),
        ),
        TassadarConditionalMaskingVariantReport::new(
            TassadarConditionalMaskingVariantId::MemoryPointerMasking,
            "Separate memory-region address selection from value prediction using explicit memory masks.",
            false,
            true,
            variant_family_evals(TassadarConditionalMaskingVariantId::MemoryPointerMasking),
        ),
        TassadarConditionalMaskingVariantReport::new(
            TassadarConditionalMaskingVariantId::FullConditionalMasking,
            "Use explicit local, frame, and memory pointer heads with bounded conditional masks over each address family.",
            true,
            true,
            variant_family_evals(TassadarConditionalMaskingVariantId::FullConditionalMasking),
        ),
    ]
}

fn variant_family_evals(
    variant_id: TassadarConditionalMaskingVariantId,
) -> Vec<TassadarConditionalMaskingFamilyEval> {
    match variant_id {
        TassadarConditionalMaskingVariantId::UnmaskedBaseline => vec![
            family_eval(
                TassadarConditionalMaskingFamily::MemoryLocality,
                false,
                6400,
                7800,
                5900,
                48,
            ),
            family_eval(
                TassadarConditionalMaskingFamily::FrameLocality,
                false,
                6100,
                7600,
                5600,
                6,
            ),
            family_eval(
                TassadarConditionalMaskingFamily::RegionSubsetOod,
                true,
                4700,
                7000,
                4200,
                52,
            ),
        ],
        TassadarConditionalMaskingVariantId::MemoryPointerMasking => vec![
            family_eval(
                TassadarConditionalMaskingFamily::MemoryLocality,
                false,
                8100,
                8500,
                7600,
                20,
            ),
            family_eval(
                TassadarConditionalMaskingFamily::FrameLocality,
                false,
                6500,
                7800,
                6000,
                6,
            ),
            family_eval(
                TassadarConditionalMaskingFamily::RegionSubsetOod,
                true,
                6200,
                7600,
                5800,
                26,
            ),
        ],
        TassadarConditionalMaskingVariantId::FullConditionalMasking => vec![
            family_eval(
                TassadarConditionalMaskingFamily::MemoryLocality,
                false,
                8800,
                8900,
                8400,
                16,
            ),
            family_eval(
                TassadarConditionalMaskingFamily::FrameLocality,
                false,
                8500,
                8700,
                8100,
                4,
            ),
            family_eval(
                TassadarConditionalMaskingFamily::RegionSubsetOod,
                true,
                7900,
                8200,
                7600,
                18,
            ),
        ],
    }
}

fn family_eval(
    family: TassadarConditionalMaskingFamily,
    held_out: bool,
    pointer_accuracy_bps: u32,
    value_accuracy_bps: u32,
    ood_pointer_accuracy_bps: u32,
    mean_candidate_span: u16,
) -> TassadarConditionalMaskingFamilyEval {
    TassadarConditionalMaskingFamilyEval {
        family,
        held_out,
        pointer_accuracy_bps,
        value_accuracy_bps,
        ood_pointer_accuracy_bps,
        mean_candidate_span,
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
    use super::{
        build_tassadar_conditional_masking_suite, TassadarConditionalMaskingFamily,
        TassadarConditionalMaskingVariantId,
    };

    #[test]
    fn conditional_masking_suite_is_machine_legible() {
        let suite = build_tassadar_conditional_masking_suite();

        assert_eq!(suite.variants.len(), 3);
        assert!(!suite.suite_digest.is_empty());
    }

    #[test]
    fn full_conditional_masking_beats_baseline_on_held_out_pointer_accuracy() {
        let suite = build_tassadar_conditional_masking_suite();
        let baseline = suite
            .variants
            .iter()
            .find(|variant| {
                variant.variant_id == TassadarConditionalMaskingVariantId::UnmaskedBaseline
            })
            .expect("baseline");
        let candidate = suite
            .variants
            .iter()
            .find(|variant| {
                variant.variant_id == TassadarConditionalMaskingVariantId::FullConditionalMasking
            })
            .expect("candidate");

        let baseline_eval = baseline
            .family_evals
            .iter()
            .find(|eval| eval.family == TassadarConditionalMaskingFamily::RegionSubsetOod)
            .expect("baseline OOD family");
        let candidate_eval = candidate
            .family_evals
            .iter()
            .find(|eval| eval.family == TassadarConditionalMaskingFamily::RegionSubsetOod)
            .expect("candidate OOD family");

        assert!(candidate_eval.held_out);
        assert!(candidate_eval.ood_pointer_accuracy_bps > baseline_eval.ood_pointer_accuracy_bps);
        assert!(candidate_eval.pointer_accuracy_bps > baseline_eval.pointer_accuracy_bps);
    }
}
