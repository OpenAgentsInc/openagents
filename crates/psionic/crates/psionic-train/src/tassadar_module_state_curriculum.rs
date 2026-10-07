use psionic_models::{
    TassadarModuleStateChannelKind, TassadarModuleStateCurriculumAnchor,
    TassadarModuleStateProgramFamily, tassadar_module_state_curriculum_anchors,
    tassadar_module_state_executor_publication,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const TASSADAR_MODULE_STATE_CURRICULUM_SUITE_SCHEMA_VERSION: u16 = 1;

/// Stable curriculum or ablation variant for the module-state redesign lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarModuleStateCurriculumVariantId {
    /// Existing flat-prefix token-trace baseline.
    FlatPrefixBaseline,
    /// Frame-local recurrent state without explicit memory-delta replay.
    FrameStateOnly,
    /// Frame-local state plus explicit memory-delta channels.
    FrameStateWithMemoryDelta,
    /// Full module-state curriculum with export-boundary state enabled.
    FullModuleCurriculum,
}

impl TassadarModuleStateCurriculumVariantId {
    /// Returns the stable variant label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FlatPrefixBaseline => "flat_prefix_baseline",
            Self::FrameStateOnly => "frame_state_only",
            Self::FrameStateWithMemoryDelta => "frame_state_with_memory_delta",
            Self::FullModuleCurriculum => "full_module_curriculum",
        }
    }
}

/// Deterministic family-level eval for one curriculum variant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleStateFamilyEval {
    /// Stable program family.
    pub family: TassadarModuleStateProgramFamily,
    /// Whether the family stayed held out during the variant curriculum.
    pub held_out: bool,
    /// Later-window exactness for the family.
    pub later_window_exactness_bps: u32,
    /// Final-state exactness for the family.
    pub final_state_exactness_bps: u32,
    /// Gap between final-state accuracy and later-window trace accuracy.
    pub trace_to_final_state_gap_bps: u32,
}

/// One curriculum-variant summary for the module-state redesign lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleStateCurriculumVariantReport {
    /// Stable variant identifier.
    pub variant_id: TassadarModuleStateCurriculumVariantId,
    /// Human-readable variant summary.
    pub description: String,
    /// Stable stage ids applied by the variant.
    pub stage_ids: Vec<String>,
    /// State channels enabled by the variant.
    pub enabled_state_channels: Vec<TassadarModuleStateChannelKind>,
    /// Average later-window exactness across all families.
    pub later_window_average_bps: u32,
    /// Average final-state exactness across all families.
    pub final_state_average_bps: u32,
    /// Average later-window exactness on held-out families only.
    pub held_out_later_window_average_bps: u32,
    /// Average final-state exactness on held-out families only.
    pub held_out_final_state_average_bps: u32,
    /// Average trace-to-final-state gap.
    pub average_trace_to_final_state_gap_bps: u32,
    /// Ordered family metrics for the variant.
    pub family_evals: Vec<TassadarModuleStateFamilyEval>,
}

impl TassadarModuleStateCurriculumVariantReport {
    fn new(
        variant_id: TassadarModuleStateCurriculumVariantId,
        description: &str,
        stage_ids: Vec<String>,
        enabled_state_channels: Vec<TassadarModuleStateChannelKind>,
        family_evals: Vec<TassadarModuleStateFamilyEval>,
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
            stage_ids,
            enabled_state_channels,
            later_window_average_bps: family_evals
                .iter()
                .map(|family| family.later_window_exactness_bps)
                .sum::<u32>()
                / family_count,
            final_state_average_bps: family_evals
                .iter()
                .map(|family| family.final_state_exactness_bps)
                .sum::<u32>()
                / family_count,
            held_out_later_window_average_bps: held_out
                .iter()
                .map(|family| family.later_window_exactness_bps)
                .sum::<u32>()
                / held_out_count,
            held_out_final_state_average_bps: held_out
                .iter()
                .map(|family| family.final_state_exactness_bps)
                .sum::<u32>()
                / held_out_count,
            average_trace_to_final_state_gap_bps: family_evals
                .iter()
                .map(|family| family.trace_to_final_state_gap_bps)
                .sum::<u32>()
                / family_count,
            family_evals,
        }
    }
}

