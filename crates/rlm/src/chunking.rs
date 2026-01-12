//! Semantic chunking for document analysis.
//!
//! Provides intelligent document splitting that respects natural boundaries
//! (markdown headers, code functions, paragraphs) rather than arbitrary offsets.

use regex::Regex;

/// Document structure detected from content.
#[derive(Debug, Clone)]
pub struct DocumentStructure {
    /// Type of document detected.
    pub doc_type: DocumentType,
    /// Sections found in the document.
    pub sections: Vec<Section>,
    /// Total document length in characters.
    pub total_length: usize,
}

/// Type of document based on content analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentType {
    /// Markdown with headers, lists, code blocks.
    Markdown,
    /// Source code with functions, classes.
    Code,
    /// Plain prose with paragraphs.
    Prose,
    /// Combination of types.
    Mixed,
}

/// A section within the document.
#[derive(Debug, Clone)]
pub struct Section {
    /// Section title (header text for markdown).
    pub title: Option<String>,
    /// Start position in the document.
    pub start: usize,
    /// End position in the document.
    pub end: usize,
    /// Depth level (1 for #, 2 for ##, etc.).
    pub depth: usize,
}

/// A chunk of the document for processing.
#[derive(Debug, Clone)]
pub struct Chunk {
    /// Unique chunk identifier.
    pub id: usize,
    /// The chunk content.
    pub content: String,
    /// Start position in original document.
    pub start_pos: usize,
    /// End position in original document.
    pub end_pos: usize,
    /// Section context (e.g., "## Browser-Based LLM Runtime").
    pub section_context: Option<String>,
}

/// Detect document structure using pattern matching.
pub fn detect_structure(content: &str) -> DocumentStructure {
    let total_length = content.len();

    // Check for markdown headers
    let md_header_re = Regex::new(r"(?m)^(#{1,6})\s+(.+)$").unwrap();
    let md_headers: Vec<_> = md_header_re.captures_iter(content).collect();

    // Check for code patterns
    let code_patterns = [
        r"(?m)^(pub\s+)?(async\s+)?fn\s+\w+",          // Rust
        r"(?m)^def\s+\w+",                             // Python
        r"(?m)^(export\s+)?(async\s+)?function\s+\w+", // JavaScript
        r"(?m)^class\s+\w+",                           // Python/JS/Rust
    ];
    let code_matches: usize = code_patterns
        .iter()
        .filter_map(|p| Regex::new(p).ok())
        .map(|re| re.find_iter(content).count())
        .sum();

    // Determine document type
    let doc_type = if !md_headers.is_empty() && md_headers.len() > 2 {
        DocumentType::Markdown
    } else if code_matches > 5 {
        DocumentType::Code
    } else if md_headers.is_empty() && code_matches == 0 {
        DocumentType::Prose
    } else {
        DocumentType::Mixed
    };

    // Build sections based on document type
    let sections = match doc_type {
        DocumentType::Markdown => extract_markdown_sections(content, &md_headers),
        DocumentType::Code => extract_code_sections(content),
        DocumentType::Prose => extract_prose_sections(content),
        DocumentType::Mixed => extract_mixed_sections(content, &md_headers),
    };

    DocumentStructure {
        doc_type,
        sections,
        total_length,
    }
}

/// Extract sections from markdown headers.
fn extract_markdown_sections(content: &str, headers: &[regex::Captures]) -> Vec<Section> {
    let mut sections = Vec::new();

    for (i, cap) in headers.iter().enumerate() {
        let full_match = cap.get(0).unwrap();
        let hashes = cap.get(1).unwrap().as_str();
        let title = cap.get(2).unwrap().as_str();

        let start = full_match.start();
        let end = if i + 1 < headers.len() {
            headers[i + 1].get(0).unwrap().start()
        } else {
            content.len()
        };

        sections.push(Section {
            title: Some(title.to_string()),
            start,
            end,
            depth: hashes.len(),
        });
    }

    // If there's content before the first header, add it
    if !sections.is_empty() && sections[0].start > 0 {
        sections.insert(
            0,
            Section {
                title: Some("Introduction".to_string()),
                start: 0,
                end: sections[0].start,
                depth: 0,
            },
        );
    }

    // If no headers found, treat whole doc as one section
    if sections.is_empty() {
        sections.push(Section {
            title: None,
            start: 0,
            end: content.len(),
            depth: 0,
        });
    }

    sections
}

