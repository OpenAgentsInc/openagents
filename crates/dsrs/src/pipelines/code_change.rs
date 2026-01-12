//! Code Change Pipeline.
//!
//! A 5-signature chain for automated code changes:
//! 1. TaskUnderstanding - Parse user intent
//! 2. QueryComposer - Generate search queries
//! 3. RetrievalRouter - Route queries to lanes
//! 4. CodeEdit - Generate unified diff patches
//! 5. Verification - Verify changes are correct

use crate::data::example::Example;
use crate::signatures::{
    CodeEditSignature, QueryComposerSignature, TaskUnderstandingSignature, VerificationSignature,
};
use crate::{LM, Predict, Predictor, GLOBAL_SETTINGS};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;

// ============================================================================
// Types
// ============================================================================

/// Task type classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskType {
    Feature,
    Bugfix,
    Refactor,
    Docs,
    Test,
    Unknown,
}

impl From<&str> for TaskType {
    fn from(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "FEATURE" => Self::Feature,
            "BUGFIX" | "BUG" | "FIX" => Self::Bugfix,
            "REFACTOR" | "REFACTORING" => Self::Refactor,
            "DOCS" | "DOCUMENTATION" => Self::Docs,
            "TEST" | "TESTS" | "TESTING" => Self::Test,
            _ => Self::Unknown,
        }
    }
}

/// Scope estimate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Scope {
    Small,
    Medium,
    Large,
}

impl From<&str> for Scope {
    fn from(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "SMALL" | "S" => Self::Small,
            "LARGE" | "L" | "BIG" => Self::Large,
            _ => Self::Medium,
        }
    }
}

/// Verification status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerificationStatus {
    Pass,
    Partial,
    Fail,
}

impl From<&str> for VerificationStatus {
    fn from(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "PASS" | "SUCCESS" | "OK" => Self::Pass,
            "FAIL" | "FAILED" | "ERROR" => Self::Fail,
            _ => Self::Partial,
        }
    }
}

// ============================================================================
// Pipeline Stage Results
// ============================================================================

/// Result from task understanding stage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskUnderstandingResult {
    pub task_type: TaskType,
    pub requirements: Vec<String>,
    pub scope: Scope,
    pub clarifying_questions: Vec<String>,
    pub confidence: f32,
}

/// Result from code exploration stage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeExplorationResult {
    pub queries: Vec<String>,
    pub lanes: Vec<String>,
    pub relevant_files: Vec<String>,
    pub rationale: String,
}

/// A single code edit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeEdit {
    pub file_path: String,
    pub unified_diff: String,
    pub summary: String,
    pub affected_lines: String,
    pub confidence: f32,
}

/// Result from verification stage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    pub status: VerificationStatus,
    pub missing_requirements: Vec<String>,
    pub issues_found: Vec<String>,
    pub suggested_fixes: Vec<String>,
    pub confidence: f32,
}

/// Complete result from the code change pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeChangeResult {
    pub task: TaskUnderstandingResult,
    pub exploration: CodeExplorationResult,
    pub edits: Vec<CodeEdit>,
    pub verification: VerificationResult,
}

// ============================================================================
// Pipeline Input
// ============================================================================

/// Input to the code change pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeChangeInput {
    /// User's task description.
    pub user_task: String,
    /// Repository context (from manifest or README).
    pub repo_context: String,
    /// File tree or structure.
    pub repo_structure: String,
    /// Files to potentially edit (with contents).
    pub file_contents: HashMap<String, String>,
}

// ============================================================================
// Code Change Pipeline
// ============================================================================

/// Callback for pipeline stage updates.
pub trait CodeChangeCallback: Send + Sync {
    fn on_stage_start(&self, stage: &str);
    fn on_stage_output(&self, stage: &str, output: &str);
    fn on_stage_complete(&self, stage: &str);
}

/// DSPy-powered code change pipeline.
pub struct CodeChangePipeline {
    lm: Option<Arc<LM>>,
}

impl Default for CodeChangePipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl CodeChangePipeline {
    /// Create a new code change pipeline using the global LM.
    pub fn new() -> Self {
        Self { lm: None }
    }

    /// Create a pipeline with a specific LM.
    pub fn with_lm(lm: Arc<LM>) -> Self {
        Self { lm: Some(lm) }
    }

    /// Helper to get string from prediction value.
    fn get_string(prediction: &crate::Prediction, key: &str) -> String {
        let val = prediction.get(key, None);
        if let Some(s) = val.as_str() {
            s.to_string()
        } else {
            val.to_string().trim_matches('"').to_string()
        }
    }

