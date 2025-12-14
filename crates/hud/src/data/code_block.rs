//! CodeBlock - sci-fi styled code display with line numbers.

use wgpui::{Bounds, Hsla, Point, Scene, Size, TextSystem};

use crate::animator::HudAnimator;
use crate::easing;
use crate::frame::FrameCorners;
use crate::theme::hud;

/// Sci-fi styled code block with line numbers.
///
/// Features:
/// - Animated line-by-line reveal
/// - Line numbers with separator
/// - Frame border
/// - Monospace styling
///
/// # Example
///
/// ```ignore
/// let mut code = CodeBlock::new()
///     .content("fn main() {\n    println!(\"Hello\");\n}")
///     .language("rust");
///
/// code.animator_mut().enter();
///
/// // In update:
/// code.tick();
///
/// // In paint:
/// code.paint(bounds, &mut scene, &mut text_system);
/// ```
pub struct CodeBlock {
    lines: Vec<String>,
    language: Option<String>,
    animator: HudAnimator,
    frame: FrameCorners,

    // Animation state
    line_progress: Vec<f32>,

    // Styling
    font_size: f32,
    line_height: f32,
    padding: f32,
    line_number_width: f32,
    stagger_offset: f32,
    show_line_numbers: bool,
}

impl CodeBlock {
    /// Create a new empty code block.
    pub fn new() -> Self {
        Self {
            lines: Vec::new(),
            language: None,
            animator: HudAnimator::new(),
            frame: FrameCorners::new()
                .corner_length(10.0)
                .line_width(1.0)
                .color(hud::FRAME_DIM),
            line_progress: Vec::new(),
            font_size: 12.0,
            line_height: 18.0,
            padding: 12.0,
            line_number_width: 35.0,
            stagger_offset: 1.0,
            show_line_numbers: true,
        }
    }

    /// Set the code content.
    pub fn content(mut self, code: impl Into<String>) -> Self {
        let code = code.into();
        self.lines = code.lines().map(|s| s.to_string()).collect();
        self.line_progress = vec![0.0; self.lines.len()];
        self
    }

    /// Set the language (for display, not syntax highlighting).
    pub fn language(mut self, lang: impl Into<String>) -> Self {
        self.language = Some(lang.into());
        self
    }

    /// Set the font size.
    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    /// Set the line height.
    pub fn line_height(mut self, height: f32) -> Self {
        self.line_height = height;
        self
    }

    /// Show or hide line numbers.
    pub fn show_line_numbers(mut self, show: bool) -> Self {
        self.show_line_numbers = show;
        self
    }

    /// Get the animator.
    pub fn animator(&self) -> &HudAnimator {
        &self.animator
    }

    /// Get mutable animator.
    pub fn animator_mut(&mut self) -> &mut HudAnimator {
        &mut self.animator
    }

    /// Tick animations.
    pub fn tick(&mut self) {
        self.animator.tick();
        self.frame.tick();

        // Sync frame with our animator
        if self.animator.state().is_visible() && !self.frame.animator().state().is_visible() {
            self.frame.animator_mut().enter();
        }

        let parent_progress = self.animator.progress();

        // Animate lines with stagger based on parent progress
        let speed = 0.2;
        for (i, progress) in self.line_progress.iter_mut().enumerate() {
            // Stagger: each line starts later based on parent progress
            let stagger_threshold = i as f32 * 0.05; // 5% offset per line
            let target = if parent_progress > stagger_threshold {
                ((parent_progress - stagger_threshold) / (1.0 - stagger_threshold)).min(1.0)
            } else {
                0.0
            };

            if *progress < target {
                *progress = (*progress + speed).min(target);
            } else if *progress > target {
                *progress = (*progress - speed).max(target);
            }
        }
    }

    /// Calculate preferred size.
    pub fn preferred_size(&self) -> Size {
        let content_height = self.lines.len() as f32 * self.line_height;
        let height = content_height + self.padding * 2.0;
        Size::new(400.0, height)
    }

    /// Paint the code block.
    pub fn paint(&self, bounds: Bounds, scene: &mut Scene, text_system: &mut TextSystem) {
        let parent_progress = self.animator.progress();
        if parent_progress <= 0.0 {
            return;
        }

        // Draw frame
        self.frame.paint(bounds, scene);

        // Draw background
        let bg_color = Hsla::new(0.0, 0.0, 0.0, 0.4 * parent_progress);
        scene.draw_quad(wgpui::Quad::new(bounds).with_background(bg_color));

        // Calculate content area
        let content_x = bounds.origin.x + self.padding;
        let content_y = bounds.origin.y + self.padding;
        let code_x = if self.show_line_numbers {
            content_x + self.line_number_width
        } else {
            content_x
        };

        // Draw line number separator
        if self.show_line_numbers && parent_progress > 0.0 {
            let separator_x = content_x + self.line_number_width - 8.0;
            let separator_height = self.lines.len() as f32 * self.line_height;

            scene.draw_quad(
                wgpui::Quad::new(Bounds::new(
                    separator_x,
                    content_y,
                    1.0,
                    separator_height * parent_progress,
                ))
                .with_background(Hsla::new(
                    hud::FRAME_DIM.h,
                    hud::FRAME_DIM.s,
                    hud::FRAME_DIM.l,
                    hud::FRAME_DIM.a * 0.5 * parent_progress,
                ))
            );
        }

        // Draw language label if present
        if let Some(lang) = &self.language {
            let label_x = bounds.origin.x + bounds.size.width - self.padding - 50.0;
            let label_y = bounds.origin.y + 4.0;
            let label_color = Hsla::new(
                hud::TEXT_MUTED.h,
                hud::TEXT_MUTED.s,
                hud::TEXT_MUTED.l,
                hud::TEXT_MUTED.a * 0.5 * parent_progress,
            );
            let label_run = text_system.layout(
                &lang.to_uppercase(),
                Point::new(label_x, label_y),
                9.0,
                label_color,
            );
            scene.draw_text(label_run);
        }

        // Draw lines
        for (i, line) in self.lines.iter().enumerate() {
            let line_progress = self.line_progress.get(i).copied().unwrap_or(0.0);
            if line_progress <= 0.0 {
                continue;
            }

            let eased = easing::ease_out_expo(line_progress);
            let line_y = content_y + i as f32 * self.line_height;

            // Draw line number
            if self.show_line_numbers {
                let line_num = format!("{}", i + 1);
                let num_color = Hsla::new(
                    hud::TEXT_MUTED.h,
                    hud::TEXT_MUTED.s,
                    hud::TEXT_MUTED.l,
                    hud::TEXT_MUTED.a * 0.6 * eased,
                );
                let num_run = text_system.layout(
                    &line_num,
                    Point::new(content_x + 5.0, line_y),
                    self.font_size,
                    num_color,
                );
                scene.draw_text(num_run);
            }

            // Draw code line with slide effect
            let slide_offset = 10.0 * (1.0 - eased);
            let code_color = Hsla::new(
                hud::TEXT.h,
                hud::TEXT.s,
                hud::TEXT.l,
                hud::TEXT.a * eased,
            );
            let code_run = text_system.layout(
                line,
                Point::new(code_x + slide_offset, line_y),
                self.font_size,
                code_color,
            );
            scene.draw_text(code_run);
        }
    }
}

impl Default for CodeBlock {
    fn default() -> Self {
        Self::new()
    }
}
