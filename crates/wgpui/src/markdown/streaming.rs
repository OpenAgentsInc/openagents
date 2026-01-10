use std::collections::VecDeque;

use super::parser::MarkdownParser;
use super::remend::remend;
use super::types::*;

#[derive(Clone, Debug)]
pub struct StreamingConfig {
    pub debounce_ms: u64,
    pub max_pending_chars: usize,
    pub fade_in_frames: Option<u32>,
}

impl Default for StreamingConfig {
    fn default() -> Self {
        Self {
            debounce_ms: 16,
            max_pending_chars: 4096,
            fade_in_frames: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct FadeState {
    pub stable_position: usize,
    pub new_content_opacity: f32,
    pub is_streaming: bool,
}

impl Default for FadeState {
    fn default() -> Self {
        Self {
            stable_position: 0,
            new_content_opacity: 1.0,
            is_streaming: false,
        }
    }
}

pub struct StreamingMarkdown {
    config: StreamingConfig,
    parser: MarkdownParser,
    source: String,
    document: MarkdownDocument,
    pending_chunks: VecDeque<String>,
    pending_chars: usize,
    frame_counter: u64,
    last_parse_frame: u64,
    needs_reparse: bool,
    fade_state: FadeState,
    last_content_frame: u64,
}

impl StreamingMarkdown {
    pub fn new() -> Self {
        Self::with_config(StreamingConfig::default())
    }

    pub fn with_config(config: StreamingConfig) -> Self {
        Self {
            config,
            parser: MarkdownParser::new(),
            source: String::new(),
            document: MarkdownDocument::new(),
            pending_chunks: VecDeque::new(),
            pending_chars: 0,
            frame_counter: 0,
            last_parse_frame: 0,
            needs_reparse: false,
            fade_state: FadeState::default(),
            last_content_frame: 0,
        }
    }

    /// Set the markdown config (font size, colors, etc.)
    pub fn set_markdown_config(&mut self, config: MarkdownConfig) {
        self.parser = MarkdownParser::with_config(config);
        // Trigger reparse with new config
        if !self.source.is_empty() {
            self.needs_reparse = true;
        }
    }

    pub fn append(&mut self, chunk: &str) {
        if chunk.is_empty() {
            return;
        }

        self.pending_chars += chunk.len();
        self.pending_chunks.push_back(chunk.to_string());
        self.needs_reparse = true;
    }

    pub fn complete(&mut self) {
        self.document.is_complete = true;
        self.drain_pending();
        self.reparse();
    }

    pub fn reset(&mut self) {
        self.source.clear();
        self.document = MarkdownDocument::new();
        self.pending_chunks.clear();
        self.pending_chars = 0;
        self.frame_counter = 0;
        self.last_parse_frame = 0;
        self.needs_reparse = false;
        self.fade_state = FadeState::default();
        self.last_content_frame = 0;
    }

    pub fn tick(&mut self) -> bool {
        self.frame_counter += 1;

        let had_pending = !self.pending_chunks.is_empty();
        let old_source_len = self.source.len();
        self.drain_pending();

        self.update_fade(old_source_len);

        if !had_pending && !self.needs_reparse {
            return false;
        }

        let frames_since_parse = self.frame_counter - self.last_parse_frame;
        let should_parse = self.needs_reparse
            && (frames_since_parse >= 1 || self.pending_chars >= self.config.max_pending_chars);

        if should_parse {
            self.reparse();
            return true;
        }

        false
    }

    fn update_fade(&mut self, old_source_len: usize) {
        let new_source_len = self.source.len();

        if new_source_len > old_source_len {
            self.last_content_frame = self.frame_counter;
            self.fade_state.is_streaming = true;
        }

        let frames_since_content = self.frame_counter.saturating_sub(self.last_content_frame);
        if frames_since_content > 3 {
            self.fade_state.is_streaming = false;
        }

        self.fade_state.stable_position = self.source.len();
        self.fade_state.new_content_opacity = 1.0;
    }

    pub fn fade_state(&self) -> &FadeState {
        &self.fade_state
    }

    pub fn document(&self) -> &MarkdownDocument {
        &self.document
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn has_pending(&self) -> bool {
        !self.pending_chunks.is_empty() || self.needs_reparse
    }

    pub fn force_reparse(&mut self) {
        self.drain_pending();
        self.reparse();
    }

    fn drain_pending(&mut self) {
        while let Some(chunk) = self.pending_chunks.pop_front() {
            self.source.push_str(&chunk);
        }
        self.pending_chars = 0;
    }

    fn reparse(&mut self) {
        let was_complete = self.document.is_complete;

        let text_to_parse = if was_complete {
            self.source.clone()
        } else {
            remend(&self.source)
        };

        self.document = self.parser.parse(&text_to_parse);
        self.document.is_complete = was_complete;
        self.last_parse_frame = self.frame_counter;
        self.needs_reparse = false;
    }
}

impl Default for StreamingMarkdown {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_streaming_basic() {
        let mut streaming = StreamingMarkdown::new();

        streaming.append("# Hello");
        streaming.tick();

        assert_eq!(streaming.source(), "# Hello");
        assert!(!streaming.document().blocks.is_empty());
    }

    #[test]
    fn test_streaming_incremental() {
        let mut streaming = StreamingMarkdown::new();

        streaming.append("# Title\n\n");
        streaming.tick();

        streaming.append("Some ");
        streaming.tick();

        streaming.append("content.");
        streaming.tick();

        streaming.complete();

        assert!(streaming.document().is_complete);
        assert_eq!(streaming.document().blocks.len(), 2);
    }

    #[test]
    fn test_streaming_reset() {
        let mut streaming = StreamingMarkdown::new();

        streaming.append("First content");
        streaming.tick();

        streaming.reset();

        assert!(streaming.source().is_empty());
        assert!(streaming.document().blocks.is_empty());
    }

    #[test]
    fn test_streaming_code_block() {
        let mut streaming = StreamingMarkdown::new();

        streaming.append("```rust\n");
        streaming.tick();

        streaming.append("fn main() {}\n");
        streaming.tick();

        streaming.append("```");
        streaming.complete();

        let has_code_block = streaming
            .document()
            .blocks
            .iter()
            .any(|b| matches!(b, MarkdownBlock::CodeBlock { .. }));
        assert!(has_code_block);
    }

    #[test]
    fn test_streaming_bold_completion() {
        let mut streaming = StreamingMarkdown::new();

        streaming.append("This is **bold");
        streaming.tick();

        let doc = streaming.document();
        match &doc.blocks[0] {
            MarkdownBlock::Paragraph(lines) => {
                let has_bold = lines[0].spans.iter().any(|s| s.style.bold);
                assert!(has_bold);
            }
            _ => panic!("Expected paragraph"),
        }
    }

    #[test]
    fn test_fade_state_initial() {
        let streaming = StreamingMarkdown::new();
        let fade = streaming.fade_state();
        assert_eq!(fade.new_content_opacity, 1.0);
        assert!(!fade.is_streaming);
    }

    #[test]
    fn test_streaming_detection() {
        let mut streaming = StreamingMarkdown::new();

        streaming.append("Test");
        streaming.tick();

        assert!(streaming.fade_state().is_streaming);

        for _ in 0..5 {
            streaming.tick();
        }

        assert!(!streaming.fade_state().is_streaming);
    }

    #[test]
    fn test_has_pending() {
        let mut streaming = StreamingMarkdown::new();

        assert!(!streaming.has_pending());

        streaming.append("Test");
        assert!(streaming.has_pending());

        streaming.tick();
        assert!(!streaming.has_pending());
    }
}