/// Extract sections from code (by function/class boundaries).
fn extract_code_sections(content: &str) -> Vec<Section> {
    let mut sections = Vec::new();
    let mut last_end = 0;

    // Find function definitions
    let fn_re = Regex::new(r"(?m)^(pub\s+)?(async\s+)?fn\s+(\w+)").unwrap();
    for cap in fn_re.captures_iter(content) {
        let full_match = cap.get(0).unwrap();
        let name = cap.get(3).unwrap().as_str();

        // If there's a gap, add it as a section
        if full_match.start() > last_end {
            sections.push(Section {
                title: None,
                start: last_end,
                end: full_match.start(),
                depth: 0,
            });
        }

        // Find the end of this function (next fn or end)
        let fn_end = find_next_fn_start(content, full_match.end()).unwrap_or(content.len());

        sections.push(Section {
            title: Some(format!("fn {}", name)),
            start: full_match.start(),
            end: fn_end,
            depth: 1,
        });

        last_end = fn_end;
    }

    // Add remaining content
    if last_end < content.len() {
        sections.push(Section {
            title: None,
            start: last_end,
            end: content.len(),
            depth: 0,
        });
    }

    if sections.is_empty() {
        sections.push(Section {
            title: None,
            start: 0,
            end: content.len(),
            depth: 0,
        });
    }

    sections
}

fn find_next_fn_start(content: &str, from: usize) -> Option<usize> {
    let fn_re = Regex::new(r"(?m)^(pub\s+)?(async\s+)?fn\s+\w+").unwrap();
    fn_re.find(&content[from..]).map(|m| from + m.start())
}

/// Extract sections from prose (by paragraphs).
fn extract_prose_sections(content: &str) -> Vec<Section> {
    let mut sections = Vec::new();
    let paragraph_re = Regex::new(r"\n\n+").unwrap();

    let mut last_end = 0;
    for mat in paragraph_re.find_iter(content) {
        if mat.start() > last_end {
            sections.push(Section {
                title: None,
                start: last_end,
                end: mat.start(),
                depth: 0,
            });
        }
        last_end = mat.end();
    }

    // Add final paragraph
    if last_end < content.len() {
        sections.push(Section {
            title: None,
            start: last_end,
            end: content.len(),
            depth: 0,
        });
    }

    if sections.is_empty() {
        sections.push(Section {
            title: None,
            start: 0,
            end: content.len(),
            depth: 0,
        });
    }

    sections
}

/// Extract sections from mixed content.
fn extract_mixed_sections(content: &str, headers: &[regex::Captures]) -> Vec<Section> {
    // Use markdown sections if any, otherwise fall back to prose
    if !headers.is_empty() {
        extract_markdown_sections(content, headers)
    } else {
        extract_prose_sections(content)
    }
}

/// Split content into chunks respecting section boundaries.
pub fn chunk_by_structure(
    content: &str,
    structure: &DocumentStructure,
    max_chunk_size: usize,
    overlap: usize,
) -> Vec<Chunk> {
    let mut chunks = Vec::new();
    let mut chunk_id = 0;

    for section in &structure.sections {
        let section_content = &content[section.start..section.end];
        let section_title = section.title.as_ref().map(|t| {
            if section.depth > 0 {
                format!("{} {}", "#".repeat(section.depth), t)
            } else {
                t.clone()
            }
        });

        if section_content.len() <= max_chunk_size {
            // Section fits in one chunk
            chunks.push(Chunk {
                id: chunk_id,
                content: section_content.to_string(),
                start_pos: section.start,
                end_pos: section.end,
                section_context: section_title,
            });
            chunk_id += 1;
        } else {
            // Section too large, split at paragraph boundaries
            let sub_chunks = split_large_section(
                section_content,
                section.start,
                max_chunk_size,
                overlap,
                &section_title,
            );
            for (sub_content, sub_start, sub_end) in sub_chunks {
                chunks.push(Chunk {
                    id: chunk_id,
                    content: sub_content,
                    start_pos: sub_start,
                    end_pos: sub_end,
                    section_context: section_title.clone(),
                });
                chunk_id += 1;
            }
        }
    }

    chunks
}

