use crate::geometry::Point;
use crate::scene::Scene;
use crate::text::TextSystem;

#[derive(Clone, Debug, Default)]
pub struct MarkdownDocument {
    pub content: String,
    pub is_complete: bool,
}

#[derive(Clone, Debug)]
pub struct StreamingConfig {
    pub fade_in_frames: Option<u32>,
}

impl Default for StreamingConfig {
    fn default() -> Self {
        Self {
            fade_in_frames: None,
        }
    }
}

pub struct StreamingMarkdown {
    document: MarkdownDocument,
    #[allow(dead_code)]
    config: StreamingConfig,
}

impl StreamingMarkdown {
    pub fn new() -> Self {
        Self::with_config(StreamingConfig::default())
    }

    pub fn with_config(config: StreamingConfig) -> Self {
        Self {
            document: MarkdownDocument::default(),
            config,
        }
    }

    pub fn append(&mut self, text: &str) {
        self.document.content.push_str(text);
    }

    pub fn complete(&mut self) {
        self.document.is_complete = true;
    }

    pub fn tick(&mut self) {}

    pub fn document(&self) -> &MarkdownDocument {
        &self.document
    }

    pub fn fade_state(&self) -> FadeState {
        FadeState {
            new_content_opacity: 1.0,
        }
    }
}

impl Default for StreamingMarkdown {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FadeState {
    pub new_content_opacity: f32,
}

pub struct MarkdownRenderer;

impl MarkdownRenderer {
    pub fn new() -> Self {
        Self
    }

    pub fn render(
        &self,
        _doc: &MarkdownDocument,
        _origin: Point,
        _width: f32,
        _text_system: &TextSystem,
        _scene: &mut Scene,
    ) {
    }

    pub fn render_with_opacity(
        &self,
        _doc: &MarkdownDocument,
        _origin: Point,
        _width: f32,
        _text_system: &TextSystem,
        _scene: &mut Scene,
        _opacity: f32,
    ) {
    }
}

impl Default for MarkdownRenderer {
    fn default() -> Self {
        Self::new()
    }
}
