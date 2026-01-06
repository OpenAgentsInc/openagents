//! BM25 index wrapper for context retrieval.
//!
//! Provides a simple interface for building and querying a BM25 index
//! over context documents or paragraphs.

use bm25::{Document, Language, SearchEngineBuilder};

/// A search result from the BM25 index.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// The matching text segment.
    pub text: String,
    /// The relevance score.
    pub score: f32,
    /// Position in the original context (segment index).
    pub index: usize,
    /// Start character position in the original context.
    pub start_pos: usize,
    /// End character position in the original context.
    pub end_pos: usize,
}

/// A segment of the context.
#[derive(Debug, Clone)]
struct Segment {
    /// The text content.
    text: String,
    /// Start position in the original context.
    start_pos: usize,
    /// End position in the original context.
    end_pos: usize,
}

/// BM25 index for context retrieval.
///
/// Splits context into segments (by paragraph or sentence) and indexes them
/// for BM25 retrieval.
pub struct Bm25Index {
    /// The original context.
    context: String,
    /// The segments of the context.
    segments: Vec<Segment>,
    /// BM25 search engine.
    search_engine: bm25::SearchEngine<usize>,
}

impl Bm25Index {
    /// Create a new BM25 index from a context string.
    ///
    /// Segments the context by paragraphs (double newlines).
    pub fn new(context: &str) -> Self {
        let segments = Self::segment_by_paragraph(context);

        if segments.is_empty() {
            // Create empty search engine
            let search_engine = SearchEngineBuilder::<usize>::with_avgdl(1.0).build();
            return Self {
                context: context.to_string(),
                segments,
                search_engine,
            };
        }

        // Create documents with indices as IDs
        let documents: Vec<Document<usize>> = segments
            .iter()
            .enumerate()
            .map(|(i, s)| Document::new(i, s.text.clone()))
            .collect();

        // Build search engine
        let search_engine =
            SearchEngineBuilder::<usize>::with_documents(Language::English, documents).build();

        Self {
            context: context.to_string(),
            segments,
            search_engine,
        }
    }

    /// Create a BM25 index with sentence-level segmentation.
    pub fn with_sentence_segmentation(context: &str) -> Self {
        let segments = Self::segment_by_sentence(context);

        if segments.is_empty() {
            let search_engine = SearchEngineBuilder::<usize>::with_avgdl(1.0).build();
            return Self {
                context: context.to_string(),
                segments,
                search_engine,
            };
        }

        let documents: Vec<Document<usize>> = segments
            .iter()
            .enumerate()
            .map(|(i, s)| Document::new(i, s.text.clone()))
            .collect();

        let search_engine =
            SearchEngineBuilder::<usize>::with_documents(Language::English, documents).build();

        Self {
            context: context.to_string(),
            segments,
            search_engine,
        }
    }

    /// Segment context by paragraphs (double newlines).
    fn segment_by_paragraph(context: &str) -> Vec<Segment> {
        let mut segments = Vec::new();
        let mut current_pos = 0;

        for paragraph in context.split("\n\n") {
            let trimmed = paragraph.trim();
            if !trimmed.is_empty() {
                let start_pos = context[current_pos..]
                    .find(trimmed)
                    .map(|p| current_pos + p)
                    .unwrap_or(current_pos);
                let end_pos = start_pos + trimmed.len();

                segments.push(Segment {
                    text: trimmed.to_string(),
                    start_pos,
                    end_pos,
                });
            }

            // Move past this paragraph and the double newline
            current_pos += paragraph.len() + 2;
        }

        segments
    }

    /// Segment context by sentences.
    fn segment_by_sentence(context: &str) -> Vec<Segment> {
        let mut segments = Vec::new();

        // Simple sentence splitting on . ! ?
        let sentence_ends = regex::Regex::new(r"[.!?]\s+").unwrap();

        let mut last_end = 0;
        for mat in sentence_ends.find_iter(context) {
            let sentence = &context[last_end..mat.end()];
            let trimmed = sentence.trim();
            if !trimmed.is_empty() {
                segments.push(Segment {
                    text: trimmed.to_string(),
                    start_pos: last_end,
                    end_pos: mat.end(),
                });
            }
            last_end = mat.end();
        }

        // Don't forget the last sentence
        if last_end < context.len() {
            let remaining = &context[last_end..];
            let trimmed = remaining.trim();
            if !trimmed.is_empty() {
                segments.push(Segment {
                    text: trimmed.to_string(),
                    start_pos: last_end,
                    end_pos: context.len(),
                });
            }
        }

        segments
    }

    /// Search the index for the top-k most relevant segments.
    pub fn search(&self, query: &str, top_k: usize) -> Vec<SearchResult> {
        if self.segments.is_empty() {
            return vec![];
        }

        // Search using the bm25 search engine
        let results = self.search_engine.search(query, top_k);

        // Convert to our SearchResult type
        results
            .into_iter()
            .filter(|r| r.score > 0.0)
            .map(|r| {
                let idx = r.document.id;
                let segment = &self.segments[idx];
                SearchResult {
                    text: segment.text.clone(),
                    score: r.score,
                    index: idx,
                    start_pos: segment.start_pos,
                    end_pos: segment.end_pos,
                }
            })
            .collect()
    }

    /// Get the number of segments in the index.
    pub fn len(&self) -> usize {
        self.segments.len()
    }

    /// Check if the index is empty.
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    /// Get the original context.
    pub fn context(&self) -> &str {
        &self.context
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paragraph_segmentation() {
        let context = "First paragraph here.\n\nSecond paragraph follows.\n\nThird one.";
        let index = Bm25Index::new(context);

        assert_eq!(index.len(), 3);
    }

    #[test]
    fn test_search() {
        let context = "The quick brown fox jumps over the lazy dog.\n\n\
                       Python is a programming language.\n\n\
                       The fox was very fast.";
        let index = Bm25Index::new(context);

        let results = index.search("fox", 2);
        assert!(!results.is_empty());
        // Results about fox should be ranked first
        assert!(results[0].text.contains("fox") || results.len() == 1);
    }

    #[test]
    fn test_empty_context() {
        let index = Bm25Index::new("");
        assert!(index.is_empty());

        let results = index.search("anything", 10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_sentence_segmentation() {
        let context = "First sentence. Second sentence! Third sentence?";
        let index = Bm25Index::with_sentence_segmentation(context);

        // Should have 3 sentences
        assert_eq!(index.len(), 3);
    }
}