/// Split a large section into smaller chunks at paragraph boundaries.
fn split_large_section(
    content: &str,
    base_offset: usize,
    max_size: usize,
    overlap: usize,
    _section_title: &Option<String>,
) -> Vec<(String, usize, usize)> {
    let mut chunks = Vec::new();
    let mut start = 0;

    while start < content.len() {
        let mut end = (start + max_size).min(content.len());

        // Try to find a paragraph break near the end
        if end < content.len() {
            if let Some(break_pos) = find_paragraph_break(&content[start..end]) {
                end = start + break_pos;
            } else if let Some(break_pos) = find_line_break(&content[start..end]) {
                end = start + break_pos;
            }
        }

        let chunk_content = content[start..end].to_string();
        chunks.push((chunk_content, base_offset + start, base_offset + end));

        // Move start, accounting for overlap
        start = if end >= content.len() {
            content.len()
        } else {
            end.saturating_sub(overlap).max(start + 1)
        };

        // Prevent infinite loop
        if start >= end && end < content.len() {
            start = end;
        }
    }

    chunks
}

/// Find a paragraph break (double newline) position.
fn find_paragraph_break(content: &str) -> Option<usize> {
    // Search backwards from the end for \n\n
    let bytes = content.as_bytes();
    for i in (1..bytes.len()).rev() {
        if bytes[i] == b'\n' && bytes[i - 1] == b'\n' {
            return Some(i + 1);
        }
    }
    None
}

/// Find a line break position.
fn find_line_break(content: &str) -> Option<usize> {
    // Search backwards from the end for \n
    content.rfind('\n').map(|i| i + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_markdown_structure() {
        let content =
            "# Title\n\nIntro text.\n\n## Section 1\n\nContent 1.\n\n## Section 2\n\nContent 2.";
        let structure = detect_structure(content);

        assert_eq!(structure.doc_type, DocumentType::Markdown);
        assert!(structure.sections.len() >= 2);
    }

    #[test]
    fn test_detect_prose_structure() {
        let content = "First paragraph here.\n\nSecond paragraph here.\n\nThird paragraph.";
        let structure = detect_structure(content);

        assert_eq!(structure.doc_type, DocumentType::Prose);
    }

    #[test]
    fn test_chunk_small_document() {
        let content = "# Test\n\nSmall content.";
        let structure = detect_structure(content);
        let chunks = chunk_by_structure(content, &structure, 1000, 50);

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].content, content);
    }

    #[test]
    fn test_chunk_large_section() {
        let content = "# Test\n\n".to_string() + &"A".repeat(10000);
        let structure = detect_structure(&content);
        let chunks = chunk_by_structure(&content, &structure, 3000, 100);

        assert!(chunks.len() > 1);
        // All chunks should be under max size (plus some tolerance for overlap)
        for chunk in &chunks {
            assert!(chunk.content.len() <= 3100);
        }
    }

    #[test]
    fn test_section_context_preserved() {
        let content = "# Main Title\n\n## Section One\n\nContent here.";
        let structure = detect_structure(content);
        let chunks = chunk_by_structure(content, &structure, 10000, 50);

        // Should have section context
        let section_chunk = chunks.iter().find(|c| c.content.contains("Content here"));
        assert!(section_chunk.is_some());
        assert!(section_chunk.unwrap().section_context.is_some());
    }
}
