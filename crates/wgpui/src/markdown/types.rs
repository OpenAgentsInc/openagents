use crate::color::Hsla;
use crate::geometry::{Bounds, Size};
use crate::theme;

#[derive(Clone, Debug)]
pub struct TextStyle {
    pub color: Hsla,
    pub font_size: f32,
    pub bold: bool,
    pub italic: bool,
    pub strikethrough: bool,
    pub monospace: bool,
    pub background: Option<Hsla>,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            color: theme::text::PRIMARY,
            font_size: theme::font_size::BASE,
            bold: false,
            italic: false,
            strikethrough: false,
            monospace: false,
            background: None,
        }
    }
}

impl TextStyle {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_color(mut self, color: Hsla) -> Self {
        self.color = color;
        self
    }

    pub fn with_font_size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    pub fn with_bold(mut self, bold: bool) -> Self {
        self.bold = bold;
        self
    }

    pub fn with_italic(mut self, italic: bool) -> Self {
        self.italic = italic;
        self
    }

    pub fn with_strikethrough(mut self, strikethrough: bool) -> Self {
        self.strikethrough = strikethrough;
        self
    }

    pub fn with_monospace(mut self, monospace: bool) -> Self {
        self.monospace = monospace;
        self
    }

    pub fn with_background(mut self, background: Hsla) -> Self {
        self.background = Some(background);
        self
    }
}

#[derive(Clone, Debug)]
pub struct StyledSpan {
    pub text: String,
    pub style: TextStyle,
    pub source_range: Option<std::ops::Range<usize>>,
}

impl StyledSpan {
    pub fn new(text: impl Into<String>, style: TextStyle) -> Self {
        Self {
            text: text.into(),
            style,
            source_range: None,
        }
    }

    pub fn with_source_range(mut self, range: std::ops::Range<usize>) -> Self {
        self.source_range = Some(range);
        self
    }
}

#[derive(Clone, Debug)]
pub struct StyledLine {
    pub spans: Vec<StyledSpan>,
    pub line_height: f32,
    pub margin_top: f32,
    pub indent: u32,
}

impl Default for StyledLine {
    fn default() -> Self {
        Self {
            spans: Vec::new(),
            line_height: theme::line_height::RELAXED,
            margin_top: 0.0,
            indent: 0,
        }
    }
}

impl StyledLine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_span(span: StyledSpan) -> Self {
        Self {
            spans: vec![span],
            ..Default::default()
        }
    }

    pub fn push(&mut self, span: StyledSpan) {
        self.spans.push(span);
    }

    pub fn with_line_height(mut self, height: f32) -> Self {
        self.line_height = height;
        self
    }

    pub fn with_margin_top(mut self, margin: f32) -> Self {
        self.margin_top = margin;
        self
    }

    pub fn with_indent(mut self, indent: u32) -> Self {
        self.indent = indent;
        self
    }
}

#[derive(Clone, Debug)]
pub enum MarkdownBlock {
    Paragraph(Vec<StyledLine>),
    Header {
        level: u8,
        lines: Vec<StyledLine>,
    },
    CodeBlock {
        language: Option<String>,
        lines: Vec<StyledLine>,
        background_bounds: Option<Bounds>,
    },
    Blockquote(Vec<MarkdownBlock>),
    UnorderedList(Vec<Vec<MarkdownBlock>>),
    OrderedList {
        start: u64,
        items: Vec<Vec<MarkdownBlock>>,
    },
    HorizontalRule,
    Table {
        headers: Vec<Vec<StyledLine>>,
        rows: Vec<Vec<Vec<StyledLine>>>,
    },
}

#[derive(Clone, Debug, Default)]
pub struct MarkdownDocument {
    pub blocks: Vec<MarkdownBlock>,
    pub total_height: f32,
    pub source: String,
    pub is_complete: bool,
}

impl MarkdownDocument {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_blocks(blocks: Vec<MarkdownBlock>) -> Self {
        Self {
            blocks,
            total_height: 0.0,
            source: String::new(),
            is_complete: true,
        }
    }

    pub fn push(&mut self, block: MarkdownBlock) {
        self.blocks.push(block);
    }

    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }
}

#[derive(Clone, Debug)]
pub struct MarkdownConfig {
    pub base_font_size: f32,
    pub header_sizes: [f32; 6],
    pub text_color: Hsla,
    pub code_background: Hsla,
    pub inline_code_background: Hsla,
    pub link_color: Hsla,
    pub header_color: Hsla,
    pub blockquote_color: Hsla,
    pub max_width: Option<f32>,
}

#[derive(Clone, Debug)]
pub struct CodeBlockLayout {
    pub bounds: Bounds,
    pub header_bounds: Bounds,
    pub content_bounds: Bounds,
    pub language: Option<String>,
    pub code: String,
    pub copy_bounds: Option<Bounds>,
}

#[derive(Clone, Debug, Default)]
pub struct MarkdownLayout {
    pub size: Size,
    pub code_blocks: Vec<CodeBlockLayout>,
}

impl Default for MarkdownConfig {
    fn default() -> Self {
        Self {
            base_font_size: theme::font_size::BASE,
            header_sizes: [1.8, 1.5, 1.3, 1.15, 1.1, 1.0],
            text_color: theme::text::PRIMARY,
            code_background: theme::bg::CODE,
            inline_code_background: Hsla::new(0.0, 0.0, 0.12, 1.0),
            link_color: theme::accent::BLUE,
            header_color: theme::text::PRIMARY,
            blockquote_color: theme::accent::PRIMARY,
            max_width: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_style_default() {
        let style = TextStyle::default();
        assert!(!style.bold);
        assert!(!style.italic);
        assert!(!style.monospace);
    }

    #[test]
    fn test_text_style_builder() {
        let style = TextStyle::new()
            .with_bold(true)
            .with_font_size(20.0)
            .with_color(theme::accent::BLUE);
        assert!(style.bold);
        assert_eq!(style.font_size, 20.0);
    }

    #[test]
    fn test_styled_span_new() {
        let span = StyledSpan::new("test", TextStyle::default());
        assert_eq!(span.text, "test");
        assert!(span.source_range.is_none());
    }

    #[test]
    fn test_styled_line_from_span() {
        let span = StyledSpan::new("text", TextStyle::default());
        let line = StyledLine::from_span(span);
        assert_eq!(line.spans.len(), 1);
    }

    #[test]
    fn test_markdown_document_new() {
        let doc = MarkdownDocument::new();
        assert!(doc.is_empty());
        assert!(!doc.is_complete);
    }

    #[test]
    fn test_markdown_config_default() {
        let config = MarkdownConfig::default();
        assert_eq!(config.header_sizes.len(), 6);
        assert!(config.max_width.is_none());
    }
}
