//! Streaming markdown support for incremental rendering.

use std::collections::VecDeque;

use super::parser::MarkdownParser;
use super::remend::remend;
use super::types::*;

/// Configuration for streaming markdown.
#[derive(Clone, Debug)]
pub struct StreamingConfig {
    /// Minimum time between re-parses in milliseconds.
    pub debounce_ms: u64,
    /// Maximum characters to accumulate before forcing a parse.
    pub max_pending_chars: usize,
    /// Number of frames over which new content fades in (None = instant).
    pub fade_in_frames: Option<u32>,
}

impl Default for StreamingConfig {
    fn default() -> Self {
        Self {
            debounce_ms: 16, // ~60fps
            max_pending_chars: 4096,
            fade_in_frames: None, // Instant by default
        }
    }
}

/// State for fade-in animation of new content.
#[derive(Clone, Debug)]
pub struct FadeState {
    /// Source position where fully-visible content ends.
    pub stable_position: usize,
    /// Current opacity for content after stable_position (0.0 to 1.0).
    pub new_content_opacity: f32,
    /// Whether content is actively streaming (received in last few frames).
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

/// Streaming markdown renderer that handles incremental text input.
pub struct StreamingMarkdown {
    config: StreamingConfig,
    parser: MarkdownParser,

    /// Current accumulated source text
    source: String,
    /// Parsed document (may be partial)
    document: MarkdownDocument,

    /// Pending text chunks not yet added to source
    pending_chunks: VecDeque<String>,
    /// Total pending characters
    pending_chars: usize,

    /// Frame counter for debouncing (since we can't use Instant in WASM easily)
    frame_counter: u64,
    /// Last parse frame
    last_parse_frame: u64,

    /// Whether a re-parse is needed
    needs_reparse: bool,

    /// Fade-in animation state
    fade_state: FadeState,
    /// Frame when fade started for current batch
    fade_start_frame: u64,
    /// Target position for current fade (source length when fade started)
    fade_target_position: usize,
    /// Last frame when content was received
    last_content_frame: u64,
}

impl StreamingMarkdown {
    /// Create a new streaming markdown instance.
    pub fn new() -> Self {
        Self::with_config(StreamingConfig::default())
    }

    /// Create a new streaming markdown instance with custom configuration.
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
            fade_start_frame: 0,
            fade_target_position: 0,
            last_content_frame: 0,
        }
    }

    /// Append a text chunk from a streaming source.
    pub fn append(&mut self, chunk: &str) {
        if chunk.is_empty() {
            return;
        }

        self.pending_chars += chunk.len();
        self.pending_chunks.push_back(chunk.to_string());
        self.needs_reparse = true;
    }

    /// Mark the stream as complete.
    pub fn complete(&mut self) {
        self.document.is_complete = true;
        // Force immediate parse on completion
        self.drain_pending();
        self.reparse();
    }

    /// Reset for new content.
    pub fn reset(&mut self) {
        self.source.clear();
        self.document = MarkdownDocument::new();
        self.pending_chunks.clear();
        self.pending_chars = 0;
        self.frame_counter = 0;
        self.last_parse_frame = 0;
        self.needs_reparse = false;
        self.fade_state = FadeState::default();
        self.fade_start_frame = 0;
        self.fade_target_position = 0;
        self.last_content_frame = 0;
    }

    /// Process pending chunks and update document.
    ///
    /// Call this once per frame. Returns true if the document was updated.
    pub fn tick(&mut self) -> bool {
        self.frame_counter += 1;

        // Drain pending chunks into source
        let had_pending = !self.pending_chunks.is_empty();
        let old_source_len = self.source.len();
        self.drain_pending();

        // Update fade animation
        self.update_fade(old_source_len);

        if !had_pending && !self.needs_reparse {
            return false;
        }

        // Check debounce (roughly 1 frame = 16ms at 60fps)
        let frames_since_parse = self.frame_counter - self.last_parse_frame;
        let should_parse = self.needs_reparse
            && (frames_since_parse >= 1 || self.pending_chars >= self.config.max_pending_chars);

        if should_parse {
            self.reparse();
            return true;
        }

        false
    }

    /// Update fade-in animation state.
    fn update_fade(&mut self, old_source_len: usize) {
        let Some(fade_frames) = self.config.fade_in_frames else {
            // No fade - everything is immediately visible
            self.fade_state.stable_position = self.source.len();
            self.fade_state.new_content_opacity = 1.0;
            self.fade_state.is_streaming = false;
            return;
        };

        let new_source_len = self.source.len();

        // Track when content arrives
        if new_source_len > old_source_len {
            self.last_content_frame = self.frame_counter;
            self.fade_state.is_streaming = true;

            // When new content arrives after a pause, start a fade
            if self.fade_state.new_content_opacity >= 1.0 {
                // Start fade-in from 0 for a more visible effect
                self.fade_start_frame = self.frame_counter;
                self.fade_state.new_content_opacity = 0.0;
            }
            self.fade_target_position = new_source_len;
        }

        // Check if streaming has paused (no content in last 3 frames)
        let frames_since_content = self.frame_counter.saturating_sub(self.last_content_frame);
        if frames_since_content > 3 {
            self.fade_state.is_streaming = false;
        }

        // Advance fade animation
        if self.fade_state.new_content_opacity < 1.0 {
            let frames_elapsed = self.frame_counter - self.fade_start_frame;
            let progress = (frames_elapsed as f32 / fade_frames as f32).min(1.0);

            // Smooth ease-out from 0 to 1
            self.fade_state.new_content_opacity = ease_out(progress);

            // When fade completes
            if progress >= 1.0 {
                self.fade_state.stable_position = self.source.len();
                self.fade_state.new_content_opacity = 1.0;
            }
        }
    }

    /// Get the current fade state for rendering.
    pub fn fade_state(&self) -> &FadeState {
        &self.fade_state
    }

    /// Get the current document (may be partial if streaming).
    pub fn document(&self) -> &MarkdownDocument {
        &self.document
    }

    /// Get the accumulated source text.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Check if there's pending content to process.
    pub fn has_pending(&self) -> bool {
        !self.pending_chunks.is_empty() || self.needs_reparse
    }

    /// Force an immediate reparse (bypasses debouncing).
    pub fn force_reparse(&mut self) {
        self.drain_pending();
        self.reparse();
    }

    /// Drain pending chunks into source.
    fn drain_pending(&mut self) {
        while let Some(chunk) = self.pending_chunks.pop_front() {
            self.source.push_str(&chunk);
        }
        self.pending_chars = 0;
    }

    /// Reparse the entire source.
    fn reparse(&mut self) {
        let was_complete = self.document.is_complete;

        // Apply remend preprocessing when streaming to complete incomplete markers
        // This makes **bold show as bold even before the closing ** arrives
        let text_to_parse = if was_complete {
            // Stream is complete - parse original source as-is
            self.source.clone()
        } else {
            // Still streaming - complete incomplete markers
            remend(&self.source)
        };

        self.document = self.parser.parse(&text_to_parse);
        self.document.is_complete = was_complete;
        self.last_parse_frame = self.frame_counter;
        self.needs_reparse = false;
    }
}

/// Ease-out function for smooth fade animation (quadratic ease-out).
fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t) * (1.0 - t)
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
        assert_eq!(streaming.document().blocks.len(), 2); // Header + paragraph
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

        // Should have a code block
        let has_code_block = streaming.document().blocks.iter().any(|b| {
            matches!(b, MarkdownBlock::CodeBlock { .. })
        });
        assert!(has_code_block);
    }
}
