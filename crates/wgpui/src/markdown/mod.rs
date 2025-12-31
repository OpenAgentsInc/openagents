mod highlighter;
mod parser;
mod remend;
mod renderer;
mod streaming;
mod types;

pub use highlighter::{SUPPORTED_LANGUAGES, SyntaxHighlighter};
pub use parser::MarkdownParser;
pub use renderer::{MarkdownRenderer, render_markdown};
pub use streaming::{FadeState, StreamingConfig, StreamingMarkdown};
pub use types::{
    MarkdownBlock, MarkdownConfig, MarkdownDocument, StyledLine, StyledSpan, TextStyle,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_reexports() {
        let _parser = MarkdownParser::new();
        let _renderer = MarkdownRenderer::new();
        let _streaming = StreamingMarkdown::new();
        let _config = MarkdownConfig::default();
        let _style = TextStyle::default();
    }

    #[test]
    fn test_full_pipeline() {
        let parser = MarkdownParser::new();
        let doc = parser.parse("# Hello\n\nThis is **bold** text.");

        assert_eq!(doc.blocks.len(), 2);
        assert!(matches!(
            doc.blocks[0],
            MarkdownBlock::Header { level: 1, .. }
        ));
        assert!(matches!(doc.blocks[1], MarkdownBlock::Paragraph(_)));
    }

    #[test]
    fn test_streaming_pipeline() {
        let mut streaming = StreamingMarkdown::new();

        streaming.append("# Title\n\n");
        streaming.tick();

        streaming.append("Some **bold");
        streaming.tick();

        let doc = streaming.document();
        assert!(!doc.blocks.is_empty());

        streaming.append("** text");
        streaming.complete();

        let final_doc = streaming.document();
        assert!(final_doc.is_complete);
    }
}
