//! Engine-orchestrated document analysis.
//!
//! Drives multi-phase analysis pipeline for large documents:
//! 1. Structure Discovery - detect document type and sections
//! 2. Chunk Generation - split on semantic boundaries
//! 3. Targeted Extraction - analyze each chunk with FM
//! 4. Synthesis - combine findings into final answer

use std::time::Instant;

use fm_bridge::FMClient;

use crate::chunking::{chunk_by_structure, detect_structure, Chunk};
use crate::error::{Result, RlmError};

/// Configuration for engine-orchestrated analysis.
#[derive(Debug, Clone)]
pub struct OrchestratorConfig {
    /// Target chunk size in characters.
    pub chunk_size: usize,
    /// Overlap between chunks for context continuity.
    pub overlap: usize,
    /// Maximum chunks to process (cost control).
    pub max_chunks: usize,
    /// Whether to use semantic boundaries.
    pub semantic_chunking: bool,
    /// Show progress during processing.
    pub verbose: bool,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            chunk_size: 6000,     // Apple FM sweet spot
            overlap: 200,         // Context continuity
            max_chunks: 50,       // Cost control
            semantic_chunking: true,
            verbose: true,
        }
    }
}

/// Result of orchestrated analysis.
#[derive(Debug, Clone)]
pub struct AnalysisResult {
    /// Final synthesized answer.
    pub answer: String,
    /// Number of chunks processed.
    pub chunks_processed: usize,
    /// Summaries from each chunk.
    pub chunk_summaries: Vec<ChunkSummary>,
    /// Total processing time in milliseconds.
    pub processing_time_ms: u64,
}

/// Summary extracted from a single chunk.
#[derive(Debug, Clone)]
pub struct ChunkSummary {
    /// Chunk identifier.
    pub chunk_id: usize,
    /// Start position in original document.
    pub start_pos: usize,
    /// End position in original document.
    pub end_pos: usize,
    /// Section title if available.
    pub section_title: Option<String>,
    /// Extracted findings relevant to query.
    pub findings: String,
}

/// Engine-driven document analyzer.
///
/// Orchestrates multi-phase analysis that works regardless of
/// where relevant content appears in the document.
pub struct EngineOrchestrator {
    client: FMClient,
    config: OrchestratorConfig,
}

impl EngineOrchestrator {
    /// Create a new orchestrator with default config.
    pub fn new(client: FMClient) -> Self {
        Self {
            client,
            config: OrchestratorConfig::default(),
        }
    }

    /// Create a new orchestrator with custom config.
    pub fn with_config(client: FMClient, config: OrchestratorConfig) -> Self {
        Self { client, config }
    }

    /// Main entry point for document analysis.
    ///
    /// Analyzes the full document systematically, extracting relevant
    /// information from each section and synthesizing into a final answer.
    pub async fn analyze(&self, context: &str, query: &str) -> Result<AnalysisResult> {
        let start_time = Instant::now();

        if self.config.verbose {
            println!("=== Engine Orchestrator ===");
            println!("Document size: {} chars", context.len());
            println!("Query: {}", query);
            println!();
        }

        // Phase 1: Structure Discovery
        if self.config.verbose {
            println!("Phase 1: Discovering document structure...");
        }
        let structure = detect_structure(context);
        if self.config.verbose {
            println!(
                "  Type: {:?}, Sections: {}",
                structure.doc_type,
                structure.sections.len()
            );
        }

        // Phase 2: Chunk Generation
        if self.config.verbose {
            println!("Phase 2: Generating semantic chunks...");
        }
        let chunks = chunk_by_structure(
            context,
            &structure,
            self.config.chunk_size,
            self.config.overlap,
        );

        // Limit chunks if needed
        let chunks: Vec<_> = chunks
            .into_iter()
            .take(self.config.max_chunks)
            .collect();

        if self.config.verbose {
            println!("  Generated {} chunks", chunks.len());
            for chunk in &chunks {
                let preview: String = chunk.content.chars().take(50).collect();
                println!(
                    "  Chunk {}: {}...{} ({} chars) - {}...",
                    chunk.id,
                    chunk.start_pos,
                    chunk.end_pos,
                    chunk.content.len(),
                    preview.replace('\n', " ")
                );
            }
            println!();
        }

        // Phase 3: Targeted Extraction
        if self.config.verbose {
            println!("Phase 3: Extracting from each chunk...");
        }
        let summaries = self.extract_from_chunks(&chunks, query).await?;

        // Count chunks with actual findings
        let chunks_with_findings = summaries
            .iter()
            .filter(|s| !s.findings.contains("No relevant content"))
            .count();

        if self.config.verbose {
            println!(
                "  Extracted findings from {} of {} chunks",
                chunks_with_findings,
                summaries.len()
            );
            println!();
        }

        // Phase 4: Synthesis
        if self.config.verbose {
            println!("Phase 4: Synthesizing final answer...");
        }
        let answer = self.synthesize(&summaries, query).await?;

        let elapsed = start_time.elapsed();

        Ok(AnalysisResult {
            answer,
            chunks_processed: summaries.len(),
            chunk_summaries: summaries,
            processing_time_ms: elapsed.as_millis() as u64,
        })
    }

