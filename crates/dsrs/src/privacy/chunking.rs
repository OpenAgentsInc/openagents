//! Chunking policies for privacy-aware content splitting.
//!
//! Controls how much context is sent to swarm providers.

use serde::{Deserialize, Serialize};
use std::ops::Range;

/// Policy for chunking content before sending to providers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChunkingPolicy {
    /// Send full context (default, no chunking).
    Full,

    /// Send minimal spans only (changed lines + limited context).
    MinimalSpans {
        /// Number of context lines before/after the target span.
        context_lines: usize,
    },

    /// Send only AST nodes (requires language-aware parsing).
    AstNodesOnly {
        /// Node types to include (e.g., "function", "class", "impl").
        node_types: Vec<String>,
    },

    /// Send fixed-size chunks.
    FixedSize {
        /// Maximum characters per chunk.
        max_chars: usize,
        /// Overlap between chunks for context.
        overlap_chars: usize,
    },
}

impl Default for ChunkingPolicy {
    fn default() -> Self {
        Self::Full
    }
}

impl ChunkingPolicy {
    /// Create minimal spans policy with default context.
    pub fn minimal() -> Self {
        Self::MinimalSpans { context_lines: 3 }
    }

    /// Create minimal spans policy with custom context.
    pub fn minimal_with_context(lines: usize) -> Self {
        Self::MinimalSpans {
            context_lines: lines,
        }
    }

    /// Create fixed-size chunking policy.
    pub fn fixed_size(max_chars: usize) -> Self {
        Self::FixedSize {
            max_chars,
            overlap_chars: 100,
        }
    }
}

/// A chunk of content with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentChunk {
    /// The chunk content.
    pub content: String,

    /// Byte range in original content.
    pub span: Range<usize>,

    /// Line range in original content.
    pub line_range: Range<usize>,

    /// Kind of chunk (e.g., "function", "context", "full").
    pub kind: ChunkKind,

    /// Optional file path this chunk belongs to.
    pub file_path: Option<String>,
}

/// Kind of content chunk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChunkKind {
    /// Full file content.
    Full,
    /// Minimal span with context.
    Span,
    /// AST node (function, class, etc.).
    AstNode(String),
    /// Fixed-size chunk.
    FixedSize,
    /// Context lines around a span.
    Context,
}

/// Chunker that splits content according to policy.
#[derive(Debug, Clone, Default)]
pub struct Chunker {
    policy: ChunkingPolicy,
}

impl Chunker {
    /// Create a new chunker with the given policy.
    pub fn new(policy: ChunkingPolicy) -> Self {
        Self { policy }
    }

    /// Split content into chunks according to policy.
    pub fn chunk(&self, content: &str, file_path: Option<&str>) -> Vec<ContentChunk> {
        match &self.policy {
            ChunkingPolicy::Full => vec![ContentChunk {
                content: content.to_string(),
                span: 0..content.len(),
                line_range: 0..content.lines().count(),
                kind: ChunkKind::Full,
                file_path: file_path.map(String::from),
            }],

            ChunkingPolicy::MinimalSpans { context_lines } => {
                // For minimal spans, we need target lines to focus on
                // This is a simplified implementation that just returns the full content
                // In practice, this would be called with specific line ranges to focus on
                vec![ContentChunk {
                    content: content.to_string(),
                    span: 0..content.len(),
                    line_range: 0..content.lines().count(),
                    kind: ChunkKind::Span,
                    file_path: file_path.map(String::from),
                }]
            }

            ChunkingPolicy::AstNodesOnly { node_types: _ } => {
                // Would require tree-sitter or similar for proper AST parsing
                // For now, return full content as a single chunk
                vec![ContentChunk {
                    content: content.to_string(),
                    span: 0..content.len(),
                    line_range: 0..content.lines().count(),
                    kind: ChunkKind::Full,
                    file_path: file_path.map(String::from),
                }]
            }

            ChunkingPolicy::FixedSize {
                max_chars,
                overlap_chars,
            } => self.chunk_fixed_size(content, *max_chars, *overlap_chars, file_path),
        }
    }

