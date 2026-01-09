//! DSPy-powered document analysis orchestrator.
//!
//! Uses DSRs (dspy-rs) modules with typed signatures to drive
//! the RLM analysis pipeline:
//!
//! 1. **Router** - Identify relevant document sections
//! 2. **Extractor** - Extract findings from each chunk (parallel)
//! 3. **Reducer** - Synthesize findings into final answer
//! 4. **Verifier** - Validate answer against evidence (optional)
//!
//! # Example
//!
//! ```rust,ignore
//! use rlm::dspy_orchestrator::{DspyOrchestrator, DspyOrchestratorConfig};
//! use rlm::dspy_bridge::configure_dspy_lm;
//!
//! // Configure global LM
//! configure_dspy_lm("openai:gpt-4o-mini", None, None).await?;
//!
//! // Create orchestrator
//! let orchestrator = DspyOrchestrator::new();
//! let result = orchestrator.analyze("What is the main topic?", &document).await?;
//!
//! println!("Answer: {}", result.answer);
//! println!("Confidence: {:.2}", result.confidence);
//! ```
//!
//! # Per-Request LM Configuration
//!
//! For production use with LmRouter integration, use `with_lm()`:
//!
//! ```rust,ignore
//! use rlm::dspy_bridge::LmRouterDspyBridge;
//! use lm_router::LmRouter;
//!
//! let router = Arc::new(LmRouter::builder().build());
//! let bridge = LmRouterDspyBridge::new(router, "gpt-4o-mini");
//! let lm = bridge.create_lm().await?;
//!
//! let orchestrator = DspyOrchestrator::with_lm(lm);
//! ```

use crate::chunking::{chunk_by_structure, detect_structure};
use crate::error::Result;
use crate::span::SpanRef;

use dsrs::{example, Example, LM, Predict, Predictor, Signature};
use std::sync::Arc;

// ============================================================================
// Signature Definitions (defined inline since the macro doesn't preserve `pub`)
// ============================================================================

/// Router signature - determines which document sections are relevant to a query.
#[Signature]
struct RouterSignature {
    /// Given a query and document preview, identify the most relevant sections to examine.

    #[input]
    pub query: String,

    #[input]
    pub document_preview: String,

    #[output]
    pub relevant_sections: String,

    #[output]
    pub confidence: f32,
}

/// Extractor signature with chain-of-thought - extracts information from document chunks.
#[Signature(cot)]
struct ExtractorSignature {
    /// Extract relevant information from this document chunk that helps answer the query.

    #[input]
    pub query: String,

    #[input]
    pub chunk: String,

    #[input]
    pub section: String,

    #[output]
    pub findings: String,

    #[output]
    pub evidence: String,

    #[output]
    pub relevance: f32,
}

/// Simple extractor without CoT - faster but less thorough.
#[Signature]
struct SimpleExtractorSignature {
    /// Extract relevant information from this chunk.

    #[input]
    pub query: String,

    #[input]
    pub chunk: String,

    #[output]
    pub findings: String,
}

/// Reducer signature - synthesizes multiple findings into a coherent answer.
#[Signature]
struct ReducerSignature {
    /// Synthesize the extracted findings into a comprehensive answer.

    #[input]
    pub query: String,

    #[input]
    pub findings: String,

    #[output]
    pub answer: String,

    #[output]
    pub citations: String,

    #[output]
    pub confidence: f32,
}

/// Verifier signature - validates answers against evidence.
#[Signature]
struct VerifierSignature {
    /// Verify that the answer is correct and supported by the evidence.

    #[input]
    pub query: String,

    #[input]
    pub answer: String,

    #[input]
    pub evidence: String,

    #[output]
    pub verdict: String,

    #[output]
    pub explanation: String,

    #[output]
    pub corrections: String,
}

// ============================================================================
// Configuration and Result Types
// ============================================================================