    /// Helper to get vec of strings from prediction value.
    fn get_string_vec(prediction: &crate::Prediction, key: &str) -> Vec<String> {
        let val = prediction.get(key, None);
        if let Some(arr) = val.as_array() {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        } else if let Some(s) = val.as_str() {
            // Try to parse as JSON array
            serde_json::from_str(s).unwrap_or_else(|_| {
                // Fallback: split by newlines
                s.lines()
                    .map(|l| l.trim().trim_start_matches('-').trim().to_string())
                    .filter(|l| !l.is_empty())
                    .collect()
            })
        } else {
            vec![]
        }
    }

    /// Helper to get f32 from prediction value.
    fn get_f32(prediction: &crate::Prediction, key: &str) -> f32 {
        let val = prediction.get(key, None);
        if let Some(n) = val.as_f64() {
            n as f32
        } else if let Some(s) = val.as_str() {
            s.parse().unwrap_or(0.0)
        } else {
            0.0
        }
    }

    /// Run a predictor with the configured LM.
    async fn run_predictor<S: crate::core::signature::MetaSignature + 'static>(
        &self,
        signature: S,
        example: Example,
    ) -> Result<crate::Prediction> {
        let predictor = Predict::new(signature);
        if let Some(lm) = &self.lm {
            predictor.forward_with_config(example, lm.clone()).await
        } else {
            predictor.forward(example).await
        }
    }

    // ========================================================================
    // Stage 1: Task Understanding
    // ========================================================================

    /// Understand the user's task.
    pub async fn understand_task(
        &self,
        user_task: &str,
        repo_context: &str,
    ) -> Result<TaskUnderstandingResult> {
        let signature = TaskUnderstandingSignature::new();

        let mut data = HashMap::new();
        data.insert("user_request".to_string(), json!(user_task));
        data.insert("repo_context".to_string(), json!(repo_context));

        let example = Example {
            data,
            input_keys: vec!["user_request".to_string(), "repo_context".to_string()],
            output_keys: vec![],
            node_id: None,
        };

        let prediction = self.run_predictor(signature, example).await?;

        let task_type_str = Self::get_string(&prediction, "task_type");
        let requirements = Self::get_string_vec(&prediction, "requirements");
        let scope_str = Self::get_string(&prediction, "scope_estimate");
        let questions = Self::get_string_vec(&prediction, "clarifying_questions");

        Ok(TaskUnderstandingResult {
            task_type: TaskType::from(task_type_str.as_str()),
            requirements,
            scope: Scope::from(scope_str.as_str()),
            clarifying_questions: questions,
            confidence: Self::get_f32(&prediction, "confidence"),
        })
    }

    // ========================================================================
    // Stage 2: Code Exploration
    // ========================================================================

    /// Explore the codebase to find relevant files.
    pub async fn explore_code(
        &self,
        requirements: &[String],
        repo_structure: &str,
    ) -> Result<CodeExplorationResult> {
        // First, compose queries
        let query_sig = QueryComposerSignature::new();

        let mut data = HashMap::new();
        data.insert("goal".to_string(), json!(requirements.join("; ")));
        data.insert("failure_log".to_string(), json!(""));
        data.insert("previous_queries".to_string(), json!("[]"));

        let example = Example {
            data,
            input_keys: vec![
                "goal".to_string(),
                "failure_log".to_string(),
                "previous_queries".to_string(),
            ],
            output_keys: vec![],
            node_id: None,
        };

        let query_prediction = self.run_predictor(query_sig, example).await?;

        let queries = Self::get_string_vec(&query_prediction, "queries");
        let lanes = Self::get_string_vec(&query_prediction, "lanes");
        let rationale = Self::get_string(&query_prediction, "rationale");

        // For now, we'll extract relevant files from the repo structure
        // In a real implementation, we'd actually run the queries
        let relevant_files: Vec<String> = repo_structure
            .lines()
            .filter(|l| {
                let lower = l.to_lowercase();
                queries.iter().any(|q| lower.contains(&q.to_lowercase()))
            })
            .map(|s| s.trim().to_string())
            .take(10)
            .collect();

        Ok(CodeExplorationResult {
            queries,
            lanes,
            relevant_files,
            rationale,
        })
    }

    // ========================================================================
    // Stage 3 & 4: Planning & Code Edit (combined)
    // ========================================================================

    /// Generate code edits for a file.
    pub async fn generate_edit(
        &self,
        file_path: &str,
        current_content: &str,
        edit_instruction: &str,
        code_context: &str,
    ) -> Result<CodeEdit> {
        let signature = CodeEditSignature::new();

        let mut data = HashMap::new();
        data.insert("file_path".to_string(), json!(file_path));
        data.insert("current_content".to_string(), json!(current_content));
        data.insert("edit_instruction".to_string(), json!(edit_instruction));
        data.insert("code_context".to_string(), json!(code_context));

        let example = Example {
            data,
            input_keys: vec![
                "file_path".to_string(),
                "current_content".to_string(),
                "edit_instruction".to_string(),
                "code_context".to_string(),
            ],
            output_keys: vec![],
            node_id: None,
        };

        let prediction = self.run_predictor(signature, example).await?;

        Ok(CodeEdit {
            file_path: file_path.to_string(),
            unified_diff: Self::get_string(&prediction, "unified_diff"),
            summary: Self::get_string(&prediction, "edit_summary"),
            affected_lines: Self::get_string(&prediction, "affected_lines"),
            confidence: Self::get_f32(&prediction, "confidence"),
        })
    }

    // ========================================================================
    // Stage 5: Verification
    // ========================================================================

    /// Verify the changes are correct.
    pub async fn verify_changes(
        &self,
        original_request: &str,
        changes_made: &str,
        test_output: &str,
    ) -> Result<VerificationResult> {
        let signature = VerificationSignature::new();

        let mut data = HashMap::new();
        data.insert("original_request".to_string(), json!(original_request));
        data.insert("changes_made".to_string(), json!(changes_made));
        data.insert("test_output".to_string(), json!(test_output));

        let example = Example {
            data,
            input_keys: vec![
                "original_request".to_string(),
                "changes_made".to_string(),
                "test_output".to_string(),
            ],
            output_keys: vec![],
            node_id: None,
        };

        let prediction = self.run_predictor(signature, example).await?;

        let status_str = Self::get_string(&prediction, "verification_status");

        Ok(VerificationResult {
            status: VerificationStatus::from(status_str.as_str()),
            missing_requirements: Self::get_string_vec(&prediction, "missing_requirements"),
            issues_found: Self::get_string_vec(&prediction, "issues_found"),
            suggested_fixes: Self::get_string_vec(&prediction, "suggested_fixes"),
            confidence: Self::get_f32(&prediction, "confidence"),
        })
    }

    // ========================================================================
    // Full Pipeline
    // ========================================================================

    /// Run the complete code change pipeline.
    pub async fn run(&self, input: &CodeChangeInput) -> Result<CodeChangeResult> {
        // Check if we have an LM available
        if self.lm.is_none() && GLOBAL_SETTINGS.read().unwrap().is_none() {
            return Err(anyhow::anyhow!("No LM available for code change pipeline"));
        }

        // Stage 1: Understand task
        let task = self
            .understand_task(&input.user_task, &input.repo_context)
            .await?;

        // Stage 2: Explore code
        let exploration = self
            .explore_code(&task.requirements, &input.repo_structure)
            .await?;

        // Stage 3 & 4: Generate edits for each relevant file
        let mut edits = Vec::new();
        let requirements_str = task.requirements.join("\n- ");

        for file_path in &exploration.relevant_files {
            if let Some(content) = input.file_contents.get(file_path) {
                let edit = self
                    .generate_edit(file_path, content, &requirements_str, "")
                    .await?;
                edits.push(edit);
            }
        }

        // Stage 5: Verify changes
        let changes_summary = edits
            .iter()
            .map(|e| format!("{}: {}", e.file_path, e.summary))
            .collect::<Vec<_>>()
            .join("\n");

        let verification = self
            .verify_changes(&input.user_task, &changes_summary, "(no tests run)")
            .await?;

        Ok(CodeChangeResult {
            task,
            exploration,
            edits,
            verification,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_type_parsing() {
        assert_eq!(TaskType::from("FEATURE"), TaskType::Feature);
        assert_eq!(TaskType::from("bugfix"), TaskType::Bugfix);
        assert_eq!(TaskType::from("REFACTOR"), TaskType::Refactor);
        assert_eq!(TaskType::from("docs"), TaskType::Docs);
        assert_eq!(TaskType::from("TEST"), TaskType::Test);
        assert_eq!(TaskType::from("unknown"), TaskType::Unknown);
    }

    #[test]
    fn test_scope_parsing() {
        assert_eq!(Scope::from("SMALL"), Scope::Small);
        assert_eq!(Scope::from("medium"), Scope::Medium);
        assert_eq!(Scope::from("LARGE"), Scope::Large);
    }

    #[test]
    fn test_verification_status_parsing() {
        assert_eq!(VerificationStatus::from("PASS"), VerificationStatus::Pass);
        assert_eq!(VerificationStatus::from("partial"), VerificationStatus::Partial);
        assert_eq!(VerificationStatus::from("FAIL"), VerificationStatus::Fail);
    }
}
