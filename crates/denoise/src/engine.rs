/// Stub engine that passes audio through without denoising
/// (ONNX models are not included in this build)

#[allow(dead_code)]
pub const BLOCK_LEN: usize = 512;
pub const BLOCK_SHIFT: usize = 128;

pub struct Engine;

impl Engine {
    pub fn new() -> Self {
        Self
    }

    /// Pass-through stub: returns input unchanged
    pub fn feed(&mut self, sub_block: &[f32; BLOCK_SHIFT]) -> [f32; BLOCK_SHIFT] {
        *sub_block
    }
}
