//! DSPy-powered verification pipeline for autopilot.
//!
//! Uses typed signatures to verify that a solution meets requirements:
//!
//! 1. **RequirementChecker** - Verify each requirement is addressed
//! 2. **TestAnalyzer** - Analyze test failures and suggest fixes
//! 3. **SolutionVerifier** - Final validation verdict
//!
//! # Example
//!
//! ```rust,ignore
//! use autopilot::dspy_verify::{VerificationPipeline, VerificationInput};
//!
//! let pipeline = VerificationPipeline::new();
//! let input = VerificationInput {
//!     requirements: vec!["Add a logout button".to_string()],
//!     solution_summary: "Added logout button to header".to_string(),
//!     test_output: "All tests passed".to_string(),
//!     build_output: "Build successful".to_string(),
//! };
//!
//! let result = pipeline.verify(&input).await?;
//! println!("Verdict: {:?}", result.verdict);
//! ```

use dspy_rs::{example, LM, Predict, Predictor, Signature};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// ============================================================================
// Signature Definitions
// ============================================================================

/// Requirement checker signature - verifies each requirement is addressed.
#[Signature]
struct RequirementCheckerSignature {
    /// Requirement Checker: Given a requirement and solution summary,
    /// determine if the requirement is satisfied.

    /// The specific requirement to verify
    #[input]
    requirement: String,

    /// Summary of the implemented solution
    #[input]
    solution_summary: String,

    /// Relevant code changes (diff or snippets)
    #[input]
    code_changes: String,

    /// SATISFIED, PARTIAL, or NOT_ADDRESSED
    #[output]
    status: String,

    /// Explanation of the verdict
    #[output]
    explanation: String,

    /// What's missing if PARTIAL or NOT_ADDRESSED
    #[output]
    missing: String,

    /// Confidence score (0.0-1.0)
    #[output]
    confidence: f32,
}

/// Test analyzer signature - analyzes test failures.
#[Signature(cot)]
struct TestAnalyzerSignature {
    /// Test Analyzer: Analyze test failures and determine root cause.
    /// Use chain-of-thought to reason through the failure.

    /// Test output including failures
    #[input]
    test_output: String,

    /// Code that was tested
    #[input]
    code_context: String,

    /// Root cause category: BUG, MISSING_IMPL, TEST_ISSUE, ENV_ISSUE
    #[output]
    root_cause: String,

    /// Detailed analysis of the failure
    #[output]
    analysis: String,

    /// Specific fix suggestions
    #[output]
    suggested_fixes: String,

    /// Files likely needing changes
    #[output]
    affected_files: String,
}

/// Build error analyzer signature - analyzes compilation errors.
#[Signature]
struct BuildAnalyzerSignature {
    /// Build Analyzer: Analyze compilation errors and suggest fixes.

    /// Compiler/build output with errors
    #[input]
    build_output: String,

    /// Error category: TYPE_ERROR, IMPORT_ERROR, SYNTAX_ERROR, LINK_ERROR
    #[output]
    error_category: String,

    /// Specific error messages extracted
    #[output]
    errors: String,

    /// Suggested fixes for each error
    #[output]
    fixes: String,

    /// Priority order for fixes
    #[output]
    fix_order: String,
}

/// Solution verifier signature - final validation verdict.
#[Signature]
struct SolutionVerifierSignature {
    /// Solution Verifier: Make final verdict on solution completeness.

    /// Original requirements
    #[input]
    requirements: String,

    /// Requirement check results (JSON array)
    #[input]
    requirement_results: String,

    /// Build status: SUCCESS or FAILED
    #[input]
    build_status: String,

    /// Test status: PASSED, FAILED, or SKIPPED
    #[input]
    test_status: String,

    /// Any blocking issues identified
    #[input]
    blocking_issues: String,

    /// Final verdict: PASS, FAIL, or RETRY
    #[output]
    verdict: String,

    /// Summary explanation
    #[output]
    explanation: String,

    /// Action for next iteration if RETRY
    #[output]
    next_action: String,

    /// Overall confidence (0.0-1.0)
    #[output]
    confidence: f32,
}

// ============================================================================
// Pipeline Types
// ============================================================================

/// Verdict from the verification pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerificationVerdict {
    /// Solution passes all checks
    Pass,
    /// Solution fails, needs more work
    Fail,
    /// Solution needs iteration, retry with suggested fixes
    Retry,
}