/// Public training-facing curriculum suite for the module-state redesign lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleStateCurriculumSuite {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable suite identifier.
    pub suite_id: String,
    /// Explicit claim class for the suite.
    pub claim_class: String,
    /// Stable model-publication identifier that owns the lane.
    pub model_publication_id: String,
    /// Stable workload-suite report used to seed the module families.
    pub source_workload_suite_ref: String,
    /// Ordered curriculum anchors.
    pub stages: Vec<TassadarModuleStateCurriculumAnchor>,
    /// Ordered variant reports.
    pub variants: Vec<TassadarModuleStateCurriculumVariantReport>,
    /// Stable digest over the suite.
    pub suite_digest: String,
}

impl TassadarModuleStateCurriculumSuite {
    fn new(
        model_publication_id: String,
        claim_class: String,
        source_workload_suite_ref: String,
        stages: Vec<TassadarModuleStateCurriculumAnchor>,
        variants: Vec<TassadarModuleStateCurriculumVariantReport>,
    ) -> Self {
        let mut suite = Self {
            schema_version: TASSADAR_MODULE_STATE_CURRICULUM_SUITE_SCHEMA_VERSION,
            suite_id: String::from("tassadar.module_state_curriculum_suite.v1"),
            claim_class,
            model_publication_id,
            source_workload_suite_ref,
            stages,
            variants,
            suite_digest: String::new(),
        };
        suite.suite_digest =
            stable_digest(b"psionic_tassadar_module_state_curriculum_suite|", &suite);
        suite
    }
}

/// Builds the canonical training-facing curriculum suite for the module-state redesign lane.
#[must_use]
pub fn build_tassadar_module_state_curriculum_suite() -> TassadarModuleStateCurriculumSuite {
    let publication = tassadar_module_state_executor_publication();
    TassadarModuleStateCurriculumSuite::new(
        publication.publication_id.clone(),
        publication.claim_class.clone(),
        publication.source_workload_suite_ref.clone(),
        tassadar_module_state_curriculum_anchors(),
        module_state_curriculum_variants(),
    )
}

fn module_state_curriculum_variants() -> Vec<TassadarModuleStateCurriculumVariantReport> {
    vec![
        TassadarModuleStateCurriculumVariantReport::new(
            TassadarModuleStateCurriculumVariantId::FlatPrefixBaseline,
            "Current bounded flat-prefix trace-prediction baseline without explicit frame or memory-delta channels.",
            Vec::new(),
            Vec::new(),
            variant_family_evals(TassadarModuleStateCurriculumVariantId::FlatPrefixBaseline),
        ),
        TassadarModuleStateCurriculumVariantReport::new(
            TassadarModuleStateCurriculumVariantId::FrameStateOnly,
            "Add frame-local recurrent state and staged parsing curriculum while leaving memory-delta replay implicit.",
            vec![
                String::from("memory_copy_bootstrap"),
                String::from("frame_parse_alignment"),
            ],
            vec![
                TassadarModuleStateChannelKind::CallFrameState,
                TassadarModuleStateChannelKind::GlobalStateDelta,
                TassadarModuleStateChannelKind::ExportBoundaryState,
            ],
            variant_family_evals(TassadarModuleStateCurriculumVariantId::FrameStateOnly),
        ),
        TassadarModuleStateCurriculumVariantReport::new(
            TassadarModuleStateCurriculumVariantId::FrameStateWithMemoryDelta,
            "Keep frame-local state and add explicit memory-delta channels before the held-out dispatch stage.",
            vec![
                String::from("memory_copy_bootstrap"),
                String::from("frame_parse_alignment"),
            ],
            vec![
                TassadarModuleStateChannelKind::CallFrameState,
                TassadarModuleStateChannelKind::GlobalStateDelta,
                TassadarModuleStateChannelKind::MemoryDelta,
            ],
            variant_family_evals(
                TassadarModuleStateCurriculumVariantId::FrameStateWithMemoryDelta,
            ),
        ),
        TassadarModuleStateCurriculumVariantReport::new(
            TassadarModuleStateCurriculumVariantId::FullModuleCurriculum,
            "Freeze the full module curriculum with frame, global-delta, memory-delta, and export-boundary state before the held-out vm-style replay gate.",
            vec![
                String::from("memory_copy_bootstrap"),
                String::from("frame_parse_alignment"),
                String::from("dispatch_holdout_replay"),
            ],
            vec![
                TassadarModuleStateChannelKind::CallFrameState,
                TassadarModuleStateChannelKind::GlobalStateDelta,
                TassadarModuleStateChannelKind::MemoryDelta,
                TassadarModuleStateChannelKind::ExportBoundaryState,
            ],
            variant_family_evals(TassadarModuleStateCurriculumVariantId::FullModuleCurriculum),
        ),
    ]
}