/// Configuration for DSPy-powered orchestrator.
#[derive(Debug, Clone)]
pub struct DspyOrchestratorConfig {
    /// Target chunk size in characters.
    pub chunk_size: usize,
    /// Overlap between chunks for context continuity.
    pub overlap: usize,
    /// Maximum chunks to process (cost control).
    pub max_chunks: usize,
    /// Whether to use semantic boundaries for chunking.
    pub semantic_chunking: bool,
    /// Use chain-of-thought extraction (slower but more thorough).
    pub use_cot_extraction: bool,
    /// Run verification pass on the final answer.
    pub verify_answer: bool,
    /// Maximum concurrent extractions for batching.
    pub max_concurrency: usize,
    /// Show progress during processing.
    pub verbose: bool,
}

impl Default for DspyOrchestratorConfig {
    fn default() -> Self {
        Self {
            chunk_size: 6000,
            overlap: 200,
            max_chunks: 50,
            semantic_chunking: true,
            use_cot_extraction: true,
            verify_answer: false,
            max_concurrency: 5,
            verbose: true,
        }
    }
}

/// Result from DSPy-powered analysis.
#[derive(Debug, Clone)]
pub struct DspyAnalysisResult {
    /// Final synthesized answer.
    pub answer: String,
    /// Key citations supporting the answer.
    pub citations: String,
    /// Confidence score 0-1 in the answer.
    pub confidence: f32,
    /// Number of chunks processed.
    pub chunks_processed: usize,
    /// Extraction results from each chunk.
    pub extractions: Vec<ChunkExtraction>,
    /// Verification result if enabled.
    pub verification: Option<VerificationResult>,
    /// Sections identified by router.
    pub relevant_sections: String,
}

/// Extraction result from a single chunk.
#[derive(Debug, Clone)]
pub struct ChunkExtraction {
    /// Chunk identifier.
    pub chunk_id: usize,
    /// Section title or identifier.
    pub section: String,
    /// Key findings extracted.
    pub findings: String,
    /// Supporting evidence/quotes.
    pub evidence: String,
    /// Relevance score 0-1.
    pub relevance: f32,
    /// SpanRef for provenance tracking (optional).
    pub span_ref: Option<SpanRef>,
}

/// Result from verification pass.
#[derive(Debug, Clone)]
pub struct VerificationResult {
    /// PASS or FAIL.
    pub verdict: String,
    /// Explanation of the verdict.
    pub explanation: String,
    /// Suggested corrections if FAIL.
    pub corrections: String,
}

// ============================================================================
// DSPy Orchestrator
// ============================================================================

/// DSPy-powered document analyzer.
///
/// Uses typed DSPy signatures and modules for each analysis phase:
/// - `RouterSignature` for identifying relevant sections
/// - `ExtractorSignature` (with CoT) for detailed extraction
/// - `ReducerSignature` for synthesizing findings
/// - `VerifierSignature` for answer validation
pub struct DspyOrchestrator {
    router: Predict,
    extractor: Predict,
    simple_extractor: Predict,
    reducer: Predict,
    verifier: Predict,
    config: DspyOrchestratorConfig,
    /// Optional per-request LM (if None, uses global configuration).
    lm: Option<Arc<LM>>,
    /// Path for SpanRef generation (for provenance tracking).
    document_path: Option<String>,
    /// Git commit for SpanRef generation.
    commit: Option<String>,
}

impl DspyOrchestrator {
    /// Create a new orchestrator with default configuration.
    ///
    /// Uses the globally configured LM (via `configure_dspy_lm()`).
    pub fn new() -> Self {
        Self::with_config(DspyOrchestratorConfig::default())
    }