    /// Extract minimal span around target lines.
    pub fn extract_span(
        &self,
        content: &str,
        target_lines: Range<usize>,
        file_path: Option<&str>,
    ) -> ContentChunk {
        let context_lines = match &self.policy {
            ChunkingPolicy::MinimalSpans { context_lines } => *context_lines,
            _ => 3,
        };

        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();

        let start_line = target_lines.start.saturating_sub(context_lines);
        let end_line = (target_lines.end + context_lines).min(total_lines);

        let span_content: String = lines[start_line..end_line].join("\n");

        // Calculate byte offset
        let byte_start: usize = lines[..start_line].iter().map(|l| l.len() + 1).sum();
        let byte_end: usize = byte_start + span_content.len();

        ContentChunk {
            content: span_content,
            span: byte_start..byte_end,
            line_range: start_line..end_line,
            kind: ChunkKind::Span,
            file_path: file_path.map(String::from),
        }
    }

    /// Split content into fixed-size chunks with overlap.
    fn chunk_fixed_size(
        &self,
        content: &str,
        max_chars: usize,
        overlap_chars: usize,
        file_path: Option<&str>,
    ) -> Vec<ContentChunk> {
        if content.len() <= max_chars {
            return vec![ContentChunk {
                content: content.to_string(),
                span: 0..content.len(),
                line_range: 0..content.lines().count(),
                kind: ChunkKind::FixedSize,
                file_path: file_path.map(String::from),
            }];
        }

        let mut chunks = Vec::new();
        let mut start = 0;
        let step = max_chars.saturating_sub(overlap_chars);

        while start < content.len() {
            let end = (start + max_chars).min(content.len());
            let chunk_content = &content[start..end];

            // Count lines up to this chunk
            let lines_before: usize = content[..start].matches('\n').count();
            let lines_in_chunk: usize = chunk_content.matches('\n').count() + 1;

            chunks.push(ContentChunk {
                content: chunk_content.to_string(),
                span: start..end,
                line_range: lines_before..(lines_before + lines_in_chunk),
                kind: ChunkKind::FixedSize,
                file_path: file_path.map(String::from),
            });

            if end >= content.len() {
                break;
            }
            start += step;
        }

        chunks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_full_chunking() {
        let chunker = Chunker::new(ChunkingPolicy::Full);
        let content = "line 1\nline 2\nline 3";
        let chunks = chunker.chunk(content, Some("test.rs"));

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].content, content);
        assert_eq!(chunks[0].kind, ChunkKind::Full);
    }

    #[test]
    fn test_fixed_size_chunking() {
        let chunker = Chunker::new(ChunkingPolicy::fixed_size(10));
        let content = "abcdefghijklmnopqrstuvwxyz";
        let chunks = chunker.chunk(content, None);

        assert!(chunks.len() > 1);
        for chunk in &chunks {
            assert!(chunk.content.len() <= 10);
            assert_eq!(chunk.kind, ChunkKind::FixedSize);
        }
    }

    #[test]
    fn test_extract_span() {
        let chunker = Chunker::new(ChunkingPolicy::minimal_with_context(1));
        let content = "line 0\nline 1\nline 2\nline 3\nline 4";
        let chunk = chunker.extract_span(content, 2..3, Some("test.rs"));

        // Should include lines 1-3 (target 2 with 1 line context each side)
        assert!(chunk.content.contains("line 1"));
        assert!(chunk.content.contains("line 2"));
        assert!(chunk.content.contains("line 3"));
        assert_eq!(chunk.kind, ChunkKind::Span);
    }

    #[test]
    fn test_span_boundary_handling() {
        let chunker = Chunker::new(ChunkingPolicy::minimal_with_context(2));
        let content = "line 0\nline 1\nline 2";

        // Target line 0 with 2 context - should not go negative
        let chunk = chunker.extract_span(content, 0..1, None);
        assert!(chunk.line_range.start == 0);
    }
}
