use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use super::highlighter::SyntaxHighlighter;
use super::types::*;
use crate::theme;

pub struct MarkdownParser {
    config: MarkdownConfig,
    highlighter: Option<SyntaxHighlighter>,
}

impl MarkdownParser {
    pub fn new() -> Self {
        Self::with_config(MarkdownConfig::default())
    }

    pub fn with_config(config: MarkdownConfig) -> Self {
        Self {
            config,
            highlighter: SyntaxHighlighter::new().ok(),
        }
    }

    pub fn config(&self) -> &MarkdownConfig {
        &self.config
    }

    pub fn parse(&self, markdown: &str) -> MarkdownDocument {
        let mut options = Options::empty();
        options.insert(Options::ENABLE_STRIKETHROUGH);
        options.insert(Options::ENABLE_TABLES);
        options.insert(Options::ENABLE_TASKLISTS);

        let parser = Parser::new_ext(markdown, options);
        let mut builder = DocumentBuilder::new(&self.config, self.highlighter.as_ref());

        for event in parser {
            builder.process_event(event);
        }

        builder.finish(markdown.to_string(), true)
    }
}

impl Default for MarkdownParser {
    fn default() -> Self {
        Self::new()
    }
}

struct DocumentBuilder<'a> {
    config: &'a MarkdownConfig,
    highlighter: Option<&'a SyntaxHighlighter>,
    blocks: Vec<MarkdownBlock>,
    current_spans: Vec<StyledSpan>,
    style_stack: Vec<TextStyle>,
    list_stack: Vec<ListContext>,
    blockquote_stack: Vec<BlockquoteContext>,
    in_code_block: bool,
    code_language: Option<String>,
    code_content: String,
    current_header_level: Option<u8>,
}

struct ListContext {
    ordered: bool,
    start: u64,
    items: Vec<Vec<MarkdownBlock>>,
    current_item_blocks: Vec<MarkdownBlock>,
}

struct BlockquoteContext {
    blocks: Vec<MarkdownBlock>,
}

impl<'a> DocumentBuilder<'a> {
    fn new(config: &'a MarkdownConfig, highlighter: Option<&'a SyntaxHighlighter>) -> Self {
        let base_style = TextStyle {
            color: config.text_color,
            font_size: config.base_font_size,
            ..Default::default()
        };

        Self {
            config,
            highlighter,
            blocks: Vec::new(),
            current_spans: Vec::new(),
            style_stack: vec![base_style],
            list_stack: Vec::new(),
            blockquote_stack: Vec::new(),
            in_code_block: false,
            code_language: None,
            code_content: String::new(),
            current_header_level: None,
        }
    }

    fn process_event(&mut self, event: Event) {
        match event {
            Event::Start(tag) => self.start_tag(tag),
            Event::End(tag) => self.end_tag(tag),
            Event::Text(text) => self.push_text(&text),
            Event::Code(code) => self.push_inline_code(&code),
            Event::SoftBreak => self.push_soft_break(),
            Event::HardBreak => self.push_hard_break(),
            Event::Rule => self.push_horizontal_rule(),
            Event::Html(html) => self.push_text(&html),
            Event::FootnoteReference(_) => {}
            Event::TaskListMarker(checked) => self.push_task_marker(checked),
            Event::InlineHtml(html) => self.push_text(&html),
            Event::InlineMath(math) => self.push_inline_code(&math),
            Event::DisplayMath(math) => self.push_text(&math),
        }
    }

