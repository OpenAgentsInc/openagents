//! DSPy signatures for provenance-first document analysis.
//!
//! These signatures use SpanRef for evidence tracking, enabling:
//! - Precise citation of sources
//! - Evidence chain verification
//! - Reproducible results
//!
//! All span references are JSON-encoded strings for DSPy compatibility.
//!
//! # Example
//!
//! ```rust,ignore
//! use rlm::signatures::*;
//! use rlm::span::SpanRef;
//!
//! // Create span for a chunk
//! let span = SpanRef::from_chunk(0, "doc.md", Some("abc123"), 1, 50, 0, 1000, content);
//!
//! // Pass to extractor as JSON
//! let span_json = span.to_json();
//! ```

use dsrs::Signature;

// ============================================================================
// Router Signature
// ============================================================================

/// Router signature - identifies relevant document sections.
///
/// Given a query and document preview, returns candidate spans to examine.
/// Output is a JSON array of partial SpanRefs (path + line ranges).
#[Signature]
pub struct RouterSignature {
    /// Router: Given a query and document preview, identify the most relevant
    /// sections to examine. Return candidate_spans as a JSON array of objects
    /// with path, start_line, end_line, and why (reason for selection).

    /// The user's question or information need
    #[input]
    pub query: String,

    /// First ~1000 chars of document for structure detection
    #[input]
    pub document_preview: String,

    /// JSON array of candidate spans: [{path, start_line, end_line, why}]
    #[output]
    pub candidate_spans: String,

    /// Confidence in routing decisions (0.0-1.0)
    #[output]
    pub confidence: f32,
}

// ============================================================================
// Extractor Signatures
// ============================================================================

/// Extractor signature with chain-of-thought and provenance.
///
/// Extracts findings from a chunk, returning evidence spans that support
/// each finding. Uses CoT for more thorough reasoning.
#[Signature(cot)]
pub struct ExtractorSignature {
    /// Extractor: Extract relevant information from this chunk that helps
    /// answer the query. Return findings with evidence_spans as a JSON array
    /// of SpanRef objects pointing to specific lines within the chunk.

    /// The user's question
    #[input]
    pub query: String,

    /// Content of this chunk
    #[input]
    pub chunk: String,

    /// JSON-encoded SpanRef for this chunk
    #[input]
    pub span_ref: String,

    /// Extracted findings as structured text
    #[output]
    pub findings: String,

    /// JSON array of SpanRefs within chunk supporting the findings
    #[output]
    pub evidence_spans: String,

    /// Relevance score (0.0-1.0)
    #[output]
    pub relevance: f32,
}

/// Simple extractor without CoT - faster but less thorough.
#[Signature]
pub struct SimpleExtractorSignature {
    /// Simple Extractor: Quickly extract relevant information from this chunk.

    /// The user's question
    #[input]
    pub query: String,

    /// Content of this chunk
    #[input]
    pub chunk: String,

    /// JSON-encoded SpanRef for this chunk
    #[input]
    pub span_ref: String,

    /// Extracted findings
    #[output]
    pub findings: String,

    /// Relevance score (0.0-1.0)
    #[output]
    pub relevance: f32,
}

// ============================================================================
// Reducer Signature
// ============================================================================

/// Reducer signature - synthesizes findings with citations.
///
/// Combines multiple findings into a coherent answer, with citations
/// as a JSON array of SpanRefs pointing to supporting evidence.
#[Signature]
pub struct ReducerSignature {
    /// Reducer: Synthesize the findings into a comprehensive answer.
    /// Include citations as a JSON array of SpanRefs for each key claim.

    /// The user's question
    #[input]
    pub query: String,

    /// Combined findings from all chunks (section-labeled)
    #[input]
    pub findings: String,

    /// JSON array of all evidence SpanRefs from extractions
    #[input]
    pub evidence_spans: String,

    /// Final synthesized answer
    #[output]
    pub answer: String,

    /// JSON array of SpanRefs cited in the answer
    #[output]
    pub citations: String,

    /// Confidence in answer (0.0-1.0)
    #[output]
    pub confidence: f32,
}

// ============================================================================
// Verifier Signature
// ============================================================================

/// Verifier signature - validates answers against evidence.
///
/// Checks that the answer is supported by the cited evidence,
/// returning missing spans if verification fails.
#[Signature]
pub struct VerifierSignature {
    /// Verifier: Verify the answer is correct and fully supported by citations.
    /// If evidence is missing, return missing_spans as JSON array of what's needed.

    /// The user's question
    #[input]
    pub query: String,

    /// The proposed answer to verify
    #[input]
    pub answer: String,

    /// JSON array of SpanRefs cited as evidence
    #[input]
    pub citations: String,

    /// PASS, FAIL, or PARTIAL
    #[output]
    pub verdict: String,

    /// Explanation of the verdict
    #[output]
    pub explanation: String,

    /// JSON array describing what evidence is missing (if FAIL/PARTIAL)
    #[output]
    pub missing_spans: String,

    /// Suggested corrections if answer is incorrect
    #[output]
    pub corrections: String,
}

// ============================================================================
// Helper Types for Parsing Signature Outputs
// ============================================================================

use serde::{Deserialize, Serialize};

/// Candidate span from router (partial SpanRef).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateSpan {
    /// File path
    pub path: String,
    /// Start line (1-indexed)
    pub start_line: u32,
    /// End line (1-indexed)
    pub end_line: u32,
    /// Reason for selection
    #[serde(default)]
    pub why: String,
}

impl CandidateSpan {
    /// Parse JSON array of candidate spans.
    pub fn parse_array(json: &str) -> Result<Vec<Self>, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Convert to JSON array.
    pub fn to_json_array(spans: &[Self]) -> String {
        serde_json::to_string(spans).unwrap_or_else(|_| "[]".to_string())
    }
}

/// Missing span request from verifier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissingSpanRequest {
    /// Description of what evidence is needed
    pub description: String,
    /// Suggested path if known
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_path: Option<String>,
    /// Claim that needs support
    pub claim: String,
}

impl MissingSpanRequest {
    /// Parse JSON array of missing span requests.
    pub fn parse_array(json: &str) -> Result<Vec<Self>, serde_json::Error> {
        serde_json::from_str(json)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_candidate_span_parsing() {
        let json = r#"[
            {"path": "src/main.rs", "start_line": 10, "end_line": 20, "why": "Contains main function"},
            {"path": "src/lib.rs", "start_line": 1, "end_line": 50}
        ]"#;

        let spans = CandidateSpan::parse_array(json).unwrap();
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].path, "src/main.rs");
        assert_eq!(spans[0].why, "Contains main function");
        assert_eq!(spans[1].why, ""); // Default empty
    }

    #[test]
    fn test_missing_span_request_parsing() {
        let json = r#"[
            {"description": "Need implementation details", "suggested_path": "src/impl.rs", "claim": "Uses SHA256"},
            {"description": "Need test coverage", "claim": "All edge cases handled"}
        ]"#;

        let requests = MissingSpanRequest::parse_array(json).unwrap();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].suggested_path, Some("src/impl.rs".to_string()));
        assert_eq!(requests[1].suggested_path, None);
    }
}