    /// Create a new orchestrator with custom configuration.
    ///
    /// Uses the globally configured LM (via `configure_dspy_lm()`).
    pub fn with_config(config: DspyOrchestratorConfig) -> Self {
        Self {
            router: Predict::new(RouterSignature::new()),
            extractor: Predict::new(ExtractorSignature::new()),
            simple_extractor: Predict::new(SimpleExtractorSignature::new()),
            reducer: Predict::new(ReducerSignature::new()),
            verifier: Predict::new(VerifierSignature::new()),
            config,
            lm: None,
            document_path: None,
            commit: None,
        }
    }

    /// Create a new orchestrator with a specific LM instance.
    ///
    /// This enables per-request LM configuration for production use:
    /// - Unified cost tracking via LmRouter
    /// - Dynamic backend selection
    /// - Per-request model selection
    ///
    /// Note: The LM is stored but the current DSRs implementation
    /// uses the global LM. Configure the global LM before calling
    /// `analyze()` if using this constructor.
    pub fn with_lm(lm: Arc<LM>) -> Self {
        Self {
            router: Predict::new(RouterSignature::new()),
            extractor: Predict::new(ExtractorSignature::new()),
            simple_extractor: Predict::new(SimpleExtractorSignature::new()),
            reducer: Predict::new(ReducerSignature::new()),
            verifier: Predict::new(VerifierSignature::new()),
            config: DspyOrchestratorConfig::default(),
            lm: Some(lm),
            document_path: None,
            commit: None,
        }
    }

    /// Set document path for SpanRef generation.
    ///
    /// When set, extractions will include SpanRefs with this path.
    pub fn with_document_path(mut self, path: impl Into<String>) -> Self {
        self.document_path = Some(path.into());
        self
    }

    /// Set git commit for SpanRef generation.
    ///
    /// When set, SpanRefs will be pinned to this commit for reproducibility.
    pub fn with_commit(mut self, commit: impl Into<String>) -> Self {
        self.commit = Some(commit.into());
        self
    }

    /// Get the LM instance if one was provided.
    pub fn lm(&self) -> Option<&Arc<LM>> {
        self.lm.as_ref()
    }