    /// Phase 3: Process each chunk with FM.
    async fn extract_from_chunks(
        &self,
        chunks: &[Chunk],
        query: &str,
    ) -> Result<Vec<ChunkSummary>> {
        let mut summaries = Vec::new();
        let total_chunks = chunks.len();

        for chunk in chunks {
            if self.config.verbose {
                print!("  Processing chunk {}/{}...", chunk.id + 1, total_chunks);
            }

            let prompt = self.build_extraction_prompt(chunk, query, total_chunks);

            match self.client.complete(&prompt, None).await {
                Ok(response) => {
                    let findings = response
                        .choices
                        .first()
                        .map(|c| c.message.content.clone())
                        .unwrap_or_else(|| "No response".to_string());

                    if self.config.verbose {
                        let preview: String = findings.chars().take(60).collect();
                        println!(" {}", preview.replace('\n', " "));
                    }

                    summaries.push(ChunkSummary {
                        chunk_id: chunk.id,
                        start_pos: chunk.start_pos,
                        end_pos: chunk.end_pos,
                        section_title: chunk.section_context.clone(),
                        findings,
                    });
                }
                Err(e) => {
                    if self.config.verbose {
                        println!(" ERROR: {}", e);
                    }
                    // Continue with other chunks instead of failing entirely
                    summaries.push(ChunkSummary {
                        chunk_id: chunk.id,
                        start_pos: chunk.start_pos,
                        end_pos: chunk.end_pos,
                        section_title: chunk.section_context.clone(),
                        findings: format!("[Error extracting: {}]", e),
                    });
                }
            }
        }

        Ok(summaries)
    }

    /// Build extraction prompt for a single chunk.
    fn build_extraction_prompt(&self, chunk: &Chunk, query: &str, total_chunks: usize) -> String {
        let section_info = chunk
            .section_context
            .as_ref()
            .map(|s| format!("\nSection: {}\n", s))
            .unwrap_or_default();

        format!(
            r#"Analyze this section of a document for the following query:

QUERY: {}

SECTION ({} of {}):{}
---
{}
---

Extract key information relevant to the query. Be specific and cite details from the text.
If this section has no relevant information, respond with exactly: "No relevant content in this section."

Keep your response concise (2-4 sentences max)."#,
            query,
            chunk.id + 1,
            total_chunks,
            section_info,
            chunk.content
        )
    }

    /// Phase 4: Synthesize findings into final answer.
    ///
    /// Uses hierarchical synthesis when there are too many findings:
    /// 1. Group findings into batches
    /// 2. Synthesize each batch into intermediate summaries
    /// 3. Synthesize intermediate summaries into final answer
    async fn synthesize(
        &self,
        summaries: &[ChunkSummary],
        query: &str,
    ) -> Result<String> {
        // Filter to only chunks with actual findings
        let relevant_findings: Vec<_> = summaries
            .iter()
            .filter(|s| {
                !s.findings.contains("No relevant content")
                    && !s.findings.starts_with("[Error")
            })
            .collect();

        if relevant_findings.is_empty() {
            return Ok("No relevant information found in the document.".to_string());
        }

        // If few findings, synthesize directly
        // Otherwise, use hierarchical synthesis
        const MAX_DIRECT_FINDINGS: usize = 10;
        const BATCH_SIZE: usize = 8;

        if relevant_findings.len() <= MAX_DIRECT_FINDINGS {
            self.synthesize_direct(&relevant_findings, query).await
        } else {
            self.synthesize_hierarchical(&relevant_findings, query, BATCH_SIZE)
                .await
        }
    }