    fn start_tag(&mut self, tag: Tag) {
        match tag {
            Tag::Heading { level, .. } => {
                let level_idx = match level {
                    HeadingLevel::H1 => 0,
                    HeadingLevel::H2 => 1,
                    HeadingLevel::H3 => 2,
                    HeadingLevel::H4 => 3,
                    HeadingLevel::H5 => 4,
                    HeadingLevel::H6 => 5,
                };
                let size = self.config.base_font_size * self.config.header_sizes[level_idx];
                self.current_header_level = Some(level_idx as u8 + 1);
                self.push_style(TextStyle {
                    color: self.config.header_color,
                    font_size: size,
                    bold: true,
                    ..Default::default()
                });
            }
            Tag::CodeBlock(kind) => {
                self.in_code_block = true;
                self.code_language = match kind {
                    CodeBlockKind::Fenced(lang) if !lang.is_empty() => Some(lang.to_string()),
                    _ => None,
                };
                self.code_content.clear();
            }
            Tag::Emphasis => {
                self.modify_style(|s| s.italic = true);
            }
            Tag::Strong => {
                self.modify_style(|s| s.bold = true);
            }
            Tag::Strikethrough => {
                self.modify_style(|s| s.strikethrough = true);
            }
            Tag::Link { .. } => {
                self.modify_style(|s| s.color = self.config.link_color);
            }
            Tag::List(start) => {
                self.flush_paragraph();
                self.list_stack.push(ListContext {
                    ordered: start.is_some(),
                    start: start.unwrap_or(1),
                    items: Vec::new(),
                    current_item_blocks: Vec::new(),
                });
            }
            Tag::Item => {}
            Tag::BlockQuote(_) => {
                self.flush_paragraph();
                self.blockquote_stack
                    .push(BlockquoteContext { blocks: Vec::new() });
            }
            Tag::Paragraph => {}
            Tag::Table(_) => {
                self.flush_paragraph();
            }
            Tag::TableHead | Tag::TableRow | Tag::TableCell => {}
            Tag::Image { .. } => {}
            Tag::FootnoteDefinition(_) => {}
            Tag::MetadataBlock(_) => {}
            Tag::DefinitionList | Tag::DefinitionListTitle | Tag::DefinitionListDefinition => {}
            Tag::HtmlBlock => {}
        }
    }

