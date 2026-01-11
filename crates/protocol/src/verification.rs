//! Verification modes for job results.
//!
//! Jobs can be verified in two modes:
//! - **Objective**: Deterministic verification (tests, linting, build status)
//! - **Subjective**: Requires judgment (summaries, analysis, hypotheses)
//!
//! Subjective jobs may use redundancy (multiple providers) and adjudication
//! strategies to ensure quality.

use serde::{Deserialize, Serialize};

/// How a job result should be verified.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum VerificationMode {
    /// Result can be verified deterministically.
    ///
    /// Examples: test pass/fail, linting results, build success, exact match.
    /// These jobs typically need redundancy=1.
    Objective,

    /// Result requires judgment to verify.
    ///
    /// Examples: code summaries, chunk analysis, reranking, hypotheses.
    /// These jobs benefit from redundancy > 1 and adjudication.
    #[default]
    Subjective,
}

/// Strategy for adjudicating between multiple provider results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AdjudicationStrategy {
    /// No adjudication - use first result (for objective or single-provider jobs).
    #[default]
    None,

    /// Use majority vote among providers.
    ///
    /// Best for categorical outputs (e.g., classification, yes/no decisions).
    MajorityVote,

    /// Use a judge model to evaluate and select the best result.
    ///
    /// Best for complex outputs where quality is subjective.
    JudgeModel,

    /// Combine results by merging/aggregating.
    ///
    /// Best for cumulative outputs (e.g., symbol lists, candidate rankings).
    Merge,
}

/// Verification configuration for a job.
///
/// # Example
///
/// ```
/// use protocol::verification::{Verification, VerificationMode, AdjudicationStrategy};
///
/// // Objective job (e.g., sandbox run)
/// let objective = Verification::objective();
/// assert_eq!(objective.mode, VerificationMode::Objective);
/// assert_eq!(objective.redundancy, 1);
///
/// // Subjective job with judge model
/// let subjective = Verification::subjective_with_judge(2);
/// assert_eq!(subjective.mode, VerificationMode::Subjective);
/// assert_eq!(subjective.redundancy, 2);
/// assert_eq!(subjective.adjudication, AdjudicationStrategy::JudgeModel);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Verification {
    /// Whether the job result is objective or subjective.
    pub mode: VerificationMode,

    /// Number of providers to run the job (for redundancy).
    ///
    /// - For objective jobs: typically 1
    /// - For subjective jobs: typically 2-3
    #[serde(default = "default_redundancy")]
    pub redundancy: u8,

    /// Strategy for combining/selecting among multiple results.
    #[serde(default)]
    pub adjudication: AdjudicationStrategy,

    /// Optional: specific judge model for JudgeModel adjudication.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub judge_model: Option<String>,
}

fn default_redundancy() -> u8 {
    1
}

impl Verification {
    /// Create an objective verification config (single provider, no adjudication).
    pub fn objective() -> Self {
        Self {
            mode: VerificationMode::Objective,
            redundancy: 1,
            adjudication: AdjudicationStrategy::None,
            judge_model: None,
        }
    }

    /// Create a subjective verification config with majority vote.
    pub fn subjective_with_majority(redundancy: u8) -> Self {
        Self {
            mode: VerificationMode::Subjective,
            redundancy,
            adjudication: AdjudicationStrategy::MajorityVote,
            judge_model: None,
        }
    }

    /// Create a subjective verification config with judge model.
    pub fn subjective_with_judge(redundancy: u8) -> Self {
        Self {
            mode: VerificationMode::Subjective,
            redundancy,
            adjudication: AdjudicationStrategy::JudgeModel,
            judge_model: None,
        }
    }

    /// Create a subjective verification config with merge strategy.
    pub fn subjective_with_merge(redundancy: u8) -> Self {
        Self {
            mode: VerificationMode::Subjective,
            redundancy,
            adjudication: AdjudicationStrategy::Merge,
            judge_model: None,
        }
    }

    /// Set a specific judge model for adjudication.
    pub fn with_judge_model(mut self, model: impl Into<String>) -> Self {
        self.judge_model = Some(model.into());
        self
    }
}

impl Default for Verification {
    fn default() -> Self {
        Self::objective()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_objective_verification() {
        let v = Verification::objective();
        assert_eq!(v.mode, VerificationMode::Objective);
        assert_eq!(v.redundancy, 1);
        assert_eq!(v.adjudication, AdjudicationStrategy::None);
    }

    #[test]
    fn test_subjective_with_majority() {
        let v = Verification::subjective_with_majority(3);
        assert_eq!(v.mode, VerificationMode::Subjective);
        assert_eq!(v.redundancy, 3);
        assert_eq!(v.adjudication, AdjudicationStrategy::MajorityVote);
    }

    #[test]
    fn test_subjective_with_judge() {
        let v = Verification::subjective_with_judge(2).with_judge_model("gpt-4");
        assert_eq!(v.mode, VerificationMode::Subjective);
        assert_eq!(v.redundancy, 2);
        assert_eq!(v.adjudication, AdjudicationStrategy::JudgeModel);
        assert_eq!(v.judge_model, Some("gpt-4".to_string()));
    }

    #[test]
    fn test_serde_roundtrip() {
        let v = Verification::subjective_with_judge(2).with_judge_model("codex-3");
        let json = serde_json::to_string(&v).unwrap();
        let parsed: Verification = serde_json::from_str(&json).unwrap();
        assert_eq!(v, parsed);
    }

    #[test]
    fn test_serde_snake_case() {
        let v = Verification::subjective_with_majority(2);
        let json = serde_json::to_string(&v).unwrap();
        assert!(json.contains("\"mode\":\"subjective\""));
        assert!(json.contains("\"adjudication\":\"majority_vote\""));
    }

    #[test]
    fn test_default_is_objective() {
        let v = Verification::default();
        assert_eq!(v.mode, VerificationMode::Objective);
    }
}