    /// Direct synthesis for small number of findings.
    async fn synthesize_direct(
        &self,
        findings: &[&ChunkSummary],
        query: &str,
    ) -> Result<String> {
        let findings_text = self.format_findings(findings);

        let prompt = format!(
            r#"Based on the following extracted information from different sections of a document,
provide a comprehensive answer to this query:

QUERY: {}

EXTRACTED FINDINGS:

{}
---

Synthesize these findings into a coherent, well-organized answer.
Include specific details and examples from the sections.
Start your response directly with the answer (no preamble like "Based on...")."#,
            query, findings_text
        );

        let response = self
            .client
            .complete(&prompt, None)
            .await
            .map_err(|e| RlmError::LlmError(e.to_string()))?;

        let answer = response
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_else(|| "Failed to synthesize answer.".to_string());

        Ok(answer)
    }

    /// Hierarchical synthesis for many findings.
    async fn synthesize_hierarchical(
        &self,
        findings: &[&ChunkSummary],
        query: &str,
        batch_size: usize,
    ) -> Result<String> {
        if self.config.verbose {
            println!(
                "  Using hierarchical synthesis ({} findings in batches of {})",
                findings.len(),
                batch_size
            );
        }

        // Split findings into batches
        let batches: Vec<_> = findings.chunks(batch_size).collect();
        let mut intermediate_summaries = Vec::new();

        for (i, batch) in batches.iter().enumerate() {
            if self.config.verbose {
                print!("  Synthesizing batch {}/{}...", i + 1, batches.len());
            }

            let findings_text = self.format_findings(batch);

            let prompt = format!(
                r#"Summarize the key themes and information from these sections relevant to: {}

SECTIONS:

{}
---

Provide a concise summary (3-5 sentences) of the main themes.
Start your response directly with the summary."#,
                query, findings_text
            );

            match self.client.complete(&prompt, None).await {
                Ok(response) => {
                    let summary = response
                        .choices
                        .first()
                        .map(|c| c.message.content.clone())
                        .unwrap_or_default();

                    if self.config.verbose {
                        let preview: String = summary.chars().take(60).collect();
                        println!(" {}", preview.replace('\n', " "));
                    }

                    intermediate_summaries.push(summary);
                }
                Err(e) => {
                    if self.config.verbose {
                        println!(" ERROR: {}", e);
                    }
                    // Continue with other batches
                }
            }
        }

        // Final synthesis from intermediate summaries
        if self.config.verbose {
            println!(
                "  Final synthesis from {} intermediate summaries...",
                intermediate_summaries.len()
            );
        }

        let combined = intermediate_summaries
            .iter()
            .enumerate()
            .map(|(i, s)| format!("### Group {}\n{}", i + 1, s))
            .collect::<Vec<_>>()
            .join("\n\n");

        let final_prompt = format!(
            r#"Based on these summaries from different parts of a document, provide a comprehensive answer to:

QUERY: {}

SUMMARIES:

{}
---

Synthesize into a well-organized final answer.
Include specific themes and details mentioned.
Start your response directly with the answer."#,
            query, combined
        );

        let response = self
            .client
            .complete(&final_prompt, None)
            .await
            .map_err(|e| RlmError::LlmError(e.to_string()))?;

        let answer = response
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_else(|| "Failed to synthesize answer.".to_string());

        Ok(answer)
    }

    /// Format findings into text for prompts.
    fn format_findings(&self, findings: &[&ChunkSummary]) -> String {
        let mut text = String::new();
        for summary in findings {
            let section_label = summary
                .section_title
                .as_ref()
                .map(|t| format!(" ({})", t))
                .unwrap_or_default();

            text.push_str(&format!(
                "### Section {}{}\n{}\n\n",
                summary.chunk_id + 1,
                section_label,
                summary.findings
            ));
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let config = OrchestratorConfig::default();
        assert_eq!(config.chunk_size, 6000);
        assert_eq!(config.overlap, 200);
        assert_eq!(config.max_chunks, 50);
        assert!(config.semantic_chunking);
        assert!(config.verbose);
    }

    #[test]
    fn test_extraction_prompt_format() {
        // Create a mock chunk
        let chunk = Chunk {
            id: 0,
            content: "Test content about WebGPU performance.".to_string(),
            start_pos: 0,
            end_pos: 38,
            section_context: Some("## Technical Details".to_string()),
        };

        // We can't easily test the prompt building without FMClient,
        // but we can verify the chunk structure
        assert_eq!(chunk.id, 0);
        assert!(chunk.section_context.is_some());
    }
}
