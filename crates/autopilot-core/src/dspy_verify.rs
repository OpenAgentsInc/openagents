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
//! use autopilot_core::dspy_verify::{VerificationPipeline, VerificationInput};
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

use dsrs::{LM, Predict, Predictor, Signature, example};
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

/// Build status classifier - classifies build output.
#[Signature]
struct BuildStatusClassifier {
    /// Build Status Classifier: Determine build status from output.

    /// Compiler/build output with errors
    #[input]
    build_output: String,

    /// Build command used
    #[input]
    command: String,

    /// Status: Success/Warning/Error/Fatal
    #[output]
    status: String,

    /// Error type category
    #[output]
    error_type: String,

    /// Whether the failure is actionable
    #[output]
    actionable: bool,
}

/// Test status classifier - classifies test output.
#[Signature]
struct TestStatusClassifier {
    /// Test Status Classifier: Determine test status from output.

    /// Test output including failures
    #[input]
    test_output: String,

    /// Test framework or command context
    #[input]
    test_framework: String,

    /// Status: Pass/Fail/Skip/Error
    #[output]
    status: String,

    /// Failure category
    #[output]
    failure_category: String,

    /// JSON array of failing tests
    #[output]
    failing_tests: String,
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

/// Execution review signature - verifies execution matched the plan.
#[Signature(cot)]
struct ExecutionReviewSignature {
    /// Execution Review: Verify that execution matched the plan and assess quality.
    /// Use chain-of-thought to carefully compare plan vs actual execution.

    /// The implementation plan that was being followed
    #[input]
    original_plan: String,

    /// JSON array of tool calls and their results during execution
    #[input]
    execution_trace: String,

    /// Git diff summary of files changed
    #[input]
    files_changed: String,

    /// Plan adherence: FULL, PARTIAL, or DEVIATED
    #[output]
    plan_adherence: String,

    /// Changes made that weren't part of the plan
    #[output]
    unexpected_changes: String,

    /// Plan steps that weren't executed
    #[output]
    missing_steps: String,

    /// Notes on code quality and best practices
    #[output]
    quality_assessment: String,

    /// Final verdict: APPROVE, REVISE, or REJECT
    #[output]
    verdict: String,

    /// Confidence in the review (0.0-1.0)
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

/// Plan adherence level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanAdherence {
    /// Execution followed plan completely
    Full,
    /// Some plan steps were skipped or modified
    Partial,
    /// Execution significantly deviated from plan
    Deviated,
}

impl From<&str> for PlanAdherence {
    fn from(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "FULL" | "COMPLETE" | "FOLLOWED" => Self::Full,
            "DEVIATED" | "DIVERGED" | "DIFFERENT" => Self::Deviated,
            _ => Self::Partial,
        }
    }
}

/// Review verdict for execution review.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReviewVerdict {
    /// Approve the execution as-is
    Approve,
    /// Revise some aspects of the execution
    Revise,
    /// Reject and start over
    Reject,
}

impl From<&str> for ReviewVerdict {
    fn from(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "APPROVE" | "APPROVED" | "ACCEPT" | "OK" => Self::Approve,
            "REJECT" | "REJECTED" | "FAIL" | "FAILED" => Self::Reject,
            _ => Self::Revise,
        }
    }
}

/// Input for execution review.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionReviewInput {
    /// The plan that was being executed
    pub original_plan: String,
    /// Trace of tool calls and results (JSON)
    pub execution_trace: String,
    /// Git diff of changes made
    pub files_changed: String,
}

/// Result from execution review.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionReviewResult {
    /// How well execution followed the plan
    pub plan_adherence: PlanAdherence,
    /// Unexpected changes (not in plan)
    pub unexpected_changes: Vec<String>,
    /// Plan steps that weren't executed
    pub missing_steps: Vec<String>,
    /// Code quality notes
    pub quality_assessment: String,
    /// Review verdict
    pub verdict: ReviewVerdict,
    /// Confidence in the review
    pub confidence: f32,
}

// ============================================================================
// Verification Pipeline
// ============================================================================