impl From<&str> for VerificationVerdict {
    fn from(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "PASS" | "PASSED" | "SUCCESS" => Self::Pass,
            "RETRY" | "ITERATE" | "CONTINUE" => Self::Retry,
            _ => Self::Fail,
        }
    }
}

/// Input to the verification pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationInput {
    /// List of requirements to verify
    pub requirements: Vec<String>,
    /// Summary of the implemented solution
    pub solution_summary: String,
    /// Relevant code changes (diff or key snippets)
    pub code_changes: String,
    /// Build output (stdout + stderr)
    pub build_output: String,
    /// Test output (stdout + stderr)
    pub test_output: String,
}

/// Result of requirement check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequirementResult {
    pub requirement: String,
    pub status: String,
    pub explanation: String,
    pub missing: String,
    pub confidence: f32,
}

/// Result from the verification pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    /// Final verdict
    pub verdict: VerificationVerdict,
    /// Explanation of verdict
    pub explanation: String,
    /// Suggested next action (if Retry)
    pub next_action: Option<String>,
    /// Individual requirement results
    pub requirement_results: Vec<RequirementResult>,
    /// Test analysis (if tests failed)
    pub test_analysis: Option<String>,
    /// Build analysis (if build failed)
    pub build_analysis: Option<String>,
    /// Overall confidence
    pub confidence: f32,
}

// ============================================================================
// Verification Pipeline
// ============================================================================

/// DSPy-powered verification pipeline.
pub struct VerificationPipeline {
    lm: Option<Arc<LM>>,
}

impl Default for VerificationPipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl VerificationPipeline {
    /// Create a new verification pipeline using the global LM.
    pub fn new() -> Self {
        Self { lm: None }
    }

    /// Create a pipeline with a specific LM.
    pub fn with_lm(lm: Arc<LM>) -> Self {
        Self { lm: Some(lm) }
    }

    /// Helper to get string from prediction value.
    fn get_string(prediction: &dspy_rs::Prediction, key: &str) -> String {
        let val = prediction.get(key, None);
        if let Some(s) = val.as_str() {
            s.to_string()
        } else {
            val.to_string().trim_matches('"').to_string()
        }
    }

    /// Helper to get f32 from prediction value.
    fn get_f32(prediction: &dspy_rs::Prediction, key: &str) -> f32 {
        let val = prediction.get(key, None);
        if let Some(n) = val.as_f64() {
            n as f32
        } else if let Some(s) = val.as_str() {
            s.parse().unwrap_or(0.0)
        } else {
            0.0
        }
    }

