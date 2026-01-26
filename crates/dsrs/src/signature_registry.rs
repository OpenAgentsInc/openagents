use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::core::signature::MetaSignature;
use crate::signatures::{
    AgentMemorySignature, CandidateRerankSignature, ChunkAnalysisToActionSignature,
    ChunkTaskSelectorSignature, CodeEditSignature, ComplexityClassificationSignature,
    ContextSelectionSignature, DeepPlanningSignature, FailureTriageSignature,
    FullAutoDecisionSignature, IssueSuggestionSignature, IssueValidationSignature,
    LaneBudgeterSignature, ParallelExplorationSignature, PlanSynthesisSignature,
    PlanningSignature, QueryComposerSignature, RetrievalRouterSignature,
    ResultValidationSignature, SandboxProfileSelectionSignature,
    TaskUnderstandingSignature, ToolCallSignature, ToolResultSignature,
    TopicDecompositionSignature, ToolSelectionSignature,
    UiComposerSignature,
    UnblockSuggestionSignature, VerificationSignature,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DsrsSignatureInfo {
    pub name: String,
    pub instruction: String,
    pub input_fields: Value,
    pub output_fields: Value,
}

pub fn signature_info<S: MetaSignature>(signature: S) -> DsrsSignatureInfo {
    DsrsSignatureInfo {
        name: signature.signature_name().to_string(),
        instruction: signature.instruction(),
        input_fields: signature.input_fields(),
        output_fields: signature.output_fields(),
    }
}

pub fn list_signatures() -> Vec<DsrsSignatureInfo> {
    vec![
        signature_info(AgentMemorySignature::new()),
        signature_info(CandidateRerankSignature::new()),
        signature_info(ChunkAnalysisToActionSignature::new()),
        signature_info(ChunkTaskSelectorSignature::new()),
        signature_info(CodeEditSignature::new()),
        signature_info(ContextSelectionSignature::new()),
        signature_info(FailureTriageSignature::new()),
        signature_info(FullAutoDecisionSignature::new()),
        signature_info(IssueSuggestionSignature::new()),
        signature_info(IssueValidationSignature::new()),
        signature_info(LaneBudgeterSignature::new()),
        signature_info(PlanningSignature::new()),
        signature_info(QueryComposerSignature::new()),
        signature_info(RetrievalRouterSignature::new()),
        signature_info(SandboxProfileSelectionSignature::new()),
        signature_info(TaskUnderstandingSignature::new()),
        signature_info(ToolCallSignature::new()),
        signature_info(ToolResultSignature::new()),
        signature_info(UnblockSuggestionSignature::new()),
        signature_info(VerificationSignature::new()),
        signature_info(TopicDecompositionSignature::new()),
        signature_info(ParallelExplorationSignature::new()),
        signature_info(PlanSynthesisSignature::new()),
        signature_info(ToolSelectionSignature::new()),
        signature_info(ComplexityClassificationSignature::new()),
        signature_info(DeepPlanningSignature::new()),
        signature_info(ResultValidationSignature::new()),
        signature_info(UiComposerSignature::new()),
    ]
}

pub fn get_signature(name: &str) -> Option<DsrsSignatureInfo> {
    list_signatures()
        .into_iter()
        .find(|signature| signature.name == name)
}