/// DSPy-powered verification pipeline.
#[derive(Clone)]
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
    fn get_string(prediction: &dsrs::Prediction, key: &str) -> String {
        let val = prediction.get(key, None);
        if let Some(s) = val.as_str() {
            s.to_string()
        } else {
            val.to_string().trim_matches('"').to_string()
        }
    }

    /// Helper to get f32 from prediction value.
    fn get_f32(prediction: &dsrs::Prediction, key: &str) -> f32 {
        let val = prediction.get(key, None);
        if let Some(n) = val.as_f64() {
            n as f32
        } else if let Some(s) = val.as_str() {
            s.parse().unwrap_or(0.0)
        } else {
            0.0
        }
    }

    /// Helper to get bool from prediction value.
    fn get_bool(prediction: &dsrs::Prediction, key: &str) -> bool {
        let val = prediction.get(key, None);
        if let Some(b) = val.as_bool() {
            b
        } else if let Some(s) = val.as_str() {
            matches!(s.to_lowercase().as_str(), "true" | "yes" | "1")
        } else {
            false
        }
    }

    /// Classify build status with DSPy, fallback to heuristics on failure.
    async fn classify_build_status(
        &self,
        build_output: &str,
        command: &str,
    ) -> (String, String, bool) {
        let classifier = Predict::new(BuildStatusClassifier::new());

        let example = example! {
            "build_output": "input" => build_output.to_string(),
            "command": "input" => command.to_string(),
        };

        let prediction = if let Some(lm) = &self.lm {
            classifier.forward_with_config(example, lm.clone()).await
        } else {
            classifier.forward(example).await
        };

        match prediction {
            Ok(pred) => (
                Self::get_string(&pred, "status"),
                Self::get_string(&pred, "error_type"),
                Self::get_bool(&pred, "actionable"),
            ),
            Err(_) => Self::heuristic_build_status(build_output),
        }
    }

    /// Heuristic fallback for build status.
    fn heuristic_build_status(build_output: &str) -> (String, String, bool) {
        let lower = build_output.to_lowercase();
        let failed = lower.contains("error") || lower.contains("failed");
        let error_type = if lower.contains("link") {
            "LinkError"
        } else if lower.contains("config") {
            "ConfigError"
        } else if failed {
            "CompileError"
        } else {
            "None"
        };

        (
            if failed { "Error" } else { "Success" }.to_string(),
            error_type.to_string(),
            failed,
        )
    }

    /// Classify test status with DSPy, fallback to heuristics on failure.
    async fn classify_test_status(
        &self,
        test_output: &str,
        test_framework: &str,
    ) -> (String, String, Vec<String>) {
        let classifier = Predict::new(TestStatusClassifier::new());

        let example = example! {
            "test_output": "input" => test_output.to_string(),
            "test_framework": "input" => test_framework.to_string(),
        };

        let prediction = if let Some(lm) = &self.lm {
            classifier.forward_with_config(example, lm.clone()).await
        } else {
            classifier.forward(example).await
        };

        match prediction {
            Ok(pred) => {
                let failing_tests =
                    Self::parse_json_array(&Self::get_string(&pred, "failing_tests"));
                (
                    Self::get_string(&pred, "status"),
                    Self::get_string(&pred, "failure_category"),
                    failing_tests,
                )
            }
            Err(_) => Self::heuristic_test_status(test_output),
        }
    }

    /// Heuristic fallback for test status.
    fn heuristic_test_status(test_output: &str) -> (String, String, Vec<String>) {
        let lower = test_output.to_lowercase();
        if test_output.trim().is_empty() {
            return ("Skip".to_string(), "NoOutput".to_string(), Vec::new());
        }

        let failed = lower.contains("failed") || lower.contains("error");
        (
            if failed { "Fail" } else { "Pass" }.to_string(),
            if failed { "Failure" } else { "None" }.to_string(),
            Vec::new(),
        )
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
                requirement_checker
                    .forward_with_config(example, lm.clone())
                    .await?
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

        // 2. Classify build status
        let (build_label, build_error_type, build_actionable) = self
            .classify_build_status(&input.build_output, "auto")
            .await;

        let build_status = if matches!(build_label.to_lowercase().as_str(), "error" | "fatal") {
            "FAILED"
        } else {
            "SUCCESS"
        };

        let build_analysis = if build_status == "FAILED" {
            let example = example! {
                "build_output": "input" => input.build_output.clone(),
            };

            let prediction = if let Some(lm) = &self.lm {
                build_analyzer
                    .forward_with_config(example, lm.clone())
                    .await?
            } else {
                build_analyzer.forward(example).await?
            };

            Some(format!(
                "{}: {} ({}; actionable: {})",
                Self::get_string(&prediction, "error_category"),
                Self::get_string(&prediction, "fixes"),
                build_error_type,
                build_actionable
            ))
        } else {
            None
        };

        // 3. Classify test status
        let (test_label, test_failure_category, failing_tests) =
            self.classify_test_status(&input.test_output, "auto").await;

        let test_status = match test_label.to_lowercase().as_str() {
            "pass" | "passed" | "success" => "PASSED",
            "skip" | "skipped" => "SKIPPED",
            "fail" | "failed" | "error" => "FAILED",
            _ => {
                if input.test_output.trim().is_empty() {
                    "SKIPPED"
                } else {
                    "FAILED"
                }
            }
        };

        let test_analysis = if test_status == "FAILED" {
            let example = example! {
                "test_output": "input" => input.test_output.clone(),
                "code_context": "input" => input.code_changes.clone(),
            };

            let prediction = if let Some(lm) = &self.lm {
                test_analyzer
                    .forward_with_config(example, lm.clone())
                    .await?
            } else {
                test_analyzer.forward(example).await?
            };

            let failing_summary = if failing_tests.is_empty() {
                "no test list".to_string()
            } else {
                failing_tests.join(", ")
            };

            Some(format!(
                "{}: {} ({}; failing: {})",
                Self::get_string(&prediction, "root_cause"),
                Self::get_string(&prediction, "suggested_fixes"),
                test_failure_category,
                failing_summary
            ))
        } else {
            None
        };

        // 4. Collect blocking issues
        let mut blocking_issues: Vec<String> = Vec::new();
        if build_status == "FAILED" {
            blocking_issues.push(format!("Build failed ({})", build_error_type));
        }
        if test_status == "FAILED" {
            blocking_issues.push(format!("Tests failed ({})", test_failure_category));
        }
        for result in &requirement_results {
            if result.status == "NOT_ADDRESSED" {
                blocking_issues.push("Requirement not addressed".to_string());
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
            solution_verifier
                .forward_with_config(example, lm.clone())
                .await?
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

    /// Parse JSON array string into Vec<String>.
    fn parse_json_array(s: &str) -> Vec<String> {
        serde_json::from_str::<Vec<String>>(s).unwrap_or_else(|_| {
            // Fallback: split by newlines if not valid JSON
            s.lines()
                .map(|l| l.trim().trim_start_matches('-').trim().to_string())
                .filter(|l| !l.is_empty())
                .collect()
        })
    }

    /// Review execution against the original plan.
    pub async fn review_execution(
        &self,
        input: &ExecutionReviewInput,
    ) -> anyhow::Result<ExecutionReviewResult> {
        let reviewer = Predict::new(ExecutionReviewSignature::new());

        let example = example! {
            "original_plan": "input" => input.original_plan.clone(),
            "execution_trace": "input" => input.execution_trace.clone(),
            "files_changed": "input" => input.files_changed.clone(),
        };

        let prediction = if let Some(lm) = &self.lm {
            reviewer.forward_with_config(example, lm.clone()).await?
        } else {
            reviewer.forward(example).await?
        };

        let adherence_str = Self::get_string(&prediction, "plan_adherence");
        let verdict_str = Self::get_string(&prediction, "verdict");

        let unexpected_changes =
            Self::parse_json_array(&Self::get_string(&prediction, "unexpected_changes"));
        let missing_steps = Self::parse_json_array(&Self::get_string(&prediction, "missing_steps"));

        Ok(ExecutionReviewResult {
            plan_adherence: PlanAdherence::from(adherence_str.as_str()),
            unexpected_changes,
            missing_steps,
            quality_assessment: Self::get_string(&prediction, "quality_assessment"),
            verdict: ReviewVerdict::from(verdict_str.as_str()),
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

    #[test]
    fn test_plan_adherence_parsing() {
        assert_eq!(PlanAdherence::from("FULL"), PlanAdherence::Full);
        assert_eq!(PlanAdherence::from("full"), PlanAdherence::Full);
        assert_eq!(PlanAdherence::from("COMPLETE"), PlanAdherence::Full);
        assert_eq!(PlanAdherence::from("PARTIAL"), PlanAdherence::Partial);
        assert_eq!(PlanAdherence::from("DEVIATED"), PlanAdherence::Deviated);
        assert_eq!(PlanAdherence::from("unknown"), PlanAdherence::Partial);
    }

    #[test]
    fn test_review_verdict_parsing() {
        assert_eq!(ReviewVerdict::from("APPROVE"), ReviewVerdict::Approve);
        assert_eq!(ReviewVerdict::from("approved"), ReviewVerdict::Approve);
        assert_eq!(ReviewVerdict::from("REVISE"), ReviewVerdict::Revise);
        assert_eq!(ReviewVerdict::from("REJECT"), ReviewVerdict::Reject);
        assert_eq!(ReviewVerdict::from("unknown"), ReviewVerdict::Revise);
    }

    #[test]
    fn test_execution_review_input_serialization() {
        let input = ExecutionReviewInput {
            original_plan: "1. Add button\n2. Test it".to_string(),
            execution_trace: "[]".to_string(),
            files_changed: "M src/ui.rs".to_string(),
        };

        let json = serde_json::to_string(&input).unwrap();
        let parsed: ExecutionReviewInput = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.original_plan, input.original_plan);
    }

    #[test]
    fn test_json_array_parsing() {
        // Valid JSON
        let valid = r#"["step1", "step2"]"#;
        let parsed = VerificationPipeline::parse_json_array(valid);
        assert_eq!(parsed, vec!["step1", "step2"]);

        // Fallback for non-JSON
        let lines = "- step1\n- step2";
        let parsed = VerificationPipeline::parse_json_array(lines);
        assert_eq!(parsed, vec!["step1", "step2"]);
    }
}
