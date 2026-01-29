//! Block-level parsing for live markdown formatting
//!
//! Identifies markdown block types and parses inline formatting.

use crate::color::Hsla;

/// Type of markdown block
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockType {
    /// Regular paragraph text
    Paragraph,
    /// Header (h1-h6)
    Header(u8),
    /// Fenced code block (inside ```)
    CodeBlock,
    /// Code block fence line (``` or ```language)
    CodeFence,
    /// Unordered list item (- or *)
    UnorderedList,
    /// Ordered list item (1. 2. etc)
    OrderedList,
    /// Blockquote (>)
    Blockquote,
    /// Horizontal rule (--- or ***)
    HorizontalRule,
    /// Empty line
    Empty,
}

/// Inline formatting span
#[derive(Debug, Clone)]
pub struct InlineSpan {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    #[allow(dead_code)] // Will be used for strikethrough rendering
    pub strikethrough: bool,
    pub code: bool,
    #[allow(dead_code)] // Will be used for link rendering
    pub link: Option<String>,
}

impl InlineSpan {
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            bold: false,
            italic: false,
            strikethrough: false,
            code: false,
            link: None,
        }
    }
}

/// Block parser for detecting markdown structure
pub struct BlockParser {
    in_code_block: bool,
}

impl BlockParser {
    pub fn new() -> Self {
        Self {
            in_code_block: false,
        }
    }

    /// Detect the block type of a line, with line index for title detection
    pub fn detect_block_type_at(&mut self, line: &str, line_index: usize) -> BlockType {
        let block_type = self.detect_block_type(line);
        // First line is always treated as H1 (title) unless it's a code block
        if line_index == 0 && matches!(block_type, BlockType::Paragraph) {
            return BlockType::Header(1);
        }
        block_type
    }

    /// Detect the block type of a line
    pub fn detect_block_type(&mut self, line: &str) -> BlockType {
        let trimmed = line.trim_start();

        // Check for code fence
        if trimmed.starts_with("```") {
            self.in_code_block = !self.in_code_block;
            return BlockType::CodeFence;
        }

        // Inside code block
        if self.in_code_block {
            return BlockType::CodeBlock;
        }

        // Empty line
        if trimmed.is_empty() {
            return BlockType::Empty;
        }

        // Headers
        if let Some(level) = Self::detect_header(trimmed) {
            return BlockType::Header(level);
        }

        // Horizontal rule
        if Self::is_horizontal_rule(trimmed) {
            return BlockType::HorizontalRule;
        }

        // Unordered list
        if Self::is_unordered_list(trimmed) {
            return BlockType::UnorderedList;
        }

        // Ordered list
        if Self::is_ordered_list(trimmed) {
            return BlockType::OrderedList;
        }

        // Blockquote
        if trimmed.starts_with('>') {
            return BlockType::Blockquote;
        }

        BlockType::Paragraph
    }

    fn detect_header(line: &str) -> Option<u8> {
        let mut level = 0u8;
        for c in line.chars() {
            if c == '#' {
                level += 1;
            } else if c == ' ' && level > 0 {
                return Some(level.min(6));
            } else {
                break;
            }
        }
        None
    }

    fn is_horizontal_rule(line: &str) -> bool {
        let chars: Vec<char> = line.chars().filter(|c| !c.is_whitespace()).collect();
        if chars.len() < 3 {
            return false;
        }
        let first = chars[0];
        (first == '-' || first == '*' || first == '_') && chars.iter().all(|&c| c == first)
    }

    fn is_unordered_list(line: &str) -> bool {
        let mut chars = line.chars();
        match chars.next() {
            Some('-') | Some('*') | Some('+') => {
                matches!(chars.next(), Some(' ') | Some('\t'))
            }
            _ => false,
        }
    }

    fn is_ordered_list(line: &str) -> bool {
        let mut chars = line.chars().peekable();
        // Check for digits
        let mut has_digit = false;
        while let Some(&c) = chars.peek() {
            if c.is_ascii_digit() {
                has_digit = true;
                chars.next();
            } else {
                break;
            }
        }
        if !has_digit {
            return false;
        }
        // Check for . or ) followed by space
        match chars.next() {
            Some('.') | Some(')') => matches!(chars.next(), Some(' ') | Some('\t')),
            _ => false,
        }
    }

    /// Reset parser state (e.g., when content changes significantly)
    #[allow(dead_code)] // Will be used when content is reloaded
    pub fn reset(&mut self) {
        self.in_code_block = false;
    }
}

