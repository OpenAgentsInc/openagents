#[derive(Clone, Debug, Default)]
pub struct ScrollState {
    pub offset: f32,
    pub viewport: f32,
    pub content: f32,
}

impl ScrollState {
    pub fn set_viewport(&mut self, height: f32) {
        self.viewport = height.max(0.0);
    }

    pub fn set_content(&mut self, height: f32) {
        self.content = height.max(0.0);
    }

    pub fn max_offset(&self) -> f32 {
        (self.content - self.viewport).max(0.0)
    }

    pub fn scroll_by(&mut self, delta: f32) {
        let next = self.offset + delta;
        self.offset = next.clamp(0.0, self.max_offset());
    }

    pub fn clamp(&mut self) {
        self.offset = self.offset.clamp(0.0, self.max_offset());
    }

    pub fn reset(&mut self) {
        self.offset = 0.0;
    }
}