    /// Main entry point for document analysis.
    ///
    /// Analyzes the document using typed DSPy modules:
    /// 1. Route to relevant sections
    /// 2. Extract from each chunk (parallel)
    /// 3. Reduce findings into answer
    /// 4. Optionally verify the answer
    pub async fn analyze(&self, query: &str, document: &str) -> Result<DspyAnalysisResult> {
        if self.config.verbose {
            println!("=== DSPy Orchestrator ===");
            println!("Document size: {} chars", document.len());
            println!("Query: {}", query);
            println!();
        }

        // Phase 1: Route to relevant sections
        if self.config.verbose {
            println!("Phase 1: Routing to relevant sections...");
        }
        let preview_len = 1000.min(document.len());
        let document_preview = &document[..preview_len];

        let routing = self
            .router
            .forward(example! {
                "query": "input" => query,
                "document_preview": "input" => document_preview
            })
            .await
            .map_err(|e: anyhow::Error| crate::error::RlmError::ExecutionError(e.to_string()))?;

        let relevant_sections = routing
            .get("relevant_sections", None)
            .as_str()
            .unwrap_or("")
            .to_string();
        let router_confidence: f32 = routing
            .get("confidence", None)
            .as_f64()
            .map(|f| f as f32)
            .unwrap_or(0.5);

        if self.config.verbose {
            println!("  Sections: {}", relevant_sections);
            println!("  Confidence: {:.2}", router_confidence);
            println!();
        }

        // Phase 2: Chunk and extract
        if self.config.verbose {
            println!("Phase 2: Chunking and extracting...");
        }
        let structure = detect_structure(document);
        let chunks = chunk_by_structure(
            document,
            &structure,
            self.config.chunk_size,
            self.config.overlap,
        );

        let chunks: Vec<_> = chunks
            .into_iter()
            .take(self.config.max_chunks)
            .collect();

        if self.config.verbose {
            println!("  Generated {} chunks", chunks.len());
        }

        // Build extraction examples
        let extraction_examples: Vec<Example> = chunks
            .iter()
            .map(|chunk| {
                let section = chunk
                    .section_context
                    .clone()
                    .unwrap_or_else(|| format!("Chunk {}", chunk.id));

                if self.config.use_cot_extraction {
                    example! {
                        "query": "input" => query,
                        "chunk": "input" => chunk.content.clone(),
                        "section": "input" => section
                    }
                } else {
                    example! {
                        "query": "input" => query,
                        "chunk": "input" => chunk.content.clone()
                    }
                }
            })
            .collect();

        // Extract in batch
        let extractor: &Predict = if self.config.use_cot_extraction {
            &self.extractor
        } else {
            &self.simple_extractor
        };

        let extraction_results = extractor
            .batch(extraction_examples)
            .await
            .map_err(|e: anyhow::Error| crate::error::RlmError::ExecutionError(e.to_string()))?;

        // Parse extraction results
        let mut extractions = Vec::new();
        for (i, (chunk, pred)) in chunks.iter().zip(extraction_results.iter()).enumerate() {
            let section = chunk
                .section_context
                .clone()
                .unwrap_or_else(|| format!("Chunk {}", chunk.id));

            let findings = pred
                .get("findings", None)
                .as_str()
                .unwrap_or("")
                .to_string();

            let evidence = if self.config.use_cot_extraction {
                pred.get("evidence", None)
                    .as_str()
                    .unwrap_or("")
                    .to_string()
            } else {
                String::new()
            };

            let relevance: f32 = if self.config.use_cot_extraction {
                pred.get("relevance", None)
                    .as_f64()
                    .map(|f| f as f32)
                    .unwrap_or(0.5)
            } else {
                0.5
            };

            // Generate SpanRef for provenance tracking if path is set
            let span_ref = self.document_path.as_ref().map(|path| {
                // Use byte positions from Chunk (line numbers would require original doc)
                SpanRef::from_chunk(
                    chunk.id,
                    path.clone(),
                    self.commit.as_deref(),
                    1, // Line number tracking requires original document
                    1, // Would need to count newlines to get accurate line numbers
                    chunk.start_pos as u64,
                    chunk.end_pos as u64,
                    &chunk.content,
                )
            });

            extractions.push(ChunkExtraction {
                chunk_id: i,
                section,
                findings,
                evidence,
                relevance,
                span_ref,
            });

            if self.config.verbose && i < 3 {
                let preview: String = extractions[i].findings.chars().take(60).collect();
                println!("  Chunk {}: {}...", i + 1, preview.replace('\n', " "));
            }
        }

        if self.config.verbose && extractions.len() > 3 {
            println!("  ... and {} more extractions", extractions.len() - 3);
            println!();
        }

        // Filter to relevant extractions
        let relevant_extractions: Vec<_> = extractions
            .iter()
            .filter(|e| e.relevance > 0.3 && !e.findings.is_empty())
            .collect();

        if relevant_extractions.is_empty() {
            return Ok(DspyAnalysisResult {
                answer: "No relevant information found in the document.".to_string(),
                citations: String::new(),
                confidence: 0.0,
                chunks_processed: chunks.len(),
                extractions,
                verification: None,
                relevant_sections,
            });
        }

        // Phase 3: Reduce findings
        if self.config.verbose {
            println!("Phase 3: Synthesizing {} findings...", relevant_extractions.len());
        }

        let all_findings = relevant_extractions
            .iter()
            .map(|e| format!("[{}] {}", e.section, e.findings))
            .collect::<Vec<_>>()
            .join("\n\n");

        let synthesis = self
            .reducer
            .forward(example! {
                "query": "input" => query,
                "findings": "input" => all_findings.clone()
            })
            .await
            .map_err(|e: anyhow::Error| crate::error::RlmError::ExecutionError(e.to_string()))?;

        let answer = synthesis
            .get("answer", None)
            .as_str()
            .unwrap_or("")
            .to_string();
        let citations = synthesis
            .get("citations", None)
            .as_str()
            .unwrap_or("")
            .to_string();
        let confidence: f32 = synthesis
            .get("confidence", None)
            .as_f64()
            .map(|f| f as f32)
            .unwrap_or(0.5);

        if self.config.verbose {
            println!("  Answer length: {} chars", answer.len());
            println!("  Confidence: {:.2}", confidence);
            println!();
        }

        // Phase 4: Verify (optional)
        let verification = if self.config.verify_answer {
            if self.config.verbose {
                println!("Phase 4: Verifying answer...");
            }

            let verification_result = self
                .verifier
                .forward(example! {
                    "query": "input" => query,
                    "answer": "input" => answer.clone(),
                    "evidence": "input" => all_findings
                })
                .await
                .map_err(|e: anyhow::Error| crate::error::RlmError::ExecutionError(e.to_string()))?;

            let verdict = verification_result
                .get("verdict", None)
                .as_str()
                .unwrap_or("UNKNOWN")
                .to_string();
            let explanation = verification_result
                .get("explanation", None)
                .as_str()
                .unwrap_or("")
                .to_string();
            let corrections = verification_result
                .get("corrections", None)
                .as_str()
                .unwrap_or("")
                .to_string();

            if self.config.verbose {
                println!("  Verdict: {}", verdict);
                if !explanation.is_empty() {
                    println!("  Explanation: {}", explanation);
                }
                println!();
            }

            Some(VerificationResult {
                verdict,
                explanation,
                corrections,
            })
        } else {
            None
        };

        Ok(DspyAnalysisResult {
            answer,
            citations,
            confidence,
            chunks_processed: chunks.len(),
            extractions,
            verification,
            relevant_sections,
        })
    }