    fn end_tag(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Heading(_) => {
                self.pop_style();
                self.finish_header();
            }
            TagEnd::CodeBlock => {
                self.finish_code_block();
            }
            TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough | TagEnd::Link => {
                self.pop_style();
            }
            TagEnd::Paragraph => {
                self.finish_paragraph();
            }
            TagEnd::List(_) => {
                self.finish_list();
            }
            TagEnd::Item => {
                self.finish_list_item();
            }
            TagEnd::BlockQuote(_) => {
                self.finish_blockquote();
            }
            TagEnd::Table => {}
            TagEnd::TableHead | TagEnd::TableRow | TagEnd::TableCell => {}
            TagEnd::Image => {}
            TagEnd::FootnoteDefinition => {}
            TagEnd::MetadataBlock(_) => {}
            TagEnd::DefinitionList
            | TagEnd::DefinitionListTitle
            | TagEnd::DefinitionListDefinition => {}
            TagEnd::HtmlBlock => {}
        }
    }

    fn push_text(&mut self, text: &str) {
        if self.in_code_block {
            self.code_content.push_str(text);
        } else {
            self.current_spans.push(StyledSpan::new(
                text.to_string(),
                self.current_style().clone(),
            ));
        }
    }

    fn push_inline_code(&mut self, code: &str) {
        let mut style = self.current_style().clone();
        style.monospace = true;
        style.background = Some(self.config.inline_code_background);
        self.current_spans
            .push(StyledSpan::new(code.to_string(), style));
    }

    fn push_soft_break(&mut self) {
        if !self.in_code_block {
            self.current_spans.push(StyledSpan::new(
                " ".to_string(),
                self.current_style().clone(),
            ));
        }
    }

    fn push_hard_break(&mut self) {
        self.push_soft_break();
    }

    fn push_horizontal_rule(&mut self) {
        self.flush_paragraph();
        self.add_block(MarkdownBlock::HorizontalRule);
    }

    fn push_task_marker(&mut self, checked: bool) {
        let marker = if checked { "[x] " } else { "[ ] " };
        let style = TextStyle {
            color: if checked {
                theme::status::SUCCESS
            } else {
                theme::text::MUTED
            },
            ..self.current_style().clone()
        };
        self.current_spans
            .push(StyledSpan::new(marker.to_string(), style));
    }

    fn finish_code_block(&mut self) {
        let lines = if let Some(ref lang) = self.code_language {
            if let Some(highlighter) = self.highlighter {
                highlighter.highlight(&self.code_content, lang, self.config)
            } else {
                self.plain_code_lines(&self.code_content)
            }
        } else {
            self.plain_code_lines(&self.code_content)
        };

        let language = self.code_language.take();

        self.add_block(MarkdownBlock::CodeBlock {
            language,
            lines,
            background_bounds: None,
        });

        self.in_code_block = false;
        self.code_content.clear();
    }

    fn plain_code_lines(&self, code: &str) -> Vec<StyledLine> {
        code.lines()
            .map(|line| {
                StyledLine::from_span(StyledSpan::new(
                    line.to_string(),
                    TextStyle {
                        color: self.config.text_color,
                        font_size: self.config.base_font_size,
                        monospace: true,
                        ..Default::default()
                    },
                ))
                .with_line_height(theme::line_height::NORMAL)
            })
            .collect()
    }

    fn finish_header(&mut self) {
        if self.current_spans.is_empty() {
            self.current_header_level = None;
            return;
        }

        let lines = vec![StyledLine {
            spans: std::mem::take(&mut self.current_spans),
            line_height: theme::line_height::TIGHT,
            margin_top: 0.0,
            indent: 0,
        }];

        let level = self.current_header_level.take().unwrap_or(1);
        self.add_block(MarkdownBlock::Header { level, lines });
    }

    fn finish_paragraph(&mut self) {
        if self.current_spans.is_empty() {
            return;
        }

        let lines = vec![StyledLine {
            spans: std::mem::take(&mut self.current_spans),
            line_height: theme::line_height::RELAXED,
            margin_top: 0.0,
            indent: 0,
        }];

        self.add_block(MarkdownBlock::Paragraph(lines));
    }

    fn flush_paragraph(&mut self) {
        self.finish_paragraph();
    }

    fn finish_list_item(&mut self) {
        if !self.current_spans.is_empty() {
            let lines = vec![StyledLine {
                spans: std::mem::take(&mut self.current_spans),
                line_height: theme::line_height::RELAXED,
                margin_top: 0.0,
                indent: 0,
            }];
            if let Some(list_ctx) = self.list_stack.last_mut() {
                list_ctx
                    .current_item_blocks
                    .push(MarkdownBlock::Paragraph(lines));
            }
        }

        if let Some(list_ctx) = self.list_stack.last_mut() {
            let item_blocks = std::mem::take(&mut list_ctx.current_item_blocks);
            list_ctx.items.push(item_blocks);
        }
    }

    fn finish_list(&mut self) {
        if let Some(list_ctx) = self.list_stack.pop() {
            let block = if list_ctx.ordered {
                MarkdownBlock::OrderedList {
                    start: list_ctx.start,
                    items: list_ctx.items,
                }
            } else {
                MarkdownBlock::UnorderedList(list_ctx.items)
            };
            self.add_block(block);
        }
    }

    fn finish_blockquote(&mut self) {
        if !self.current_spans.is_empty() {
            let lines = vec![StyledLine {
                spans: std::mem::take(&mut self.current_spans),
                line_height: theme::line_height::RELAXED,
                margin_top: 0.0,
                indent: 0,
            }];
            if let Some(bq_ctx) = self.blockquote_stack.last_mut() {
                bq_ctx.blocks.push(MarkdownBlock::Paragraph(lines));
            }
        }

        if let Some(bq_ctx) = self.blockquote_stack.pop() {
            self.add_block(MarkdownBlock::Blockquote(bq_ctx.blocks));
        }
    }

    fn add_block(&mut self, block: MarkdownBlock) {
        if let Some(list_ctx) = self.list_stack.last_mut() {
            list_ctx.current_item_blocks.push(block);
        } else if let Some(bq_ctx) = self.blockquote_stack.last_mut() {
            bq_ctx.blocks.push(block);
        } else {
            self.blocks.push(block);
        }
    }

    fn finish(mut self, source: String, is_complete: bool) -> MarkdownDocument {
        self.flush_paragraph();

        MarkdownDocument {
            blocks: self.blocks,
            total_height: 0.0,
            source,
            is_complete,
        }
    }

    fn push_style(&mut self, style: TextStyle) {
        self.style_stack.push(style);
    }

    fn pop_style(&mut self) {
        if self.style_stack.len() > 1 {
            self.style_stack.pop();
        }
    }

    fn modify_style<F: FnOnce(&mut TextStyle)>(&mut self, f: F) {
        let mut style = self.current_style().clone();
        f(&mut style);
        self.push_style(style);
    }

    fn current_style(&self) -> &TextStyle {
        self.style_stack.last().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_paragraph() {
        let parser = MarkdownParser::new();
        let doc = parser.parse("Hello, world!");

        assert_eq!(doc.blocks.len(), 1);
        match &doc.blocks[0] {
            MarkdownBlock::Paragraph(lines) => {
                assert_eq!(lines.len(), 1);
                assert_eq!(lines[0].spans.len(), 1);
                assert_eq!(lines[0].spans[0].text, "Hello, world!");
            }
            _ => panic!("Expected paragraph"),
        }
    }

    #[test]
    fn test_parse_header() {
        let parser = MarkdownParser::new();
        let doc = parser.parse("# Hello");

        assert_eq!(doc.blocks.len(), 1);
        match &doc.blocks[0] {
            MarkdownBlock::Header { level, lines } => {
                assert_eq!(*level, 1);
                assert_eq!(lines[0].spans[0].text, "Hello");
            }
            _ => panic!("Expected header"),
        }
    }

    #[test]
    fn test_parse_code_block() {
        let parser = MarkdownParser::new();
        let doc = parser.parse("```rust\nfn main() {}\n```");

        assert_eq!(doc.blocks.len(), 1);
        match &doc.blocks[0] {
            MarkdownBlock::CodeBlock {
                language, lines, ..
            } => {
                assert_eq!(language.as_deref(), Some("rust"));
                assert!(!lines.is_empty());
            }
            _ => panic!("Expected code block"),
        }
    }

    #[test]
    fn test_parse_list() {
        let parser = MarkdownParser::new();
        let doc = parser.parse("- Item 1\n- Item 2");

        assert_eq!(doc.blocks.len(), 1);
        match &doc.blocks[0] {
            MarkdownBlock::UnorderedList(items) => {
                assert_eq!(items.len(), 2);
            }
            _ => panic!("Expected unordered list"),
        }
    }

    #[test]
    fn test_parse_blockquote() {
        let parser = MarkdownParser::new();
        let doc = parser.parse("> This is a quote");

        assert_eq!(doc.blocks.len(), 1);
        match &doc.blocks[0] {
            MarkdownBlock::Blockquote(blocks) => {
                assert_eq!(blocks.len(), 1);
                match &blocks[0] {
                    MarkdownBlock::Paragraph(lines) => {
                        assert_eq!(lines[0].spans[0].text, "This is a quote");
                    }
                    _ => panic!("Expected paragraph inside blockquote"),
                }
            }
            _ => panic!("Expected blockquote, got {:?}", doc.blocks[0]),
        }
    }

    #[test]
    fn test_parse_bold() {
        let parser = MarkdownParser::new();
        let doc = parser.parse("This is **bold** text");

        match &doc.blocks[0] {
            MarkdownBlock::Paragraph(lines) => {
                let bold_span = lines[0].spans.iter().find(|s| s.style.bold);
                assert!(bold_span.is_some());
                assert_eq!(bold_span.unwrap().text, "bold");
            }
            _ => panic!("Expected paragraph"),
        }
    }

    #[test]
    fn test_parse_italic() {
        let parser = MarkdownParser::new();
        let doc = parser.parse("This is *italic* text");

        match &doc.blocks[0] {
            MarkdownBlock::Paragraph(lines) => {
                let italic_span = lines[0].spans.iter().find(|s| s.style.italic);
                assert!(italic_span.is_some());
                assert_eq!(italic_span.unwrap().text, "italic");
            }
            _ => panic!("Expected paragraph"),
        }
    }

    #[test]
    fn test_parse_inline_code() {
        let parser = MarkdownParser::new();
        let doc = parser.parse("Use `code` here");

        match &doc.blocks[0] {
            MarkdownBlock::Paragraph(lines) => {
                let code_span = lines[0].spans.iter().find(|s| s.style.monospace);
                assert!(code_span.is_some());
                assert_eq!(code_span.unwrap().text, "code");
            }
            _ => panic!("Expected paragraph"),
        }
    }
}
