use gpui::{FontStyle, FontWeight, HighlightStyle, Hsla, StyledText};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use std::ops::Range;

/// Style configuration for markdown rendering
pub struct MarkdownStyle {
    pub code_color: Hsla,
    pub heading_weight: FontWeight,
}

impl Default for MarkdownStyle {
    fn default() -> Self {
        Self {
            code_color: gpui::hsla(0.55, 0.6, 0.6, 1.0), // Cyan-ish for code
            heading_weight: FontWeight::BOLD,
        }
    }
}

/// Parse markdown text and return StyledText with appropriate formatting
pub fn render_markdown(text: &str, style: &MarkdownStyle) -> StyledText {
    let mut output = String::new();
    let mut highlights: Vec<(Range<usize>, HighlightStyle)> = Vec::new();

    // Track current styling state
    let mut bold_start: Option<usize> = None;
    let mut italic_start: Option<usize> = None;
    let mut code_start: Option<usize> = None;
    let mut heading_start: Option<usize> = None;

    let options = Options::empty();
    let parser = Parser::new_ext(text, options);

    for event in parser {
        match event {
            Event::Start(tag) => match tag {
                Tag::Strong => {
                    bold_start = Some(output.len());
                }
                Tag::Emphasis => {
                    italic_start = Some(output.len());
                }
                Tag::CodeBlock(_) => {
                    code_start = Some(output.len());
                }
                Tag::Heading { .. } => {
                    heading_start = Some(output.len());
                }
                Tag::Paragraph => {
                    if !output.is_empty() && !output.ends_with('\n') {
                        output.push('\n');
                    }
                }
                Tag::Item => {
                    output.push_str("  - ");
                }
                _ => {}
            },
            Event::End(tag_end) => match tag_end {
                TagEnd::Strong => {
                    if let Some(start) = bold_start.take() {
                        highlights.push((
                            start..output.len(),
                            FontWeight::BOLD.into(),
                        ));
                    }
                }
                TagEnd::Emphasis => {
                    if let Some(start) = italic_start.take() {
                        highlights.push((
                            start..output.len(),
                            FontStyle::Italic.into(),
                        ));
                    }
                }
                TagEnd::CodeBlock => {
                    if let Some(start) = code_start.take() {
                        highlights.push((
                            start..output.len(),
                            HighlightStyle {
                                color: Some(style.code_color),
                                ..Default::default()
                            },
                        ));
                    }
                    output.push('\n');
                }
                TagEnd::Heading(_) => {
                    if let Some(start) = heading_start.take() {
                        highlights.push((
                            start..output.len(),
                            style.heading_weight.into(),
                        ));
                    }
                    output.push('\n');
                }
                TagEnd::Paragraph => {
                    output.push('\n');
                }
                TagEnd::Item => {
                    if !output.ends_with('\n') {
                        output.push('\n');
                    }
                }
                _ => {}
            },
            Event::Text(text) => {
                output.push_str(&text);
            }
            Event::Code(code) => {
                let start = output.len();
                output.push('`');
                output.push_str(&code);
                output.push('`');
                highlights.push((
                    start..output.len(),
                    HighlightStyle {
                        color: Some(style.code_color),
                        ..Default::default()
                    },
                ));
            }
            Event::SoftBreak | Event::HardBreak => {
                output.push('\n');
            }
            _ => {}
        }
    }

    // Trim trailing whitespace and adjust highlight ranges
    let trimmed_len = output.trim_end().len();
    output.truncate(trimmed_len);

    // Filter out highlights that extend beyond the trimmed text
    let highlights: Vec<_> = highlights
        .into_iter()
        .filter(|(range, _)| range.start < output.len())
        .map(|(range, style)| {
            let end = range.end.min(output.len());
            (range.start..end, style)
        })
        .filter(|(range, _)| range.start < range.end)
        .collect();

    StyledText::new(output).with_highlights(highlights)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to extract the text content from rendered markdown
    /// This simulates what pulldown-cmark will parse without triggering StyledText
    fn render_text(markdown: &str) -> String {
        let mut output = String::new();
        let options = pulldown_cmark::Options::empty();
        let parser = pulldown_cmark::Parser::new_ext(markdown, options);

        for event in parser {
            match event {
                pulldown_cmark::Event::Text(text) => output.push_str(&text),
                pulldown_cmark::Event::Code(code) => {
                    output.push('`');
                    output.push_str(&code);
                    output.push('`');
                }
                pulldown_cmark::Event::SoftBreak | pulldown_cmark::Event::HardBreak => {
                    output.push('\n');
                }
                _ => {}
            }
        }
        output
    }

    /// Test that render_markdown creates a valid StyledText without panicking
    fn test_render_markdown(markdown: &str) {
        let style = MarkdownStyle::default();
        let _ = render_markdown(markdown, &style);
    }

    #[test]
    fn test_plain_text() {
        let text = render_text("Hello world");
        assert_eq!(text, "Hello world");
    }

    #[test]
    fn test_bold_text() {
        let text = render_text("Hello **bold** world");
        assert_eq!(text, "Hello bold world");
    }

    #[test]
    fn test_italic_text() {
        let text = render_text("Hello *italic* world");
        assert_eq!(text, "Hello italic world");
    }

    #[test]
    fn test_inline_code() {
        let text = render_text("Use `code` here");
        assert_eq!(text, "Use `code` here");
    }

    #[test]
    fn test_mixed_formatting() {
        let text = render_text("**bold** and *italic* and `code`");
        assert_eq!(text, "bold and italic and `code`");
    }

    #[test]
    fn test_multiline_text() {
        let text = render_text("Line 1\nLine 2");
        assert!(text.contains("Line 1"));
        assert!(text.contains("Line 2"));
    }

    #[test]
    fn test_heading() {
        let text = render_text("# Heading");
        assert_eq!(text, "Heading");
    }

    #[test]
    fn test_code_block() {
        // Code blocks in pulldown-cmark emit Text events for the content
        let markdown = "```rust\nfn main() {}\n```";
        let text = render_text(markdown);
        // Code block content should be captured
        assert!(text.contains("fn main()") || text.contains("main"));
    }

    #[test]
    fn test_list_items() {
        let markdown = "- Item 1\n- Item 2";
        let text = render_text(markdown);
        assert!(text.contains("Item 1"));
        assert!(text.contains("Item 2"));
    }

    #[test]
    fn test_empty_string() {
        let text = render_text("");
        assert_eq!(text, "");
    }

    #[test]
    fn test_special_characters() {
        let text = render_text("Test with < > & characters");
        assert!(text.contains("<"));
        assert!(text.contains(">"));
        assert!(text.contains("&"));
    }

    #[test]
    fn test_nested_formatting() {
        // Bold inside italic, etc.
        let text = render_text("***bold and italic***");
        assert!(text.contains("bold and italic"));
    }

    #[test]
    fn test_default_style() {
        let style = MarkdownStyle::default();
        // Code color should be cyan-ish
        assert!(style.code_color.s > 0.5); // saturation > 0.5
        assert_eq!(style.heading_weight, gpui::FontWeight::BOLD);
    }

    // Tests that render_markdown doesn't panic (validates highlight boundaries)
    #[test]
    fn test_render_plain_text_no_panic() {
        test_render_markdown("Hello world");
    }

    #[test]
    fn test_render_bold_no_panic() {
        test_render_markdown("Hello **bold** world");
    }

    #[test]
    fn test_render_italic_no_panic() {
        test_render_markdown("Hello *italic* world");
    }

    #[test]
    fn test_render_code_no_panic() {
        test_render_markdown("Use `code` here");
    }

    #[test]
    fn test_render_code_block_no_panic() {
        test_render_markdown("```rust\nfn main() {}\n```");
    }

    #[test]
    fn test_render_heading_no_panic() {
        test_render_markdown("# Heading\n\nParagraph");
    }

    #[test]
    fn test_render_list_no_panic() {
        test_render_markdown("- Item 1\n- Item 2\n- Item 3");
    }

    #[test]
    fn test_render_mixed_no_panic() {
        test_render_markdown("# Title\n\n**Bold** and *italic* and `code`\n\n```\nblock\n```");
    }

    #[test]
    fn test_render_empty_no_panic() {
        test_render_markdown("");
    }

    #[test]
    fn test_render_trailing_whitespace_no_panic() {
        test_render_markdown("Hello world   \n\n  ");
    }
}