    /// Quick analysis without routing or verification.
    ///
    /// Simpler pipeline: chunk -> extract -> reduce
    pub async fn analyze_quick(&self, query: &str, document: &str) -> Result<DspyAnalysisResult> {
        let mut config = self.config.clone();
        config.use_cot_extraction = false;
        config.verify_answer = false;

        let quick_orchestrator = DspyOrchestrator::with_config(config);
        quick_orchestrator.analyze(query, document).await
    }
}

impl Default for DspyOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let config = DspyOrchestratorConfig::default();
        assert_eq!(config.chunk_size, 6000);
        assert_eq!(config.overlap, 200);
        assert_eq!(config.max_chunks, 50);
        assert!(config.semantic_chunking);
        assert!(config.use_cot_extraction);
        assert!(!config.verify_answer);
        assert_eq!(config.max_concurrency, 5);
        assert!(config.verbose);
    }

    #[test]
    fn test_orchestrator_creation() {
        let _orchestrator = DspyOrchestrator::new();
        let _orchestrator_with_config =
            DspyOrchestrator::with_config(DspyOrchestratorConfig::default());
    }

    #[test]
    fn test_orchestrator_with_provenance() {
        let orchestrator = DspyOrchestrator::new()
            .with_document_path("docs/test.md")
            .with_commit("abc123");

        assert_eq!(orchestrator.document_path, Some("docs/test.md".to_string()));
        assert_eq!(orchestrator.commit, Some("abc123".to_string()));
    }

    #[test]
    fn test_chunk_extraction_with_span_ref() {
        let span = SpanRef::from_chunk(
            0,
            "test.md",
            Some("abc123"),
            1,
            10,
            0,
            100,
            "test content",
        );

        let extraction = ChunkExtraction {
            chunk_id: 0,
            section: "Test Section".to_string(),
            findings: "Found something".to_string(),
            evidence: "The evidence".to_string(),
            relevance: 0.9,
            span_ref: Some(span.clone()),
        };

        assert_eq!(extraction.span_ref.as_ref().unwrap().path, "test.md");
        assert!(extraction.span_ref.as_ref().unwrap().content_hash.is_some());
    }
}
