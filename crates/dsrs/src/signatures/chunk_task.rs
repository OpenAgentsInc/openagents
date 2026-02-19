//! Chunk Task Selector Signature.
//!
//! Decides what analysis tasks to perform on a code chunk.

use crate::core::signature::MetaSignature;
use crate::data::example::Example;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Types of analysis tasks for code chunks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnalysisTask {
    /// Summarize the code's purpose.
    Summarize,

    /// Generate hypotheses about potential bugs.
    BugHypothesis,

    /// Extract symbol definitions and usages.
    ExtractSymbols,

    /// Identify dependencies and imports.
    AnalyzeDependencies,

    /// Check for security issues.
    SecurityAudit,

    /// Identify performance concerns.
    PerformanceAnalysis,

    /// Extract test coverage information.
    TestCoverage,
}

impl std::fmt::Display for AnalysisTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnalysisTask::Summarize => write!(f, "summarize"),
            AnalysisTask::BugHypothesis => write!(f, "bug_hypothesis"),
            AnalysisTask::ExtractSymbols => write!(f, "extract_symbols"),
            AnalysisTask::AnalyzeDependencies => write!(f, "analyze_dependencies"),
            AnalysisTask::SecurityAudit => write!(f, "security_audit"),
            AnalysisTask::PerformanceAnalysis => write!(f, "performance_analysis"),
            AnalysisTask::TestCoverage => write!(f, "test_coverage"),
        }
    }
}

/// A code chunk for analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeChunk {
    /// File path.
    pub path: String,

    /// Starting line (1-indexed).
    pub start_line: usize,

    /// Ending line (1-indexed).
    pub end_line: usize,

    /// The code content.
    pub content: String,

    /// Programming language (if known).
    pub language: Option<String>,
}

/// Signature for selecting analysis tasks for a code chunk.
///
/// # Inputs
/// - `chunk`: The code chunk to analyze
/// - `user_task`: The user's overall task
/// - `previous_findings`: Findings from previous analyses
///
/// # Outputs
/// - `tasks`: List of analysis tasks to perform
/// - `priority`: Priority order for tasks
/// - `rationale`: Explanation of task selection
#[derive(Debug, Clone)]
pub struct ChunkTaskSelectorSignature {
    instruction: String,
    demos: Vec<Example>,
}

impl Default for ChunkTaskSelectorSignature {
    fn default() -> Self {
        Self {
            instruction:
                r#"You are an expert at analyzing code. Given a code chunk and the user's task,
decide what analysis tasks would be most useful.

Available analysis tasks:
- summarize: Get a high-level summary of what the code does
- bug_hypothesis: Generate hypotheses about potential bugs
- extract_symbols: Extract function, class, and type definitions
- analyze_dependencies: Identify imports and dependencies
- security_audit: Check for security vulnerabilities
- performance_analysis: Identify performance concerns
- test_coverage: Analyze test coverage and gaps

Consider:
1. What the user is trying to accomplish
2. What type of code this is (library, application, test, config)
3. What information has already been gathered
4. Cost-effectiveness of each analysis

Output the most useful tasks in priority order."#
                    .to_string(),
            demos: vec![],
        }
    }
}

impl ChunkTaskSelectorSignature {
    /// Create a new chunk task selector signature.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set custom instruction.
    pub fn with_instruction(mut self, instruction: impl Into<String>) -> Self {
        self.instruction = instruction.into();
        self
    }

    /// Add a demonstration example.
    pub fn with_demo(mut self, demo: Example) -> Self {
        self.demos.push(demo);
        self
    }
}

impl MetaSignature for ChunkTaskSelectorSignature {
    fn demos(&self) -> Vec<Example> {
        self.demos.clone()
    }

    fn set_demos(&mut self, demos: Vec<Example>) -> Result<()> {
        self.demos = demos;
        Ok(())
    }

    fn instruction(&self) -> String {
        self.instruction.clone()
    }

    fn input_fields(&self) -> Value {
        json!({
            "chunk": {
                "type": "CodeChunk",
                "desc": "The code chunk to analyze",
                "__dsrs_field_type": "input"
            },
            "user_task": {
                "type": "String",
                "desc": "The user's overall task",
                "__dsrs_field_type": "input"
            },
            "previous_findings": {
                "type": "String",
                "desc": "Findings from previous analyses (optional)",
                "__dsrs_field_type": "input"
            }
        })
    }

    fn output_fields(&self) -> Value {
        json!({
            "tasks": {
                "type": "Vec<AnalysisTask>",
                "desc": "List of analysis tasks to perform",
                "__dsrs_field_type": "output"
            },
            "priority": {
                "type": "Vec<usize>",
                "desc": "Priority order for tasks (indices)",
                "__dsrs_field_type": "output"
            },
            "rationale": {
                "type": "String",
                "desc": "Explanation of task selection",
                "__dsrs_field_type": "output"
            }
        })
    }

    fn update_instruction(&mut self, instruction: String) -> Result<()> {
        self.instruction = instruction;
        Ok(())
    }

    fn append(&mut self, _name: &str, _value: Value) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_task_selector_signature() {
        let sig = ChunkTaskSelectorSignature::new();

        let inputs = sig.input_fields();
        assert!(inputs.get("chunk").is_some());
        assert!(inputs.get("user_task").is_some());
        assert!(inputs.get("previous_findings").is_some());

        let outputs = sig.output_fields();
        assert!(outputs.get("tasks").is_some());
        assert!(outputs.get("priority").is_some());
        assert!(outputs.get("rationale").is_some());
    }

    #[test]
    fn test_analysis_task_display() {
        assert_eq!(AnalysisTask::Summarize.to_string(), "summarize");
        assert_eq!(AnalysisTask::BugHypothesis.to_string(), "bug_hypothesis");
        assert_eq!(AnalysisTask::ExtractSymbols.to_string(), "extract_symbols");
    }

    #[test]
    fn test_code_chunk() {
        let chunk = CodeChunk {
            path: "src/main.rs".to_string(),
            start_line: 1,
            end_line: 10,
            content: "fn main() {}".to_string(),
            language: Some("rust".to_string()),
        };

        assert_eq!(chunk.path, "src/main.rs");
        assert_eq!(chunk.language, Some("rust".to_string()));
    }
}
