//! Core types for markdown rendering.

use crate::color::Hsla;
use crate::geometry::Bounds;
use crate::theme;

/// Style applied to a text span.
#[derive(Clone, Debug)]
pub struct TextStyle {
    /// Text color
    pub color: Hsla,
    /// Font size in logical pixels
    pub font_size: f32,
    /// Whether text is bold
    pub bold: bool,
    /// Whether text is italic
    pub italic: bool,
    /// Whether text has strikethrough
    pub strikethrough: bool,
    /// Whether this is monospace/code text
    pub monospace: bool,
    /// Background color (for inline code, highlights)
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
    /// Create a new text style with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the color.
    pub fn with_color(mut self, color: Hsla) -> Self {
        self.color = color;
        self
    }

    /// Set the font size.
    pub fn with_font_size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    /// Set bold.
    pub fn with_bold(mut self, bold: bool) -> Self {
        self.bold = bold;
        self
    }

    /// Set italic.
    pub fn with_italic(mut self, italic: bool) -> Self {
        self.italic = italic;
        self
    }

    /// Set strikethrough.
    pub fn with_strikethrough(mut self, strikethrough: bool) -> Self {
        self.strikethrough = strikethrough;
        self
    }

    /// Set monospace.
    pub fn with_monospace(mut self, monospace: bool) -> Self {
        self.monospace = monospace;
        self
    }

    /// Set background color.
    pub fn with_background(mut self, background: Hsla) -> Self {
        self.background = Some(background);
        self
    }
}

/// A styled span of text.
#[derive(Clone, Debug)]
pub struct StyledSpan {
    /// The text content
    pub text: String,
    /// Style for this span
    pub style: TextStyle,
    /// Source byte range in original markdown (for interactivity)
    pub source_range: Option<std::ops::Range<usize>>,
}

impl StyledSpan {
    /// Create a new styled span.
    pub fn new(text: impl Into<String>, style: TextStyle) -> Self {
        Self {
            text: text.into(),
            style,
            source_range: None,
        }
    }

    /// Set the source range.
    pub fn with_source_range(mut self, range: std::ops::Range<usize>) -> Self {
        self.source_range = Some(range);
        self
    }
}

/// A line of styled text (multiple spans).
#[derive(Clone, Debug)]
pub struct StyledLine {
    /// Spans in this line
    pub spans: Vec<StyledSpan>,
    /// Line height multiplier
    pub line_height: f32,
    /// Additional vertical spacing before this line
    pub margin_top: f32,
    /// Indentation level (for lists, blockquotes)
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
    /// Create a new styled line.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a line with a single span.
    pub fn from_span(span: StyledSpan) -> Self {
        Self {
            spans: vec![span],
            ..Default::default()
        }
    }

    /// Add a span to this line.
    pub fn push(&mut self, span: StyledSpan) {
        self.spans.push(span);
    }

    /// Set line height.
    pub fn with_line_height(mut self, height: f32) -> Self {
        self.line_height = height;
        self
    }

    /// Set margin top.
    pub fn with_margin_top(mut self, margin: f32) -> Self {
        self.margin_top = margin;
        self
    }

    /// Set indent level.
    pub fn with_indent(mut self, indent: u32) -> Self {
        self.indent = indent;
        self
    }
}

/// A block element in rendered markdown.
#[derive(Clone, Debug)]
pub enum MarkdownBlock {
    /// Paragraph of styled text
    Paragraph(Vec<StyledLine>),

    /// Header with level (1-6)
    Header { level: u8, lines: Vec<StyledLine> },

    /// Code block with optional language
    CodeBlock {
        language: Option<String>,
        lines: Vec<StyledLine>,
        /// Background bounds (computed during layout)
        background_bounds: Option<Bounds>,
    },

    /// Blockquote (nested blocks)
    Blockquote(Vec<MarkdownBlock>),

    /// Unordered list (bullet points)
    UnorderedList(Vec<Vec<MarkdownBlock>>),

    /// Ordered list with start number
    OrderedList {
        start: u64,
        items: Vec<Vec<MarkdownBlock>>,
    },

    /// Horizontal rule
    HorizontalRule,

    /// Table with headers and rows
    Table {
        headers: Vec<Vec<StyledLine>>,
        rows: Vec<Vec<Vec<StyledLine>>>,
    },
}

/// Complete parsed markdown document.
#[derive(Clone, Debug, Default)]
pub struct MarkdownDocument {
    /// Blocks in the document
    pub blocks: Vec<MarkdownBlock>,
    /// Total height after layout (computed)
    pub total_height: f32,
    /// Original source text
    pub source: String,
    /// Whether document is complete (for streaming)
    pub is_complete: bool,
}

impl MarkdownDocument {
    /// Create a new empty document.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a document from blocks.
    pub fn from_blocks(blocks: Vec<MarkdownBlock>) -> Self {
        Self {
            blocks,
            total_height: 0.0,
            source: String::new(),
            is_complete: true,
        }
    }

    /// Add a block to the document.
    pub fn push(&mut self, block: MarkdownBlock) {
        self.blocks.push(block);
    }

    /// Check if document is empty.
    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }
}

/// Configuration for markdown parsing and rendering.
#[derive(Clone, Debug)]
pub struct MarkdownConfig {
    /// Base font size
    pub base_font_size: f32,
    /// Font size multipliers for headers (H1-H6)
    pub header_sizes: [f32; 6],
    /// Default text color
    pub text_color: Hsla,
    /// Code block background color
    pub code_background: Hsla,
    /// Inline code background
    pub inline_code_background: Hsla,
    /// Link color
    pub link_color: Hsla,
    /// Header color
    pub header_color: Hsla,
    /// Blockquote border color
    pub blockquote_color: Hsla,
    /// Maximum width for text wrapping (None = no wrap)
    pub max_width: Option<f32>,
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