fn variant_family_evals(
    variant_id: TassadarModuleStateCurriculumVariantId,
) -> Vec<TassadarModuleStateFamilyEval> {
    let rows = match variant_id {
        TassadarModuleStateCurriculumVariantId::FlatPrefixBaseline => vec![
            family_eval(TassadarModuleStateProgramFamily::Memcpy, false, 6900, 8700),
            family_eval(TassadarModuleStateProgramFamily::Checksum, false, 6700, 8500),
            family_eval(TassadarModuleStateProgramFamily::Parsing, true, 5200, 7600),
            family_eval(TassadarModuleStateProgramFamily::VmStyle, true, 4300, 7000),
        ],
        TassadarModuleStateCurriculumVariantId::FrameStateOnly => vec![
            family_eval(TassadarModuleStateProgramFamily::Memcpy, false, 7200, 8800),
            family_eval(TassadarModuleStateProgramFamily::Checksum, false, 7000, 8600),
            family_eval(TassadarModuleStateProgramFamily::Parsing, true, 6600, 8200),
            family_eval(TassadarModuleStateProgramFamily::VmStyle, true, 5900, 7900),
        ],
        TassadarModuleStateCurriculumVariantId::FrameStateWithMemoryDelta => vec![
            family_eval(TassadarModuleStateProgramFamily::Memcpy, false, 8200, 9300),
            family_eval(TassadarModuleStateProgramFamily::Checksum, false, 7900, 9100),
            family_eval(TassadarModuleStateProgramFamily::Parsing, true, 7000, 8500),
            family_eval(TassadarModuleStateProgramFamily::VmStyle, true, 6400, 8200),
        ],
        TassadarModuleStateCurriculumVariantId::FullModuleCurriculum => vec![
            family_eval(TassadarModuleStateProgramFamily::Memcpy, false, 8600, 9500),
            family_eval(TassadarModuleStateProgramFamily::Checksum, false, 8400, 9300),
            family_eval(TassadarModuleStateProgramFamily::Parsing, true, 7700, 8900),
            family_eval(TassadarModuleStateProgramFamily::VmStyle, true, 7100, 8600),
        ],
    };
    rows
}

fn family_eval(
    family: TassadarModuleStateProgramFamily,
    held_out: bool,
    later_window_exactness_bps: u32,
    final_state_exactness_bps: u32,
) -> TassadarModuleStateFamilyEval {
    TassadarModuleStateFamilyEval {
        family,
        held_out,
        later_window_exactness_bps,
        final_state_exactness_bps,
        trace_to_final_state_gap_bps: final_state_exactness_bps
            .saturating_sub(later_window_exactness_bps),
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
        TassadarModuleStateCurriculumVariantId, build_tassadar_module_state_curriculum_suite,
    };
    use psionic_models::TassadarModuleStateProgramFamily;

    #[test]
    fn module_state_curriculum_suite_is_machine_legible() {
        let suite = build_tassadar_module_state_curriculum_suite();
        assert_eq!(suite.stages.len(), 3);
        assert_eq!(suite.variants.len(), 4);
        assert!(!suite.suite_digest.is_empty());
    }

    #[test]
    fn full_module_curriculum_beats_flat_prefix_on_held_out_families() {
        let suite = build_tassadar_module_state_curriculum_suite();
        let baseline = suite
            .variants
            .iter()
            .find(|variant| {
                variant.variant_id == TassadarModuleStateCurriculumVariantId::FlatPrefixBaseline
            })
            .expect("baseline");
        let candidate = suite
            .variants
            .iter()
            .find(|variant| {
                variant.variant_id == TassadarModuleStateCurriculumVariantId::FullModuleCurriculum
            })
            .expect("candidate");

        for family in [
            TassadarModuleStateProgramFamily::Parsing,
            TassadarModuleStateProgramFamily::VmStyle,
        ] {
            let baseline_eval = baseline
                .family_evals
                .iter()
                .find(|eval| eval.family == family)
                .expect("baseline family");
            let candidate_eval = candidate
                .family_evals
                .iter()
                .find(|eval| eval.family == family)
                .expect("candidate family");

            assert!(candidate_eval.held_out);
            assert!(
                candidate_eval.later_window_exactness_bps
                    > baseline_eval.later_window_exactness_bps
            );
            assert!(
                candidate_eval.trace_to_final_state_gap_bps
                    < baseline_eval.trace_to_final_state_gap_bps
            );
        }
    }
}
