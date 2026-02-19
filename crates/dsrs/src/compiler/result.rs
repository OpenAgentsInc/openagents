//! Compile result types for SwarmCompiler
//!
//! Contains the output of a successful compilation run.

use crate::compiler::{BudgetReport, ExecutionTrace};
use crate::evaluate::{PromotionResult, ScorecardResult};
use crate::manifest::CompiledModuleManifest;
use serde::{Deserialize, Serialize};

/// Result of a SwarmCompiler compilation run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompileResult {
    /// The compiled module manifest.
    pub manifest: CompiledModuleManifest,
    /// Evaluation scorecard from validation phase.
    pub scorecard: ScorecardResult,
    /// Result of promotion gate evaluation.
    pub promotion_result: PromotionResult,
    /// Budget usage report.
    pub budget_report: BudgetReport,
    /// Execution traces collected during compilation.
    pub traces: Vec<ExecutionTrace>,
}

impl CompileResult {
    /// Create a new compile result.
    pub fn new(
        manifest: CompiledModuleManifest,
        scorecard: ScorecardResult,
        promotion_result: PromotionResult,
        budget_report: BudgetReport,
        traces: Vec<ExecutionTrace>,
    ) -> Self {
        Self {
            manifest,
            scorecard,
            promotion_result,
            budget_report,
            traces,
        }
    }

    /// Check if the compilation was successful and module was promoted.
    pub fn is_promoted(&self) -> bool {
        self.promotion_result.success
    }

    /// Get the compiled module ID.
    pub fn compiled_id(&self) -> Option<&str> {
        self.manifest.compiled_id.as_deref()
    }

    /// Get total cost of compilation in millisatoshis.
    pub fn total_cost(&self) -> u64 {
        self.budget_report.spent
    }

    /// Get the overall score from validation.
    pub fn overall_score(&self) -> f64 {
        self.scorecard.overall_score
    }

    /// Get the promotion state after compilation.
    pub fn promotion_state(&self) -> Option<&crate::evaluate::PromotionState> {
        self.promotion_result.new_state.as_ref()
    }

    /// Get summary of the compilation result.
    pub fn summary(&self) -> CompileSummary {
        CompileSummary {
            compiled_id: self
                .manifest
                .compiled_id
                .clone()
                .unwrap_or_else(|| "unfinalized".to_string()),
            signature_name: self.manifest.signature_name.clone(),
            optimizer: self.manifest.optimizer.clone(),
            overall_score: self.scorecard.overall_score,
            total_cost_msats: self.budget_report.spent,
            promoted: self.promotion_result.success,
            new_state: self
                .promotion_result
                .new_state
                .as_ref()
                .map(|s| format!("{:?}", s))
                .unwrap_or_else(|| "None".to_string()),
            trace_count: self.traces.len(),
        }
    }
}

/// Summary of a compile result for quick inspection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompileSummary {
    /// Compiled module ID.
    pub compiled_id: String,
    /// Signature name.
    pub signature_name: String,
    /// Optimizer used.
    pub optimizer: String,
    /// Overall validation score.
    pub overall_score: f64,
    /// Total cost in millisatoshis.
    pub total_cost_msats: u64,
    /// Whether module was promoted.
    pub promoted: bool,
    /// New promotion state.
    pub new_state: String,
    /// Number of traces collected.
    pub trace_count: usize,
}

impl std::fmt::Display for CompileSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "CompileResult[{}]:\n  signature: {}\n  optimizer: {}\n  score: {:.2}\n  cost: {} msats\n  promoted: {}\n  state: {}\n  traces: {}",
            &self.compiled_id[..8.min(self.compiled_id.len())],
            self.signature_name,
            self.optimizer,
            self.overall_score,
            self.total_cost_msats,
            self.promoted,
            self.new_state,
            self.trace_count
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evaluate::PromotionState;
    use std::collections::HashMap;

    fn make_manifest() -> CompiledModuleManifest {
        CompiledModuleManifest::new("TestSignature", "MIPROv2")
    }

    fn make_scorecard() -> ScorecardResult {
        ScorecardResult {
            overall_score: 0.85,
            per_metric: HashMap::new(),
            per_task: HashMap::new(),
            total_cost_msats: 500,
            total_duration_ms: 100,
            tasks_evaluated: 10,
            tasks_skipped: 0,
        }
    }

    fn make_promotion_result(success: bool) -> PromotionResult {
        PromotionResult {
            success,
            gate_name: "test".to_string(),
            new_state: Some(if success {
                PromotionState::Staged
            } else {
                PromotionState::Candidate
            }),
            reason: "Test".to_string(),
            requirement_results: vec![],
        }
    }

    fn make_budget_report() -> BudgetReport {
        BudgetReport {
            total: 1000,
            spent: 600,
            by_phase: HashMap::new(),
        }
    }

    #[test]
    fn test_compile_result_creation() {
        let result = CompileResult::new(
            make_manifest(),
            make_scorecard(),
            make_promotion_result(true),
            make_budget_report(),
            vec![],
        );

        assert!(result.is_promoted());
        assert_eq!(result.overall_score(), 0.85);
        assert_eq!(result.total_cost(), 600);
    }

    #[test]
    fn test_compile_result_not_promoted() {
        let result = CompileResult::new(
            make_manifest(),
            make_scorecard(),
            make_promotion_result(false),
            make_budget_report(),
            vec![],
        );

        assert!(!result.is_promoted());
    }

    #[test]
    fn test_compile_summary() {
        let result = CompileResult::new(
            make_manifest(),
            make_scorecard(),
            make_promotion_result(true),
            make_budget_report(),
            vec![],
        );

        let summary = result.summary();
        assert_eq!(summary.signature_name, "TestSignature");
        assert_eq!(summary.optimizer, "MIPROv2");
        assert!(summary.promoted);
    }

    #[test]
    fn test_compile_summary_display() {
        let result = CompileResult::new(
            make_manifest(),
            make_scorecard(),
            make_promotion_result(true),
            make_budget_report(),
            vec![],
        );

        let display = format!("{}", result.summary());
        assert!(display.contains("TestSignature"));
        assert!(display.contains("MIPROv2"));
    }
}
