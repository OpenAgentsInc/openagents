//! Startup animation sequence

use autopilot_core::{AgentModel, LogStatus, StartupPhase, StartupState, wrap_text};
use std::time::Instant;
use wgpui::{
    Bounds, Component, EventContext, EventResult, Hsla, InputEvent, PaintContext, Point,
    components::hud::{CornerConfig, DrawDirection, Frame, FrameAnimation},
};

/// Animated startup sequence before revealing the full shell
pub struct StartupSequence {
    start_time: Instant,
    startup_state: StartupState,
    complete: bool,
}

impl StartupSequence {
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            startup_state: StartupState::with_model(AgentModel::Sonnet),
            complete: false,
        }
    }

    pub fn is_complete(&self) -> bool {
        self.complete
    }

    fn ease_out_cubic(t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        1.0 - (1.0 - t).powi(3)
    }
}

impl Default for StartupSequence {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for StartupSequence {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let elapsed = self.start_time.elapsed().as_secs_f32();

        // Frame animation starts at 0.8s and completes by 1.8s
        let frame_progress = Self::ease_out_cubic(((elapsed - 0.8) / 1.0).clamp(0.0, 1.0));

        // Tick startup state after frame is mostly visible
        if frame_progress > 0.7 {
            let startup_elapsed = elapsed - 1.8;
            if startup_elapsed > 0.0 {
                self.startup_state.tick(startup_elapsed);
            }
        }

        // Check if startup is done (phase is Complete + some buffer time)
        if self.startup_state.phase == StartupPhase::Complete && elapsed > 4.0 {
            self.complete = true;
            return;
        }

        // Draw centered frame
        if frame_progress > 0.0 {
            let frame_w = 1000.0;
            let frame_h = 600.0;
            let frame_x = (bounds.size.width - frame_w) / 2.0;
            let frame_y = (bounds.size.height - frame_h) / 2.0;

            let line_color = Hsla::new(0.0, 0.0, 0.7, frame_progress);
            let bg_color = Hsla::new(0.0, 0.0, 0.05, 0.95 * frame_progress);

            let mut frame = Frame::nefrex()
                .line_color(line_color)
                .bg_color(bg_color)
                .stroke_width(1.5)
                .corner_config(CornerConfig::all())
                .square_size(8.0)
                .small_line_length(8.0)
                .large_line_length(30.0)
                .animation_mode(FrameAnimation::Assemble)
                .draw_direction(DrawDirection::CenterOut)
                .animation_progress(frame_progress);

            frame.paint(
                Bounds::new(
                    frame_x + bounds.origin.x,
                    frame_y + bounds.origin.y,
                    frame_w,
                    frame_h,
                ),
                cx,
            );

            // Draw startup text inside frame
            if frame_progress > 0.5 {
                let text_alpha = ((frame_progress - 0.5) * 2.0).min(1.0);
                let line_height = 22.0;
                let font_size = 12.0;
                let padding = 16.0;
                let text_area_x = frame_x + bounds.origin.x + padding;
                let text_area_y = frame_y + bounds.origin.y + padding;
                let text_area_w = frame_w - padding * 2.0;
                let text_area_h = frame_h - padding * 2.0;
                let max_y = text_area_y + text_area_h - line_height;

                let char_width = 7.2;
                let max_chars = (text_area_w / char_width) as usize;

                let mut y = text_area_y;
                for log_line in &self.startup_state.lines {
                    if y > max_y {
                        break; // Stop if we'd overflow
                    }
                    let color = match log_line.status {
                        LogStatus::Pending => Hsla::new(45.0, 0.9, 0.65, text_alpha),
                        LogStatus::Success => Hsla::new(120.0, 0.7, 0.6, text_alpha),
                        LogStatus::Error => Hsla::new(0.0, 0.8, 0.6, text_alpha),
                        LogStatus::Info => Hsla::new(0.0, 0.0, 0.7, text_alpha),
                        LogStatus::Thinking => Hsla::new(270.0, 0.5, 0.6, text_alpha * 0.7),
                    };

                    let prefix = match log_line.status {
                        LogStatus::Pending => "> ",
                        _ => "  ",
                    };

                    let full_text = format!("{}{}", prefix, log_line.text);
                    let wrapped = wrap_text(&full_text, max_chars);

                    for line in wrapped {
                        if y > max_y {
                            break;
                        }
                        let text_run =
                            cx.text
                                .layout(&line, Point::new(text_area_x, y), font_size, color);
                        cx.scene.draw_text(text_run);
                        y += line_height;
                    }
                }
            }
        }
    }

    fn event(&mut self, _: &InputEvent, _: Bounds, _: &mut EventContext) -> EventResult {
        EventResult::Ignored
    }
}