/// Parse inline formatting in a line
pub fn parse_inline(text: &str) -> Vec<InlineSpan> {
    let mut spans = Vec::new();
    let mut current = String::new();
    let mut bold = false;
    let mut italic = false;
    let mut strikethrough = false;
    let mut code = false;

    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();

        // Inline code (backtick)
        if c == '`' && !code {
            if !current.is_empty() {
                spans.push(InlineSpan {
                    text: std::mem::take(&mut current),
                    bold,
                    italic,
                    strikethrough,
                    code: false,
                    link: None,
                });
            }
            code = true;
            i += 1;
            continue;
        }
        if c == '`' && code {
            spans.push(InlineSpan {
                text: std::mem::take(&mut current),
                bold: false,
                italic: false,
                strikethrough: false,
                code: true,
                link: None,
            });
            code = false;
            i += 1;
            continue;
        }

        // Inside code, just accumulate
        if code {
            current.push(c);
            i += 1;
            continue;
        }

        // Bold (**) or italic (*)
        if c == '*' {
            if next == Some('*') {
                // Bold toggle
                if !current.is_empty() {
                    spans.push(InlineSpan {
                        text: std::mem::take(&mut current),
                        bold,
                        italic,
                        strikethrough,
                        code: false,
                        link: None,
                    });
                }
                bold = !bold;
                i += 2;
                continue;
            } else {
                // Italic toggle
                if !current.is_empty() {
                    spans.push(InlineSpan {
                        text: std::mem::take(&mut current),
                        bold,
                        italic,
                        strikethrough,
                        code: false,
                        link: None,
                    });
                }
                italic = !italic;
                i += 1;
                continue;
            }
        }

        // Strikethrough (~~)
        if c == '~' && next == Some('~') {
            if !current.is_empty() {
                spans.push(InlineSpan {
                    text: std::mem::take(&mut current),
                    bold,
                    italic,
                    strikethrough,
                    code: false,
                    link: None,
                });
            }
            strikethrough = !strikethrough;
            i += 2;
            continue;
        }

        current.push(c);
        i += 1;
    }

    // Push remaining text
    if !current.is_empty() {
        spans.push(InlineSpan {
            text: current,
            bold,
            italic,
            strikethrough,
            code,
            link: None,
        });
    }

    // If no spans were created, return a single plain span
    if spans.is_empty() {
        spans.push(InlineSpan::plain(""));
    }

    spans
}

/// Get font size multiplier for header level
pub fn header_font_scale(level: u8) -> f32 {
    match level {
        1 => 1.4,
        2 => 1.25,
        3 => 1.15,
        4 => 1.1,
        5 => 1.05,
        _ => 1.0,
    }
}

/// Get color for code background
#[allow(dead_code)] // Will be used for code block backgrounds
pub fn code_background_color() -> Hsla {
    Hsla::new(0.0, 0.0, 0.15, 1.0)
}

/// Get color for inline code background
pub fn inline_code_background() -> Hsla {
    Hsla::new(0.0, 0.0, 0.2, 1.0)
}

/// Strip markdown header prefix (# symbols and space)
pub fn strip_header_prefix(line: &str) -> &str {
    let trimmed = line.trim_start();
    let mut chars = trimmed.chars();
    let mut skip = 0;

    // Skip # symbols
    for c in chars {
        if c == '#' {
            skip += 1;
        } else if c == ' ' {
            skip += 1;
            break;
        } else {
            break;
        }
    }

    &trimmed[skip.min(trimmed.len())..]
}

/// Strip list prefix (- or * or 1. etc)
pub fn strip_list_prefix(line: &str) -> &str {
    let trimmed = line.trim_start();

    // Unordered list
    if trimmed.starts_with("- ") || trimmed.starts_with("* ") || trimmed.starts_with("+ ") {
        return &trimmed[2..];
    }

    // Ordered list
    let mut chars = trimmed.chars().peekable();
    let mut digits = 0;
    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() {
            digits += 1;
            chars.next();
        } else {
            break;
        }
    }
    if digits > 0
        && let Some(c) = chars.next()
        && matches!(c, '.' | ')')
        && chars.next() == Some(' ')
    {
        return &trimmed[digits + 2..];
    }

    trimmed
}

/// Strip blockquote prefix (> and optional space)
pub fn strip_blockquote_prefix(line: &str) -> &str {
    let trimmed = line.trim_start();
    trimmed
        .strip_prefix("> ")
        .or_else(|| trimmed.strip_prefix('>'))
        .unwrap_or(trimmed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_header() {
        let mut parser = BlockParser::new();
        assert_eq!(parser.detect_block_type("# Header"), BlockType::Header(1));
        assert_eq!(parser.detect_block_type("## Header"), BlockType::Header(2));
        assert_eq!(parser.detect_block_type("### Header"), BlockType::Header(3));
    }

    #[test]
    fn test_detect_code_block() {
        let mut parser = BlockParser::new();
        assert_eq!(parser.detect_block_type("```rust"), BlockType::CodeFence);
        assert_eq!(parser.detect_block_type("let x = 1;"), BlockType::CodeBlock);
        assert_eq!(parser.detect_block_type("```"), BlockType::CodeFence);
        assert_eq!(
            parser.detect_block_type("normal text"),
            BlockType::Paragraph
        );
    }

    #[test]
    fn test_detect_list() {
        let mut parser = BlockParser::new();
        assert_eq!(parser.detect_block_type("- item"), BlockType::UnorderedList);
        assert_eq!(parser.detect_block_type("* item"), BlockType::UnorderedList);
        assert_eq!(parser.detect_block_type("1. item"), BlockType::OrderedList);
        assert_eq!(parser.detect_block_type("10. item"), BlockType::OrderedList);
    }

    #[test]
    fn test_parse_inline_bold() {
        let spans = parse_inline("hello **world**");
        assert_eq!(spans.len(), 2);
        assert!(!spans[0].bold);
        assert!(spans[1].bold);
    }

    #[test]
    fn test_parse_inline_code() {
        let spans = parse_inline("use `code` here");
        assert_eq!(spans.len(), 3);
        assert!(!spans[0].code);
        assert!(spans[1].code);
        assert!(!spans[2].code);
    }

    #[test]
    fn test_strip_header_prefix() {
        assert_eq!(strip_header_prefix("# Hello"), "Hello");
        assert_eq!(strip_header_prefix("## World"), "World");
        assert_eq!(strip_header_prefix("### Test"), "Test");
    }
}
