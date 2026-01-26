//! Plan mode signature catalog and helpers.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanModeSignatureKind {
    TopicDecomposition,
    ParallelExploration,
    PlanSynthesis,
    ComplexityClassification,
    DeepPlanning,
    ResultValidation,
}

impl PlanModeSignatureKind {
    pub const ALL: [PlanModeSignatureKind; 6] = [
        PlanModeSignatureKind::TopicDecomposition,
        PlanModeSignatureKind::ParallelExploration,
        PlanModeSignatureKind::PlanSynthesis,
        PlanModeSignatureKind::ComplexityClassification,
        PlanModeSignatureKind::DeepPlanning,
        PlanModeSignatureKind::ResultValidation,
    ];

    pub fn name(self) -> &'static str {
        match self {
            PlanModeSignatureKind::TopicDecomposition => "TopicDecompositionSignature",
            PlanModeSignatureKind::ParallelExploration => "ParallelExplorationSignature",
            PlanModeSignatureKind::PlanSynthesis => "PlanSynthesisSignature",
            PlanModeSignatureKind::ComplexityClassification => "ComplexityClassificationSignature",
            PlanModeSignatureKind::DeepPlanning => "DeepPlanningSignature",
            PlanModeSignatureKind::ResultValidation => "ResultValidationSignature",
        }
    }

    pub fn filename_stem(self) -> String {
        sanitize_filename(self.name())
    }
}

pub fn sanitize_filename(name: &str) -> String {
    name.replace("::", "_")
        .replace(':', "_")
        .replace('/', "_")
}
