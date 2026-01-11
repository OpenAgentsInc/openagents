//! Code chunk analysis job type (`oa.code_chunk_analysis.v1`).
//!
//! This job type is used for analyzing code chunks during agent exploration.
//! It produces summaries, symbol information, suspected faults, and
//! recommendations for next probes.

use crate::provenance::Provenance;
use crate::verification::Verification;
use crate::version::SchemaVersion;
use serde::{Deserialize, Serialize};

use super::{JobRequest, JobResponse};

/// A code chunk to analyze.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CodeChunk {
    /// File path relative to repository root.
    pub path: String,

    /// Start line (1-indexed).
    pub start_line: u32,

    /// End line (1-indexed, inclusive).
    pub end_line: u32,

    /// The actual code content.
    pub content: String,

    /// Optional: language identifier (e.g., "rust", "python").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

impl CodeChunk {
    /// Create a new code chunk.
    pub fn new(path: impl Into<String>, start_line: u32, end_line: u32, content: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            start_line,
            end_line,
            content: content.into(),
            language: None,
        }
    }

    /// Set the language identifier.
    pub fn with_language(mut self, lang: impl Into<String>) -> Self {
        self.language = Some(lang.into());
        self
    }
}

/// Constraints on the analysis output.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct OutputConstraints {
    /// Maximum length for the summary.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_summary_length: Option<u32>,

    /// Maximum number of symbols to extract.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_symbols: Option<u32>,

    /// Maximum number of suspected faults.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_faults: Option<u32>,

    /// Maximum number of next probes to recommend.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_next_probes: Option<u32>,
}

/// Request for code chunk analysis.
///
/// # Example
///
/// ```
/// use protocol::jobs::{ChunkAnalysisRequest, chunk_analysis::CodeChunk};
///
/// let request = ChunkAnalysisRequest {
///     task: "Understand authentication flow".into(),
///     user_task: Some("Fix login bug".into()),
///     chunk: CodeChunk::new("src/auth.rs", 10, 50, "fn login() { ... }"),
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChunkAnalysisRequest {
    /// The analysis task/question.
    pub task: String,

    /// Optional: the user's original task for context.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_task: Option<String>,

    /// The code chunk to analyze.
    pub chunk: CodeChunk,

    /// Output constraints.
    #[serde(default)]
    pub output_constraints: OutputConstraints,

    /// Verification settings.
    #[serde(default = "default_verification")]
    pub verification: Verification,
}

fn default_verification() -> Verification {
    Verification::subjective_with_judge(2)
}

impl Default for ChunkAnalysisRequest {
    fn default() -> Self {
        Self {
            task: String::new(),
            user_task: None,
            chunk: CodeChunk::new("", 0, 0, ""),
            output_constraints: OutputConstraints::default(),
            verification: default_verification(),
        }
    }
}

impl JobRequest for ChunkAnalysisRequest {
    const JOB_TYPE: &'static str = "oa.code_chunk_analysis.v1";
    const SCHEMA_VERSION: SchemaVersion = SchemaVersion::new(1, 0, 0);

    fn verification(&self) -> &Verification {
        &self.verification
    }
}

/// A symbol extracted from the code chunk.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Symbol {
    /// Symbol name.
    pub name: String,

    /// Symbol kind (function, struct, trait, etc.).
    pub kind: String,

    /// Line number where the symbol is defined.
    pub line: u32,

    /// Brief description of the symbol.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// A suspected fault or issue in the code.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SuspectedFault {
    /// Line number or range.
    pub line: u32,

    /// Severity: low, medium, high.
    pub severity: String,

    /// Description of the suspected fault.
    pub description: String,

    /// Suggested fix or investigation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

/// A recommended next probe location.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NextProbe {
    /// File path to probe.
    pub path: String,

    /// Why this location should be probed.
    pub reason: String,

    /// Priority: low, medium, high.
    #[serde(default = "default_priority")]
    pub priority: String,
}

fn default_priority() -> String {
    "medium".to_string()
}

/// Response from code chunk analysis.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChunkAnalysisResponse {
    /// Summary of the code chunk.
    pub summary: String,

    /// Symbols extracted from the chunk.
    #[serde(default)]
    pub symbols: Vec<Symbol>,

    /// Suspected faults or issues.
    #[serde(default)]
    pub suspected_faults: Vec<SuspectedFault>,

    /// Recommended next probes.
    #[serde(default)]
    pub recommended_next_probes: Vec<NextProbe>,

    /// Confidence score (0.0 to 1.0).
    #[serde(default)]
    pub confidence: f32,

    /// Provenance information.
    pub provenance: Provenance,
}

impl JobResponse for ChunkAnalysisResponse {
    type Request = ChunkAnalysisRequest;

    fn provenance(&self) -> &Provenance {
        &self.provenance
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::JobEnvelope;

    #[test]
    fn test_chunk_analysis_request_hash() {
        let request = ChunkAnalysisRequest {
            task: "Analyze this code".into(),
            user_task: None,
            chunk: CodeChunk::new("src/lib.rs", 1, 10, "fn main() {}"),
            ..Default::default()
        };

        let hash1 = request.compute_hash().unwrap();
        let hash2 = request.compute_hash().unwrap();
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64);
    }

    #[test]
    fn test_job_type_constant() {
        assert_eq!(ChunkAnalysisRequest::JOB_TYPE, "oa.code_chunk_analysis.v1");
        assert_eq!(ChunkAnalysisRequest::SCHEMA_VERSION, SchemaVersion::new(1, 0, 0));
    }

    #[test]
    fn test_default_verification() {
        let request = ChunkAnalysisRequest::default();
        assert_eq!(request.verification.redundancy, 2);
    }

    #[test]
    fn test_request_serde() {
        let request = ChunkAnalysisRequest {
            task: "Understand this".into(),
            user_task: Some("Fix bug".into()),
            chunk: CodeChunk::new("src/auth.rs", 10, 50, "code here").with_language("rust"),
            output_constraints: OutputConstraints {
                max_symbols: Some(10),
                ..Default::default()
            },
            verification: Verification::subjective_with_judge(3),
        };

        let json = serde_json::to_string(&request).unwrap();
        let parsed: ChunkAnalysisRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(request, parsed);
    }

    #[test]
    fn test_response_serde() {
        let response = ChunkAnalysisResponse {
            summary: "This code handles authentication".into(),
            symbols: vec![Symbol {
                name: "login".into(),
                kind: "function".into(),
                line: 15,
                description: Some("Handles user login".into()),
            }],
            suspected_faults: vec![],
            recommended_next_probes: vec![NextProbe {
                path: "src/session.rs".into(),
                reason: "Session management related".into(),
                priority: "high".into(),
            }],
            confidence: 0.85,
            provenance: Provenance::new("codex-3-sonnet"),
        };

        let json = serde_json::to_string(&response).unwrap();
        let parsed: ChunkAnalysisResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(response, parsed);
    }

    #[test]
    fn test_envelope_integration() {
        let request = ChunkAnalysisRequest {
            task: "Analyze".into(),
            chunk: CodeChunk::new("test.rs", 1, 5, "fn test() {}"),
            ..Default::default()
        };

        let envelope = JobEnvelope::from_request(request);
        assert_eq!(envelope.job_type, "oa.code_chunk_analysis.v1");
        assert!(envelope.job_hash.is_some());
    }
}
