use std::time::Instant;

use crate::autopilot_loop::DspyStage;

/// Visualization data for a DSPy pipeline stage.
pub(crate) struct DspyStageVisualization {
    pub(crate) stage: DspyStage,
    /// Index of the message this stage is associated with.
    pub(crate) message_index: usize,
    /// Timestamp when this stage was received.
    pub(crate) timestamp: Instant,
}

impl DspyStageVisualization {
    pub(crate) fn new(stage: DspyStage, message_index: usize) -> Self {
        Self {
            stage,
            message_index,
            timestamp: Instant::now(),
        }
    }
}

/// Layout for DSPy stage cards displayed inline in chat.
pub(crate) struct DspyStageLayout {
    pub(crate) message_index: usize,
    pub(crate) y_offset: f32,
    pub(crate) height: f32,
    pub(crate) stage_index: usize,
}