    /// Run the verification pipeline.
    pub async fn verify(&self, input: &VerificationInput) -> anyhow::Result<VerificationResult> {
        // Create predictors with signature instances
        let requirement_checker = Predict::new(RequirementCheckerSignature::new());
        let test_analyzer = Predict::new(TestAnalyzerSignature::new());
        let build_analyzer = Predict::new(BuildAnalyzerSignature::new());
        let solution_verifier = Predict::new(SolutionVerifierSignature::new());

        // 1. Check each requirement
        let mut requirement_results = Vec::new();
        for req in &input.requirements {
            let example = example! {
                "requirement": "input" => req.clone(),
                "solution_summary": "input" => input.solution_summary.clone(),
                "code_changes": "input" => input.code_changes.clone(),
            };

            let prediction = if let Some(lm) = &self.lm {
                requirement_checker.forward_with_config(example, lm.clone()).await?
            } else {
                requirement_checker.forward(example).await?
            };

            requirement_results.push(RequirementResult {
                requirement: req.clone(),
                status: Self::get_string(&prediction, "status"),
                explanation: Self::get_string(&prediction, "explanation"),
                missing: Self::get_string(&prediction, "missing"),
                confidence: Self::get_f32(&prediction, "confidence"),
            });
        }

        // 2. Analyze build if failed
        let build_status = if input.build_output.to_lowercase().contains("error")
            || input.build_output.to_lowercase().contains("failed")
        {
            "FAILED"
        } else {
            "SUCCESS"
        };

        let build_analysis = if build_status == "FAILED" {
            let example = example! {
                "build_output": "input" => input.build_output.clone(),
            };

            let prediction = if let Some(lm) = &self.lm {
                build_analyzer.forward_with_config(example, lm.clone()).await?
            } else {
                build_analyzer.forward(example).await?
            };

            Some(format!(
                "{}: {}",
                Self::get_string(&prediction, "error_category"),
                Self::get_string(&prediction, "fixes")
            ))
        } else {
            None
        };

        // 3. Analyze tests if failed
        let test_status = if input.test_output.to_lowercase().contains("failed")
            || input.test_output.to_lowercase().contains("error")
        {
            "FAILED"
        } else if input.test_output.is_empty() {
            "SKIPPED"
        } else {
            "PASSED"
        };

        let test_analysis = if test_status == "FAILED" {
            let example = example! {
                "test_output": "input" => input.test_output.clone(),
                "code_context": "input" => input.code_changes.clone(),
            };

            let prediction = if let Some(lm) = &self.lm {
                test_analyzer.forward_with_config(example, lm.clone()).await?
            } else {
                test_analyzer.forward(example).await?
            };

            Some(format!(
                "{}: {}",
                Self::get_string(&prediction, "root_cause"),
                Self::get_string(&prediction, "suggested_fixes")
            ))
        } else {
            None
        };

        // 4. Collect blocking issues
        let mut blocking_issues = Vec::new();
        if build_status == "FAILED" {
            blocking_issues.push("Build failed");
        }
        if test_status == "FAILED" {
            blocking_issues.push("Tests failed");
        }
        for result in &requirement_results {
            if result.status == "NOT_ADDRESSED" {
                blocking_issues.push("Requirement not addressed");
                break;
            }
        }

        // 5. Final verdict
        let requirement_results_json =
            serde_json::to_string(&requirement_results).unwrap_or_else(|_| "[]".to_string());

        let example = example! {
            "requirements": "input" => input.requirements.join("; "),
            "requirement_results": "input" => requirement_results_json,
            "build_status": "input" => build_status.to_string(),
            "test_status": "input" => test_status.to_string(),
            "blocking_issues": "input" => blocking_issues.join(", "),
        };

        let prediction = if let Some(lm) = &self.lm {
            solution_verifier.forward_with_config(example, lm.clone()).await?
        } else {
            solution_verifier.forward(example).await?
        };

        let verdict_str = Self::get_string(&prediction, "verdict");
        let verdict = VerificationVerdict::from(verdict_str.as_str());

        let next_action = if verdict == VerificationVerdict::Retry {
            let action = Self::get_string(&prediction, "next_action");
            if action.is_empty() {
                None
            } else {
                Some(action)
            }
        } else {
            None
        };

        Ok(VerificationResult {
            verdict,
            explanation: Self::get_string(&prediction, "explanation"),
            next_action,
            requirement_results,
            test_analysis,
            build_analysis,
            confidence: Self::get_f32(&prediction, "confidence"),
        })
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verdict_parsing() {
        assert_eq!(VerificationVerdict::from("PASS"), VerificationVerdict::Pass);
        assert_eq!(
            VerificationVerdict::from("PASSED"),
            VerificationVerdict::Pass
        );
        assert_eq!(
            VerificationVerdict::from("SUCCESS"),
            VerificationVerdict::Pass
        );
        assert_eq!(
            VerificationVerdict::from("RETRY"),
            VerificationVerdict::Retry
        );
        assert_eq!(
            VerificationVerdict::from("ITERATE"),
            VerificationVerdict::Retry
        );
        assert_eq!(VerificationVerdict::from("FAIL"), VerificationVerdict::Fail);
        assert_eq!(
            VerificationVerdict::from("FAILED"),
            VerificationVerdict::Fail
        );
        assert_eq!(
            VerificationVerdict::from("unknown"),
            VerificationVerdict::Fail
        );
    }

    #[test]
    fn test_verification_input_serialization() {
        let input = VerificationInput {
            requirements: vec!["Add logout button".to_string()],
            solution_summary: "Added button to header".to_string(),
            code_changes: "diff --git a/src/header.rs".to_string(),
            build_output: "Build successful".to_string(),
            test_output: "All tests passed".to_string(),
        };

        let json = serde_json::to_string(&input).unwrap();
        let parsed: VerificationInput = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.requirements, input.requirements);
    }

    #[test]
    fn test_requirement_result_serialization() {
        let result = RequirementResult {
            requirement: "Add logout button".to_string(),
            status: "SATISFIED".to_string(),
            explanation: "Button was added".to_string(),
            missing: "".to_string(),
            confidence: 0.95,
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("SATISFIED"));
    }
}
